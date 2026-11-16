//! On-device speech: quantized multilingual base on Android, large-v3-turbo
//! on desktop. Model downloads are checked against a pinned SHA-256.

use crate::http::{self, Request};
use crate::{Segment, Transcriber, Word};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperSegment, WhisperState};

pub const MODEL_FILE: &str = if cfg!(target_os = "android") { "ggml-base-q5_1.bin" } else { "ggml-large-v3-turbo-q5_0.bin" };
pub const MODEL_URL: &str = if cfg!(target_os = "android") {
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base-q5_1.bin"
} else {
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin"
};
pub const MODEL_SHA256: &str = if cfg!(target_os = "android") {
    "422f1ae452ade6f30a004d7e5c6a43195e4433bc370bf23fac9cc591f01a8898"
} else {
    "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2"
};
pub const MODEL_BYTES: u64 = if cfg!(target_os = "android") { 59_707_625 } else { 574_041_195 };

pub struct Whisper {
    /// The model file. It is known from the start, so a process restarted in
    /// the writing stage loads it without passing through downloading again.
    path: PathBuf,
    /// Whether `prepare` may fetch the file (false for `VTO_WHISPER_MODEL`).
    fetch: bool,
    // A state owns its model context and reusable compute buffers; with the
    // model's first special token (end of text), below which tokens are text.
    state: Mutex<Option<(WhisperState, i32)>>,
    language: Mutex<Option<String>>,
    mobile: bool,
}

impl Whisper {
    /// The model under `data_dir/models`, fetched on first use; or `model_override` as is.
    pub fn new(data_dir: &Path, model_override: Option<PathBuf>) -> Self {
        whisper_rs::install_logging_hooks(); // whisper.cpp's own logging off: it prints per call
        let fetch = model_override.is_none();
        let path = model_override.unwrap_or_else(|| data_dir.join("models").join(MODEL_FILE));
        Whisper { path, fetch, state: Mutex::new(None), language: Mutex::new(None), mobile: cfg!(target_os = "android") }
    }

    /// The phone's decoding (greedy, no temperature retries, at most 4 threads)
    /// on any OS: the timing harness compares plans with the phone's output on the PC.
    pub fn with_mobile_params(mut self) -> Self {
        self.mobile = true;
        self
    }

    fn state(&self) -> Result<std::sync::MutexGuard<'_, Option<(WhisperState, i32)>>, String> {
        let mut state = self.state.lock().unwrap();
        if state.is_none() {
            let c = WhisperContext::new_with_params(&self.path, WhisperContextParameters::default())
                .map_err(|e| format!("cannot load {}: {e}", self.path.display()))?;
            let eot = c.token_eot();
            *state = Some((c.create_state().map_err(|e| e.to_string())?, eot));
        }
        Ok(state)
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
        if !cpu_runs_model() {
            return Err(crate::pipeline::UNSUPPORTED_CPU.into());
        }
        if !self.fetch {
            return if self.path.is_file() { Ok(false) } else { Err(format!("model file missing: {}", self.path.display())) };
        }
        if !self.needs_fetch() {
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

    fn needs_fetch(&self) -> bool {
        self.fetch && !std::fs::metadata(&self.path).is_ok_and(|m| m.len() == MODEL_BYTES)
    }

    fn load(&self) -> Result<(), String> {
        self.state().map(|_| ())
    }

    fn transcribe(&self, pcm: &[f32], progress: &mut dyn FnMut(f32)) -> Result<Vec<Segment>, String> {
        let mut state_guard = self.state()?;
        let (state, eot) = state_guard.as_mut().unwrap();
        let eot = *eot;
        // Keep the detected language across windows, without carrying text into another job.
        let known = self.language.lock().unwrap().clone();
        let mut params = inference_params(self.mobile);
        params.set_language(Some(known.as_deref().unwrap_or("auto")));
        params.set_no_context(true);
        params.set_suppress_blank(true);
        params.set_suppress_nst(true);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_special(false);
        params.set_print_timestamps(false);
        // Word times, for reconciling two windows that both heard a cut (decoding is unchanged).
        params.set_token_timestamps(true);
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
        let duration_ms = (pcm.len() * 1000 / crate::audio::RATE) as i64;
        let segs = state
            .as_iter()
            .map(|s| Segment {
                start_ms: (s.start_timestamp() * 10).clamp(0, duration_ms),
                end_ms: (s.end_timestamp() * 10).clamp(0, duration_ms),
                text: s.to_str_lossy().map(|t| t.into_owned()).unwrap_or_default(),
                words: words(&s, eot, duration_ms),
            })
            .filter(|s| s.end_ms > s.start_ms && !s.text.trim().is_empty())
            .collect();
        progress(1.0);
        Ok(segs)
    }

    fn language(&self) -> Option<String> {
        self.language.lock().unwrap().clone()
    }
}

/// Android's speech engine is built for ARMv8.2 with dot product and half
/// precision (`src-tauri/android/whisper.toolchain.cmake`): about 30 % faster
/// on a Galaxy S21+, and a CPU without them would stop at the first one.
#[cfg(all(target_os = "android", target_arch = "aarch64"))]
fn cpu_runs_model() -> bool {
    std::arch::is_aarch64_feature_detected!("dotprod") && std::arch::is_aarch64_feature_detected!("fp16")
}

#[cfg(not(all(target_os = "android", target_arch = "aarch64")))]
fn cpu_runs_model() -> bool {
    true
}

/// A segment's words from its tokens: a word starts at a token that begins
/// with whitespace, or with a kana or CJK ideograph (written without spaces);
/// special tokens (from end of text up) are left out.
fn words(s: &WhisperSegment, eot: i32, duration_ms: i64) -> Vec<Word> {
    let mut out: Vec<(i64, i64, Vec<u8>)> = Vec::new();
    for i in 0..s.n_tokens() {
        let Some(t) = s.get_token(i) else { continue };
        let Ok(bytes) = t.to_bytes() else { continue };
        if t.token_id() >= eot || bytes.is_empty() {
            continue;
        }
        let d = t.token_data();
        let (t0, t1) = ((d.t0 * 10).clamp(0, duration_ms), (d.t1 * 10).clamp(0, duration_ms));
        let cjk = std::str::from_utf8(bytes).ok().and_then(|x| x.chars().next()).is_some_and(|c| ('\u{3040}'..='\u{30ff}').contains(&c) || ('\u{3400}'..='\u{9fff}').contains(&c));
        match out.last_mut() {
            Some(w) if !(bytes[0].is_ascii_whitespace() || cjk) => {
                w.1 = w.1.max(t1);
                w.2.extend_from_slice(bytes);
            }
            _ => out.push((t0, t1.max(t0), bytes.to_vec())),
        }
    }
    out.into_iter().map(|(a, b, t)| Word { start_ms: a, end_ms: b, text: String::from_utf8_lossy(&t).into_owned() }).collect()
}

fn inference_params(mobile: bool) -> FullParams<'static, 'static> {
    let strategy = if mobile { SamplingStrategy::Greedy { best_of: 1 } } else { SamplingStrategy::BeamSearch { beam_size: 5, patience: -1.0 } };
    let mut params = FullParams::new(strategy);
    params.set_n_threads(std::thread::available_parallelism().map(|n| n.get().min(if mobile { 4 } else { 8 })).unwrap_or(4) as i32);
    if mobile {
        // Six temperature passes and five decoder candidates make difficult speech stall a phone.
        params.set_temperature_inc(0.0);
    }
    params
}

#[cfg(test)]
#[path = "whisper_tests.rs"]
mod tests;
