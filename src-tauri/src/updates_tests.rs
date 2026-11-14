use super::*;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};

fn release(v: &str) -> Value {
    json!({"tag_name":format!("v{v}"), "draft":false, "prerelease":false,
        "body":format!("{APPROVED}\n\n## Features\nSaved translations and release notices.\n\n## Installation\nInstall manually."),
        "assets":(["android-arm64", "windows-x64"].map(|p| {
            let name = asset_name(v, p).unwrap();
            json!({"name":name,"size":123,"state":"uploaded","browser_download_url":format!("https://github.com/{REPOSITORY}/releases/download/v{v}/{name}")})
        }))})
}

fn select_values(values: Vec<Value>, p: &str) -> Result<Option<Update>, String> {
    select(&serde_json::to_vec(&values).unwrap(), "0.1.6", p)
}

#[test]
fn stable_versions_and_platform_artifacts_are_ordered_numerically() {
    for platform in ["android-arm64", "windows-x64"] {
        let selected = select_values(vec![release("0.1.6"), release("0.1.10"), release("0.1.9"), release("0.1.5")], platform).unwrap().unwrap();
        assert_eq!(selected.version, "0.1.10");
        assert!(selected.download_url.ends_with(&asset_name("0.1.10", platform).unwrap()));
        assert_eq!(selected.summary, "Saved translations and release notices.");
    }
    assert_eq!(select_values(vec![release("0.1.5"), release("0.1.6")], "android-arm64").unwrap(), None);
    for v in ["01.2.3", "1.2", "1.2.3-beta.1", "1.2.3+build", "1.2.4294967296", "1.2.-1", "1.2.3.4"] { assert!(version(v).is_none(), "{v}"); }
}

#[test]
fn only_approved_complete_platform_releases_announce() {
    let good = release("0.1.7");
    let mut invalid = Vec::new();
    for (field, value) in [("draft", json!(true)), ("prerelease", json!(true)), ("body", json!("## Features\nUnapproved")),
        ("body", json!(format!("{APPROVED}\n## Features\n{}", "x".repeat(601)))), ("body", Value::Null),
        ("tag_name", json!("v0.1.8-beta.1")), ("assets", json!([]))] {
        let mut bad = good.clone(); bad[field] = value; invalid.push(bad);
    }
    for (field, value) in [("browser_download_url", json!("https://github.com.evil.test/file.apk")),
        ("browser_download_url", json!("http://github.com/LittleBitAI/video-translate-overlay-2/releases/download/v0.1.7/file.apk")),
        ("size", json!(0)), ("state", json!("new")), ("name", json!("wrong.apk"))] {
        let mut bad = good.clone(); bad["assets"][0][field] = value; invalid.push(bad);
    }
    assert!(select_values(vec![json!({"draft":false})], "android-arm64").is_err());
    for bad in invalid {
        assert_eq!(select_values(vec![bad.clone()], "android-arm64").unwrap(), None, "{bad}");
        assert_eq!(select_values(vec![bad, good.clone()], "android-arm64").unwrap().unwrap().version, "0.1.7");
    }
    let mut windows_only = good.clone(); windows_only["assets"].as_array_mut().unwrap().remove(0);
    assert!(select_values(vec![windows_only.clone()], "android-arm64").unwrap().is_none());
    assert!(select_values(vec![windows_only], "windows-x64").unwrap().is_some());
    assert!(select_values(vec![good], "unsupported").unwrap().is_none());
    for bytes in [b"null".as_slice(), b"{}", b"[", &vec![b' '; MAX_METADATA + 1]] { assert!(select(bytes, "0.1.6", "android-arm64").is_err()); }
}

