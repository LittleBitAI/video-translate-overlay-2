//! The background worker: one job at a time, every step recorded in SQLite
//! so that a killed process picks up where it stopped.

use crate::gemini::{self, Client};
use crate::store::{Job, Store};
use crate::{api, audio, log, media, stitch, Core};
use serde_json::Value;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Why a job stopped for good; becomes `failure_reason`.
type Stop = String;

pub fn run_worker(core: Arc<Core>) {
    let mut resumed = false;
    loop {
        // Captured databases keep running after sign-out; the worker never resolves a new owner mid-job.
        let next = (|| -> Result<_, String> {
            for store in core.job_stores()? {
                sweep_audio(&store);
                if let Some(job) = store.active_job()? { return Ok(Some((store, job))); }
            }
            Ok(None)
        })();
        match next {
            Ok(Some((store, job))) => {
                let title = store.video(&job.video_key).ok().flatten().map(|v| v.title).unwrap_or_default();
                if !resumed {
                    log(format!("resuming job {} at {}", job.id, job.stage));
                    core.svc.notifier.ongoing(&job.id, "shown", &title, job.percent);
                }
                resumed = true;
                let id = job.id.clone();
                let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_job(&core, &store, job)));
                if run.is_err() {
                    // A bug must not leave the job active, holding up every later one. The
                    // old connection is dropped (rolling back anything it left open) and a
                    // fresh one records the failure.
                    let path = store.path().to_path_buf();
                    drop(store);
                    match Store::open(&path) {
                        Ok(fresh) => give_up(&core, &fresh, &id, &title, "panic"),
                        Err(e) => log(format!("worker cannot reopen its owning database: {e}")),
                    }
                }
            }
            Ok(None) => {
                resumed = true;
                core.wait_for_work(Duration::from_secs(5));
            }
            Err(e) => {
                log(format!("worker: {e}"));
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
}

/// Runs the jobs in progress in `store` to their end, here and now: the
/// worker's steps for one library a tool names (`examples/remake_saved.rs`).
pub fn run_pending(core: &Core, store: &Store) -> Result<(), String> {
    while let Some(job) = store.active_job()? {
        run_job(core, store, job);
    }
    Ok(())
}

struct Ctx<'a> {
    core: &'a Core,
    store: &'a Store,
    title: String,
    page_url: String,
}

/// One stage's progress. The job row keeps the stage's own 0-100 (E2); the
/// ongoing notice shows the whole job's, this stage taking `from..to` of it.
struct Stage {
    last: i64,
    from: i64,
    to: i64,
}

impl Stage {
    // ponytail: fixed shares, writing dominates on the device; measure per stage if the bar feels uneven.
    const DOWNLOADING: (i64, i64) = (0, 10);
    const WRITING: (i64, i64) = (10, 90);
    const TRANSLATING: (i64, i64) = (90, 100);
    fn new((from, to): (i64, i64)) -> Self {
        Stage { last: -1, from, to }
    }
}

impl Ctx<'_> {
    fn percent(&self, job: &str, p: i64, at: &mut Stage) {
        if p > at.last {
            // Every 10% step that is passed, not only exact multiples: progress often jumps over them.
            let step = p / 10 > at.last / 10;
            at.last = p;
            let _ = self.store.raise_percent(job, p);
            if step {
                let whole = at.from + p.min(100) * (at.to - at.from) / 100;
                self.core.svc.notifier.ongoing(job, "updated", &self.title, whole);
            }
        }
    }
}

