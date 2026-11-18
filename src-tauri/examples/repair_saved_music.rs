//! Explicit, one-cue repair. Audio inspection is read-only; --apply atomically publishes a saved replacement.
use std::path::Path;
use vto::{audio, store::Store, whisper::voice::Detector};

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    if !(args.len() == 7 || (args.len() == 8 && args[7] == "--apply")) {
        return Err("usage: repair_saved_music <database> <video_key> <selected_version> <cue_seq> <source_audio_at_0> <models> [--apply]".into());
    }
    if !Path::new(&args[1]).is_file() { return Err("database does not exist".into()); }
    let seq: usize = args[4].parse().map_err(|_| "cue_seq must be a nonnegative integer")?;
    let store = Store::open(Path::new(&args[1]))?;
    let cue = store.inspect_silent_cue(&args[2], &args[3], seq)?;
    let pcm = audio::decode_file(Path::new(&args[5]))?;
    let start = (cue.start_ms.saturating_sub(300).max(0) as usize) * audio::RATE / 1000;
    let end = (cue.end_ms as usize + 300) * audio::RATE / 1000;
    if end > pcm.len() { return Err("source audio does not cover the cue and its context".into()); }
    let detector = Detector::new(Path::new(&args[6]));
    detector.prepare(false)?;
    let samples: Vec<_> = pcm[start..end].iter().map(|&x| x as f32 / 32768.0).collect();
    if !detector.spans(&samples)?.is_empty() { return Err("speech detected: retain this cue and inspect the source".into()); }
    println!("No speech at {}..{} ms; retain every other cue and existing translation; no paid calls", cue.start_ms, cue.end_ms);
    if args.len() == 8 {
        let identity = store.repair_silent_cue(&args[2], &args[3], &cue)?;
        println!("Published {identity}");
    } else {
        println!("Preview only; no saved data changed");
    }
    Ok(())
}
