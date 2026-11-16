//! The downloading stage: the worker's first attempt takes the source P6 just
//! probed for that job instead of asking the site again, and P7 says when the
//! stage is fetching the speech model rather than the audio, from before the
//! fetch connects until it ends.

use super::*;
use crate::Transcriber;
use std::sync::mpsc;
use std::sync::Mutex;

/// A model fetch that waits, before any byte or progress (as while it
/// connects), until the test sends how it ends.
struct Pausing {
    reached: Mutex<mpsc::Sender<()>>,
    go: Mutex<mpsc::Receiver<Result<bool, String>>>,
}
impl Transcriber for Pausing {
    fn needs_fetch(&self) -> bool {
        true
    }
    fn prepare(&self, _: &Path, progress: &mut dyn FnMut(f32)) -> Result<bool, String> {
        self.reached.lock().unwrap().send(()).unwrap();
        let end = self.go.lock().unwrap().recv().unwrap();
        if end.is_ok() {
            progress(1.0);
        }
        end
    }
    fn transcribe(&self, _: &[f32], _: &mut dyn FnMut(f32)) -> Result<Vec<crate::Segment>, String> {
        Ok(Vec::new())
    }
}

const PAGE: &str = "https://lectures.example.org/talk";

fn preparing(core: &Core) -> bool {
    api::get_job(core, PAGE).unwrap()["preparing_model"] == true
}

#[test]
fn the_first_attempt_uses_the_probed_source_and_reports_the_model_fetch() {
    let dir = std::env::temp_dir().join(format!("vto-handed-source-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("audio")).unwrap();
    let (reached, waiting) = mpsc::channel();
    let (go, wait) = mpsc::channel();
    let mut core = super::worker_tests::idle_core(&dir, "");
    core.svc.transcriber = Box::new(Pausing { reached: Mutex::new(reached), go: Mutex::new(wait) });
    let core = Arc::new(core);
    let key = api::video_key(&url::Url::parse(PAGE).unwrap());
    // No media URL on a site page: probing again would be refused as `no_media`.
    core.store.create_job("j", &key, PAGE, "other", "Talk", 1_000, None, "m").unwrap();
    let src = media::Source::File { url: format!("{}/talk.wav", crate::api::duration_tests::serve_wav()), ext: "wav" };
    *core.running.probed.lock().unwrap() = Some(("j".into(), src));

    let worker = core.clone();
    let run = std::thread::spawn(move || {
        let cx = Ctx { core: &worker, store: &worker.store, title: "Talk".into(), page_url: PAGE.into() };
        download(&cx, &worker.store.job("j").unwrap().unwrap())
    });
    waiting.recv().unwrap();
    assert!(preparing(&core), "P7 does not say the model is being fetched before its first byte");
    go.send(Ok(true)).unwrap();
    run.join().unwrap().unwrap();
    assert!(!preparing(&core), "the audio transfer is still reported as model preparation");
    let audio = core.store.job("j").unwrap().unwrap().audio_path.unwrap();
    assert_eq!(std::fs::metadata(audio).unwrap().len(), 32_044, "the handed source was not downloaded");

    // A job resumed at writing makes sure of the model too, and says so while it fetches.
    let worker = core.clone();
    let run = std::thread::spawn(move || {
        let cx = Ctx { core: &worker, store: &worker.store, title: "Talk".into(), page_url: PAGE.into() };
        let _ = write(&cx, &worker.store.job("j").unwrap().unwrap());
    });
    waiting.recv().unwrap();
    assert!(preparing(&core), "P7 does not say the model is being fetched when writing resumes");
    go.send(Ok(true)).unwrap();
    run.join().unwrap();
    assert!(!preparing(&core));

    // A model fetch that fails ends the preparation report with it.
    core.store.fail("j", "download_failed").unwrap();
    core.store.create_job("f", &key, PAGE, "other", "Talk", 1_000, None, "m").unwrap();
    let src = media::Source::File { url: "http://unused.invalid/".into(), ext: "wav" };
    *core.running.probed.lock().unwrap() = Some(("f".into(), src));
    let worker = core.clone();
    let run = std::thread::spawn(move || {
        let cx = Ctx { core: &worker, store: &worker.store, title: "Talk".into(), page_url: PAGE.into() };
        download(&cx, &worker.store.job("f").unwrap().unwrap())
    });
    waiting.recv().unwrap();
    assert!(preparing(&core));
    go.send(Err("model download: unreachable".into())).unwrap();
    assert!(run.join().unwrap().unwrap_err().starts_with("model: "));
    assert!(!preparing(&core), "a failed model fetch is still reported as preparation");

    // Another job's probe is never used: this one asks again, and the page names no media.
    core.store.fail("f", "download_failed").unwrap();
    core.store.create_job("k", &key, PAGE, "other", "Talk", 1_000, None, "m").unwrap();
    *core.running.probed.lock().unwrap() = Some(("j".into(), media::Source::File { url: "http://unused.invalid/".into(), ext: "wav" }));
    let cx = Ctx { core: &core, store: &core.store, title: "Talk".into(), page_url: PAGE.into() };
    assert_eq!(download(&cx, &core.store.job("k").unwrap().unwrap()).unwrap_err(), "no_media");
    drop(cx);
    let _ = std::fs::remove_dir_all(&dir);
}
