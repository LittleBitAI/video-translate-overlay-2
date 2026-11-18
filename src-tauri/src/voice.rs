//! Speech evidence on the original window clock; never concatenate or move audio.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use whisper_rs::{WhisperVadContext, WhisperVadContextParams, WhisperVadParams};

const FILE: &str = "ggml-silero-v6.2.0.bin";
const URL: &str = "https://huggingface.co/ggml-org/whisper-vad/resolve/main/ggml-silero-v6.2.0.bin";
const BYTES: u64 = 885_098;
const SHA256: &str = "2aa269b785eeb53a82983a20501ddf7c1d9c48e33ab63a41391ac6c9f7fb6987";

pub struct Detector {
    path: PathBuf,
    context: Mutex<Option<WhisperVadContext>>,
}

impl Detector {
    pub fn new(models: &Path) -> Self {
        Self { path: models.join(FILE), context: Mutex::new(None) }
    }

    pub(super) fn needs_fetch(&self) -> bool {
        !std::fs::metadata(&self.path).is_ok_and(|m| m.len() == BYTES)
    }

    pub fn prepare(&self, fetch: bool) -> Result<bool, String> {
        if !self.needs_fetch() { return Ok(false); }
        if !fetch { return Err(format!("speech detector missing: {}", self.path.display())); }
        std::fs::create_dir_all(self.path.parent().unwrap()).map_err(|e| e.to_string())?;
        super::fetch_model(&self.path, URL, BYTES, SHA256, &mut |_| {})?;
        Ok(true)
    }

    /// Conservative speech spans, padded 300 ms to preserve word edges and the timing tolerance.
    pub fn spans(&self, pcm: &[f32]) -> Result<Vec<(i64, i64)>, String> {
        if pcm.is_empty() { return Ok(Vec::new()); }
        let mut context = self.context.lock().unwrap();
        if context.is_none() {
            let mut params = WhisperVadContextParams::default();
            params.set_n_threads(2);
            *context = Some(WhisperVadContext::new(self.path.to_str().ok_or("speech detector path is not UTF-8")?, params)
                .map_err(|e| format!("speech detector: {e}"))?);
        }
        // Quiet recordings need the same decision as their normally recorded counterparts.
        let peak = pcm.iter().fold(0.0f32, |p, x| p.max(x.abs()));
        let gain = if peak > 0.0 && peak < 0.1 { 0.1 / peak } else { 1.0 };
        let samples: Vec<_> = pcm.iter().map(|x| x * gain).collect();
        let mut params = WhisperVadParams::default();
        params.set_min_speech_duration(100);
        params.set_speech_pad(300);
        let spans = context.as_mut().unwrap().segments_from_samples(params, &samples)
            .map_err(|e| format!("speech detector: {e}"))?;
        #[cfg(test)]
        println!("voice: {} samples, peak={peak}, probabilities={:?}", pcm.len(), context.as_ref().unwrap().probabilities().iter().copied().fold(0.0f32, f32::max));
        Ok((0..spans.num_segments()).map(|i| {
            ((spans.get_segment_start_timestamp(i).unwrap() * 10.0).round() as i64,
             (spans.get_segment_end_timestamp(i).unwrap() * 10.0).round() as i64)
        }).collect())
    }
}
