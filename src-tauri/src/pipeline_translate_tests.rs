//! Translation requests run side by side, and each is still one cue with the
//! glossary and only the cue before it (A6, R9); every call is metered, and
//! cost never stops a job.

use super::*;
use serde_json::json;
use std::io::{BufRead, BufReader, Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};

/// How the fake answers its `n`th call (from 0): after how many ms, billing
/// how many prompt and output tokens.
type Bill = Arc<dyn Fn(usize) -> (u64, i64, i64) + Send + Sync>;

fn flat(delay_ms: u64, input: i64) -> Bill {
    Arc::new(move |_| (delay_ms, input, 1))
}

/// A Gemini that answers every connection on its own thread as `bill` says,
/// keeping each order it got and the most it was answering at once.
fn gemini(bill: Bill, orders: Arc<Mutex<Vec<Value>>>, most: Arc<AtomicUsize>) -> String {
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let at = server.local_addr().unwrap();
    let (now, arrived) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
    std::thread::spawn(move || {
        for stream in server.incoming() {
            let (orders, most, now, arrived, bill) = (orders.clone(), most.clone(), now.clone(), arrived.clone(), bill.clone());
            std::thread::spawn(move || {
                let mut stream = stream.unwrap();
                let mut r = BufReader::new(stream.try_clone().unwrap());
                let mut len = 0;
                loop {
                    let mut line = String::new();
                    r.read_line(&mut line).unwrap();
                    if line.trim().is_empty() {
                        break;
                    }
                    if let Some(n) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        len = n.trim().parse().unwrap();
                    }
                }
                let mut body = vec![0; len];
                r.read_exact(&mut body).unwrap();
                let body: Value = serde_json::from_slice(&body).unwrap();
                let order: Value = serde_json::from_str(body["contents"][0]["parts"][0]["text"].as_str().unwrap()).unwrap();
                let (delay_ms, input, output) = bill(arrived.fetch_add(1, Ordering::SeqCst));
                most.fetch_max(now.fetch_add(1, Ordering::SeqCst) + 1, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(delay_ms));
                now.fetch_sub(1, Ordering::SeqCst);
                let text = if order["task"] == "glossary" {
                    json!({"terms": [{"source": "attention", "target": "어텐션"}], "mishearings": [{"heard": "realu", "meant": "ReLU"}]})
                } else {
                    let id = &order["segments"][0]["id"];
                    json!({"segments": [{"id": id, "ko": format!("번역 {id}")}]})
                };
                orders.lock().unwrap().push(order);
                let body = json!({
                    "candidates": [{"content": {"parts": [{"text": text.to_string()}]}}],
                    "usageMetadata": {"promptTokenCount": input, "candidatesTokenCount": output}
                })
                .to_string();
                let head = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(body.as_bytes());
            });
        }
    });
    format!("http://{at}")
}

