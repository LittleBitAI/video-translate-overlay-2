//! Decoding on every OS (symphonia, and libopus for Opus): MP4/M4A, AAC (ADTS),
//! MP3, WebM; and MPEG-TS, whose audio is unwrapped here (symphonia has no TS
//! reader). Audio is placed by its timestamps, so a gap stays a gap.

use crate::audio::{Resampler, MAX_SAMPLES, RATE, TOO_LONG};
use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CodecRegistry, DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

const TS_PACKET: usize = 188;

/// An MPEG-TS file: sync bytes at the first two packets.
pub fn is_ts(path: &Path) -> bool {
    let mut head = [0u8; TS_PACKET + 1];
    std::fs::File::open(path).is_ok_and(|f| std::io::Read::read_exact(&mut &f, &mut head).is_ok()) && head[0] == 0x47 && head[TS_PACKET] == 0x47
}

pub fn decode(path: &Path) -> Result<Vec<i16>, String> {
    let ext = path.extension().and_then(|e| e.to_str()).map(str::to_string);
    let sidecar = crate::media::sections_path(path);
    if sidecar.exists() {
        let index = std::fs::read_to_string(&sidecar).map_err(|e| format!("read sections: {e}"))?;
        return decode_sections(path, &index, ext.as_deref());
    }
    if is_ts(path) {
        let len = std::fs::metadata(path).map_err(|e| format!("read audio: {e}"))?.len();
        return decode_ts(&|| Ranges::open(path, vec![(0, len)]));
    }
    decode_edited(path, ext.as_deref())
}

/// Byte ranges of one file read as one stream (an HLS section: its init
/// section, then its segments), straight from the file: a download is never
/// held in memory, however long the lecture or large its video.
struct Ranges {
    f: std::fs::File,
    parts: Vec<(u64, u64)>,
    len: u64,
    pos: u64,
    /// Where the file is, when it is known: a read that follows on needs no seek.
    at: Option<u64>,
}

impl Ranges {
    /// `parts`: `(offset, length)` in the file, in order.
    fn open(path: &Path, parts: Vec<(u64, u64)>) -> Result<Ranges, String> {
        let f = std::fs::File::open(path).map_err(|e| format!("open audio: {e}"))?;
        let parts: Vec<(u64, u64)> = parts.into_iter().filter(|p| p.1 > 0).collect();
        Ok(Ranges { len: parts.iter().map(|p| p.1).sum(), f, parts, pos: 0, at: None })
    }
}

impl std::io::Read for Ranges {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        use std::io::{Seek, SeekFrom};
        let mut base = 0;
        for &(off, n) in &self.parts {
            if self.pos < base + n {
                let within = self.pos - base;
                if self.at != Some(off + within) {
                    self.f.seek(SeekFrom::Start(off + within))?;
                }
                let want = (buf.len() as u64).min(n - within) as usize;
                let got = self.f.read(&mut buf[..want])?;
                self.pos += got as u64;
                self.at = Some(off + within + got as u64);
                return Ok(got);
            }
            base += n;
        }
        Ok(0)
    }
}

impl std::io::Seek for Ranges {
    fn seek(&mut self, to: std::io::SeekFrom) -> std::io::Result<u64> {
        let to = match to {
            std::io::SeekFrom::Start(n) => Some(n),
            std::io::SeekFrom::End(d) => self.len.checked_add_signed(d),
            std::io::SeekFrom::Current(d) => self.pos.checked_add_signed(d),
        };
        self.pos = to.ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "seek before the start"))?;
        Ok(self.pos)
    }
}

impl MediaSource for Ranges {
    fn is_seekable(&self) -> bool {
        true
    }
    fn byte_len(&self) -> Option<u64> {
        Some(self.len)
    }
}

/// Places `pcm` at sample `at`: what came before is cut or padded to end there.
/// `exact` cuts too; otherwise only a gap is padded and overlapping audio runs on.
fn place(out: &mut Vec<i16>, at: usize, pcm: Vec<i16>, exact: bool) -> Result<(), String> {
    if at.saturating_add(pcm.len()) > MAX_SAMPLES || out.len().saturating_add(pcm.len()) > MAX_SAMPLES {
        return Err(TOO_LONG.into());
    }
    // The usual case, one piece from 0: kept as it is, not copied.
    if out.is_empty() && at == 0 {
        *out = pcm;
        return Ok(());
    }
    if at > out.len() || exact {
        out.resize(at, 0);
    }
    out.extend(pcm);
    Ok(())
}

/// Every run of a TS stream decoded; the error only when none decoded at all, or the audio is too long.
fn collect(pieces: impl Iterator<Item = (usize, Result<Vec<i16>, String>)>) -> Result<Vec<i16>, String> {
    let (mut out, mut failed) = (Vec::new(), None);
    for (at, pcm) in pieces {
        match pcm {
            Ok(pcm) => place(&mut out, at, pcm, false)?,
            Err(e) if e == TOO_LONG => return Err(e),
            // A run of damaged data loses its own seconds, not the lecture.
            Err(e) => failed = Some(e),
        }
    }
    match failed {
        Some(e) if out.is_empty() => Err(e),
        _ => Ok(out),
    }
}

