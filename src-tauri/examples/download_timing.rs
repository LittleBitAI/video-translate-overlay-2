//! Times what the app's downloading stage does, step by step, with the app's
//! own code: the two source probes (job creation, then the worker), the model
//! fetch, the audio transfer and the decode. Run on the phone under `adb shell`
//! with a signed audio URL the app's YouTube session issued (or any media URL):
//!
//! `download_timing <media-url> <work-dir> [--model] [--reuse]`
//!
//! One `timing step=...` line per step; the downloaded file is kept as
//! `<work-dir>/audio.<ext>` so runs can be compared byte for byte.

use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::time::Instant;

// The Android model of `src/whisper.rs`; kept here so the harness needs no whisper.cpp build.
const MODEL_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base-q5_1.bin";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (Some(raw), Some(dir)) = (args.get(1), args.get(2)) else {
        eprintln!("usage: download_timing <media-url> <work-dir> [--model]");
        std::process::exit(2);
    };
    let dir = std::path::Path::new(dir);
    std::fs::create_dir_all(dir).unwrap();
    // A YouTube hint is accepted only as `api::resolve_source` accepts it.
    let raw = vto::youtube::browser_audio_url(raw).unwrap_or(raw);
    let probe = || {
        let url = vto::media::check_media_url(raw, false).map_err(|r| r.code)?;
        vto::media::probe(&url, false).map_err(|r| r.code)
    };
    // `--pieces <n>`: only time n 10 MiB ranges fetched at once, each on its own connection.
    if let Some(n) = args.iter().position(|a| a == "--pieces").and_then(|i| args.get(i + 1)?.parse::<u64>().ok()) {
        let t = Instant::now();
        let runs: Vec<_> = (0..n)
            .map(|i| {
                let raw = raw.to_string();
                std::thread::spawn(move || {
                    let t = Instant::now();
                    let (off, len) = (i * (10 << 20), 10u64 << 20);
                    let req = vto::http::Request::get(&raw).header("Range", &format!("bytes={off}-{}", off + len - 1));
                    let r = vto::http::send(req).expect("piece request");
                    let first = t.elapsed().as_millis();
                    let got = r.bytes(len as usize).expect("piece body").len();
                    (i, first, t.elapsed().as_millis(), got)
                })
            })
            .collect();
        for run in runs {
            let (i, first, ms, got) = run.join().unwrap();
            println!("timing step=piece index={i} first_byte_ms={first} ms={ms} bytes={got}");
        }
        println!("timing step=pieces n={n} ms={}", t.elapsed().as_millis());
        return;
    }
    let t = Instant::now();
    let checked = vto::media::check_media_url(raw, false);
    println!("timing step=address-check ms={} ok={}", t.elapsed().as_millis(), checked.is_ok());
    // `--reuse`: the worker takes the source job creation probed, as the app now does.
    let probes: &[&str] = if args.iter().any(|a| a == "--reuse") { &["create-resolve"] } else { &["create-resolve", "worker-resolve"] };
    let mut src = None;
    for &name in probes {
        let t = Instant::now();
        let got = probe();
        println!("timing step={name} ms={} ok={}", t.elapsed().as_millis(), got.is_ok());
        src = Some(got.unwrap_or_else(|code| panic!("probe refused: {code}")));
    }
    let src = src.unwrap();
    if args.iter().any(|a| a == "--model") {
        let t = Instant::now();
        let mut r = vto::http::send(vto::http::Request::get(MODEL_URL)).expect("model request");
        let mut buf = vec![0u8; 1 << 20];
        let (mut n, mut hash) = (0u64, Sha256::new());
        let mut out = std::io::BufWriter::new(std::fs::File::create(dir.join("model.part")).unwrap());
        loop {
            let k = r.read(&mut buf).expect("model read");
            if k == 0 {
                break;
            }
            hash.update(&buf[..k]);
            out.write_all(&buf[..k]).unwrap();
            n += k as u64;
        }
        out.flush().unwrap();
        drop(out);
        let _ = std::fs::remove_file(dir.join("model.part"));
        println!("timing step=model ms={} bytes={n} sha256={:x}", t.elapsed().as_millis(), hash.finalize());
    }
    let dest = dir.join(format!("audio.{}", src.ext()));
    let t = Instant::now();
    let got = vto::media::download(&src, &dest, false, &mut |_| {});
    let bytes = std::fs::metadata(&dest).map_or(0, |m| m.len());
    println!("timing step=transfer ms={} bytes={bytes} ok={}", t.elapsed().as_millis(), got.is_ok());
    got.expect("download");
    let digest = Sha256::digest(std::fs::read(&dest).unwrap());
    println!("timing step=file sha256={digest:x}");
    let t = Instant::now();
    let pcm = vto::audio::decode_file(&dest).expect("decode");
    // 16 kHz mono: the decoded length on the source timeline, and where sound first starts.
    let first = pcm.iter().position(|&s| s.unsigned_abs() > 500).unwrap_or(pcm.len());
    let pcm_digest = Sha256::digest(pcm.iter().flat_map(|s| s.to_le_bytes()).collect::<Vec<u8>>());
    println!(
        "timing step=decode ms={} samples={} duration_ms={} first_sound_ms={} pcm_sha256={pcm_digest:x}",
        t.elapsed().as_millis(),
        pcm.len(),
        pcm.len() as u64 * 1000 / 16_000,
        first as u64 * 1000 / 16_000
    );
}
