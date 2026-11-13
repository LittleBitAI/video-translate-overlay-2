//! The app in a browser: the built screens and the API on one port.
//!
//! `PORT` is the port (on 127.0.0.1); `APP_DATA_DIR` holds all the app's data
//! (SQLite, downloads, the model); `VTO_DIST` the built screens (default: the
//! repository's `dist/`, from `npm run build`). `scripts/live.mjs` builds both
//! and starts it.
//!
//! The site a person opens is shown by the first screen itself, so the "browser"
//! here only accepts the address (P1 records it for P9). Notices are the first
//! screen's: it polls P7. The on-device model is in the build with the
//! `whisper` feature; without it a job stops at writing as `download_failed`.
//!
//! `--stand-ins` swaps the device and network pieces for stand-ins, the way the
//! contract host does (see [`stand_in`]), so every state of the screens can be
//! seen without a Gemini key, the model or YouTube. Never use it for real work.

use std::path::PathBuf;
use std::sync::Arc;
#[cfg(not(feature = "whisper"))]
use vto::Segment;
use vto::{youtube, AudioExtractor, Browser, Config, Core, KeyStore, Notifier, Services, Transcriber};

struct Page;

impl Browser for Page {
    fn navigate(&self, _url: &str, _overlay: bool) -> Result<(), String> {
        Ok(())
    }
    /// The browser's cookies are its own: only the YouTube account screen is reset (by P2).
    fn clear_site_data(&self) -> Result<(), String> {
        Ok(())
    }
}

struct Log;

impl Notifier for Log {
    fn ongoing(&self, job_id: &str, state: &str, _title: &str, percent: i64) {
        vto::log(format!("job {job_id}: {state} {percent}%"));
    }
    fn ready(&self, job_id: &str, _text: &str, _url: &str) {
        vto::log(format!("job {job_id}: subtitles ready"));
    }
}

#[cfg(not(feature = "whisper"))]
struct NoModel;

#[cfg(not(feature = "whisper"))]
impl Transcriber for NoModel {
    fn prepare(&self, _: &std::path::Path, _: &mut dyn FnMut(f32)) -> Result<bool, String> {
        Err("this build has no on-device model (build vto-web with --features whisper)".into())
    }
    fn transcribe(&self, _: &[f32], _: &mut dyn FnMut(f32)) -> Result<Vec<Segment>, String> {
        Err("no on-device model".into())
    }
}

fn main() {
    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).expect("PORT: the port to serve on");
    let data_dir = PathBuf::from(std::env::var_os("APP_DATA_DIR").expect("APP_DATA_DIR: the app's data folder"));
    std::fs::create_dir_all(&data_dir).expect("create APP_DATA_DIR");
    let dist = std::env::var_os("VTO_DIST").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dist"));
    assert!(dist.join("index.html").is_file(), "no built screens at {} (npm run build)", dist.display());

    let config = |gemini_base_url: String, allow_loopback_media| Config { data_dir: data_dir.clone(), gemini_base_url, allow_loopback_media };
    let services = |keystore: Box<dyn KeyStore>, transcriber: Box<dyn Transcriber>, extractor: Box<dyn AudioExtractor>| Services {
        keystore,
        browser: Box::new(Page),
        notifier: Box::new(Log),
        transcriber,
        extractor,
    };
    let (cfg, svc) = if std::env::args().any(|a| a == "--stand-ins") {
        let base = stand_in::serve();
        vto::log(format!("stand-ins: Gemini, YouTube's audio and the model are fakes at {base}"));
        let lecture = Box::new(stand_in::Lecture(format!("{base}/lecture.wav")));
        (config(base, true), services(Box::new(stand_in::FileKey(data_dir.join("stand-in-key"))), Box::<stand_in::Lines>::default(), lecture))
    } else {
        #[cfg(feature = "whisper")]
        let transcriber: Box<dyn Transcriber> = Box::new(vto::whisper::Whisper::new(&data_dir, std::env::var_os("VTO_WHISPER_MODEL").map(Into::into)));
        #[cfg(not(feature = "whisper"))]
        let transcriber: Box<dyn Transcriber> = Box::new(NoModel);
        let keystore = Box::new(vto::platform::CredentialStore::for_folder(&data_dir));
        (config(vto::GEMINI_BASE_URL.into(), false), services(keystore, transcriber, Box::new(youtube::InnertubeExtractor)))
    };
    let core = Core::start(cfg, svc).expect("start the app core");
    let listener = std::net::TcpListener::bind(("127.0.0.1", port)).expect("bind 127.0.0.1:PORT");
    vto::log(format!("serving http://127.0.0.1:{port}/ from {}", dist.display()));
    vto::web::serve(listener, Arc::clone(&core), dist);
}