/// An MPEG-TS stream (`open` gives it from its start, as often as asked), read
/// twice and never whole: once for its audio stream and runs ([`ts_plan`]), then
/// once per run, its audio handed to the decoder as it is read ([`TsAudio`]).
/// Each run is placed at its time: a gap in the audio stays a gap.
fn decode_ts(open: &dyn Fn() -> Result<Ranges, String>) -> Result<Vec<i16>, String> {
    use std::io::{BufReader, Seek, SeekFrom};
    let plan = ts_plan(&mut BufReader::with_capacity(1 << 18, open()?))?;
    let pieces = plan.runs.iter().enumerate().map(|(i, r)| {
        let end = plan.runs.get(i + 1).map_or(plan.es_len, |n| n.es);
        let pcm = open().and_then(|mut src| {
            src.seek(SeekFrom::Start(r.file)).map_err(|e| format!("read audio: {e}"))?;
            let audio = TsAudio { r: BufReader::with_capacity(1 << 18, src), pid: plan.pid, left: end - r.es, buf: Vec::new(), at: 0 };
            decode_stream(Box::new(symphonia::core::io::ReadOnlySource::new(audio)), Some(plan.ext), Some(0.0), &[])
        });
        ((r.at.max(0) as u64 * RATE as u64 / PTS_HZ) as usize, pcm)
    });
    collect(pieces)
}

/// An HLS download in sections ([`crate::media::sections_path`]): each
/// section's timestamps start afresh, so it is decoded on its own after its own
/// init section and placed where the playlist's EXTINFs put it, as players do.
/// A section that does not decode fails the whole file: a lecture is not
/// written with a stretch of it silently missing.
fn decode_sections(path: &Path, index: &str, ext: Option<&str>) -> Result<Vec<i16>, String> {
    let size = std::fs::metadata(path).map_err(|e| format!("read audio: {e}"))?.len();
    let bad = || "sections index does not match the audio".to_string();
    let mut out = Vec::new();
    for (i, l) in index.lines().enumerate() {
        let f: Vec<&str> = l.split(' ').collect();
        let [from, to, at, init_from, init_to] = f[..] else { return Err(bad()) };
        let n = |s: &str| s.parse::<u64>().map_err(|_| bad());
        let (from, to, init_from, init_to) = (n(from)?, n(to)?, n(init_from)?, n(init_to)?);
        let at: f64 = at.parse().ok().filter(|a: &f64| a.is_finite() && *a >= 0.0).ok_or_else(bad)?;
        if from > to || to > size || init_from > init_to || init_to > size {
            return Err(bad());
        }
        let parts = vec![(init_from, init_to - init_from), (from, to - from)];
        let open = || Ranges::open(path, parts.clone());
        let mut head = [0u8; TS_PACKET + 1];
        let ts = std::io::Read::read_exact(&mut open()?, &mut head).is_ok() && head[0] == 0x47 && head[TS_PACKET] == 0x47;
        // fMP4: each fragment at its tfdt, the section starting where its earliest
        // track does, as players start it; other sections (packed audio) have
        // one stream, whose first timestamp starts the section.
        let timing = if ts { None } else { fmp4_timing(path, (init_from, init_to), (from, to)) };
        let pcm = if ts {
            decode_ts(&open)
        } else {
            let (start, jumps) = timing.map_or((None, Vec::new()), |t| (Some(t.edit - t.earliest), t.jumps));
            decode_stream(Box::new(open()?), ext, start, &jumps)
        }
            .map_err(|e| if e == TOO_LONG { e } else { format!("section {} of the playlist: {e}", i + 1) })?;
        place(&mut out, (at * RATE as f64).round().min(usize::MAX as f64) as usize, pcm, true)?;
    }
    Ok(out)
}

/// Symphonia's codecs, plus Opus (WebM audio) through libopus.
fn codecs() -> &'static CodecRegistry {
    static CODECS: std::sync::OnceLock<CodecRegistry> = std::sync::OnceLock::new();
    CODECS.get_or_init(|| {
        let mut r = CodecRegistry::new();
        symphonia::default::register_enabled_codecs(&mut r);
        r.register_all::<symphonia_adapter_libopus::OpusDecoder>();
        r
    })
}

