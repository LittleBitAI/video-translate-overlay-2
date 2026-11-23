//! The harness itself: a failing test leaves what it saw behind.

use std::sync::{Arc, Mutex};
use vto_contract_tests::*;

/// The property: an app whose test panics keeps its data folder and its
/// host's log for reading afterwards; one whose test passes removes them.
#[test]
fn a_failing_test_keeps_its_app_data_and_a_passing_one_removes_it() {
    for (name, fails, kept) in [("a failing test", true, true), ("a passing test", false, false)] {
        let seen = Arc::new(Mutex::new(None));
        let dir = seen.clone();
        let outcome = std::panic::catch_unwind(move || {
            let mut app = App::new(Options::default());
            *dir.lock().unwrap() = Some(app.data_dir());
            app.launch();
            assert!(!fails, "the test fails on purpose");
        });
        assert_eq!(outcome.is_err(), fails, "{name}");
        let data = seen.lock().unwrap().clone().expect("the app was made");
        let log = data.parent().unwrap().join("host-stderr.log");
        assert_eq!((data.exists(), log.exists()), (kept, kept), "{name}: {data:?}");
        if kept { std::fs::remove_dir_all(data.parent().unwrap()).unwrap(); }
    }
}
