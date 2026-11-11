//! Media the app may download: URL policy, probing (direct file, HLS, or a
//! reason it cannot be had), and the download itself.

use crate::http::{self, Request};
use std::io::{Read, Write};
use std::net::IpAddr;
use std::path::Path;
use url::Url;

/// Why a video's audio cannot be downloaded; `code` goes to the overlay with `message`.
#[derive(Debug, Clone, PartialEq)]
pub struct Refusal {
    pub code: &'static str,
    pub message: String,
}

pub fn refuse(code: &'static str, message: impl Into<String>) -> Refusal {
    Refusal { code, message: message.into() }
}

/// A source the downloader knows how to fetch.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    File { url: String, ext: &'static str },
    Hls { init: Option<String>, segments: Vec<String>, ext: &'static str },
}

/// Only http(s), and never this device or a private network unless allowed
/// (the contract host serves media from loopback).
pub fn check_media_url(raw: &str, allow_loopback: bool) -> Result<Url, Refusal> {
    let url = Url::parse(raw).map_err(|_| refuse("bad_url", "영상 주소를 읽을 수 없습니다."))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(refuse("bad_url", "http나 https 주소만 받을 수 있습니다."));
    }
    let private = match url.host() {
        Some(url::Host::Ipv4(ip)) => is_private(IpAddr::V4(ip)),
        Some(url::Host::Ipv6(ip)) => is_private(IpAddr::V6(ip)),
        Some(url::Host::Domain(d)) => d.eq_ignore_ascii_case("localhost") || d.ends_with(".localhost") || d.ends_with(".local"),
        None => true,
    };
    // ponytail: a public name that resolves to a private address is not caught; resolve-then-connect pinning if that matters.
    if private && !allow_loopback {
        return Err(refuse("private_address", "기기 자신이나 사설망을 가리키는 주소는 받지 않습니다."));
    }
    Ok(url)
}

fn is_private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => v.is_loopback() || v.is_private() || v.is_link_local() || v.is_unspecified() || v.octets()[0] == 100 && v.octets()[1] & 0xC0 == 64,
        IpAddr::V6(v) => v.is_loopback() || v.is_unspecified() || (v.segments()[0] & 0xFE00) == 0xFC00 || (v.segments()[0] & 0xFFC0) == 0xFE80,
    }
}

fn fetch_text(url: &str) -> Result<(Option<String>, Vec<u8>), Refusal> {
    let unreachable = |e: String| refuse("unreachable", format!("영상 주소에 닿을 수 없습니다: {e}"));
    let r = http::send(Request::get(url).header("Range", "bytes=0-65535")).map_err(|e| unreachable(e.0))?;
    if !(200..300).contains(&r.status) {
        return Err(unreachable(format!("HTTP {}", r.status)));
    }
    let ct = r.content_type.clone();
    Ok((ct, r.bytes(65_536).map_err(|e| unreachable(e.0))?))
}

/// Looks at what the URL serves and decides whether and how to download it.
pub fn probe(url: &Url, allow_loopback: bool) -> Result<Source, Refusal> {
    let (ct, head) = fetch_text(url.as_str())?;
    let ct = ct.unwrap_or_default().to_ascii_lowercase();
    let text = String::from_utf8_lossy(&head[..head.len().min(4096)]).to_string();
    if ct.contains("mpegurl") || text.trim_start().starts_with("#EXTM3U") {
        return probe_hls(url, &String::from_utf8_lossy(&head), allow_loopback, 0);
    }
    if ct.contains("dash+xml") || text.contains("<MPD") {
        return Err(refuse("dash", "DASH 방식 영상은 아직 내려받을 수 없습니다."));
    }
    if ct.starts_with("text/html") || text.trim_start().starts_with('<') {
        return Err(refuse("not_media", "이 주소는 영상 파일이 아닌 웹 페이지입니다."));
    }
    Ok(Source::File { url: url.to_string(), ext: file_ext(&ct, url, &head) })
}

