use super::*;
use serde_json::{json, Value};
use crate::sync::{local, Change};

impl Store {
    pub(crate) fn playback(&self, key: &str) -> R<Option<i64>> {
        Ok(self.db.query_one("SELECT position_ms FROM playback WHERE video_key=?", p![key])?.map(|row| row[0].int()))
    }

    pub(crate) fn save_playback(&self, key: &str, position: i64, duration: i64) -> R<()> {
        if !(1..=crate::sync::MAX_DURATION_MS).contains(&duration) || !(0..=duration).contains(&position) {
            return Err("invalid playback position".into());
        }
        self.library_write(|db| {
            let row = db.query_one("SELECT duration_ms FROM video WHERE video_key=? AND NOT EXISTS(SELECT 1 FROM deleted_video WHERE video_key=?)", p![key,key])?
                .ok_or("playback unavailable")?;
            let saved_duration = row[0].int();
            if saved_duration > 0 && ((saved_duration - duration).abs() > 1000 || position > saved_duration) {
                return Err("playback player mismatch".into());
            }
            if local::owned(db)? && !local::deletions_cleaned(db, key)? { return Err("playback unavailable".into()); }
            if db.query_one("SELECT position_ms FROM playback WHERE video_key=?", p![key])?.is_some_and(|row| row[0].int() == position) { return Ok(()); }
            local::write(db, Change::Playback {key:key.into(),position_ms:position})?;
            db.execute("INSERT INTO playback VALUES (?,?) ON CONFLICT(video_key) DO UPDATE SET position_ms=excluded.position_ms", p![key,position])?;
            Ok(())
        })
    }

    // Mutations use a separate connection so concurrent IPC never joins another transaction.
    fn library_write<T>(&self, f: impl FnOnce(&Db) -> R<T>) -> R<T> {
        let db = Db::open(&self.path)?;
        db.exec("BEGIN IMMEDIATE")?;
        let result = f(&db);
        db.exec(if result.is_ok() { "COMMIT" } else { "ROLLBACK" })?;
        result
    }

    pub fn library(&self) -> R<Value> {
        let db = Db::open(&self.path)?;
        db.exec("BEGIN")?;
        let owned = local::owned(&db)?;
        let videos = db
            .query(
                "SELECT v.video_key,v.page_url,v.title,v.duration_ms,
            EXISTS(SELECT 1 FROM job j WHERE j.video_key=v.video_key AND j.stage='ready'),
            EXISTS(SELECT 1 FROM job j WHERE j.video_key=v.video_key AND j.stage IN ('downloading','writing','translating')),
            EXISTS(SELECT 1 FROM deleted_video d WHERE d.video_key=v.video_key AND d.complete=0)
            FROM video v ORDER BY v.created_at DESC,v.rowid DESC",
                &[],
            )?
            .iter()
            .map(|r| -> R<Value> {
                let versions = if owned && r[6].int()==0 {
                    db.query("SELECT r.identity,COALESCE(r.identity=s.identity,0) FROM sync_record r LEFT JOIN sync_selected_translation s ON s.video_key=r.video_key WHERE r.video_key=? AND r.identity LIKE 't/%' ORDER BY r.clock DESC,r.replica DESC", p![r[0].text()])?
                        .iter().map(|v| json!({"id":v[0].text(),"selected":v[1].int()!=0})).collect::<Vec<_>>()
                } else { Vec::new() };
                Ok(json!({"video_key":r[0].text(),"page_url":r[1].text(),"title":r[2].text(),"duration_ms":r[3].int(),
                "saved":r[4].int()!=0 && r[6].int()==0,"active":r[5].int()!=0,"deleting":r[6].int()!=0,"versions":versions}))
            })
            .collect::<R<Vec<_>>>()?;
        let mut playlists = Vec::new();
        for r in db.query("SELECT id,name FROM playlist ORDER BY position,id", &[])? {
            let entries = db
                .query("SELECT video_key FROM playlist_entry WHERE playlist_id=? ORDER BY position,video_key", p![r[0].text()])?
                .iter()
                .map(|e| e[0].text())
                .collect::<Vec<_>>();
            playlists.push(json!({"id":r[0].text(),"name":r[1].text(),"entries":entries}));
        }
        db.exec("COMMIT")?;
        Ok(json!({"videos":videos,"playlists":playlists}))
    }