fn run_job(core: &Core, store: &Store, job: Job) {
    let Ok(Some(video)) = store.video(&job.video_key) else {
        let _ = store.fail(&job.id, failure_reason("video row missing", &job.stage));
        return;
    };
    let cx = Ctx { core, store, title: video.title.clone(), page_url: video.page_url.clone() };
    let result = (|| -> Result<(), Stop> {
        let mut stage = job.stage.clone();
        if stage == "downloading" {
            download(&cx, &job)?;
            store.set_stage(&job.id, "writing")?;
            stage = "writing".into();
        }
        if stage == "writing" {
            let fresh = store.job(&job.id)?.ok_or("job vanished")?;
            // The model (1.5 GB for large-v3-turbo) is not kept through translation and idle time.
            let written = write(&cx, &fresh);
            core.svc.transcriber.unload();
            written?;
        }
        // A job that already notified only has its ready mark left to write.
        if !store.job(&job.id)?.is_some_and(|j| j.notified) {
            translate(&cx, &job.id)?;
            // Sent first, recorded after: a process killed in between notifies again
            // when the job resumes, rather than never. Only this worker sends it.
            let text = format!("한국어 자막이 준비됐습니다: {}", cx.title);
            core.svc.notifier.ready(&job.id, &text, &cx.page_url);
            store.mark_notified(&job.id)?;
        }
        core.svc.notifier.ongoing(&job.id, "hidden", &cx.title, 100);
        store.finish(&job.id)?;
        log(format!("job {} ready", job.id));
        Ok(())
    })();
    if let Err(detail) = result {
        give_up(core, store, &job.id, &cx.title, &detail);
    }
}

/// Fails the job for good: the reason for the stage it stopped in, its audio
/// file removed, and the ongoing notice taken down.
fn give_up(core: &Core, store: &Store, job_id: &str, title: &str, detail: &str) {
    let at = store.job(job_id).ok().flatten();
    let reason = failure_reason(detail, at.as_ref().map_or("downloading", |j| j.stage.as_str()));
    log(format!("job {job_id} failed: {reason} ({detail})"));
    let _ = store.fail(job_id, reason);
    if let Some(p) = at.and_then(|j| j.audio_path) {
        drop_audio(store, job_id, &p);
    }
    core.svc.notifier.ongoing(job_id, "hidden", title, 0);
}

/// What a transcriber's `prepare` returns on a device whose CPU cannot run
/// its speech engine (the Android build needs ARMv8.2 dot product and half
/// precision); also that job's `failure_reason`.
pub const UNSUPPORTED_CPU: &str = "unsupported_cpu";

/// The `failure_reason` stored for a stop: one of the values E2 allows
/// (`cost_limit` is only on jobs from before translation stopped at a cost).
/// `detail` is the internal cause (logged); `stage` is where the job stopped.
pub fn failure_reason(detail: &str, stage: &str) -> &'static str {
    match detail {
        UNSUPPORTED_CPU => UNSUPPORTED_CPU,
        "key_rejected" | "no_key" => "key_rejected",
        "no_speech" => "no_speech",
        _ if stage == "translating" => "translate_failed",
        // Media refusals, network or decoding trouble, and a missing or broken
        // model file (fetched in the downloading stage, A4) all mean the audio
        // could not be turned into text.
        _ => "download_failed",
    }
}

