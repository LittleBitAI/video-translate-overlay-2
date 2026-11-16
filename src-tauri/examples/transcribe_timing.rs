//! Times the writing stage step by step with the app's own code: decoding,
//! speech-region planning, model loading and inference per region. Run on the
//! phone under `adb shell`, or on the PC for the large-model reference:
//!
//! `transcribe_timing <audio> <model> <out.json> [--passes N] [--limit-s S] [--plan P] [--mobile] [--overlap-ms MS] [--forced-cuts]`
//!
//! Pass 1 runs in a fresh process (cold: the model is loaded first); later
//! passes reuse the loaded model (warm). `--limit-s` writes only the regions
//! that start before S seconds; `--plan` picks the segmentation plan (the
//! current one by default, 1 for the windows of jobs made before plans);
//! `--mobile` decodes as the phone does, on any OS. `--overlap-ms` and
//! `--forced-cuts` plan plan-2 windows with another overlap, or with every cut
//! inside a span forced through speech, to tune the overlap;
//! `transcribe_timing --replay <report.json> <out.json>` reconciles a report's
//! saved segments again without the model, to tune the reconciliation. Thermal readings come from the host
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

/// A segment as the model gave it, in window time.
fn seg_json(s: &vto::Segment) -> Value {
    json!({"start_ms": s.start_ms, "end_ms": s.end_ms, "text": s.text,
           "words": s.words.iter().map(|w| json!([w.start_ms, w.end_ms, w.text])).collect::<Vec<_>>()})
}

/// `--replay <report.json> <out.json>`: the report's first run reconciled again
/// from its saved segments, without the model, as a report with new cues.
fn replay(report: &str, out: &str) {
    let mut d: Value = serde_json::from_slice(&std::fs::read(report).expect("read report")).expect("report");
    let timed = d["runs"][0]["regions"].as_array().expect("regions").clone();
    let ms = |v: &Value| v.as_i64().unwrap() as usize * vto::audio::RATE / 1000;
    let regions: Vec<vto::audio::Region> = timed.iter().map(|r| vto::audio::Region { start: ms(&r["start_ms"]), end: ms(&r["end_ms"]) }).collect();
    let (mut prev_end, mut cues, mut held) = (0i64, Vec::<Value>::new(), Vec::new());
    for (i, (r, t)) in regions.iter().zip(&timed).enumerate() {
        let segs = t["segs"].as_array().expect("segs: run the report with this harness").iter().map(|s| vto::Segment {
            start_ms: s["start_ms"].as_i64().unwrap(),
            end_ms: s["end_ms"].as_i64().unwrap(),
            text: s["text"].as_str().unwrap().into(),
            words: s["words"].as_array().unwrap().iter().map(|w| vto::Word { start_ms: w[0].as_i64().unwrap(), end_ms: w[1].as_i64().unwrap(), text: w[2].as_str().unwrap().into() }).collect(),
        }).collect();
        let committed;
        (committed, held) = vto::stitch::window_cues(r, segs, std::mem::take(&mut held), i.checked_sub(1).map(|p| &regions[p]), regions.get(i + 1), &mut prev_end);
        for (start, end, text) in committed {
            cues.push(json!({"region": i, "start_ms": start, "end_ms": end, "text": text}));
        }
    }
    d["runs"][0]["cues"] = Value::from(cues);
    std::fs::write(out, serde_json::to_vec_pretty(&d).unwrap()).expect("write report");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--replay") {
        return replay(&args[2], &args[3]);
    }
    let (Some(audio), Some(model), Some(out)) = (args.get(1), args.get(2), args.get(3)) else {
        eprintln!("usage: transcribe_timing <audio> <model> <out.json> [--passes N] [--limit-s S] [--plan P] [--mobile] [--overlap-ms MS] [--forced-cuts]");
        std::process::exit(2);
    };
    let passes: usize = arg(&args, "--passes").unwrap_or(1);
    let limit_ms: i64 = arg::<i64>(&args, "--limit-s").map_or(i64::MAX, |s| s * 1000);
    let plan_no: i64 = arg(&args, "--plan").unwrap_or(vto::audio::PLAN);

    let t = Instant::now();
    let pcm = vto::audio::decode_file(audio.as_ref()).expect("decode");
    let decode_ms = t.elapsed().as_millis();
    println!("timing step=decode ms={decode_ms} samples={}", pcm.len());

    let t = Instant::now();
    let overlap_ms: Option<usize> = arg(&args, "--overlap-ms");
    let forced = args.iter().any(|a| a == "--forced-cuts");
    let plan = if overlap_ms.is_some() || forced {
        let overlap = overlap_ms.map_or(vto::audio::OVERLAP, |ms| ms * vto::audio::RATE / 1000);
        vto::audio::speech_windows_tuned(&pcm, overlap, !forced)
    } else {
        vto::audio::plan(plan_no, &pcm)
    };
    let vad_ms = t.elapsed().as_millis();
    let regions: Vec<_> = plan.into_iter().filter(|r| r.start_ms() < limit_ms).collect();
    let speech_ms: i64 = regions.iter().map(|r| r.end_ms() - r.start_ms()).sum();
    let shared = regions.windows(2).filter(|w| w[1].start < w[0].end).count();
    println!("timing step=vad ms={vad_ms} plan={plan_no} regions={} overlaps={shared} speech_ms={speech_ms}", regions.len());

    let mut whisper = vto::whisper::Whisper::new(std::path::Path::new("."), Some(model.into()));
    if args.iter().any(|a| a == "--mobile") {
        whisper = whisper.with_mobile_params();
    }
    let t = Instant::now();
    whisper.load().expect("model load");
    let load_ms = t.elapsed().as_millis();
    println!("timing step=model-load ms={load_ms}");

    let mut runs = Vec::new();
    for pass in 1..=passes {
        whisper.prepare(std::path::Path::new("."), &mut |_| {}).expect("prepare");
        let started = Instant::now();
        let (mut prev_end, mut cues, mut timed, mut held) = (0i64, Vec::<Value>::new(), Vec::<Value>::new(), Vec::new());
        for (i, r) in regions.iter().enumerate() {
            let samples: Vec<f32> = pcm[r.start..r.end].iter().map(|&s| s as f32 / 32768.0).collect();
            let t = Instant::now();
            let segs = whisper.transcribe(&samples, &mut |_| {}).expect("transcribe");
            let ms = t.elapsed().as_millis();
            let audio_ms = r.end_ms() - r.start_ms();
            println!("timing step=region pass={pass} i={i} start_ms={} audio_ms={audio_ms} ms={ms} segs={}", r.start_ms(), segs.len());
            timed.push(json!({"i": i, "start_ms": r.start_ms(), "end_ms": r.end_ms(), "ms": ms as u64}));
            let words: Vec<Value> = segs.iter().flat_map(|s| &s.words).map(|w| json!([w.start_ms + r.start_ms(), w.end_ms + r.start_ms(), w.text])).collect();
            timed.last_mut().unwrap()["words"] = Value::from(words);
            timed.last_mut().unwrap()["segs"] = segs.iter().map(seg_json).collect();
            let committed;
            (committed, held) = vto::stitch::window_cues(r, segs, std::mem::take(&mut held), i.checked_sub(1).map(|p| &regions[p]), regions.get(i + 1), &mut prev_end);
            for (start, end, text) in committed {
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
        "plan": plan_no, "language": whisper.language(), "runs": runs,
    });
    std::fs::write(out, serde_json::to_vec_pretty(&report).unwrap()).expect("write report");
}
