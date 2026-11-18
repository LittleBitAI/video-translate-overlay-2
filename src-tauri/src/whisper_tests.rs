//! Opt-in inference benchmark. Feed the same technical speech to all models;
//! these numbers describe the benchmark machine, never an unmeasured phone.

use super::*;
use serde_json::json;
use std::time::Instant;

#[test]
fn speech_model_override_provisions_only_the_detector_in_the_app_cache() {
    let dir = std::env::temp_dir().join(format!("vto-override-{}",std::process::id()));
    std::fs::create_dir_all(dir.join("existing")).unwrap();
    let model = dir.join("existing/speech.bin");
    std::fs::write(&model,b"existing speech model").unwrap();
    let app = dir.join("app");
    let w = Whisper::new(&app,Some(model.clone()));
    assert!(w.needs_fetch(),"the missing detector still needs provisioning");
    let mut downloads = 0;
    let mut provision = |path: &Path, url: &str, bytes: u64, sha: &str, _: &mut dyn FnMut(f32)| {
        downloads += 1;
        assert_eq!(path,app.join("models/ggml-silero-v6.2.0.bin"));
        assert!(url.ends_with("ggml-silero-v6.2.0.bin"));
        assert_eq!(bytes,885098);
        assert_eq!(sha,"2aa269b785eeb53a82983a20501ddf7c1d9c48e33ab63a41391ac6c9f7fb6987");
        // The downloader is substituted here; real pinned bytes are exercised by the audio test.
        std::fs::File::create(path).unwrap().set_len(885098).unwrap();
        Ok(())
    };
    assert!(w.prepare_models(&mut |_| {}, &mut provision).unwrap());
    assert!(!w.prepare_models(&mut |_| {}, &mut provision).unwrap());
    assert_eq!(downloads,1);
    assert!(!w.needs_fetch());
    assert_eq!(std::fs::read(&model).unwrap(),b"existing speech model");
    assert_eq!(std::fs::read_dir(model.parent().unwrap()).unwrap().count(),1);
    std::fs::remove_file(app.join("models/ggml-silero-v6.2.0.bin")).unwrap();
    assert_eq!(w.prepare_models(&mut |_| {}, &mut |_,_,_,_,_| Err("provision unavailable".into())).unwrap_err(),"provision unavailable");
    assert!(w.needs_fetch());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "needs VTO_MIT_OPENING_AUDIO, VTO_SPOKEN_REPETITION_AUDIO and VTO_MUSIC_MODELS: real audio and pinned local models"]
fn mit_music_and_contrasting_speech_keep_their_source_clock() {
    let models = PathBuf::from(std::env::var_os("VTO_MUSIC_MODELS").expect("models"));
    let path = PathBuf::from(std::env::var_os("VTO_MIT_OPENING_AUDIO").expect("MIT opening"));
    let pcm = crate::audio::decode_file(&path).unwrap();
    let samples: Vec<_> = pcm.iter().map(|&s| s as f32 / 32768.0).collect();
    let rate = crate::audio::RATE;
    let detector = voice::Detector::new(&models);
    detector.prepare(false).unwrap();
    assert!(detector.spans(&samples[..8 * rate]).unwrap().is_empty(), "music became speech");
    let cropped = &samples[1180 * rate / 1000..5800 * rate / 1000];
    assert!(detector.spans(cropped).unwrap().is_empty(), "the original cropped music window became speech");
    let spans = detector.spans(&samples[..20 * rate]).unwrap();
    println!("MIT opening speech spans: {spans:?}");
    assert!(spans.iter().all(|&(a, _)| a >= 8500), "music acquired a speech span: {spans:?}");
    let speech = &samples[10 * rate..17 * rate];
    let normal = detector.spans(speech).unwrap();
    let quiet: Vec<_> = speech.iter().map(|x| x * 0.005).collect();
    let quiet_spans = detector.spans(&quiet).unwrap();
    assert!(!normal.is_empty() && !quiet_spans.is_empty());
    assert!((normal[0].0 - quiet_spans[0].0).abs() <= 300);
    let music = &samples[rate..8 * rate];
    let mixed: Vec<_> = speech.iter().zip(music).map(|(s, m)| s + m * 0.3).collect();
    assert!(!detector.spans(&mixed).unwrap().is_empty(), "speech over music was lost");
    assert_eq!(detector.spans(&samples[..20 * rate]).unwrap(), spans, "reused state changed the speech clock");
    assert_eq!(voice::Detector::new(&models).spans(&samples[..20 * rate]).unwrap(), spans, "reopening changed the speech clock");
    let repetition_path = PathBuf::from(std::env::var_os("VTO_SPOKEN_REPETITION_AUDIO").expect("spoken repetition"));
    let repetition: Vec<_> = crate::audio::decode_file(&repetition_path).unwrap().into_iter().map(|x| x as f32 / 32768.0).collect();
    assert!(detector.spans(&repetition).unwrap().iter().any(|&(a,b)| a < 3500 && b > 9500), "genuine rhythmic speech was lost");
    for (name, mobile) in [("ggml-base-q5_1.bin", true), ("ggml-large-v3-turbo-q5_0.bin", false)] {
        let mut w = Whisper::new(models.parent().unwrap(), Some(models.join(name)));
        w.mobile = mobile;
        assert!(w.transcribe(&samples[..8 * rate], &mut |_| {}).unwrap().is_empty());
        assert!(w.transcribe(cropped, &mut |_| {}).unwrap().is_empty());
        let segs = w.transcribe(&samples[..20 * rate], &mut |_| {}).unwrap();
        println!("{name}: {segs:?}");
        assert!(!segs.is_empty() && segs.iter().all(|s| s.start_ms >= 8500));
        assert!(segs.iter().map(|s| s.text.to_lowercase()).collect::<String>().contains("afternoon"));
        let mut end = 0;
        let (cues, _) = crate::stitch::window_cues(&crate::audio::Region { start: 0, end: 20 * rate }, segs, Vec::new(), None, None, &mut end);
        assert!(cues.windows(2).all(|w| w[0].1 <= w[1].0));
        assert!((cues[0].0 - 9100).abs() <= 300, "speech timing moved: {cues:?}");
    }
}