fn download(cx: &Ctx, job: &Job) -> Result<(), Stop> {
    let page = url::Url::parse(&cx.page_url).map_err(|_| "bad_url".to_string())?;
    let mut tries = 0;
    loop {
        let hint = if tries > 0 && crate::youtube::is_youtube(&page) { None } else { job.media_url.as_deref() };
        let t = std::time::Instant::now();
        // The source P6 probed for this job, on the first attempt; a retry or a resumed job probes afresh.
        let handed = cx.core.running.probed.lock().unwrap_or_else(|p| p.into_inner()).take().filter(|(id, _)| tries == 0 && *id == job.id);
        let reused = handed.is_some();
        let src = match handed {
            Some((_, src)) => Ok(src),
            None => api::resolve_source(cx.core, &page, hint).map_err(|r| r.code.to_string()),
        };
        log(format!("timing job {} resolve ms={} hint={} reused={reused} try={tries} ok={}", job.id, t.elapsed().as_millis(), hint.is_some(), src.is_ok()));
        let src = match src {
            Ok(s) => s,
            Err(code) if code == "unreachable" && tries < 3 => {
                tries += 1;
                std::thread::sleep(Duration::from_secs(2 * tries));
                continue;
            }
            Err(code) => return Err(code),
        };
        // The on-device model is fetched here the first time (A4); its share of
        // the stage is the first half when it has to come down.
        let mut last = Stage::new(Stage::DOWNLOADING);
        let t = std::time::Instant::now();
        let fetched = prepare_model(cx, job, &mut |p| cx.percent(&job.id, (p.clamp(0.0, 1.0) * 50.0) as i64, &mut last));
        let fetched = fetched?;
        log(format!("timing job {} model ms={} fetched={fetched}", job.id, t.elapsed().as_millis()));
        let (base, span) = if fetched { (50, 50) } else { (0, 100) };
        let dest = cx.core.cfg.data_dir.join("audio").join(format!("{}.{}", job.id, src.ext()));
        cx.store.set_audio(&job.id, Some(&dest.to_string_lossy()))?;
        let t = std::time::Instant::now();
        let got = media::download(&src, &dest, cx.core.cfg.allow_loopback_media, &mut |p| cx.percent(&job.id, base + p * span / 100, &mut last));
        let bytes = std::fs::metadata(&dest).map_or(0, |m| m.len());
        log(format!("timing job {} transfer ms={} bytes={bytes} try={tries} ok={}", job.id, t.elapsed().as_millis(), got.is_ok()));
        match got {
            Ok(()) => return Ok(()),
            Err(e) if tries < 3 => {
                tries += 1;
                log(format!("job {}: download failed ({e}), retrying", job.id));
                std::thread::sleep(Duration::from_secs(2 * tries));
            }
            Err(e) => {
                log(format!("job {}: download failed: {e}", job.id));
                return Err("download_failed".into());
            }
        }
    }
}

/// The model's `prepare` for `job`. When it will fetch, P7 reports model
/// preparation from before the fetch connects, through its waits and retries,
/// until it ends either way.
fn prepare_model(cx: &Ctx, job: &Job, progress: &mut dyn FnMut(f32)) -> Result<bool, String> {
    let preparing = &cx.core.running.preparing_model;
    if cx.core.svc.transcriber.needs_fetch() {
        *preparing.lock().unwrap_or_else(|p| p.into_inner()) = Some(job.id.clone());
    }
    let got = cx.core.svc.transcriber.prepare(&cx.core.cfg.data_dir, progress);
    *preparing.lock().unwrap_or_else(|p| p.into_inner()) = None;
    got.map_err(|e| if e == UNSUPPORTED_CPU { e } else { format!("model: {e}") })
}

