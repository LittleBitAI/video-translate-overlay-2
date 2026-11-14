//! The window of the app's screens ("main": S1 `/` and S2 `/preferences`).
//! Outside the `app` feature so the gate's tests reach it on the mock runtime.

use tauri::{Manager, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use sha2::{Digest, Sha256};

pub const MAIN: &str = "main";

/// The screens window, shown and in front. Closing it while no job runs
/// destroys it, yet the site window keeps the app running, so it is made
/// again when it is gone; the address bar and the settings are never out of reach.
/// `script`: Android's one window is also the site window and needs the overlay.
pub fn show_main<R: Runtime, M: Manager<R>>(app: &M, script: Option<&str>) -> tauri::Result<WebviewWindow<R>> {
    if let Some(w) = app.get_webview_window(MAIN) {
        #[cfg(desktop)]
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return Ok(w);
    }
    let profile: String = app.try_state::<std::sync::Arc<crate::Core>>()
        .map(|core| Sha256::digest(core.cfg.data_dir.to_string_lossy().as_bytes()).iter().map(|b| format!("{b:02x}")).collect())
        .unwrap_or_default();
    let b = WebviewWindowBuilder::new(app, MAIN, WebviewUrl::App("index.html".into()))
        .initialization_script(format!("Object.defineProperties(window,{{__WIKI_NATIVE_BUILD_HEAD__:{{value:'{}'}},__WIKI_NATIVE_PROFILE__:{{value:'{}'}}}})", env!("VTO_BUILD_HEAD"), profile));
    #[cfg(desktop)]
    let b = b.title("영상 번역 오버레이").inner_size(1024.0, 760.0);
    let b = match script {
        Some(s) => b.initialization_script_for_all_frames(s),
        None => b,
    };
    b.build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::test::{mock_builder, mock_context, noop_assets};

    #[test]
    fn the_screens_window_is_made_again_when_only_the_site_window_is_left() {
        // What a close while no job runs leaves: the site window, no screens window.
        // (The mock runtime never reports a window destroyed to the app, so the
        // test starts from that state rather than closing one.)
        let app = mock_builder().build(mock_context(noop_assets())).unwrap();
        WebviewWindowBuilder::new(&app, "site", WebviewUrl::External("https://www.youtube.com/".parse().unwrap())).build().unwrap();
        assert!(app.get_webview_window(MAIN).is_none());

        // The app is launched again: the screens are back, and the site window is untouched.
        let again = show_main(&app, None).expect("the screens window is made again");
        assert_eq!(again.label(), MAIN);
        assert!(app.get_webview_window(MAIN).is_some());
        assert!(app.get_webview_window("site").is_some());
        // While it is open, it is the same window brought forward, not a second one.
        show_main(&app, None).expect("an open screens window is reused");
        assert_eq!(app.webview_windows().len(), 2);
    }
}
