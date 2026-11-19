//! Writing against reference speech: continuous speech the windows must cut,
//! words crossing those cuts, a word said again and again, technical terms,
//! quiet speech, long silence, and a write killed and resumed. Every spoken
//! word ends up in the cues once, in order, inside its cue's time, and a
//! resumed write commits exactly what an uninterrupted one does, under the
//! plan its job was made with.

use super::*;
use crate::audio::{Region, RATE};
use crate::{Segment, Transcriber, Word};
use std::sync::atomic::{AtomicUsize, Ordering};

/// The reference: each word's source time and text.
type Script = Vec<(i64, i64, String)>;

/// A model that hears the script: the words inside the window it is given,
/// and a fragment of a word the window's edge cuts (its first or last half).
/// Call `fail_at` fails, as a process killed there would stop.
struct Heard {
    source: Vec<i16>,
    script: Script,
    calls: Arc<AtomicUsize>,
    fail_at: Option<usize>,
    unloads: Arc<AtomicUsize>,
}

impl Transcriber for Heard {
    fn unload(&self) {
        self.unloads.fetch_add(1, Ordering::SeqCst);
    }
    fn transcribe(&self, pcm: &[f32], _: &mut dyn FnMut(f32)) -> Result<Vec<Segment>, String> {
        if Some(self.calls.fetch_add(1, Ordering::SeqCst)) == self.fail_at {
            return Err("killed".into());
        }
        let probe: Vec<i16> = pcm[..64].iter().map(|x| (x * 32768.0).round() as i16).collect();
        let at = self.source.windows(64).position(|w| w == probe).expect("a window of the source");
        let (w0, w1) = ((at * 1000 / RATE) as i64, ((at + pcm.len()) * 1000 / RATE) as i64);
        let mut words = Vec::new();
        for (a, b, text) in &self.script {
            let inside = (b.min(&w1) - a.max(&w0)) as f64 / (b - a) as f64;
            let text = if inside >= 1.0 {
                text.clone()
            } else if inside >= 0.4 {
                let half = text.chars().count() / 2;
                if *a < w0 { text.chars().skip(half).collect() } else { text.chars().take(half).collect() }
            } else {
                continue;
            };
            words.push(Word { start_ms: (*a).max(w0) - w0, end_ms: (*b).min(w1) - w0, text: format!(" {text}") });
        }
        // A segment for every 8 words heard, as the model ends sentences.
        Ok(words
            .chunks(8)
            .map(|ws| Segment { start_ms: ws[0].start_ms, end_ms: ws.last().unwrap().end_ms, text: ws.iter().map(|w| w.text.as_str()).collect(), words: ws.to_vec() })
            .collect())
    }
}

/// Speech-like noise over `spans` (ms), at `peak`; faint noise elsewhere.
fn talk(total_ms: usize, spans: &[(usize, usize, i32)]) -> Vec<i16> {
    let mut x = 99u32;
    (0..total_ms * RATE / 1000)
        .map(|i| {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let r = (x >> 16) as i32 % 2001 - 1000;
            let ms = i * 1000 / RATE;
            let peak = spans.iter().find(|&&(a, b, _)| (a..b).contains(&ms)).map_or(5, |s| s.2);
            (r * peak / 1000) as i16
        })
        .collect()
}

const LOUD: i32 = 8_000;
const QUIET: i32 = 700;
/// 75 s of talk with no pause, then a 1 s pause and 6 s more; 12 s of silence;
/// then 10 s of quiet speech.
const SPANS: [(usize, usize, i32); 3] = [(500, 75_500, LOUD), (76_500, 82_500, LOUD), (94_500, 104_500, QUIET)];

/// The lecture's audio as stored and as the writing stage decodes it, and its
/// words: one every 400 ms through every span; around every place two windows
/// of `plan` overlap, a word said three times and two technical terms.
fn lecture(plan: i64) -> (Vec<i16>, Vec<i16>, Script) {
    let raw = talk(106_000, &SPANS);
    // One file per call: tests run side by side, and one must not read a file another is rewriting.
    static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let file = std::env::temp_dir().join(format!("vto-boundary-lecture-{plan}-{}-{call}.wav", std::process::id()));
    std::fs::write(&file, wav(&raw)).unwrap();
    let pcm = audio::decode_file(&file).unwrap();
    let _ = std::fs::remove_file(&file);
    let shared: Vec<(i64, i64)> = audio::plan(plan, &pcm).windows(2).filter(|w| w[1].start < w[0].end).map(|w| (w[1].start_ms(), w[0].end_ms())).collect();
    let mut script = Script::new();
    for &(a, b, _) in &SPANS {
        let mut t = a as i64 + 100;
        while t + 300 <= b as i64 - 100 {
            script.push((t, t + 300, format!("word{}", script.len())));
            t += 400;
        }
    }
    for &(z0, z1) in &shared {
        let first = script.iter().position(|w| w.0 >= z0 - 400).unwrap();
        for (k, text) in ["Transformer", "very", "very", "very", "GPT-4o"].iter().enumerate() {
            script[first + k].2 = text.to_string();
        }
        assert!(script[first + 4].1 <= z1 + 1_200, "the terms fit around the overlap");
    }
    (raw, pcm, script)
}

