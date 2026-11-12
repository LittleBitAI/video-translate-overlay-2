//! Audio: decoding to 16 kHz mono, and finding the spans that hold speech.

use std::io::{BufReader, Read};
use std::path::Path;

pub const RATE: usize = 16_000;

/// The most audio decoded: six hours, about 690 MB of 16 kHz PCM. A file that
/// decodes (or whose header or timestamps claim) more is an error, never an
/// allocation the phone cannot make.
pub const MAX_SAMPLES: usize = 6 * 3600 * RATE;
pub const TOO_LONG: &str = "audio longer than six hours";

/// Decodes a downloaded file to 16 kHz mono PCM16, streaming so that an hour of
/// 48 kHz stereo never sits in memory as floats.
///
/// WAV is parsed here; everything else goes through [`crate::decode`] on every
/// OS, which places audio by its timestamps so a gap stays a gap. Media
/// Foundation is only the fallback on Windows, for what symphonia cannot open.
pub fn decode_file(path: &Path) -> Result<Vec<i16>, String> {
    let mut f = BufReader::new(std::fs::File::open(path).map_err(|e| format!("open audio: {e}"))?);
    let mut head = [0u8; 12];
    if f.read_exact(&mut head).is_ok() && &head[0..4] == b"RIFF" && &head[8..12] == b"WAVE" {
        return decode_wav(f);
    }
    let pcm = crate::decode::decode(path);
    // Not for an HLS download in sections: Media Foundation would read the joined
    // file on one timeline, the very thing the sections are there to avoid.
    #[cfg(windows)]
    if pcm.is_err() && !crate::media::sections_path(path).exists() {
        if let Ok(p) = platform::decode(path) {
            return Ok(p);
        }
    }
    pcm
}

/// Downmixes and box-filters (or holds, when upsampling) to 16 kHz as samples arrive.
pub struct Resampler {
    step: f64,
    next: f64,
    seen: u64,
    acc: f32,
    n: u32,
    last: f32,
    pub out: Vec<i16>,
    /// Set once [`MAX_SAMPLES`] are out: nothing more is kept.
    pub over: bool,
}

impl Resampler {
    pub fn new(rate: usize) -> Self {
        let step = rate as f64 / RATE as f64;
        Resampler { step, next: step, seen: 0, acc: 0.0, n: 0, last: 0.0, out: Vec::new(), over: false }
    }

    /// The PCM, or [`TOO_LONG`] when it was cut at [`MAX_SAMPLES`].
    pub fn finish(self) -> Result<Vec<i16>, String> {
        if self.over {
            return Err(TOO_LONG.into());
        }
        Ok(self.out)
    }

    pub fn push(&mut self, x: f32) {
        if self.out.len() >= MAX_SAMPLES {
            self.over = true;
            return;
        }
        self.acc += x;
        self.n += 1;
        self.seen += 1;
        while self.seen as f64 >= self.next - 1e-9 {
            if self.n > 0 {
                self.last = self.acc / self.n as f32;
                self.acc = 0.0;
                self.n = 0;
            }
            self.out.push((self.last.clamp(-1.0, 1.0) * 32767.0) as i16);
            self.next += self.step;
        }
    }

    pub fn push_frames(&mut self, interleaved: &[f32], channels: usize) {
        for frame in interleaved.chunks(channels.max(1)) {
            self.push(frame.iter().sum::<f32>() / frame.len() as f32);
        }
    }

    /// Silence up to source frame `at` when the next audio starts more than
    /// 100 ms past what was pushed: a gap in the timestamps stays a gap here.
    pub fn pad_to(&mut self, at: u64) {
        let rate = (self.step * RATE as f64).round() as u64;
        if at > self.seen + rate / 10 {
            // No more source frames than fit under the cap: a timestamp hours ahead costs no time.
            let room = ((MAX_SAMPLES.saturating_sub(self.out.len()) + 1) as f64 * self.step).ceil() as u64;
            for _ in self.seen..at.min(self.seen + room) {
                self.push(0.0);
            }
            if at > self.seen {
                self.over = true;
            }
        }
    }
}

