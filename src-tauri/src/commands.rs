//! The Tauri side: commands P1-P8 and the capabilities that decide who may call them.
//!
//! The app's own screen (local origin) may call every command. Site pages in
//! the in-app browser, where the overlay runs, may call only P6-P8, so a site
//! can never read or replace the key (A1).

use crate::api::{self, ApiError, ApiResult};
use crate::Core;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::Arc;
use tauri::ipc::RuntimeAuthority;
use tauri::utils::acl::{capability::Capability, manifest::Manifest, resolved::Resolved, APP_ACL_KEY};
use tauri::{Runtime, State};

/// Every registered command, P1 to P8.
pub const COMMANDS: [&str; 8] =
    ["browser_open", "clear_site_data", "get_settings", "put_api_key", "delete_api_key", "create_job", "get_job", "get_subtitles"];

/// The commands a site page (the overlay) may call.
pub const OVERLAY_COMMANDS: [&str; 3] = ["create_job", "get_job", "get_subtitles"];

pub const MAIN_CAPABILITY: &str = include_str!("../capabilities/main.json");
pub const OVERLAY_CAPABILITY: &str = include_str!("../capabilities/overlay.json");
/// Android shows site pages in the one `main` window; resolved only on Android.
pub const OVERLAY_ANDROID_CAPABILITY: &str = include_str!("../capabilities/overlay-android.json");

/// The app's ACL: one `allow-<command>` permission per command, and the two capability files.
pub fn authority() -> RuntimeAuthority {
    let acl = authority_acl();
    let caps: BTreeMap<String, Capability> = [MAIN_CAPABILITY, OVERLAY_CAPABILITY, OVERLAY_ANDROID_CAPABILITY]
        .iter()
        .map(|s| serde_json::from_str::<Capability>(s).expect("capability file"))
        .map(|c| (c.identifier.clone(), c))
        .collect();
    let resolved = Resolved::resolve(&acl, caps, tauri::utils::platform::Target::current()).expect("resolve capabilities");
    RuntimeAuthority::new(acl, resolved)
}

/// One `allow-<command>` permission per command, as the app's ACL manifest.
fn authority_acl() -> BTreeMap<String, Manifest> {
    let permissions: serde_json::Map<String, Value> = COMMANDS
        .iter()
        .map(|c| {
            let id = format!("allow-{}", c.replace('_', "-"));
            (id.clone(), json!({"identifier": id, "description": format!("Allows {c}"), "commands": {"allow": [c], "deny": []}}))
        })
        .collect();
    let manifest: Manifest = serde_json::from_value(json!({
        "default_permission": null,
        "permissions": permissions,
        "permission_sets": {},
        "global_scope_schema": null
    }))
    .expect("app ACL manifest");
    BTreeMap::from([(APP_ACL_KEY.to_string(), manifest)])
}

async fn blocking(f: impl FnOnce() -> ApiResult + Send + 'static) -> ApiResult {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| ApiError::from(e.to_string()))?
}

type Shared<'a> = State<'a, Arc<Core>>;

#[tauri::command(async, rename_all = "snake_case")]
pub async fn browser_open(core: Shared<'_>, url: String) -> ApiResult {
    let core = core.inner().clone();
    blocking(move || api::browser_open(&core, &url)).await
}

#[tauri::command(async, rename_all = "snake_case")]
pub async fn clear_site_data(core: Shared<'_>) -> ApiResult {
    let core = core.inner().clone();
    blocking(move || api::clear_site_data(&core)).await
}

#[tauri::command(async, rename_all = "snake_case")]
pub async fn get_settings(core: Shared<'_>) -> ApiResult {
    let core = core.inner().clone();
    blocking(move || api::get_settings(&core)).await
}

#[tauri::command(async, rename_all = "snake_case")]
pub async fn put_api_key(core: Shared<'_>, api_key: String) -> ApiResult {
    let core = core.inner().clone();
    blocking(move || api::put_api_key(&core, &api_key)).await
}

