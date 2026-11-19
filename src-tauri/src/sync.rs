//! Account-scoped, typed replication storage. No sockets, credentials, or Tauri commands.

use crate::{db::Db, p};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub const MAX_PAGE_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_PAGE_RECORDS: usize = 64;
const MAX_RECORD_BYTES: usize = MAX_PAGE_BYTES / 2;
pub(crate) const MAX_DURATION_MS: i64 = 6 * 60 * 60 * 1000;
type Result<T> = std::result::Result<T, String>;
pub type Deletions = BTreeMap<String, i64>;

#[path = "sync_local.rs"]
pub(crate) mod local;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct Stamp {
    pub clock: i64,
    pub replica: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Video {
    pub key: String,
    pub page_url: String,
    pub title: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Cue {
    pub start_ms: i64,
    pub end_ms: i64,
    pub source_text: String,
    pub text_ko: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Term {
    pub source: String,
    pub target: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Translation {
    pub video: Video,
    pub duration_ms: i64,
    pub source_language: Option<String>,
    pub terms: Vec<Term>,
    pub model: String,
    pub cost_krw: f64,
    pub cues: Vec<Cue>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Change {
    Video {
        video: Video,
    },
    Translation {
        translation: Translation,
    },
    DeleteVideo {
        key: String,
    },
    PlaylistName {
        id: String,
        name: String,
    },
    PlaylistOrder {
        id: String,
        position: i64,
    },
    DeletePlaylist {
        id: String,
    },
    EntryPresence {
        id: String,
        key: String,
        present: bool,
    },
    EntryOrder {
        id: String,
        key: String,
        position: i64,
    },
    Playback {
        key: String,
        position_ms: i64,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub stamp: Stamp,
    pub seen_deletions: Deletions,
    pub change: Change,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TransferRecord {
    pub sequence: i64,
    pub record: Record,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Page {
    pub account: String,
    pub replica: String,
    pub after: i64,
    pub through: i64,
    pub records: Vec<TransferRecord>,
}

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn hex(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn text(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max && !value.contains('\0')
}

impl Video {
    fn valid(&self) -> bool {
        let Ok(url) = url::Url::parse(&self.page_url) else {
            return false;
        };
        self.page_url.len() <= 8192
            && text(&self.title, 2048)
            && matches!(url.scheme(), "http" | "https")
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && crate::api::video_key(&url) == self.key
            && identifier(&self.key)
    }
}

impl Change {
    fn video_key(&self) -> Option<&str> {
        match self {
            Self::Video { video } => Some(&video.key),
            Self::Translation { translation } => Some(&translation.video.key),
            Self::DeleteVideo { key }
            | Self::EntryPresence { key, .. }
            | Self::EntryOrder { key, .. }
            | Self::Playback { key, .. } => Some(key),
            _ => None,
        }
    }

    fn playlist(&self) -> Option<&str> {
        match self {
            Self::PlaylistName { id, .. }
            | Self::PlaylistOrder { id, .. }
            | Self::DeletePlaylist { id }
            | Self::EntryPresence { id, .. }
            | Self::EntryOrder { id, .. } => Some(id),
            _ => None,
        }
    }

    fn valid(&self) -> bool {
        if self.video_key().is_some_and(|s| !identifier(s))
            || self.playlist().is_some_and(|s| !identifier(s))
        {
            return false;
        }
        match self {
            Self::Video { video } => video.valid(),
            Self::Translation { translation: t } => {
                if !t.video.valid()
                    || !(1..=MAX_DURATION_MS).contains(&t.duration_ms)
                    || !t.cost_krw.is_finite()
                    || !(0.0..=1_000_000_000.0).contains(&t.cost_krw)
                    || !text(&t.model, 128)
                    || t.source_language.as_ref().is_some_and(|v| !text(v, 32))
                    || t.terms.len() > 10_000
                    || t.terms
                        .iter()
                        .any(|term| !text(&term.source, 4096) || !text(&term.target, 4096))
                    || t.cues.is_empty()
                    || t.cues.len() > 100_000
                {
                    return false;
                }
                let mut end = 0;
                t.cues.iter().all(|cue| {
                    let valid = cue.start_ms >= end
                        && cue.end_ms > cue.start_ms
                        && cue.end_ms <= t.duration_ms
                        && text(&cue.source_text, 16_384)
                        && text(&cue.text_ko, 16_384);
                    end = cue.end_ms;
                    valid
                })
            }
            Self::PlaylistName { name, .. } => text(name, 800) && name.chars().count() <= 200,
            Self::PlaylistOrder { position, .. } | Self::EntryOrder { position, .. } => {
                (0..=1_000_000).contains(position)
            }
            Self::Playback { position_ms, .. } => (0..=MAX_DURATION_MS).contains(position_ms),
            _ => true,
        }
    }
}

impl Record {
    pub(crate) fn identity(&self) -> String {
        match &self.change {
            Change::Video { video } => format!("v/{}", video.key),
            Change::Translation { translation } => format!(
                "t/{}/{}",
                translation.video.key,
                digest(&serde_json::to_vec(translation).unwrap())
            ),
            Change::DeleteVideo { key } => format!("d/{key}/{}", self.stamp.replica),
            Change::PlaylistName { id, .. } => format!("pn/{id}"),
            Change::PlaylistOrder { id, .. } => format!("po/{id}"),
            Change::DeletePlaylist { id } => format!("pd/{id}"),
            Change::EntryPresence { id, key, .. } => format!("ep/{id}/{key}"),
            Change::EntryOrder { id, key, .. } => format!("eo/{id}/{key}"),
            Change::Playback { key, .. } => format!("p/{key}"),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if !hex(&self.stamp.replica, 32)
            || self.stamp.clock <= 0
            || self.stamp.clock == i64::MAX
            || self.seen_deletions.len() > 64
            || self
                .seen_deletions
                .iter()
                .any(|(id, n)| !hex(id, 32) || *n <= 0 || *n >= self.stamp.clock)
            || !self.change.valid()
            || (self.change.video_key().is_none() && !self.seen_deletions.is_empty())
            || serde_json::to_vec(self)
                .map_err(|_| "invalid record")?
                .len()
                > MAX_RECORD_BYTES
        {
            return Err("invalid sync record".into());
        }
        Ok(())
    }
}

impl Page {
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_PAGE_BYTES {
            return Err("sync page too large".into());
        }
        let page: Self = serde_json::from_slice(bytes).map_err(|_| "invalid sync page")?;
        page.validate()?;
        Ok(page)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        serde_json::to_vec(self).map_err(|_| "invalid sync page".into())
    }

    fn validate(&self) -> Result<()> {
        if !hex(&self.account, 64)
            || !hex(&self.replica, 32)
            || self.after < 0
            || self.through < self.after
            || self.through == i64::MAX
            || self.records.len() > MAX_PAGE_RECORDS
        {
            return Err("invalid sync page".into());
        }
        let mut sequence = self.after;
        for item in &self.records {
            item.record.validate()?;
            if item.sequence <= sequence || item.sequence > self.through {
                return Err("invalid sync sequence".into());
            }
            sequence = item.sequence;
        }
        if serde_json::to_vec(self)
            .map_err(|_| "invalid sync page")?
            .len()
            > MAX_PAGE_BYTES
        {
            return Err("sync page too large".into());
        }
        Ok(())
    }
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS sync_meta (id INTEGER PRIMARY KEY CHECK(id=1), replica TEXT NOT NULL, clock INTEGER NOT NULL, sequence INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS sync_record (identity TEXT PRIMARY KEY, video_key TEXT, playlist_id TEXT, clock INTEGER NOT NULL, replica TEXT NOT NULL, sequence INTEGER NOT NULL UNIQUE, body TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS sync_video ON sync_record(video_key);
CREATE INDEX IF NOT EXISTS sync_playlist ON sync_record(playlist_id);
CREATE TABLE IF NOT EXISTS sync_seen (replica TEXT NOT NULL, clock INTEGER NOT NULL, digest TEXT NOT NULL, PRIMARY KEY(replica,clock));
CREATE TABLE IF NOT EXISTS sync_cursor (replica TEXT PRIMARY KEY, sequence INTEGER NOT NULL)
";

/// Each operation owns its connection and transaction; the session owns this journal.
/// The native session supplies a verified Google subject; this type never authenticates it.
pub struct Journal {
    path: PathBuf,
    account: String,
    replica: String,
    // Keep WAL open between operations; drop closes it with the owning journal.
    _wal: Db,
}

impl Journal {
    pub fn open(root: &Path, identity: &crate::google_auth::VerifiedIdentity) -> Result<Self> {
        Self::open_subject(root,identity.subject())
    }

    pub(crate) fn for_device(root:&Path,identity:&crate::account::DeviceIdentity)->Result<Self> {
        Self::open_subject(root,identity.subject())
    }

    fn open_subject(root:&Path,google_subject:&str)->Result<Self> {
        if !text(google_subject, 255) || !google_subject.is_ascii() {
            return Err("invalid Google subject".into());
        }
        let account = digest(format!("https://accounts.google.com\0{google_subject}").as_bytes());
        let dir = root.join("sync").join(&account);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join("sync.sqlite3");
        let db = Db::open(&path)?;
        db.exec("PRAGMA journal_mode=WAL")?;
        db.exec("BEGIN IMMEDIATE")?;
        db.exec(SCHEMA)?;
        db.exec(crate::store::SCHEMA)?;
        // Projection writes these columns before the library is opened as a store.
        crate::store::add_columns(&db)?;
        db.exec(library::SCHEMA)?;
        db.exec(local::SCHEMA)?;
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).map_err(|e| e.to_string())?;
        let replica: String = random.iter().map(|b| format!("{b:02x}")).collect();
        db.execute(
            "INSERT OR IGNORE INTO sync_meta VALUES (1,?,0,0)",
            p![replica],
        )?;
        let replica = db
            .query_one("SELECT replica FROM sync_meta WHERE id=1", &[])?
            .ok_or("missing sync metadata")?[0]
            .text();
        db.exec("COMMIT")?;
        Ok(Self {
            path,
            account,
            replica,
            _wal: db,
        })
    }

    pub fn account(&self) -> &str {
        &self.account
    }
    pub(crate) fn peer_expiry(&self,certificate:&str)->Result<Option<i64>> {
        if !hex(certificate,64) { return Err("invalid peer certificate".into()); }
        Ok(Db::open(&self.path)?.query_one("SELECT expires_at FROM sync_peer WHERE certificate=?",p![certificate])?.map(|r|r[0].int()))
    }
    pub(crate) fn trust_peer(&self,certificate:&str,expires_at:i64)->Result<()> {
        if !hex(certificate,64) || expires_at<=0 { return Err("invalid peer credential".into()); }
        self.transaction(|db| {
            db.execute("INSERT INTO sync_peer VALUES (?,?) ON CONFLICT(certificate) DO UPDATE SET expires_at=MAX(expires_at,excluded.expires_at)",p![certificate,expires_at])?;
            Ok(())
        })
    }
    pub fn replica(&self) -> &str {
        &self.replica
    }

    fn transaction<T>(&self, f: impl FnOnce(&Db) -> Result<T>) -> Result<T> {
        let db = Db::open(&self.path)?;
        db.exec("BEGIN IMMEDIATE")?;
        let result = f(&db);
        db.exec(if result.is_ok() { "COMMIT" } else { "ROLLBACK" })?;
        result
    }

    /// Consent captures this context before a local restore; a worker must not reread it later.
    pub fn deletions(&self, key: &str) -> Result<Deletions> {
        deletion_marks(&Db::open(&self.path)?, key)
    }

    pub fn write(&self, change: Change, seen_deletions: Deletions) -> Result<Record> {
        self.transaction(|db| {
            let clock = db
                .query_one("SELECT clock FROM sync_meta WHERE id=1", &[])?
                .ok_or("missing sync metadata")?[0]
                .int()
                .checked_add(1)
                .ok_or("sync clock exhausted")?;
            let record = Record {
                stamp: Stamp {
                    clock,
                    replica: self.replica.clone(),
                },
                change,
                seen_deletions,
            };
            record.validate()?;
            if suppressed(db, &record)? {
                return Err("sync item was deleted; fresh confirmation is required".into());
            }
            merge(db, &record)?;
            library::project(db, 64)?;
            Ok(record)
        })
    }

    pub fn cursor(&self, replica: &str) -> Result<i64> {
        if !hex(replica, 32) {
            return Err("invalid sync replica".into());
        }
        cursor(&Db::open(&self.path)?, replica)
    }

    pub fn page(&self, after: i64) -> Result<Page> {
        let db = Db::open(&self.path)?;
        db.exec("BEGIN")?;
        let maximum = db
            .query_one("SELECT sequence FROM sync_meta WHERE id=1", &[])?
            .ok_or("missing sync metadata")?[0]
            .int();
        if after < 0 || after > maximum {
            return Err("invalid sync cursor".into());
        }
        let mut page = Page {
            account: self.account.clone(),
            replica: self.replica.clone(),
            after,
            through: after,
            records: Vec::new(),
        };
        let mut size = 1024;
        loop {
            let next = db.query_one(
                "SELECT sequence,body FROM sync_record WHERE sequence>? ORDER BY sequence LIMIT 1",
                p![page.through],
            )?;
            let Some(row) = next else {
                page.through = maximum;
                break;
            };
            let body = row[1].text();
            if page.records.len() == MAX_PAGE_RECORDS || size + body.len() + 128 > MAX_PAGE_BYTES {
                break;
            }
            size += body.len() + 128;
            let record = serde_json::from_str(&body).map_err(|_| "invalid stored sync record")?;
            page.through = row[0].int();
            page.records.push(TransferRecord {
                sequence: page.through,
                record,
            });
        }
        page.validate()?;
        db.exec("COMMIT")?;
        Ok(page)
    }

    /// Called only after native peer authentication, with the expected replica from that connection.
    pub fn receive(&self, expected_replica: &str, page: &Page) -> Result<bool> {
        page.validate()?;
        if page.account != self.account
            || page.replica != expected_replica
            || page.replica == self.replica
        {
            return Err("sync account or peer mismatch".into());
        }
        self.transaction(|db| {
            let saved = cursor(db, &page.replica)?;
            if page.through <= saved { return Ok(false); }
            if page.after != saved { return Err("sync cursor gap".into()); }
            let mut changed = false;
            for item in &page.records { changed |= merge(db, &item.record)?; }
            library::project(db, 64)?;
            db.execute("INSERT INTO sync_cursor VALUES (?,?) ON CONFLICT(replica) DO UPDATE SET sequence=excluded.sequence", p![&page.replica, page.through])?;
            Ok(changed)
        })
    }

    /// Current records, including deletion knowledge and every distinct completed translation.
    pub fn records(&self) -> Result<Vec<Record>> {
        let db = Db::open(&self.path)?;
        db.exec("BEGIN")?;
        let mut records = Vec::new();
        let mut after = 0;
        while let Some(row) = db.query_one(
            "SELECT sequence,body FROM sync_record WHERE sequence>? ORDER BY sequence LIMIT 1",
            p![after],
        )? {
            after = row[0].int();
            records.push(
                serde_json::from_str(&row[1].text()).map_err(|_| "invalid stored sync record")?,
            );
        }
        db.exec("COMMIT")?;
        Ok(records)
    }
}

fn cursor(db: &Db, replica: &str) -> Result<i64> {
    Ok(db
        .query_one(
            "SELECT sequence FROM sync_cursor WHERE replica=?",
            p![replica],
        )?
        .map_or(0, |r| r[0].int()))
}

fn deletion_marks(db: &Db, key: &str) -> Result<Deletions> {
    Ok(db
        .query(
            "SELECT replica,clock FROM sync_record WHERE video_key=? AND identity LIKE 'd/%'",
            p![key],
        )?
        .iter()
        .map(|r| (r[0].text(), r[1].int()))
        .collect())
}

fn suppressed(db: &Db, record: &Record) -> Result<bool> {
    if matches!(
        record.change,
        Change::DeleteVideo { .. } | Change::DeletePlaylist { .. }
    ) {
        return Ok(false);
    }
    if let Some(id) = record.change.playlist() {
        if db
            .query_one(
                "SELECT identity FROM sync_record WHERE identity=?",
                p![format!("pd/{id}")],
            )?
            .is_some()
        {
            return Ok(true);
        }
    }
    if let Some(key) = record.change.video_key() {
        for (replica, clock) in deletion_marks(db, key)? {
            if record.seen_deletions.get(&replica).copied().unwrap_or(0) < clock {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn merge(db: &Db, record: &Record) -> Result<bool> {
    let body = serde_json::to_string(record).map_err(|_| "invalid sync record")?;
    let hash = digest(body.as_bytes());
    if let Some(row) = db.query_one(
        "SELECT digest FROM sync_seen WHERE replica=? AND clock=?",
        p![&record.stamp.replica, record.stamp.clock],
    )? {
        if row[0].text() != hash {
            return Err("conflicting sync record identity".into());
        }
        return Ok(false);
    }
    db.execute(
        "INSERT INTO sync_seen VALUES (?,?,?)",
        p![&record.stamp.replica, record.stamp.clock, hash],
    )?;
    db.execute(
        "UPDATE sync_meta SET clock=MAX(clock,?) WHERE id=1",
        p![record.stamp.clock],
    )?;
    let identity = record.identity();
    let existing = db
        .query_one(
            "SELECT body FROM sync_record WHERE identity=?",
            p![&identity],
        )?
        .map(|row| {
            serde_json::from_str::<Record>(&row[0].text()).map_err(|_| "invalid stored sync record")
        })
        .transpose()?;
    if let Some(old) = &existing {
        if old.stamp == record.stamp && old != record {
            return Err("conflicting sync record identity".into());
        }
        if old.stamp >= record.stamp {
            return Ok(false);
        }
    }
    if suppressed(db, record)? {
        return Ok(false);
    }
    db.execute(
        "UPDATE sync_meta SET sequence=sequence+1 WHERE id=1 AND sequence<?",
        p![i64::MAX - 1],
    )?;
    let sequence = db
        .query_one("SELECT sequence FROM sync_meta WHERE id=1", &[])?
        .ok_or("missing sync metadata")?[0]
        .int();
    if sequence >= i64::MAX - 1 {
        return Err("sync sequence exhausted".into());
    }
    db.execute("INSERT INTO sync_record (identity,video_key,playlist_id,clock,replica,sequence,body) VALUES (?,?,?,?,?,?,?)
        ON CONFLICT(identity) DO UPDATE SET clock=excluded.clock,replica=excluded.replica,sequence=excluded.sequence,body=excluded.body",
        p![identity, record.change.video_key(), record.change.playlist(), record.stamp.clock, &record.stamp.replica, sequence, body])?;
    library::dirty(db, record)?;
    if matches!(
        record.change,
        Change::DeleteVideo { .. } | Change::DeletePlaylist { .. }
    ) {
        let candidates = if let Change::DeleteVideo { key } = &record.change {
            db.query(
                "SELECT identity FROM sync_record WHERE video_key=?",
                p![key],
            )?
        } else {
            db.query(
                "SELECT identity FROM sync_record WHERE playlist_id=?",
                p![record.change.playlist()],
            )?
        };
        for row in candidates {
            let id = row[0].text();
            let body = db
                .query_one("SELECT body FROM sync_record WHERE identity=?", p![&id])?
                .ok_or("missing sync record")?[0]
                .text();
            let stored: Record =
                serde_json::from_str(&body).map_err(|_| "invalid stored sync record")?;
            if suppressed(db, &stored)? {
                db.execute("DELETE FROM sync_record WHERE identity=?", p![id])?;
            }
        }
    }
    Ok(true)
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod tests;

#[path = "sync_store.rs"]
mod library;