#[test]
#[ignore = "needs the original audio of vSCYL8ySh9g in VTO_WHISPER_BENCH_AUDIO, models and VTO_SYNC_RESULT"]
fn problem_video_keeps_its_late_speech_on_the_source_clock() {
    let models = PathBuf::from(std::env::var_os("VTO_WHISPER_BENCH_MODELS").expect("models"));
    let path = PathBuf::from(std::env::var_os("VTO_WHISPER_BENCH_AUDIO").expect("audio"));
    let dest = PathBuf::from(std::env::var_os("VTO_SYNC_RESULT").expect("result"));
    let pcm = crate::audio::decode_file(&path).unwrap();
    assert!((pcm.len() as f64 / crate::audio::RATE as f64 - 904.324).abs() < 0.1);
    std::fs::write(dest.with_extension("pcm"), pcm.iter().flat_map(|s| s.to_le_bytes()).collect::<Vec<_>>()).unwrap();
    let regions = crate::audio::speech_regions(&pcm);
    let mut w = Whisper::new(models.parent().unwrap(), Some(models.join("ggml-base-q5_1.bin")));
    w.mobile = true;
    let mut cues = Vec::new();
    let mut prev_end = 0;
    let begin = Instant::now();
    for (i, r) in regions.iter().enumerate() {
        let samples: Vec<_> = pcm[r.start..r.end].iter().map(|&s| s as f32 / 32768.0).collect();
        for s in w.transcribe(&samples, &mut |_| {}).unwrap() {
            let start = (r.start_ms() + s.start_ms).max(prev_end);
            let end = (r.start_ms() + s.end_ms).min(r.end_ms());
            if end > start && !s.text.trim().is_empty() {
                cues.push(json!({"start_ms":start,"end_ms":end,"text":s.text}));
                prev_end = end;
            }
        }
        println!("region {i}/{} at {} ms ({:.1} s elapsed)", regions.len(), r.start_ms(), begin.elapsed().as_secs_f64());
        std::fs::write(&dest, serde_json::to_vec_pretty(&json!({"duration_ms":pcm.len()*1000/crate::audio::RATE,"cues":cues})).unwrap()).unwrap();
    }
    // Phrases visible in the original English captions at these source positions.
    for (at, phrase) in [(30_000, "one loop"), (420_000, "try out"), (600_000, "weights"), (800_000, "rumors"), (890_000, "gpt")] {
        let cue = cues.iter().find(|c| c["start_ms"].as_i64().unwrap() <= at && c["end_ms"].as_i64().unwrap() > at).expect("cue at source position");
        assert!(cue["text"].as_str().unwrap().to_lowercase().contains(phrase), "{at}: {cue}");
    }
}

