//! A bounded Google-session blob sealed to the Windows user with DPAPI.

use crate::google_auth::AuthError;
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::{LocalFree, HLOCAL},
        Security::Cryptography::{
            CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
        },
    },
};

const MAX_PLAINTEXT: usize = 64 * 1024;
const MAX_SEALED: usize = 128 * 1024;

struct Blob(CRYPT_INTEGER_BLOB);
impl Drop for Blob {
    fn drop(&mut self) {
        if !self.0.pbData.is_null() {
            unsafe {
                std::slice::from_raw_parts_mut(self.0.pbData, self.0.cbData as usize).fill(0);
                let _ = LocalFree(Some(HLOCAL(self.0.pbData.cast())));
            }
        }
    }
}

/// The native account provider owns this path; each operation closes all file and DPAPI handles.
/// No IPC command accepts a path or reads the plaintext blob.
pub struct SecretFile {
    path: PathBuf,
}

impl SecretFile {
    pub fn for_folder(folder: &Path) -> Self {
        Self {
            path: folder.join("google-account.sealed"),
        }
    }

    pub fn get(&self) -> Result<Option<String>, AuthError> {
        let file = match std::fs::File::open(&self.path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(AuthError::Unavailable),
        };
        let mut sealed = Vec::new();
        file.take((MAX_SEALED + 1) as u64)
            .read_to_end(&mut sealed)
            .map_err(|_| AuthError::Unavailable)?;
        if sealed.len() > MAX_SEALED {
            return Err(AuthError::Invalid);
        }
        let input = CRYPT_INTEGER_BLOB {
            cbData: sealed.len() as u32,
            pbData: sealed.as_mut_ptr(),
        };
        let mut output = Blob(CRYPT_INTEGER_BLOB::default());
        unsafe {
            CryptUnprotectData(
                &input,
                None,
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output.0,
            )
        }
        .map_err(|_| AuthError::Invalid)?;
        if output.0.cbData as usize > MAX_PLAINTEXT || output.0.pbData.is_null() {
            return Err(AuthError::Invalid);
        }
        let plain =
            unsafe { std::slice::from_raw_parts(output.0.pbData, output.0.cbData as usize) };
        Ok(Some(
            std::str::from_utf8(plain)
                .map_err(|_| AuthError::Invalid)?
                .to_string(),
        ))
    }

    pub fn set(&self, value: &str) -> Result<(), AuthError> {
        if value.len() > MAX_PLAINTEXT {
            return Err(AuthError::Invalid);
        }
        let input = CRYPT_INTEGER_BLOB {
            cbData: value.len() as u32,
            pbData: value.as_ptr() as *mut u8,
        };
        let mut output = Blob(CRYPT_INTEGER_BLOB::default());
        unsafe {
            CryptProtectData(
                &input,
                PCWSTR::null(),
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output.0,
            )
        }
        .map_err(|_| AuthError::Unavailable)?;
        if output.0.cbData as usize > MAX_SEALED || output.0.pbData.is_null() {
            return Err(AuthError::Invalid);
        }
        let sealed =
            unsafe { std::slice::from_raw_parts(output.0.pbData, output.0.cbData as usize) };
        let parent = self.path.parent().ok_or(AuthError::Invalid)?;
        std::fs::create_dir_all(parent).map_err(|_| AuthError::Unavailable)?;
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).map_err(|_| AuthError::Unavailable)?;
        let suffix: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let temporary = parent.join(format!("google-account-{suffix}.tmp"));
        let result = (|| {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(|_| AuthError::Unavailable)?;
            file.write_all(sealed).map_err(|_| AuthError::Unavailable)?;
            file.sync_all().map_err(|_| AuthError::Unavailable)?;
            drop(file);
            std::fs::rename(&temporary, &self.path).map_err(|_| AuthError::Unavailable)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result
    }

    pub fn delete(&self) -> Result<(), AuthError> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(AuthError::Unavailable),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Profile(PathBuf);
    impl Profile {
        fn new() -> Self {
            let mut random = [0u8; 16];
            getrandom::fill(&mut random).unwrap();
            let name: String = random.iter().map(|b| format!("{b:02x}")).collect();
            let dir = std::env::temp_dir().join(format!("vto-account-secrets-{name}"));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }
    impl Drop for Profile {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn sealed_sessions_survive_reopen_and_replacement_without_plaintext_or_unrelated_changes() {
        let profile = Profile::new();
        let store = SecretFile::for_folder(&profile.0);
        std::fs::write(profile.0.join("unrelated.txt"), b"keep").unwrap();
        assert!(store.get().unwrap().is_none());
        let plain = "offline-test-google-session".repeat(200);
        store.set(&plain).unwrap();
        assert_eq!(
            SecretFile::for_folder(&profile.0).get().unwrap().as_deref(),
            Some(plain.as_str())
        );
        let sealed = std::fs::read(&store.path).unwrap();
        assert!(!sealed
            .windows(26)
            .any(|part| part == b"offline-test-google-session"));
        store.set("replacement test session").unwrap();
        assert_eq!(
            store.get().unwrap().as_deref(),
            Some("replacement test session")
        );
        assert_eq!(std::fs::read_dir(&profile.0).unwrap().count(), 2);
        store.delete().unwrap();
        store.delete().unwrap();
        assert!(store.get().unwrap().is_none());
        assert_eq!(
            std::fs::read(profile.0.join("unrelated.txt")).unwrap(),
            b"keep"
        );
    }

    #[test]
    fn damaged_and_unbounded_blobs_are_refused_without_replacing_the_previous_session() {
        let profile = Profile::new();
        let store = SecretFile::for_folder(&profile.0);
        store.set("original test session").unwrap();
        assert_eq!(
            store.set(&"x".repeat(MAX_PLAINTEXT + 1)),
            Err(AuthError::Invalid)
        );
        assert_eq!(
            store.get().unwrap().as_deref(),
            Some("original test session")
        );
        let mut sealed = std::fs::read(&store.path).unwrap();
        let last = sealed.len() - 1;
        sealed[last] ^= 1;
        std::fs::write(&store.path, sealed).unwrap();
        assert!(matches!(store.get(), Err(AuthError::Invalid)));
        std::fs::write(&store.path, vec![0; MAX_SEALED + 1]).unwrap();
        assert!(matches!(store.get(), Err(AuthError::Invalid)));
    }
}