/// Decodes the first audio track with every packet at its timestamp plus
/// `offset` seconds: a jump in the timestamps (an MP4 sample that lasts 10 s,
/// a WebM block after a pause) is silence, not pulled-in speech, and audio
/// placed before 0 (cut by an edit list) is dropped. `offset` `None`: the
/// first packet plays at 0 (an HLS section, whose timestamps start anywhere).
/// `jumps`: ticks added to the track's own timestamps from where each applies
/// (an fMP4 section's fragments, [`Fmp4Timing::jumps`]).
fn decode_stream(src: Box<dyn MediaSource>, ext: Option<&str>, offset: Option<f64>, jumps: &[(u64, i64)]) -> Result<Vec<i16>, String> {
    let mut hint = Hint::new();
    if let Some(e) = ext {
        hint.with_extension(e);
    }
    let mss = MediaSourceStream::new(src, Default::default());
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| format!("open media: {e}"))?;
    let mut format = probed.format;
    let track = format.tracks().iter().find(|t| t.codec_params.codec != CODEC_TYPE_NULL).ok_or("no audio stream")?;
    let (id, tb) = (track.id, track.codec_params.time_base);
    let mut decoder = codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("no decoder for this audio: {e}"))?;
    let mut rs: Option<Resampler> = None;
    let mut buf: Option<SampleBuffer<f32>> = None;
    let mut first: Option<i64> = None;
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            // The end of the stream (a truncated download included) ends decoding.
            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(Error::ResetRequired) => break,
            Err(e) => return Err(format!("read: {e}")),
        };
        if packet.track_id() != id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(audio) => {
                let spec = *audio.spec();
                let channels = spec.channels.count();
                let rate = u64::from(spec.rate);
                // As for WAV: a rate no real audio has would multiply every sample.
                if !(4_000..=384_000).contains(&rate) {
                    return Err(format!("{rate} Hz is not audio this app decodes"));
                }
                let rs = rs.get_or_insert_with(|| Resampler::new(spec.rate as usize));
                let ts = (i128::from(packet.ts) + i128::from(jump_at(jumps, packet.ts))).max(0) as u128;
                let at = tb.map_or(0, |tb| (ts * u128::from(tb.numer) * u128::from(rate) / u128::from(tb.denom.max(1))) as i64);
                // Where this packet plays, in source frames; before 0 it was edited out.
                let shift = match offset {
                    Some(o) => (o * rate as f64).round() as i64,
                    None => -*first.get_or_insert(at),
                };
                let pos = shift + at;
                if buf.as_ref().is_none_or(|b| b.capacity() < audio.capacity() * channels) {
                    buf = Some(SampleBuffer::new(audio.capacity() as u64, spec));
                }
                let b = buf.as_mut().unwrap();
                b.copy_interleaved_ref(audio);
                let cut = usize::try_from(-pos).unwrap_or(0).saturating_mul(channels);
                if let Some(kept) = b.samples().get(cut..).filter(|s| !s.is_empty()) {
                    rs.pad_to(pos.max(0) as u64);
                    rs.push_frames(kept, channels);
                }
                if rs.over {
                    break;
                }
            }
            // One damaged frame does not end the lecture.
            Err(Error::DecodeError(_)) => continue,
            Err(e) => return Err(format!("decode: {e}")),
        }
    }
    rs.ok_or_else(|| "no audio decoded".to_string())?.finish()
}

/// The boxes inside `b`: `(type, body)`.
fn boxes(mut b: &[u8]) -> impl Iterator<Item = (&[u8], &[u8])> {
    std::iter::from_fn(move || {
        let (size, hdr) = crate::media::box_header(b)?;
        let size = if size == 0 { b.len() as u64 } else { size };
        let (this, rest) = b.split_at_checked(usize::try_from(size).ok()?)?;
        b = rest;
        Some((&this[4..8], this.get(hdr as usize..)?))
    })
}

fn child<'a>(b: &'a [u8], kind: &[u8; 4]) -> Option<&'a [u8]> {
    boxes(b).find(|(k, _)| k == kind).map(|(_, body)| body)
}

/// The edit list of an MP4's audio track (empty when it has none), which maps
/// the track's own timeline onto the movie's (the player's). Symphonia's
/// reader does not apply edit lists, so without it a late-starting track would
/// be pulled to 0, a trimmed one would play its cut part, and a skipped part
/// would push every later word late.
fn mp4_edits(moov: &[u8]) -> Option<Vec<Edit>> {
    let movie = timescale(child(moov, b"mvhd")?)?;
    let trak = traks(moov).find(|t| is_sound(t))?;
    track_edits(trak, movie)
}

/// An MP4's `moov` body and where the box ends in the file; `None` when the
/// file is not MP4 (its first box is not `ftyp`).
fn mp4_moov(path: &Path) -> Option<(Vec<u8>, u64)> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let mut off = 0u64;
    loop {
        f.seek(SeekFrom::Start(off)).ok()?;
        let mut h = Vec::with_capacity(16);
        (&mut f).take(16).read_to_end(&mut h).ok()?;
        let (size, hdr) = crate::media::box_header(&h)?;
        match &h[4..8] {
            b"ftyp" if off == 0 => {}
            _ if off == 0 => return None, // not MP4
            b"moov" => {
                let mut body = Vec::new();
                f.seek(SeekFrom::Start(off + hdr)).ok()?;
                (&mut f).take(size.saturating_sub(hdr).min(64 << 20)).read_to_end(&mut body).ok()?;
                return Some((body, off.checked_add(size)?));
            }
            _ => {}
        }
        if size < hdr {
            return None;
        }
        off += size;
    }
}

/// The ticks of a track's samples in the `moov` itself (none in fMP4's init),
/// where symphonia starts the track's first fragment.
fn stts_ticks(trak: &[u8]) -> u64 {
    let stts = child(trak, b"mdia").and_then(|m| child(m, b"minf")).and_then(|m| child(m, b"stbl")).and_then(|s| child(s, b"stts"));
    let Some(stts) = stts else { return 0 };
    let n = be32(stts, 4).unwrap_or(0) as usize;
    stts.get(8..).unwrap_or_default().chunks_exact(8).take(n).map(|e| u64::from(be32(e, 0).unwrap_or(0)) * u64::from(be32(e, 4).unwrap_or(0))).sum()
}