/// Writes the speech regions not yet written, then moves the job to
/// translating. Stages never go back (E2), so a lost audio file fails the job.
fn write(cx: &Ctx, job: &Job) -> Result<(), Stop> {
    let path = job.audio_path.clone().filter(|p| std::path::Path::new(p).is_file());
    let Some(path) = path else {
        return Err("audio file missing while writing".into());
    };
    // A job resumed here after a restart never passed through downloading in
    // this process: make sure the model is on the device (normally a size check).
    prepare_model(cx, job, &mut |_| {})?;
    let t = std::time::Instant::now();
    let pcm = audio::decode_file(path.as_ref()).map_err(|e| format!("decode: {e}"))?;
    log(format!("timing job {} decode ms={} samples={}", job.id, t.elapsed().as_millis(), pcm.len()));
    let t = std::time::Instant::now();
    let regions = audio::plan(job.plan, &pcm);
    let speech_ms: i64 = regions.iter().map(|r| r.end_ms() - r.start_ms()).sum();
    log(format!("timing job {} vad ms={} plan={} regions={} speech_ms={speech_ms} from={}", job.id, t.elapsed().as_millis(), job.plan, regions.len(), job.written_regions));
    let t = std::time::Instant::now();
    cx.core.svc.transcriber.load().map_err(|e| format!("model: {e}"))?;
    log(format!("timing job {} model-load ms={}", job.id, t.elapsed().as_millis()));
    let (mut infer_ms, mut infer_audio_ms, writing) = (0u128, 0i64, std::time::Instant::now());
    let total: usize = regions.iter().map(|r| r.end - r.start).sum::<usize>().max(1);
    let mut done: usize = regions.iter().take(job.written_regions as usize).map(|r| r.end - r.start).sum();
    let mut last = Stage::new(Stage::WRITING);
    let mut prev_end = cx.store.last_cue_end(&job.video_key).map_err(|e| e.to_string())?;
    let mut held: Vec<stitch::Piece> = match job.held.as_deref() {
        Some(h) => serde_json::from_str(h).map_err(|e| format!("held cues: {e}"))?,
        None => Vec::new(),
    };
    let mut tail: crate::sentences::Tail = match job.tail.as_deref() {
        Some(t) => serde_json::from_str(t).map_err(|e| format!("open sentence: {e}"))?,
        None => Vec::new(),
    };
    // This build stores the open sentence as the first held piece too (see
    // below); here it is the tail only. Where it is not the first held piece, a
    // build that ignores `tail` (0.1.9) wrote `held` since and committed those
    // words from it: the tail is stale, and resuming it would write them twice.
    if !tail.is_empty() {
        if held.first().is_some_and(|p| p.words == tail) {
            held.remove(0);
        } else {
            tail.clear();
        }
    }
    // The title helps the PC's model spell the talk's terms (A4); the phone ignores it.
    cx.core.svc.transcriber.set_hint(Some(&cx.title));
    for (i, r) in regions.iter().enumerate().skip(job.written_regions as usize) {
        let len = r.end - r.start;
        let samples: Vec<f32> = pcm[r.start..r.end].iter().map(|&s| s as f32 / 32768.0).collect();
        let t = std::time::Instant::now();
        let segs = cx
            .core
            .svc
            .transcriber
            .transcribe(&samples, &mut |p| cx.percent(&job.id, ((done as f64 + p.clamp(0.0, 1.0) as f64 * len as f64) * 100.0 / total as f64) as i64, &mut last))
            .map_err(|e| format!("model: {e}"))?;
        let ms = t.elapsed().as_millis();
        (infer_ms, infer_audio_ms) = (infer_ms + ms, infer_audio_ms + r.end_ms() - r.start_ms());
        log(format!("timing job {} region i={i} start_ms={} audio_ms={} ms={ms} segs={}", job.id, r.start_ms(), r.end_ms() - r.start_ms(), segs.len()));
        let prev = i.checked_sub(1).map(|p| &regions[p]);
        let cues;
        (cues, held) = stitch::window_cues(r, segs, std::mem::take(&mut held), prev, regions.get(i + 1), &mut tail, &mut prev_end);
        // A build from before sentence cues resumes from `held` and ignores
        // `tail`, so the open sentence's words, already past the resume point,
        // also go first in `held`: such a build commits them as a cue of their
        // own instead of losing them.
        let mut keep = held.clone();
        if let (Some(a), Some(b)) = (tail.first(), tail.last()) {
            keep.insert(0, stitch::Piece { start_ms: a.start_ms, end_ms: b.end_ms, words: tail.clone() });
        }
        let kept = (!keep.is_empty()).then(|| serde_json::to_string(&keep).unwrap());
        let open = (!tail.is_empty()).then(|| serde_json::to_string(&tail).unwrap());
        cx.store.save_window(&job.id, &job.video_key, i as i64, &cues, kept.as_deref(), open.as_deref()).map_err(|e| e.to_string())?;
        done += len;
        cx.percent(&job.id, (done * 100 / total) as i64, &mut last);
    }
    log(format!("timing job {} infer ms={infer_ms} audio_ms={infer_audio_ms} wall_ms={}", job.id, writing.elapsed().as_millis()));
    if cx.store.cues(&job.video_key)?.is_empty() {
        return Err("no_speech".into());
    }
    if let Some(lang) = cx.core.svc.transcriber.language() {
        let _ = cx.store.set_language(&job.video_key, &lang);
    }
    // Record the stage change first; only then is the audio no longer needed.
    cx.store.end_writing(&job.id)?;
    drop_audio(cx.store, &job.id, &path);
    Ok(())
}

