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
    // The sentences translation asks are rebuilt from these cues, so they match too.
    assert_eq!(cues(&core), cues(&whole));
    // One sentence per cue: no sentence end before a cue's last word.
    for c in cues(&whole) {
        let words: Vec<&str> = c.2.split(' ').collect();
        assert!(words[..words.len() - 1].iter().all(|w| !w.ends_with('.')), "two sentences in {c:?}");
    }
}

/// The writing loop of a build from before sentence cues (0.1.9): it resumes
/// from `held` and knows no `tail`, and commits each piece as a cue of its own.
/// Its save is 0.1.9's own SQL, which leaves `tail` as it was.
/// It stops after `windows` windows, if given, as a process killed there would.
fn write_as_released(core: &Core, pcm: &[i16], script: &Script, windows: Option<usize>) {
    let job = core.store.job("j").unwrap().unwrap();
    let model = Heard { source: pcm.to_vec(), script: script.clone(), calls: Default::default(), fail_at: None, unloads: Default::default() };
    let regions = audio::plan(job.plan, pcm);
    let mut prev_end = core.store.last_cue_end("v").unwrap();
    let mut held: Vec<stitch::Piece> = job.held.as_deref().map(|h| serde_json::from_str(h).unwrap()).unwrap_or_default();
    let db = crate::db::Db::open(core.store.path()).unwrap();
    for (i, r) in regions.iter().enumerate().skip(job.written_regions as usize).take(windows.unwrap_or(usize::MAX)) {
        let samples: Vec<f32> = pcm[r.start..r.end].iter().map(|&s| s as f32 / 32768.0).collect();
        let segs = model.transcribe(&samples, &mut |_| {}).unwrap();
        let committed;
        (committed, held) = stitch::window_pieces(r, segs, std::mem::take(&mut held), i.checked_sub(1).map(|p| &regions[p]), regions.get(i + 1));
        let mut cues = Vec::new();
        for p in committed {
            let start = p.start_ms.max(prev_end);
            if p.text().is_empty() || p.end_ms <= start {
                continue;
            }
            prev_end = p.end_ms;
            cues.push((start, p.end_ms, p.text()));
        }
        let kept = (!held.is_empty()).then(|| serde_json::to_string(&held).unwrap());
        db.exec("BEGIN IMMEDIATE").unwrap();
        let next = db.query_one("SELECT COALESCE(MAX(seq),-1)+1 FROM cue WHERE video_key='v'", &[]).unwrap().unwrap()[0].int();
        for (k, (a, b, text)) in cues.iter().enumerate() {
            db.execute("INSERT INTO cue (video_key, seq, start_ms, end_ms, source_text) VALUES ('v',?,?,?,?)", crate::p![next + k as i64, *a, *b, text.as_str()]).unwrap();
        }
        db.execute("UPDATE job SET written_regions=?, held=? WHERE id='j'", crate::p![i as i64 + 1, kept.as_deref()]).unwrap();
        db.exec("COMMIT").unwrap();
    }
}

#[test]
fn a_write_killed_with_a_sentence_open_loses_no_word_when_the_released_build_resumes_it() {
    let (raw, pcm, script) = lecture(audio::PLAN);
    let script = sentences_of(&script);
    let windows = audio::plan(audio::PLAN, &pcm);
    let spoken: Vec<&str> = script.iter().map(|w| w.2.as_str()).collect();
    // Killed on each window from the second on, so it stops with a sentence open at every kind of cut.
    for k in 1..windows.len() {
        let mut core = job_at_writing(&format!("released-{k}"), audio::PLAN, &raw);
        assert!(write_with(&mut core, &pcm, &script, Some(k)).0.is_err());
        let open = core.store.job("j").unwrap().unwrap().tail.is_some();
        write_as_released(&core, &pcm, &script, None);
        let written: Vec<String> = cues(&core).iter().flat_map(|c| c.2.split(' ').map(String::from).collect::<Vec<_>>()).collect();
        assert_eq!(written, spoken, "killed at window {k} (a sentence open: {open})");
        let cs = cues(&core);
        assert!(cs.windows(2).all(|w| w[0].1 <= w[1].0), "killed at window {k}: cues overlap");
    }
}

