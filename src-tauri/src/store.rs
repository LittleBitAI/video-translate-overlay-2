//! The app's SQLite file: videos, jobs, cues and the single settings row.

use crate::db::{Db, Row, Val};
use crate::p;
use std::path::Path;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS playlist (
  id TEXT PRIMARY KEY, name TEXT NOT NULL, position INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS playlist_entry (
  playlist_id TEXT NOT NULL REFERENCES playlist(id),
  video_key TEXT NOT NULL REFERENCES video(video_key), position INTEGER NOT NULL,
  PRIMARY KEY (playlist_id, video_key)
);
CREATE TABLE IF NOT EXISTS deleted_video (
  video_key TEXT PRIMARY KEY, token TEXT NOT NULL, complete INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS video (
  video_key TEXT PRIMARY KEY,
  page_url TEXT NOT NULL,
  site TEXT NOT NULL,
  title TEXT NOT NULL,
  duration_ms INTEGER NOT NULL,
  source_language TEXT,
  terms TEXT,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS job (
  id TEXT PRIMARY KEY,
  video_key TEXT NOT NULL REFERENCES video(video_key),
  media_url TEXT,
  stage TEXT NOT NULL,
  percent INTEGER NOT NULL DEFAULT 0,
  failure_reason TEXT,
  audio_path TEXT,
  written_regions INTEGER NOT NULL DEFAULT 0,
  translation_model TEXT NOT NULL,
  input_amount INTEGER NOT NULL DEFAULT 0,
  output_amount INTEGER NOT NULL DEFAULT 0,
  cost_krw REAL NOT NULL DEFAULT 0,
  notified_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS job_by_video ON job(video_key, created_at);
CREATE UNIQUE INDEX IF NOT EXISTS one_running_job ON job(stage IN ('downloading','writing','translating'))
  WHERE stage IN ('downloading','writing','translating');
CREATE TABLE IF NOT EXISTS cue (
  video_key TEXT NOT NULL REFERENCES video(video_key),
  seq INTEGER NOT NULL,
  start_ms INTEGER NOT NULL,
  end_ms INTEGER NOT NULL,
  source_text TEXT NOT NULL,
  text_ko TEXT,
  PRIMARY KEY (video_key, seq)
);
CREATE TABLE IF NOT EXISTS setting (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  youtube_gate_seen INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS retranslation_cue (
  job_id TEXT NOT NULL REFERENCES job(id),
  seq INTEGER NOT NULL,
  text_ko TEXT,
  PRIMARY KEY (job_id, seq)
);
INSERT OR IGNORE INTO setting (id) VALUES (1);
-- The page the in-app browser was last sent to (P9).
CREATE TABLE IF NOT EXISTS browser (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  url TEXT NOT NULL
)
";

const NOW: &str = "strftime('%Y-%m-%dT%H:%M:%fZ','now')";

pub const ACTIVE: [&str; 3] = ["downloading", "writing", "translating"];

#[derive(Debug, Clone)]
pub struct Job {
    pub id: String,
    pub video_key: String,
    pub media_url: Option<String>,
    pub stage: String,
    pub percent: i64,
    pub failure_reason: Option<String>,
    pub audio_path: Option<String>,
    pub written_regions: i64,
    pub translation_model: String,
    pub input_amount: i64,
    pub output_amount: i64,
    pub cost_krw: f64,
    pub notified: bool,
}

const JOB_COLS: &str = "id, video_key, media_url, stage, percent, failure_reason, audio_path, written_regions, \
    translation_model, input_amount, output_amount, cost_krw, notified_at";

fn job(r: &Row) -> Job {
    Job {
        id: r[0].text(),
        video_key: r[1].text(),
        media_url: r[2].opt_text(),
        stage: r[3].text(),
        percent: r[4].int(),
        failure_reason: r[5].opt_text(),
        audio_path: r[6].opt_text(),
        written_regions: r[7].int(),
        translation_model: r[8].text(),
        input_amount: r[9].int(),
        output_amount: r[10].int(),
        cost_krw: r[11].real(),
        notified: r[12] != Val::Null,
    }
}

#[derive(Debug, Clone)]
pub struct Video {
    pub video_key: String,
    pub page_url: String,
    pub title: String,
    pub duration_ms: i64,
    pub source_language: Option<String>,
    pub terms: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cue {
    pub seq: i64,
    pub start_ms: i64,
    pub end_ms: i64,
    pub source_text: String,
    pub text_ko: Option<String>,
}

pub struct Store {
    db: Db,
    path: std::path::PathBuf,
}

type R<T> = Result<T, String>;

impl Store {
    pub fn open(path: &Path) -> R<Store> {
        let db = Db::open(path)?;
        // Readers (the overlay polling a job) never wait on the worker's writes,
        // even when a slow disk stalls a commit. The mode stays with the file.
        db.exec("PRAGMA journal_mode=WAL")?;
        db.exec(SCHEMA)?;
        Ok(Store { db, path: path.to_path_buf() })
    }

    // ------------------------------------------------------------ videos

    pub fn video(&self, key: &str) -> R<Option<Video>> {
        Ok(self
            .db
            .query_one("SELECT video_key, page_url, title, duration_ms, source_language, terms FROM video WHERE video_key=?", p![key])?
            .map(|r| Video {
                video_key: r[0].text(),
                page_url: r[1].text(),
                title: r[2].text(),
                duration_ms: r[3].int(),
                source_language: r[4].opt_text(),
                terms: r[5].opt_text(),
            }))
    }

    pub fn set_terms(&self, key: &str, terms: &str) -> R<()> {
        self.db.execute("UPDATE video SET terms=? WHERE video_key=?", p![terms, key])?;
        Ok(())
    }

    pub fn set_language(&self, key: &str, lang: &str) -> R<()> {
        self.db.execute("UPDATE video SET source_language=? WHERE video_key=?", p![lang, key])?;
        Ok(())
    }

    // ------------------------------------------------------------ jobs

    /// Records the video, forgets an unfinished earlier attempt's cues and
    /// glossary, and inserts the job, in one transaction on its own
    /// connection. The `one_running_job` index refuses a second running job
    /// even if a caller skipped the checks.
    #[allow(clippy::too_many_arguments)]
    pub fn create_job(
        &self,
        id: &str,
        key: &str,
        page_url: &str,
        site: &str,
        title: &str,
        duration_ms: i64,
        media_url: Option<&str>,
        model: &str,
    ) -> R<()> {
        let db = Db::open(&self.path)?;
        db.exec("BEGIN IMMEDIATE")?;
        let r = (|| {
            db.execute(
                &format!(
                    "INSERT INTO video (video_key, page_url, site, title, duration_ms, created_at) VALUES (?,?,?,?,?,{NOW})
                     ON CONFLICT(video_key) DO UPDATE SET page_url=excluded.page_url, title=excluded.title, duration_ms=excluded.duration_ms"
                ),
                p![key, page_url, site, title, duration_ms],
            )?;
            db.execute("DELETE FROM cue WHERE video_key=?", p![key])?;
            db.execute("UPDATE video SET terms=NULL, source_language=NULL WHERE video_key=?", p![key])?;
            db.execute(
                &format!(
                    "INSERT INTO job (id, video_key, media_url, stage, translation_model, created_at, updated_at)
                     VALUES (?,?,?,'downloading',?,{NOW},{NOW})"
                ),
                p![id, key, media_url, model],
            )?;
            db.execute("DELETE FROM deleted_video WHERE video_key=?", p![key])?;
            Ok(())
        })();
        db.exec(if r.is_ok() { "COMMIT" } else { "ROLLBACK" })?;
        r
    }

    /// Retranslates saved speech without downloading or changing its source clock.
    pub fn retranslate(&self, id: &str, previous: &Job, model: &str) -> R<()> {
        self.db.exec("BEGIN IMMEDIATE")?;
        let result = (|| {
            self.db.execute(
                &format!("INSERT INTO job (id,video_key,stage,written_regions,translation_model,input_amount,output_amount,cost_krw,created_at,updated_at)
                    VALUES (?,?,'translating',?,?,?,?,?,{NOW},{NOW})"),
                p![id, previous.video_key.as_str(), previous.written_regions, model, previous.input_amount, previous.output_amount, previous.cost_krw],
            )?;
            self.db.execute("INSERT INTO retranslation_cue (job_id, seq) SELECT ?, seq FROM cue WHERE video_key=?", p![id, previous.video_key.as_str()])?;
            Ok(())
        })();
        self.db.exec(if result.is_ok() { "COMMIT" } else { "ROLLBACK" })?;
        result
    }

    pub fn job(&self, id: &str) -> R<Option<Job>> {
        Ok(self.db.query_one(&format!("SELECT {JOB_COLS} FROM job WHERE id=?"), p![id])?.map(|r| job(&r)))
    }

    pub fn latest_job(&self, video_key: &str) -> R<Option<Job>> {
        Ok(self
            .db
            .query_one(&format!("SELECT {JOB_COLS} FROM job WHERE video_key=? ORDER BY created_at DESC, rowid DESC LIMIT 1"), p![video_key])?
            .map(|r| job(&r)))
    }

    pub fn ready_job(&self, video_key: &str) -> R<Option<Job>> {
        Ok(self.db.query_one(
            &format!("SELECT {JOB_COLS} FROM job WHERE video_key=? AND stage='ready' ORDER BY created_at DESC, rowid DESC LIMIT 1"),
            p![video_key],
        )?.map(|r| job(&r)))
    }

    /// The oldest job still in progress, if any.
    pub fn active_job(&self) -> R<Option<Job>> {
        Ok(self
            .db
            .query_one(
                &format!("SELECT {JOB_COLS} FROM job WHERE stage IN ('downloading','writing','translating') ORDER BY created_at, rowid LIMIT 1"),
                &[],
            )?
            .map(|r| job(&r)))
    }

    pub fn set_stage(&self, id: &str, stage: &str) -> R<()> {
        self.db.execute(&format!("UPDATE job SET stage=?, percent=0, updated_at={NOW} WHERE id=?"), p![stage, id])?;
        Ok(())
    }

    /// Raises the percent of the current stage; it never goes back.
    pub fn raise_percent(&self, id: &str, percent: i64) -> R<()> {
        self.db.execute(
            &format!("UPDATE job SET percent=?, updated_at={NOW} WHERE id=? AND percent<?"),
            p![percent.clamp(0, 100), id, percent.clamp(0, 100)],
        )?;
        Ok(())
    }

    pub fn set_audio(&self, id: &str, path: Option<&str>) -> R<()> {
        self.db.execute(&format!("UPDATE job SET audio_path=?, updated_at={NOW} WHERE id=?"), p![path, id])?;
        Ok(())
    }

    pub fn fail(&self, id: &str, reason: &str) -> R<()> {
        self.db.execute(
            &format!("UPDATE job SET stage='failed', failure_reason=?, updated_at={NOW} WHERE id=?"),
            p![reason, id],
        )?;
        Ok(())
    }

    /// Records that the ready notice went out (E2 `notified_at`); the first time stays.
    pub fn mark_notified(&self, id: &str) -> R<()> {
        self.db.execute(&format!("UPDATE job SET notified_at={NOW}, updated_at={NOW} WHERE id=? AND notified_at IS NULL"), p![id])?;
        Ok(())
    }

    pub fn finish(&self, id: &str) -> R<()> {
        self.db.exec("BEGIN IMMEDIATE")?;
        let result = (|| {
            if self.db.query_one("SELECT seq FROM retranslation_cue WHERE job_id=? AND text_ko IS NULL LIMIT 1", p![id])?.is_some() {
                return Err("retranslation incomplete".into());
            }
            self.db.execute(
                "UPDATE cue SET text_ko=(SELECT text_ko FROM retranslation_cue WHERE job_id=? AND seq=cue.seq)
                 WHERE video_key=(SELECT video_key FROM job WHERE id=?) AND seq IN (SELECT seq FROM retranslation_cue WHERE job_id=?)",
                p![id, id, id],
            )?;
            self.db.execute("DELETE FROM retranslation_cue WHERE job_id=?", p![id])?;
            self.db.execute(&format!("UPDATE job SET stage='ready', percent=100, updated_at={NOW} WHERE id=?"), p![id])?;
            Ok(())
        })();
        self.db.exec(if result.is_ok() { "COMMIT" } else { "ROLLBACK" })?;
        result
    }

    /// Ends the writing stage. The audio path stays until the file is gone
    /// (`set_audio(id, None)` after the delete), so a kill or a failed delete
    /// in between leaves it for [`Store::leftover_audio`].
    pub fn end_writing(&self, id: &str) -> R<()> {
        self.db.execute(&format!("UPDATE job SET stage='translating', percent=0, updated_at={NOW} WHERE id=?"), p![id])?;
        Ok(())
    }

    /// `(job id, audio path)` of jobs past writing whose audio file was not
    /// deleted yet: write is over for them, so the file must go (A3).
    pub fn leftover_audio(&self) -> R<Vec<(String, String)>> {
        let rows = self.db.query(
            "SELECT id, audio_path FROM job WHERE audio_path IS NOT NULL AND stage NOT IN ('downloading', 'writing')",
            &[],
        )?;
        Ok(rows.iter().map(|r| (r[0].text(), r[1].text())).collect())
    }

    pub fn add_usage(&self, id: &str, input: i64, output: i64, cost_of: impl Fn(i64, i64) -> f64) -> R<Job> {
        let j = self.job(id)?.ok_or("job vanished")?;
        let (i, o) = (j.input_amount + input, j.output_amount + output);
        self.db.execute(
            &format!("UPDATE job SET input_amount=?, output_amount=?, cost_krw=?, updated_at={NOW} WHERE id=?"),
            p![i, o, cost_of(i, o), id],
        )?;
        self.job(id)?.ok_or_else(|| "job vanished".into())
    }

    // ------------------------------------------------------------ cues

    /// Stores the cues of one written region and moves the resume point past it, atomically.
    pub fn save_region(&self, job_id: &str, video_key: &str, region: i64, cues: &[(i64, i64, String)]) -> R<()> {
        self.db.exec("BEGIN IMMEDIATE")?;
        let r = (|| {
            // seq counts from 0 in start_ms order (E3).
            let next = self.db.query_one("SELECT COALESCE(MAX(seq),-1)+1 FROM cue WHERE video_key=?", p![video_key])?.unwrap()[0].int();
            for (i, (a, b, text)) in cues.iter().enumerate() {
                self.db.execute(
                    "INSERT INTO cue (video_key, seq, start_ms, end_ms, source_text) VALUES (?,?,?,?,?)",
                    p![video_key, next + i as i64, *a, *b, text],
                )?;
            }
            self.db.execute(&format!("UPDATE job SET written_regions=?, updated_at={NOW} WHERE id=?"), p![region + 1, job_id])?;
            Ok(())
        })();
        self.db.exec(if r.is_ok() { "COMMIT" } else { "ROLLBACK" })?;
        r
    }

    pub fn last_cue_end(&self, video_key: &str) -> R<i64> {
        Ok(self.db.query_one("SELECT COALESCE(MAX(end_ms),0) FROM cue WHERE video_key=?", p![video_key])?.unwrap()[0].int())
    }

    pub fn cues(&self, video_key: &str) -> R<Vec<Cue>> {
        Ok(self
            .db
            .query("SELECT seq, start_ms, end_ms, source_text, text_ko FROM cue WHERE video_key=? ORDER BY seq", p![video_key])?
            .iter()
            .map(|r| Cue { seq: r[0].int(), start_ms: r[1].int(), end_ms: r[2].int(), source_text: r[3].text(), text_ko: r[4].opt_text() })
            .collect())
    }

    /// A replacement set stays separate from the published cues until finish.
    pub fn translation_cues(&self, job: &Job) -> R<Vec<Cue>> {
        let mut cues = self.cues(&job.video_key)?;
        let pending = self.db.query("SELECT seq, text_ko FROM retranslation_cue WHERE job_id=? ORDER BY seq", p![job.id.as_str()])?;
        if !pending.is_empty() {
            for (cue, row) in cues.iter_mut().zip(pending) {
                cue.text_ko = row[1].opt_text();
            }
        }
        Ok(cues)
    }

    pub fn set_translations(&self, job_id: &str, ko: &[(i64, String)]) -> R<()> {
        self.db.exec("BEGIN IMMEDIATE")?;
        let r = (|| {
            let pending = self.db.query_one("SELECT seq FROM retranslation_cue WHERE job_id=? LIMIT 1", p![job_id])?.is_some();
            for (seq, t) in ko {
                if pending {
                    self.db.execute("UPDATE retranslation_cue SET text_ko=? WHERE job_id=? AND seq=?", p![t, job_id, *seq])?;
                } else {
                    self.db.execute("UPDATE cue SET text_ko=? WHERE video_key=(SELECT video_key FROM job WHERE id=?) AND seq=?", p![t, job_id, *seq])?;
                }
            }
            Ok(())
        })();
        self.db.exec(if r.is_ok() { "COMMIT" } else { "ROLLBACK" })?;
        r
    }

    // ------------------------------------------------------------ settings

    pub fn youtube_gate_seen(&self) -> R<bool> {
        Ok(self.db.query_one("SELECT youtube_gate_seen FROM setting WHERE id=1", &[])?.is_some_and(|r| r[0].int() != 0))
    }

    pub fn set_youtube_gate_seen(&self, seen: bool) -> R<()> {
        self.db.execute("UPDATE setting SET youtube_gate_seen=? WHERE id=1", p![seen as i64])?;
        Ok(())
    }

    pub fn opened_url(&self) -> R<Option<String>> {
        Ok(self.db.query_one("SELECT url FROM browser WHERE id=1", &[])?.map(|r| r[0].text()))
    }

    pub fn set_opened_url(&self, url: &str) -> R<()> {
        self.db.execute("INSERT INTO browser (id, url) VALUES (1, ?) ON CONFLICT(id) DO UPDATE SET url=excluded.url", p![url])?;
        Ok(())
    }

    pub fn finish_youtube_sign_in(&self, gate: &str, url: &str) -> R<()> {
        self.db.execute("UPDATE browser SET url=? WHERE id=1 AND url=?", p![url, gate])?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "store_wal_tests.rs"]
mod wal_tests;

#[path = "library_store.rs"]
mod library;

#[cfg(test)]
#[path = "library_store_tests.rs"]
mod library_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_late_sign_in_does_not_replace_the_newer_opened_url() {
        let dir = std::env::temp_dir().join(format!("vto-sign-in-return-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let s = Store::open(&dir.join("test.sqlite3")).unwrap();
        s.set_opened_url("old gate").unwrap();
        s.set_opened_url("new video").unwrap();
        s.finish_youtube_sign_in("old gate", "old video").unwrap();
        assert_eq!(s.opened_url().unwrap().as_deref(), Some("new video"));
    }

    #[test]
    fn region_save_is_atomic_and_percent_never_drops() {
        let dir = std::env::temp_dir().join(format!("vto-store-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let s = Store::open(&dir.join("t.sqlite3")).unwrap();
        s.create_job("j", "v", "https://a.example/x", "other", "T", 1000, None, "m").unwrap();
        s.save_region("j", "v", 0, &[(0, 10, "a".into()), (10, 20, "b".into())]).unwrap();
        s.save_region("j", "v", 1, &[(30, 40, "c".into())]).unwrap();
        let seqs: Vec<i64> = s.cues("v").unwrap().iter().map(|c| c.seq).collect();
        assert_eq!(seqs, [0, 1, 2], "seq counts from 0 (E3)");
        assert_eq!(s.job("j").unwrap().unwrap().written_regions, 2);
        s.raise_percent("j", 40).unwrap();
        s.raise_percent("j", 30).unwrap();
        assert_eq!(s.job("j").unwrap().unwrap().percent, 40);

        // A second running job is refused by the database itself.
        assert!(s.create_job("k", "w", "https://a.example/y", "other", "T", 1000, None, "m").is_err());
        assert!(s.job("k").unwrap().is_none(), "the refused job left a row");
        assert_eq!(s.cues("v").unwrap().len(), 3, "the refused job touched another video");

        // The ready notice is recorded once it went out.
        assert!(!s.job("j").unwrap().unwrap().notified);
        s.mark_notified("j").unwrap();
        assert!(s.job("j").unwrap().unwrap().notified);
        drop(s);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