fn file_ext(ct: &str, url: &Url, head: &[u8]) -> &'static str {
    if head.starts_with(b"RIFF") {
        return "wav";
    }
    if head.len() > 8 && &head[4..8] == b"ftyp" {
        return "m4a";
    }
    if head.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        return "webm";
    }
    if head.starts_with(b"ID3") || head.len() > 1 && head[0] == 0xFF && head[1] & 0xE0 == 0xE0 {
        return if head.len() > 1 && head[1] & 0x06 == 0 && !head.starts_with(b"ID3") { "aac" } else { "mp3" };
    }
    if head.first() == Some(&0x47) {
        return "ts";
    }
    let path = url.path().to_ascii_lowercase();
    for (needle, ext) in [("wav", "wav"), ("mp4", "m4a"), ("m4a", "m4a"), ("aac", "aac"), ("mpeg", "mp3"), ("mp3", "mp3"), ("webm", "webm")] {
        if ct.contains(needle) || path.ends_with(&format!(".{needle}")) {
            return ext;
        }
    }
    "mp4"
}

fn attr<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let at = line.find(&format!("{name}="))? + name.len() + 1;
    let rest = &line[at..];
    Some(if let Some(q) = rest.strip_prefix('"') { &q[..q.find('"').unwrap_or(q.len())] } else { rest.split(',').next().unwrap_or("") })
}

fn probe_hls(base: &Url, playlist: &str, allow_loopback: bool, depth: u8) -> Result<Source, Refusal> {
    let join = |u: &str| base.join(u).map_err(|_| refuse("bad_url", "재생 목록의 주소를 읽을 수 없습니다."));
    let lines: Vec<&str> = playlist.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    for l in &lines {
        if l.starts_with("#EXT-X-KEY") || l.starts_with("#EXT-X-SESSION-KEY") {
            let method = attr(l, "METHOD").unwrap_or("");
            if method.starts_with("SAMPLE-AES") || attr(l, "KEYFORMAT").is_some_and(|f| f != "identity") {
                return Err(refuse("drm", "DRM으로 보호된 영상이라 오디오를 내려받을 수 없습니다."));
            }
            if method != "NONE" {
                return Err(refuse("encrypted", "암호화된 영상(AES-128)은 아직 내려받을 수 없습니다."));
            }
        }
    }
    // A master playlist: prefer an audio-only rendition, else the lowest-bandwidth variant.
    if lines.iter().any(|l| l.starts_with("#EXT-X-STREAM-INF")) {
        if depth > 0 {
            return Err(refuse("not_media", "재생 목록이 너무 깊게 이어져 있습니다."));
        }
        let audio = lines.iter().find(|l| l.starts_with("#EXT-X-MEDIA") && l.contains("TYPE=AUDIO")).and_then(|l| attr(l, "URI"));
        let variant = lines
            .windows(2)
            .filter(|w| w[0].starts_with("#EXT-X-STREAM-INF") && !w[1].starts_with('#'))
            .min_by_key(|w| attr(w[0], "BANDWIDTH").and_then(|b| b.parse::<u64>().ok()).unwrap_or(u64::MAX))
            .map(|w| w[1]);
        let next = join(audio.or(variant).ok_or_else(|| refuse("not_media", "재생 목록에 영상이 없습니다."))?)?;
        let next = check_media_url(next.as_str(), allow_loopback)?;
        let (_, body) = fetch_text(next.as_str())?;
        return probe_hls(&next, &String::from_utf8_lossy(&body), allow_loopback, depth + 1);
    }
    if !lines.iter().any(|l| l.starts_with("#EXT-X-ENDLIST")) {
        return Err(refuse("live", "라이브 방송은 번역하지 않습니다."));
    }
    let init = lines.iter().find(|l| l.starts_with("#EXT-X-MAP")).and_then(|l| attr(l, "URI")).map(join).transpose()?;
    let segments = lines.iter().filter(|l| !l.starts_with('#')).map(|l| join(l).map(String::from)).collect::<Result<Vec<_>, _>>()?;
    if segments.is_empty() {
        return Err(refuse("not_media", "재생 목록에 조각이 없습니다."));
    }
    let first = segments[0].to_ascii_lowercase();
    let ext = if init.is_some() || first.contains(".m4s") || first.contains(".mp4") {
        "mp4"
    } else if first.contains(".aac") {
        "aac"
    } else if first.contains(".mp3") {
        "mp3"
    } else {
        "ts"
    };
    Ok(Source::Hls { init: init.map(String::from), segments, ext })
}

