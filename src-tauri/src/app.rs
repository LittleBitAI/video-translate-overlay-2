//! The app: the settings screen ("main"), the in-app browser ("site") with
//! the overlay injected into every page, OS notifications, and whisper.cpp.
//! Built with `--features app`; the contract host is the test-only twin.
//! A library module, because Android loads the app as a shared library;
//! `main.rs` calls [`run`] on the desktop.

use crate as vto;
use crate::{commands, whisper::Whisper, youtube, Browser, Config, Core, Notifier, Services};
use std::sync::{Arc, OnceLock};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent, Wry};
use tauri_plugin_notification::NotificationExt;

/// src/overlay/inject.ts bundled by build.rs.
const OVERLAY_JS: &str = include_str!(concat!(env!("OUT_DIR"), "/overlay/overlay.js"));

type Handle = Arc<OnceLock<AppHandle<Wry>>>;

fn app(h: &Handle) -> Result<&AppHandle<Wry>, String> {
    h.get().ok_or_else(|| "the app is still starting".to_string())
}

/// The window site pages show in: "site" on the desktop; on Android, where an
/// app has one webview, "main" (the back button returns to the settings screen).
const SITE: &str = if cfg!(mobile) { "main" } else { "site" };

/// The site window. Its overlay script runs on every page it loads and
/// stays off Google's sign-in pages itself, so `overlay` needs no switch here.
struct TauriBrowser(Handle);

impl Browser for TauriBrowser {
    fn navigate(&self, url: &str, _overlay: bool) -> Result<(), String> {
        let app = app(&self.0)?;
        let url: tauri::Url = url.parse().map_err(|e| format!("{e}"))?;
        if let Some(w) = app.get_webview_window(SITE) {
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
        let w = app.get_webview_window(SITE).or_else(|| app.get_webview_window("main")).ok_or("no webview")?;
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
        // Android: the foreground service's ongoing notification carries the progress.
        #[cfg(target_os = "android")]
        if let Ok(app) = app(&self.0) {
            app.state::<crate::android::Mobile>().job(state, title, percent);
            return;
        }
        // ponytail: desktop toasts cannot be updated in place, so progress goes to the
        // window title and one toast says the job runs on.
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

    fn ready(&self, job_id: &str, text: &str, url: &str) {
        // The settings screen lists it with an open button too.
        if let Ok(app) = app(&self.0) {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.eval(format!("window.vtoReady && window.vtoReady({})", serde_json::json!({ "text": text, "url": url })));
            }
            #[cfg(windows)]
            match ready_toast(app, text, job_id) {
                Ok(()) => return,
                Err(e) => vto::log(format!("ready toast: {e}")),
            }
            #[cfg(target_os = "android")]
            match app.state::<crate::android::Mobile>().ready(job_id, text) {
                Ok(()) => return,
                Err(e) => vto::log(format!("ready notice: {e}")),
            }
        }
        let _ = job_id;
        self.show("자막 준비 완료", text);
    }
}

/// Opens the finished video in the site window (F4).
fn open_video(h: &Handle, url: &str) {
    if let Err(e) = TauriBrowser(h.clone()).navigate(url, true) {
        vto::log(format!("open {url}: {e}"));
    }
}

/// The ready toast's click launches `vto-open:<job id>`.
const SCHEME: &str = "vto-open";

/// Opens the video of a `vto-open:<job id>` argument, if there is one. Only a
/// job that finished here can be named, so a link elsewhere cannot steer the app.
fn open_from_args(h: &Handle, args: &[String]) {
    let Some(id) = args.iter().find_map(|a| a.strip_prefix(SCHEME)?.strip_prefix(':')) else { return };
    let id = id.trim_matches('/');
    let Ok(app) = app(h) else { return };
    let core = app.state::<Arc<Core>>();
    let page = core.store.job(id).ok().flatten().filter(|j| j.notified).and_then(|j| core.store.video(&j.video_key).ok().flatten());
    match page {
        Some(v) => open_video(h, &v.page_url),
        None => vto::log(format!("{SCHEME}: no finished job {id}")),
    }
}

/// A Windows toast whose click opens the video at any later time, even after
/// the app has quit: it is a protocol launch of this executable, not an event
/// on a live object. A running app gets the launch through single-instance.
#[cfg(windows)]
fn ready_toast(app: &AppHandle<Wry>, text: &str, job_id: &str) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    // An unpackaged build has no registered app id of its own; the plugin uses PowerShell's then too.
    let dev = exe.parent().is_some_and(|d| d.ends_with("target/debug") || d.ends_with("target/release"));
    let app_id = if dev { POWERSHELL_APP_ID.to_string() } else { app.config().identifier.clone() };
    // WinRT needs COM on this (the worker's) thread; an earlier init is fine too.
    unsafe {
        let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED);
    }
    show_once(&app_id, &toast_xml(text, job_id), job_id).map(|_| ()).map_err(|e| e.to_string())
}

