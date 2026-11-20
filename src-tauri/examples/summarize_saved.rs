//! Asks P19 for one saved video's summary with the installed app's Gemini
//! key, as the 요약 button does. Tools only; not part of the app.
//!
//! `summarize_saved <work-dir> <video-key> [--again]` opens `work-dir` as the
//! app's data folder (copy an account library to `<work-dir>/app.sqlite3`
//! first, so the installed app's data is never written), sends the request
//! through the same API path as the screen, and prints the reply and the
//! summary row's recorded usage. A stored summary is answered without a call
//! unless `--again` is given. No job runs: the core is opened without a worker.

use std::path::Path;
use vto::{Config, Core, Services, Segment};

struct Quiet;
impl vto::Browser for Quiet {
    fn navigate(&self, _: &str, _: bool) -> Result<(), String> { Ok(()) }
    fn clear_site_data(&self) -> Result<(), String> { Ok(()) }
}
impl vto::Notifier for Quiet {
    fn ongoing(&self, _: &str, _: &str, _: &str, _: i64) {}
    fn ready(&self, _: &str, _: &str, _: &str) {}
}
impl vto::Transcriber for Quiet {
    fn transcribe(&self, _: &[f32], _: &mut dyn FnMut(f32)) -> Result<Vec<Segment>, String> { Err("no transcription in this tool".into()) }
}

fn main() -> Result<(), String> {
    let a: Vec<String> = std::env::args().collect();
    let (Some(work), Some(key)) = (a.get(1), a.get(2)) else { return Err("usage: summarize_saved <work-dir> <video-key> [--again]".into()) };
    let action = if a.get(3).is_some_and(|f| f == "--again") { "again" } else { "make" };
    let core = Core::open(
        Config { data_dir: Path::new(work).into(), gemini_base_url: vto::GEMINI_BASE_URL.into(), allow_loopback_media: false },
        Services {
            keystore: Box::new(vto::platform::CredentialStore::default()),
            browser: Box::new(Quiet),
            notifier: Box::new(Quiet),
            transcriber: Box::new(Quiet),
            extractor: Box::new(vto::youtube::InnertubeExtractor),
        },
    )?;
    let generation = core.account.status().map_err(|e| format!("{e:?}"))?.generation;
    let reply = vto::api::library_request(&core, Some(generation), "video_summary", &serde_json::json!({"video_key": key, "summary_action": action}))
        .map_err(|e| format!("{} {}: {}", e.status, e.error, e.message))?;
    println!("{}", serde_json::to_string_pretty(&reply).map_err(|e| e.to_string())?);
    let job = vto::api::library_request(&core, Some(generation), "get_job", &serde_json::json!({"page_url": core.store.video(key)?.ok_or("no video")?.page_url}))
        .map_err(|e| e.message)?;
    println!("job cost_krw={} summary_cost_krw={} input_amount={} output_amount={}", job["cost_krw"], job["summary_cost_krw"], job["input_amount"], job["output_amount"]);
    Ok(())
}
