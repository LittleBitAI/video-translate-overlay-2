use super::*;

pub fn get_library(core: &Core) -> ApiResult {
    Ok(core.store.library()?)
}

/// Local screens only; site overlays keep their existing three permissions.
pub fn library_action(core: &Core, action: &Value) -> ApiResult {
    let _writing = core.create_lock.lock().unwrap_or_else(|p| p.into_inner());
    let s = |name: &str| action[name].as_str().unwrap_or("");
    let id = s("playlist_id");
    let name = s("name").trim();
    let op = s("action");
    if matches!(op, "create_playlist" | "rename_playlist") && (name.is_empty() || name.chars().count() > 200) {
        return Err(err(400, "bad_name", "재생목록 이름은 1~200자로 넣어 주세요."));
    }
    if matches!(op, "rename_playlist" | "delete_playlist" | "move_playlist" | "add_entry" | "remove_entry" | "move_entry") && !core.store.playlist_exists(id)? {
        return Err(err(404, "no_playlist", "재생목록이 없습니다. 다시 불러와 주세요."));
    }
    let delta = action["delta"].as_i64().unwrap_or(0);
    if matches!(op, "move_playlist" | "move_entry") && ![-1, 1].contains(&delta) {
        return Err(err(400, "bad_move", "위나 아래로 한 칸씩 이동해 주세요."));
    }
    match op {
        "create_playlist" => core.store.create_playlist(&new_job_id(), name)?,
        "rename_playlist" => core.store.rename_playlist(id, name)?,
        "delete_playlist" => core.store.delete_playlist(id)?,
        "move_playlist" => core.store.move_library_item(id, None, delta)?,
        "add_entry" => {
            let page = http_url(s("page_url"))?;
            if youtube::is_youtube(&page) && youtube::video_id(&page).is_none() {
                return Err(err(400, "bad_url", "유튜브 영상 주소를 넣어 주세요."));
            }
            let key = video_key(&page);
            if core.store.deleted(&key)?.is_some_and(|(_, complete)| !complete) {
                return Err(err(409, "cleanup_pending", "먼저 보관함에서 삭제를 마쳐 주세요."));
            }
            core.store.add_entry(id, &key, page.as_str(), site(&page), &clip_title(s("title"), page.as_str()))?;
        }
        "remove_entry" => core.store.remove_entry(id, s("video_key"))?,
        "move_entry" => core.store.move_library_item(id, Some(s("video_key")), delta)?,
        "delete_video" => {
            let key = s("video_key");
            let video = core.store.video(key)?.ok_or_else(|| err(404, "no_video", "영상이 없습니다."))?;
            if action["confirmed"] != true || s("title") != video.title {
                return Err(err(400, "confirmation_required", "영상 이름을 확인한 뒤 영구 삭제를 확인해 주세요. 나중에 다시 번역하면 API 비용이 발생합니다."));
            }
            if core.store.active_job()?.is_some_and(|j| j.video_key == key) {
                return Err(err(409, "video_active", "이 영상의 처리가 끝난 뒤에 삭제해 주세요."));
            }
            core.store.begin_delete(key, &new_job_id())?;
            core.svc.browser.invalidate_video(key)?;
            let cleanup = || -> Result<(), String> {
                let audio = core.cfg.data_dir.join("audio");
                let jobs = core.store.video_jobs(key)?;
                // Only the job's generated files in audio/ belong to this video.
                for entry in std::fs::read_dir(&audio).map_err(|e| e.to_string())? {
                    let entry = entry.map_err(|e| e.to_string())?;
                    let file = entry.file_name().to_string_lossy().into_owned();
                    if jobs.iter().any(|(id, _)| file.starts_with(&format!("{id}."))) {
                        match std::fs::remove_file(entry.path()) {
                            Ok(()) => {}
                            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                            Err(e) => return Err(e.to_string()),
                        }
                    }
                }
                Ok(())
            };
            cleanup().map_err(|e| {
                log(format!("video cleanup: {e}"));
                err(503, "cleanup_failed", "파일을 지우지 못했습니다. 자막은 숨겼습니다. 보관함에서 삭제를 다시 시도해 주세요.")
            })?;
            let inspected = core.store.opened_url()?.filter(|raw| http_url(raw).ok().is_some_and(|u| video_key(&u) == key));
            core.store.finish_delete(key, inspected.as_deref())?;
            if let Err(e) = core.svc.browser.invalidate_video(key) {
                log(format!("refresh after video deletion: {e}"));
            }
        }
        _ => return Err(err(400, "bad_action", "지원하지 않는 보관함 작업입니다.")),
    }
    Ok(json!({"changed":true}))
}
