//! The app: the settings screen ("main"), the in-app browser ("site") with
//! the overlay injected into every page, OS notifications, and whisper.cpp.
//! Built with `--features app`; the contract host is the test-only twin.

use std::sync::{Arc, OnceLock};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent, Wry};
use tauri_plugin_notification::NotificationExt;
use vto::{commands, platform, whisper::Whisper, youtube, Browser, Config, Core, Notifier, Services};

/// src/overlay/inject.ts bundled by build.rs.
const OVERLAY_JS: &str = include_str!(concat!(env!("OUT_DIR"), "/overlay/overlay.js"));

type Handle = Arc<OnceLock<AppHandle<Wry>>>;

fn app(h: &Handle) -> Result<&AppHandle<Wry>, String> {
    h.get().ok_or_else(|| "the app is still starting".to_string())
}

/// The "site" window. Its overlay script runs on every page it loads and
/// stays off Google's sign-in pages itself, so `overlay` needs no switch here.
struct TauriBrowser(Handle);

impl Browser for TauriBrowser {
    fn navigate(&self, url: &str, _overlay: bool) -> Result<(), String> {
        let app = app(&self.0)?;
        let url: tauri::Url = url.parse().map_err(|e| format!("{e}"))?;
        if let Some(w) = app.get_webview_window("site") {
            w.navigate(url).map_err(|e| e.to_string())?;
            let _ = w.show();
            let _ = w.set_focus();
            return Ok(());
        }
        let b = WebviewWindowBuilder::new(app, "site", WebviewUrl::External(url)).initialization_script(OVERLAY_JS);
        #[cfg(desktop)]
        let b = b.title("영상").inner_size(1280.0, 800.0);
        b.build().map(|_| ()).map_err(|e| e.to_string())
    }

    fn clear_site_data(&self) -> Result<(), String> {
        let app = app(&self.0)?;
        // Every window shares the webview profile, so clearing through one clears all.
        let w = app.get_webview_window("site").or_else(|| app.get_webview_window("main")).ok_or("no webview")?;
        w.clear_all_browsing_data().map_err(|e| e.to_string())
    }
}

struct TauriNotifier(Handle);

impl TauriNotifier {
    fn show(&self, title: &str, body: &str) {
        if let Ok(app) = app(&self.0) {
            if let Err(e) = app.notification().builder().title(title).body(body).show() {
                vto::log(format!("notification: {e}"));
            }
        }
    }
}

impl Notifier for TauriNotifier {
    fn ongoing(&self, _job_id: &str, state: &str, title: &str, percent: i64) {
        // ponytail: desktop toasts cannot be updated in place, so progress goes to the
        // window title and one toast says the job runs on; Android needs an ongoing notification.
        if let Ok(app) = app(&self.0) {
            if let Some(w) = app.get_webview_window("main") {
                let t = if state == "hidden" { "영상 번역 오버레이".to_string() } else { format!("자막 만드는 중 {percent}% · {title}") };
                let _ = w.set_title(&t);
            }
        }
        if state == "shown" {
            self.show("자막을 만들고 있습니다", &format!("{title} — 창을 닫아도 계속됩니다."));
        }
    }

    fn ready(&self, _job_id: &str, text: &str, url: &str) {
        // The settings screen lists it with an open button, which outlives the toast.
        if let Ok(app) = app(&self.0) {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.eval(format!("window.vtoReady && window.vtoReady({})", serde_json::json!({ "text": text, "url": url })));
            }
        }
        #[cfg(windows)]
        match ready_toast(self.0.clone(), text, url) {
            Ok(()) => return,
            Err(e) => vto::log(format!("ready toast: {e}")),
        }
        self.show("자막 준비 완료", text);
    }
}

/// Opens the finished video in the site window (F4).
fn open_video(h: &Handle, url: &str) {
    if let Err(e) = TauriBrowser(h.clone()).navigate(url, true) {
        vto::log(format!("open {url}: {e}"));
    }
}

/// A Windows toast whose click opens the video. The toast is kept alive on its
/// own thread for an hour: its click event only reaches a live object.
#[cfg(windows)]
fn ready_toast(h: Handle, text: &str, url: &str) -> Result<(), String> {
    use tauri_winrt_notification::Toast;
    let app = app(&h)?;
    // An unpackaged build has no registered app id of its own; the plugin uses PowerShell's then too.
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dev = exe.parent().is_some_and(|d| d.ends_with("target/debug") || d.ends_with("target/release"));
    let app_id = if dev { Toast::POWERSHELL_APP_ID.to_string() } else { app.config().identifier.clone() };
    let (text, url) = (text.to_string(), url.to_string());
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let click = h.clone();
        let target = url.clone();
        let toast = Toast::new(&app_id).title("자막 준비 완료").text1(&text).on_activated(move |_| {
            open_video(&click, &target);
            Ok(())
        });
        let shown = toast.show().map_err(|e| e.to_string());
        let ok = shown.is_ok();
        let _ = tx.send(shown);
        if ok {
            std::thread::sleep(std::time::Duration::from_secs(3600));
        }
        drop(toast);
    });
    rx.recv().map_err(|e| e.to_string())?
}

fn main() {
    let handle: Handle = Arc::new(OnceLock::new());
    let setup_handle = handle.clone();
    let mut ctx = tauri::generate_context!();
    *ctx.runtime_authority_mut() = commands::authority();
    commands::handlers(tauri::Builder::default())
        .plugin(tauri_plugin_notification::init())
        .setup(move |app| {
            let _ = setup_handle.set(app.handle().clone());
            let data_dir = app.path().app_local_data_dir()?;
            let whisper = Whisper::new(&data_dir, std::env::var_os("VTO_WHISPER_MODEL").map(Into::into));
            let core = Core::start(
                Config { data_dir, gemini_base_url: vto::GEMINI_BASE_URL.into(), allow_loopback_media: false },
                Services {
                    keystore: Box::new(platform::CredentialStore),
                    browser: Box::new(TauriBrowser(setup_handle.clone())),
                    notifier: Box::new(TauriNotifier(setup_handle.clone())),
                    transcriber: Box::new(whisper),
                    extractor: Box::new(youtube::InnertubeExtractor),
                },
            )?;
            app.manage(core);
            let main = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()));
            #[cfg(desktop)]
            let main = main.title("영상 번역 오버레이").inner_size(480.0, 720.0);
            main.build()?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the screen while a job runs only hides it: the job goes on (R6).
            if let WindowEvent::CloseRequested { api, .. } = event {
                let core = window.app_handle().state::<Arc<Core>>();
                if core.store.active_job().ok().flatten().is_some() {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .build(ctx)
        .expect("start the app")
        .run(|_, _| {});
}
