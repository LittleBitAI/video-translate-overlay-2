//! Pure-Rust decoding (symphonia) where Media Foundation is not there, such as
//! Android: MP4/M4A, AAC (ADTS), MP3, and MPEG-TS, whose audio is unwrapped
//! here because symphonia has no TS reader.

use crate::audio::Resampler;
use std::io::Cursor;
use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

const TS_PACKET: usize = 188;

pub fn decode(path: &Path) -> Result<Vec<i16>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("open audio: {e}"))?;
    let mut head = [0u8; TS_PACKET + 1];
    let is_ts = std::io::Read::read_exact(&mut &file, &mut head).is_ok() && head[0] == 0x47 && head[TS_PACKET] == 0x47;
    if is_ts {
        // ponytail: the TS audio is gathered in memory (about 60 MB for an hour of 128 kbit/s AAC).
        let data = std::fs::read(path).map_err(|e| format!("read audio: {e}"))?;
        let (es, ext) = ts_audio(&data)?;
        return decode_stream(Box::new(Cursor::new(es)), Some(ext));
    }
    let ext = path.extension().and_then(|e| e.to_str()).map(str::to_string);
    let file = std::fs::File::open(path).map_err(|e| format!("open audio: {e}"))?;
    decode_stream(Box::new(file), ext.as_deref())
}

fn decode_stream(src: Box<dyn MediaSource>, ext: Option<&str>) -> Result<Vec<i16>, String> {
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
    let id = track.id;
    let mut decoder = symphonia::default::get_codecs()
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
                let rs = rs.get_or_insert_with(|| Resampler::new(spec.rate as usize));
                if buf.as_ref().is_none_or(|b| b.capacity() < audio.capacity() * channels) {
                    buf = Some(SampleBuffer::new(audio.capacity() as u64, spec));
                }
                let b = buf.as_mut().unwrap();
                b.copy_interleaved_ref(audio);
                rs.push_frames(b.samples(), channels);
            }
            // One damaged frame does not end the lecture.
            Err(Error::DecodeError(_)) => continue,
            Err(e) => return Err(format!("decode: {e}")),
        }
    }
    rs.map(|r| r.out).ok_or_else(|| "no audio decoded".to_string())
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

/// The first audio elementary stream of an MPEG-TS file (ADTS AAC or MPEG
/// audio) and the extension symphonia should read it as.
fn ts_audio(data: &[u8]) -> Result<(Vec<u8>, &'static str), String> {
    let (mut pmt, mut audio): (Option<u16>, Option<(u16, &'static str)>) = (None, None);
    let mut out = Vec::new();
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
                out.extend_from_slice(payload.get(9 + usize::from(payload[8])..).unwrap_or(&[]));
            } else {
                out.extend_from_slice(payload);
            }
        }
    }
    match audio {
        Some((_, ext)) if !out.is_empty() => Ok((out, ext)),
        _ => Err("no AAC or MPEG audio in this MPEG-TS".into()),
    }
}
