//! Decoding on every OS (symphonia, and libopus for Opus): MP4/M4A, AAC (ADTS),
//! MP3, WebM; and MPEG-TS, whose audio is unwrapped here (symphonia has no TS
//! reader). Audio is placed by its timestamps, so a gap stays a gap.

use crate::audio::{Resampler, MAX_SAMPLES, RATE, TOO_LONG};
use std::io::Cursor;
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
        // ponytail: the TS audio is gathered in memory (about 60 MB for an hour of 128 kbit/s AAC).
        return decode_ts(&std::fs::read(path).map_err(|e| format!("read audio: {e}"))?);
    }
    decode_edited(path, ext.as_deref())
}

/// Places `pcm` at sample `at`: what came before is cut or padded to end there.
/// `exact` cuts too; otherwise only a gap is padded and overlapping audio runs on.
fn place(out: &mut Vec<i16>, at: usize, pcm: Vec<i16>, exact: bool) -> Result<(), String> {
    if at.saturating_add(pcm.len()) > MAX_SAMPLES || out.len().saturating_add(pcm.len()) > MAX_SAMPLES {
        return Err(TOO_LONG.into());
    }
    if at > out.len() || exact {
        out.resize(at, 0);
    }
    out.extend(pcm);
    Ok(())
}

/// Every piece decoded; the error only when none decoded at all, or the audio is too long.
fn collect(pieces: impl Iterator<Item = (usize, Result<Vec<i16>, String>)>, exact: bool) -> Result<Vec<i16>, String> {
    let (mut out, mut failed) = (Vec::new(), None);
    for (at, pcm) in pieces {
        match pcm {
            Ok(pcm) => place(&mut out, at, pcm, exact)?,
            Err(e) if e == TOO_LONG => return Err(e),
            // A piece of damaged data loses its own seconds, not the lecture.
            Err(e) => failed = Some(e),
        }
    }
    match failed {
        Some(e) if out.is_empty() => Err(e),
        _ => Ok(out),
    }
}

fn decode_ts(data: &[u8]) -> Result<Vec<i16>, String> {
    let (es, ext, runs) = ts_audio(data)?;
    // Each run is decoded on its own and placed at its time: a gap in the audio stays a gap.
    let pieces = runs.iter().enumerate().map(|(i, &(start, at))| {
        let end = runs.get(i + 1).map_or(es.len(), |r| r.0);
        ((at * RATE as u64 / PTS_HZ) as usize, decode_stream(Box::new(Cursor::new(es[start..end].to_vec())), Some(ext), 0.0))
    });
    collect(pieces, false)
}

/// An HLS download with `EXT-X-DISCONTINUITY` ([`crate::media::sections_path`]):
/// each section's timestamps start afresh, so it is decoded on its own (after
/// the init section) and placed where the playlist's EXTINFs put it, as players do.
fn decode_sections(path: &Path, index: &str, ext: Option<&str>) -> Result<Vec<i16>, String> {
    let mut lines = index.lines();
    let init: usize = lines.next().and_then(|l| l.strip_prefix("init ")).and_then(|n| n.parse().ok()).ok_or("bad sections index")?;
    let mut sections = Vec::new();
    for l in lines {
        let (off, at) = l.split_once(' ').ok_or("bad sections index")?;
        let off: usize = off.parse().map_err(|_| "bad sections index")?;
        let at: f64 = at.parse().ok().filter(|a: &f64| a.is_finite() && *a >= 0.0).ok_or("bad sections index")?;
        sections.push((off, at));
    }
    // ponytail: read whole, like a TS file (about 60 MB an hour).
    let data = std::fs::read(path).map_err(|e| format!("read audio: {e}"))?;
    if init > data.len() || sections.windows(2).any(|w| w[1].0 < w[0].0) || sections.last().is_some_and(|s| s.0 > data.len()) {
        return Err("sections index does not match the audio".into());
    }
    let pieces = sections.iter().enumerate().map(|(i, &(off, at))| {
        let end = sections.get(i + 1).map_or(data.len(), |s| s.0);
        let bytes = [&data[..init], &data[off.max(init)..end]].concat();
        let at = (at * RATE as f64).round().min(usize::MAX as f64) as usize;
        let pcm = if bytes.len() > TS_PACKET && bytes[0] == 0x47 && bytes[TS_PACKET] == 0x47 {
            decode_ts(&bytes)
        } else {
            decode_stream(Box::new(Cursor::new(bytes)), ext, 0.0)
        };
        (at, pcm)
    });
    collect(pieces, true)
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
/// placed before 0 (cut by an edit list) is dropped.
fn decode_stream(src: Box<dyn MediaSource>, ext: Option<&str>, offset: f64) -> Result<Vec<i16>, String> {
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
                let at = tb.map_or(0, |tb| (u128::from(packet.ts) * u128::from(tb.numer) * u128::from(rate) / u128::from(tb.denom.max(1))) as i64);
                // Where this packet plays, in source frames; before 0 it was edited out.
                let pos = (offset * rate as f64).round() as i64 + at;
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
fn mp4_edits(path: &Path) -> Option<Vec<Edit>> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let mut off = 0u64;
    let moov = loop {
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
                break body;
            }
            _ => {}
        }
        if size < hdr {
            return None;
        }
        off += size;
    };
    // mvhd and mdhd keep their timescale at the same place.
    let timescale = |b: &[u8]| -> Option<f64> {
        let at = if b.first()? == &1 { 20 } else { 12 };
        Some(f64::from(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?).max(1)))
    };
    let movie = timescale(child(&moov, b"mvhd")?)?;
    let trak = boxes(&moov).filter(|(k, _)| k == b"trak").map(|(_, t)| t).find(|t| {
        child(t, b"mdia").and_then(|m| child(m, b"hdlr")).and_then(|h| h.get(8..12)) == Some(b"soun")
    })?;
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

