//! P1-P8: what each command does, independent of Tauri.

use crate::gemini::{self, MODEL};
use crate::media::{self, Refusal};
use crate::{log, youtube, Core};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ApiError {
    pub status: u16,
    pub error: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deletion_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_generation: Option<u64>,
}

pub type ApiResult = Result<Value, ApiError>;

pub fn open_privacy_policy(core: &Core) -> ApiResult {
    core.svc.browser.open_privacy_policy()
        .map_err(|_| err(503, "privacy_unavailable", "개인정보처리방침을 열지 못했습니다. 다시 시도해 주세요."))?;
    Ok(json!({"opened":true,"url":crate::PRIVACY_POLICY_URL}))
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackAction { Get, Set }

pub fn err(status: u16, error: &str, message: impl Into<String>) -> ApiError {
    ApiError { status, error: error.into(), message: message.into(), deletion_token: None, account_generation: None }
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

/// Query parameters that say how a link was shared, not which video it shows.
const TRACKING: [&str; 14] =
    ["fbclid", "gclid", "dclid", "msclkid", "yclid", "igshid", "mc_cid", "mc_eid", "si", "feature", "ref", "ref_src", "spm", "_ga"];

fn is_tracking(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.starts_with("utm_") || TRACKING.contains(&n.as_str())
}

/// The stable key of the video a page shows (E1): the YouTube video id, or
/// the SHA-256 of the page URL without its fragment and tracking parameters.
pub fn video_key(page: &url::Url) -> String {
    if let Some(id) = youtube::video_id(page) {
        return id;
    }
    let mut u = page.clone();
    u.set_fragment(None);
    let kept: Vec<(String, String)> = u.query_pairs().filter(|(k, _)| !is_tracking(k)).map(|(k, v)| (k.into_owned(), v.into_owned())).collect();
    if kept.is_empty() {
        u.set_query(None);
    } else {
        u.query_pairs_mut().clear().extend_pairs(kept);
    }
    use sha2::{Digest, Sha256};
    Sha256::digest(u.as_str().as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// `youtube` or `other` (E1).
fn site(page: &url::Url) -> &'static str {
    if youtube::is_youtube(page) {
        "youtube"
    } else {
        "other"
    }
}

/// A random (version 4) UUID.
fn new_job_id() -> String {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).expect("OS random source");
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

/// Titles are cut at 200 characters (E1).
fn clip_title(title: &str, fallback: &str) -> String {
    let t = if title.trim().is_empty() { fallback.trim() } else { title.trim() };
    t.chars().take(200).collect()
}

// ---------------------------------------------------------------- P1, P2

/// Only Google's YouTube sign-in screen can keep a return address.
fn youtube_sign_in_target(page: &url::Url) -> Option<url::Url> {
    if page.scheme() != "https" || page.host_str() != Some("accounts.google.com") || page.path() != "/ServiceLogin" {
        return None;
    }
    let target = page.query_pairs().find(|(k, _)| k == "continue")?.1;
    let mut target = http_url(&target).ok()?;
    if !youtube::is_youtube(&target) {
        return None;
    }
    target.set_scheme("https").ok()?;
    target.set_port(None).ok()?;
    Some(target)
}

/// A finished sign-in must not restore its login page or replace a newer video.
pub fn youtube_page_loaded(core: &Core, raw: &str) -> Result<(), String> {
    let Ok(page) = http_url(raw) else { return Ok(()) };
    let library = core.library(None).map_err(|_| "account changed")?;
    let Some(gate) = library.run(|store| Ok(store.opened_url()?)).map_err(|_| "account changed")? else { return Ok(()) };
    let Some(target) = url::Url::parse(&gate).ok().as_ref().and_then(youtube_sign_in_target) else { return Ok(()) };
    let home = target.path() == "/" && target.query().is_none() && page.path() == "/";
    if page.scheme() == "https" && youtube::is_youtube(&page) && (home || video_key(&page) == video_key(&target)) {
        library.run(|store| Ok(store.finish_youtube_sign_in(&gate, raw)?)).map_err(|_| "account changed")?;
    }
    Ok(())
}

pub fn browser_open(core: &Core, raw: &str) -> ApiResult {
    browser_open_in(&core.library(None)?, raw)
}

fn browser_open_in(library: &crate::library_scope::Library<'_>, raw: &str) -> ApiResult {
    let core = library.core;
    let page = http_url(raw)?;
    let site = site(&page);
    if youtube_sign_in_target(&page).is_some() {
        core.svc.browser.navigate(page.as_str(), false)?;
        core.store.set_youtube_gate_seen(true)?;
        library.run(|store| Ok(store.set_opened_url(page.as_str())?))?;
        return Ok(json!({"opened_url": page.as_str(), "site": "youtube", "account_gate": true}));
    }
    if site == "youtube" && !core.store.youtube_gate_seen()? {
        // Sign-in happens on Google's own page; the app never sees a password.
        let mut gate = url::Url::parse("https://accounts.google.com/ServiceLogin").unwrap();
        gate.query_pairs_mut().append_pair("service", "youtube").append_pair("continue", raw.trim());
        core.svc.browser.navigate(gate.as_str(), false)?;
        core.store.set_youtube_gate_seen(true)?;
        library.run(|store| Ok(store.set_opened_url(gate.as_str())?))?;
        return Ok(json!({"opened_url": gate.as_str(), "site": site, "account_gate": true}));
    }
    core.svc.browser.navigate(raw.trim(), true)?;
    library.run(|store| Ok(store.set_opened_url(raw.trim())?))?;
    Ok(json!({"opened_url": raw.trim(), "site": site, "account_gate": false}))
}

/// P9: the page P1 last sent the in-app browser to (Google's account screen
/// while it is shown), so the first screen can show it again after a restart,
/// and that page's latest job as P7 gives it (`null` when it has none), so
/// opening the screen asks nothing that answers "not found".
pub fn get_browser(core: &Core) -> ApiResult {
    get_browser_in(&core.library(None)?)
}

fn get_browser_in(library: &crate::library_scope::Library<'_>) -> ApiResult {
    let url = library.run(|store| Ok(store.opened_url()?))?;
    let job = match url.as_deref().map(|u| get_job_in(library, u)) {
        Some(Ok(job)) => job,
        Some(Err(e)) if e.status != 404 && e.status != 410 => return Err(e),
        _ => Value::Null,
    };
    Ok(json!({"url": url, "job": job}))
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

/// The last four characters (by character, so a key from elsewhere cannot split one).
fn last4(key: &str) -> &str {
    key.char_indices().rev().nth(3).map_or(key, |(i, _)| &key[i..])
}

pub fn get_settings(core: &Core) -> ApiResult {
    // The tail comes from the key itself, so it cannot disagree with what is stored.
    let key = core.svc.keystore.get()?;
    Ok(json!({
        "key_set": key.is_some(),
        "key_masked": key.as_deref().map(|k| masked(last4(k))),
        "translation_model": MODEL,
        "favorite_voice_available": false,
    }))
}

pub fn get_google_account(core: &Core) -> ApiResult {
    let mut status = core.account.status().map_err(|_| err(503, "auth_unavailable", "계정 상태를 확인하지 못했습니다."))?;
    let connection = core.sync.get(status.generation);
    status.sync_ready = status.state == crate::account::AuthState::SignedIn
        && connection.as_ref().is_some_and(|snapshot| snapshot.state == crate::sync_runtime::ConnectionState::Connected);
    let mut result = serde_json::to_value(status).map_err(|_| err(500, "internal", "계정 상태를 불러오지 못했습니다."))?;
    if let Some(connection) = connection { result["connection"] = json!(connection); }
    Ok(result)
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
    log(format!("api key stored (…{})", last4(key)));
    Ok(json!({"key_set": true, "key_masked": masked(last4(key))}))
}

pub fn delete_api_key(core: &Core) -> ApiResult {
    core.svc.keystore.delete()?;
    log("api key deleted");
    Ok(json!({"key_set": false}))
}

// ---------------------------------------------------------------- P6-P8

/// Prefer YouTube's browser-session audio; otherwise use its native extractor.
pub fn resolve_source(core: &Core, page: &url::Url, media_url: Option<&str>) -> Result<media::Source, Refusal> {
    resolve_source_with(core, page, media_url, |raw| {
        let url = media::check_media_url(raw, core.cfg.allow_loopback_media)?;
        media::probe(&url, core.cfg.allow_loopback_media)
    })
}

fn resolve_source_with(core: &Core, page: &url::Url, media_url: Option<&str>, mut probe: impl FnMut(&str) -> Result<media::Source, Refusal>) -> Result<media::Source, Refusal> {
    let raw = if youtube::is_youtube(page) {
        if let Some(url) = media_url.and_then(youtube::browser_audio_url) {
            match probe(url) {
                Err(e) if e.code == "unreachable" => {},
                result => return result,
            }
        }
        core.svc.extractor.audio_url(page.as_str())?
    } else {
        media_url
            .filter(|m| !m.trim().is_empty())
            .ok_or_else(|| media::refuse("no_media", "이 페이지에서 영상 파일을 찾지 못했습니다."))?
            .trim()
            .to_string()
    };
    probe(&raw)
}

pub fn create_job(core: &Core, page_url: &str, media_url: Option<&str>, title: &str, duration_ms: i64) -> ApiResult {
    create_job_with_consent(core, page_url, media_url, title, duration_ms, None)
}

pub fn create_job_with_consent(core: &Core, page_url: &str, media_url: Option<&str>, title: &str, duration_ms: i64, deletion_token: Option<&str>) -> ApiResult {
    create_job_in(&core.library(None)?, page_url, media_url, title, duration_ms, deletion_token)
}

fn create_job_in(library: &crate::library_scope::Library<'_>, page_url: &str, media_url: Option<&str>, title: &str, duration_ms: i64, deletion_token: Option<&str>) -> ApiResult {
    let core = library.core;
    let page = http_url(page_url)?;
    if duration_ms <= 0 {
        return Err(err(400, "bad_duration", "영상 길이를 알 수 없습니다. 영상이 재생 준비된 뒤에 다시 눌러 주세요."));
    }
    let key = video_key(&page);
    // Check-then-create runs under one lock, so two presses can never make two
    // jobs or let a late one wipe the cues of the first (one running job per device).
    let _creating = core.create_lock.lock().unwrap_or_else(|p| p.into_inner());
    let existing = library.run(|store| {
    if let Some((token, complete)) = store.deleted(&key)? {
        if !complete || deletion_token != Some(token.as_str()) {
            return Err(deleted_error(token, complete));
        }
    }

    let active = store.active_job()?;
    if !active.as_ref().is_some_and(|j| j.video_key == key) {
        if let Some(last) = store.ready_job(&key)? {
            return Ok(Some(json!({"job_id": last.id, "video_key": key, "stage": "ready", "reused": true})));
        }
    }
    if let Some(active) = active {
        if active.video_key == key {
            return Ok(Some(json!({"job_id": active.id, "video_key": key, "stage": active.stage, "reused": false})));
        }
        return Err(err(409, "busy", "다른 영상의 작업이 돌고 있습니다. 그 작업이 끝난 뒤에 다시 눌러 주세요."));
    }
    Ok(None)
    })?;
    if let Some(existing) = existing { return Ok(existing); }
    if core.has_active_job()? {
        return Err(err(409, "busy", "다른 영상의 작업이 돌고 있습니다. 그 작업이 끝난 뒤에 다시 눌러 주세요."));
    }
    if core.svc.keystore.get()?.is_none() {
        return Err(err(428, "no_key", "설정에서 Gemini API 키를 먼저 넣어 주세요."));
    }
    if let Some(m) = media_url.filter(|m| !m.trim().is_empty()) {
        http_url(m).map_err(|_| err(400, "bad_url", "media_url은 http나 https 주소여야 합니다."))?;
    }
    let id = new_job_id();
    let title = clip_title(title, page_url);
    // Now, while the press keeps the app on screen: the media check below can take
    // tens of seconds, and by its end the user may have gone home (R6).
    if let Err(e) = core.svc.notifier.hold(&id, &title) {
        log(format!("background work refused: {e}"));
        return Err(err(409, "background_refused", "휴대폰이 백그라운드 작업을 막아 시작하지 않았습니다. 앱 화면에서 다시 눌러 주세요."));
    }
    let created = (|| -> Result<(), ApiError> {
        let source = resolve_source(core, &page, media_url)?;
        // The player's length is an ad's if one was playing when the button was
        // pressed; YouTube's stream carries the lecture's own (R18).
        let duration_ms = match &source {
            media::Source::File { url, .. } if youtube::is_youtube(&page) => youtube::stream_duration_ms(url).unwrap_or(duration_ms),
            _ => duration_ms,
        };
        library.run(|store| {
        store.create_job_with_consent(&id, &key, page_url.trim(), site(&page), &title, duration_ms, media_url.map(str::trim).filter(|m| !m.is_empty()), MODEL, deletion_token)
            .map_err(|detail| if detail == "sync deletion confirmation required" {
                store.deleted(&key).ok().flatten().map(|(token,complete)| deleted_error(token,complete))
                    .unwrap_or_else(|| err(409,"cleanup_pending","다른 기기에서 영상을 삭제했습니다. 보관함을 다시 불러와 주세요."))
            } else { detail.into() })?;
        Ok(())
        })
    })();
    if let Err(e) = created {
        core.svc.notifier.ongoing(&id, "hidden", &title, 0);
        return Err(e);
    }
    core.svc.notifier.ongoing(&id, "shown", &title, 0);
    log(format!("job {id} created for {key}"));
    core.wake_worker();
    Ok(json!({"job_id": id, "video_key": key, "stage": "downloading", "reused": false}))
}

pub fn retranslate_job(core: &Core, page_url: &str) -> ApiResult {
    retranslate_job_in(&core.library(None)?, page_url)
}

fn retranslate_job_in(library: &crate::library_scope::Library<'_>, page_url: &str) -> ApiResult {
    let core = library.core;
    let key = video_key(&http_url(page_url)?);
    let _creating = core.create_lock.lock().unwrap_or_else(|p| p.into_inner());
    let active = library.run(|store| {
        if let Some((token, complete)) = store.deleted(&key)? { return Err(deleted_error(token, complete)); }
        Ok(store.active_job()?)
    })?;
    if let Some(active) = active {
        if active.video_key == key {
            return Ok(json!({"job_id":active.id,"video_key":key,"stage":active.stage,"reused":false}));
        }
        return Err(err(409, "busy", "다른 영상의 작업이 끝난 뒤에 다시 눌러 주세요."));
    }
    if core.has_active_job()? {
        return Err(err(409, "busy", "다른 영상의 작업이 끝난 뒤에 다시 눌러 주세요."));
    }
    let (previous, video) = library.run(|store| {
        let ready = store.ready_job(&key)?.is_some();
        let mut previous = store.latest_job(&key)?.filter(|_| ready)
            .ok_or_else(|| err(409, "not_ready", "완료된 자막만 다시 번역할 수 있습니다."))?;
        previous.cost_krw = store.retranslation_cost(&key)?;
        let video = store.video(&key)?.ok_or_else(|| err(404, "no_video", "저장된 원문이 없습니다."))?;
        Ok((previous, video))
    })?;
    if core.svc.keystore.get()?.is_none() {
        return Err(err(428, "no_key", "설정에서 Gemini API 키를 먼저 넣어 주세요."));
    }
    if previous.cost_krw >= gemini::limit_krw(video.duration_ms) {
        return Err(err(409, "cost_limit", "이 영상의 번역 비용 한도에 도달했습니다."));
    }
    let id = new_job_id();
    core.svc.notifier.hold(&id, &video.title).map_err(|_| err(409, "background_refused", "앱 화면에서 다시 눌러 주세요."))?;
    if let Err(e) = library.run(|store| Ok(store.retranslate(&id, &previous, MODEL)?)) {
        core.svc.notifier.ongoing(&id, "hidden", &video.title, 0);
        return Err(e.into());
    }
    core.svc.notifier.ongoing(&id, "shown", &video.title, 0);
    core.wake_worker();
    Ok(json!({"job_id":id,"video_key":key,"stage":"translating","reused":false}))
}

pub fn get_job(core: &Core, page_url: &str) -> ApiResult {
    get_job_in(&core.library(None)?, page_url)
}

fn get_job_in(library: &crate::library_scope::Library<'_>, page_url: &str) -> ApiResult {
    let key = video_key(&http_url(page_url)?);
    library.run(|store| {
    if let Some((token, complete)) = store.deleted(&key)? { return Err(deleted_error(token, complete)); }
    let job = store.latest_job(&key)?.ok_or_else(|| err(404, "no_job", "이 영상의 작업이 없습니다."))?;
    let title = store.video(&key)?.map(|v| v.title).unwrap_or_default();
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
    })
}

pub fn get_subtitles(core: &Core, video_key: &str) -> ApiResult {
    get_subtitles_in(&core.library(None)?, video_key)
}

fn get_subtitles_in(library: &crate::library_scope::Library<'_>, video_key: &str) -> ApiResult {
    library.run(|store| {
    if let Some((token, complete)) = store.deleted(video_key)? { return Err(deleted_error(token, complete)); }
    let video = store.video(video_key)?.ok_or_else(|| err(404, "no_video", "이 영상의 자막이 없습니다."))?;
    let ready = store.ready_job(video_key)?.is_some();
    if !ready {
        return Err(err(409, "not_ready", "자막을 만드는 중입니다. 다 되면 알림으로 알려 드립니다."));
    }
    let cues: Vec<Value> = store
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
    })
}

fn playback_in(library: &crate::library_scope::Library<'_>, args: &Value) -> ApiResult {
    let page = http_url(args["page_url"].as_str().unwrap_or_default())?;
    let key = video_key(&page);
    let action: PlaybackAction = serde_json::from_value(args["playback_action"].clone())
        .map_err(|_| err(400, "bad_playback", "지원하지 않는 재생 위치 요청입니다."))?;
    library.run(|store| {
        if let Some((token, complete)) = store.deleted(&key)? { return Err(deleted_error(token, complete)); }
        let video = store.video(&key)?.ok_or_else(|| err(404, "no_video", "보관함에 저장된 영상의 재생 위치만 저장합니다."))?;
        if matches!(action, PlaybackAction::Set) {
            let position = args["position_ms"].as_i64().ok_or_else(|| err(400, "bad_position", "재생 위치를 확인하지 못했습니다."))?;
            let duration = args["duration_ms"].as_i64().ok_or_else(|| err(400, "bad_position", "영상 길이를 확인하지 못했습니다."))?;
            store.save_playback(&key, position, duration).map_err(|detail| match detail.as_str() {
                "invalid playback position" => err(400, "bad_position", "재생 위치가 영상 범위를 벗어났습니다."),
                "playback player mismatch" => err(409, "player_mismatch", "저장된 본편과 다른 플레이어의 위치는 저장하지 않았습니다."),
                "playback unavailable" => err(409, "playback_unavailable", "이 영상의 저장 상태가 바뀌었습니다. 보관함을 다시 불러와 주세요."),
                _ => detail.into(),
            })?;
        }
        Ok(json!({"video_key":key,"position_ms":store.playback(&key)?,"duration_ms":video.duration_ms}))
    })
}

/// Native screen and overlay requests bind to the generation displayed when the action began.
pub fn library_request(core: &Core, generation: Option<u64>, command: &str, args: &Value) -> ApiResult {
    if generation.is_none() && core.account.status().map_err(|_| err(503, "account_unavailable", "계정 상태를 불러오지 못했습니다."))?.state != crate::account::AuthState::Unsupported {
        let mut error = err(409, "account_required", "계정 상태를 확인한 뒤 화면을 다시 불러와 주세요.");
        if command == "get_job" { error.account_generation = Some(core.account.status().map_err(|_| err(503,"account_unavailable","계정 상태를 불러오지 못했습니다."))?.generation); }
        return Err(error);
    }
    let library = core.library(generation)?;
    let s = |key: &str| args[key].as_str().unwrap_or_default();
    let mut result = match command {
        "browser_open" => browser_open_in(&library, s("url")),
        "get_browser" => get_browser_in(&library),
        "get_library" => library::get_library_in(&library),
        "library_action" if args["action"]["action"] == "import_legacy" => {
            if args["action"]["confirmed"] != true {
                return Err(err(400, "confirmation_required", "이 기기의 기존 보관함을 현재 Google 계정에 복사할지 확인해 주세요."));
            }
            let _writing = core.create_lock.lock().unwrap_or_else(|p| p.into_inner());
            Ok(json!({"changed": true, "imported": library.import_legacy()?}))
        },
        "library_action" => library::library_action_in(&library, &args["action"]),
        "create_job" if args["retranslate"] == true => retranslate_job_in(&library, s("page_url")),
        "create_job" => create_job_in(&library, s("page_url"), args["media_url"].as_str(), s("title"), args["duration_ms"].as_i64().unwrap_or(0), args["deletion_token"].as_str()),
        "get_job" if !args["playback_action"].is_null() => playback_in(&library, args),
        "get_job" => get_job_in(&library, s("page_url")),
        "get_subtitles" => get_subtitles_in(&library, s("video_key")),
        _ => Err(err(404, "no_route", "지원하지 않는 보관함 요청입니다.")),
    }?;
    library.run(|_| Ok(()))?;
    result["account_generation"] = library.generation().into();
    Ok(result)
}

fn deleted_error(token: String, complete: bool) -> ApiError {
    let mut e = err(410, "video_deleted", if complete { "저장된 영상을 삭제했습니다. 다시 번역하면 API 비용이 발생합니다." } else { "삭제를 마치지 못했습니다. 보관함에서 삭제를 다시 시도해 주세요." });
    if complete { e.deletion_token = Some(token); }
    e
}

#[path = "library_api.rs"]
mod library;
pub use library::{get_library, library_action};

#[cfg(test)]
#[path = "api_key_tests.rs"]
mod key_tests;

#[cfg(test)]
#[path = "api_duration_tests.rs"]
pub(crate) mod duration_tests;

#[cfg(test)]
#[path = "api_hold_tests.rs"]
mod hold_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn privacy_opener_uses_only_the_fixed_public_page_and_preserves_account_and_video_state() {
        use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
        struct Opener(Arc<Mutex<Vec<String>>>, Arc<AtomicBool>);
        impl crate::Browser for Opener {
            fn open_download(&self, url: &str) -> Result<(), String> {
                self.0.lock().unwrap().push(url.into());
                if self.1.load(Ordering::Acquire) { Err("private platform diagnostic".into()) } else { Ok(()) }
            }
            fn navigate(&self, _: &str, _: bool) -> Result<(), String> { panic!("privacy must not navigate a video") }
            fn clear_site_data(&self) -> Result<(), String> { panic!("privacy must not clear cookies") }
        }
        let folder = std::env::temp_dir().join(format!("vto-privacy-{}-{}", std::process::id(), new_job_id()));
        std::fs::create_dir_all(&folder).unwrap();
        let mut core = crate::pipeline::worker_tests::idle_core(&folder, "");
        let opened = Arc::new(Mutex::new(Vec::new()));
        let fail = Arc::new(AtomicBool::new(false));
        core.svc.browser = Box::new(Opener(opened.clone(), fail.clone()));
        let generation = core.account.status().unwrap().generation;
        assert_eq!(open_privacy_policy(&core).unwrap(), json!({"opened":true,"url":"https://littlebitai.github.io/video-translate-overlay-2/privacy.html"}));
        fail.store(true, Ordering::Release);
        let error = open_privacy_policy(&core).unwrap_err();
        assert_eq!((error.status, error.error.as_str()), (503, "privacy_unavailable"));
        assert!(!error.message.contains("private platform"));
        assert_eq!(*opened.lock().unwrap(), vec![crate::PRIVACY_POLICY_URL; 2]);
        assert_eq!(core.account.status().unwrap().generation, generation);
        assert!(core.store.opened_url().unwrap().is_none());
        drop(core);
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn youtube_sign_in_return_addresses_are_limited_to_youtube() {
        let target = |s: &str| youtube_sign_in_target(&url::Url::parse(s).unwrap()).map(|u| u.to_string());
        assert_eq!(
            target("https://accounts.google.com/ServiceLogin?continue=http%3A%2F%2Fyoutu.be%2FabcDEF12345"),
            Some("https://youtu.be/abcDEF12345".into())
        );
        for bad in [
            "https://accounts.google.com/ServiceLogin?continue=https%3A%2F%2Fevil.example%2F",
            "https://accounts.google.com.evil.example/ServiceLogin?continue=https%3A%2F%2Fyoutube.com%2F",
            "http://accounts.google.com/ServiceLogin?continue=https%3A%2F%2Fyoutube.com%2F",
            "https://accounts.google.com/ServiceLogin?continue=javascript%3Aalert(1)",
        ] {
            assert_eq!(target(bad), None, "{bad}");
        }
    }

    #[test]
    fn video_keys_follow_the_video_not_the_url_form() {
        let k = |s: &str| video_key(&url::Url::parse(s).unwrap());
        assert_eq!(k("https://www.youtube.com/watch?v=abcDEF12345&t=30"), "abcDEF12345");
        assert_eq!(k("https://youtu.be/abcDEF12345?si=xyz"), "abcDEF12345");
        assert_eq!(k("https://m.youtube.com/shorts/abcDEF12345"), "abcDEF12345");
        let plain = k("https://a.example/talk");
        assert_eq!(plain.len(), 64, "SHA-256 hex");
        assert_eq!(k("https://a.example/talk#x"), plain);
        assert_eq!(k("https://a.example/talk?utm_source=share&fbclid=1"), plain);
        assert_eq!(k("https://a.example/talk?id=7&utm_medium=x"), k("https://a.example/talk?id=7"));
        assert_ne!(k("https://a.example/talk?id=7"), plain);
        assert_ne!(plain, k("https://a.example/talk2"));
    }

    #[test]
    fn ids_and_titles_follow_the_entity_rules() {
        let id = new_job_id();
        assert_eq!((id.len(), &id[14..15]), (36, "4"), "{id}");
        assert_ne!(id, new_job_id());
        assert_eq!(clip_title(&"가".repeat(201), "x").chars().count(), 200);
        assert_eq!(clip_title("  ", "https://a.example/t"), "https://a.example/t");
    }
}