/// The timescale of an `mvhd` or `mdhd`, which keep it at the same place.
fn timescale(b: &[u8]) -> Option<f64> {
    let at = if b.first()? == &1 { 20 } else { 12 };
    Some(f64::from(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?).max(1)))
}

fn traks(moov: &[u8]) -> impl Iterator<Item = &[u8]> {
    boxes(moov).filter(|(k, _)| k == b"trak").map(|(_, t)| t)
}

fn is_sound(trak: &[u8]) -> bool {
    child(trak, b"mdia").and_then(|m| child(m, b"hdlr")).and_then(|h| h.get(8..12)) == Some(b"soun")
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// A track's edit list (empty when it has none); `movie` is `mvhd`'s timescale.
fn track_edits(trak: &[u8], movie: f64) -> Option<Vec<Edit>> {
    let media = timescale(child(child(trak, b"mdia")?, b"mdhd")?)?;
    let Some(elst) = child(trak, b"edts").and_then(|e| child(e, b"elst")) else { return Some(Vec::new()) };
    let (v1, count) = (elst.first()? == &1, u32::from_be_bytes(elst.get(4..8)?.try_into().ok()?));
    let size = if v1 { 20 } else { 12 };
    let mut edits = Vec::new();
    for e in elst.get(8..)?.chunks_exact(size).take(count as usize) {
        let (duration, media_time) = if v1 {
            (u64::from_be_bytes(e[..8].try_into().ok()?), i64::from_be_bytes(e[8..16].try_into().ok()?))
        } else {
            (u64::from(u32::from_be_bytes(e[..4].try_into().ok()?)), i64::from(i32::from_be_bytes(e[4..8].try_into().ok()?)))
        };
        // ponytail: the edit's rate is taken as 1 (a dwell or a speed change is played at normal speed).
        edits.push(Edit { from: (media_time >= 0).then(|| media_time as f64 / media), secs: duration as f64 / movie });
    }
    Some(edits)
}

/// One entry of an MP4 edit list: `secs` of the movie, played from media time
/// `from` (seconds on the track's own timeline), or silence when `None`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Edit {
    from: Option<f64>,
    secs: f64,
}

/// Where a track's media time 0 plays on the movie's timeline, in seconds:
/// after the empty edits that lead, less where its (first) media edit starts.
fn edit_shift(edits: &[Edit]) -> f64 {
    let lead: f64 = edits.iter().take_while(|e| e.from.is_none()).map(|e| e.secs).sum();
    lead - edits.iter().find_map(|e| e.from).unwrap_or(0.0)
}

/// How fragmented MP4 audio is timed ([`fmp4_timing`]).
#[derive(Debug, Default, PartialEq)]
struct Fmp4Timing {
    /// Where the audio track's own time 0 plays on the movie's timeline, in
    /// seconds, by its edit list.
    edit: f64,
    /// Where the earliest track's first fragment plays on the movie's timeline,
    /// in seconds: where an HLS section starts.
    earliest: f64,
    /// Where the audio's fragments are: from the track tick symphonia gives a
    /// fragment's first packet on, the ticks to add to reach its `tfdt`.
    jumps: Vec<(u64, i64)>,
}

/// The ticks to add to a packet at track tick `ts` (see [`Fmp4Timing::jumps`]).
fn jump_at(jumps: &[(u64, i64)], ts: u64) -> i64 {
    jumps.partition_point(|j| j.0 <= ts).checked_sub(1).map_or(0, |i| jumps[i].1)
}

/// The ticks a track fragment's samples last, as symphonia counts them: each
/// run's own durations, or the `tfhd`'s default, or the `trex`'s.
fn traf_ticks(traf: &[u8], trex_default: u32) -> Option<u64> {
    let tfhd = child(traf, b"tfhd")?;
    let flags = be32(tfhd, 0)? & 0xFF_FFFF;
    let at = 8 + if flags & 0x01 != 0 { 8 } else { 0 } + if flags & 0x02 != 0 { 4 } else { 0 };
    let default = if flags & 0x08 != 0 { be32(tfhd, at)? } else { trex_default };
    let mut ticks = 0u64;
    for (_, trun) in boxes(traf).filter(|(k, _)| k == b"trun") {
        let f = be32(trun, 0)? & 0xFF_FFFF;
        let count = be32(trun, 4)?;
        if f & 0x100 == 0 {
            ticks += u64::from(count) * u64::from(default);
            continue;
        }
        let per = [0x100, 0x200, 0x400, 0x800].iter().filter(|b| f & **b != 0).count() * 4;
        let mut at = 8 + if f & 0x01 != 0 { 4 } else { 0 } + if f & 0x04 != 0 { 4 } else { 0 };
        for _ in 0..count {
            ticks += u64::from(be32(trun, at)?);
            at += per;
        }
    }
    Some(ticks)
}

