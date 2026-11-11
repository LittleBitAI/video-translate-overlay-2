//! P1-P8: what each command does, independent of Tauri.

use crate::gemini::{self, MODEL};
use crate::media::{self, Refusal};
use crate::{log, youtube, Core};
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ApiError {
    pub status: u16,
    pub error: String,
    pub message: String,
}

pub type ApiResult = Result<Value, ApiError>;

pub fn err(status: u16, error: &str, message: impl Into<String>) -> ApiError {
    ApiError { status, error: error.into(), message: message.into() }
}

impl From<String> for ApiError {
    fn from(e: String) -> Self {
        log(format!("internal error: {e}"));
        err(500, "internal", "앱 안에서 문제가 생겼습니다.")
    }
}

impl From<Refusal> for ApiError {
    fn from(r: Refusal) -> Self {
        err(if r.code == "bad_url" { 400 } else { 422 }, r.code, r.message)
    }
}

fn http_url(raw: &str) -> Result<url::Url, ApiError> {
    let bad = || err(400, "bad_url", "http나 https 주소만 열 수 있습니다.");
    let u = url::Url::parse(raw.trim()).map_err(|_| bad())?;
    if !matches!(u.scheme(), "http" | "https") || u.host_str().is_none() {
        return Err(bad());
    }
    Ok(u)
}

/// The stable key of the video a page shows.
pub fn video_key(page: &url::Url) -> String {
    if let Some(id) = youtube::video_id(page) {
        return format!("yt-{id}");
    }
    let mut u = page.clone();
    u.set_fragment(None);
    // FNV-1a: short and stable across runs and builds.
    let h = u.as_str().bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3));
    format!("web-{h:016x}")
}

fn site(page: &url::Url) -> String {
    if youtube::is_youtube(page) {
        "youtube".into()
    } else {
        page.host_str().unwrap_or("").trim_start_matches("www.").to_string()
    }
}

fn new_job_id() -> String {
    static N: AtomicU64 = AtomicU64::new(0);
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos() as u64;
    format!("job-{t:016x}-{:04x}", N.fetch_add(1, Ordering::Relaxed) & 0xffff)
}

// ---------------------------------------------------------------- P1, P2

pub fn browser_open(core: &Core, raw: &str) -> ApiResult {
    let page = http_url(raw)?;
    let site = site(&page);
    if site == "youtube" && !core.store.youtube_gate_seen()? {
        // Sign-in happens on Google's own page; the app never sees a password.
        let mut gate = url::Url::parse("https://accounts.google.com/ServiceLogin").unwrap();
        gate.query_pairs_mut().append_pair("service", "youtube").append_pair("continue", raw.trim());
        core.svc.browser.navigate(gate.as_str(), false)?;
        core.store.set_youtube_gate_seen(true)?;
        return Ok(json!({"opened_url": gate.as_str(), "site": site, "account_gate": true}));
    }
    core.svc.browser.navigate(raw.trim(), true)?;
    Ok(json!({"opened_url": raw.trim(), "site": site, "account_gate": false}))
}

pub fn clear_site_data(core: &Core) -> ApiResult {
    core.svc.browser.clear_site_data()?;
    core.store.set_youtube_gate_seen(false)?;
    Ok(json!({"cleared": true}))
}

// ---------------------------------------------------------------- P3-P5

fn masked(last4: &str) -> String {
    format!("••••{last4}")
}

pub fn get_settings(core: &Core) -> ApiResult {
    let key_set = core.svc.keystore.get()?.is_some();
    let last4 = if key_set { core.store.key_last4()? } else { None };
    Ok(json!({
        "key_set": key_set,
        "key_masked": last4.as_deref().map(masked),
        "translation_model": MODEL,
        "favorite_voice_available": false,
    }))
}

pub fn put_api_key(core: &Core, api_key: &str) -> ApiResult {
    let key = api_key.trim();
    if !(20..=200).contains(&key.len()) || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
        return Err(err(400, "bad_key", "Gemini API 키 모양이 아닙니다."));
    }
    let client = gemini::Client { base_url: core.cfg.gemini_base_url.clone(), key: key.into(), model: MODEL.into() };
    match client.check_key() {
        Ok(()) => {}
        Err(gemini::Error::Key(e)) => {
            log(format!("key check refused: {e}"));
            return Err(err(401, "key_rejected", "Gemini가 이 키를 받아 주지 않았습니다."));
        }
        Err(e) => {
            log(format!("key check failed: {e:?}"));
            return Err(err(503, "gemini_unreachable", "Gemini에 닿지 않아 키를 확인하지 못했습니다."));
        }
    }
    core.svc.keystore.set(key)?;
    let last4 = &key[key.len() - 4..];
    core.store.set_key_last4(Some(last4))?;
    log(format!("api key stored (…{last4})"));
    Ok(json!({"key_set": true, "key_masked": masked(last4)}))
}

pub fn delete_api_key(core: &Core) -> ApiResult {
    core.svc.keystore.delete()?;
    core.store.set_key_last4(None)?;
    log("api key deleted");
    Ok(json!({"key_set": false}))
}

