pub const DESKTOP_CLIENT_ID: &str =
    "1072382283165-lar1422vmd8fnouh4dlpr771gcc7lq4f.apps.googleusercontent.com";

// Shared by the publisher's build script and tests; runtime only needs the public client ID.
#[allow(dead_code)]
pub fn desktop_secret(raw: &[u8]) -> Result<String, &'static str> {
    if raw.len() > 16 * 1024 || raw.starts_with(&[0xef, 0xbb, 0xbf]) {
        return Err("invalid Google Desktop credential file");
    }
    let value: serde_json::Value =
        serde_json::from_slice(raw).map_err(|_| "invalid Google Desktop credential file")?;
    let installed = &value["installed"];
    if value.get("web").is_some()
        || installed["client_id"] != DESKTOP_CLIENT_ID
        || installed["project_id"] != "gen-lang-client-0430697481"
    {
        return Err("Google Desktop credential registration does not match this app");
    }
    let secret = installed["client_secret"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 256 && s.bytes().all(|b| b.is_ascii_graphic()))
        .ok_or("Google Desktop client secret is missing or invalid")?;
    Ok(secret.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn publisher_credentials_require_the_selected_desktop_registration_and_bounded_secret() {
        let valid = json!({"installed": {
            "client_id": DESKTOP_CLIENT_ID, "project_id": "gen-lang-client-0430697481",
            "client_secret": "offline-fixture-secret", "token_uri": "https://example.org/ignored"
        }});
        let bytes = serde_json::to_vec(&valid).unwrap();
        assert_eq!(desktop_secret(&bytes).unwrap(), "offline-fixture-secret");
        for (field, invalid) in [
            ("client_id", "wrong-client"),
            ("project_id", "wrong-project"),
            ("client_secret", ""),
            ("client_secret", "private\nvalue"),
        ] {
            let mut value = valid.clone();
            value["installed"][field] = invalid.into();
            let error = desktop_secret(&serde_json::to_vec(&value).unwrap()).unwrap_err();
            assert!(!error.contains(invalid) || invalid.is_empty());
        }
        for invalid in [
            json!({"web":valid["installed"]}),
            json!({"installed":{},"web":{}}),
            json!({"installed":{"client_id":DESKTOP_CLIENT_ID,"project_id":"gen-lang-client-0430697481","client_secret":"x".repeat(257)}}),
        ] {
            assert!(desktop_secret(&serde_json::to_vec(&invalid).unwrap()).is_err());
        }
        assert!(desktop_secret(&[&[0xef, 0xbb, 0xbf][..], &bytes].concat()).is_err());
        assert!(desktop_secret(&vec![b' '; 16 * 1024 + 1]).is_err());
        assert!(desktop_secret(b"not JSON or a credential file").is_err());
    }
}
