//! A job is read (the overlay's polling, R11, R24) even while the worker holds
//! a write open: the database runs in WAL mode.

use super::*;

#[test]
fn reads_do_not_wait_for_an_open_write() {
    let dir = std::env::temp_dir().join(format!("vto-wal-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("app.sqlite3");
    let worker = Store::open(&path).unwrap();
    worker.create_job("j", "v", "https://a.example/x", "other", "T", 1000, None, "m").unwrap();
    let mode = worker.db.query_one("PRAGMA journal_mode", &[]).unwrap().unwrap();
    assert_eq!(mode[0].text(), "wal");

    let commands = Store::open(&path).unwrap();
    worker.db.exec("BEGIN IMMEDIATE").unwrap();
    worker.raise_percent("j", 40).unwrap();
    let started = std::time::Instant::now();
    let seen = commands.job("j").unwrap().unwrap();
    assert!(started.elapsed() < std::time::Duration::from_secs(1), "the read waited for the write");
    assert_eq!(seen.percent, 0, "an uncommitted write is not seen");
    worker.db.exec("COMMIT").unwrap();
    assert_eq!(commands.job("j").unwrap().unwrap().percent, 40);
    drop((worker, commands));
    let _ = std::fs::remove_dir_all(&dir);
}
