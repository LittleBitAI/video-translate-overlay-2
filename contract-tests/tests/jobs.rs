//! Jobs: P6 POST /jobs, P7 GET /jobs, and the background pipeline behind them.

use vto_contract_tests::serde_json::json;
use vto_contract_tests::*;

const YT: &str = "https://www.youtube.com/watch?v=contract0001";
const TALK: &str = "https://lectures.example.org/talks/42";

#[test]
fn test_detect_button_starts_downloading_job() {
    let mut app = App::new(Options::default());
    app.media.serve("/yt/0001.wav", "audio/wav", speech_wav(20_000, &[(1_000, 5_000), (6_000, 12_000)]));
    app.media.hold("/yt/0001.wav");
    app.youtube(YT, "/yt/0001.wav");
    app.launch();
    app.set_key(KEY);

    let r = app.create_job(YT, None, "Attention Is All You Need, explained", 20_000);
    assert_eq!(r.status, 200, "POST /jobs: {r:?}");
    let job_id = r.str("job_id");
    assert!(!job_id.is_empty());
    assert_eq!(r.body["reused"], false, "{r:?}");
    assert_eq!(r.body["stage"], "downloading", "{r:?}");

    let j = app.job(YT);
    assert_eq!(j.status, 200, "GET /jobs: {j:?}");
    assert_eq!(j.body["job_id"], job_id.as_str());
    assert_eq!(j.body["stage"], "downloading", "{j:?}");

    // Nobody picked a file: the app fetched the audio by itself.
    let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !app.media.hits().iter().any(|h| h == "GET /yt/0001.wav") {
        assert!(std::time::Instant::now() < end, "the audio was never requested: {:?}", app.media.hits());
        sleep_ms(50);
    }
    app.media.release("/yt/0001.wav");
}

#[test]
fn test_job_without_key_asks_for_key() {
    let mut app = App::new(Options::default());
    app.media.serve("/yt/0001.wav", "audio/wav", speech_wav(10_000, &[(1_000, 5_000)]));
    app.youtube(YT, "/yt/0001.wav");
    app.launch();

    let r = app.create_job(YT, None, "A lecture", 10_000);
    assert_eq!(r.status, 428, "POST /jobs without a key: {r:?}");
    let j = app.job(YT);
    assert_eq!(j.status, 404, "a job was made anyway: {j:?}");
}

#[test]
fn test_job_advances_in_background_and_resumes() {
    let mut app = App::new(Options { ms_per_audio_second: 200, ..Options::default() });
    let url = app.media.serve("/talks/42.wav", "audio/wav", speech_wav(60_000, &regular_spans(30, 200, 2_000, 1_500)));
    app.launch();
    app.set_key(KEY);
    let r = app.create_job(TALK, Some(&url), "Backpropagation from scratch", 60_000);
    assert_eq!(r.status, 200, "POST /jobs: {r:?}");
    let job_id = r.str("job_id");

    let seen = app.poll_job(TALK, 20, 60, |b| b["stage"] == "writing");
    let before = progress(seen.last().unwrap());
    sleep_ms(1_500); // no command is called meanwhile
    let mid = app.job(TALK);
    let after = progress(&mid.body);
    assert!(after > before, "no progress without the screen: {before:?} then {after:?}");
    assert_eq!(mid.body["stage"], "writing", "write ended too soon to test a restart: {mid:?}");

    app.restart();
    let resumed = app.wait_ready(TALK);
    assert_eq!(resumed[0]["job_id"], job_id.as_str(), "a different job after restart: {}", resumed[0]);
    let mut last = after;
    for b in &resumed {
        let p = progress(b);
        assert!(p >= last, "went back after restart: {last:?} then {p:?} ({:?})", stages(&resumed));
        last = p;
    }
    app.wait_notification("ready", &job_id);
}

