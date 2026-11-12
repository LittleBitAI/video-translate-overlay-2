//! Device services of the shipped app that do not need Tauri.

use crate::KeyStore;

/// The Gemini key in Windows Credential Manager (per user, encrypted by the OS).
#[cfg(windows)]
pub struct CredentialStore;

#[cfg(windows)]
mod cred {
    use windows::core::{HSTRING, PCWSTR, PWSTR};
    use windows::Win32::Security::Credentials::*;

    const TARGET: &str = "video-translate-overlay/gemini-api-key";

    impl super::KeyStore for super::CredentialStore {
        fn get(&self) -> Result<Option<String>, String> {
            let target = HSTRING::from(TARGET);
            let mut p: *mut CREDENTIALW = std::ptr::null_mut();
            unsafe {
                if CredReadW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None, &mut p).is_err() {
                    return Ok(None); // not found (or unreadable): no key
                }
                let c = &*p;
                let blob = std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize).to_vec();
                CredFree(p as *const _);
                Ok(Some(String::from_utf8(blob).map_err(|_| "stored key is not UTF-8".to_string())?))
            }
        }

        fn set(&self, key: &str) -> Result<(), String> {
            let mut target: Vec<u16> = TARGET.encode_utf16().chain([0]).collect();
            let mut blob = key.as_bytes().to_vec();
            let c = CREDENTIALW {
                Type: CRED_TYPE_GENERIC,
                TargetName: PWSTR(target.as_mut_ptr()),
                CredentialBlobSize: blob.len() as u32,
                CredentialBlob: blob.as_mut_ptr(),
                Persist: CRED_PERSIST_LOCAL_MACHINE,
                ..Default::default()
            };
            unsafe { CredWriteW(&c, 0) }.map_err(|e| format!("Credential Manager: {e}"))
        }

        fn delete(&self) -> Result<(), String> {
            let target = HSTRING::from(TARGET);
            let _ = unsafe { CredDeleteW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None) };
            Ok(())
        }
    }
}

/// Where the Android Keystore binding goes; until it exists the app refuses to
/// keep a key rather than write it in plain text.
#[cfg(not(windows))]
pub struct CredentialStore;

#[cfg(not(windows))]
impl KeyStore for CredentialStore {
    fn get(&self) -> Result<Option<String>, String> {
        Ok(None)
    }
    fn set(&self, _key: &str) -> Result<(), String> {
        Err("no secure key store on this platform yet".into())
    }
    fn delete(&self) -> Result<(), String> {
        Ok(())
    }
}

