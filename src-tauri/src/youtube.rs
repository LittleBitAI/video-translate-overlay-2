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

/// Asks YouTube's player API for the stream list, as the Android VR client
/// does (its stream URLs need no signature deciphering), and picks AAC audio.
///
/// ponytail: YouTube changes this API without notice; when it breaks, jobs
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
            "context": {"client": {"clientName": "ANDROID_VR", "clientVersion": "1.62.27", "androidSdkVersion": 32, "hl": "en"}},
            "contentCheckOk": true,
            "racyCheckOk": true
        })
        .to_string();
        let r = http::send(
            Request::post("https://www.youtube.com/youtubei/v1/player?prettyPrint=false", body.as_bytes())
                .header("Content-Type", "application/json")
                .header("User-Agent", "com.google.android.apps.youtube.vr.oculus/1.62.27 (Linux; U; Android 12L) gzip"),
        )
        .map_err(|e| unavailable(e.0))?;
        let v: Value = serde_json::from_slice(&r.bytes(8 << 20).map_err(|e| unavailable(e.0))?).map_err(|e| unavailable(e.to_string()))?;
        pick_audio(&v).ok_or_else(|| {
            let status = v["playabilityStatus"]["status"].as_str().unwrap_or("?");
            if v["playabilityStatus"]["liveStreamability"].is_object() || v["videoDetails"]["isLive"] == true {
                return refuse("live", "라이브 방송은 번역하지 않습니다.");
            }
            unavailable(format!("no audio stream (playability {status})"))
        })
    }
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
mod tests {
    use super::*;

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