    pub fn select_translation(&self, key: &str, identity: &str) -> R<()> {
        self.library_write(|db| local::select_translation(db, key, identity))
    }

    /// Starts making a saved video again from its audio (`audio`, already on
    /// this device), as a new job at writing in this account library. The
    /// selected saved version stays and is returned, to select again if the
    /// job fails; the new version is selected when it is ready. The job starts
    /// with everything the video has spent, so its cost adds to that record.
    pub fn begin_remake(&self, id: &str, key: &str, audio: &str, model: &str) -> R<String> {
        self.library_write(|db| {
            if !local::owned(db)? {
                return Err("a re-make needs an account library with saved versions".into());
            }
            if db.query_one("SELECT id FROM job WHERE stage IN ('downloading','writing','translating')", &[])?.is_some()
                || db.query_one("SELECT video_key FROM deleted_video WHERE video_key=?", p![key])?.is_some()
                || !local::deletions_cleaned(db, key)?
            {
                return Err("another job is running, or the video is being deleted".into());
            }
            let selected = db.query_one("SELECT identity FROM sync_selected_translation WHERE video_key=?", p![key])?.ok_or("no saved version is selected")?[0].text();
            let spent = self.retranslation_cost(key)?;
            db.execute(
                "INSERT INTO job (id, video_key, stage, audio_path, plan, translation_model, cost_krw, created_at, updated_at)
                 VALUES (?,?,'writing',?,?,?,?,strftime('%Y-%m-%dT%H:%M:%fZ','now'),strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
                p![id, key, audio, crate::audio::PLAN, model, spent],
            )?;
            db.execute("DELETE FROM cue WHERE video_key=?", p![key])?;
            db.execute("UPDATE video SET terms=NULL, mishearings=NULL, source_language=NULL WHERE video_key=?", p![key])?;
            local::capture_job(db, id, key)?;
            Ok(selected)
        })
    }

    /// Sends a failed re-make back to translating: what it wrote and
    /// translated stays, so only the cues without Korean are asked again. It
    /// becomes the video's newest job: a restore after the failure projected
    /// the previous version as a newer job, and the newest ready job is the
    /// one the overlay shows when no job matches the selected version.
    pub fn continue_remake(&self, id: &str) -> R<()> {
        self.library_write(|db| {
            let n = db.execute("UPDATE job SET stage='translating', failure_reason=NULL, created_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=? AND id LIKE 'remake-%' AND stage='failed'", p![id])?;
            if n == 0 { Err("no failed re-make with this id".into()) } else { Ok(()) }
        })
    }

    /// Projects `identity` again after a failed re-make overwrote the cue
    /// table. [`Self::select_translation`] keeps the table when the identity
    /// is already selected, so the selection is dropped first.
    pub fn restore_translation(&self, key: &str, identity: &str) -> R<()> {
        self.library_write(|db| {
            db.execute("DELETE FROM sync_selected_translation WHERE video_key=?", p![key])?;
            local::select_translation(db, key, identity)
        })
    }

    /// Capture the requested selection and projected cue in one SQLite snapshot before audio inspection.
    pub fn inspect_silent_cue(&self, key: &str, identity: &str, seq: usize) -> R<Cue> {
        let seq = i64::try_from(seq).map_err(|_| "cue unavailable")?;
        let row = self.db.query_one("SELECT c.seq,c.start_ms,c.end_ms,c.source_text,c.text_ko FROM cue c JOIN sync_selected_translation s ON s.video_key=c.video_key WHERE c.video_key=? AND s.identity=? AND c.seq=?", p![key,identity,seq])?
            .ok_or("selected version or cue unavailable; inspect it again before repair")?;
        Ok(Cue { seq:row[0].int(), start_ms:row[1].int(), end_ms:row[2].int(), source_text:row[3].text(), text_ko:row[4].opt_text() })
    }

    /// The caller checks this exact cue's original audio first. Publication rechecks it atomically.
    pub fn repair_silent_cue(&self, key: &str, identity: &str, inspected: &Cue) -> R<String> {
        self.library_write(|db| local::repair_silent_cue(db, key, identity, inspected))
    }

    pub fn deleted(&self, key: &str) -> R<Option<(String, bool)>> {
        Ok(self.db.query_one("SELECT token,complete FROM deleted_video WHERE video_key=?", p![key])?.map(|r| (r[0].text(), r[1].int() != 0)))
    }

    pub fn begin_delete(&self, key: &str, token: &str) -> R<()> {
        self.library_write(|db| {
            if db.query_one("SELECT id FROM job WHERE video_key=? AND stage IN ('downloading','writing','translating')", p![key])?.is_some() {
                return Err("active video".into());
            }
            local::delete_video(db, key)?;
            db.execute(
                "INSERT INTO deleted_video (video_key,token) VALUES (?,?) ON CONFLICT(video_key) DO UPDATE SET token=excluded.token,complete=0",
                p![key, token],
            )?;
            Ok(())
        })
    }

    pub fn video_jobs(&self, key: &str) -> R<Vec<(String, Option<String>)>> {
        Ok(self.db.query("SELECT id,audio_path FROM job WHERE video_key=?", p![key])?.iter().map(|r| (r[0].text(), r[1].opt_text())).collect())
    }

    pub(crate) fn pending_video_deletions(&self, after: &str) -> R<Vec<String>> {
        Ok(self.db.query("SELECT d.video_key FROM deleted_video d WHERE d.complete=0 AND d.video_key>? AND NOT EXISTS(SELECT 1 FROM job j WHERE j.video_key=d.video_key AND j.stage IN ('downloading','writing','translating')) ORDER BY d.video_key LIMIT 16", p![after])?
            .iter().map(|row| row[0].text()).collect())
    }

    pub fn finish_delete(&self, key: &str, inspected_url: Option<&str>) -> R<()> {
        self.library_write(|db| {
            local::cleanup_video(db, key)?;
            db.execute("DELETE FROM retranslation_cue WHERE job_id IN (SELECT id FROM job WHERE video_key=?)", p![key])?;
            db.execute("DELETE FROM playlist_entry WHERE video_key=?", p![key])?;
            db.execute("DELETE FROM cue WHERE video_key=?", p![key])?;
            db.execute("DELETE FROM playback WHERE video_key=?", p![key])?;
            db.execute("DELETE FROM job WHERE video_key=?", p![key])?;
            db.execute("DELETE FROM video WHERE video_key=?", p![key])?;
            if let Some(url) = inspected_url {
                db.execute("DELETE FROM browser WHERE id=1 AND url=?", p![url])?;
            }
            db.execute("UPDATE deleted_video SET complete=1 WHERE video_key=?", p![key])?;
            Ok(())
        })
    }

    pub fn playlist_exists(&self, id: &str) -> R<bool> {
        Ok(self.db.query_one("SELECT id FROM playlist WHERE id=?", p![id])?.is_some())
    }

    pub fn create_playlist(&self, id: &str, name: &str) -> R<()> {
        self.library_write(|db| {
            db.execute("INSERT INTO playlist (id,name,position) SELECT ?,?,COALESCE(MAX(position),-1)+1 FROM playlist", p![id, name])?;
            local::write(db, Change::PlaylistName { id:id.into(), name:name.into() })?;
            let position = db.query_one("SELECT position FROM playlist WHERE id=?", p![id])?.ok_or("missing playlist")?[0].int();
            local::write(db, Change::PlaylistOrder { id:id.into(), position })?;
            Ok(())
        })
    }

    pub fn rename_playlist(&self, id: &str, name: &str) -> R<()> {
        self.library_write(|db| {
            if db.execute("UPDATE playlist SET name=? WHERE id=? AND name!=?", p![name,id,name])? != 0 {
                local::write(db, Change::PlaylistName { id:id.into(),name:name.into() })?;
            }
            Ok(())
        })
    }

    pub fn delete_playlist(&self, id: &str) -> R<()> {
        self.library_write(|db| {
            local::write(db, Change::DeletePlaylist { id:id.into() })?;
            db.execute("DELETE FROM playlist_entry WHERE playlist_id=?", p![id])?;
            db.execute("DELETE FROM playlist WHERE id=?", p![id])?;
            Ok(())
        })
    }

    pub fn add_entry(&self, id: &str, key: &str, page: &str, site: &str, title: &str) -> R<()> {
        self.library_write(|db| {
            db.execute(&format!("INSERT OR IGNORE INTO video (video_key,page_url,site,title,duration_ms,created_at) VALUES (?,?,?,?,0,{NOW})"), p![key,page,site,title])?;
            db.execute("INSERT OR IGNORE INTO playlist_entry (playlist_id,video_key,position) SELECT ?,?,COALESCE(MAX(position),-1)+1 FROM playlist_entry WHERE playlist_id=?", p![id,key,id])?;
            if local::owned(db)? {
                local::video(db, key)?;
                local::write(db, Change::EntryPresence { id:id.into(), key:key.into(), present:true })?;
                let position = db.query_one("SELECT position FROM playlist_entry WHERE playlist_id=? AND video_key=?", p![id,key])?.ok_or("missing playlist entry")?[0].int();
                local::write(db, Change::EntryOrder { id:id.into(), key:key.into(), position })?;
            }
            Ok(())
        })
    }

    pub fn remove_entry(&self, id: &str, key: &str) -> R<()> {
        self.library_write(|db| {
            db.execute("DELETE FROM playlist_entry WHERE playlist_id=? AND video_key=?", p![id,key])?;
            local::write(db, Change::EntryPresence { id:id.into(),key:key.into(),present:false })?;
            Ok(())
        })
    }

    pub fn move_library_item(&self, id: &str, key: Option<&str>, delta: i64) -> R<()> {
        self.library_write(|db| {
            let rows = if key.is_some() {
                db.query("SELECT video_key FROM playlist_entry WHERE playlist_id=? ORDER BY position,video_key", p![id])?
            } else {
                db.query("SELECT id FROM playlist ORDER BY position,id", &[])?
            };
            let mut ids = rows.iter().map(|r| r[0].text()).collect::<Vec<_>>();
            let from = ids.iter().position(|v| v == key.unwrap_or(id)).ok_or("missing item")?;
            // Any step, clamped at the ends; the items between shift by one place. Saturating, so a
            // delta near the i64 limits lands at an end instead of wrapping or panicking.
            let to = (from as i64).saturating_add(delta).clamp(0, ids.len() as i64 - 1) as usize;
            let moved = ids.remove(from);
            ids.insert(to, moved);
            for (i, value) in ids.iter().enumerate() {
                if key.is_some() {
                    if db.execute("UPDATE playlist_entry SET position=? WHERE playlist_id=? AND video_key=? AND position!=?", p![i as i64,id,value,i as i64])? != 0 {
                        local::write(db, Change::EntryOrder { id:id.into(),key:value.clone(),position:i as i64 })?;
                    }
                } else {
                    if db.execute("UPDATE playlist SET position=? WHERE id=? AND position!=?", p![i as i64,value,i as i64])? != 0 {
                        local::write(db, Change::PlaylistOrder { id:value.clone(),position:i as i64 })?;
                    }
                }
            }
            Ok(())
        })
    }
}