fn dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("vto-translate-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A job past writing with `n` cues, "cue 0" to "cue n-1", for an hour of video.
fn written(core: &Core, n: i64) {
    let s = &core.store;
    s.create_job("j", "v", "https://a.example/talk", "other", "Lecture", 3_600_000, None, "m").unwrap();
    let cues: Vec<_> = (0..n).map(|i| (i * 1000, i * 1000 + 900, format!("cue {i}"))).collect();
    s.save_region("j", "v", 0, &cues, None).unwrap();
    s.end_writing("j").unwrap();
}

#[test]
fn cues_are_translated_one_per_request_with_requests_overlapping() {
    let (orders, most) = (Arc::new(Mutex::new(Vec::new())), Arc::new(AtomicUsize::new(0)));
    let core = super::worker_tests::idle_core(&dir("overlap"), &gemini(flat(150, 10), orders.clone(), most.clone()));
    written(&core, 24);
    run_job(&core, &core.store, core.store.job("j").unwrap().unwrap());

    assert_eq!(core.store.job("j").unwrap().unwrap().stage, "ready");
    for c in core.store.cues("v").unwrap() {
        assert_eq!(c.text_ko, Some(format!("번역 {}", c.seq)));
    }
    let orders = orders.lock().unwrap();
    let translations: Vec<&Value> = orders.iter().filter(|o| o["task"] == "translate").collect();
    assert_eq!(translations.len(), 24);
    for o in &translations {
        let segs = o["segments"].as_array().unwrap();
        assert_eq!(segs.len(), 1, "a request with more than one cue: {o}");
        let id = segs[0]["id"].as_i64().unwrap();
        assert_eq!(segs[0]["text"], format!("cue {id}"));
        let before = (id > 0).then(|| format!("cue {}", id - 1));
        assert_eq!(o["context_before"].as_str().map(String::from), before, "the context is not the cue before: {o}");
        assert_eq!(o["glossary"], json!([{"source": "attention", "target": "어텐션"}]));
    }
    let most = most.load(Ordering::SeqCst);
    assert!((4..=IN_FLIGHT).contains(&most), "{most} requests at once");
    // Every call metered once: the glossary's and each cue's.
    assert_eq!(core.store.job("j").unwrap().unwrap().input_amount, 250);
}

#[test]
fn cost_is_metered_per_call_and_never_stops_translation() {
    // An hour's lecture that already spent 1,450 KRW, then 61 calls of 36.25 KRW each.
    let (orders, most) = (Arc::new(Mutex::new(Vec::new())), Arc::new(AtomicUsize::new(0)));
    let core = super::worker_tests::idle_core(&dir("metered"), &gemini(flat(50, 100_000), orders.clone(), most));
    written(&core, 60);
    core.store.add_usage("j", 0, 0, |_, _| 1_450.0).unwrap();
    run_job(&core, &core.store, core.store.job("j").unwrap().unwrap());

    let job = core.store.job("j").unwrap().unwrap();
    assert_eq!((job.stage.as_str(), job.failure_reason.as_deref()), ("ready", None));
    let calls = orders.lock().unwrap().len();
    assert_eq!(calls, 61, "the glossary and one request per cue");
    assert!((job.cost_krw - 1_450.0 - calls as f64 * gemini::cost_krw(100_000, 1)).abs() < 1e-6, "a call was not metered: {} KRW", job.cost_krw);
}

#[test]
fn a_continued_remake_publishes_its_draft_and_a_restored_one_cannot_continue() {
    use crate::sync::{Change, Cue as Saved, Deletions, Journal, Translation, Video};
    let (orders, most) = (Arc::new(Mutex::new(Vec::new())), Arc::new(AtomicUsize::new(0)));
    let d = dir("remake");
    let core = super::worker_tests::idle_core(&d, &gemini(flat(0, 1), orders.clone(), most));
    let journal = Journal::open(&d.join("account"), &crate::google_auth::VerifiedIdentity::fixture("remake-account")).unwrap();
    let page = "https://lectures.example.org/remade";
    let key = crate::api::video_key(&url::Url::parse(page).unwrap());
    let old = Saved { start_ms: 0, end_ms: 900, source_text: "Said before.".into(), text_ko: "예전 번역".into() };
    let video = Video { key: key.clone(), page_url: page.into(), title: "Lecture".into() };
    let previous = journal
        .write(Change::Translation { translation: Translation { video, duration_ms: 5000, source_language: Some("en".into()), terms: vec![], model: "m".into(), cost_krw: 10.0, cues: vec![old.clone()] } }, Deletions::new())
        .unwrap()
        .identity();
    let store = journal.library().unwrap();
    // What a re-make's writing left before it failed: one cue translated, one not.
    let draft = |id: &str, first: &str, second: &str| {
        std::thread::sleep(Duration::from_millis(5));
        store.begin_remake(id, &key, "a.m4a", "m").unwrap();
        let db = crate::db::Db::open(store.path()).unwrap();
        db.execute("INSERT INTO cue VALUES (?,0,0,900,?,'초안 번역')", crate::p![key.as_str(), first]).unwrap();
        db.execute("INSERT INTO cue VALUES (?,1,1000,1900,?,NULL)", crate::p![key.as_str(), second]).unwrap();
        db.execute("UPDATE video SET terms='[]', mishearings='[]' WHERE video_key=?", crate::p![key.as_str()]).unwrap();
        store.fail(id, "translate_failed").unwrap();
    };
    let selected = |store: &Store| store.library().unwrap()["videos"][0]["versions"].as_array().unwrap().iter().find(|v| v["selected"] == true).unwrap()["id"].as_str().unwrap().to_string();

    let played = |store: &Store| crate::api::subtitles(store, &key).unwrap();
    let was = json!({"video_key": key, "duration_ms": 5000, "source_language": "en", "cues": [{"seq": 0, "start_ms": 0, "end_ms": 900, "text_ko": "예전 번역"}]});

    // Continued: only the untranslated draft cue is asked, and the draft is published.
    draft("remake-a", "Draft one.", "Draft two.");
    assert_eq!(played(&store), was, "playback was given the failed re-make's draft, not the selected version");
    store.continue_remake("remake-a").unwrap();
    assert_eq!(played(&store), was, "playback was given the draft while it is translated");
    assert_eq!(store.latest_job(&key).unwrap().unwrap().id, "remake-a", "the overlay shows the newest job's cost");
    run_job(&core, &store, store.job("remake-a").unwrap().unwrap());
    assert_eq!(store.job("remake-a").unwrap().unwrap().stage, "ready");
    assert!(store.continue_remake("remake-a").is_err(), "only a failed re-make continues");
    let asked: Vec<Value> = orders.lock().unwrap().iter().filter(|o| o["task"] == "translate").cloned().collect();
    assert_eq!(asked.iter().map(|o| o["segments"].clone()).collect::<Vec<_>>(), [json!([{"id": 1, "text": "Draft two."}])]);
    let made = selected(&store);
    assert_ne!(made, previous);
    let texts: Vec<(String, Option<String>)> = store.cues(&key).unwrap().into_iter().map(|c| (c.source_text, c.text_ko)).collect();
    assert_eq!(texts, [("Draft one.".into(), Some("초안 번역".into())), ("Draft two.".into(), Some("번역 1".into()))]);
    assert!(journal.records().unwrap().iter().any(|r| r.identity() == made
        && matches!(&r.change, Change::Translation { translation } if translation.cues.iter().map(|c| c.source_text.as_str()).collect::<Vec<_>>() == ["Draft one.", "Draft two."])));
    assert_eq!(played(&store)["cues"].as_array().unwrap().iter().map(|c| c["text_ko"].as_str().unwrap()).collect::<Vec<_>>(), ["초안 번역", "번역 1"], "the ready re-make is played");

    // Restored instead: the previous version is back, and the re-make can no longer continue.
    draft("remake-b", "Another one.", "Another two.");
    store.restore_translation(&key, &made).unwrap();
    assert_eq!(selected(&store), made);
    assert!(store.continue_remake("remake-b").is_err(), "a re-make whose draft was replaced continued");
    assert_eq!(store.job("remake-b").unwrap().unwrap().stage, "failed");
    assert_eq!(store.cues(&key).unwrap().iter().map(|c| c.source_text.as_str()).collect::<Vec<_>>(), ["Draft one.", "Draft two."]);
    assert_eq!(orders.lock().unwrap().iter().filter(|o| o["task"] == "translate").count(), 1, "nothing more was asked");
}

#[test]
fn a_selected_version_is_translated_with_its_own_terms_and_mishearings_found_for_it() {
    use crate::sync::{Change, Cue as Saved, Deletions, Journal, Term, Translation, Video};
    let (orders, most) = (Arc::new(Mutex::new(Vec::new())), Arc::new(AtomicUsize::new(0)));
    let d = dir("versions");
    let core = super::worker_tests::idle_core(&d, &gemini(flat(0, 1), orders.clone(), most));
    let journal = Journal::open(&d.join("account"), &crate::google_auth::VerifiedIdentity::fixture("versions-account")).unwrap();
    let page = "https://lectures.example.org/versions";
    let key = crate::api::video_key(&url::Url::parse(page).unwrap());
    let version = |term: &str, said: &str| {
        let video = Video { key: key.clone(), page_url: page.into(), title: "Lecture".into() };
        let cues = vec![Saved { start_ms: 0, end_ms: 900, source_text: said.into(), text_ko: "번역".into() }];
        let terms = vec![Term { source: term.into(), target: format!("{term} 번역") }];
        journal.write(Change::Translation { translation: Translation { video, duration_ms: 5000, source_language: Some("en".into()), terms, model: "m".into(), cost_krw: 1.0, cues } }, Deletions::new()).unwrap().identity()
    };
    let older = version("encoder", "The encoder reads it.");
    let newer = version("decoder", "The decoder writes it.");
    let store = journal.library().unwrap();
    // The mishearings found for one version's transcript.
    store.set_glossary(&key, r#"[{"source":"decoder","target":"decoder 번역"}]"#, r#"[{"heard":"decodr","meant":"decoder"}]"#).unwrap();
    for (selected, term) in [(&older, "encoder"), (&newer, "decoder")] {
        store.select_translation(&key, selected).unwrap();
        let video = store.video(&key).unwrap().unwrap();
        assert_eq!(video.mishearings, None, "{selected}: another version's mishearings stayed");
        assert!(video.terms.unwrap().contains(term), "{selected}: not its own glossary");
    }
    // Re-translated, the selected version asks for its mishearings once and keeps its terms.
    std::thread::sleep(Duration::from_millis(5));
    let previous = store.latest_job(&key).unwrap().unwrap();
    store.retranslate("again", &previous, "m").unwrap();
    run_job(&core, &store, store.job("again").unwrap().unwrap());
    assert_eq!(store.job("again").unwrap().unwrap().stage, "ready");
    let orders = orders.lock().unwrap();
    assert_eq!(orders.iter().filter(|o| o["task"] == "glossary").count(), 1);
    let asked: Vec<&Value> = orders.iter().filter(|o| o["task"] == "translate").collect();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0]["glossary"], json!([{"source": "decoder", "target": "decoder 번역"}]), "the version's own terms");
    assert_eq!(asked[0]["mishearings"], json!([{"heard": "realu", "meant": "ReLU"}]), "the mishearings found for this transcript");
    assert_eq!(store.video(&key).unwrap().unwrap().mishearings.as_deref(), Some(r#"[{"heard":"realu","meant":"ReLU"}]"#));
}

#[test]
fn only_a_remake_that_failed_translating_continues() {
    use crate::sync::{Change, Cue as Saved, Deletions, Journal, Translation, Video};
    let d = dir("continue-table");
    let journal = Journal::open(&d.join("account"), &crate::google_auth::VerifiedIdentity::fixture("continue-account")).unwrap();
    let page = "https://lectures.example.org/stops";
    let key = crate::api::video_key(&url::Url::parse(page).unwrap());
    let video = Video { key: key.clone(), page_url: page.into(), title: "Lecture".into() };
    let cues = vec![Saved { start_ms: 0, end_ms: 900, source_text: "Said.".into(), text_ko: "번역".into() }];
    journal.write(Change::Translation { translation: Translation { video, duration_ms: 5000, source_language: Some("en".into()), terms: vec![], model: "m".into(), cost_krw: 1.0, cues } }, Deletions::new()).unwrap();
    let store = journal.library().unwrap();
    // (where it stopped, as stored; whether its transcript is whole)
    for (n, (reason, whole)) in [("translate_failed", true), ("key_rejected", true), ("download_failed", false), ("no_speech", false), ("unsupported_cpu", false)].into_iter().enumerate() {
        std::thread::sleep(Duration::from_millis(5));
        let id = format!("remake-{n}");
        store.begin_remake(&id, &key, "a.m4a", "m").unwrap();
        store.fail(&id, reason).unwrap();
        assert_eq!(store.continue_remake(&id).is_ok(), whole, "{reason}");
        assert_eq!(store.job(&id).unwrap().unwrap().stage, if whole { "translating" } else { "failed" }, "{reason}");
        store.fail(&id, reason).unwrap();
    }
}

#[test]
fn a_refused_remake_leaves_the_audio_of_an_interrupted_one_as_it_was() {
    use crate::sync::{Change, Cue as Saved, Deletions, Journal, Translation, Video};
    let d = dir("staged-audio");
    let journal = Journal::open(&d.join("account"), &crate::google_auth::VerifiedIdentity::fixture("staged-account")).unwrap();
    let page = "https://lectures.example.org/staged";
    let key = crate::api::video_key(&url::Url::parse(page).unwrap());
    let video = Video { key: key.clone(), page_url: page.into(), title: "Lecture".into() };
    let cues = vec![Saved { start_ms: 0, end_ms: 900, source_text: "Said.".into(), text_ko: "번역".into() }];
    journal.write(Change::Translation { translation: Translation { video, duration_ms: 5000, source_language: Some("en".into()), terms: vec![], model: "m".into(), cost_krw: 1.0, cues } }, Deletions::new()).unwrap();
    let store = journal.library().unwrap();
    let (work, inputs) = (d.join("work"), d.join("inputs"));
    std::fs::create_dir_all(&inputs).unwrap();
    let input = |name: &str, bytes: &[u8]| { let p = inputs.join(name); std::fs::write(&p, bytes).unwrap(); p };
    // A re-make interrupted while writing: still the active job, its audio staged.
    let first = input("first.m4a", b"first audio");
    store.stage_remake("remake-a", &key, &first, &work, "m").unwrap();
    let job = store.job("remake-a").unwrap().unwrap();
    let staged = std::path::PathBuf::from(job.audio_path.unwrap());
    assert_eq!(job.stage, "writing");
    // (form, video, job id, its input): every one is refused.
    for (form, video, id, file) in [
        ("same video, same extension", key.as_str(), "remake-b", input("second.m4a", b"second audio")),
        ("same video, another extension", key.as_str(), "remake-c", input("second.webm", b"second audio")),
        ("another video", "no-such-video", "remake-d", input("third.m4a", b"third audio")),
        ("the same job id again", key.as_str(), "remake-a", input("again.m4a", b"again audio")),
    ] {
        assert!(store.stage_remake(id, video, &file, &work, "m").is_err(), "{form}: accepted");
        assert_eq!(std::fs::read(&staged).unwrap(), b"first audio", "{form}: the interrupted job's audio changed");
        let left: Vec<String> = std::fs::read_dir(&work).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(left, ["remake-a.m4a"], "{form}: a refused copy stayed");
        assert_eq!(store.active_job().unwrap().unwrap().id, "remake-a", "{form}");
    }
}

#[test]
fn an_older_builds_selection_never_pairs_a_transcript_with_another_ones_mishearings() {
    use crate::sync::{Change, Cue as Saved, Deletions, Journal, Term, Translation, Video};
    let (orders, most) = (Arc::new(Mutex::new(Vec::new())), Arc::new(AtomicUsize::new(0)));
    let d = dir("older-selection");
    let core = super::worker_tests::idle_core(&d, &gemini(flat(0, 1), orders.clone(), most));
    let journal = Journal::open(&d.join("account"), &crate::google_auth::VerifiedIdentity::fixture("older-account")).unwrap();
    let page = "https://lectures.example.org/older";
    let key = crate::api::video_key(&url::Url::parse(page).unwrap());
    let version = |term: &str, said: &str| {
        let video = Video { key: key.clone(), page_url: page.into(), title: "Lecture".into() };
        let cues = vec![Saved { start_ms: 0, end_ms: 900, source_text: said.into(), text_ko: "번역".into() }];
        let terms = vec![Term { source: term.into(), target: format!("{term} 번역") }];
        journal.write(Change::Translation { translation: Translation { video, duration_ms: 5000, source_language: Some("en".into()), terms, model: "m".into(), cost_krw: 1.0, cues } }, Deletions::new()).unwrap().identity()
    };
    let a = version("encoder", "The encoder reads it.");
    let b = version("decoder", "The decoder writes it.");
    let store = journal.library().unwrap();
    let retranslate = |id: &str| {
        std::thread::sleep(Duration::from_millis(5));
        let previous = store.latest_job(&key).unwrap().unwrap();
        store.retranslate(id, &previous, "m").unwrap();
        run_job(&core, &store, store.job(id).unwrap().unwrap());
        assert_eq!(store.job(id).unwrap().unwrap().stage, "ready", "{id}");
    };
    // This build finds B's mishearings for B's transcript.
    store.select_translation(&key, &b).unwrap();
    retranslate("b-again");
    let db = crate::db::Db::open(store.path()).unwrap();
    let left = db.query_one("SELECT mishearings, mishearings_of FROM video WHERE video_key=?", crate::p![key.as_str()]).unwrap().unwrap();
    assert!(store.current_mishearings(&key).unwrap().is_some());
    // An older build selects A: its projection writes A's terms and cues and leaves the columns it does not know.
    store.select_translation(&key, &a).unwrap();
    db.execute("UPDATE video SET mishearings=?, mishearings_of=? WHERE video_key=?", crate::p![left[0].text(), left[1].text(), key.as_str()]).unwrap();
    assert_eq!(store.current_mishearings(&key).unwrap(), None, "B's mishearings were taken for A's transcript");
    // Reopened here, A's re-translation finds A's own first.
    let before = orders.lock().unwrap().len();
    retranslate("a-again");
    let orders = orders.lock().unwrap();
    let asked: Vec<&Value> = orders[before..].iter().collect();
    assert_eq!(asked.iter().map(|o| o["task"].as_str().unwrap()).collect::<Vec<_>>(), ["glossary", "translate"]);
    assert_eq!(asked[0]["transcript"], json!(["The encoder reads it."]), "the mishearings were found in A's transcript");
    assert_eq!(asked[1]["glossary"], json!([{"source": "encoder", "target": "encoder 번역"}]));
    assert_eq!(asked[1]["segments"], json!([{"id": 0, "text": "The encoder reads it."}]));
    assert!(store.current_mishearings(&key).unwrap().is_some(), "A's mishearings are kept for A's transcript");
}