/// How fragmented MP4 audio is timed, an HLS section's (`init` its init
/// section) or a downloaded file's (`init` its `ftyp` and `moov`, `section` the
/// rest); `None` when it is not fragmented MP4. Every fragment plays at its own
/// `tfdt`, shifted by its track's edit list: audio missing for ten seconds while
/// the video runs on stays ten seconds of silence, and audio that starts 2 s
/// after the video is heard 2 s in. Symphonia reads neither `tfdt` nor edit
/// lists, timing each track from the end of its `moov` samples and every
/// fragment straight after the one before, so both are added back. Only the
/// `moof` boxes are read, never the media.
fn fmp4_timing(path: &Path, init: (u64, u64), section: (u64, u64)) -> Option<Fmp4Timing> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let mut read = |at: u64, len: u64| -> Option<Vec<u8>> {
        f.seek(SeekFrom::Start(at)).ok()?;
        let mut b = Vec::new();
        (&mut f).take(len).read_to_end(&mut b).ok()?;
        Some(b)
    };
    let init = read(init.0, (init.1 - init.0).min(64 << 20))?;
    let moov = boxes(&init).find(|(k, _)| k == b"moov")?.1;
    let movie = timescale(child(moov, b"mvhd")?)?;
    // (track ID, timescale, shift by its edits, sound), in the moov's order.
    let tracks: Vec<(u32, f64, f64, bool)> = traks(moov)
        .filter_map(|t| {
            let tkhd = child(t, b"tkhd")?;
            let id = be32(tkhd, if tkhd.first()? == &1 { 20 } else { 12 })?;
            let ts = timescale(child(child(t, b"mdia")?, b"mdhd")?)?;
            Some((id, ts, edit_shift(&track_edits(t, movie).unwrap_or_default()), is_sound(t)))
        })
        .collect();
    let sound = tracks.iter().position(|t| t.3)?;
    let sound_trak = traks(moov).find(|t| is_sound(t))?;
    // The audio's default sample duration in the init's `mvex`.
    let trex_default = child(moov, b"mvex")
        .and_then(|m| boxes(m).filter(|(k, _)| k == b"trex").find(|(_, t)| be32(t, 4) == Some(tracks[sound].0)))
        .and_then(|(_, t)| be32(t, 12))
        .unwrap_or(0);
    let mut first: Vec<Option<f64>> = vec![None; tracks.len()];
    // Where symphonia starts the audio's next fragment.
    let mut played = stts_ticks(sound_trak);
    let mut jumps: Vec<(u64, i64)> = Vec::new();
    let mut off = section.0;
    while off < section.1 {
        let h = read(off, 16)?;
        let (size, hdr) = crate::media::box_header(&h)?;
        let size = if size == 0 { section.1 - off } else { size };
        if size < hdr {
            break;
        }
        if &h[4..8] == b"moof" {
            let moof = read(off + hdr, (size - hdr).min(16 << 20))?;
            for (_, traf) in boxes(&moof).filter(|(k, _)| k == b"traf") {
                let Some(i) = child(traf, b"tfhd").and_then(|t| be32(t, 4)).and_then(|id| tracks.iter().position(|t| t.0 == id)) else { continue };
                let base = child(traf, b"tfdt").and_then(|tfdt| {
                    if tfdt.first() == Some(&1) {
                        tfdt.get(4..12).and_then(|b| b.try_into().ok()).map(u64::from_be_bytes)
                    } else {
                        be32(tfdt, 4).map(u64::from)
                    }
                });
                if let (None, Some(base)) = (first[i], base) {
                    first[i] = Some(base as f64 / tracks[i].1 + tracks[i].2);
                }
                if i != sound {
                    continue;
                }
                if let Some(base) = base {
                    let shift = (i128::from(base) - i128::from(played)).clamp(i64::MIN.into(), i64::MAX.into()) as i64;
                    if shift != jumps.last().map_or(0, |j| j.1) {
                        jumps.push((played, shift));
                    }
                }
                played += traf_ticks(traf, trex_default)?;
            }
        }
        off += size;
    }
    let audio = first[sound]?;
    let earliest = first.iter().flatten().copied().fold(audio, f64::min);
    Some(Fmp4Timing { edit: tracks[sound].2, earliest, jumps })
}

/// Decodes an MP4 (or any other file, whose edit list is empty) on the movie's
/// timeline. With at most one media edit, the edit is an offset applied while
/// decoding; edits that skip or repeat parts build the timeline from the
/// track decoded on its own one. A fragmented MP4 (a `moov` with `mvex`) has
/// each fragment placed at its `tfdt` as well, as in an HLS section.
fn decode_edited(path: &Path, ext: Option<&str>) -> Result<Vec<i16>, String> {
    let open = || std::fs::File::open(path).map_err(|e| format!("open audio: {e}"));
    let moov = mp4_moov(path);
    let edits = moov.as_ref().and_then(|(m, _)| mp4_edits(m)).unwrap_or_default();
    let jumps = match &moov {
        Some((m, end)) if child(m, b"mvex").is_some() => {
            let len = std::fs::metadata(path).map_err(|e| format!("read audio: {e}"))?.len();
            fmp4_timing(path, (0, *end), (*end, len)).map(|t| t.jumps).unwrap_or_default()
        }
        _ => Vec::new(),
    };
    if edits.iter().filter(|e| e.from.is_some()).count() <= 1 {
        let lead: f64 = edits.iter().take_while(|e| e.from.is_none()).map(|e| e.secs).sum();
        let media = edits.iter().find(|e| e.from.is_some());
        let mut pcm = decode_stream(Box::new(open()?), ext, Some(edit_shift(&edits)), &jumps)?;
        // The movie ends where the edits do: the media edit's length (track
        // audio past it is not played), then any empty edits after it (silence).
        // A media edit of length 0 runs to the end of the track.
        if let Some(e) = media.filter(|e| e.secs > 0.0) {
            let samples = |secs: f64| (secs * RATE as f64).round() as usize;
            let end = samples(edits.iter().map(|e| e.secs).sum());
            if end > MAX_SAMPLES {
                return Err(TOO_LONG.into());
            }
            pcm.truncate(samples(lead + e.secs));
            pcm.resize(end, 0);
        }
        return Ok(pcm);
    }
    // ponytail: a second copy of the PCM (about 115 MB for an hour) for files with several edits.
    let track = decode_stream(Box::new(open()?), ext, Some(0.0), &jumps)?;
    apply_edits(&track, &edits)
}

