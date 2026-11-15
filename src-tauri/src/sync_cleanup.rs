//! Local maintenance has its own worker so filesystem cleanup never holds up listener shutdown.
use super::*;
use crate::api::{self, ApiError};

#[derive(Default)]
struct Cursor { generation: Option<u64>, after: String }

fn once(core: &Core, cursor: &mut Cursor, stopped: &AtomicBool) -> Result<bool, ApiError> {
    let library = core.library(None)?;
    if !library.owns_account() { return Ok(false); }
    let Ok(_writing) = core.create_lock.try_lock() else { return Ok(false) };
    if cursor.generation != Some(library.generation()) {
        cursor.generation = Some(library.generation()); cursor.after.clear();
    }
    let mut changed = library.project_pending()? > 0;
    let keys = library.run(|store| Ok(store.pending_video_deletions(&cursor.after)?))?;
    if keys.is_empty() { cursor.after.clear(); }
    let mut failed = false;
    for key in keys {
        cursor.after = key.clone();
        if stopped.load(Ordering::Acquire) { break; }
        let jobs = library.run(|store| Ok(store.video_jobs(&key)?))?;
        changed = true;
        let _ = core.svc.browser.invalidate_video(&key);
        let cleanup = core.cleanup_video_files(&jobs, || !stopped.load(Ordering::Acquire) && library.run(|_| Ok(())).is_ok());
        if cleanup.is_err() {
            if stopped.load(Ordering::Acquire) { break; }
            library.run(|_| Ok(()))?;
            failed = true; continue;
        }
        library.run(|store| {
            let inspected = store.opened_url()?.filter(|raw| url::Url::parse(raw).is_ok_and(|url| api::video_key(&url) == key));
            store.finish_delete(&key, inspected.as_deref())?;
            Ok(())
        })?;
        library.project_pending()?;
        let _ = core.svc.browser.invalidate_video(&key);
    }
    if changed { let _ = core.svc.browser.refresh_library(); }
    if failed { return Err(api::err(503, "cleanup_failed", "삭제할 파일을 지우지 못했습니다. 변경을 보관하고 자동으로 다시 시도합니다.")); }
    Ok(changed)
}

pub(super) fn run(weak: Weak<Core>, controls: Arc<Connections>) {
    let mut cursor = Cursor::default();
    let mut next = Instant::now();
    let mut reported = false;
    while !controls.stopped.load(Ordering::Acquire) {
        if Instant::now() >= next {
            next = Instant::now() + INTERVAL;
            let Some(core) = weak.upgrade() else { break };
            match once(&core, &mut cursor, &controls.stopped) {
                Err(error) if error.error == "cleanup_failed" => {
                    if !reported { crate::log("local sync deletion cleanup needs retry"); }
                    reported = true;
                },
                Ok(_) => reported = false,
                Err(_) => {},
            }
        }
        thread::sleep(POLL);
    }
}

#[cfg(test)]
#[path = "sync_cleanup_tests.rs"]
mod tests;
