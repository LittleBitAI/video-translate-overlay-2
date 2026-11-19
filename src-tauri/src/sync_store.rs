use super::*;
use crate::store::Store;

pub(super) const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS sync_dirty (kind TEXT NOT NULL, key TEXT NOT NULL, PRIMARY KEY(kind,key));
CREATE TABLE IF NOT EXISTS sync_selected_translation (video_key TEXT PRIMARY KEY, identity TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sync_import_seen (digest TEXT PRIMARY KEY)
";

impl Journal {
    pub(crate) fn has_legacy_changes(&self, source: &Store) -> Result<bool> {
        let source = legacy_source(source)?;
        let destination = Db::open(&self.path)?;
        let mut missing = false;
        visit_legacy(&source, |change| {
            let hash = digest(&serde_json::to_vec(&change).map_err(|_| "invalid legacy library")?);
            missing = destination.query_one("SELECT digest FROM sync_import_seen WHERE digest=?", p![hash])?.is_none();
            Ok(!missing)
        })?;
        source.exec("COMMIT")?;
        Ok(missing)
    }

    pub fn library(&self) -> Result<Store> {
        Store::open(&self.path)
    }

    /// Pending rows stay durable when an active job or a partial page prevents projection.
    pub fn project_pending(&self) -> Result<usize> {
        self.transaction(|db| project(db, 64))
    }

    pub fn pending_projection(&self) -> Result<i64> {
        Ok(Db::open(&self.path)?
            .query_one("SELECT COUNT(*) FROM sync_dirty", &[])?
            .unwrap()[0]
            .int())
    }

    /// The caller must hold the captured account ticket and explicit import confirmation.
    /// A source read snapshot and destination transaction preserve the original library.
    pub fn import_legacy(&self, source: &Store) -> Result<usize> {
        let source = legacy_source(source)?;
        let result = self.transaction(|db| {
            let mut imported = 0;
            visit_legacy(&source, |change| {
                let hash =
                    digest(&serde_json::to_vec(&change).map_err(|_| "invalid legacy library")?);
                if db
                    .query_one(
                        "SELECT digest FROM sync_import_seen WHERE digest=?",
                        p![&hash],
                    )?
                    .is_some()
                {
                    return Ok(true);
                }
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
                    seen_deletions: Deletions::new(),
                    change,
                };
                record.validate()?;
                if suppressed(db, &record)? {
                    return Err("legacy data was deleted from this account".into());
                }
                merge(db, &record)?;
                db.execute("INSERT INTO sync_import_seen VALUES (?)", p![hash])?;
                imported += 1;
                Ok(true)
            })?;
            project(db, 64)?;
            Ok(imported)
        });
        source.exec("COMMIT")?;
        result
    }
}

fn legacy_source(store: &Store) -> Result<Db> {
    let db = Db::open(store.path())?;
    db.exec("BEGIN")?;
    if local::owned(&db)? { return Err("account data cannot be imported as an unowned device library".into()); }
    Ok(db)
}

