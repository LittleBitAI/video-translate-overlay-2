//! Only the `app` feature (the real app binary) needs a build step: the
//! screens (`dist/`, tauri.conf.json's frontendDist), the overlay bundle it
//! injects into site pages, and tauri-build's context. The contract-test
//! build skips them all.

fn main() {
    let output = std::process::Command::new("git").args(["rev-parse", "HEAD"]).output().expect("read build revision");
    assert!(output.status.success(), "read build revision");
    let head = String::from_utf8(output.stdout).unwrap();
    println!("cargo:rustc-env=VTO_BUILD_HEAD={}", head.trim());
    for name in ["HEAD", "packed-refs"] {
        let path = std::process::Command::new("git").args(["rev-parse", "--git-path", name]).output().unwrap();
        println!("cargo:rerun-if-changed={}", String::from_utf8(path.stdout).unwrap().trim());
    }
    let reference = std::process::Command::new("git").args(["symbolic-ref", "-q", "HEAD"]).output().unwrap();
    if reference.status.success() {
        let name = String::from_utf8(reference.stdout).unwrap();
        let path = std::process::Command::new("git").args(["rev-parse", "--git-path", name.trim()]).output().unwrap();
        println!("cargo:rerun-if-changed={}", String::from_utf8(path.stdout).unwrap().trim());
    }
    #[cfg(feature = "app")]
    app();
}

#[cfg(feature = "app")]
fn app() {
    use std::path::PathBuf;
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let root = manifest.parent().unwrap();
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("overlay");
    for p in ["src/overlay", "src/ui", "index.html", "vite.config.ts", "vite.overlay.config.ts"] {
        println!("cargo:rerun-if-changed={}", root.join(p).display());
    }
    for (config, what) in [("vite.config.ts", "the screens"), ("vite.overlay.config.ts", "the overlay")] {
        let status = std::process::Command::new("node")
            .arg(root.join("node_modules/vite/bin/vite.js"))
            .args(["build", "-c", config, "--logLevel", "warn"])
            .env("VTO_OVERLAY_OUT", &out)
            .current_dir(root)
            .status()
            .expect("run node (npm ci first)");
        assert!(status.success(), "building {what} failed");
    }

    // whisper-rs-sys tests the host OS, not the target, and asks for advapi32 even
    // on Android when built on Windows. Nothing uses it there: give the linker an empty one.
    if std::env::var("TARGET").unwrap().contains("android") {
        let stub = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("stub");
        std::fs::create_dir_all(&stub).unwrap();
        std::fs::write(stub.join("libadvapi32.a"), b"!<arch>\n").unwrap();
        println!("cargo:rustc-link-search=native={}", stub.display());
    }

    // The same commands commands::COMMANDS registers, as `allow-<command>` permissions.
    const COMMANDS: &[&str] = &["browser_open", "clear_site_data", "get_settings", "put_api_key", "delete_api_key", "create_job", "get_job", "get_subtitles", "get_browser", "get_library", "library_action"];
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(COMMANDS))).expect("tauri-build");
}