#[test]
fn a_write_the_released_build_advanced_resumes_without_its_stale_open_sentence() {
    // Killed with a sentence open, advanced one or two windows by 0.1.9 (which
    // commits the open sentence from `held` and leaves `tail` alone), killed
    // again, and finished by this build: the words of an uninterrupted write,
    // each once, in order. Under the current plan and under plan 1, whose short
    // regions put a stale sentence within the pause that joins sentences.
    let words = |core: &Core| -> Vec<String> { cues(core).iter().flat_map(|c| c.2.split(' ').map(String::from).collect::<Vec<_>>()).collect() };
    // Three short plan-1 regions, the middle one without words, with the first
    // region's last word ending 2.6 s, or (plan 1 pads both regions) 1.43 s,
    // before the third region's first word: past and within the pause that
    // keeps two parts of one sentence in one cue.
    let short = |name: &'static str, spans: [(usize, usize); 3], words: [(i64, i64); 4]| {
        let raw = talk(7_000, &spans.map(|(a, b)| (a, b, LOUD)));
        let file = std::env::temp_dir().join(format!("vto-boundary-{name}-{}.wav", std::process::id()));
        std::fs::write(&file, wav(&raw)).unwrap();
        let pcm = audio::decode_file(&file).unwrap();
        let _ = std::fs::remove_file(&file);
        let script: Script = words.iter().enumerate().map(|(i, &(a, b))| (a, b, format!("word{i}"))).collect();
        assert_eq!(audio::plan(1, &pcm).len(), 3, "{name}: three regions");
        (name, 1, raw, pcm, script)
    };
    let shorts = [
        short("short", [(500, 1_700), (2_550, 2_950), (3_800, 5_000)], [(600, 900), (1_000, 1_300), (3_900, 4_200), (4_300, 4_600)]),
        short("short-gap", [(500, 1_700), (2_520, 2_680), (3_500, 5_000)], [(600, 900), (1_000, 1_890), (3_320, 3_600), (3_700, 4_000)]),
    ];
    let lectures = [("lecture", 1), ("lecture", audio::PLAN)].map(|(name, plan)| { let (raw, pcm, script) = lecture(plan); (name, plan, raw, pcm, sentences_of(&script)) });
    for (name, plan, raw, pcm, script) in shorts.into_iter().chain(lectures) {
        let windows = audio::plan(plan, &pcm);
        let mut whole = job_at_writing(&format!("advanced-{name}-{plan}-whole"), plan, &raw);
        write_with(&mut whole, &pcm, &script, None).0.unwrap();
        let spoken = words(&whole);
        let mut open = 0;
        for k in 1..windows.len() - 1 {
            for advanced in [1, 2].into_iter().filter(|n| k + n < windows.len()) {
                let mut core = job_at_writing(&format!("advanced-{name}-{plan}-{k}-{advanced}"), plan, &raw);
                assert!(write_with(&mut core, &pcm, &script, Some(k)).0.is_err());
                if core.store.job("j").unwrap().unwrap().tail.is_none() {
                    continue;
                }
                open += 1;
                write_as_released(&core, &pcm, &script, Some(advanced));
                let j = core.store.job("j").unwrap().unwrap();
                assert!(j.tail.is_some() && j.written_regions == (k + advanced) as i64, "{name}: 0.1.9 advanced the job and left its open sentence");
                write_with(&mut core, &pcm, &script, None).0.unwrap();
                assert_eq!(words(&core), spoken, "{name}, plan {plan}, killed at window {k}, advanced {advanced} by 0.1.9");
                let cs = cues(&core);
                assert!(cs.windows(2).all(|w| w[0].1 <= w[1].0), "plan {plan}, window {k}, advanced {advanced}: cues overlap");
            }
        }
        assert!(open > 0, "{name}, plan {plan}: no kill left a sentence open");
    }
}

#[test]
fn a_remake_that_failed_writing_cannot_continue_or_be_retranslated_and_playback_keeps_the_selected_version() {
    use crate::sync::{Change, Cue as Saved, Deletions, Translation, Video};
    let (raw, pcm, script) = lecture(audio::PLAN);
    let dir = std::env::temp_dir().join(format!("vto-boundary-remake-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut core = super::worker_tests::idle_core(&dir, "");
    // Signed in, so the API's re-translation works on the account library the re-make used.
    core.account = crate::account::tests::signed_in_manager(&dir, "remake-writing");
    let page = "https://lectures.example.org/unfinished";
    let key = crate::api::video_key(&url::Url::parse(page).unwrap());
    let video = Video { key: key.clone(), page_url: page.into(), title: "Talk".into() };
    let old = vec![Saved { start_ms: 500, end_ms: 1_400, source_text: "Said before.".into(), text_ko: "예전 번역".into() }];
    let scope = core.account.capture_library().unwrap();
    let store = core.account.with_library(&scope, |journal| {
        let journal = journal.expect("an account library");
        journal.write(Change::Translation { translation: Translation { video, duration_ms: 106_000, source_language: Some("en".into()), terms: vec![], model: "m".into(), cost_krw: 1.0, cues: old } }, Deletions::new())?;
        journal.library()
    }).unwrap();
    let audio = dir.join("audio.wav");
    std::fs::write(&audio, wav(&raw)).unwrap();
    let selected = store.begin_remake("remake-w", &key, &audio.to_string_lossy(), "m").unwrap();
    // The model stops on the second window, after the first one's cues were committed.
    core.svc.transcriber = Box::new(Heard { source: pcm.clone(), script: sentences_of(&script), calls: Default::default(), fail_at: Some(1), unloads: Default::default() });
    run_job(&core, &store, store.job("remake-w").unwrap().unwrap());
    let job = store.job("remake-w").unwrap().unwrap();
    assert_eq!((job.stage.as_str(), job.written_regions), ("failed", 1));
    assert!(!store.cues(&key).unwrap().is_empty(), "the first window's cues were committed");

    assert!(store.continue_remake(&key, "remake-w").is_err(), "a re-make with an unfinished transcript continued");
    // Neither an ordinary re-translation nor the store under it takes the draft as its source.
    let asked = crate::api::retranslate_job(&core, page).unwrap_err();
    assert_eq!((asked.status, asked.error.as_str()), (409, "not_ready"));
    let previous = store.latest_job(&key).unwrap().unwrap();
    assert_eq!(store.retranslate("again", &previous, "m").unwrap_err(), crate::store::REMAKE_UNFINISHED);
    assert!(store.job("again").unwrap().is_none());
    assert_eq!(store.job("remake-w").unwrap().unwrap().stage, "failed");
    let played = crate::api::subtitles(&store, &key).unwrap();
    assert_eq!(played, serde_json::json!({"video_key": key, "duration_ms": 106_000, "source_language": "en", "cues": [{"seq": 0, "start_ms": 500, "end_ms": 1_400, "text_ko": "예전 번역"}]}));
    // Once the selected version is restored, it can be re-translated again.
    store.restore_translation(&key, &selected).unwrap();
    let again = crate::api::retranslate_job(&core, page).unwrap();
    assert_eq!(again["stage"], "translating");
}