/// Deletes a job's audio file, and only then forgets its path: a process
/// killed in between, or a delete that fails, leaves the path recorded, and
/// the worker's next [`sweep_audio`] tries again.
fn drop_audio(store: &Store, job_id: &str, path: &str) {
    // The HLS sections index goes first: an index without its audio is never read.
    let _ = std::fs::remove_file(media::sections_path(Path::new(path)));
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => log(format!("job {job_id}: audio not deleted yet: {e}")),
        _ => {
            let _ = store.set_audio(job_id, None);
        }
    }
}

/// Deletes the audio of every job past writing that still has one (A3).
fn sweep_audio(store: &Store) {
    for (id, path) in store.leftover_audio().unwrap_or_default() {
        drop_audio(store, &id, &path);
    }
}

/// How often the glossary is asked for before the job fails without one.
const GLOSSARY_TRIES: usize = 3;

/// Translation requests waiting for replies at once. Each is still one
/// sentence with only what came before it (A6), so running them side by side
/// changes no request and no reply; only the waiting overlaps.
const IN_FLIGHT: usize = 8;

/// How many passes in a row may save nothing before the job stops. A sentence
/// is only ever asked for whole, so one whose replies keep merging or dropping
/// parts stops the job (`translate_failed`) rather than being saved part by
/// part; a re-translation asks again for what is left.
const STALLED_PASSES: u32 = 3;

/// The cues of `cues` grouped by sentence, in order: a cue whose seq `parts`
/// holds continues the group before it. Each group is `(seq, source)` pairs.
fn sentences(cues: &[crate::store::Cue], parts: &std::collections::HashSet<i64>) -> Vec<Vec<(i64, String)>> {
    let mut out: Vec<Vec<(i64, String)>> = Vec::new();
    for c in cues {
        match out.last_mut() {
            Some(g) if parts.contains(&c.seq) && g.last().is_some_and(|(s, _)| *s == c.seq - 1) => g.push((c.seq, c.source_text.clone())),
            _ => out.push(vec![(c.seq, c.source_text.clone())]),
        }
    }
    out
}

