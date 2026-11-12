//! Downloaded audio is deleted once write is over (A3), even when the process
//! was killed right after writing ended, or the delete failed: the path stays
//! recorded until the file is gone, and the worker deletes what is left.

use super::*;

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("vto-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// What a kill between the end of writing and the delete leaves: a job past
/// writing (here also one that failed) whose audio file is still on disk.
#[test]
fn audio_left_after_writing_is_deleted_on_the_next_round() {
    let dir = temp_dir("leftover");
    let s = Store::open(&dir.join("app.sqlite3")).unwrap();
    // One job runs at a time: each is created once the one before has left the running stages.
    let job_with_audio = |id: &str| {
        let audio = dir.join(format!("{id}.wav"));
        std::fs::write(&audio, b"RIFF").unwrap();
        s.create_job(id, &format!("v{id}"), "https://a.example/talk", "other", "Talk", 1000, None, "m").unwrap();
        s.set_audio(id, Some(audio.to_str().unwrap())).unwrap();
        audio
    };
    let failed = job_with_audio("f");
    s.fail("f", "translate_failed").unwrap();
    let translating = job_with_audio("t");
    s.end_writing("t").unwrap();
    assert_eq!(s.leftover_audio().unwrap().len(), 2, "the paths outlive the stage change");

    sweep_audio(&s);
    for (id, audio) in [("f", &failed), ("t", &translating)] {
        assert!(!audio.exists(), "{id}: the audio is still on disk");
        assert_eq!(s.job(id).unwrap().unwrap().audio_path, None, "{id}");
    }

    s.finish("t").unwrap();
    let writing = job_with_audio("w");
    s.set_stage("w", "writing").unwrap();
    sweep_audio(&s);
    assert!(writing.exists(), "a job still writing keeps its audio");
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_delete_keeps_the_path_for_the_next_try() {
    let dir = temp_dir("undeletable");
    let s = Store::open(&dir.join("app.sqlite3")).unwrap();
    // A directory where the file should be: remove_file fails, and not with NotFound.
    let audio = dir.join("j.wav");
    std::fs::create_dir(&audio).unwrap();
    s.create_job("j", "v", "https://a.example/talk", "other", "Talk", 1000, None, "m").unwrap();
    s.set_audio("j", Some(audio.to_str().unwrap())).unwrap();
    s.end_writing("j").unwrap();

    sweep_audio(&s);
    assert_eq!(s.job("j").unwrap().unwrap().audio_path.as_deref(), audio.to_str(), "a path was forgotten while its file remains");

    std::fs::remove_dir(&audio).unwrap();
    sweep_audio(&s); // already gone counts as deleted
    assert_eq!(s.job("j").unwrap().unwrap().audio_path, None);
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}