// A false visitor result stops after the first missing content receipt.
fn visit_legacy(db: &Db, mut visit: impl FnMut(Change) -> Result<bool>) -> Result<()> {
    for row in db.query("SELECT v.video_key,v.page_url,v.title,v.duration_ms,v.source_language,v.terms FROM video v WHERE NOT EXISTS(SELECT 1 FROM deleted_video d WHERE d.video_key=v.video_key) ORDER BY v.video_key", &[])? {
        let video = Video { key: row[0].text(), page_url: row[1].text(), title: row[2].text() };
        if !visit(Change::Video { video: video.clone() })? { return Ok(()); }
        if let Some(position) = db.query_one("SELECT position_ms FROM playback WHERE video_key=?", p![&video.key])? {
            if !visit(Change::Playback { key: video.key.clone(), position_ms: position[0].int() })? { return Ok(()); }
        }
        if db.query_one("SELECT id FROM job WHERE video_key=? AND stage IN ('downloading','writing','translating')", p![&video.key])?.is_some() { continue; }
        let Some(job) = db.query_one("SELECT translation_model,cost_krw FROM job WHERE video_key=? AND stage='ready' ORDER BY created_at DESC,rowid DESC LIMIT 1", p![&video.key])? else { continue; };
        let cues = db.query("SELECT start_ms,end_ms,source_text,text_ko FROM cue WHERE video_key=? ORDER BY seq", p![&video.key])?
            .iter().map(|r| Ok(Cue { start_ms:r[0].int(), end_ms:r[1].int(), source_text:r[2].text(), text_ko:r[3].opt_text().ok_or("incomplete legacy translation")? })).collect::<Result<Vec<_>>>()?;
        let terms = row[5].opt_text().map(|s| serde_json::from_str::<Vec<Term>>(&s).map_err(|_| "invalid legacy glossary".to_string())).transpose()?.unwrap_or_default();
        if !visit(Change::Translation { translation: Translation { video, duration_ms:row[3].int(), source_language:row[4].opt_text(), terms, model:job[0].text(), cost_krw:job[1].real(), cues } })? { return Ok(()); }
    }
    for (position, row) in db
        .query("SELECT id,name FROM playlist ORDER BY position,rowid", &[])?
        .iter()
        .enumerate()
    {
        let id = row[0].text();
        if !visit(Change::PlaylistName {
            id: id.clone(),
            name: row[1].text(),
        })? { return Ok(()); }
        if !visit(Change::PlaylistOrder {
            id: id.clone(),
            position: position as i64,
        })? { return Ok(()); }
        for (position, entry) in db.query("SELECT e.video_key FROM playlist_entry e WHERE playlist_id=? AND NOT EXISTS(SELECT 1 FROM deleted_video d WHERE d.video_key=e.video_key) ORDER BY position,rowid", p![&id])?.iter().enumerate() {
            let key = entry[0].text();
            if !visit(Change::EntryPresence { id:id.clone(), key:key.clone(), present:true })? { return Ok(()); }
            if !visit(Change::EntryOrder { id:id.clone(), key, position:position as i64 })? { return Ok(()); }
        }
    }
    Ok(())
}

pub(super) fn dirty(db: &Db, record: &Record) -> Result<()> {
    let (kind, key) = match &record.change {
        Change::Video { video } => ("video", video.key.clone()),
        Change::Translation { translation } => ("video", translation.video.key.clone()),
        Change::DeleteVideo { key } => ("video", key.clone()),
        Change::PlaylistName { id, .. }
        | Change::PlaylistOrder { id, .. }
        | Change::DeletePlaylist { id } => ("playlist", id.clone()),
        Change::EntryPresence { id, key, .. } | Change::EntryOrder { id, key, .. } => {
            ("entry", format!("{id}/{key}"))
        }
        Change::Playback { key, .. } => ("playback", key.clone()),
    };
    db.execute(
        "INSERT OR IGNORE INTO sync_dirty VALUES (?,?)",
        p![kind, key],
    )?;
    Ok(())
}

fn record(db: &Db, identity: &str) -> Result<Option<Record>> {
    db.query_one(
        "SELECT body FROM sync_record WHERE identity=?",
        p![identity],
    )?
    .map(|r| decode(&r[0].text()))
    .transpose()
}

pub(super) fn decode(body: &str) -> Result<Record> {
    let record: Record = serde_json::from_str(body).map_err(|_| "invalid stored sync record")?;
    record.validate()?;
    Ok(record)
}