fn translate(cx: &Ctx, job_id: &str) -> Result<(), Stop> {
    let store = cx.store;
    let job = store.job(job_id)?.ok_or("job vanished")?;
    let video_key = &job.video_key;
    let cues = store.translation_cues(&job)?;
    if cues.is_empty() {
        return Err("no_speech".into());
    }
    let key = cx.core.svc.keystore.get()?.ok_or_else(|| "no_key".to_string())?;
    let model = store.job(job_id)?.map(|j| j.translation_model).unwrap_or_else(|| gemini::MODEL.into());
    let client = Client { base_url: cx.core.cfg.gemini_base_url.clone(), key, model };

    // The worker's connection holds one transaction at a time, so every store
    // access of the calls below (usage, translations, progress) is made
    // holding `ledger`; only the requests themselves run side by side.
    let ledger = Mutex::new(());

    // Every call is billed, so its usage is added to the job's cost after it (A6).
    let call = |order: &Value| -> Result<Option<Value>, Stop> {
        let mut attempts = 0u32;
        loop {
            let (reply, usage) = match client.order(order) {
                Ok((v, u)) => (Some(v), u),
                Err(gemini::Error::Reply(e, u)) => {
                    log(format!("job {job_id}: unusable reply: {e}"));
                    (None, u)
                }
                Err(gemini::Error::Key(e)) => {
                    log(format!("job {job_id}: key refused: {e}"));
                    return Err("key_rejected".into());
                }
                Err(gemini::Error::Transient(e)) => {
                    attempts += 1;
                    if attempts > 8 {
                        log(format!("job {job_id}: Gemini unreachable: {e}"));
                        return Err("network".into());
                    }
                    let wait = (1u64 << attempts.min(5)).min(30);
                    log(format!("job {job_id}: Gemini failed ({e}), retry in {wait}s"));
                    std::thread::sleep(Duration::from_secs(wait));
                    continue;
                }
            };
            let _held = ledger.lock().unwrap();
            store.add_usage(job_id, usage.input, usage.output, gemini::cost_krw)?;
            return Ok(reply);
        }
    };

    let video = store.video(&video_key)?.ok_or("internal")?;
    let terms = video.terms.as_deref().and_then(|t| serde_json::from_str::<Vec<Value>>(t).ok());
    let found = store.current_mishearings(video_key)?.and_then(|m| serde_json::from_str::<Vec<Value>>(&m).ok());
    let left = cues.iter().any(|c| c.text_ko.is_none());
    let (glossary, mishearings): (Vec<Value>, Vec<Value>) = match (terms, found) {
        (Some(g), Some(m)) => (g, m),
        (Some(g), None) if !left => (g, Vec::new()),
        // No glossary yet, or a saved version's glossary without mishearings
        // found in its own transcript (none kept, or another transcript's left
        // by a build that does not know them): the glossary request finds them;
        // the version's own terms stay.
        (terms, _) => {
            let transcript: Vec<String> = cues.iter().map(|c| c.source_text.clone()).collect();
            let glossary_started = std::time::Instant::now();
            let order = gemini::glossary_order(&video.title, &transcript);
            // Every request carries the glossary (A6, R9): a reply that is not one is
            // asked for again, and a job that gets none fails instead of translating
            // without it. A list with no terms in it is a glossary (a talk may have none).
            let mut g = None;
            for _ in 0..GLOSSARY_TRIES {
                if let Some(r) = call(&order)?.filter(|r| r["terms"].is_array()) {
                    g = Some((gemini::parse_terms(&r), gemini::parse_mishearings(&r)));
                    break;
                }
                log(format!("job {job_id}: the glossary reply has no term list"));
            }
            log(format!("timing job {job_id} glossary ms={} cues={}", glossary_started.elapsed().as_millis(), transcript.len()));
            let (g, m) = g.ok_or_else(|| "glossary_failed".to_string())?;
            let g = terms.unwrap_or(g);
            store.set_glossary(&video_key, &Value::from(g.clone()).to_string(), &Value::from(m.clone()).to_string())?;
            (g, m)
        }
    };

    // The sentences are a function of the saved cues alone, so the first
    // translation, a re-translation and a version selected again or received
    // from another device all ask the same groups.
    let groups = sentences(&cues, &crate::sentences::rebuilt_parts(&cues));
    let total = cues.len() as i64;
    let last = Mutex::new(Stage::new(Stage::TRANSLATING));
    let mut stalls = 0;
    let (translating, calls) = (std::time::Instant::now(), AtomicUsize::new(0));
    loop {
        let done: std::collections::HashSet<i64> = store.translation_cues(&job)?.into_iter().filter(|c| c.text_ko.is_some()).map(|c| c.seq).collect();
        cx.percent(job_id, done.len() as i64 * 100 / total, &mut last.lock().unwrap());
        // Every sentence with a part left goes whole, all its parts in one request.
        let todo: Vec<(usize, Vec<(i64, String)>)> = groups
            .iter()
            .enumerate()
            .filter(|(_, group)| group.iter().any(|(s, _)| !done.contains(s)))
            .map(|(g, group)| (g, group.clone()))
            .collect();
        if todo.is_empty() {
            log(format!(
                "timing job {job_id} translate ms={} cues={total} sentences={} calls={} in_flight={IN_FLIGHT}",
                translating.elapsed().as_millis(),
                groups.len(),
                calls.load(Ordering::Relaxed)
            ));
            return Ok(());
        }
        let (units, progressed, stop) = (Mutex::new(todo.iter()), AtomicBool::new(false), Mutex::new(None::<Stop>));
        // One request per sentence (or part); IN_FLIGHT of them waiting for replies at once.
        let one = |(g, asked): &(usize, Vec<(i64, String)>)| -> Result<(), Stop> {
            let mut order = gemini::translate_order(&glossary, &mishearings, asked);
            // The sentence before, read-only; never anything said later (A6).
            if let Some(previous) = g.checked_sub(1).map(|p| &groups[p]) {
                order["context_before"] = Value::String(previous.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>().join(" "));
            }
            calls.fetch_add(1, Ordering::Relaxed);
            let reply = call(&order)?;
            let ko = reply.as_ref().map(|r| gemini::parse_translations(r, asked)).unwrap_or_default();
            let _held = ledger.lock().unwrap();
            if ko.is_empty() {
                if let Some(r) = &reply {
                    // What came back, without its text: the ids and which `ko` were empty.
                    let got: Vec<String> = r["segments"].as_array().into_iter().flatten()
                        .map(|s| format!("{}{}", s["id"], if s["ko"].as_str().is_some_and(|k| !k.trim().is_empty()) { "" } else { ":empty" }))
                        .collect();
                    log(format!("job {job_id}: the reply for sentence {} did not answer its {} parts; it held [{}]", groups[*g][0].0, asked.len(), got.join(",")));
                }
                return Ok(());
            }
            progressed.store(true, Ordering::Relaxed);
            store.set_translations(job_id, &ko)?;
            let left = store.translation_cues(&job)?.iter().filter(|c| c.text_ko.is_none()).count() as i64;
            cx.percent(job_id, (total - left) * 100 / total, &mut last.lock().unwrap());
            Ok(())
        };
        std::thread::scope(|s| {
            for _ in 0..IN_FLIGHT {
                s.spawn(|| loop {
                    let next = if stop.lock().unwrap().is_some() { None } else { units.lock().unwrap().next() };
                    let Some(unit) = next else { return };
                    if let Err(e) = one(unit) {
                        stop.lock().unwrap().get_or_insert(e);
                    }
                });
            }
        });
        if let Some(e) = stop.into_inner().unwrap() {
            return Err(e);
        }
        stalls = if progressed.into_inner() { 0 } else { stalls + 1 };
        if stalls >= STALLED_PASSES {
            return Err("translation_incomplete".into());
        }
    }
}

