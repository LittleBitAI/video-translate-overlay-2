//! A live YouTube broadcast is refused (A3) even when its player response
//! lists an audio stream with a plain URL.

use super::*;

fn player(details: Value) -> Value {
    json!({
        "videoDetails": details,
        "playabilityStatus": {"status": "OK"},
        "streamingData": {"adaptiveFormats": [{"mimeType": "audio/mp4; codecs=\"mp4a.40.2\"", "bitrate": 128000, "url": "https://cdn.example.org/live.m4a"}]}
    })
}

#[test]
fn a_live_broadcast_with_audio_is_still_refused() {
    for details in [json!({"isLive": true}), json!({"isUpcoming": true})] {
        let r = audio_of(&player(details.clone()));
        assert_eq!(r.err().flatten().map(|e| e.code), Some("live"), "{details}");
    }
    let mut live = player(json!({}));
    live["playabilityStatus"]["liveStreamability"] = json!({"liveStreamabilityRenderer": {}});
    assert_eq!(audio_of(&live).err().flatten().map(|e| e.code), Some("live"));
}

#[test]
fn a_recorded_lecture_gives_its_audio() {
    // A finished broadcast is a recording (isLiveContent stays true): it is allowed.
    assert_eq!(audio_of(&player(json!({"isLiveContent": true}))).ok().as_deref(), Some("https://cdn.example.org/live.m4a"));
    assert!(matches!(audio_of(&json!({"videoDetails": {}})), Err(None)));
}