/// The movie's timeline out of `track` (16 kHz, on the track's own timeline);
/// [`TOO_LONG`] when the edits add up to more than [`MAX_SAMPLES`].
fn apply_edits(track: &[i16], edits: &[Edit]) -> Result<Vec<i16>, String> {
    let at = |secs: f64| (secs * RATE as f64).round() as usize;
    let mut out = Vec::new();
    for e in edits {
        let n = at(e.secs);
        let (start, end) = match e.from {
            None => (0, 0),
            Some(from) => {
                let start = at(from).min(track.len());
                // A length of 0 runs to the end of the track.
                (start, if n == 0 { track.len() } else { start.saturating_add(n).min(track.len()) })
            }
        };
        let len = n.max(end - start);
        if out.len().saturating_add(len) > MAX_SAMPLES {
            return Err(TOO_LONG.into());
        }
        out.extend_from_slice(&track[start..end]);
        out.resize(out.len() + len - (end - start), 0);
    }
    Ok(out)
}

fn pid(hi: u8, lo: u8) -> u16 {
    u16::from(hi & 0x1F) << 8 | u16::from(lo)
}

/// A PSI section after its pointer field: `(section bytes up to the CRC)`.
fn section(payload: &[u8]) -> Option<&[u8]> {
    let s = payload.get(1 + usize::from(*payload.first()?)..)?;
    let len = usize::from(s.get(1)? & 0x0F) << 8 | usize::from(*s.get(2)?);
    s.get(..(3 + len).checked_sub(4)?)
}

/// PES timestamps tick at 90 kHz on a 33-bit clock.
const PTS_HZ: u64 = 90_000;

/// The PTS in a PES header, if it carries one.
fn pts(pes: &[u8]) -> Option<u64> {
    if pes.len() < 14 || pes[7] & 0x80 == 0 {
        return None;
    }
    let b = |i: usize| u64::from(pes[i]);
    Some((b(9) >> 1 & 7) << 30 | b(10) << 22 | (b(11) >> 1) << 15 | b(12) << 7 | b(13) >> 1)
}

/// How far `to` is past `from` on the 33-bit clock (negative when before; wraps).
fn ahead(from: u64, to: u64) -> i64 {
    let d = to.wrapping_sub(from) & ((1 << 33) - 1);
    if d >= 1 << 32 {
        d as i64 - (1 << 33)
    } else {
        d as i64
    }
}

/// The length and duration (90 kHz ticks) of the ADTS or MPEG audio frame at the start of `h`.
fn frame(h: &[u8], ext: &str) -> Option<(usize, u64)> {
    if h.len() < 7 || h[0] != 0xFF {
        return None;
    }
    if ext == "aac" {
        const RATES: [u64; 13] = [96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350];
        if h[1] & 0xF6 != 0xF0 {
            return None;
        }
        let rate = *RATES.get(usize::from(h[2] >> 2 & 0x0F))?;
        let len = usize::from(h[3] & 3) << 11 | usize::from(h[4]) << 3 | usize::from(h[5] >> 5);
        let blocks = u64::from(h[6] & 3) + 1;
        return (len >= 7).then_some((len, 1024 * blocks * PTS_HZ / rate));
    }
    // MPEG audio, layers I-III, versions 1, 2 and 2.5.
    let (version, layer) = (h[1] >> 3 & 3, h[1] >> 1 & 3);
    if h[1] & 0xE0 != 0xE0 || version == 1 || layer == 0 {
        return None;
    }
    let v1 = version == 3;
    let rate = [[11025, 12000, 8000], [0; 3], [22050, 24000, 16000], [44100, 48000, 32000]][usize::from(version)]
        .get(usize::from(h[2] >> 2 & 3))
        .copied()
        .filter(|&r| r > 0)?;
    const KBPS: [[u64; 15]; 5] = [
        [0, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448],
        [0, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384],
        [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320],
        [0, 32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256],
        [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160],
    ];
    let row = match (v1, layer) {
        (true, 3) => 0,
        (true, 2) => 1,
        (true, _) => 2,
        (false, 3) => 3,
        _ => 4,
    };
    let kbps = *KBPS[row].get(usize::from(h[2] >> 4)).filter(|&&k| k > 0)?;
    let pad = u64::from(h[2] >> 1 & 1);
    let (samples, len) = match layer {
        3 => (384, (12 * kbps * 1000 / rate + pad) * 4),
        2 => (1152, 144 * kbps * 1000 / rate + pad),
        _ if v1 => (1152, 144 * kbps * 1000 / rate + pad),
        _ => (576, 72 * kbps * 1000 / rate + pad),
    };
    (len >= 4).then_some((len as usize, samples * PTS_HZ / rate))
}

