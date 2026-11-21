//! What the library holds when something happens to the video or the account
//! while the paid summary call is out: the fake Gemini holds its reply until
//! the test has done the interleaved step.

use crate::account::tests::signed_in_manager;
use crate::api::{library_request, video_key};
use crate::pipeline::worker_tests::{idle_core, local_gemini};
use crate::store::Store;
use crate::Core;
use serde_json::{json, Value};
use std::sync::{mpsc, Arc, Mutex};

const PAGE: &str = "https://lectures.example.org/summary-flow";

fn reply() -> Value {
    json!({"gist": "어텐션을 설명합니다.", "key_points": ["어텐션은 가중합입니다."], "topics": ["attention"],
           "level": "beginner", "prerequisites": ["선형대수 기초"], "next_topics": ["multi-head attention"]})
}

fn key() -> String { video_key(&url::Url::parse(PAGE).unwrap()) }

type Step = Box<dyn FnOnce() + Send>;

/// Steps a test runs when a request in its own library file reaches a named
/// point of `summary_in`: `read` (a read between its pending check and its
/// row) or `opened` (a call that has just opened its token).
static PAUSES: Mutex<Vec<(&'static str, std::path::PathBuf, Step)>> = Mutex::new(Vec::new());

pub(super) fn pause(point: &'static str, library: &crate::library_scope::Library<'_>) {
    let path = library.run_original(|s| Ok(s.path().to_path_buf())).unwrap();
    let step = {
        let mut pauses = PAUSES.lock().unwrap();
        pauses.iter().position(|(at, file, _)| *at == point && *file == path).map(|i| pauses.remove(i).2)
    };
    if let Some(step) = step { step() }
}

fn at(point: &'static str, core: &Core, step: impl FnOnce() + Send + 'static) {
    let path = in_library(core, |s| s.path().to_path_buf());
    PAUSES.lock().unwrap().push((point, path, Box::new(step)));
}

fn temp() -> std::path::PathBuf {
    let mut random = [0u8; 8];
    getrandom::fill(&mut random).unwrap();
    let id: String = random.iter().map(|b| format!("{b:02x}")).collect();
    let dir = std::env::temp_dir().join(format!("vto-summary-{id}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A core signed in to one account, whose Gemini signals each call on the
/// returned receiver and answers only once the returned sender says so.
fn core(dir: &std::path::Path) -> (Core, mpsc::Receiver<()>, mpsc::Sender<()>) {
    core_answering(dir, Arc::new(Mutex::new(reply())))
}

/// As [`core`], with Gemini answering whatever `answer` holds at the time.
fn core_answering(dir: &std::path::Path, answer: Arc<Mutex<Value>>) -> (Core, mpsc::Receiver<()>, mpsc::Sender<()>) {
    let (arrived, called) = mpsc::channel();
    let (go, wait) = mpsc::channel::<()>();
    let gemini = local_gemini(move |_| {
        arrived.send(()).unwrap();
        wait.recv().unwrap();
        (200, answer.lock().unwrap().clone(), 1000)
    });
    let mut core = idle_core(dir, &gemini);
    core.account = signed_in_manager(dir, "summary-one");
    (core, called, go)
}

fn ready(store: &Store, job: &str, token: Option<&str>) { ready_page(store, job, PAGE, token) }

fn ready_page(store: &Store, job: &str, page: &str, token: Option<&str>) {
    let key = video_key(&url::Url::parse(page).unwrap());
    store.create_job_with_consent(job, &key, page, "other", "Attention", 60_000, None, "m", token).unwrap();
    store.save_region(job, &key, 0, &[(0, 900, "Attention is a weighted sum.".into())], None).unwrap();
    store.set_language(&key, "en").unwrap();
    store.set_terms(&key, "[]").unwrap();
    store.set_translations(job, &[(0, "어텐션은 가중합입니다.".into())]).unwrap();
    store.finish(job).unwrap();
}

fn generation(core: &Core) -> u64 { core.account.status().unwrap().generation }

fn in_library<T>(core: &Core, f: impl FnOnce(&Store) -> T) -> T {
    core.library(Some(generation(core))).unwrap().run(|s| Ok(f(s))).unwrap()
}

/// One 요약 press; `during` acts while Gemini holds its reply.
fn summarize(core: &Core, called: &mpsc::Receiver<()>, go: &mpsc::Sender<()>, during: impl FnOnce(&Core)) -> Result<Value, String> {
    press(core, "make", called, go, during)
}

/// One press of 요약 (`make`) or 다시 요약 (`again`); `during` acts while Gemini holds its reply.
fn press(core: &Core, action: &str, called: &mpsc::Receiver<()>, go: &mpsc::Sender<()>, during: impl FnOnce(&Core)) -> Result<Value, String> {
    let at = generation(core);
    std::thread::scope(|scope| {
        let press = scope.spawn(|| library_request(core, Some(at), "video_summary", &json!({"video_key": key(), "summary_action": action})));
        called.recv().unwrap();
        during(core);
        go.send(()).unwrap();
        press.join().unwrap().map_err(|e| e.error)
    })
}

/// The property: a reply is written only into the entry it was read from.
/// Whatever replaces that entry during the call, even one under the same
/// job id (a synced restore projects `sync-<hash>` from the same content),
/// gets neither the summary nor its usage.
#[test]
fn a_summary_is_stored_only_into_the_entry_it_was_read_from() {
    fn delete(s: &Store) { s.begin_delete(&key(), "token-a").unwrap(); s.finish_delete(&key(), None).unwrap(); }
    let steps: [(&str, fn(&Store), Result<(), &str>); 5] = [
        ("nothing happened", |_| {}, Ok(())),
        ("the video was deleted", delete, Err("video_deleted")),
        ("the video is being deleted", |s| s.begin_delete(&key(), "token-a").unwrap(), Err("video_deleted")),
        ("the video was deleted and translated again", |s| { delete(s); ready(s, "second", Some("token-a")); }, Err("video_deleted")),
        ("the video was deleted and came back under the same job id", |s| { delete(s); ready(s, "first", Some("token-a")); }, Err("video_deleted")),
    ];
    for (name, step, outcome) in steps {
        let dir = temp();
        let (core, called, go) = core(&dir);
        in_library(&core, |s| ready(s, "first", None));
        let got = summarize(&core, &called, &go, |core| in_library(core, step));
        assert_eq!(got.as_ref().map(|_| ()).map_err(String::as_str), outcome, "{name}");
        let row = in_library(&core, |s| s.summary(&key()).unwrap());
        assert_eq!(row.as_ref().is_some_and(|r| r.body.is_some()), outcome.is_ok(), "{name}: a summary is stored");
        assert_eq!(row.as_ref().map_or(0, |r| r.input), if outcome.is_ok() { 1000 } else { 0 }, "{name}: usage recorded");
        drop(core);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// The guard: no form of 요약 reaches Gemini for a video marked for deletion.
#[test]
fn a_video_marked_for_deletion_is_never_sent_to_gemini() {
    for action in ["make", "again"] {
        let dir = temp();
        let (core, called, _go) = core(&dir);
        in_library(&core, |s| { ready(s, "first", None); s.begin_delete(&key(), "token-a").unwrap(); });
        let got = library_request(&core, Some(generation(&core)), "video_summary", &json!({"video_key": key(), "summary_action": action}));
        assert_eq!(got.unwrap_err().error, "video_deleted", "{action}");
        assert!(called.try_recv().is_err(), "{action}: Gemini was called");
        drop(core);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// The property: a read while a call is out says so, beside any stored
/// summary, and a read after it shows what the call did: the new summary, or
/// the stored one (if any) and why the call stored nothing.
#[test]
fn a_read_during_and_after_a_call_tells_what_the_call_did() {
    let read = |core: &Core| library_request(core, Some(generation(core)), "video_summary", &json!({"video_key": key(), "summary_action": "get"}));
    let broken = { let mut r = reply(); r["topics"] = json!([]); r };
    let newer = { let mut r = reply(); r["gist"] = json!("어텐션을 다시 설명합니다."); r };
    let (old, new) = (Some("어텐션을 설명합니다."), Some("어텐션을 다시 설명합니다."));
    for (name, stored_before, action, answer, during_gist, after_gist, failed) in [
        ("a first summary", false, "make", reply(), None, old, false),
        ("a first summary whose reply breaks a rule", false, "make", broken.clone(), None, None, true),
        ("a re-summary", true, "again", newer, old, new, false),
        ("a re-summary whose reply breaks a rule", true, "again", broken, old, old, true),
    ] {
        let dir = temp();
        let answer_now = Arc::new(Mutex::new(reply()));
        let (core, called, go) = core_answering(&dir, answer_now.clone());
        in_library(&core, |s| ready(s, "first", None));
        if stored_before { summarize(&core, &called, &go, |_| {}).unwrap(); }
        *answer_now.lock().unwrap() = answer;
        let mut during = None;
        let _ = press(&core, action, &called, &go, |core| during = Some(read(core).unwrap()));
        let during = during.unwrap();
        assert_eq!(during["pending"], true, "{name}: during");
        assert_eq!(during["summary"]["gist"].as_str(), during_gist, "{name}: during");
        let after = read(&core).unwrap();
        assert!(after.get("pending").is_none(), "{name}: after");
        assert_eq!(after["summary"]["gist"].as_str(), after_gist, "{name}: after");
        assert_eq!(after["failure"].as_str().is_some_and(|f| f.contains("규칙에 맞지 않아")), failed, "{name}: failure");
        drop(core);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// The property: a read that overlaps any part of a call never looks
/// final. Whether the call ends during the read (writes its result between
/// the read's looks), starts during it, runs whole during it, or opens its
/// token at the read, the read says pending, and the read after the call
/// shows what it did.
#[test]
fn a_read_overlapping_any_part_of_a_call_says_pending() {
    let read = |core: &Core| library_request(core, Some(generation(core)), "video_summary", &json!({"video_key": key(), "summary_action": "get"}));
    let broken = { let mut r = reply(); r["topics"] = json!([]); r };
    let newer = { let mut r = reply(); r["gist"] = json!("어텐션을 다시 설명합니다."); r };
    let (old, new) = (Some("어텐션을 설명합니다."), Some("어텐션을 다시 설명합니다."));
    for edge in ["end", "start", "begins during the read", "runs whole during the read"] {
        for (name, stored_before, action, answer, after_gist, failed) in [
            ("a first summary", false, "make", reply(), old, false),
            ("a first summary whose reply breaks a rule", false, "make", broken.clone(), None, true),
            ("a re-summary", true, "again", newer.clone(), new, false),
            ("a re-summary whose reply breaks a rule", true, "again", broken.clone(), old, true),
        ] {
            let dir = temp();
            let answer_now = Arc::new(Mutex::new(reply()));
            let (core, called, go) = core_answering(&dir, answer_now.clone());
            in_library(&core, |s| ready(s, "first", None));
            if stored_before { summarize(&core, &called, &go, |_| {}).unwrap(); }
            *answer_now.lock().unwrap() = answer;
            let at_edge = std::thread::scope(|scope| {
                let (done, finished) = mpsc::channel();
                let (reached, opened) = mpsc::channel();
                let (resume, held) = mpsc::channel::<()>();
                let core_ref = &core;
                if edge == "start" {
                    at("opened", &core, move || { reached.send(()).unwrap(); held.recv().unwrap(); });
                } else if edge.contains("during the read") {
                    // The read looks for a call (none), then the call starts before it reads the row.
                    at("read", &core, move || { reached.send(()).unwrap(); held.recv().unwrap(); });
                    let reading = scope.spawn(move || read(core_ref).unwrap());
                    opened.recv().unwrap();
                    let mut post = Some(scope.spawn(move || library_request(core_ref, Some(generation(core_ref)), "video_summary", &json!({"video_key": key(), "summary_action": action}))));
                    called.recv().unwrap();
                    let whole = edge == "runs whole during the read";
                    if whole { go.send(()).unwrap(); let _ = post.take().unwrap().join().unwrap(); }
                    resume.send(()).unwrap();
                    let seen = reading.join().unwrap();
                    if !whole { go.send(()).unwrap(); let _ = post.take().unwrap().join().unwrap(); }
                    return seen;
                }
                let post = scope.spawn(move || {
                    let core = core_ref;
                    let r = library_request(core, Some(generation(core)), "video_summary", &json!({"video_key": key(), "summary_action": action}));
                    done.send(()).unwrap();
                    r
                });
                let seen = if edge == "start" {
                    opened.recv().unwrap();
                    let seen = read(&core).unwrap();
                    resume.send(()).unwrap();
                    called.recv().unwrap();
                    go.send(()).unwrap();
                    seen
                } else {
                    called.recv().unwrap();
                    // Between the read's pending check and its row, the call ends: it writes and stops being pending.
                    let go = go.clone();
                    at("read", &core, move || { go.send(()).unwrap(); finished.recv().unwrap(); });
                    read(&core).unwrap()
                };
                let _ = post.join().unwrap();
                seen
            });
            assert_eq!(at_edge["pending"], true, "{edge} of {name}: {at_edge}");
            let after = read(&core).unwrap();
            assert!(after.get("pending").is_none(), "{edge} of {name}: after");
            assert_eq!(after["summary"]["gist"].as_str(), after_gist, "{edge} of {name}: after");
            assert_eq!(after["failure"].as_str().is_some_and(|f| f.contains("규칙에 맞지 않아")), failed, "{edge} of {name}: failure");
            drop(core);
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}

/// The property: a press is pending from the moment it is made, also while
/// it waits behind another call. A screen reopened on the queued video says
/// a summary is coming, and the read after its turn shows what it did.
#[test]
fn a_press_queued_behind_another_call_says_pending() {
    const OTHER: &str = "https://lectures.example.org/summary-flow-other";
    let request = |core: &Core, page: &str, action: &str| library_request(core, Some(generation(core)), "video_summary",
        &json!({"video_key": video_key(&url::Url::parse(page).unwrap()), "summary_action": action}));
    let broken = { let mut r = reply(); r["topics"] = json!([]); r };
    let newer = { let mut r = reply(); r["gist"] = json!("어텐션을 다시 설명합니다."); r };
    for (name, page, stored_before, action, answer, after_gist, failed) in [
        ("a first summary of another video", OTHER, false, "make", reply(), Some("어텐션을 설명합니다."), false),
        ("a re-summary of another video", OTHER, true, "again", newer, Some("어텐션을 다시 설명합니다."), false),
        ("a first summary of another video whose reply breaks a rule", OTHER, false, "make", broken, None, true),
        ("a second press on the same video", PAGE, false, "make", reply(), Some("어텐션을 설명합니다."), false),
    ] {
        let dir = temp();
        let answer_now = Arc::new(Mutex::new(reply()));
        let (core, called, go) = core_answering(&dir, answer_now.clone());
        in_library(&core, |s| { ready(s, "first", None); ready_page(s, "other", OTHER, None); });
        if stored_before {
            std::thread::scope(|scope| {
                let first = scope.spawn(|| request(&core, OTHER, "make"));
                called.recv().unwrap();
                go.send(()).unwrap();
                first.join().unwrap().unwrap();
            });
        }
        let (queued, after) = std::thread::scope(|scope| {
            let core = &core;
            let running = scope.spawn(move || request(core, PAGE, "make"));
            called.recv().unwrap();
            let (reached, waiting) = mpsc::channel();
            at("queued", core, move || reached.send(()).unwrap());
            let behind = scope.spawn(move || request(core, page, action));
            waiting.recv().unwrap();
            let queued = request(core, page, "get");
            go.send(()).unwrap();
            running.join().unwrap().unwrap();
            if page == OTHER {
                *answer_now.lock().unwrap() = answer;
                called.recv().unwrap();
                go.send(()).unwrap();
            }
            let _ = behind.join().unwrap();
            (queued, request(core, page, "get").unwrap())
        });
        assert_eq!(queued.as_ref().map(|q| q["pending"].clone()).map_err(|e| e.error.clone()), Ok(json!(true)), "{name}: queued");
        assert!(after.get("pending").is_none(), "{name}: after");
        assert_eq!(after["summary"]["gist"].as_str(), after_gist, "{name}: after");
        assert_eq!(after["failure"].as_str().is_some_and(|f| f.contains("규칙에 맞지 않아")), failed, "{name}: failure");
        drop(core);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// The property: a press spends only on the entry it was made on. While it
/// waits behind another video's call, whatever replaces that entry, even
/// ready again under the same key and job id, is not sent to Gemini and
/// gets neither a summary nor a charge.
#[test]
fn a_queued_press_spends_only_on_the_entry_it_was_made_on() {
    const OTHER: &str = "https://lectures.example.org/summary-flow-other";
    let other = || video_key(&url::Url::parse(OTHER).unwrap());
    fn delete(s: &Store) {
        let key = video_key(&url::Url::parse(OTHER).unwrap());
        s.begin_delete(&key, "token-b").unwrap();
        s.finish_delete(&key, None).unwrap();
    }
    let steps: [(&str, fn(&Store), Result<(), &str>); 5] = [
        ("nothing happened", |_| {}, Ok(())),
        ("the video was deleted", delete, Err("video_deleted")),
        ("the video is being deleted", |s| s.begin_delete(&video_key(&url::Url::parse(OTHER).unwrap()), "token-b").unwrap(), Err("video_deleted")),
        ("the video was deleted and translated again", |s| { delete(s); ready_page(s, "other-again", OTHER, Some("token-b")); }, Err("video_replaced")),
        ("the video was deleted and came back under the same job id", |s| { delete(s); ready_page(s, "other", OTHER, Some("token-b")); }, Err("video_replaced")),
    ];
    for (name, step, outcome) in steps {
        let dir = temp();
        let (core, called, go) = core(&dir);
        in_library(&core, |s| { ready(s, "first", None); ready_page(s, "other", OTHER, None); });
        let request = |core: &Core, key: String, action: &str| library_request(core, Some(generation(core)), "video_summary",
            &json!({"video_key": key, "summary_action": action}));
        let result = std::thread::scope(|scope| {
            let core = &core;
            let running = scope.spawn(move || request(core, key(), "make"));
            called.recv().unwrap();
            let (reached, waiting) = mpsc::channel();
            at("queued", core, move || reached.send(()).unwrap());
            let behind = scope.spawn(move || request(core, other(), "make"));
            waiting.recv().unwrap();
            in_library(core, step);
            go.send(()).unwrap();
            running.join().unwrap().unwrap();
            if outcome.is_ok() { called.recv().unwrap(); go.send(()).unwrap(); }
            behind.join().unwrap()
        });
        assert_eq!(result.as_ref().map(|_| ()).map_err(|e| e.error.as_str()), outcome, "{name}");
        assert!(called.try_recv().is_err(), "{name}: no other Gemini call");
        let row = in_library(&core, |s| s.summary(&other()).unwrap());
        assert_eq!(row.map(|r| (r.body.is_some(), r.cost_krw > 0.0)), outcome.is_ok().then_some((true, true)), "{name}: the entry there now");
        drop(core);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// The property: a summary is checked against the language of the transcript
/// it summarizes. A re-make that recorded another language on the video and
/// then failed leaves the selected saved version to be summarized, and that
/// version's language holds the reply: a next topic in the re-make's script
/// is refused, whatever language the re-make recorded, or none.
#[test]
fn a_failed_re_make_lends_its_language_to_no_summary() {
    use crate::sync::{Change, Cue as Saved, Deletions, Translation, Video};
    for (recorded, item) in [(Some("ja"), "ベイズ推定"), (Some("ko"), "확률론"), (Some("ru"), "теория вероятностей"), (None, "ベイズ推定")] {
        let dir = temp();
        let answer = Arc::new(Mutex::new(reply()));
        let (core, called, go) = core_answering(&dir, answer.clone());
        let video = Video { key: key(), page_url: PAGE.into(), title: "Attention".into() };
        let cues = vec![Saved { start_ms: 0, end_ms: 900, source_text: "Attention is a weighted sum.".into(), text_ko: "어텐션은 가중합입니다.".into() }];
        let scope = core.account.capture_library().unwrap();
        let store = core.account.with_library(&scope, |journal| {
            let journal = journal.expect("an account library");
            journal.write(Change::Translation { translation: Translation { video, duration_ms: 60_000, source_language: Some("en".into()), terms: vec![], model: "m".into(), cost_krw: 1.0, cues } }, Deletions::new())?;
            journal.library()
        }).unwrap();
        store.begin_remake("remake-a", &key(), "audio.wav", "m").unwrap();
        if let Some(language) = recorded { store.set_language(&key(), language).unwrap(); }
        store.fail("remake-a", "stopped").unwrap();
        assert!(summarize(&core, &called, &go, |_| {}).is_ok(), "{recorded:?}: the base reply");
        answer.lock().unwrap()["next_topics"] = json!([item]);
        assert_eq!(press(&core, "again", &called, &go, |_| {}).unwrap_err(), "summary_invalid", "{recorded:?}");
        drop(store);
        drop(core);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn signing_out_during_the_call_keeps_the_reply_and_its_usage_in_the_account_that_paid() {
    let dir = temp();
    let (core, called, go) = core(&dir);
    in_library(&core, |s| ready(s, "first", None));
    let got = summarize(&core, &called, &go, |core| { core.account.sign_out(generation(core)).unwrap(); });
    // The screen that asked belongs to the retired account state.
    assert_eq!(got.unwrap_err(), "account_changed");
    // The signed-out library never saw the video or its summary.
    assert!(core.store.summary(&key()).unwrap().is_none());
    core.account.sign_in(generation(&core)).unwrap();
    let back = generation(&core);
    let shown = library_request(&core, Some(back), "video_summary", &json!({"video_key": key(), "summary_action": "get"})).unwrap();
    assert_eq!(shown["summary"]["gist"], "어텐션을 설명합니다.");
    let job = library_request(&core, Some(back), "get_job", &json!({"page_url": PAGE})).unwrap();
    assert_eq!(job["summary_cost_krw"], crate::gemini::cost_krw(1000, 1));
    drop(core);
    let _ = std::fs::remove_dir_all(&dir);
}
