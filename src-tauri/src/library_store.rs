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

    pub(crate) fn select_translation(&self, key: &str, identity: &str) -> R<()> {
        self.library_write(|db| local::select_translation(db, key, identity))
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
            let to = (from as i64 + delta).clamp(0, ids.len() as i64 - 1) as usize;
            ids.swap(from, to);
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
