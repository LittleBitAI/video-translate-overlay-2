//! The background worker: one job at a time, every step recorded in SQLite
//! so that a killed process picks up where it stopped.

use crate::gemini::{self, Client};
use crate::store::{Job, Store};
use crate::{api, audio, log, media, Core};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

/// Why a job stopped for good; becomes `failure_reason`.
type Stop = String;

pub fn run_worker(core: Arc<Core>) {
    // Its own connection, so the worker's transactions never mix with the commands' writes.
    let store = match Store::open(&core.db_path()) {
        Ok(s) => s,
        Err(e) => return log(format!("worker cannot open the database: {e}")),
    };
    let mut resumed = false;
    loop {
        match store.active_job() {
            Ok(Some(job)) => {
                if !resumed {
                    log(format!("resuming job {} at {}", job.id, job.stage));
                    let title = store.video(&job.video_key).ok().flatten().map(|v| v.title).unwrap_or_default();
                    core.svc.notifier.ongoing(&job.id, "shown", &title, job.percent);
                }
                resumed = true;
                run_job(&core, &store, job);
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

struct Ctx<'a> {
    core: &'a Core,
    store: &'a Store,
    title: String,
    page_url: String,
    duration_ms: i64,
}

impl Ctx<'_> {
    fn percent(&self, job: &str, p: i64, last: &mut i64) {
        if p > *last {
            *last = p;
            let _ = self.store.raise_percent(job, p);
            if p % 10 == 0 {
                self.core.svc.notifier.ongoing(job, "updated", &self.title, p);
            }
        }
    }
}

fn run_job(core: &Core, store: &Store, job: Job) {
    let Ok(Some(video)) = store.video(&job.video_key) else {
        let _ = store.fail(&job.id, "internal");
        return;
    };
    let cx = Ctx { core, store, title: video.title.clone(), page_url: video.page_url.clone(), duration_ms: video.duration_ms };
    let result = (|| -> Result<(), Stop> {
        let db = |e: String| -> Stop {
            log(format!("job {}: {e}", job.id));
            "internal".into()
        };
        let mut stage = job.stage.clone();
        if stage == "downloading" {
            download(&cx, &job)?;
            store.set_stage(&job.id, "writing").map_err(db)?;
            stage = "writing".into();
        }
        if stage == "writing" {
            let fresh = store.job(&job.id).map_err(db)?.ok_or("internal")?;
            if !write(&cx, &fresh)? {
                return Ok(()); // audio was gone: back to downloading, the worker picks it up again
            }
            store.set_stage(&job.id, "translating").map_err(db)?;
        }
        translate(&cx, &job.id)?;
        let text = format!("한국어 자막이 준비됐습니다: {}", cx.title);
        core.svc.notifier.ready(&job.id, &text, &cx.page_url);
        core.svc.notifier.ongoing(&job.id, "hidden", &cx.title, 100);
        store.finish(&job.id).map_err(db)?;
        log(format!("job {} ready", job.id));
        Ok(())
    })();
    if let Err(reason) = result {
        log(format!("job {} failed: {reason}", job.id));
        if let Ok(Some(j)) = store.job(&job.id) {
            if let Some(p) = j.audio_path {
                let _ = std::fs::remove_file(p);
            }
        }
        let _ = store.fail(&job.id, &reason);
        core.svc.notifier.ongoing(&job.id, "hidden", &cx.title, 0);
    }
}

fn download(cx: &Ctx, job: &Job) -> Result<(), Stop> {
    let page = url::Url::parse(&cx.page_url).map_err(|_| "bad_url".to_string())?;
    let mut tries = 0;
    loop {
        let src = api::resolve_source(cx.core, &page, job.media_url.as_deref()).map_err(|r| r.code.to_string());
        let src = match src {
            Ok(s) => s,
            Err(code) if code == "unreachable" && tries < 3 => {
                tries += 1;
                std::thread::sleep(Duration::from_secs(2 * tries));
                continue;
            }
            Err(code) => return Err(code),
        };
        let dest = cx.core.cfg.data_dir.join("audio").join(format!("{}.{}", job.id, src.ext()));
        cx.store.set_audio(&job.id, Some(&dest.to_string_lossy())).map_err(|e| e.to_string())?;
        let mut last = -1;
        match media::download(&src, &dest, &mut |p| cx.percent(&job.id, p, &mut last)) {
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

/// Writes the speech regions not yet written. Returns false when the audio
/// file is gone and the job went back to downloading.
fn write(cx: &Ctx, job: &Job) -> Result<bool, Stop> {
    let path = job.audio_path.clone().filter(|p| std::path::Path::new(p).is_file());
    let Some(path) = path else {
        log(format!("job {}: audio missing, downloading again", job.id));
        cx.store.set_stage(&job.id, "downloading").map_err(|e| e.to_string())?;
        return Ok(false);
    };
    let pcm = audio::decode_file(path.as_ref()).map_err(|e| {
        log(format!("job {}: {e}", job.id));
        "decode_failed".to_string()
    })?;
    if cx.duration_ms <= 0 {
        let _ = cx.store.upsert_video(&job.video_key, &cx.page_url, "", &cx.title, (pcm.len() * 1000 / audio::RATE) as i64);
    }
    let regions = audio::speech_regions(&pcm);
    let total: usize = regions.iter().map(|r| r.end - r.start).sum::<usize>().max(1);
    let mut done: usize = regions.iter().take(job.written_regions as usize).map(|r| r.end - r.start).sum();
    let mut last = -1;
    let mut prev_end = cx.store.last_cue_end(&job.video_key).map_err(|e| e.to_string())?;
    for (i, r) in regions.iter().enumerate().skip(job.written_regions as usize) {
        let len = r.end - r.start;
        let samples: Vec<f32> = pcm[r.start..r.end].iter().map(|&s| s as f32 / 32768.0).collect();
        let segs = cx
            .core
            .svc
            .transcriber
            .transcribe(&samples, &mut |p| cx.percent(&job.id, ((done as f64 + p.clamp(0.0, 1.0) as f64 * len as f64) * 100.0 / total as f64) as i64, &mut last))
            .map_err(|e| {
                log(format!("job {}: model: {e}", job.id));
                "write_failed".to_string()
            })?;
        let mut cues = Vec::new();
        for s in segs {
            let text = s.text.trim().to_string();
            let start = (r.start_ms() + s.start_ms).max(prev_end);
            let end = (r.start_ms() + s.end_ms).min(r.end_ms());
            if text.is_empty() || end <= start {
                continue;
            }
            prev_end = end;
            cues.push((start, end, text));
        }
        cx.store.save_region(&job.id, &job.video_key, i as i64, &cues).map_err(|e| e.to_string())?;
        done += len;
        cx.percent(&job.id, (done * 100 / total) as i64, &mut last);
    }
    if let Some(lang) = cx.core.svc.transcriber.language() {
        let _ = cx.store.set_language(&job.video_key, &lang);
    }
    let _ = std::fs::remove_file(&path);
    cx.store.set_audio(&job.id, None).map_err(|e| e.to_string())?;
    Ok(true)
}

fn translate(cx: &Ctx, job_id: &str) -> Result<(), Stop> {
    let store = cx.store;
    let video_key = store.job(job_id)?.ok_or("internal")?.video_key;
    let cues = store.cues(&video_key)?;
    if cues.is_empty() {
        return Ok(());
    }
    let key = cx.core.svc.keystore.get()?.ok_or_else(|| "no_key".to_string())?;
    let model = store.job(job_id)?.map(|j| j.translation_model).unwrap_or_else(|| gemini::MODEL.into());
    let client = Client { base_url: cx.core.cfg.gemini_base_url.clone(), key, model };
    let limit = gemini::limit_krw(store.video(&video_key)?.map(|v| v.duration_ms).unwrap_or(0));

    // Every call is billed, so usage is added and the limit checked after each one.
    let mut attempts = 0u32;
    let mut call = |order: &Value| -> Result<Option<Value>, Stop> {
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
            let j = store.add_usage(job_id, usage.input, usage.output, gemini::cost_krw)?;
            if j.cost_krw > limit {
                log(format!("job {job_id}: cost {:.2} KRW passed the limit {limit:.2}", j.cost_krw));
                return Err("cost_limit".into());
            }
            return Ok(reply);
        }
    };

    let video = store.video(&video_key)?.ok_or("internal")?;
    let glossary: Vec<Value> = match video.terms.as_deref().and_then(|t| serde_json::from_str::<Vec<Value>>(t).ok()) {
        Some(g) => g,
        None => {
            let transcript: Vec<String> = cues.iter().map(|c| c.source_text.clone()).collect();
            let g = call(&gemini::glossary_order(&video.title, &transcript))?.map(|r| gemini::parse_terms(&r)).unwrap_or_default();
            store.set_terms(&video_key, &Value::from(g.clone()).to_string())?;
            g
        }
    };

    let total = cues.len() as i64;
    let mut last = -1;
    let mut stalls = 0;
    loop {
        let todo: Vec<(i64, String)> = store.cues(&video_key)?.into_iter().filter(|c| c.text_ko.is_none()).map(|c| (c.seq, c.source_text)).collect();
        cx.percent(job_id, (total - todo.len() as i64) * 100 / total, &mut last);
        if todo.is_empty() {
            return Ok(());
        }
        let mut progressed = false;
        for batch in todo.chunks(gemini::BATCH) {
            let Some(reply) = call(&gemini::translate_order(&glossary, batch))? else { continue };
            let ko = gemini::parse_translations(&reply, batch);
            progressed |= !ko.is_empty();
            store.set_translations(&video_key, &ko)?;
            let left = store.cues(&video_key)?.iter().filter(|c| c.text_ko.is_none()).count() as i64;
            cx.percent(job_id, (total - left) * 100 / total, &mut last);
        }
        stalls = if progressed { 0 } else { stalls + 1 };
        if stalls >= 2 {
            return Err("translation_incomplete".into());
        }
    }
}
