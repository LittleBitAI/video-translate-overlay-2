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
