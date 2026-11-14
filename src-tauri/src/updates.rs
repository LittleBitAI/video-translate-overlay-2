//! Approved GitHub releases. Checks never share the video worker or its database.
use crate::api::{err, ApiResult};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

pub const REPOSITORY: &str = "LittleBitAI/video-translate-overlay-2";
pub const SOURCE: &str = "https://api.github.com/repos/LittleBitAI/video-translate-overlay-2/releases?per_page=100";
const APPROVED: &str = "<!-- vto-approved:v1 -->";
const MAX_METADATA: usize = 512 * 1024;
const COOLDOWN: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Update {
    pub version: String,
    pub summary: String,
    pub download_url: String,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    body: Option<String>,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
    state: String,
}

// Only stable three-part versions are distributed. No prerelease/build aliases.
fn version(raw: &str) -> Option<[u32; 3]> {
    let mut parts = raw.split('.');
    let mut result = [0; 3];
    for n in &mut result {
        let p = parts.next()?;
        if p.is_empty() || (p.len() > 1 && p.starts_with('0')) || !p.bytes().all(|b| b.is_ascii_digit()) { return None; }
        *n = p.parse().ok()?;
    }
    parts.next().is_none().then_some(result)
}

pub fn platform() -> &'static str {
    if cfg!(target_os = "android") { "android-arm64" } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) { "windows-x64" } else { "unsupported" }
}

pub fn asset_name(v: &str, platform: &str) -> Option<String> {
    version(v)?;
    let suffix = match platform { "android-arm64" => "arm64.apk", "windows-x64" => "windows-x64-setup.exe", _ => return None };
    Some(format!("video-translate-overlay-{v}-{suffix}"))
}

pub(crate) fn dismissed(data: &Path, v: &str) -> bool {
    let saved = std::fs::read_to_string(data.join("dismissed-update.txt")).unwrap_or_default();
    version(saved.trim()).is_some_and(|d| version(v).is_some_and(|v| v <= d))
}

fn summary(body: &str) -> Option<String> {
    if !body.lines().any(|s| s == APPROVED) { return None; }
    let mut lines = body.lines().skip_while(|s| *s != "## Features");
    lines.next()?;
    let text = lines.take_while(|s| !s.starts_with("## ")).collect::<Vec<_>>().join("\n").trim().to_string();
    (!text.is_empty() && text.chars().count() <= 600 && !text.chars().any(|c| c.is_control() && c != '\n')).then_some(text)
}

pub fn select(bytes: &[u8], installed: &str, platform: &str) -> Result<Option<Update>, String> {
    if bytes.len() > MAX_METADATA { return Err("release metadata too large".into()); }
    let current = version(installed).ok_or("invalid installed version")?;
    let releases: Vec<Release> = serde_json::from_slice(bytes).map_err(|_| "invalid release metadata")?;
    if releases.len() > 100 { return Err("too many releases".into()); }
    let mut best: Option<([u32; 3], Update)> = None;
    for release in releases {
        let Some(v) = release.tag_name.strip_prefix('v') else { continue };
        let Some(order) = version(v) else { continue };
        if release.draft || release.prerelease || order <= current || best.as_ref().is_some_and(|(b, _)| order <= *b) { continue; }
        let Some(summary) = release.body.as_deref().and_then(summary) else { continue };
        let Some(name) = asset_name(v, platform) else { continue };
        let expected = format!("https://github.com/{REPOSITORY}/releases/download/v{v}/{name}");
        if !release.assets.iter().any(|a| a.name == name && a.browser_download_url == expected && a.size > 0 && a.state == "uploaded") { continue; }
        best = Some((order, Update { version: v.into(), summary, download_url: expected }));
    }
    Ok(best.map(|(_, update)| update))
}

fn fetch(data: &Path) -> Result<Option<Update>, String> {
    // Test overrides are private debug-build files, never environment-controlled in a release.
    #[cfg(debug_assertions)]
    let installed = std::fs::read_to_string(data.join("update-installed.txt")).unwrap_or_else(|_| env!("CARGO_PKG_VERSION").into());
    #[cfg(not(debug_assertions))]
    let installed = env!("CARGO_PKG_VERSION");
    #[cfg(debug_assertions)]
    if let Ok(bytes) = std::fs::read(data.join("update-fixture.json")) {
        return select(&bytes, installed.trim(), platform());
    }
    let _ = data;
    let mut request = crate::http::Request::get(SOURCE).header("Accept", "application/vnd.github+json").header("User-Agent", "video-translate-overlay");
    request.timeout_ms = 5_000;
    request.follow_redirects = false;
    let response = crate::http::send(request).map_err(|e| e.to_string())?;
    if response.status != 200 { return Err(format!("release server returned {}", response.status)); }
    let bytes = response.bytes(MAX_METADATA + 1).map_err(|e| e.to_string())?;
    select(&bytes, installed.trim(), platform())
}