/// `--stand-ins`: what the contract host fakes, faked the same way here, so the
/// seed can fill the app (a saved key, a video with its subtitles ready) with no
/// Gemini key, model or network. The rest (P1-P9, SQLite, the worker, decoding,
/// voice activity, timing, cost) is the app's own code.
mod stand_in {
    use serde_json::{json, Value};
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use vto::{media::Refusal, AudioExtractor, KeyStore, Segment, Transcriber};

    /// What the lecture says, one line per stretch of speech, and its Korean.
    const LINES: [(&str, &str); 3] = [
        ("Attention is all you need.", "어텐션만 있으면 됩니다."),
        ("Each token looks at every other token.", "토큰 하나하나가 다른 모든 토큰을 봅니다."),
        ("That is how the transformer learns context.", "트랜스포머는 그렇게 문맥을 배웁니다."),
    ];
    /// Where the lecture speaks, in seconds.
    const SPEECH: [(f32, f32); 3] = [(0.0, 2.6), (4.4, 7.2), (9.0, 11.8)];
    const LENGTH_S: f32 = 13.0;

    /// The key store: a file in the data folder.
    pub struct FileKey(pub PathBuf);

    impl KeyStore for FileKey {
        fn get(&self) -> Result<Option<String>, String> {
            Ok(std::fs::read_to_string(&self.0).ok().filter(|s| !s.is_empty()))
        }
        fn set(&self, key: &str) -> Result<(), String> {
            std::fs::write(&self.0, key).map_err(|e| e.to_string())
        }
        fn delete(&self) -> Result<(), String> {
            match std::fs::remove_file(&self.0) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
                _ => Ok(()),
            }
        }
    }

    /// The model: each stretch of speech it is handed is the lecture's next line.
    #[derive(Default)]
    pub struct Lines(AtomicUsize);

    impl Transcriber for Lines {
        fn transcribe(&self, pcm: &[f32], progress: &mut dyn FnMut(f32)) -> Result<Vec<Segment>, String> {
            progress(1.0);
            let text = LINES[self.0.fetch_add(1, Ordering::SeqCst) % LINES.len()].0.to_string();
            Ok(vec![Segment { start_ms: 0, end_ms: (pcm.len() as i64) * 1000 / 16_000, text }])
        }
        fn language(&self) -> Option<String> {
            Some("en".into())
        }
    }

    /// YouTube: every video's audio is the lecture.
    pub struct Lecture(pub String);

    impl AudioExtractor for Lecture {
        fn audio_url(&self, _page_url: &str) -> Result<String, Refusal> {
            Ok(self.0.clone())
        }
    }

    /// Serves Gemini (any key of the right shape works; replies translate `LINES`)
    /// and `/lecture.wav` on 127.0.0.1; returns its base URL.
    pub fn serve() -> String {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind the stand-ins");
        let base = format!("http://{}", listener.local_addr().expect("stand-ins address"));
        std::thread::spawn(move || {
            for s in listener.incoming().flatten() {
                std::thread::spawn(move || {
                    let _ = answer(s);
                });
            }
        });
        base
    }

    fn answer(mut s: TcpStream) -> std::io::Result<()> {
        let mut r = BufReader::new(s.try_clone()?);
        let mut line = String::new();
        r.read_line(&mut line)?;
        let mut words = line.split_whitespace();
        let (method, path) = (words.next().unwrap_or("").to_string(), words.next().unwrap_or("").to_string());
        let mut len = 0usize;
        loop {
            let mut h = String::new();
            if r.read_line(&mut h)? == 0 || h.trim().is_empty() {
                break;
            }
            if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
                len = v.trim().parse().unwrap_or(0);
            }
        }
        let mut body = vec![0; len.min(1 << 20)];
        r.read_exact(&mut body)?;
        let (status, kind, bytes) = if path == "/lecture.wav" {
            ("200 OK", "audio/wav", wav())
        } else if method == "POST" && path.ends_with(":generateContent") {
            ("200 OK", "application/json", reply(&body).to_string().into_bytes())
        } else if method == "GET" && path.starts_with("/v1beta/models/") {
            ("200 OK", "application/json", json!({"name": path.trim_start_matches("/v1beta/")}).to_string().into_bytes())
        } else {
            ("404 Not Found", "text/plain", b"no such stand-in".to_vec())
        };
        write!(s, "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", bytes.len())?;
        if method != "HEAD" {
            s.write_all(&bytes)?;
        }
        s.flush()
    }

    /// Gemini's answer to a glossary or translation order.
    pub(super) fn reply(body: &[u8]) -> Value {
        let req: Value = serde_json::from_slice(body).unwrap_or_default();
        let order: Value = serde_json::from_str(req["contents"][0]["parts"][0]["text"].as_str().unwrap_or("")).unwrap_or_default();
        let answer = if order["task"] == "glossary" {
            json!({"terms": [{"source": "attention", "target": "어텐션"}, {"source": "token", "target": "토큰"}, {"source": "transformer", "target": "트랜스포머"}]})
        } else {
            let segs: Vec<Value> = order["segments"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(|s| {
                    let text = s["text"].as_str().unwrap_or("");
                    let ko = LINES.iter().find(|(en, _)| *en == text).map_or(text, |(_, ko)| ko);
                    json!({"id": s["id"], "ko": ko})
                })
                .collect();
            json!({"segments": segs})
        };
        let text = answer.to_string();
        json!({
            "candidates": [{"content": {"parts": [{"text": text}]}}],
            "usageMetadata": {"promptTokenCount": body.len() / 4, "candidatesTokenCount": text.len() / 4},
        })
    }

    /// The lecture: 16 kHz mono 16-bit, a voice-like hum (220 Hz, swelling four
    /// times a second) where it speaks over a faint noise floor.
    pub(super) fn wav() -> Vec<u8> {
        let n = (LENGTH_S * 16_000.0) as usize;
        let mut seed = 1u32;
        let mut out = Vec::with_capacity(44 + 2 * n);
        let data = (2 * n) as u32;
        // RIFF header: PCM, mono, 16 kHz, 32,000 bytes a second, 2-byte frames of 16 bits.
        let (riff, fmt_len, pcm, mono) = ((36 + data).to_le_bytes(), 16u32.to_le_bytes(), 1u16.to_le_bytes(), 1u16.to_le_bytes());
        let (rate, bytes_per_s, frame, bits) = (16_000u32.to_le_bytes(), 32_000u32.to_le_bytes(), 2u16.to_le_bytes(), 16u16.to_le_bytes());
        for part in [&b"RIFF"[..], &riff, b"WAVEfmt ", &fmt_len, &pcm, &mono, &rate, &bytes_per_s, &frame, &bits, b"data", &data.to_le_bytes()] {
            out.extend_from_slice(part);
        }
        for i in 0..n {
            let t = i as f32 / 16_000.0;
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let noise = ((seed >> 16) as f32 / 32_768.0 - 1.0) * 0.003;
            let voice = if SPEECH.iter().any(|(a, b)| (*a..*b).contains(&t)) {
                0.3 * (std::f32::consts::TAU * 220.0 * t).sin() * (0.6 + 0.4 * (std::f32::consts::TAU * 4.0 * t).sin())
            } else {
                0.0
            };
            out.extend_from_slice(&(((voice + noise) * 32_767.0) as i16).to_le_bytes());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::stand_in::{reply, wav};
    use serde_json::{json, Value};
    use vto::gemini;

    #[test]
    fn the_stand_in_lecture_speaks_three_times_and_gemini_translates_it() {
        let wav = wav();
        assert_eq!(&wav[..4], b"RIFF");
        let pcm: Vec<i16> = wav[44..].chunks(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect();
        let regions: Vec<(i64, i64)> = vto::audio::speech_regions(&pcm).iter().map(|r| (r.start_ms(), r.end_ms())).collect();
        assert_eq!(regions.len(), 3, "{regions:?}");
        assert_eq!(regions[0].0, 0, "the first line starts with the video, where a still shot sees it");

        let ask = |o: Value| {
            let r = reply(json!({"contents": [{"parts": [{"text": o.to_string()}]}]}).to_string().as_bytes());
            serde_json::from_str::<Value>(r["candidates"][0]["content"]["parts"][0]["text"].as_str().unwrap()).unwrap()
        };
        assert!(!gemini::parse_terms(&ask(gemini::glossary_order("t", &[]))).is_empty());
        let asked = vec![(7, "Attention is all you need.".to_string())];
        assert_eq!(gemini::parse_translations(&ask(gemini::translate_order(&[], &asked)), &asked), vec![(7, "어텐션만 있으면 됩니다.".to_string())]);
    }
}