#[test]
#[ignore = "needs VTO_WHISPER_BENCH_MODELS and VTO_WHISPER_BENCH_AUDIO (technical speech)"]
fn reused_mobile_state_keeps_fifteen_minutes_of_source_offsets() {
    let models = PathBuf::from(std::env::var_os("VTO_WHISPER_BENCH_MODELS").expect("model directory"));
    let path = PathBuf::from(std::env::var_os("VTO_WHISPER_BENCH_AUDIO").expect("technical speech WAV"));
    let speech = crate::audio::decode_file(&path).unwrap();
    let first = crate::audio::speech_regions(&speech)[0];
    let piece = &speech[first.start..first.end];
    let mut pcm = vec![0; 15 * 60 * crate::audio::RATE];
    for minute in 0..15 {
        let at = (minute * 60 + 1) * crate::audio::RATE;
        pcm[at..at + piece.len()].copy_from_slice(piece);
    }
    let regions = crate::audio::speech_regions(&pcm);
    assert_eq!(regions.len(), 15);
    let mut w = Whisper::new(models.parent().unwrap(), Some(models.join("ggml-base-q5_1.bin")));
    w.mobile = true;
    for (minute, r) in regions.iter().enumerate() {
        let samples: Vec<_> = pcm[r.start..r.end].iter().map(|&s| s as f32 / 32768.0).collect();
        let segments = w.transcribe(&samples, &mut |_| {}).unwrap();
        let first = segments.iter().find(|s| s.text.to_lowercase().contains("deep learning")).expect("first sentence");
        let at = r.start_ms() + first.start_ms;
        let expected = (minute as i64 * 60 + 1) * 1000;
        println!("minute {minute}: expected {expected}, actual {at}, error {} ms", at - expected);
        assert!((at - expected).abs() <= 300, "late cue: {first:?}");
    }
}

#[test]
#[ignore = "needs VTO_WHISPER_BENCH_MODELS and VTO_WHISPER_BENCH_AUDIO, real model files and a release build"]
fn compare_mobile_models_on_technical_speech() {
    let models = PathBuf::from(std::env::var_os("VTO_WHISPER_BENCH_MODELS").expect("model directory"));
    let audio = PathBuf::from(std::env::var_os("VTO_WHISPER_BENCH_AUDIO").expect("technical speech WAV"));
    let pcm = crate::audio::decode_file(&audio).unwrap();
    let regions = crate::audio::speech_regions(&pcm);
    assert!(!regions.is_empty());
    let mut results = Vec::new();
    for (name, mobile, flash) in [
        ("base-q5_1", true, true), ("base-q5_1", true, false),
        ("small-q5_1", true, true), ("large-v3-turbo-q5_0", false, false),
    ] {
        let mut w = Whisper::new(models.parent().unwrap(), Some(models.join(format!("ggml-{name}.bin"))));
        w.mobile = mobile;
        let load = Instant::now();
        let mut params = WhisperContextParameters::default();
        params.flash_attn(flash);
        let ctx = WhisperContext::new_with_params(&w.path, params).unwrap();
        *w.state.lock().unwrap() = Some((ctx.create_state().unwrap(), ctx.token_eot()));
        drop(ctx);
        let load_ms = load.elapsed().as_millis();
        let mut runs = Vec::new();
        // The second job must reuse buffers without inheriting the first job's text/language.
        for _ in 0..2 {
            assert!(!w.prepare(&models, &mut |_| {}).unwrap());
            assert_eq!(w.language(), None);
            let begin = Instant::now();
            let mut text = String::new();
            let mut last_progress = 0.0;
            for r in &regions {
                let samples: Vec<f32> = pcm[r.start..r.end].iter().map(|&s| s as f32 / 32768.0).collect();
                let segments = w.transcribe(&samples, &mut |p| last_progress = p).unwrap();
                for s in segments {
                    assert!(s.start_ms >= 0 && s.end_ms >= s.start_ms && s.end_ms <= r.end_ms() - r.start_ms() + 1000, "{s:?}");
                    text.push_str(&s.text);
                }
            }
            let normalized = text.to_lowercase().replace('-', " ");
            for word in ["deep learning", "gradient descent", "transformer", "attention"] {
                assert!(normalized.contains(word), "{name}: {word:?} missing from {text:?}");
            }
            assert_eq!(last_progress, 1.0);
            assert_eq!(w.language().as_deref(), Some("en"));
            runs.push(json!({"inference_ms": begin.elapsed().as_millis(), "text": text}));
        }
        let result = json!({"model": name, "mobile": mobile, "flash": flash, "load_ms": load_ms, "audio_ms": pcm.len() * 1000 / crate::audio::RATE, "regions": regions.len(), "runs": runs});
        println!("{result}");
        results.push(result);
    }
    if let Some(dest) = std::env::var_os("VTO_WHISPER_BENCH_RESULT") {
        std::fs::write(dest, serde_json::to_vec_pretty(&results).unwrap()).unwrap();
    }
}