/// Shows the toast tagged with its job, unless that job's toast is still in
/// Action Center: a job resumed after a crash between showing its notice and
/// recording it sends again, and that must not make a second one (E2).
/// True when it was shown.
#[cfg(windows)]
fn show_once(app_id: &str, xml: &str, job_id: &str) -> windows::core::Result<bool> {
    use windows::core::HSTRING;
    use windows::Data::Xml::Dom::XmlDocument;
    use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
    let (app_id, tag) = (HSTRING::from(app_id), HSTRING::from(job_id));
    let earlier = ToastNotificationManager::History()?.GetHistoryWithId(&app_id)?;
    if earlier.into_iter().any(|t| t.Tag().is_ok_and(|g| g == tag) && t.Group().is_ok_and(|g| g == TOAST_GROUP)) {
        return Ok(false);
    }
    let doc = XmlDocument::new()?;
    doc.LoadXml(&HSTRING::from(xml))?;
    let toast = ToastNotification::CreateToastNotification(&doc)?;
    toast.SetTag(&tag)?; // job ids are 36 characters; tags may have 64
    toast.SetGroup(&HSTRING::from(TOAST_GROUP))?;
    ToastNotificationManager::CreateToastNotifierWithId(&app_id)?.Show(&toast)?;
    Ok(true)
}

#[cfg(windows)]
const TOAST_GROUP: &str = "vto-ready";

/// The ready toast; a click launches `vto-open:<job id>`.
#[cfg(windows)]
fn toast_xml(text: &str, job_id: &str) -> String {
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;");
    format!(
        r#"<toast activationType="protocol" launch="{SCHEME}:{}"><visual><binding template="ToastGeneric"><text>자막 준비 완료</text><text>{}</text></binding></visual></toast>"#,
        esc(job_id),
        esc(text)
    )
}

#[cfg(all(test, windows))]
#[path = "app_toast_tests.rs"]
mod toast_tests;

#[cfg(windows)]
const POWERSHELL_APP_ID: &str = r"{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\WindowsPowerShell\v1.0\powershell.exe";

/// `HKCU\Software\Classes\vto-open`: this user's protocol handler, pointing at this executable.
#[cfg(windows)]
fn register_scheme(exe: &std::path::Path) -> Result<(), String> {
    use windows::core::HSTRING;
    use windows::Win32::System::Registry::{RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ};
    let set = |key: &str, name: Option<&str>, value: &str| {
        let wide: Vec<u16> = value.encode_utf16().chain([0]).collect();
        let bytes = unsafe { std::slice::from_raw_parts(wide.as_ptr().cast::<u8>(), wide.len() * 2) };
        let name = name.map(HSTRING::from);
        let err = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                &HSTRING::from(format!(r"Software\Classes\{SCHEME}{key}")),
                name.as_ref().map_or(windows::core::PCWSTR::null(), |n| windows::core::PCWSTR(n.as_ptr())),
                REG_SZ.0,
                Some(bytes.as_ptr().cast()),
                bytes.len() as u32,
            )
        };
        err.ok().map_err(|e| format!("register {SCHEME}: {e}"))
    };
    set("", None, "URL:영상 번역 오버레이")?;
    set("", Some("URL Protocol"), "")?;
    set(r"\shell\open\command", None, &format!("\"{}\" \"%1\"", exe.display()))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let handle: Handle = Arc::new(OnceLock::new());
    let setup_handle = handle.clone();
    let mut ctx = tauri::generate_context!();
    *ctx.runtime_authority_mut() = commands::authority();
    let builder = commands::handlers(tauri::Builder::default());
    // Registered first: a second launch (a clicked toast) hands its arguments over and exits
    // before it opens the store the running app's worker uses.
    #[cfg(desktop)]
    let builder = {
        let h = handle.clone();
        builder.plugin(tauri_plugin_single_instance::init(move |_, args, _| open_from_args(&h, &args)))
    };
    #[cfg(target_os = "android")]
    let builder = builder.plugin(crate::android::plugin());
    builder
        .plugin(tauri_plugin_notification::init())
        .setup(move |app| {
            let _ = setup_handle.set(app.handle().clone());
            let data_dir = app.path().app_local_data_dir()?;
            let whisper = Whisper::new(&data_dir, std::env::var_os("VTO_WHISPER_MODEL").map(Into::into));
            #[cfg(target_os = "android")]
            let keystore: Box<dyn vto::KeyStore> = Box::new(app.state::<crate::android::Mobile>().inner().clone());
            #[cfg(not(target_os = "android"))]
            let keystore: Box<dyn vto::KeyStore> = Box::new(crate::platform::CredentialStore);
            // Debug builds only: device tests name a stand-in Gemini in the app's private data folder.
            #[cfg(debug_assertions)]
            let gemini = std::fs::read_to_string(data_dir.join("gemini_base_url.txt")).ok().map(|s| s.trim().to_string());
            #[cfg(not(debug_assertions))]
            let gemini: Option<String> = None;
            let gemini_base_url = gemini.unwrap_or_else(|| vto::GEMINI_BASE_URL.into());
            let core = Core::start(
                Config { data_dir, gemini_base_url, allow_loopback_media: false },
                Services {
                    keystore,
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
            // The one window is also the site window on Android.
            #[cfg(mobile)]
            let main = main.initialization_script(OVERLAY_JS);
            main.build()?;
            #[cfg(target_os = "android")]
            {
                let h = setup_handle.clone();
                let open = move |id: &str| open_from_args(&h, &[format!("{SCHEME}:{id}")]);
                if let Err(e) = app.state::<crate::android::Mobile>().watch_open(open) {
                    vto::log(e);
                }
            }
            // On every start, so the handler follows the executable if it moves.
            #[cfg(windows)]
            if let Err(e) = std::env::current_exe().map_err(|e| e.to_string()).and_then(|exe| register_scheme(&exe)) {
                vto::log(e);
            }
            // Started by a clicked toast while the app was not running.
            open_from_args(&setup_handle, &std::env::args().collect::<Vec<_>>());
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