/// The audio frames in an elementary stream fed to it piece by piece, and how
/// long they play (90 kHz ticks): only a frame header is ever held.
struct Frames {
    ext: &'static str,
    head: Vec<u8>,
    skip: usize,
    ticks: u64,
}

impl Frames {
    fn feed(&mut self, mut b: &[u8]) {
        while !b.is_empty() {
            if self.skip > 0 {
                let n = self.skip.min(b.len());
                self.skip -= n;
                b = &b[n..];
                continue;
            }
            self.head.push(b[0]);
            b = &b[1..];
            // Seven bytes decide an ADTS or an MPEG audio header.
            while self.head.len() >= 7 {
                match frame(&self.head, self.ext) {
                    Some((len, ticks)) => {
                        self.ticks += ticks;
                        if len >= self.head.len() {
                            self.skip = len - self.head.len();
                            self.head.clear();
                        } else {
                            self.head.drain(..len);
                        }
                    }
                    // Not a frame here: look one byte on.
                    None => {
                        self.head.remove(0);
                    }
                }
            }
        }
    }
}

/// Where an audio run starts: the TS packet it starts in, its byte offset in
/// the audio stream, and its 90 kHz ticks on the presentation's timeline.
struct Run {
    file: u64,
    es: u64,
    at: i64,
}

/// What [`ts_plan`] found: the audio stream, its runs, and its length in bytes.
struct TsPlan {
    pid: u16,
    ext: &'static str,
    runs: Vec<Run>,
    es_len: u64,
}

/// Each 188-byte packet of `r` with its offset (one without its sync byte is passed over).
fn packets(r: &mut impl std::io::Read, mut each: impl FnMut(u64, &[u8])) -> Result<(), String> {
    let mut p = [0u8; TS_PACKET];
    let mut off = 0u64;
    loop {
        match r.read_exact(&mut p) {
            Ok(()) => {}
            // The end, a cut-short last packet included.
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(e) => return Err(format!("read audio: {e}")),
        }
        if p[0] == 0x47 {
            each(off, &p);
        }
        off += TS_PACKET as u64;
    }
}

/// A packet's PID, whether it starts a PES or section, and its payload.
fn split(p: &[u8]) -> Option<(u16, bool, &[u8])> {
    let control = (p[3] >> 4) & 3;
    let at = if control & 2 != 0 { 5 + usize::from(p[4]) } else { 4 };
    if control & 1 == 0 || at >= TS_PACKET {
        return None;
    }
    Some((pid(p[1], p[2]), p[1] & 0x40 != 0, &p[at..]))
}

/// The audio bytes of a PES packet: after its header on the first packet; `None` for a start that is no PES.
fn audio_bytes(start: bool, payload: &[u8]) -> Option<&[u8]> {
    if !start {
        return Some(payload);
    }
    // PES header: 00 00 01, stream id, length, two flag bytes, header length.
    if payload.len() < 9 || payload[..3] != [0, 0, 1] {
        return None;
    }
    Some(payload.get(9 + usize::from(payload[8])..).unwrap_or(&[]))
}

