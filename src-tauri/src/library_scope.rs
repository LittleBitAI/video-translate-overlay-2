//! A request keeps its original library; only short database operations hold the account lock.

use crate::{account::LibraryScope, api::{self, ApiError}, store::Store, Core};

pub(crate) struct Library<'a> {
    pub core: &'a Core,
    scope: LibraryScope,
    store: Store,
}

fn changed() -> ApiError {
    api::err(409, "account_changed", "계정 상태가 바뀌었습니다. 화면을 다시 불러와 주세요.")
}

fn unavailable(error: crate::google_auth::AuthError) -> ApiError {
    if error == crate::google_auth::AuthError::Cancelled { changed() }
    else { api::err(503, "account_unavailable", "계정 상태를 확인하지 못했습니다. 잠시 뒤 다시 불러와 주세요.") }
}

impl Core {
    pub(crate) fn cleanup_video_files(&self, jobs: &[(String, Option<String>)], current: impl Fn() -> bool) -> Result<(), String> {
        let entries = match std::fs::read_dir(self.cfg.data_dir.join("audio")) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.to_string()),
        };
        for entry in entries {
            if !current() { return Err("cleanup cancelled".into()); }
            let entry = entry.map_err(|e| e.to_string())?;
            let file = entry.file_name().to_string_lossy().into_owned();
            if jobs.iter().any(|(id, _)| file.starts_with(&format!("{id}."))) {
                match std::fs::remove_file(entry.path()) {
                    Ok(()) => {},
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
                    Err(e) => return Err(e.to_string()),
                }
            }
        }
        Ok(())
    }

    pub(crate) fn library(&self, expected: Option<u64>) -> Result<Library<'_>, ApiError> {
        let scope = self.account.capture_library().map_err(unavailable)?;
        if expected.is_some_and(|generation| generation != scope.generation()) {
            return Err(changed());
        }
        let store = self.account.with_library(&scope, |journal| Ok(match journal {
            Some(journal) => journal.library(),
            None => Store::open(self.store.path()),
        })).map_err(unavailable)??;
        Ok(Library { core: self, scope, store })
    }

    /// Only fixed app-owned database names are inspected; peers never supply these paths.
    pub(crate) fn job_stores(&self) -> Result<Vec<Store>, String> {
        let mut paths = vec![self.store.path().to_path_buf()];
        let directory = self.cfg.data_dir.join("sync");
        match std::fs::read_dir(directory) {
            Ok(entries) => {
                let mut owned = Vec::new();
                for entry in entries {
                    let entry = entry.map_err(|e| e.to_string())?;
                    let name = entry.file_name();
                    let Some(name) = name.to_str() else { continue };
                    if name.len() != 64 || !name.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
                        || !entry.file_type().map_err(|e| e.to_string())?.is_dir() {
                        continue;
                    }
                    let path = entry.path().join("sync.sqlite3");
                    match std::fs::symlink_metadata(&path) {
                        Ok(meta) if meta.file_type().is_file() => owned.push(path),
                        Ok(_) => return Err("account database is not a regular file".into()),
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
                        Err(e) => return Err(e.to_string()),
                    }
                }
                owned.sort();
                paths.extend(owned);
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
            Err(e) => return Err(e.to_string()),
        }
        paths.iter().map(|path| Store::open(path)).collect()
    }

    pub(crate) fn has_active_job(&self) -> Result<bool, String> {
        for store in self.job_stores()? {
            if store.active_job()?.is_some() { return Ok(true); }
        }
        Ok(false)
    }
}

impl Library<'_> {
    pub(crate) fn owns_account(&self) -> bool { self.scope.owns_account() }
    pub(crate) fn generation(&self) -> u64 { self.scope.generation() }
    pub(crate) fn email(&self) -> Option<&str> { self.scope.email() }

    pub(crate) fn project_pending(&self) -> Result<usize, ApiError> {
        self.core.account.with_library(&self.scope, |journal| Ok(match journal {
            Some(journal) => journal.project_pending(), None => Ok(0),
        })).map_err(unavailable)?.map_err(ApiError::from)
    }

    pub(crate) fn run<T>(&self, operation: impl FnOnce(&Store) -> Result<T, ApiError>) -> Result<T, ApiError> {
        self.core.account.with_library(&self.scope, |_| Ok(operation(&self.store)))
            .map_err(unavailable)?
    }

    pub(crate) fn import_legacy(&self) -> Result<usize, ApiError> {
        self.core.account.with_library(&self.scope, |journal| Ok(match journal {
            Some(journal) => journal.import_legacy(&self.core.store).map_err(|_| api::err(503, "import_failed", "보관함을 가져오지 못했습니다. 원본은 유지했습니다. 다시 시도해 주세요.")),
            None => Err(api::err(409, "account_required", "먼저 Google 계정으로 로그인해 주세요.")),
        })).map_err(unavailable)?
    }

    pub(crate) fn legacy_available(&self) -> Result<bool, ApiError> {
        self.core.account.with_library(&self.scope, |journal| Ok((|| -> Result<bool, String> {
            let Some(journal) = journal else { return Ok(false) };
            if journal.imported_legacy()? { return Ok(false); }
            let legacy = self.core.store.library()?;
            Ok(legacy["videos"].as_array().is_some_and(|v| !v.is_empty())
                || legacy["playlists"].as_array().is_some_and(|p| !p.is_empty()))
        })())).map_err(unavailable)?.map_err(ApiError::from)
    }
}