impl Source {
    pub fn ext(&self) -> &'static str {
        match self {
            Source::File { ext, .. } | Source::Hls { ext, .. } => ext,
        }
    }
}

fn get(url: &str) -> Result<http::Response, String> {
    let mut req = Request::get(url);
    req.timeout_ms = 60_000;
    let r = http::send(req).map_err(|e| e.0)?;
    if !(200..300).contains(&r.status) {
        return Err(format!("HTTP {} for media", r.status));
    }
    Ok(r)
}

/// Downloads `src` into `dest`, reporting a 0..=100 percent.
pub fn download(src: &Source, dest: &Path, progress: &mut dyn FnMut(i64)) -> Result<(), String> {
    let mut out = std::io::BufWriter::new(std::fs::File::create(dest).map_err(|e| e.to_string())?);
    let mut buf = vec![0u8; 1 << 16];
    match src {
        Source::File { url, .. } => {
            let mut r = get(url)?;
            let total = r.content_length.unwrap_or(0);
            let mut done = 0u64;
            loop {
                let n = r.read(&mut buf).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
                done += n as u64;
                if total > 0 {
                    progress((done * 100 / total) as i64);
                }
            }
            if total > 0 && done < total {
                return Err(format!("download ended at {done} of {total} bytes"));
            }
        }
        Source::Hls { init, segments, .. } => {
            let all: Vec<&String> = init.iter().chain(segments.iter()).collect();
            for (i, seg) in all.iter().enumerate() {
                let mut r = get(seg)?;
                std::io::copy(&mut r, &mut out).map_err(|e| e.to_string())?;
                progress(((i + 1) * 100 / all.len()) as i64);
            }
        }
    }
    out.flush().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Url {
        Url::parse("https://cdn.example.org/v/index.m3u8").unwrap()
    }

    #[test]
    fn hls_refusals_name_their_reason() {
        let drm = "#EXTM3U\n#EXT-X-KEY:METHOD=SAMPLE-AES,URI=\"skd://k\",KEYFORMAT=\"com.apple.streamingkeydelivery\"\n#EXTINF:4,\na.aac\n#EXT-X-ENDLIST";
        assert_eq!(probe_hls(&base(), drm, false, 0).unwrap_err().code, "drm");
        let aes = "#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"k.bin\"\n#EXTINF:4,\na.ts\n#EXT-X-ENDLIST";
        assert_eq!(probe_hls(&base(), aes, false, 0).unwrap_err().code, "encrypted");
        let live = "#EXTM3U\n#EXTINF:4,\na.ts\n";
        assert_eq!(probe_hls(&base(), live, false, 0).unwrap_err().code, "live");
    }

    #[test]
    fn vod_hls_lists_absolute_segments() {
        let vod = "#EXTM3U\n#EXTINF:4,\nseg0.aac\n#EXTINF:4,\n/x/seg1.aac\n#EXT-X-ENDLIST";
        let s = probe_hls(&base(), vod, false, 0).unwrap();
        assert_eq!(
            s,
            Source::Hls { init: None, segments: vec!["https://cdn.example.org/v/seg0.aac".into(), "https://cdn.example.org/x/seg1.aac".into()], ext: "aac" }
        );
    }

    #[test]
    fn private_addresses_are_refused() {
        for u in ["http://127.0.0.1/a.wav", "http://10.1.2.3/a", "http://192.168.0.2/a", "http://[::1]/a", "http://localhost/a"] {
            assert_eq!(check_media_url(u, false).unwrap_err().code, "private_address", "{u}");
        }
        assert!(check_media_url("http://127.0.0.1/a.wav", true).is_ok());
        assert_eq!(check_media_url("file:///etc/passwd", true).unwrap_err().code, "bad_url");
        assert!(check_media_url("https://cdn.example.org/a.m4a", false).is_ok());
    }
}
