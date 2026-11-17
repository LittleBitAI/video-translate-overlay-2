use super::*;

pub fn get_library(core: &Core) -> ApiResult {
    get_library_in(&core.library(None)?)
}

pub(super) fn get_library_in(library: &crate::library_scope::Library<'_>) -> ApiResult {
    let mut result = library.run(|store| Ok(store.library()?))?;
    result["legacy_available"] = library.legacy_available()?.into();
    result["account_email"] = json!(library.email());
    Ok(result)
}

/// Local screens only; site overlays keep their existing three permissions.
pub fn library_action(core: &Core, action: &Value) -> ApiResult {
    library_action_in(&core.library(None)?, action)
}

pub(super) fn library_action_in(library: &crate::library_scope::Library<'_>, action: &Value) -> ApiResult {
    let core = library.core;
    let _writing = core.create_lock.lock().unwrap_or_else(|p| p.into_inner());
    let s = |name: &str| action[name].as_str().unwrap_or("");
    let id = s("playlist_id");
    let name = s("name").trim();
    let op = s("action");
    if matches!(op, "create_playlist" | "rename_playlist") && (name.is_empty() || name.chars().count() > 200) {
        return Err(err(400, "bad_name", "재생목록 이름은 1~200자로 넣어 주세요."));
    }
    library.run(|store| {
    if matches!(op, "rename_playlist" | "delete_playlist" | "move_playlist" | "add_entry" | "remove_entry" | "move_entry") && !store.playlist_exists(id)? {
        return Err(err(404, "no_playlist", "재생목록이 없습니다. 다시 불러와 주세요."));
    }
    let delta = action["delta"].as_i64().unwrap_or(0);
    if matches!(op, "move_playlist" | "move_entry") && delta == 0 {
        return Err(err(400, "bad_move", "옮길 칸 수를 정해 주세요."));
    }
    match op {
        "create_playlist" => store.create_playlist(&new_job_id(), name)?,
        "rename_playlist" => store.rename_playlist(id, name)?,
        "delete_playlist" => store.delete_playlist(id)?,
        "move_playlist" => store.move_library_item(id, None, delta)?,
        "add_entry" => {
            let page = http_url(s("page_url"))?;
            if youtube::is_youtube(&page) && youtube::video_id(&page).is_none() {
                return Err(err(400, "bad_url", "유튜브 영상 주소를 넣어 주세요."));
            }
            let key = video_key(&page);
            if store.deleted(&key)?.is_some_and(|(_, complete)| !complete) {
                return Err(err(409, "cleanup_pending", "먼저 보관함에서 삭제를 마쳐 주세요."));
            }
            store.add_entry(id, &key, page.as_str(), site(&page), &clip_title(s("title"), page.as_str()))?;
        }
        "remove_entry" => store.remove_entry(id, s("video_key"))?,
        "move_entry" => store.move_library_item(id, Some(s("video_key")), delta)?,
        "select_translation" => store.select_translation(s("video_key"), s("translation_id"))
            .map_err(|_| err(409, "version_unavailable", "저장본을 선택하지 못했습니다. 처리와 삭제 상태를 확인하고 보관함을 다시 불러와 주세요."))?,
        "delete_video" => {
            let key = s("video_key");
            let video = store.video(key)?.ok_or_else(|| err(404, "no_video", "영상이 없습니다."))?;
            if action["confirmed"] != true || s("title") != video.title {
                return Err(err(400, "confirmation_required", "영상 이름을 확인한 뒤 영구 삭제를 확인해 주세요. 나중에 다시 번역하면 API 비용이 발생합니다."));
            }
            if store.active_job()?.is_some_and(|j| j.video_key == key) {
                return Err(err(409, "video_active", "이 영상의 처리가 끝난 뒤에 삭제해 주세요."));
            }
            store.begin_delete(key, &new_job_id())?;
        }
        _ => return Err(err(400, "bad_action", "지원하지 않는 보관함 작업입니다.")),
    }
    Ok(())
    })?;
    if op == "delete_video" {
            let key = s("video_key");
            core.svc.browser.invalidate_video(key)?;
            let jobs = library.run(|store| Ok(store.video_jobs(key)?))?;
            core.cleanup_video_files(&jobs, || library.run(|_| Ok(())).is_ok()).map_err(|e| {
                log(format!("video cleanup: {e}"));
                err(503, "cleanup_failed", "파일을 지우지 못했습니다. 자막은 숨겼습니다. 보관함에서 삭제를 다시 시도해 주세요.")
            })?;
            library.run(|store| {
                let inspected = store.opened_url()?.filter(|raw| http_url(raw).ok().is_some_and(|u| video_key(&u) == key));
                store.finish_delete(key, inspected.as_deref())?;
                Ok(())
            })?;
            if let Err(e) = core.svc.browser.invalidate_video(key) {
                log(format!("refresh after video deletion: {e}"));
            }
    }
    Ok(json!({"changed":true}))
}