pub(super) fn project(db: &Db, limit: i64) -> Result<usize> {
    let rows = db.query(
        "SELECT kind,key FROM sync_dirty ORDER BY rowid LIMIT ?",
        p![limit],
    )?;
    let mut completed = 0;
    for row in rows {
        let kind = row[0].text();
        let key = row[1].text();
        let done = match kind.as_str() {
            "video" => video(db, &key)?,
            "playlist" => playlist(db, &key)?,
            "entry" => entry(db, &key)?,
            "playback" => {
                if let Some(Record {
                    change: Change::Playback { position_ms, .. },
                    ..
                }) = record(db, &format!("p/{key}"))?
                {
                    db.execute("INSERT INTO playback VALUES (?,?) ON CONFLICT(video_key) DO UPDATE SET position_ms=excluded.position_ms", p![&key,position_ms])?;
                } else {
                    db.execute("DELETE FROM playback WHERE video_key=?", p![&key])?;
                }
                true
            }
            _ => return Err("invalid pending projection".into()),
        };
        if done {
            db.execute(
                "DELETE FROM sync_dirty WHERE kind=? AND key=?",
                p![kind, key],
            )?;
            completed += 1;
        } else {
            // A missing dependency or active job must not starve other pending changes.
            db.execute(
                "INSERT OR REPLACE INTO sync_dirty VALUES (?,?)",
                p![kind, key],
            )?;
        }
    }
    Ok(completed)
}

pub(super) fn video(db: &Db, key: &str) -> Result<bool> {
    if db.query_one("SELECT id FROM job WHERE video_key=? AND stage IN ('downloading','writing','translating')", p![key])?.is_some() { return Ok(false); }
    let newest = db.query_one("SELECT body FROM sync_record WHERE video_key=? AND identity LIKE 't/%' ORDER BY clock DESC,replica DESC LIMIT 1", p![key])?
        .map(|r| decode(&r[0].text())).transpose()?;
    let metadata = record(db, &format!("v/{key}"))?;
    let description = metadata
        .as_ref()
        .into_iter()
        .chain(newest.as_ref())
        .max_by_key(|r| &r.stamp);
    // Fresh metadata cannot preserve an old local response whose deletion has not been cleaned.
    if !local::deletions_cleaned(db, key)? {
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).map_err(|_| "deletion token unavailable")?;
        db.execute(
                "INSERT INTO deleted_video VALUES (?,?,0) ON CONFLICT(video_key) DO UPDATE SET token=excluded.token,complete=0 WHERE deleted_video.complete=1",
                p![key, digest(&random)],
            )?;
        db.execute("DELETE FROM playlist_entry WHERE video_key=?", p![key])?;
        db.execute("DELETE FROM playback WHERE video_key=?", p![key])?;
        db.execute(
            "DELETE FROM sync_selected_translation WHERE video_key=?",
            p![key],
        )?;
        if let Some(row) = db.query_one("SELECT url FROM browser WHERE id=1", &[])? {
            let raw = row[0].text();
            if url::Url::parse(&raw).is_ok_and(|u| crate::api::video_key(&u) == key) {
                db.execute("DELETE FROM browser WHERE id=1 AND url=?", p![raw])?;
            }
        }
        return Ok(description.is_none());
    }
    if description.is_none() {
        return Ok(true);
    }
    if db
        .query_one(
            "SELECT video_key FROM deleted_video WHERE video_key=? AND complete=0",
            p![key],
        )?
        .is_some()
    {
        return Ok(false);
    }
    let description = match &description.unwrap().change {
        Change::Video { video } => video,
        Change::Translation { translation } => &translation.video,
        _ => return Err("invalid video projection".into()),
    };
    let page =
        url::Url::parse(&description.page_url).map_err(|_| "invalid video projection URL")?;
    let site = if crate::youtube::is_youtube(&page) {
        "youtube"
    } else {
        "other"
    };
    db.execute("INSERT INTO video(video_key,page_url,site,title,duration_ms,created_at) VALUES (?,?,?,?,0,strftime('%Y-%m-%dT%H:%M:%fZ','now')) ON CONFLICT(video_key) DO UPDATE SET page_url=excluded.page_url,title=excluded.title", p![key,&description.page_url,site,&description.title])?;
    let Some(newest) = newest else {
        return Ok(true);
    };
    let identity = newest.identity();
    let Change::Translation { translation } = newest.change else {
        return Err("invalid translation projection".into());
    };
    let selected = db.query_one(
        "SELECT identity FROM sync_selected_translation WHERE video_key=?",
        p![key],
    )?;
    if selected.is_some_and(|r| r[0].text() == identity) {
        return Ok(true);
    }
    let terms =
        serde_json::to_string(&translation.terms).map_err(|_| "invalid glossary projection")?;
    // A saved version does not carry its mishearings: the video's belong to
    // whichever transcript found them, so they go, and the next translation of
    // this version finds its own again.
    db.execute(
        "UPDATE video SET duration_ms=?,source_language=?,terms=?,mishearings=NULL WHERE video_key=?",
        p![
            translation.duration_ms,
            translation.source_language,
            terms,
            key
        ],
    )?;
    db.execute("DELETE FROM cue WHERE video_key=?", p![key])?;
    for (seq, cue) in translation.cues.iter().enumerate() {
        db.execute(
            "INSERT INTO cue VALUES (?,?,?,?,?,?)",
            p![
                key,
                seq as i64,
                cue.start_ms,
                cue.end_ms,
                &cue.source_text,
                &cue.text_ko
            ],
        )?;
    }
    let job_id = format!(
        "sync-{}",
        identity
            .rsplit('/')
            .next()
            .ok_or("invalid translation identity")?
    );
    db.execute("INSERT INTO job(id,video_key,stage,percent,translation_model,cost_krw,created_at,updated_at) VALUES (?,?,'ready',100,?,?,strftime('%Y-%m-%dT%H:%M:%fZ','now'),strftime('%Y-%m-%dT%H:%M:%fZ','now')) ON CONFLICT(id) DO UPDATE SET created_at=excluded.created_at,updated_at=excluded.updated_at", p![job_id,key,translation.model,translation.cost_krw])?;
    db.execute("INSERT INTO sync_selected_translation VALUES (?,?) ON CONFLICT(video_key) DO UPDATE SET identity=excluded.identity", p![key,identity])?;
    db.execute(
        "DELETE FROM deleted_video WHERE video_key=? AND complete=1",
        p![key],
    )?;
    Ok(true)
}