/// The first pass over an MPEG-TS stream: its first audio stream (ADTS AAC or
/// MPEG audio) and where its runs start. The timeline starts at the earliest
/// PTS of any stream, as players count it; a run starts wherever a PES's PTS
/// is more than 100 ms past the end of the audio before it, so the gap is kept.
fn ts_plan(r: &mut impl std::io::Read) -> Result<TsPlan, String> {
    const TOLERANCE: i64 = PTS_HZ as i64 / 10;
    let (mut pmt, mut audio): (Option<u16>, Option<(u16, &'static str)>) = (None, None);
    // The first PTS of every other audio or video stream, for where the timeline starts.
    let mut firsts: Vec<(u16, u64)> = Vec::new();
    let mut first_audio: Option<u64> = None;
    let mut frames: Option<Frames> = None;
    let mut runs = Vec::new();
    let mut es_len = 0u64;
    // The clock: the PTS before, the time now, and where the current run started (time, audio ticks).
    let (mut prev, mut t, mut run_at, mut run_from): (Option<u64>, i64, i64, i64) = (None, 0, 0, 0);
    packets(r, |off, p| {
        let Some((id, start, payload)) = split(p) else { return };
        if id == 0 && start && pmt.is_none() {
            pmt = pat_pmt(payload);
        } else if Some(id) == pmt && start && audio.is_none() {
            audio = section(payload).and_then(pmt_audio);
            if let Some((_, ext)) = audio {
                frames = Some(Frames { ext, head: Vec::new(), skip: 0, ticks: 0 });
                // Audio from the packet after the PMT that named it.
                runs.push(Run { file: off + TS_PACKET as u64, es: 0, at: 0 });
            }
        } else if let Some(f) = frames.as_mut().filter(|_| audio.is_some_and(|(a, _)| a == id)) {
            let Some(bytes) = audio_bytes(start, payload) else { return };
            if let Some(p) = start.then(|| pts(payload)).flatten() {
                first_audio.get_or_insert(p);
                if let Some(q) = prev {
                    t += ahead(q, p);
                    let expected = run_at + (f.ticks as i64 - run_from);
                    if t > expected + TOLERANCE {
                        runs.push(Run { file: off, es: es_len, at: t });
                        (run_at, run_from) = (t, f.ticks as i64);
                    } else if t < expected - TOLERANCE {
                        // The clock jumped back (a discontinuity): carry on where the audio is.
                        t = expected;
                    }
                }
                prev = Some(p);
            }
            f.feed(bytes);
            es_len += bytes.len() as u64;
        } else if start && payload.len() > 3 && payload[..3] == [0, 0, 1] && (0xC0..=0xEF).contains(&payload[3]) {
            if let (Some(t), false) = (pts(payload), firsts.iter().any(|f| f.0 == id)) {
                firsts.push((id, t));
            }
        }
    })?;
    let Some((pid, ext)) = audio.filter(|_| es_len > 0) else {
        return Err("no AAC or MPEG audio in this MPEG-TS".into());
    };
    // How long after the earliest stream the audio starts.
    let lead = first_audio.map_or(0, |a| firsts.iter().map(|f| -ahead(a, f.1)).max().unwrap_or(0).max(0));
    for r in &mut runs {
        r.at += lead;
    }
    Ok(TsPlan { pid, ext, runs, es_len })
}

/// The second pass: one run's audio bytes, read out of the TS packets as the decoder asks.
struct TsAudio<R> {
    r: R,
    pid: u16,
    left: u64,
    buf: Vec<u8>,
    at: usize,
}

impl<R: std::io::Read> std::io::Read for TsAudio<R> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        loop {
            if self.left == 0 || out.is_empty() {
                return Ok(0);
            }
            if self.at < self.buf.len() {
                let n = out.len().min(self.buf.len() - self.at).min(usize::try_from(self.left).unwrap_or(usize::MAX));
                out[..n].copy_from_slice(&self.buf[self.at..self.at + n]);
                self.at += n;
                self.left -= n as u64;
                return Ok(n);
            }
            let mut p = [0u8; TS_PACKET];
            match self.r.read_exact(&mut p) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(0),
                Err(e) => return Err(e),
            }
            if p[0] != 0x47 {
                continue;
            }
            let Some((id, start, payload)) = split(&p) else { continue };
            if id != self.pid {
                continue;
            }
            if let Some(bytes) = audio_bytes(start, payload) {
                self.buf.clear();
                self.buf.extend_from_slice(bytes);
                self.at = 0;
            }
        }
    }
}

/// PAT: the first program's PMT PID.
fn pat_pmt(payload: &[u8]) -> Option<u16> {
    section(payload)?.get(8..)?.chunks_exact(4).find(|e| e[0] != 0 || e[1] != 0).map(|e| pid(e[2], e[3]))
}

/// The first audio stream a PMT section lists (ADTS AAC or MPEG audio): its PID and extension.
fn pmt_audio(s: &[u8]) -> Option<(u16, &'static str)> {
    let info = usize::from(s.get(10)? & 0x0F) << 8 | usize::from(*s.get(11)?);
    let mut j = 12 + info;
    while j + 5 <= s.len() {
        let kind = match s[j] {
            0x0F => Some("aac"),
            0x03 | 0x04 => Some("mp3"),
            _ => None,
        };
        if let Some(k) = kind {
            return Some((pid(s[j + 1], s[j + 2]), k));
        }
        j += 5 + (usize::from(s[j + 3] & 0x0F) << 8 | usize::from(s[j + 4]));
    }
    None
}

/// Whether the first program of an MPEG-TS head lists an audio stream this
/// app decodes; `None` when its PMT is not in `data`.
pub(crate) fn ts_has_audio(data: &[u8]) -> Option<bool> {
    let mut pmt = None;
    for p in data.chunks_exact(TS_PACKET).filter(|p| p[0] == 0x47 && p[1] & 0x40 != 0) {
        let control = (p[3] >> 4) & 3;
        let at = if control & 2 != 0 { 5 + usize::from(p[4]) } else { 4 };
        if control & 1 == 0 || at >= TS_PACKET {
            continue;
        }
        let (id, payload) = (pid(p[1], p[2]), &p[at..]);
        if id == 0 && pmt.is_none() {
            pmt = pat_pmt(payload);
        } else if Some(id) == pmt {
            if let Some(s) = section(payload) {
                return Some(pmt_audio(s).is_some());
            }
        }
    }
    None
}

#[cfg(test)]
#[path = "decode_edit_tests.rs"]
mod edit_tests;

#[cfg(test)]
#[path = "decode_fmp4_lead_tests.rs"]
mod fmp4_lead_tests;

#[cfg(test)]
#[path = "decode_fmp4_gap_tests.rs"]
mod fmp4_gap_tests;

#[cfg(test)]
#[path = "decode_fmp4_direct_tests.rs"]
mod fmp4_direct_tests;