#[tauri::command(async, rename_all = "snake_case")]
pub async fn delete_api_key(core: Shared<'_>) -> ApiResult {
    let core = core.inner().clone();
    blocking(move || api::delete_api_key(&core)).await
}

#[tauri::command(async, rename_all = "snake_case")]
pub async fn create_job(core: Shared<'_>, page_url: String, media_url: Option<String>, title: Option<String>, duration_ms: Option<i64>) -> ApiResult {
    let core = core.inner().clone();
    blocking(move || api::create_job(&core, &page_url, media_url.as_deref(), title.as_deref().unwrap_or(""), duration_ms.unwrap_or(0))).await
}

#[tauri::command(async, rename_all = "snake_case")]
pub async fn get_job(core: Shared<'_>, page_url: String) -> ApiResult {
    let core = core.inner().clone();
    blocking(move || api::get_job(&core, &page_url)).await
}

#[tauri::command(async, rename_all = "snake_case")]
pub async fn get_subtitles(core: Shared<'_>, video_key: String) -> ApiResult {
    let core = core.inner().clone();
    blocking(move || api::get_subtitles(&core, &video_key)).await
}

/// Registers the commands and their state on a builder.
pub fn register<R: Runtime>(builder: tauri::Builder<R>, core: Arc<Core>) -> tauri::Builder<R> {
    handlers(builder.manage(core))
}

/// Only the handlers; the app manages its `Arc<Core>` in `setup`, once its data folder is known.
pub fn handlers<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.invoke_handler(tauri::generate_handler![
        browser_open,
        clear_site_data,
        get_settings,
        put_api_key,
        delete_api_key,
        create_job,
        get_job,
        get_subtitles
    ])
}

/// Maps the design's `METHOD /path` notation to a command and its arguments.
pub fn route(method: &str, path: &str, body: &Value) -> Option<(&'static str, Value)> {
    let mut args = if body.is_object() { body.clone() } else { json!({}) };
    let cmd = match (method, path) {
        ("POST", "/browser/open") => "browser_open",
        ("DELETE", "/browser/site-data") => "clear_site_data",
        ("GET", "/settings") => "get_settings",
        ("PUT", "/settings/api-key") => "put_api_key",
        ("DELETE", "/settings/api-key") => "delete_api_key",
        ("POST", "/jobs") => "create_job",
        ("GET", "/jobs") => "get_job",
        ("GET", p) => {
            let key = p.strip_prefix("/videos/")?.strip_suffix("/subtitles")?;
            if key.is_empty() || key.contains('/') {
                return None;
            }
            args["video_key"] = json!(key);
            "get_subtitles"
        }
        _ => return None,
    };
    Some((cmd, args))
}

#[cfg(test)]
#[path = "commands_android_tests.rs"]
mod android_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_capability_grants_only_p6_to_p8() {
        let cap: Capability = serde_json::from_str(OVERLAY_CAPABILITY).unwrap();
        let mut granted: Vec<String> = cap.permissions.iter().map(|p| p.identifier().get().replace("allow-", "").replace('-', "_")).collect();
        granted.sort();
        let mut want: Vec<String> = OVERLAY_COMMANDS.iter().map(|s| s.to_string()).collect();
        want.sort();
        assert_eq!(granted, want);
        assert!(!cap.local, "the overlay capability must not apply to the app's own screen");
        let _ = authority(); // both files resolve
    }

    #[test]
    fn routes_cover_every_command() {
        let mut seen: Vec<&str> = [
            ("POST", "/browser/open"),
            ("DELETE", "/browser/site-data"),
            ("GET", "/settings"),
            ("PUT", "/settings/api-key"),
            ("DELETE", "/settings/api-key"),
            ("POST", "/jobs"),
            ("GET", "/jobs"),
            ("GET", "/videos/yt-abc/subtitles"),
        ]
        .iter()
        .map(|(m, p)| route(m, p, &Value::Null).unwrap().0)
        .collect();
        seen.sort();
        let mut all = COMMANDS.to_vec();
        all.sort();
        assert_eq!(seen, all);
        assert!(route("GET", "/videos/a/b/subtitles", &Value::Null).is_none());
    }
}
