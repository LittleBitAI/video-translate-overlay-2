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

fn ready(store: &Store, job: &str, token: Option<&str>) {
    let key = key();
    store.create_job_with_consent(job, &key, PAGE, "other", "Attention", 60_000, None, "m", token).unwrap();
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
