//! Makes a saved video again with this build's pipeline, as a new saved
//! version beside the old one. Tools only; not part of the app.
//!
//! `remake_saved fetch <page-url> <audio-out>` fetches the video's audio the
//! way the app does without a browser session (its YouTube extractor, address
//! checks and downloader). A signed googlevideo address in place of the page
//! (one the app's own YouTube session issued) is downloaded directly.
//!
//! `remake_saved run <account-db> <video-key> <audio> <work-dir> [--apply]`
//! writes `audio` with the PC's speech model and translates it with the saved
//! Gemini key, as a job of the account library `account-db`; the new version
//! is selected when ready. Without `--apply` it only prints what was spent. The
//! installed app must be closed: its worker would take the job too.
//!
//! `remake_saved resume <account-db> <video-key> <work-dir>` carries on a
//! re-make whose process was stopped, from what its job last wrote.
//!
//! `remake_saved continue <account-db> <video-key> <job-id> <work-dir>` sends
//! a failed re-make back to translating; only cues without Korean are asked.
//! If it fails again its cues stay for another `continue`; the previous
//! version is still the selected one.
//!
//! `remake_saved restore <account-db> <video-key> <work-dir>` gives a failed
//! re-make up and projects the previous version again, as `run` and `resume`
//! do when their re-make does not end ready.

use std::path::Path;
use std::sync::Arc;
use vto::{gemini, media, store::Store, Config, Core, Services};

struct Quiet;
impl vto::Browser for Quiet {
    fn navigate(&self, _: &str, _: bool) -> Result<(), String> { Ok(()) }
    fn clear_site_data(&self) -> Result<(), String> { Ok(()) }
}
impl vto::Notifier for Quiet {
    fn ongoing(&self, job: &str, state: &str, _: &str, percent: i64) {
        println!("job {job} {state} {percent}%");
    }
    fn ready(&self, job: &str, text: &str, _: &str) {
        println!("job {job} ready: {text}");
    }
}

fn fetch(page: &str, out: &Path) -> Result<(), String> {
    use vto::AudioExtractor;
    // A signed audio address the app's YouTube session issued is taken as is.
    let raw = match url::Url::parse(page) {
        Ok(u) if u.host_str().is_some_and(|h| h.ends_with(".googlevideo.com")) => page.to_string(),
        _ => vto::youtube::InnertubeExtractor.audio_url(page).map_err(|r| format!("{}: {}", r.code, r.message))?,
    };
    let url = media::check_media_url(&raw, false).map_err(|r| r.code.to_string())?;
    let src = media::probe(&url, false).map_err(|r| r.code.to_string())?;
    let mut last = -1;
    media::download(&src, out, false, &mut |p| {
        if p / 10 > last / 10 {
            println!("download {p}%");
        }
        last = p;
    })?;
    println!("saved {} ({} bytes, .{})", out.display(), std::fs::metadata(out).map_err(|e| e.to_string())?.len(), src.ext());
    Ok(())
}