fn playlist(db: &Db, id: &str) -> Result<bool> {
    if record(db, &format!("pd/{id}"))?.is_some() {
        db.execute("DELETE FROM playlist_entry WHERE playlist_id=?", p![id])?;
        db.execute("DELETE FROM playlist WHERE id=?", p![id])?;
        return Ok(true);
    }
    let Some(Record {
        change: Change::PlaylistName { name, .. },
        ..
    }) = record(db, &format!("pn/{id}"))?
    else {
        return Ok(false);
    };
    let position = match record(db, &format!("po/{id}"))? {
        Some(Record {
            change: Change::PlaylistOrder { position, .. },
            ..
        }) => position,
        _ => 0,
    };
    db.execute("INSERT INTO playlist VALUES (?,?,?) ON CONFLICT(id) DO UPDATE SET name=excluded.name,position=excluded.position", p![id,name,position])?;
    Ok(true)
}

fn entry(db: &Db, composite: &str) -> Result<bool> {
    let (id, key) = composite.split_once('/').ok_or("invalid pending entry")?;
    let present = matches!(
        record(db, &format!("ep/{composite}"))?,
        Some(Record {
            change: Change::EntryPresence { present: true, .. },
            ..
        })
    );
    if !present {
        db.execute(
            "DELETE FROM playlist_entry WHERE playlist_id=? AND video_key=?",
            p![id, key],
        )?;
        return Ok(true);
    }
    if db.query_one("SELECT id FROM playlist WHERE id=?", p![id])?.is_none()
        || db.query_one("SELECT video_key FROM video WHERE video_key=? AND NOT EXISTS(SELECT 1 FROM deleted_video WHERE video_key=? AND complete=0)", p![key,key])?.is_none() { return Ok(false); }
    let position = match record(db, &format!("eo/{composite}"))? {
        Some(Record {
            change: Change::EntryOrder { position, .. },
            ..
        }) => position,
        _ => 0,
    };
    db.execute("INSERT INTO playlist_entry VALUES (?,?,?) ON CONFLICT(playlist_id,video_key) DO UPDATE SET position=excluded.position", p![id,key,position])?;
    Ok(true)
}

#[cfg(test)]
#[path = "sync_store_tests.rs"]
mod tests;