fn wav(pcm: &[i16]) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend(b"RIFF");
    b.extend((36 + pcm.len() as u32 * 2).to_le_bytes());
    b.extend(b"WAVEfmt ");
    b.extend(16u32.to_le_bytes());
    b.extend(1u16.to_le_bytes());
    b.extend(1u16.to_le_bytes());
    b.extend((RATE as u32).to_le_bytes());
    b.extend((RATE as u32 * 2).to_le_bytes());
    b.extend(2u16.to_le_bytes());
    b.extend(16u16.to_le_bytes());
    b.extend(b"data");
    b.extend((pcm.len() as u32 * 2).to_le_bytes());
    for s in pcm {
        b.extend(s.to_le_bytes());
    }
    b
}

/// A job at writing for the lecture under `plan`, its audio in `name`'s directory.
fn job_at_writing(name: &str, plan: i64, raw: &[i16]) -> Core {
    let dir = std::env::temp_dir().join(format!("vto-boundary-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let core = super::worker_tests::idle_core(&dir, "");
    core.store.create_job("j", "v", "https://a.example/talk", "other", "Talk", 106_000, None, "m").unwrap();
    let audio = dir.join("audio.wav");
    std::fs::write(&audio, wav(raw)).unwrap();
    core.store.set_audio("j", Some(&audio.to_string_lossy())).unwrap();
    core.store.set_stage("j", "writing").unwrap();
    if plan != audio::PLAN {
        crate::db::Db::open(&dir.join("app.sqlite3")).unwrap().execute("UPDATE job SET plan=? WHERE id='j'", crate::p![plan]).unwrap();
    }
    core
}

/// Writes the job with a model that fails at call `fail_at`, if any; how many calls it got.
fn write_with(core: &mut Core, pcm: &[i16], script: &Script, fail_at: Option<usize>) -> (Result<(), Stop>, usize) {
    let calls = Arc::new(AtomicUsize::new(0));
    core.svc.transcriber = Box::new(Heard { source: pcm.to_vec(), script: script.clone(), calls: calls.clone(), fail_at, unloads: Default::default() });
    let cx = Ctx { core, store: &core.store, title: "Talk".into(), page_url: String::new() };
    let job = core.store.job("j").unwrap().unwrap();
    let got = write(&cx, &job);
    (got, calls.load(Ordering::SeqCst))
}

fn cues(core: &Core) -> Vec<(i64, i64, String)> {
    core.store.cues("v").unwrap().into_iter().map(|c| (c.start_ms, c.end_ms, c.source_text)).collect()
}

#[test]
fn every_spoken_word_is_written_once_in_order_and_in_time() {
    let (raw, pcm, script) = lecture(audio::PLAN);
    let windows: Vec<Region> = audio::plan(audio::PLAN, &pcm);
    assert!(windows.windows(2).filter(|w| w[1].start < w[0].end).count() >= 2, "the lecture has cuts through speech");
    let mut core = job_at_writing("whole", audio::PLAN, &raw);
    write_with(&mut core, &pcm, &script, None).0.unwrap();
    let cues = cues(&core);

    let spoken: Vec<&str> = script.iter().map(|w| w.2.as_str()).collect();
    let written: Vec<&str> = cues.iter().flat_map(|c| c.2.split(' ')).collect();
    assert_eq!(written, spoken);
    for c in &cues {
        assert!(c.0 < c.1, "empty or reversed cue {c:?}");
    }
    for w in cues.windows(2) {
        assert!(w[0].1 <= w[1].0, "cues overlap: {w:?}");
    }
    // Each word inside its cue's time, within 300 ms.
    let mut words = script.iter();
    for c in &cues {
        for _ in c.2.split(' ') {
            let (a, b, text) = words.next().unwrap();
            assert!(c.0 - 300 <= (a + b) / 2 && (a + b) / 2 <= c.1 + 300, "{text} at {a}-{b} ms in cue {c:?}");
        }
    }
    // Nothing in the long silence; the quiet speech is there.
    assert!(cues.iter().all(|c| !(83_000..94_000).contains(&c.0)), "a cue in the silence: {cues:?}");
    assert!(cues.iter().any(|c| c.0 >= 94_000));
    assert!(core.store.job("j").unwrap().unwrap().held.is_none());
}

#[test]
fn a_write_killed_after_a_held_boundary_resumes_to_the_same_cues() {
    let (raw, pcm, script) = lecture(audio::PLAN);
    let mut whole = job_at_writing("uncut", audio::PLAN, &raw);
    write_with(&mut whole, &pcm, &script, None).0.unwrap();

    // Killed on the window after the first cut through speech: its predecessor's boundary words are held.
    let windows = audio::plan(audio::PLAN, &pcm);
    let k = windows.windows(2).position(|w| w[1].start < w[0].end).unwrap() + 1;
    let mut core = job_at_writing("killed", audio::PLAN, &raw);
    let (got, _) = write_with(&mut core, &pcm, &script, Some(k));
    assert!(got.is_err());
    let job = core.store.job("j").unwrap().unwrap();
    assert_eq!((job.written_regions, job.held.is_some()), (k as i64, true));
    write_with(&mut core, &pcm, &script, None).0.unwrap();
    assert_eq!(cues(&core), cues(&whole));
}

#[test]
fn a_job_made_before_plans_resumes_on_its_own_windows() {
    let (raw, pcm, script) = lecture(1);
    let windows = audio::speech_regions(&pcm).len();
    let mut whole = job_at_writing("legacy-uncut", 1, &raw);
    // Its windows are plan 1's, one model call each.
    assert_eq!(write_with(&mut whole, &pcm, &script, None), (Ok(()), windows));
    let mut core = job_at_writing("legacy", 1, &raw);
    assert!(write_with(&mut core, &pcm, &script, Some(2)).0.is_err());
    assert_eq!(core.store.job("j").unwrap().unwrap().written_regions, 2);
    assert_eq!(write_with(&mut core, &pcm, &script, None), (Ok(()), windows - 2));
    assert_eq!(cues(&core), cues(&whole));
}

#[test]
fn the_model_is_released_when_writing_ends_either_way() {
    let (raw, pcm, script) = lecture(audio::PLAN);
    for (name, fail_at) in [("release-done", None), ("release-failed", Some(1))] {
        let mut core = job_at_writing(name, audio::PLAN, &raw);
        let unloads = Arc::new(AtomicUsize::new(0));
        core.svc.transcriber = Box::new(Heard { source: pcm.clone(), script: script.clone(), calls: Default::default(), fail_at, unloads: unloads.clone() });
        // Marked notified, the job skips translation and ends right after writing.
        core.store.mark_notified("j").unwrap();
        run_job(&core, &core.store, core.store.job("j").unwrap().unwrap());
        assert_eq!(unloads.load(Ordering::SeqCst), 1, "{name}");
        let stage = core.store.job("j").unwrap().unwrap().stage;
        assert_eq!(stage, if fail_at.is_none() { "ready" } else { "failed" }, "{name}");
        assert!(fail_at.is_some() || cues(&core).len() > 1, "{name}: the lecture was written");
    }
}

/// The script as sentences of nine words: a period on every ninth word and
/// a capital on the word after it, so sentences run across the windows' cuts.
fn sentences_of(script: &Script) -> Script {
    let mut out = script.clone();
    for k in 0..out.len() {
        if k % 9 == 8 {
            out[k].2.push('.');
        } else if k > 0 && k % 9 == 0 {
            let w = &mut out[k].2;
            *w = w[..1].to_uppercase() + &w[1..];
        }
    }
    out
}

#[test]
fn a_write_killed_with_a_sentence_open_across_a_cut_resumes_to_the_same_cues() {
    let (raw, pcm, script) = lecture(audio::PLAN);
    let script = sentences_of(&script);
    let mut whole = job_at_writing("sentences-uncut", audio::PLAN, &raw);
    write_with(&mut whole, &pcm, &script, None).0.unwrap();
    // Killed on the window after the first cut through speech, with a sentence running across that cut.
    let windows = audio::plan(audio::PLAN, &pcm);
    let k = windows.windows(2).position(|w| w[1].start < w[0].end).unwrap() + 1;
    let cut = windows[k].start_ms();
    assert!(cues(&whole).iter().any(|c| c.0 < cut && cut < c.1), "no cue runs across the cut at {cut} ms");

    let mut core = job_at_writing("sentences-killed", audio::PLAN, &raw);
    assert!(write_with(&mut core, &pcm, &script, Some(k)).0.is_err());
    let job = core.store.job("j").unwrap().unwrap();
    assert_eq!(job.written_regions, k as i64);
    assert!(job.tail.is_some(), "the sentence open at the kill was not kept");
    write_with(&mut core, &pcm, &script, None).0.unwrap();
    assert_eq!(cues(&core), cues(&whole));
    let parts = |c: &Core| c.store.job("j").unwrap().unwrap().parts;
    assert_eq!(parts(&core), parts(&whole));
    // One sentence per cue: no sentence end before a cue's last word.
    for c in cues(&whole) {
        let words: Vec<&str> = c.2.split(' ').collect();
        assert!(words[..words.len() - 1].iter().all(|w| !w.ends_with('.')), "two sentences in {c:?}");
    }
}
