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
                    let segments: Vec<Value> = order["segments"].as_array().unwrap().iter().map(|s| json!({"id": s["id"], "ko": format!("번역 {}", s["id"])})).collect();
                    json!({ "segments": segments })
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

/// A job past writing with `n` one-cue sentences, "Cue 0." to "Cue n-1.", for an hour of video.
fn written(core: &Core, n: i64) {
    let s = &core.store;
    s.create_job("j", "v", "https://a.example/talk", "other", "Lecture", 3_600_000, None, "m").unwrap();
    let cues: Vec<_> = (0..n).map(|i| (i * 1000, i * 1000 + 900, format!("Cue {i}."))).collect();
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
        assert_eq!(segs[0]["text"], format!("Cue {id}."));
        let before = (id > 0).then(|| format!("Cue {}.", id - 1));
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
    store.continue_remake(&key, "remake-a").unwrap();
    assert_eq!(played(&store), was, "playback was given the draft while it is translated");
    assert_eq!(store.latest_job(&key).unwrap().unwrap().id, "remake-a", "the overlay shows the newest job's cost");
    run_job(&core, &store, store.job("remake-a").unwrap().unwrap());
    assert_eq!(store.job("remake-a").unwrap().unwrap().stage, "ready");
    assert!(store.continue_remake(&key, "remake-a").is_err(), "only a failed re-make continues");
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
    assert!(store.continue_remake(&key, "remake-b").is_err(), "a re-make whose draft was replaced continued");
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
fn every_translation_of_the_same_cues_asks_the_same_sentences() {
    use crate::sentences::{push, Tail};
    use crate::Word;
    let word = |a: i64, b: i64, t: &str| Word { start_ms: a, end_ms: b, text: format!(" {t}") };
    let said = |at: i64, text: &str| -> Vec<Word> { text.split(' ').enumerate().map(|(i, t)| word(at + i as i64 * 300, at + i as i64 * 300 + 250, t)).collect() };
    let bare = (0..40).map(|i| format!("word{i:02}")).collect::<Vec<_>>().join(" ");
    let opening = (0..40).map(|i| format!("step{i}")).collect::<Vec<_>>().join(" ");
    // (form, the words the writer got): the forms on which the writer's own
    // grouping and the one rebuilt from its cues have come apart.
    let forms = [
        // A word with no time after a sentence end, then a bare run past the limit.
        ("no time, then a bare run", [vec![word(0, 500, "Done."), word(500, 500, "now")], said(600, &bare)].concat()),
        // A sentence with no time right after an ellipsis.
        ("after an ellipsis", vec![word(0, 500, "No..."), word(500, 500, "Okay."), word(510, 800, "we"), word(800, 1_100, "left.")]),
        // The first comma comes after the limit.
        ("first comma after the limit", said(0, &format!("{opening}, and only then does the sentence end. Next one."))),
    ];
    for (name, words) in forms {
        let (orders, most) = (Arc::new(Mutex::new(Vec::new())), Arc::new(AtomicUsize::new(0)));
        let d = dir(&format!("same-{}", name.replace(' ', "-")));
        let core = super::worker_tests::idle_core(&d, &gemini(flat(0, 1), orders.clone(), most));
        let journal = crate::sync::Journal::open(&d.join("account"), &crate::google_auth::VerifiedIdentity::fixture("same-account")).unwrap();
        let store = journal.library().unwrap();
        let page = "https://lectures.example.org/same";
        let key = crate::api::video_key(&url::Url::parse(page).unwrap());
        // The sentences of each translation, as the requests asked them.
        let asked = || -> Vec<Vec<String>> {
            let mut orders = orders.lock().unwrap();
            let mut groups: Vec<Vec<String>> = orders.drain(..).filter(|o| o["task"] == "translate").map(|o| o["segments"].as_array().unwrap().iter().map(|s| s["text"].as_str().unwrap().to_string()).collect()).collect();
            groups.sort();
            groups
        };

        store.create_job("j", &key, page, "other", "Lecture", 60_000, None, "m").unwrap();
        let (mut tail, mut prev_end) = (Tail::new(), 0);
        let cues = push(&mut tail, vec![crate::stitch::Piece { start_ms: words[0].start_ms, end_ms: words.last().unwrap().end_ms, words }], true, &mut prev_end);
        store.save_window("j", &key, 0, &cues, None, None).unwrap();
        store.end_writing("j").unwrap();
        run_job(&core, &store, store.job("j").unwrap().unwrap());
        assert_eq!(store.job("j").unwrap().unwrap().stage, "ready", "{name}");
        let first = asked();
        assert!(name == "after an ellipsis" || first.iter().any(|g| g.len() > 1), "{name}: no sentence in parts: {first:?}");

        // The saved version projected again (as when selected again or received), then re-translated.
        let version = store.library().unwrap()["videos"][0]["versions"].as_array().unwrap().iter().find(|v| v["selected"] == true).unwrap()["id"].as_str().unwrap().to_string();
        store.restore_translation(&key, &version).unwrap();
        std::thread::sleep(Duration::from_millis(5));
        store.retranslate("again", &store.latest_job(&key).unwrap().unwrap(), "m").unwrap();
        run_job(&core, &store, store.job("again").unwrap().unwrap());
        assert_eq!(store.job("again").unwrap().unwrap().stage, "ready", "{name}");
        assert_eq!(asked(), first, "{name}: the re-translation asked other sentences");
    }
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
        assert_eq!(store.continue_remake(&key, &id).is_ok(), whole, "{reason}");
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
fn a_stopped_remake_finds_the_version_it_started_from_in_the_database_alone() {
    use crate::sync::{Change, Cue as Saved, Deletions, Journal, Translation, Video};
    // (form, what happened after the job committed)
    for (form, stop) in [("stopped right after the job committed", None), ("failed writing", Some("write_failed")), ("failed translating", Some("translate_failed"))] {
        let d = dir(&format!("stopped-{}", form.replace(' ', "-")));
        let journal = Journal::open(&d.join("account"), &crate::google_auth::VerifiedIdentity::fixture("stopped-account")).unwrap();
        let page = "https://lectures.example.org/stopped";
        let key = crate::api::video_key(&url::Url::parse(page).unwrap());
        let video = Video { key: key.clone(), page_url: page.into(), title: "Lecture".into() };
        let cues = vec![Saved { start_ms: 0, end_ms: 900, source_text: "Said before.".into(), text_ko: "예전 번역".into() }];
        journal.write(Change::Translation { translation: Translation { video, duration_ms: 5000, source_language: Some("en".into()), terms: vec![], model: "m".into(), cost_krw: 1.0, cues } }, Deletions::new()).unwrap();
        let store = journal.library().unwrap();
        let input = d.join("input.m4a");
        std::fs::write(&input, b"audio").unwrap();
        let work = d.join("work");
        let previous = store.stage_remake("remake-a", &key, &input, &work.join("audio"), "m").unwrap();
        if let Some(reason) = stop {
            store.fail("remake-a", reason).unwrap();
        }
        let files: Vec<String> = std::fs::read_dir(&work).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(files, ["audio"], "{form}: the work folder holds more than the staged audio");
        assert_eq!(store.selected_version(&key).unwrap(), previous, "{form}");
        match stop {
            // `resume` takes the still active job and its staged audio.
            None => {
                let job = store.active_job().unwrap().unwrap();
                assert!(job.id == "remake-a" && std::path::Path::new(&job.audio_path.unwrap()).exists(), "{form}");
            }
            // `restore`: the version is projected again.
            Some(_) => {
                store.restore_translation(&key, &store.selected_version(&key).unwrap()).unwrap();
                assert_eq!(store.cues(&key).unwrap().iter().map(|c| c.text_ko.clone()).collect::<Vec<_>>(), [Some("예전 번역".to_string())], "{form}");
            }
        }
    }
}

#[test]
fn of_remakes_staged_at_once_under_one_job_id_one_wins_and_keeps_its_own_audio() {
    use crate::sync::{Change, Cue as Saved, Deletions, Journal, Translation, Video};
    const AT_ONCE: usize = 6;
    for round in 0..8 {
        let d = dir(&format!("staged-at-once-{round}"));
        let journal = Journal::open(&d.join("account"), &crate::google_auth::VerifiedIdentity::fixture("staged-at-once")).unwrap();
        let page = "https://lectures.example.org/at-once";
        let key = crate::api::video_key(&url::Url::parse(page).unwrap());
        let video = Video { key: key.clone(), page_url: page.into(), title: "Lecture".into() };
        let cues = vec![Saved { start_ms: 0, end_ms: 900, source_text: "Said.".into(), text_ko: "번역".into() }];
        journal.write(Change::Translation { translation: Translation { video, duration_ms: 5000, source_language: Some("en".into()), terms: vec![], model: "m".into(), cost_krw: 1.0, cues } }, Deletions::new()).unwrap();
        let path = journal.library().unwrap().path().to_path_buf();
        let (work, inputs) = (d.join("work"), d.join("inputs"));
        std::fs::create_dir_all(&inputs).unwrap();
        let start = Arc::new(std::sync::Barrier::new(AT_ONCE));
        let results: Vec<(usize, bool)> = (0..AT_ONCE)
            .map(|i| {
                let (path, work, key, start) = (path.clone(), work.clone(), key.clone(), start.clone());
                let input = inputs.join(format!("input-{i}.m4a"));
                std::fs::write(&input, format!("audio {i} ").repeat(50_000)).unwrap();
                std::thread::spawn(move || {
                    let store = Store::open(&path).unwrap();
                    start.wait();
                    (i, store.stage_remake("remake-same", &key, &input, &work, "m").is_ok())
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|t| t.join().unwrap())
            .collect();
        let won: Vec<usize> = results.iter().filter(|r| r.1).map(|r| r.0).collect();
        assert_eq!(won.len(), 1, "round {round}: {results:?}");
        let store = Store::open(&path).unwrap();
        let staged = std::path::PathBuf::from(store.job("remake-same").unwrap().unwrap().audio_path.unwrap());
        let kept = std::fs::read(&staged).ok();
        assert!(kept == Some(format!("audio {} ", won[0]).repeat(50_000).into_bytes()), "round {round}: the winner's audio was replaced or deleted ({:?} bytes)", kept.map(|k| k.len()));
        let left: Vec<String> = std::fs::read_dir(&work).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(left, ["remake-same.m4a"], "round {round}");
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

#[test]
fn a_remake_continues_only_under_its_own_video() {
    use crate::sync::{Change, Cue as Saved, Deletions, Journal, Translation, Video};
    let d = dir("continue-own-video");
    let journal = Journal::open(&d.join("account"), &crate::google_auth::VerifiedIdentity::fixture("continue-own-account")).unwrap();
    let keys: Vec<String> = ["https://lectures.example.org/first", "https://lectures.example.org/second", "https://lectures.example.org/third"].iter().map(|page| {
        let key = crate::api::video_key(&url::Url::parse(page).unwrap());
        let video = Video { key: key.clone(), page_url: (*page).into(), title: "Lecture".into() };
        let cues = vec![Saved { start_ms: 0, end_ms: 900, source_text: "Said.".into(), text_ko: "번역".into() }];
        journal.write(Change::Translation { translation: Translation { video, duration_ms: 5000, source_language: Some("en".into()), terms: vec![], model: "m".into(), cost_krw: 1.0, cues } }, Deletions::new()).unwrap();
        key
    }).collect();
    let store = journal.library().unwrap();
    let (a, b, c) = (keys[0].as_str(), keys[1].as_str(), keys[2].as_str());
    for (id, key) in [("remake-first", a), ("remake-second", b)] {
        std::thread::sleep(Duration::from_millis(5));
        store.begin_remake(id, key, "a.m4a", "m").unwrap();
        store.fail(id, "translate_failed").unwrap();
    }
    let stages = || ["remake-first", "remake-second"].map(|j| store.job(j).unwrap().unwrap().stage);
    // (video key given, job id given, whether another video's job is running, whether it continues):
    // a job moves only with its own video's key and while no other job runs.
    let forms = [
        (b, "remake-first", false, false),
        (a, "remake-second", false, false),
        ("unknown-video", "remake-first", false, false),
        (a, "remake-first", true, false),
        (a, "remake-first", false, true),
        (b, "remake-second", false, true),
    ];
    for (key, id, running, own) in forms {
        if running {
            store.begin_remake("remake-third", c, "a.m4a", "m").unwrap();
        }
        assert_eq!(store.continue_remake(key, id).is_ok(), own, "{key} {id} running {running}");
        if own {
            assert_eq!(store.job(id).unwrap().unwrap().stage, "translating", "{key} {id}");
            store.fail(id, "translate_failed").unwrap();
        } else {
            assert_eq!(stages(), ["failed", "failed"], "{key} {id} running {running}: a job moved");
        }
        if running {
            store.fail("remake-third", "translate_failed").unwrap();
        }
    }
}

#[test]
fn a_named_run_sends_requests_only_for_its_own_job() {
    let (orders, most) = (Arc::new(Mutex::new(Vec::new())), Arc::new(AtomicUsize::new(0)));
    let core = super::worker_tests::idle_core(&dir("named-run"), &gemini(flat(0, 1), orders.clone(), most));
    let s = &core.store;
    let ready_to_translate = |id: &str, key: &str| {
        s.create_job(id, key, &format!("https://a.example/{key}"), "other", "Lecture", 60_000, None, "m").unwrap();
        s.save_region(id, key, 0, &[(0, 900, format!("Said in {key}."))], None).unwrap();
        s.end_writing(id).unwrap();
    };
    let asked = || orders.lock().unwrap().iter().filter(|o| o["task"] == "translate").map(|o| o["segments"][0]["text"].as_str().unwrap().to_string()).collect::<Vec<_>>();

    // Its own job: run to its end, asking only for its own text.
    ready_to_translate("own", "first");
    run_named(&core, s, "own").unwrap();
    assert_eq!(s.job("own").unwrap().unwrap().stage, "ready");
    assert_eq!(asked(), ["Said in first."]);
    ready_to_translate("stopped", "third");
    s.fail("stopped", "translate_failed").unwrap();

    // Another video's job is running when the named one has ended, as it is at the
    // lookup after the named job's last commit: it is left alone.
    ready_to_translate("other", "second");
    for named in ["own", "stopped", "unknown-job"] {
        run_named(&core, s, named).unwrap();
        assert_eq!(asked(), ["Said in first."], "the run named {named} asked for another video");
        assert_eq!(s.job("other").unwrap().unwrap().stage, "translating", "the run named {named} took another video's job");
    }
    // Its own runner still takes it.
    run_named(&core, s, "other").unwrap();
    assert_eq!(s.job("other").unwrap().unwrap().stage, "ready");
    assert_eq!(asked(), ["Said in first.", "Said in second."]);
}

#[test]
fn a_second_runner_of_a_library_is_refused_until_the_first_ends() {
    let db = dir("runner-claim").join("library.sqlite3");
    let first = claim_runner(&db).unwrap();
    assert!(claim_runner(&db).is_err(), "a second runner of the same library was let in");
    let other = claim_runner(&db.with_file_name("other.sqlite3"));
    assert!(other.is_ok(), "another library's runner was refused");
    drop(first);
    assert!(claim_runner(&db).is_ok(), "the claim outlived its runner");
}