#[test]
fn test_subtitles_refused_until_ready() {
    let mut app = App::new(Options { ms_per_audio_second: 1_000, ..Options::default() });
    let url = app.media.serve("/talks/42.wav", "audio/wav", speech_wav(60_000, &regular_spans(30, 200, 2_000, 1_500)));
    app.launch();
    app.set_key(KEY);
    let r = app.create_job(TALK, Some(&url), "Long lecture", 60_000);
    assert_eq!(r.status, 200, "POST /jobs: {r:?}");
    let video_key = r.str("video_key");
    app.poll_job(TALK, 20, 60, |b| b["stage"] == "writing");

    let s = app.subtitles(TALK, &video_key);
    assert_eq!(s.status, 409, "subtitles before ready: {s:?}");

    let j = app.job(TALK);
    assert_eq!(j.body["stage"], "writing");
    assert!(j.body["percent"].is_i64(), "{j:?}");
    let allowed = [
        "job_id", "video_key", "title", "stage", "percent", "failure_reason",
        "input_amount", "output_amount", "cost_krw", "notified",
    ];
    for k in j.body.as_object().expect("GET /jobs body is an object").keys() {
        assert!(allowed.contains(&k.as_str()), "GET /jobs leaks `{k}` while writing: {j:?}");
    }
}

#[test]
fn test_write_finishes_offline_on_device() {
    let lines: Vec<String> = ["offline line alpha about attention", "offline line beta about tokens", "offline line gamma about loss"]
        .map(String::from)
        .to_vec();
    let mut app = App::new(Options { lines: lines.clone(), ms_per_audio_second: 300, ..Options::default() });
    let url = app.media.serve("/talks/42.wav", "audio/wav", speech_wav(30_000, &regular_spans(9, 500, 3_000, 2_000)));
    app.launch();
    app.set_key(KEY);
    let r = app.create_job(TALK, Some(&url), "Offline lecture", 30_000);
    assert_eq!(r.status, 200, "POST /jobs: {r:?}");
    let video_key = r.str("video_key");
    app.poll_job(TALK, 20, 60, |b| b["stage"] == "writing");

    app.block_network();
    app.media.stop();
    app.gemini.stop();

    app.poll_job(TALK, 20, 120, |b| {
        assert_ne!(b["stage"], "failed", "write needed the network: {b}");
        b["stage"] == "translating"
    });
    assert!(!app.sqlite_files().is_empty(), "no SQLite file under the data folder");
    assert!(
        lines.iter().any(|line| app.sqlite_contains(line.as_bytes())),
        "no written cue of {video_key} is in SQLite"
    );
}

#[test]
fn test_other_site_media_follows_same_stages() {
    let mut app = App::new(Options {
        ms_per_audio_second: 300,
        gemini: GeminiOptions { delay_ms: 400, ..Default::default() },
        ..Options::default()
    });
    app.media.serve("/hls/index.m3u8", "application/vnd.apple.mpegurl", include_bytes!("../fixtures/hls/index.m3u8").to_vec());
    for (i, seg) in [
        &include_bytes!("../fixtures/hls/seg0.aac")[..],
        &include_bytes!("../fixtures/hls/seg1.aac")[..],
        &include_bytes!("../fixtures/hls/seg2.aac")[..],
    ]
    .iter()
    .enumerate()
    {
        app.media.serve(&format!("/hls/seg{i}.aac"), "audio/aac", seg.to_vec());
    }
    app.launch();
    app.set_key(KEY);

    let page = "https://lectures.example.org/hls-talk";
    let hls = app.media.url("/hls/index.m3u8");
    let seen = app.run_job(page, Some(&hls), "Diffusion models, a lecture", 12_000);
    assert_eq!(stages(&seen), STAGES.to_vec(), "stages went out of order");
    let job_id = seen[0]["job_id"].as_str().unwrap().to_string();
    app.wait_notification("ready", &job_id);
}

#[test]
fn test_undownloadable_video_shows_reason() {
    let mut app = App::new(Options::default());
    let drm = "#EXTM3U\n#EXT-X-VERSION:5\n#EXT-X-TARGETDURATION:4\n\
        #EXT-X-KEY:METHOD=SAMPLE-AES,URI=\"skd://drm.example.org/key/1\",KEYFORMAT=\"com.apple.streamingkeydelivery\",KEYFORMATVERSIONS=\"1\"\n\
        #EXTINF:4.0,\nseg0.aac\n#EXT-X-ENDLIST\n";
    let url = app.media.serve("/drm/index.m3u8", "application/vnd.apple.mpegurl", drm.as_bytes().to_vec());
    app.media.serve("/drm/seg0.aac", "audio/aac", include_bytes!("../fixtures/hls/seg0.aac").to_vec());
    app.launch();
    app.set_key(KEY);

    let page = "https://lectures.example.org/protected-talk";
    let r = app.create_job(page, Some(&url), "Protected lecture", 4_000);
    assert_eq!(r.status, 422, "POST /jobs on DRM media: {r:?}");
    assert!(contains_ci(r.body.to_string().as_bytes(), "drm"), "the reason does not name DRM: {r:?}");
    let j = app.job(page);
    assert_eq!(j.status, 404, "a job was left behind: {j:?}");
}