#[test]
fn dismissal_survives_recreation_manual_check_reveals_it_and_a_newer_version_returns() {
    let dir = std::env::temp_dir().join(format!("vto-update-{}-dismiss", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let checker = Checker::default();
    let update = select_values(vec![release("0.1.7")], "android-arm64").unwrap();
    let reply = checker.check_with(&dir, false, move || Ok(update)).unwrap();
    assert_eq!(reply["update"]["version"], "0.1.7");
    checker.dismiss(&dir, "0.1.7").unwrap();
    assert!(checker.check_with(&dir, false, || panic!("cooldown must reuse cache")).unwrap()["dismissed"].as_bool().unwrap());
    let checker = Checker::default();
    let update = select_values(vec![release("0.1.7")], "android-arm64").unwrap();
    assert!(checker.check_with(&dir, false, move || Ok(update)).unwrap()["update"].is_null());
    let update = select_values(vec![release("0.1.7")], "android-arm64").unwrap();
    assert_eq!(checker.check_with(&dir, true, move || Ok(update)).unwrap()["update"]["version"], "0.1.7");
    let update = select_values(vec![release("0.1.8")], "android-arm64").unwrap();
    assert_eq!(checker.check_with(&dir, true, move || Ok(update)).unwrap()["update"]["version"], "0.1.8");
    checker.dismiss(&dir, "0.1.8").unwrap();
    checker.dismiss(&dir, "0.1.7").unwrap();
    let update = select_values(vec![release("0.1.7")], "android-arm64").unwrap();
    assert!(checker.check_with(&dir, true, move || Ok(update)).unwrap()["update"].is_object());
    assert!(checker.check_with(&dir, false, || panic!("reuse the cached release")).unwrap()["update"].is_null());
    std::fs::remove_file(dir.join("dismissed-update.txt")).unwrap();
    std::fs::remove_dir(dir).unwrap();
}

#[test]
fn simultaneous_checks_share_one_request_and_failures_remain_failures() {
    let checker = Arc::new(Checker::default());
    let count = Arc::new(AtomicUsize::new(0));
    let threads: Vec<_> = (0..8).map(|_| {
        let checker = checker.clone(); let count = count.clone();
        std::thread::spawn(move || checker.check_with(Path::new("."), false, move || {
            count.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(30));
            Err("server unreachable".into())
        }))
    }).collect();
    for thread in threads { assert_eq!(thread.join().unwrap().unwrap_err().error, "update_check_failed"); }
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert!(checker.check_with(Path::new("."), true, || Ok(None)).unwrap()["update"].is_null());
}

#[test]
fn download_revalidates_the_selected_version_and_never_accepts_a_caller_url() {
    let dir = std::env::temp_dir().join(format!("vto-update-{}-download", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("update-fixture.json"), serde_json::to_vec(&vec![release("0.1.7")]).unwrap()).unwrap();
    std::fs::write(dir.join("update-installed.txt"), "0.1.6").unwrap();
    let checker = Checker::default();
    if let Some(name) = asset_name("0.1.7", platform()) {
        assert!(checker.download(&dir, "0.1.7").unwrap().ends_with(&name));
    } else {
        assert_eq!(checker.download(&dir, "0.1.7").unwrap_err().status, 409);
    }
    for invalid in ["0.1.8", "https://evil.test/app.exe"] {
        assert_eq!(checker.download(&dir, invalid).unwrap_err().status, 409);
    }
    std::fs::write(dir.join("update-fixture.json"), b"[]").unwrap();
    assert_eq!(checker.download(&dir, "0.1.7").unwrap_err().status, 409);
    for name in ["update-fixture.json", "update-installed.txt"] { std::fs::remove_file(dir.join(name)).unwrap(); }
    std::fs::remove_dir(dir).unwrap();
}

#[test]
fn caller_timeout_keeps_the_slow_request_as_the_only_in_flight_worker() {
    let checker = Checker::default();
    let (release, wait) = std::sync::mpsc::channel();
    let result = checker.check_with(Path::new("."), false, move || { wait.recv().unwrap(); Ok(None) });
    assert_eq!(result.unwrap_err().error, "update_check_failed");
    assert!(checker.shared.0.lock().unwrap().busy);
    release.send(()).unwrap();
    assert!(checker.check_with(Path::new("."), false, || panic!("must join the existing request")).unwrap()["update"].is_null());
}
