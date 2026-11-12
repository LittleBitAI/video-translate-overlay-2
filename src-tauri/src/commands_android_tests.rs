//! Android's overlay capability: site pages there load in `main`, so the
//! capability names `main`, but still only for remote pages and only P6-P8.

use super::*;
use tauri::utils::acl::ExecutionContext;
use tauri::utils::platform::Target;

#[test]
fn android_site_pages_get_only_p6_to_p8_and_only_on_android() {
    let android: Capability = serde_json::from_str(OVERLAY_ANDROID_CAPABILITY).unwrap();
    let desktop: Capability = serde_json::from_str(OVERLAY_CAPABILITY).unwrap();
    assert_eq!(android.permissions, desktop.permissions);
    assert_eq!(android.remote, desktop.remote);
    assert!(!android.local, "the app's own screen keeps the main capability alone");
    assert_eq!(android.platforms, Some(vec![Target::Android]));

    let acl = authority_acl();
    let caps = |t| {
        let all = [MAIN_CAPABILITY, OVERLAY_CAPABILITY, OVERLAY_ANDROID_CAPABILITY]
            .iter()
            .map(|s| serde_json::from_str::<Capability>(s).unwrap())
            .map(|c| (c.identifier.clone(), c))
            .collect();
        Resolved::resolve(&acl, all, t).unwrap()
    };
    // Resolution drops it on Windows and keeps it on Android.
    let on = |t| {
        let r = caps(t);
        let remote_main: Vec<&String> = r
            .allowed_commands
            .iter()
            .filter(|(_, cs)| cs.iter().any(|c| c.windows.iter().any(|w| w.matches("main")) && matches!(c.context, ExecutionContext::Remote { .. })))
            .map(|(k, _)| k)
            .collect();
        // Never settings, the key or the browser for a remote page.
        assert!(remote_main.iter().all(|k| OVERLAY_COMMANDS.iter().any(|c| k.ends_with(c))), "{remote_main:?}");
        !remote_main.is_empty()
    };
    assert!(!on(Target::Windows));
    assert!(on(Target::Android));
}
