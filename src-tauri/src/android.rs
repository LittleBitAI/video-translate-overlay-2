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
    pub fn register_sync(&self, owner: &str) -> Result<(), String> {
        self.call("registerSync", json!({"owner":owner})).map(drop)
    }
    pub fn sync_background(&self, owner: &str, generation: u64, enabled: bool) -> Result<bool, String> {
        Ok(self.call("syncBackground", json!({"owner":owner,"generation":generation,"enabled":enabled}))?["active"].as_bool().unwrap_or(false))
    }
    pub fn update_notice(&self, update: &crate::updates::Update) -> Result<bool, String> {
        Ok(self.call("updateNotice", serde_json::to_value(update).map_err(|e| e.to_string())?)?["download"].as_bool().unwrap_or(false))
    }
    pub fn open_download(&self, url: &str) -> Result<(), String> {
        self.call("openDownload", json!({"url":url})).map(drop)
    }
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

impl crate::account::NativeAccount for Mobile {
    fn platform(&self) -> crate::google_auth::Platform {
        crate::google_auth::Platform::Android
    }
    fn read(&self) -> Result<Option<String>, crate::google_auth::AuthError> {
        self.call("getGoogleSession", json!({}))
            .map(|v| v["value"].as_str().map(str::to_string))
            .map_err(|_| crate::google_auth::AuthError::Unavailable)
    }
    fn write(&self, value: &str) -> Result<(), crate::google_auth::AuthError> {
        self.call("setGoogleSession", json!({"value":value}))
            .map(drop)
            .map_err(|_| crate::google_auth::AuthError::Unavailable)
    }
    fn delete(&self) -> Result<(), crate::google_auth::AuthError> {
        self.call("deleteGoogleSession", json!({}))
            .map(drop)
            .map_err(|_| crate::google_auth::AuthError::Unavailable)
    }
    fn cancel_sign_in(&self) {
        let _ = self.call("googleCancel", json!({}));
    }
    fn sign_in(
        &self,
        nonce: &str,
        cancelled: &std::sync::atomic::AtomicBool,
        verifier: &crate::google_auth::Verifier,
    ) -> Result<crate::google_desktop::Credentials, crate::google_auth::AuthError> {
        use crate::google_auth::{AuthError, Platform, ANDROID_AUDIENCE};
        use std::{
            sync::{atomic::Ordering, mpsc},
            time::{Duration, Instant},
        };
        let (send, receive) = mpsc::channel();
        let channel: Channel<Value> = Channel::new(move |body| {
            if let InvokeResponseBody::Json(raw) = body {
                if let Ok(value) = serde_json::from_str::<Value>(&raw) {
                    let _ = send.send(value);
                }
            }
            Ok(())
        });
        self.call(
            "googleSignIn",
            json!({"nonce":nonce,"webClientId":ANDROID_AUDIENCE,"channel":channel}),
        )
        .map_err(|_| AuthError::Unavailable)?;
        let started = Instant::now();
        loop {
            if cancelled.load(Ordering::Acquire) {
                return Err(AuthError::Cancelled);
            }
            if started.elapsed() > Duration::from_secs(180) {
                self.cancel_sign_in();
                return Err(AuthError::TimedOut);
            }
            match receive.recv_timeout(Duration::from_millis(100)) {
                Ok(value) => {
                    if value["error"] == "cancelled" {
                        return Err(AuthError::Cancelled);
                    }
                    let token = value["idToken"].as_str().ok_or(AuthError::Unavailable)?;
                    let identity = verifier.verify(token, nonce, Platform::Android)?;
                    return Ok(crate::google_desktop::Credentials {
                        identity,
                        id_token: token.into(),
                        refresh_token: None,
                    });
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => return Err(AuthError::Unavailable),
            }
        }
    }
}
