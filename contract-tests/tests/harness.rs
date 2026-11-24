//! The harness itself: a failing test leaves what it saw behind.

use std::sync::{Arc, Mutex};
use vto_contract_tests::*;

/// The property: an app whose test panics after its host launched keeps its
/// data folder and its host's log for reading afterwards; one whose test
/// passes removes them. Each case checks that its host answered the launch
/// and that a failure is the deliberate one, not a failed launch.
#[test]
fn a_failing_test_keeps_its_app_data_and_a_passing_one_removes_it() {
    const ON_PURPOSE: &str = "the test fails on purpose";
    for (name, fails, kept) in [("a failing test", true, true), ("a passing test", false, false)] {
        let seen = Arc::new(Mutex::new((None, false)));
        let dir = seen.clone();
        let outcome = std::panic::catch_unwind(move || {
            let mut app = App::new(Options::default());
            dir.lock().unwrap().0 = Some(app.data_dir());
            app.launch();
            dir.lock().unwrap().1 = true;
            assert!(!fails, "{ON_PURPOSE}");
        });
        let why = outcome.err().map(|e| e.downcast_ref::<String>().cloned().unwrap_or_default());
        assert_eq!(why.as_deref(), fails.then_some(ON_PURPOSE), "{name}");
        let (data, launched) = seen.lock().unwrap().clone();
        let data = data.expect("the app was made");
        assert!(launched, "{name}: the host answered its launch");
        let log = data.parent().unwrap().join("host-stderr.log");
        assert_eq!((data.exists(), log.exists()), (kept, kept), "{name}: {data:?}");
        if kept { std::fs::remove_dir_all(data.parent().unwrap()).unwrap(); }
    }
}