fn run(db: &Path, key: &str, audio: &Path, work: &Path, apply: bool) -> Result<(), String> {
    let store = Store::open(db)?;
    let video = store.video(key)?.ok_or("no such video")?;
    let spent = store.retranslation_cost(key)?;
    println!("{key} {:?}: spent {spent:.2} KRW so far", video.title);
    if !apply {
        println!("preview only; nothing changed");
        return Ok(());
    }
    std::fs::create_dir_all(work.join("audio")).map_err(|e| e.to_string())?;
    // The writing stage deletes its audio when done: it gets a copy.
    let copy = work.join("audio").join(format!("{key}.{}", audio.extension().and_then(|e| e.to_str()).unwrap_or("m4a")));
    std::fs::copy(audio, &copy).map_err(|e| e.to_string())?;
    let core = start(work)?;
    let id = format!("remake-{key}-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());
    let previous = store.begin_remake(&id, key, &copy.to_string_lossy(), gemini::MODEL)?;
    std::fs::write(work.join("previous.txt"), &previous).map_err(|e| e.to_string())?;
    println!("job {id} started; previous version {previous}");
    finish(&core, &store, &id, key, &previous, true)
}

/// Carries on a re-make whose process was stopped: the job resumes from what it wrote.
fn resume(db: &Path, key: &str, work: &Path) -> Result<(), String> {
    let store = Store::open(db)?;
    let job = store.active_job()?.filter(|j| j.video_key == key && j.id.starts_with("remake-")).ok_or("no re-make of this video is running")?;
    let previous = std::fs::read_to_string(work.join("previous.txt")).map_err(|e| format!("previous.txt: {e}"))?;
    println!("job {} resumed at {}; previous version {previous}", job.id, job.stage);
    let core = start(work)?;
    finish(&core, &store, &job.id, key, &previous, true)
}

/// Translates what a failed re-make left without Korean.
fn carry_on(db: &Path, key: &str, id: &str, work: &Path) -> Result<(), String> {
    let store = Store::open(db)?;
    let previous = std::fs::read_to_string(work.join("previous.txt")).map_err(|e| format!("previous.txt: {e}"))?;
    store.continue_remake(id)?;
    println!("job {id} continued at translating; previous version {previous}");
    let core = start(work)?;
    finish(&core, &store, id, key, &previous, false)
}

/// Gives a failed re-make up: the previous version is projected again.
fn restore(db: &Path, key: &str, work: &Path) -> Result<(), String> {
    let previous = std::fs::read_to_string(work.join("previous.txt")).map_err(|e| format!("previous.txt: {e}"))?;
    Store::open(db)?.restore_translation(key, &previous)?;
    println!("{previous} is projected again");
    Ok(())
}

fn finish(core: &Core, store: &Store, id: &str, key: &str, previous: &str, restore: bool) -> Result<(), String> {
    vto::pipeline::run_pending(core, store)?;
    let job = store.job(id)?.ok_or("job vanished")?;
    println!("job {id} {}: cost {:.4} KRW (input {} output {} tokens), failure {:?}", job.stage, job.cost_krw, job.input_amount, job.output_amount, job.failure_reason);
    if job.stage != "ready" && !restore {
        return Err(format!("the re-make failed again; its cues stay for `continue`, and {previous} is still selected"));
    }
    if job.stage != "ready" {
        store.restore_translation(key, previous)?;
        return Err(format!("the re-make failed; {previous} is selected again"));
    }
    Ok(())
}

fn start(work: &Path) -> Result<Arc<Core>, String> {
    let models = std::env::var_os("VTO_WHISPER_MODEL").map(Into::into);
    Core::start(
        Config { data_dir: work.to_path_buf(), gemini_base_url: vto::GEMINI_BASE_URL.into(), allow_loopback_media: false },
        Services {
            keystore: Box::new(vto::platform::CredentialStore::default()),
            browser: Box::new(Quiet),
            notifier: Box::new(Quiet),
            transcriber: Box::new(vto::whisper::Whisper::new(work, models)),
            extractor: Box::new(vto::youtube::InnertubeExtractor),
        },
    )
}

fn main() -> Result<(), String> {
    let a: Vec<String> = std::env::args().collect();
    match a.get(1).map(String::as_str) {
        Some("fetch") if a.len() == 4 => fetch(&a[2], Path::new(&a[3])),
        Some("run") if a.len() == 6 || a.len() == 7 => run(Path::new(&a[2]), &a[3], Path::new(&a[4]), Path::new(&a[5]), a.get(6).is_some_and(|f| f == "--apply")),
        Some("resume") if a.len() == 5 => resume(Path::new(&a[2]), &a[3], Path::new(&a[4])),
        Some("continue") if a.len() == 6 => carry_on(Path::new(&a[2]), &a[3], &a[4], Path::new(&a[5])),
        Some("restore") if a.len() == 5 => restore(Path::new(&a[2]), &a[3], Path::new(&a[4])),
        _ => Err("usage: remake_saved fetch <page-url> <audio-out> | remake_saved run <account-db> <video-key> <audio> <work-dir> [--apply] | remake_saved resume <account-db> <video-key> <work-dir> | remake_saved continue <account-db> <video-key> <job-id> <work-dir> | remake_saved restore <account-db> <video-key> <work-dir>".into()),
    }
}
