//! A WAV whose data ends before the size its header gives is a file cut short:
//! it fails instead of being written as if the lecture ended there, so no job
//! is made ready (and later reused) with the rest of it missing. A stream's
//! WAV (size 0 or 0xFFFFFFFF, its length unknown when written) is read to its end.
//!
//! And quiet speech is still speech: a recording whose every sample is within
//! ±190 has its spans found, while a steady quiet hum and digital silence have none.

use super::*;

/// A 16 kHz mono PCM16 WAV whose `data` chunk says `declared` bytes and holds `pcm`.
fn wav(declared: u32, pcm: &[i16]) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend(b"RIFF");
    b.extend((36 + pcm.len() as u32 * 2).to_le_bytes());
    b.extend(b"WAVEfmt ");
    b.extend(16u32.to_le_bytes());
    b.extend(1u16.to_le_bytes());
    b.extend(1u16.to_le_bytes());
    b.extend((RATE as u32).to_le_bytes());
    b.extend((RATE as u32 * 2).to_le_bytes());
    b.extend(2u16.to_le_bytes());
    b.extend(16u16.to_le_bytes());
    b.extend(b"data");
    b.extend(declared.to_le_bytes());
    for s in pcm {
        b.extend(s.to_le_bytes());
    }
    b
}

fn decode(name: &str, bytes: &[u8]) -> Result<Vec<i16>, String> {
    let path = std::env::temp_dir().join(format!("vto-cut-{name}-{}.wav", std::process::id()));
    std::fs::write(&path, bytes).unwrap();
    let pcm = decode_file(&path);
    let _ = std::fs::remove_file(&path);
    pcm
}

/// A voice-like sound (140 Hz and its harmonics, a syllable-rate swell) over
/// `spans` (seconds), at most `peak` in magnitude, in `total` seconds.
fn voice(total: f64, spans: &[(f64, f64)], peak: f64) -> Vec<i16> {
    let n = (total * RATE as f64) as usize;
    let raw: Vec<f64> = (0..n)
        .map(|i| {
            let t = i as f64 / RATE as f64;
            if !spans.iter().any(|&(a, b)| (a..b).contains(&t)) {
                return 0.0;
            }
            let v: f64 = (1..=5).map(|k| (2.0 * std::f64::consts::PI * 140.0 * k as f64 * t).sin() / k as f64).sum();
            v * (0.6 + 0.4 * (2.0 * std::f64::consts::PI * 4.0 * t).sin())
        })
        .collect();
    let top = raw.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    raw.iter().map(|v| (v / top * peak).round() as i16).collect()
}

#[test]
fn a_wav_cut_short_of_its_data_size_fails() {
    // 60 s declared, the first 10 s (speech) there.
    let pcm = voice(10.0, &[(1.0, 9.0)], 12_000.0);
    let e = decode("short", &wav(60 * RATE as u32 * 2, &pcm)).unwrap_err();
    assert!(e.contains("ends at 320000 of its 1920000 bytes"), "{e}");
    // Whole, it decodes to its length.
    assert_eq!(decode("whole", &wav(pcm.len() as u32 * 2, &pcm)).unwrap().len(), pcm.len());
}

#[test]
fn a_streamed_wav_is_read_to_its_end() {
    let pcm = voice(3.0, &[(0.5, 2.5)], 12_000.0);
    for declared in [0, u32::MAX] {
        assert_eq!(decode("stream", &wav(declared, &pcm)).unwrap().len(), pcm.len(), "data size {declared:#x}");
    }
}

#[test]
fn quiet_speech_is_found() {
    let spans = [(1.0, 3.0), (6.0, 8.5), (12.0, 14.0)];
    let pcm = voice(16.0, &spans, 190.0);
    assert!(pcm.iter().all(|s| s.unsigned_abs() <= 190));
    let r = speech_regions(&pcm);
    assert_eq!(r.len(), spans.len(), "{r:?}");
    for (r, (a, b)) in r.iter().zip(spans) {
        let (a, b) = ((a * 1000.0) as i64, (b * 1000.0) as i64);
        assert!((r.start_ms() - (a - 200)).abs() <= 60 && (r.end_ms() - (b + 200)).abs() <= 60, "{r:?} for {a}-{b} ms");
    }
    // The same speech at a normal level gives the same spans.
    assert_eq!(speech_regions(&voice(16.0, &spans, 12_000.0)), r);
}

#[test]
fn a_quiet_steady_hum_and_digital_silence_are_not_speech() {
    let mut x = 12_345u32;
    let hum: Vec<i16> = (0..10 * RATE)
        .map(|_| {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            ((x >> 16) % 201) as i16 - 100
        })
        .collect();
    assert_eq!(speech_regions(&hum), vec![]);
    assert_eq!(speech_regions(&vec![0i16; 10 * RATE]), vec![]);
}
