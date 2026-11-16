//! Times the writing stage step by step with the app's own code: decoding,
//! speech-region planning, model loading and inference per region. Run on the
//! phone under `adb shell`, or on the PC for the large-model reference:
//!
//! `transcribe_timing <audio> <model> <out.json> [--passes N] [--limit-s S]`
//!
//! Pass 1 runs in a fresh process (cold: the model is loaded first); later
//! passes reuse the loaded model (warm). `--limit-s` writes only the regions
//! that start before S seconds. Thermal readings come from the host
//! (`dumpsys thermalservice`), which the shell user may read.
//!
//! The JSON keeps every pass's region timings and cues on the source timeline,
//! so the phone's text can be compared with a reference and between builds.

use serde_json::{json, Value};
use std::time::Instant;
use vto::Transcriber;

fn arg<T: std::str::FromStr>(args: &[String], name: &str) -> Option<T> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)?.parse().ok())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (Some(audio), Some(model), Some(out)) = (args.get(1), args.get(2), args.get(3)) else {
        eprintln!("usage: transcribe_timing <audio> <model> <out.json> [--passes N] [--limit-s S]");
        std::process::exit(2);
    };
    let passes: usize = arg(&args, "--passes").unwrap_or(1);
    let limit_ms: i64 = arg::<i64>(&args, "--limit-s").map_or(i64::MAX, |s| s * 1000);

    let t = Instant::now();
    let pcm = vto::audio::decode_file(audio.as_ref()).expect("decode");
    let decode_ms = t.elapsed().as_millis();
    println!("timing step=decode ms={decode_ms} samples={}", pcm.len());

    let t = Instant::now();
    let plan = vto::audio::speech_regions(&pcm);
    let vad_ms = t.elapsed().as_millis();
    let regions: Vec<_> = plan.into_iter().filter(|r| r.start_ms() < limit_ms).collect();
    let speech_ms: i64 = regions.iter().map(|r| r.end_ms() - r.start_ms()).sum();
    println!("timing step=vad ms={vad_ms} regions={} speech_ms={speech_ms}", regions.len());

    let whisper = vto::whisper::Whisper::new(std::path::Path::new("."), Some(model.into()));
    let t = Instant::now();
    whisper.load().expect("model load");
    let load_ms = t.elapsed().as_millis();
    println!("timing step=model-load ms={load_ms}");

    let mut runs = Vec::new();
    for pass in 1..=passes {
        whisper.prepare(std::path::Path::new("."), &mut |_| {}).expect("prepare");
        let started = Instant::now();
        let (mut prev_end, mut cues, mut timed) = (0i64, Vec::<Value>::new(), Vec::<Value>::new());
        for (i, r) in regions.iter().enumerate() {
            let samples: Vec<f32> = pcm[r.start..r.end].iter().map(|&s| s as f32 / 32768.0).collect();
            let t = Instant::now();
            let segs = whisper.transcribe(&samples, &mut |_| {}).expect("transcribe");
            let ms = t.elapsed().as_millis();
            let audio_ms = r.end_ms() - r.start_ms();
            println!("timing step=region pass={pass} i={i} start_ms={} audio_ms={audio_ms} ms={ms} segs={}", r.start_ms(), segs.len());
            timed.push(json!({"i": i, "start_ms": r.start_ms(), "end_ms": r.end_ms(), "ms": ms as u64}));
            for (start, end, text) in vto::pipeline::region_cues(r, segs, &mut prev_end) {
                cues.push(json!({"region": i, "start_ms": start, "end_ms": end, "text": text}));
            }
        }
        let wall_ms = started.elapsed().as_millis();
        println!("timing step=infer pass={pass} ms={wall_ms} audio_ms={speech_ms} cues={}", cues.len());
        runs.push(json!({"pass": pass, "warm": pass > 1, "infer_ms": wall_ms as u64, "regions": timed, "cues": cues}));
    }
    let report = json!({
        "audio": audio, "model": model, "samples": pcm.len(), "audio_ms": pcm.len() as i64 * 1000 / vto::audio::RATE as i64,
        "decode_ms": decode_ms as u64, "vad_ms": vad_ms as u64, "load_ms": load_ms as u64, "speech_ms": speech_ms,
        "language": whisper.language(), "runs": runs,
    });
    std::fs::write(out, serde_json::to_vec_pretty(&report).unwrap()).expect("write report");
}
