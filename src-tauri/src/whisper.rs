//! The on-device speech model (A4): whisper.cpp running Whisper
//! large-v3-turbo (q5_0, about 0.6 GB), fetched on the first job into the
//! app's data folder and checked against a pinned SHA-256.

use crate::http::{self, Request};
use crate::{Segment, Transcriber};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub const MODEL_FILE: &str = "ggml-large-v3-turbo-q5_0.bin";
pub const MODEL_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin";
pub const MODEL_SHA256: &str = "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2";
pub const MODEL_BYTES: u64 = 574_041_195;

pub struct Whisper {
    /// The model file. It is known from the start, so a process restarted in
    /// the writing stage loads it without passing through downloading again.
    path: PathBuf,
    /// Whether `prepare` may fetch the file (false for `VTO_WHISPER_MODEL`).
    fetch: bool,
    ctx: Mutex<Option<WhisperContext>>,
    language: Mutex<Option<String>>,
}

impl Whisper {
    /// The model under `data_dir/models`, fetched on first use; or `model_override` as is.
    pub fn new(data_dir: &Path, model_override: Option<PathBuf>) -> Self {
        whisper_rs::install_logging_hooks(); // whisper.cpp's own logging off: it prints per call
        let fetch = model_override.is_none();
        let path = model_override.unwrap_or_else(|| data_dir.join("models").join(MODEL_FILE));
        Whisper { path, fetch, ctx: Mutex::new(None), language: Mutex::new(None) }
    }

    fn context(&self) -> Result<std::sync::MutexGuard<'_, Option<WhisperContext>>, String> {
        let mut ctx = self.ctx.lock().unwrap();
        if ctx.is_none() {
            let c = WhisperContext::new_with_params(&self.path, WhisperContextParameters::default())
                .map_err(|e| format!("cannot load {}: {e}", self.path.display()))?;
            *ctx = Some(c);
        }
        Ok(ctx)
    }
}

fn fetch_model(dest: &Path, progress: &mut dyn FnMut(f32)) -> Result<(), String> {
    let part = dest.with_extension("part");
    let mut req = Request::get(MODEL_URL);
    req.timeout_ms = 120_000;
    let mut r = http::send(req).map_err(|e| format!("model download: {e}"))?;
    if r.status != 200 {
        return Err(format!("model download: HTTP {}", r.status));
    }
    let mut out = std::io::BufWriter::new(std::fs::File::create(&part).map_err(|e| e.to_string())?);
    let mut hash = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut done = 0u64;
    loop {
        let n = r.read(&mut buf).map_err(|e| format!("model download: {e}"))?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
        out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        done += n as u64;
        progress(done as f32 / MODEL_BYTES as f32);
    }
    out.flush().map_err(|e| e.to_string())?;
    drop(out);
    let got: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if got != MODEL_SHA256 {
        let _ = std::fs::remove_file(&part);
        return Err(format!("model download: SHA-256 {got} is not the pinned {MODEL_SHA256}"));
    }
    std::fs::rename(&part, dest).map_err(|e| e.to_string())
}

impl Transcriber for Whisper {
    fn prepare(&self, _data_dir: &Path, progress: &mut dyn FnMut(f32)) -> Result<bool, String> {
        // The start of a job's downloading or writing: its language is detected
        // afresh on the first window it writes.
        *self.language.lock().unwrap() = None;
        if !self.fetch {
            return if self.path.is_file() { Ok(false) } else { Err(format!("model file missing: {}", self.path.display())) };
        }
        if std::fs::metadata(&self.path).is_ok_and(|m| m.len() == MODEL_BYTES) {
            return Ok(false);
        }
        std::fs::create_dir_all(self.path.parent().unwrap()).map_err(|e| e.to_string())?;
        let mut tries = 0;
        while let Err(e) = fetch_model(&self.path, progress) {
            tries += 1;
            if tries >= 3 {
                return Err(e);
            }
            crate::log(format!("{e}; retrying"));
            std::thread::sleep(Duration::from_secs(5 * tries));
        }
        Ok(true)
    }

    fn transcribe(&self, pcm: &[f32], progress: &mut dyn FnMut(f32)) -> Result<Vec<Segment>, String> {
        let ctx = self.context()?;
        let mut state = ctx.as_ref().unwrap().create_state().map_err(|e| e.to_string())?;
        // Accuracy first (R24): beam search. The language is detected on the job's
        // first window and kept: per-window detection flips on short or accented
        // windows and then writes English speech out in another script.
        let known = self.language.lock().unwrap().clone();
        let mut params = FullParams::new(SamplingStrategy::BeamSearch { beam_size: 5, patience: -1.0 });
        params.set_language(Some(known.as_deref().unwrap_or("auto")));
        params.set_n_threads(std::thread::available_parallelism().map(|n| n.get().min(8)).unwrap_or(4) as i32);
        params.set_suppress_blank(true);
        params.set_suppress_nst(true);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_special(false);
        params.set_print_timestamps(false);
        let percent = Arc::new(AtomicI32::new(0));
        let seen = percent.clone();
        params.set_progress_callback_safe(move |p: i32| seen.store(p, Ordering::Relaxed));
        let result = std::thread::scope(|s| {
            let run = s.spawn(|| state.full(params, pcm).map(|_| ()));
            while !run.is_finished() {
                progress(percent.load(Ordering::Relaxed) as f32 / 100.0);
                std::thread::sleep(Duration::from_millis(250));
            }
            run.join().map_err(|_| "whisper.cpp panicked".to_string())
        })?;
        result.map_err(|e| e.to_string())?;
        if known.is_none() {
            *self.language.lock().unwrap() = whisper_rs::get_lang_str(state.full_lang_id_from_state()).map(String::from);
        }
        let segs = state
            .as_iter()
            .map(|s| Segment {
                start_ms: s.start_timestamp() * 10,
                end_ms: s.end_timestamp() * 10,
                text: s.to_str_lossy().map(|t| t.into_owned()).unwrap_or_default(),
            })
            .collect();
        progress(1.0);
        Ok(segs)
    }

    fn language(&self) -> Option<String> {
        self.language.lock().unwrap().clone()
    }
}
