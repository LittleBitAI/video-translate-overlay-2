//! Called inside the originating Store transaction, never by incoming projection.
use super::*;

pub(crate) const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS sync_job_context (job_id TEXT PRIMARY KEY, deletions TEXT NOT NULL)
;CREATE TABLE IF NOT EXISTS sync_cleanup_context (video_key TEXT PRIMARY KEY, deletions TEXT NOT NULL)
";

pub(crate) fn owned(db: &Db) -> Result<bool> {
    Ok(db
        .query_one(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='sync_meta'",
            &[],
        )?
        .is_some())
}

pub(crate) fn write(db: &Db, change: Change) -> Result<()> {
    if !owned(db)? {
        return Ok(());
    }
    let seen = change
        .video_key()
        .map(|key| deletion_marks(db, key))
        .transpose()?
        .unwrap_or_default();
    append(db, change, seen, false)?;
    Ok(())
}

pub(crate) fn video(db: &Db, key: &str) -> Result<()> {
    if !owned(db)? {
        return Ok(());
    }
    let row = db
        .query_one(
            "SELECT page_url,title FROM video WHERE video_key=?",
            p![key],
        )?
        .ok_or("missing owned video")?;
    write(
        db,
        Change::Video {
            video: Video {
                key: key.into(),
                page_url: row[0].text(),
                title: row[1].text(),
            },
        },
    )
}

fn append(
    db: &Db,
    change: Change,
    seen_deletions: Deletions,
    allow_deleted: bool,
) -> Result<Option<Record>> {
    let meta = db
        .query_one("SELECT replica,clock FROM sync_meta WHERE id=1", &[])?
        .ok_or("missing sync metadata")?;
    let record = Record {
        stamp: Stamp {
            replica: meta[0].text(),
            clock: meta[1].int().checked_add(1).ok_or("sync clock exhausted")?,
        },
        change,
        seen_deletions,
    };
    record.validate()?;
    if suppressed(db, &record)? {
        if allow_deleted {
            return Ok(None);
        }
        return Err("sync item was deleted; fresh confirmation is required".into());
    }
    merge(db, &record)?;
    Ok(Some(record))
}

pub(crate) fn capture_job(db: &Db, id: &str, key: &str) -> Result<()> {
    if !owned(db)? {
        return Ok(());
    }
    let context =
        serde_json::to_string(&deletion_marks(db, key)?).map_err(|_| "invalid deletion context")?;
    db.execute("INSERT INTO sync_job_context VALUES (?,?)", p![id, context])?;
    Ok(())
}

pub(crate) fn can_start_job(db: &Db, key: &str) -> Result<bool> {
    // A queued remote deletion may not yet have projected its local cleanup marker.
    let marks = deletion_marks(db, key)?;
    if marks.is_empty() {
        return Ok(true);
    }
    if db
        .query_one(
            "SELECT video_key FROM deleted_video WHERE video_key=? AND complete=1",
            p![key],
        )?
        .is_some()
    {
        let saved = db.query_one(
            "SELECT deletions FROM sync_cleanup_context WHERE video_key=?",
            p![key],
        )?;
        let context: Option<Deletions> = saved
            .map(|r| serde_json::from_str(&r[0].text()).map_err(|_| "invalid cleanup context"))
            .transpose()?;
        return Ok(context.as_ref() == Some(&marks));
    }
    for row in db.query("SELECT body FROM sync_record WHERE video_key=? AND (identity LIKE 'v/%' OR identity LIKE 't/%')", p![key])? {
        let record: Record = serde_json::from_str(&row[0].text()).map_err(|_| "invalid stored sync record")?;
        if !suppressed(db, &record)? { return Ok(true); }
    }
    Ok(false)
}

pub(crate) fn finish_job(db: &Db, id: &str) -> Result<()> {
    if !owned(db)? {
        return Ok(());
    }
    let row = db.query_one("SELECT v.video_key,v.page_url,v.title,v.duration_ms,v.source_language,v.terms,j.translation_model,j.cost_krw,c.deletions FROM job j JOIN video v ON v.video_key=j.video_key JOIN sync_job_context c ON c.job_id=j.id WHERE j.id=? AND j.stage='ready'", p![id])?.ok_or("missing owned job context")?;
    let key = row[0].text();
    let cues = db
        .query(
            "SELECT start_ms,end_ms,source_text,text_ko FROM cue WHERE video_key=? ORDER BY seq",
            p![&key],
        )?
        .iter()
        .map(|r| {
            Ok(Cue {
                start_ms: r[0].int(),
                end_ms: r[1].int(),
                source_text: r[2].text(),
                text_ko: r[3].opt_text().ok_or("incomplete translation")?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let terms = row[5]
        .opt_text()
        .map(|s| serde_json::from_str(&s).map_err(|_| "invalid glossary".to_string()))
        .transpose()?
        .unwrap_or_default();
    let context =
        serde_json::from_str(&row[8].text()).map_err(|_| "invalid job deletion context")?;
    let change = Change::Translation {
        translation: Translation {
            video: Video {
                key: key.clone(),
                page_url: row[1].text(),
                title: row[2].text(),
            },
            duration_ms: row[3].int(),
            source_language: row[4].opt_text(),
            terms,
            model: row[6].text(),
            cost_krw: row[7].real(),
            cues,
        },
    };
    // A deletion received during processing still wins; job completion cannot supply new consent.
    if let Some(record) = append(db, change, context, true)? {
        db.execute("INSERT INTO sync_selected_translation VALUES (?,?) ON CONFLICT(video_key) DO UPDATE SET identity=excluded.identity", p![key,record.identity()])?;
    }
    Ok(())
}

pub(crate) fn delete_video(db: &Db, key: &str) -> Result<()> {
    if !owned(db)? {
        return Ok(());
    }
    if db
        .query_one(
            "SELECT video_key FROM deleted_video WHERE video_key=? AND complete=0",
            p![key],
        )?
        .is_none()
    {
        write(db, Change::DeleteVideo { key: key.into() })?;
    }
    Ok(())
}

pub(crate) fn cleanup_video(db: &Db, key: &str) -> Result<()> {
    if !owned(db)? {
        return Ok(());
    }
    db.execute(
        "DELETE FROM sync_job_context WHERE job_id IN (SELECT id FROM job WHERE video_key=?)",
        p![key],
    )?;
    db.execute(
        "DELETE FROM sync_selected_translation WHERE video_key=?",
        p![key],
    )?;
    db.execute("DELETE FROM playback WHERE video_key=?", p![key])?;
    let context =
        serde_json::to_string(&deletion_marks(db, key)?).map_err(|_| "invalid cleanup context")?;
    db.execute("INSERT INTO sync_cleanup_context VALUES (?,?) ON CONFLICT(video_key) DO UPDATE SET deletions=excluded.deletions", p![key,context])?;
    Ok(())
}
