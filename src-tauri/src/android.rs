//! The Android services: the Gemini key sealed by the Android Keystore (A1),
//! the running job's foreground service with its ongoing notification (A5),
//! and the ready notice whose tap opens the video (F4). The Kotlin half is
//! `gen/android/app/src/main/java/dev/vto/overlay/VtoPlugin.kt`.
//!
//! These are reached only from Rust: the app's ACL ([`crate::commands::authority`])
//! grants no `plugin:vto|…` command to any page, the app's own included.

use serde_json::{json, Value};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::plugin::{PluginHandle, TauriPlugin};
use tauri::{Manager, Wry};

#[derive(Clone)]
pub struct Mobile(PluginHandle<Wry>);

pub fn plugin() -> TauriPlugin<Wry> {
    tauri::plugin::Builder::new("vto")
        .setup(|app, api| {
            app.manage(Mobile(api.register_android_plugin("dev.vto.overlay", "VtoPlugin")?));
            Ok(())
        })
        .build()
}

impl Mobile {
    fn call(&self, method: &str, args: Value) -> Result<Value, String> {
        self.0.run_mobile_plugin::<Value>(method, args).map_err(|e| format!("{method}: {e}"))
    }

    pub fn job(&self, state: &str, title: &str, percent: i64) {
        if let Err(e) = self.call("job", json!({ "state": state, "title": title, "percent": percent })) {
            crate::log(e);
        }
    }

    /// Starts the job's foreground service, or says why Android would not.
    pub fn start(&self, title: &str) -> Result<(), String> {
        self.call("job", json!({ "state": "shown", "title": title, "percent": 0 })).map(drop)
    }

    pub fn ready(&self, job_id: &str, text: &str) -> Result<(), String> {
        self.call("ready", json!({ "jobId": job_id, "text": text })).map(drop)
    }

    pub fn clear_site_data(&self) -> Result<(), String> {
        self.call("clearSiteData", json!({})).map(drop)
    }

    /// `open(job id)` for every tapped ready notice, the one that started the app included.
    pub fn watch_open(&self, open: impl Fn(&str) + Send + Sync + 'static) -> Result<(), String> {
        let channel: Channel<Value> = Channel::new(move |body| {
            if let InvokeResponseBody::Json(s) = body {
                if let Some(id) = serde_json::from_str::<Value>(&s).ok().and_then(|v| v["jobId"].as_str().map(str::to_string)) {
                    open(&id);
                }
            }
            Ok(())
        });
        self.call("watchOpen", json!({ "channel": channel })).map(drop)
    }
}

impl crate::KeyStore for Mobile {
    fn get(&self) -> Result<Option<String>, String> {
        Ok(self.call("getKey", json!({}))?.get("key").and_then(Value::as_str).map(str::to_string))
    }

    fn set(&self, key: &str) -> Result<(), String> {
        self.call("setKey", json!({ "key": key })).map(drop)
    }

    fn delete(&self) -> Result<(), String> {
        self.call("deleteKey", json!({})).map(drop)
    }
}