#[cfg(test)]
#[path = "pipeline_worker_tests.rs"]
pub(crate) mod worker_tests;

#[cfg(test)]
#[path = "pipeline_audio_tests.rs"]
mod audio_tests;

#[cfg(test)]
#[path = "pipeline_boundary_tests.rs"]
mod boundary_tests;

#[cfg(test)]
#[path = "pipeline_translate_tests.rs"]
mod translate_tests;

#[cfg(test)]
#[path = "pipeline_download_tests.rs"]
mod download_tests;

#[cfg(test)]
#[path = "pipeline_glossary_tests.rs"]
mod glossary_tests;

#[cfg(test)]
mod tests {
    use super::failure_reason;

    #[test]
    fn every_stop_maps_to_one_of_the_e2_reasons() {
        let cases = [
            ("no_key", "translating", "key_rejected"),
            ("key_rejected", "translating", "key_rejected"),
            ("no_speech", "writing", "no_speech"),
            ("network", "translating", "translate_failed"),
            ("translation_incomplete", "translating", "translate_failed"),
            ("unreachable", "downloading", "download_failed"),
            ("model: file missing", "downloading", "download_failed"),
            ("unsupported_cpu", "downloading", "unsupported_cpu"),
            ("unsupported_cpu", "writing", "unsupported_cpu"),
            ("decode: bad header", "writing", "download_failed"),
        ];
        for (detail, stage, want) in cases {
            assert_eq!(failure_reason(detail, stage), want, "{detail} at {stage}");
        }
    }
}
