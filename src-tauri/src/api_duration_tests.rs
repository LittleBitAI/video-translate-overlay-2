//! 영상감지 pressed while YouTube plays an ad sends the ad's length; the job
//! keeps the lecture's own, which YouTube's stream URL carries (`dur`), so the
//! overlay does not take the lecture for an ad afterwards (R18).

use super::*;
use crate::{AudioExtractor, Browser, Config, KeyStore, Notifier, Services, Transcriber};
use std::io::{BufRead, BufReader, Write};

struct Fake(String);
impl KeyStore for Fake {
    fn get(&self) -> Result<Option<String>, String> {
        Ok(Some("AIzaSyD-test-key-0123".into()))
    }
    fn set(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn delete(&self) -> Result<(), String> {
        Ok(())
    }
}
impl Browser for Fake {
    fn navigate(&self, _: &str, _: bool) -> Result<(), String> {
        Ok(())
    }
    fn clear_site_data(&self) -> Result<(), String> {
        Ok(())
    }
}
impl Notifier for Fake {
    fn ongoing(&self, _: &str, _: &str, _: &str, _: i64) {}
    fn ready(&self, _: &str, _: &str, _: &str) {}
}
impl Transcriber for Fake {
    fn transcribe(&self, _: &[f32], _: &mut dyn FnMut(f32)) -> Result<Vec<crate::Segment>, String> {
        Ok(Vec::new())
    }
}
impl AudioExtractor for Fake {
    fn audio_url(&self, _: &str) -> Result<String, media::Refusal> {
        Ok(self.0.clone())
    }
}

/// Serves a one-second silent WAV on every request.
pub(super) fn serve_wav() -> String {
    let mut wav = b"RIFF\x00\x00\x00\x00WAVEfmt \x10\x00\x00\x00\x01\x00\x01\x00\x80\x3e\x00\x00\x00\x7d\x00\x00\x02\x00\x10\x00data".to_vec();
    wav.extend_from_slice(&32_000u32.to_le_bytes());
    wav.resize(wav.len() + 32_000, 0);
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let at = server.local_addr().unwrap();
    std::thread::spawn(move || {
        for s in server.incoming() {
            let Ok(s) = s else { continue };
            let mut r = BufReader::new(s.try_clone().unwrap());
            loop {
                let mut l = String::new();
                if r.read_line(&mut l).unwrap_or(0) == 0 || l.trim().is_empty() {
                    break;
                }
            }
            let head = format!("HTTP/1.1 200 OK\r\nContent-Type: audio/wav\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", wav.len());
            let _ = (&s).write_all(head.as_bytes());
            let _ = (&s).write_all(&wav);
        }
    });
    format!("http://{at}")
}

#[test]
fn a_youtube_job_keeps_the_streams_length_not_the_players() {
    let dir = std::env::temp_dir().join(format!("vto-dur-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let stream = format!("{}/videoplayback?itag=140&dur=3601.250&mime=audio%2Fmp4", serve_wav());
    let svc = |u: &str| Services {
        keystore: Box::new(Fake(u.into())),
        browser: Box::new(Fake(u.into())),
        notifier: Box::new(Fake(u.into())),
        transcriber: Box::new(Fake(u.into())),
        extractor: Box::new(Fake(u.into())),
    };
    let core = Core::start(Config { data_dir: dir.clone(), gemini_base_url: String::new(), allow_loopback_media: true }, svc(&stream)).unwrap();
    // The 30 s an ad's player reports.
    let r = create_job(&core, "https://www.youtube.com/watch?v=dQw4w9WgXcQ", None, "A lecture", 30_000).unwrap();
    let key = r["video_key"].as_str().unwrap();
    assert_eq!(core.store.video(key).unwrap().unwrap().duration_ms, 3_601_250);

    assert_eq!(youtube::stream_duration_ms("https://r1.googlevideo.com/videoplayback?itag=140"), None);
    assert_eq!(youtube::stream_duration_ms("https://r1.googlevideo.com/videoplayback?dur=0.000"), None);
}

#[test]
fn resumed_youtube_source_refreshes_unavailable_hints_but_preserves_refusals() {
    use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
    struct Fresh {
        url: String,
        calls: Arc<AtomicUsize>,
    }
    impl AudioExtractor for Fresh {
        fn audio_url(&self, _: &str) -> Result<String, media::Refusal> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.url.clone())
        }
    }
    let hint = "https://rr1.googlevideo.com/videoplayback?source=youtube&mime=audio%2Fmp4&dur=60&expire=1";
    let page = url::Url::parse("https://www.youtube.com/watch?v=dQw4w9WgXcQ").unwrap();
    let fresh = format!("{}/audio.wav", serve_wav());
    // Only availability failures refresh a saved hint; policy and content refusals stay refusals.
    for (case, failure, expected_calls) in [
        ("working", None, 0),
        ("expired", Some("unreachable"), 1),
        ("drm", Some("drm"), 0),
        ("private", Some("private_address"), 0),
        ("unsupported", Some("not_audio"), 0),
    ] {
        let dir = std::env::temp_dir().join(format!("vto-source-{case}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("app.sqlite3");
        {
            let s = crate::store::Store::open(&path).unwrap();
            s.create_job("j", "v", page.as_str(), "youtube", "Lecture", 60_000, Some(hint), "m").unwrap();
        }
        let calls = Arc::new(AtomicUsize::new(0));
        let core = Core {
            updates: Default::default(),
            cfg: Config { data_dir: dir, gemini_base_url: String::new(), allow_loopback_media: true },
            store: crate::store::Store::open(&path).unwrap(),
            svc: Services {
                keystore: Box::new(Fake(String::new())),
                browser: Box::new(Fake(String::new())),
                notifier: Box::new(Fake(String::new())),
                transcriber: Box::new(Fake(String::new())),
                extractor: Box::new(Fresh { url: fresh.clone(), calls: calls.clone() }),
            },
            wake: Default::default(),
            create_lock: Default::default(),
        };
        let resumed = core.store.job("j").unwrap().unwrap();
        assert!(youtube::browser_audio_url(resumed.media_url.as_deref().unwrap()).is_some());
        let mut probed = Vec::new();
        let result = resolve_source_with(&core, &page, resumed.media_url.as_deref(), |raw| {
            probed.push(raw.to_string());
            if raw == hint {
                if let Some(code) = failure { return Err(media::refuse(code, "fixture refusal")); }
                return Ok(media::Source::File { url: raw.into(), ext: "m4a" });
            }
            assert_eq!(raw, fresh);
            let url = media::check_media_url(raw, true)?;
            media::probe(&url, true)
        });
        assert_eq!(calls.load(Ordering::SeqCst), expected_calls, "{case}");
        if failure.is_none() || failure == Some("unreachable") {
            let media::Source::File { url, .. } = result.unwrap() else { panic!("expected a file") };
            assert_eq!(url, if expected_calls == 1 { fresh.as_str() } else { hint }, "{case}");
        } else {
            assert_eq!(result.unwrap_err().code, failure.unwrap(), "{case}");
        }
        assert_eq!(probed.len(), expected_calls + 1, "{case}");
    }
}