#[derive(Default)]
struct State {
    busy: bool,
    last: Option<Instant>,
    result: Option<Result<Option<Update>, String>>,
}

#[derive(Default)]
pub struct Checker { shared: Arc<(Mutex<State>, Condvar)> }

pub fn open(core: &crate::Core, version: &str) -> ApiResult {
    let url = core.updates.download(&core.cfg.data_dir, version)?;
    core.svc.browser.open_download(&url).map_err(|_| err(503, "download_open_failed", "내려받기 링크를 열지 못했습니다. 다시 시도해 주세요."))?;
    Ok(json!({"opened":true}))
}

impl Checker {
    pub fn check(&self, data: &Path, manual: bool) -> ApiResult {
        let directory = data.to_path_buf();
        self.check_with(data, manual, move || fetch(&directory))
    }

    fn check_with(&self, data: &Path, manual: bool, fetch: impl FnOnce() -> Result<Option<Update>, String> + Send + 'static) -> ApiResult {
        let (lock, ready) = &*self.shared;
        let mut state = lock.lock().unwrap();
        if !state.busy && (manual || state.last.is_none_or(|t| t.elapsed() >= COOLDOWN)) {
            state.busy = true;
            state.last = Some(Instant::now());
            let shared = self.shared.clone();
            if std::thread::Builder::new().name("release-check".into()).spawn(move || {
                let result = fetch();
                let (lock, ready) = &*shared;
                let mut state = lock.lock().unwrap();
                state.result = Some(result);
                state.busy = false;
                ready.notify_all();
            }).is_err() {
                state.busy = false;
                state.result = Some(Err("could not start release check".into()));
            }
        }
        if state.busy {
            let (next, timeout) = ready.wait_timeout_while(state, Duration::from_secs(15), |s| s.busy).unwrap();
            state = next;
            if timeout.timed_out() && state.busy { return Err(err(503, "update_check_failed", "업데이트를 확인하지 못했습니다. 잠시 뒤 다시 시도해 주세요.")); }
        }
        match state.result.as_ref() {
            Some(Ok(update)) => {
                let visible = update.as_ref().filter(|u| manual || !dismissed(data, &u.version));
                Ok(json!({"installed_version":env!("CARGO_PKG_VERSION"), "platform":platform(), "update":visible, "dismissed":update.is_some() && visible.is_none()}))
            }
            _ => Err(err(503, "update_check_failed", "업데이트를 확인하지 못했습니다. 인터넷 연결을 확인하고 다시 시도해 주세요.")),
        }
    }

    pub fn dismiss(&self, data: &Path, v: &str) -> ApiResult {
        let requested = version(v).ok_or_else(|| err(400, "bad_version", "잘못된 버전입니다."))?;
        let _state = self.shared.0.lock().unwrap();
        let previous = std::fs::read_to_string(data.join("dismissed-update.txt")).unwrap_or_default();
        if version(previous.trim()).is_none_or(|p| requested > p) {
            let temporary = data.join("dismissed-update.tmp");
            std::fs::write(&temporary, v).map_err(|e| e.to_string())?;
            std::fs::rename(temporary, data.join("dismissed-update.txt")).map_err(|e| e.to_string())?;
        }
        Ok(json!({"dismissed":v}))
    }

    pub fn download(&self, data: &Path, v: &str) -> Result<String, crate::api::ApiError> {
        let result = self.check(data, true)?;
        result["update"].as_object().filter(|u| u["version"] == v).and_then(|u| u["download_url"].as_str()).map(str::to_string)
            .ok_or_else(|| err(409, "update_unavailable", "이 업데이트를 지금 내려받을 수 없습니다. 다시 확인해 주세요."))
    }
}

#[cfg(test)]
#[path = "updates_tests.rs"]
mod tests;
