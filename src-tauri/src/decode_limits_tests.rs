//! A small downloaded file can never make the decoder allocate more than six
//! hours of PCM, whatever its header or timestamps claim: it fails the job
//! instead of the phone running out of memory.

use super::*;
use std::time::{Duration, Instant};

fn wav(rate: u32, fmt_size: u32, data: usize) -> Vec<u8> {
    let mut b = b"RIFF\0\0\0\0WAVEfmt ".to_vec();
    b.extend_from_slice(&fmt_size.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes()); // PCM
    b.extend_from_slice(&1u16.to_le_bytes()); // mono
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.resize(b.len() + fmt_size.saturating_sub(16).min(64) as usize, 0);
    b.extend_from_slice(b"data");
    b.extend_from_slice(&(data as u32).to_le_bytes());
    b.resize(b.len() + data, 0);
    b
}

fn decode_bytes(name: &str, bytes: &[u8]) -> Result<Vec<i16>, String> {
    let path = std::env::temp_dir().join(format!("vto-limit-{}-{name}", std::process::id()));
    std::fs::write(&path, bytes).unwrap();
    let r = decode_file(&path);
    let _ = std::fs::remove_file(&path);
    r
}

#[test]
fn a_wav_at_one_hertz_is_refused_not_multiplied() {
    let started = Instant::now();
    let e = decode_bytes("1hz.wav", &wav(1, 16, 1 << 20)).unwrap_err();
    assert!(e.contains("1 Hz"), "{e}");
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(decode_bytes("ok.wav", &wav(16_000, 16, 3_200)).unwrap().len(), 1_600);
}

#[test]
fn a_wav_header_claiming_a_huge_fmt_chunk_is_not_allocated() {
    let e = decode_bytes("fmt.wav", &wav(16_000, u32::MAX - 1, 0)).unwrap_err();
    assert!(e.contains("too large"), "{e}");
}

#[test]
fn a_timestamp_a_thousand_hours_ahead_stops_at_the_cap() {
    let started = Instant::now();
    let mut rs = Resampler::new(48_000);
    rs.push_frames(&[0.5; 480], 1);
    rs.pad_to(48_000 * 3600 * 1000); // a thousand hours later
    assert!(rs.over);
    assert!(rs.out.len() <= MAX_SAMPLES + 4);
    assert_eq!(rs.finish().unwrap_err(), TOO_LONG);
    assert!(started.elapsed() < Duration::from_secs(30), "padding took {:?}", started.elapsed());
}
