//! Only the `app` feature (the real app binary) needs a build step: the
//! overlay bundle it injects into site pages, and tauri-build's context.
//! The contract-test build skips both.

fn main() {
    #[cfg(feature = "app")]
    app();
}

#[cfg(feature = "app")]
fn app() {
    use std::path::PathBuf;
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let root = manifest.parent().unwrap();
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("overlay");
    for p in ["src/overlay", "vite.overlay.config.ts"] {
        println!("cargo:rerun-if-changed={}", root.join(p).display());
    }
    let status = std::process::Command::new("node")
        .arg(root.join("node_modules/vite/bin/vite.js"))
        .args(["build", "-c", "vite.overlay.config.ts", "--logLevel", "warn"])
        .env("VTO_OVERLAY_OUT", &out)
        .current_dir(root)
        .status()
        .expect("run node (npm ci first)");
    assert!(status.success(), "bundling the overlay failed");

    // The same eight commands commands::COMMANDS registers, as `allow-<command>` permissions.
    const COMMANDS: &[&str] = &["browser_open", "clear_site_data", "get_settings", "put_api_key", "delete_api_key", "create_job", "get_job", "get_subtitles"];
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(COMMANDS))).expect("tauri-build");
}