// ---------------------------------------------------------------- P6-P8

/// Where a job's audio comes from: the YouTube extractor, or the overlay's media URL.
pub fn resolve_source(core: &Core, page: &url::Url, media_url: Option<&str>) -> Result<media::Source, Refusal> {
    let raw = if youtube::is_youtube(page) {
        core.svc.extractor.audio_url(page.as_str())?
    } else {
        media_url
            .filter(|m| !m.trim().is_empty())
            .ok_or_else(|| media::refuse("no_media", "이 페이지에서 영상 파일을 찾지 못했습니다."))?
            .trim()
            .to_string()
    };
    let url = media::check_media_url(&raw, core.cfg.allow_loopback_media)?;
    media::probe(&url, core.cfg.allow_loopback_media)
}

pub fn create_job(core: &Core, page_url: &str, media_url: Option<&str>, title: &str, duration_ms: i64) -> ApiResult {
    let page = http_url(page_url)?;
    let key = video_key(&page);

    // Subtitles made before are shown again without any work (and without a key).
    if let Some(last) = core.store.latest_job(&key)? {
        if last.stage == "ready" {
            return Ok(json!({"job_id": last.id, "video_key": key, "stage": "ready", "reused": true}));
        }
    }
    if let Some(active) = core.store.active_job()? {
        if active.video_key == key {
            return Ok(json!({"job_id": active.id, "video_key": key, "stage": active.stage, "reused": false}));
        }
        return Err(err(409, "busy", "다른 영상의 작업이 돌고 있습니다. 그 작업이 끝난 뒤에 다시 눌러 주세요."));
    }
    if core.svc.keystore.get()?.is_none() {
        return Err(err(428, "no_key", "설정에서 Gemini API 키를 먼저 넣어 주세요."));
    }
    if let Some(m) = media_url.filter(|m| !m.trim().is_empty()) {
        http_url(m).map_err(|_| err(400, "bad_url", "media_url은 http나 https 주소여야 합니다."))?;
    }
    resolve_source(core, &page, media_url)?;

    let id = new_job_id();
    let title = if title.trim().is_empty() { page_url.trim() } else { title.trim() };
    core.store.upsert_video(&key, page_url.trim(), &site(&page), title, duration_ms.max(0))?;
    core.store.reset_video(&key)?;
    core.store.insert_job(&id, &key, media_url.map(str::trim).filter(|m| !m.is_empty()), MODEL)?;
    core.svc.notifier.ongoing(&id, "shown", title, 0);
    log(format!("job {id} created for {key}"));
    core.wake_worker();
    Ok(json!({"job_id": id, "video_key": key, "stage": "downloading", "reused": false}))
}

pub fn get_job(core: &Core, page_url: &str) -> ApiResult {
    let key = video_key(&http_url(page_url)?);
    let job = core.store.latest_job(&key)?.ok_or_else(|| err(404, "no_job", "이 영상의 작업이 없습니다."))?;
    let title = core.store.video(&key)?.map(|v| v.title).unwrap_or_default();
    Ok(json!({
        "job_id": job.id,
        "video_key": key,
        "title": title,
        "stage": job.stage,
        "percent": job.percent,
        "failure_reason": job.failure_reason,
        "input_amount": job.input_amount,
        "output_amount": job.output_amount,
        "cost_krw": job.cost_krw,
        "notified": job.notified,
    }))
}

pub fn get_subtitles(core: &Core, video_key: &str) -> ApiResult {
    let video = core.store.video(video_key)?.ok_or_else(|| err(404, "no_video", "이 영상의 자막이 없습니다."))?;
    let ready = core.store.latest_job(video_key)?.is_some_and(|j| j.stage == "ready");
    if !ready {
        return Err(err(409, "not_ready", "자막을 만드는 중입니다. 다 되면 알림으로 알려 드립니다."));
    }
    let cues: Vec<Value> = core
        .store
        .cues(video_key)?
        .into_iter()
        .map(|c| json!({"seq": c.seq, "start_ms": c.start_ms, "end_ms": c.end_ms, "text_ko": c.text_ko.unwrap_or(c.source_text)}))
        .collect();
    Ok(json!({
        "video_key": video.video_key,
        "duration_ms": video.duration_ms,
        "source_language": video.source_language.unwrap_or_else(|| "und".into()),
        "cues": cues,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_keys_follow_the_video_not_the_url_form() {
        let k = |s: &str| video_key(&url::Url::parse(s).unwrap());
        assert_eq!(k("https://www.youtube.com/watch?v=abcDEF12345&t=30"), "yt-abcDEF12345");
        assert_eq!(k("https://youtu.be/abcDEF12345"), "yt-abcDEF12345");
        assert_eq!(k("https://m.youtube.com/shorts/abcDEF12345"), "yt-abcDEF12345");
        assert_eq!(k("https://a.example/talk#x"), k("https://a.example/talk"));
        assert_ne!(k("https://a.example/talk"), k("https://a.example/talk2"));
    }
}
