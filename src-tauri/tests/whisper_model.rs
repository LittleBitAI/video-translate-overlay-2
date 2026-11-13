//! The real on-device model on real speech: Windows' speech synthesizer reads
//! a sentence, and the pipeline's own decode → speech regions → whisper.cpp
//! path has to write it back with times inside the audio.
//!
//! Needs the `app` feature and the 0.6 GB model file, so it is opt-in:
//! `VTO_WHISPER_MODEL=<ggml-large-v3-turbo-q5_0.bin> cargo test --features app --test whisper_model -- --ignored`
#![cfg(all(feature = "app", windows))]

use vto::whisper::Whisper;
use vto::Transcriber;

#[test]
#[ignore = "needs VTO_WHISPER_MODEL (the 0.6 GB model file) and a release-speed CPU run"]
fn the_model_writes_synthesized_speech() {
    let model = std::env::var_os("VTO_WHISPER_MODEL").expect("set VTO_WHISPER_MODEL to the model file");
    let dir = std::env::temp_dir().join(format!("vto-whisper-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let wav = dir.join("speech.wav");
    let say = "Deep learning models are trained with gradient descent. The transformer architecture relies on self attention.";
    let script = format!(
        // An en-US voice: a Korean Windows reads English with its Korean default voice otherwise.
        "Add-Type -AssemblyName System.Speech; $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; \
         $s.SelectVoiceByHints('NotSet', 'NotSet', 0, [Globalization.CultureInfo]'en-US'); \
         $s.SetOutputToWaveFile('{}'); $s.Speak('{say}'); $s.Dispose()",
        wav.display()
    );
    let ok = std::process::Command::new("pwsh").args(["-NoProfile", "-Command", &script]).status().expect("pwsh");
    assert!(ok.success() && wav.is_file(), "speech synthesis failed");

    let pcm = vto::audio::decode_file(&wav).unwrap();
    let regions = vto::audio::speech_regions(&pcm);
    assert!(!regions.is_empty(), "no speech found");
    // A fresh instance writes without `prepare`, as one restarted in the writing stage does.
    let w = Whisper::new(&dir, Some(model.into()));
    let mut text = String::new();
    let mut last_progress = 0.0f32;
    for r in &regions {
        let samples: Vec<f32> = pcm[r.start..r.end].iter().map(|&s| s as f32 / 32768.0).collect();
        let segs = w.transcribe(&samples, &mut |p| last_progress = p).unwrap();
        for s in &segs {
            assert!(0 <= s.start_ms && s.start_ms <= s.end_ms && s.end_ms <= r.end_ms() - r.start_ms() + 1000, "{s:?} outside its region");
            text.push_str(&s.text);
            text.push(' ');
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    let lower = text.to_lowercase();
    for word in ["deep learning", "gradient descent", "transformer", "attention"] {
        assert!(lower.contains(word), "{word:?} missing from {text:?}");
    }
    assert_eq!(last_progress, 1.0);
    assert_eq!(w.language().as_deref(), Some("en"));
    assert!(!w.prepare(&dir, &mut |_| {}).unwrap(), "an override model is never fetched");
    assert_eq!(w.language(), None, "prepare starts a job: the language is detected again");
}
