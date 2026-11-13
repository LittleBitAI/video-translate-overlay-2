//! YouTube pages: recognising them, and finding their audio-only stream.

use crate::http::{self, Request};
use crate::media::{refuse, Refusal};
use serde_json::{json, Value};

pub fn is_youtube(u: &url::Url) -> bool {
    let h = u.host_str().unwrap_or("").to_ascii_lowercase();
    h == "youtu.be" || h == "youtube.com" || h.ends_with(".youtube.com") || h == "youtube-nocookie.com" || h.ends_with(".youtube-nocookie.com")
}

/// The 11-character id of the video a YouTube URL shows.
pub fn video_id(u: &url::Url) -> Option<String> {
    if !is_youtube(u) {
        return None;
    }
    let ok = |s: &str| (s.len() == 11 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')).then(|| s.to_string());
    if u.host_str() == Some("youtu.be") {
        return u.path_segments()?.next().and_then(ok);
    }
    if let Some((_, v)) = u.query_pairs().find(|(k, _)| k == "v") {
        return ok(&v);
    }
    let mut seg = u.path_segments()?;
    match seg.next()? {
        "shorts" | "embed" | "live" | "v" => seg.next().and_then(ok),
        _ => None,
    }
}

/// A recorded audio URL issued to the browser's YouTube session. The media
/// policy and probe still check its address, redirects, content and DRM.
pub fn browser_audio_url(raw: &str) -> Option<&str> {
    let u = url::Url::parse(raw).ok()?;
    let param = |name: &str| u.query_pairs().find(|(k, _)| k == name).map(|(_, v)| v.into_owned());
    (u.scheme() == "https"
        && u.host_str()?.ends_with(".googlevideo.com")
        && u.path() == "/videoplayback"
        && param("source").as_deref() == Some("youtube")
        && param("mime").is_some_and(|m| m.starts_with("audio/"))
        && !u.query_pairs().any(|(k, _)| k == "live" || k == "sq")
        && stream_duration_ms(raw).is_some())
    .then_some(raw)
}

/// Fallback for pages that did not supply their session's signed audio URL.
///
/// shortcut: YouTube changes this API without notice; when it breaks, jobs
/// get the `youtube_unavailable` reason (R20) until the client is updated.
pub struct InnertubeExtractor;

impl crate::AudioExtractor for InnertubeExtractor {
    fn audio_url(&self, page_url: &str) -> Result<String, Refusal> {
        let unavailable = |why: String| {
            crate::log(format!("youtube extractor: {why}"));
            refuse("youtube_unavailable", "유튜브에서 이 영상의 오디오를 받을 수 없습니다.")
        };
        let id = url::Url::parse(page_url).ok().as_ref().and_then(video_id).ok_or_else(|| unavailable("no video id".into()))?;
        let body = json!({
            "videoId": id,
            "context": {"client": {"clientName": "VISIONOS", "clientVersion": "1.02", "deviceMake": "Apple", "deviceModel": "RealityDevice17,1", "osName": "visionOS", "osVersion": "26.5.23O471", "hl": "en"}},
            "contentCheckOk": true,
            "racyCheckOk": true
        })
        .to_string();
        let r = http::send(
            Request::post("https://www.youtube.com/youtubei/v1/player?prettyPrint=false", body.as_bytes())
                .header("Content-Type", "application/json")
                .header("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15"),
        )
        .map_err(|e| unavailable(e.0))?;
        let v: Value = serde_json::from_slice(&r.bytes(8 << 20).map_err(|e| unavailable(e.0))?).map_err(|e| unavailable(e.to_string()))?;
        audio_of(&v).map_err(|why| why.unwrap_or_else(|| unavailable(format!("no audio stream (playability {})", v["playabilityStatus"]["status"].as_str().unwrap_or("?")))))
    }
}

/// The audio stream of a player response. A live (or not yet started)
/// broadcast is refused whether or not it lists audio (A3); `Err(None)` when
/// there is no audio stream to take.
fn audio_of(player: &Value) -> Result<String, Option<Refusal>> {
    let details = &player["videoDetails"];
    if details["isLive"] == true || details["isUpcoming"] == true || player["playabilityStatus"]["liveStreamability"].is_object() {
        return Err(Some(refuse("live", "라이브 방송은 번역하지 않습니다.")));
    }
    pick_audio(player).ok_or(None)
}

/// A YouTube stream URL's own length, from its `dur` parameter (seconds):
/// the lecture's, even when the player was showing an ad when 영상감지 was pressed.
pub fn stream_duration_ms(stream_url: &str) -> Option<i64> {
    let u = url::Url::parse(stream_url).ok()?;
    let secs: f64 = u.query_pairs().find(|(k, _)| k == "dur")?.1.parse().ok()?;
    (secs.is_finite() && secs > 0.0).then(|| (secs * 1000.0).round() as i64)
}

/// The best audio-only stream with a plain URL, preferring AAC (itag 140) that every decoder reads.
fn pick_audio(player: &Value) -> Option<String> {
    let formats = player["streamingData"]["adaptiveFormats"].as_array()?;
    formats
        .iter()
        .filter(|f| f["mimeType"].as_str().is_some_and(|m| m.starts_with("audio/")) && f["url"].is_string())
        .max_by_key(|f| {
            let aac = f["mimeType"].as_str().unwrap_or("").contains("mp4a");
            (aac, f["bitrate"].as_i64().unwrap_or(0))
        })
        .and_then(|f| f["url"].as_str().map(String::from))
}

#[cfg(test)]
#[path = "youtube_live_tests.rs"]
mod live_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_hint_requires_recorded_youtube_audio() {
        let good = "https://rr1.googlevideo.com/videoplayback?source=youtube&mime=audio%2Fmp4&dur=19.021&sig=kept";
        assert_eq!(browser_audio_url(good), Some(good));
        for bad in [good.replace("https:", "http:"), good.replace("rr1.googlevideo.com", "googlevideo.com.evil.test"),
            good.replace("source=youtube", "source=yt_live_broadcast"), good.replace("audio%2Fmp4", "video%2Fmp4"),
            good.replace("dur=19.021", "dur=NaN"), good.replace("dur=19.021", "dur=0"),
            format!("{good}&live=1"), format!("{good}&sq=42")] {
            assert!(browser_audio_url(&bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn aac_audio_is_preferred() {
        let p = json!({"streamingData": {"adaptiveFormats": [
            {"mimeType": "video/mp4", "url": "v", "bitrate": 9},
            {"mimeType": "audio/webm; codecs=\"opus\"", "url": "opus", "bitrate": 160000},
            {"mimeType": "audio/mp4; codecs=\"mp4a.40.2\"", "url": "aac", "bitrate": 130000},
            {"mimeType": "audio/mp4; codecs=\"mp4a.40.5\"", "signatureCipher": "x", "bitrate": 999999}
        ]}});
        assert_eq!(pick_audio(&p).as_deref(), Some("aac"));
    }
}