/// Decodes an MP4 (or any other file, whose edit list is empty) on the movie's
/// timeline. With at most one media edit, the edit is an offset applied while
/// decoding; edits that skip or repeat parts build the timeline from the
/// track decoded on its own one.
fn decode_edited(path: &Path, ext: Option<&str>) -> Result<Vec<i16>, String> {
    let open = || std::fs::File::open(path).map_err(|e| format!("open audio: {e}"));
    let edits = mp4_edits(path).unwrap_or_default();
    if edits.iter().filter(|e| e.from.is_some()).count() <= 1 {
        let lead: f64 = edits.iter().take_while(|e| e.from.is_none()).map(|e| e.secs).sum();
        let media = edits.iter().find(|e| e.from.is_some());
        let mut pcm = decode_stream(Box::new(open()?), ext, lead - media.and_then(|e| e.from).unwrap_or(0.0))?;
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
    let track = decode_stream(Box::new(open()?), ext, 0.0)?;
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

/// Where the audio runs start: `(byte offset in es, 90 kHz ticks on the
/// presentation's timeline)`. The timeline starts at the earliest PTS of any
/// stream, as players count it; a run starts wherever a PES's PTS is more than
/// 100 ms past the end of the audio before it, so the gap is kept.
fn runs(es: &[u8], ext: &str, pes: &[(usize, Option<u64>)], lead: u64) -> Vec<(usize, u64)> {
    const TOLERANCE: i64 = PTS_HZ as i64 / 10;
    let mut out = vec![(0, lead)];
    let Some(first) = pes.iter().find_map(|p| p.1) else { return out };
    // Audio before each byte offset, in ticks: the frames that start before it.
    let (mut i, mut before) = (0usize, 0u64);
    let next_frame = |upto: usize, i: &mut usize, before: &mut u64| {
        while *i < upto.min(es.len()) {
            match frame(&es[*i..], ext) {
                Some((len, ticks)) => {
                    *before += ticks;
                    *i += len;
                }
                None => *i += 1,
            }
        }
    };
    let (mut prev, mut t) = (first, lead as i64);
    let (mut run_at, mut run_from) = (lead as i64, 0i64);
    for &(at, p) in pes {
        next_frame(at, &mut i, &mut before);
        let Some(p) = p else { continue };
        t += ahead(prev, p);
        prev = p;
        let expected = run_at + (before as i64 - run_from);
        if t > expected + TOLERANCE {
            out.push((at, t as u64));
            (run_at, run_from) = (t, before as i64);
        } else if t < expected - TOLERANCE {
            // The clock jumped back (a discontinuity): carry on where the audio is.
            t = expected;
        }
    }
    out
}

/// The first audio elementary stream of an MPEG-TS file (ADTS AAC or MPEG
/// audio), the extension symphonia should read it as, and its [`runs`].
fn ts_audio(data: &[u8]) -> Result<(Vec<u8>, &'static str, Vec<(usize, u64)>), String> {
    let (mut pmt, mut audio): (Option<u16>, Option<(u16, &'static str)>) = (None, None);
    let mut out = Vec::new();
    // Each audio PES: where its payload starts in `out`, and its PTS.
    let mut pes: Vec<(usize, Option<u64>)> = Vec::new();
    // The first PTS of every other audio or video stream, for where the timeline starts.
    let mut firsts: Vec<(u16, u64)> = Vec::new();
    for p in data.chunks_exact(TS_PACKET).filter(|p| p[0] == 0x47) {
        let start = p[1] & 0x40 != 0;
        let id = pid(p[1], p[2]);
        let control = (p[3] >> 4) & 3;
        let at = if control & 2 != 0 { 5 + usize::from(p[4]) } else { 4 };
        if control & 1 == 0 || at >= TS_PACKET {
            continue;
        }
        let payload = &p[at..];
        if id == 0 && start && pmt.is_none() {
            // PAT: the first program's PMT.
            pmt = section(payload).and_then(|s| s.get(8..)).and_then(|e| {
                e.chunks_exact(4).find(|e| e[0] != 0 || e[1] != 0).map(|e| pid(e[2], e[3]))
            });
        } else if Some(id) == pmt && start && audio.is_none() {
            audio = section(payload).and_then(|s| {
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
            });
        } else if audio.is_some_and(|(a, _)| a == id) {
            if start {
                // PES header: 00 00 01, stream id, length, two flag bytes, header length.
                if payload.len() < 9 || payload[..3] != [0, 0, 1] {
                    continue;
                }
                pes.push((out.len(), pts(payload)));
                out.extend_from_slice(payload.get(9 + usize::from(payload[8])..).unwrap_or(&[]));
            } else {
                out.extend_from_slice(payload);
            }
        } else if start && payload.len() > 3 && payload[..3] == [0, 0, 1] && (0xC0..=0xEF).contains(&payload[3]) {
            if let (Some(t), false) = (pts(payload), firsts.iter().any(|f| f.0 == id)) {
                firsts.push((id, t));
            }
        }
    }
    let Some((_, ext)) = audio.filter(|_| !out.is_empty()) else {
        return Err("no AAC or MPEG audio in this MPEG-TS".into());
    };
    // How long after the earliest stream the audio starts.
    let lead = pes.iter().find_map(|p| p.1).map_or(0, |a| firsts.iter().map(|f| -ahead(a, f.1)).max().unwrap_or(0).max(0) as u64);
    let runs = runs(&out, ext, &pes, lead);
    Ok((out, ext, runs))
}

#[cfg(test)]
#[path = "decode_edit_tests.rs"]
mod edit_tests;