/// Parses the rest of a RIFF/WAVE stream whose 12-byte header was consumed.
fn decode_wav(mut f: impl Read) -> Result<Vec<i16>, String> {
    let (mut format, mut channels, mut rate, mut bits) = (0u16, 0usize, 0usize, 0usize);
    loop {
        let mut hdr = [0u8; 8];
        f.read_exact(&mut hdr).map_err(|_| "wav: no data chunk".to_string())?;
        let size = u32::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]) as usize;
        if &hdr[0..4] == b"fmt " {
            // 16 to 40 bytes in practice; a header claiming more is not read into memory.
            if size > 1024 {
                return Err("wav: fmt chunk too large".into());
            }
            let mut b = vec![0u8; size + (size & 1)];
            f.read_exact(&mut b).map_err(|e| format!("wav fmt: {e}"))?;
            if size < 16 {
                return Err("wav: short fmt".into());
            }
            let u16le = |i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
            format = u16le(0);
            channels = u16le(2) as usize;
            rate = u32::from_le_bytes([b[4], b[5], b[6], b[7]]) as usize;
            bits = u16le(14) as usize;
            if format == 0xFFFE && size >= 26 {
                format = u16le(24); // extensible: the sub-format GUID starts with the tag
            }
        } else if &hdr[0..4] == b"data" {
            if channels == 0 || rate == 0 {
                return Err("wav: data before fmt".into());
            }
            // Real audio runs at 4-384 kHz; a rate of 1 Hz would make 16,000 samples of every one.
            if !(4_000..=384_000).contains(&rate) || channels > 32 {
                return Err(format!("wav: {rate} Hz with {channels} channels is not audio this app decodes"));
            }
            if !matches!((format, bits), (1, 8) | (1, 16) | (1, 24) | (1, 32) | (3, 32)) {
                return Err(format!("wav: unsupported format {format}/{bits}"));
            }
            let width = bits / 8;
            let mut rs = Resampler::new(rate);
            let mut frame = vec![0u8; width * channels];
            let mut floats = vec![0f32; channels];
            for _ in 0..size / frame.len() {
                if f.read_exact(&mut frame).is_err() {
                    break; // a truncated download still yields what it has
                }
                for (c, v) in floats.iter_mut().enumerate() {
                    let d = &frame[c * width..(c + 1) * width];
                    *v = match (format, bits) {
                        (1, 8) => (d[0] as f32 - 128.0) / 128.0,
                        (1, 16) => i16::from_le_bytes([d[0], d[1]]) as f32 / 32768.0,
                        (1, 24) => (i32::from_le_bytes([0, d[0], d[1], d[2]]) >> 8) as f32 / 8_388_608.0,
                        (1, _) => i32::from_le_bytes([d[0], d[1], d[2], d[3]]) as f32 / 2_147_483_648.0,
                        _ => f32::from_le_bytes([d[0], d[1], d[2], d[3]]),
                    };
                }
                rs.push_frames(&floats, channels);
                if rs.over {
                    break;
                }
            }
            return rs.finish();
        } else {
            std::io::copy(&mut (&mut f).take((size + (size & 1)) as u64), &mut std::io::sink()).map_err(|e| e.to_string())?;
        }
    }
}

/// A span of the 16 kHz signal, in samples.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Region {
    pub start: usize,
    pub end: usize,
}

impl Region {
    pub fn start_ms(&self) -> i64 {
        (self.start * 1000 / RATE) as i64
    }
    pub fn end_ms(&self) -> i64 {
        (self.end * 1000 / RATE) as i64
    }
}

const FRAME: usize = RATE / 50; // 20 ms
const MERGE_GAP_FRAMES: usize = 40; // pauses under 800 ms stay inside one span
const MIN_FRAMES: usize = 8; // drop blips under 160 ms
const PAD: usize = RATE / 5; // 200 ms of context on each side
const MAX_REGION: usize = 30 * RATE; // what the model takes at once

/// Speech spans with silence cut out, so the model never sees long silence
/// (where Whisper-style models invent text). Deterministic for given audio,
/// which lets an interrupted write resume at the same span boundaries.
pub fn speech_regions(pcm: &[i16]) -> Vec<Region> {
    let rms: Vec<f32> = pcm
        .chunks(FRAME)
        .map(|f| (f.iter().map(|&s| (s as f32 / 32768.0).powi(2)).sum::<f32>() / f.len() as f32).sqrt())
        .collect();
    if rms.is_empty() {
        return Vec::new();
    }
    let mut sorted = rms.clone();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let floor = sorted[sorted.len() / 10];
    // Three times the noise floor, kept between a whisper and quiet speech.
    let threshold = (floor * 3.0).clamp(0.006, 0.02);

    let mut spans: Vec<(usize, usize)> = Vec::new(); // frame ranges
    for (i, &v) in rms.iter().enumerate() {
        if v < threshold {
            continue;
        }
        match spans.last_mut() {
            Some((_, end)) if i - *end <= MERGE_GAP_FRAMES => *end = i + 1,
            _ => spans.push((i, i + 1)),
        }
    }
    let mut out = Vec::new();
    for (a, b) in spans.into_iter().filter(|(a, b)| b - a >= MIN_FRAMES) {
        let start = (a * FRAME).saturating_sub(PAD).max(out.last().map_or(0, |r: &Region| r.end));
        let end = (b * FRAME + PAD).min(pcm.len());
        split_long(&rms, start, end, &mut out);
    }
    out
}

/// Cuts spans longer than the model's window at the quietest frame of their last 10 s.
fn split_long(rms: &[f32], mut start: usize, end: usize, out: &mut Vec<Region>) {
    while end - start > MAX_REGION {
        let lo = (start + MAX_REGION - 10 * RATE) / FRAME;
        let hi = ((start + MAX_REGION) / FRAME).min(rms.len());
        let cut = (lo..hi).min_by(|&x, &y| rms[x].total_cmp(&rms[y])).unwrap_or(hi) * FRAME;
        out.push(Region { start, end: cut });
        start = cut;
    }
    out.push(Region { start, end });
}