#[test]
fn test_write_percent_and_ongoing_notice() {
    let mut app = App::new(Options { ms_per_audio_second: 80, ..Options::default() });
    let url = app.media.serve("/talks/42.wav", "audio/wav", speech_wav(120_000, &regular_spans(60, 200, 2_000, 1_500)));
    app.launch();
    app.set_key(KEY);
    let r = app.create_job(TALK, Some(&url), "Two-minute lecture", 120_000);
    assert_eq!(r.status, 200, "POST /jobs: {r:?}");
    let job_id = r.str("job_id");

    let first = app.poll_job(TALK, 20, 60, |b| b["stage"] == "writing");
    let shade = app.notifications();
    let ongoing = |n: &vto_contract_tests::serde_json::Value, state: &str| {
        n["kind"] == "ongoing" && n["job_id"] == job_id.as_str() && n["state"] == state
    };
    assert!(shade.iter().any(|n| ongoing(n, "shown")), "no ongoing notice once the job started: {shade:?}");
    assert!(!shade.iter().any(|n| ongoing(n, "hidden")), "ongoing notice gone while writing: {shade:?}");

    let mut percents = vec![first.last().unwrap()["percent"].as_i64().unwrap()];
    let rest = app.poll_job(TALK, 100, 120, |b| b["stage"] != "writing");
    percents.extend(rest.iter().filter(|b| b["stage"] == "writing").map(|b| b["percent"].as_i64().unwrap()));
    assert!(percents.iter().all(|p| (0..=100).contains(p)), "percent out of range: {percents:?}");
    assert!(percents.windows(2).all(|w| w[0] <= w[1]), "percent went down: {percents:?}");
    let mut distinct = percents.clone();
    distinct.dedup();
    assert!(distinct.len() >= 3, "percent barely moved during a long write: {percents:?}");

    app.wait_ready(TALK);
    let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let shade = app.notifications();
        let shown = shade.iter().position(|n| ongoing(n, "shown"));
        let hidden = shade.iter().rposition(|n| ongoing(n, "hidden"));
        if matches!((shown, hidden), (Some(s), Some(h)) if h > s) {
            break;
        }
        assert!(std::time::Instant::now() < end, "ongoing notice not taken down at the end: {shade:?}");
        sleep_ms(50);
    }
}

#[test]
fn test_repeat_detect_reuses_saved_subtitles() {
    let mut app = App::new(Options::default());
    app.media.serve("/yt/0001.wav", "audio/wav", speech_wav(12_000, &regular_spans(4, 500, 3_000, 2_000)));
    app.youtube(YT, "/yt/0001.wav");
    app.launch();
    app.set_key(KEY);
    let seen = app.run_job(YT, None, "Attention Is All You Need, explained", 12_000);
    let video_key = seen[0]["video_key"].as_str().unwrap().to_string();
    let first = cues(&app.subtitles(YT, &video_key));
    assert!(!first.is_empty(), "the first run made no cues");
    let cost = app.job(YT).body["cost_krw"].clone();
    let calls = app.gemini.generate_calls().len();

    let d = app.app_call("DELETE", "/settings/api-key", json!(null));
    assert_eq!(d.status, 200, "DELETE /settings/api-key: {d:?}");

    let r = app.create_job(YT, None, "Attention Is All You Need, explained", 12_000);
    assert_eq!(r.status, 200, "POST /jobs again without a key: {r:?}");
    assert_eq!(r.body["reused"], true, "{r:?}");
    assert_eq!(r.body["stage"], "ready", "{r:?}");
    assert_eq!(r.body["video_key"], video_key.as_str());
    assert_eq!(cues(&app.subtitles(YT, &video_key)), first);

    sleep_ms(1_000);
    assert_eq!(app.gemini.generate_calls().len(), calls, "Gemini was called again");
    let j = app.job(YT);
    assert_eq!(j.body["stage"], "ready");
    assert_eq!(j.body["cost_krw"], cost, "cost grew on reuse");
}