#[cfg(windows)]
mod platform {
    use super::Resampler;
    use std::path::Path;
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::Media::MediaFoundation::*;
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

    pub fn decode(path: &Path) -> Result<Vec<i16>, String> {
        let e = |what: &'static str| move |err: windows::core::Error| format!("{what}: {err}");
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            MFStartup(MF_VERSION, MFSTARTUP_FULL).map_err(e("MFStartup"))?;
            let result = (|| {
                let url = HSTRING::from(path.as_os_str());
                let reader = MFCreateSourceReaderFromURL(PCWSTR(url.as_ptr()), None).map_err(e("open media"))?;
                let audio = MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32;
                reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false).map_err(e("select"))?;
                reader.SetStreamSelection(audio, true).map_err(e("no audio stream"))?;
                let want = MFCreateMediaType().map_err(e("media type"))?;
                want.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio).map_err(e("major type"))?;
                want.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_Float).map_err(e("subtype"))?;
                reader.SetCurrentMediaType(audio, None, &want).map_err(e("no decoder for this audio"))?;
                let got = reader.GetCurrentMediaType(audio).map_err(e("current type"))?;
                let rate = got.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND).map_err(e("rate"))? as usize;
                let channels = got.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS).map_err(e("channels"))?.max(1) as usize;
                let mut rs = Resampler::new(rate);
                loop {
                    let mut flags = 0u32;
                    let mut at = 0i64; // 100 ns units
                    let mut sample: Option<IMFSample> = None;
                    reader
                        .ReadSample(audio, 0, None, Some(&mut flags), Some(&mut at), Some(&mut sample))
                        .map_err(e("read"))?;
                    if let Some(s) = sample {
                        rs.pad_to((at.max(0) as u128 * rate as u128 / 10_000_000) as u64);
                        let buf = s.ConvertToContiguousBuffer().map_err(e("buffer"))?;
                        let mut ptr = std::ptr::null_mut();
                        let mut len = 0u32;
                        buf.Lock(&mut ptr, None, Some(&mut len)).map_err(e("lock"))?;
                        rs.push_frames(std::slice::from_raw_parts(ptr as *const f32, len as usize / 4), channels);
                        let _ = buf.Unlock();
                    }
                    if rs.over || flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                        break;
                    }
                }
                rs.finish()
            })();
            let _ = MFShutdown();
            result
        }
    }
}

#[cfg(test)]
#[path = "decode_tests.rs"]
mod decode_tests;

#[cfg(test)]
#[path = "decode_gap_tests.rs"]
mod decode_gap_tests;

#[cfg(test)]
#[path = "decode_container_gap_tests.rs"]
mod decode_container_gap_tests;

#[cfg(test)]
#[path = "decode_limits_tests.rs"]
mod decode_limits_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(ms: usize) -> Vec<i16> {
        (0..ms * RATE / 1000).map(|i| ((i as f32 * 0.055).sin() * 8000.0) as i16).collect()
    }

    #[test]
    fn regions_skip_silence_and_keep_source_times() {
        let mut pcm = vec![0i16; RATE]; // 1 s silence
        pcm.extend(tone(2_000)); // speech 1.0-3.0 s
        pcm.extend(vec![0; 10 * RATE]); // 10 s silence
        pcm.extend(tone(1_000)); // speech 13.0-14.0 s
        let r = speech_regions(&pcm);
        assert_eq!(r.len(), 2, "{r:?}");
        assert!((r[0].start_ms() - 800).abs() <= 20 && (r[0].end_ms() - 3_200).abs() <= 20, "{r:?}");
        assert!((r[1].start_ms() - 12_800).abs() <= 20, "{r:?}");
    }

    #[test]
    fn long_speech_is_cut_into_model_windows() {
        let r = speech_regions(&tone(70_000));
        assert!(r.len() >= 3 && r.iter().all(|x| x.end - x.start <= MAX_REGION), "{r:?}");
        assert!(r.windows(2).all(|w| w[0].end == w[1].start));
    }

    #[test]
    fn stereo_wav_is_downmixed_and_resampled() {
        let mut b = Vec::new();
        b.extend(b"RIFF\0\0\0\0WAVEfmt ");
        b.extend(16u32.to_le_bytes());
        b.extend(1u16.to_le_bytes());
        b.extend(2u16.to_le_bytes());
        b.extend(32_000u32.to_le_bytes());
        b.extend((32_000u32 * 4).to_le_bytes());
        b.extend(4u16.to_le_bytes());
        b.extend(16u16.to_le_bytes());
        b.extend(b"LIST");
        b.extend(3u32.to_le_bytes());
        b.extend(b"abc\0");
        b.extend(b"data");
        b.extend((32_000u32 * 4).to_le_bytes());
        for _ in 0..32_000 {
            b.extend(16_384i16.to_le_bytes());
            b.extend(0i16.to_le_bytes());
        }
        let pcm = decode_wav(&b[12..]).unwrap();
        assert_eq!(pcm.len(), 16_000);
        assert!((pcm[100] as i32 - 8_191).abs() <= 2, "{}", pcm[100]);
    }
}
