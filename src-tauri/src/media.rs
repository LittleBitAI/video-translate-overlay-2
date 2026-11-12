//! Media the app may download: URL policy, probing (direct file, HLS, or a
//! reason it cannot be had), and the download itself.
//!
//! Every URL fetched here (the media URL, each redirect hop, each variant
//! playlist, init section and segment) passes [`check_media_url`] first, so a
//! page can never make the app request this device or its private network.

use crate::http::{self, Request};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{IpAddr, ToSocketAddrs};
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

/// One piece to fetch: a URL and, for `EXT-X-BYTERANGE`, the `(offset, length)` inside it.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    pub url: String,
    pub range: Option<(u64, u64)>,
}

/// A source the downloader knows how to fetch.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    File { url: String, ext: &'static str },
    /// The init section (if any) first, then the segments, in playlist order.
    Hls { parts: Vec<Part>, ext: &'static str },
}

/// Whole playlists are read (a 64 KiB head can cut a long VOD before `#EXT-X-ENDLIST`).
const MAX_PLAYLIST: usize = 8 << 20;
const MAX_REDIRECTS: usize = 5;

/// Only http(s), and never this device or a private network unless allowed
/// (the contract host serves media from loopback). Host names are resolved and
/// refused if any address they resolve to is private.
pub fn check_media_url(raw: &str, allow_loopback: bool) -> Result<Url, Refusal> {
    let url = Url::parse(raw).map_err(|_| refuse("bad_url", "영상 주소를 읽을 수 없습니다."))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(refuse("bad_url", "http나 https 주소만 받을 수 있습니다."));
    }
    if allow_loopback {
        return Ok(url);
    }
    let private = match url.host() {
        Some(url::Host::Ipv4(ip)) => is_private(IpAddr::V4(ip)),
        Some(url::Host::Ipv6(ip)) => is_private(IpAddr::V6(ip)),
        Some(url::Host::Domain(d)) => {
            let d = d.to_ascii_lowercase();
            d == "localhost"
                || [".localhost", ".local", ".internal"].iter().any(|s| d.ends_with(s))
                || (d.as_str(), url.port_or_known_default().unwrap_or(443))
                    .to_socket_addrs()
                    // A name that does not resolve cannot be connected to either.
                    .map(|mut addrs| addrs.any(|a| is_private(a.ip())))
                    .unwrap_or(false)
        }
        None => true,
    };
    // ponytail: the HTTP stack resolves the name again when it connects (a DNS
    // rebinding window of milliseconds); connect to the checked address if that matters.
    if private {
        return Err(refuse("private_address", "기기 자신이나 사설망을 가리키는 주소는 받지 않습니다."));
    }
    Ok(url)
}

fn is_private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            let o = v.octets();
            v.is_loopback() || v.is_private() || v.is_link_local() || v.is_unspecified() || v.is_broadcast() || o[0] == 100 && o[1] & 0xC0 == 64
        }
        IpAddr::V6(v) => {
            let s = v.segments();
            v.is_loopback()
                || v.is_unspecified()
                || (s[0] & 0xFE00) == 0xFC00
                || (s[0] & 0xFFC0) == 0xFE80
                || v.to_ipv4_mapped().is_some_and(|m| is_private(IpAddr::V4(m)))
        }
    }
}

fn unreachable(e: impl std::fmt::Display) -> Refusal {
    refuse("unreachable", format!("영상 주소에 닿을 수 없습니다: {e}"))
}

/// GET with redirects followed by hand, so every hop passes [`check_media_url`].
fn get(url: &str, range: Option<(u64, u64)>, allow_loopback: bool, timeout_ms: i32) -> Result<http::Response, Refusal> {
    let mut current = check_media_url(url, allow_loopback)?;
    for _ in 0..=MAX_REDIRECTS {
        let mut req = Request::get(current.as_str());
        req.timeout_ms = timeout_ms;
        req.follow_redirects = false;
        if let Some((off, len)) = range {
            req = req.header("Range", &format!("bytes={off}-{}", off + len.max(1) - 1));
        }
        let r = http::send(req).map_err(unreachable)?;
        if (300..400).contains(&r.status) {
            let to = r.location.clone().ok_or_else(|| unreachable(format!("HTTP {} without Location", r.status)))?;
            let to = current.join(to.trim()).map_err(|_| unreachable("bad redirect"))?;
            current = check_media_url(to.as_str(), allow_loopback)?;
            continue;
        }
        if !(200..300).contains(&r.status) {
            return Err(unreachable(format!("HTTP {}", r.status)));
        }
        return Ok(r);
    }
    Err(unreachable("too many redirects"))
}

fn fetch(url: &str, range: Option<(u64, u64)>, limit: usize, allow_loopback: bool) -> Result<(Option<String>, Vec<u8>), Refusal> {
    let r = get(url, range, allow_loopback, 30_000)?;
    let ct = r.content_type.clone();
    Ok((ct, r.bytes(limit).map_err(unreachable)?))
}

/// Looks at what the URL serves and decides whether and how to download it.
pub fn probe(url: &Url, allow_loopback: bool) -> Result<Source, Refusal> {
    let (ct, head) = fetch(url.as_str(), Some((0, 65_536)), 65_536, allow_loopback)?;
    let ct = ct.unwrap_or_default().to_ascii_lowercase();
    let text = String::from_utf8_lossy(&head[..head.len().min(4096)]).to_string();
    if ct.contains("mpegurl") || text.trim_start().starts_with("#EXTM3U") {
        let (_, whole) = fetch(url.as_str(), None, MAX_PLAYLIST, allow_loopback)?;
        return probe_hls(url, &String::from_utf8_lossy(&whole), allow_loopback, 0);
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
    // NAME= only at an attribute start, so `URI` never matches inside `KEYURI`.
    let pat = format!("{name}=");
    let mut from = 0;
    while let Some(i) = line[from..].find(&pat) {
        let at = from + i;
        if matches!(line[..at].chars().last(), None | Some(',') | Some(':')) {
            let rest = &line[at + pat.len()..];
            return Some(if let Some(q) = rest.strip_prefix('"') { &q[..q.find('"').unwrap_or(q.len())] } else { rest.split(',').next().unwrap_or("") });
        }
        from = at + 1;
    }
    None
}

/// `<length>[@<offset>]`; without an offset the range follows the previous one of the same URI.
fn byte_range(spec: &str, url: &str, ends: &mut HashMap<String, u64>) -> Result<(u64, u64), Refusal> {
    let bad = || refuse("not_media", "재생 목록의 바이트 범위를 읽을 수 없습니다.");
    let (len, off) = match spec.trim().split_once('@') {
        Some((l, o)) => (l.parse::<u64>().map_err(|_| bad())?, Some(o.parse::<u64>().map_err(|_| bad())?)),
        None => (spec.trim().parse::<u64>().map_err(|_| bad())?, None),
    };
    // No earlier range of this URI: start of the resource, as players do.
    let off = off.unwrap_or_else(|| ends.get(url).copied().unwrap_or(0));
    ends.insert(url.to_string(), off + len);
    Ok((off, len))
}

pub fn probe_hls(base: &Url, playlist: &str, allow_loopback: bool, depth: u8) -> Result<Source, Refusal> {
    let join = |u: &str| -> Result<Url, Refusal> {
        let abs = base.join(u).map_err(|_| refuse("bad_url", "재생 목록의 주소를 읽을 수 없습니다."))?;
        check_media_url(abs.as_str(), allow_loopback)
    };
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
        let (_, body) = fetch(next.as_str(), None, MAX_PLAYLIST, allow_loopback)?;
        return probe_hls(&next, &String::from_utf8_lossy(&body), allow_loopback, depth + 1);
    }
    if !lines.iter().any(|l| l.starts_with("#EXT-X-ENDLIST")) {
        return Err(refuse("live", "라이브 방송은 번역하지 않습니다."));
    }
    let mut ends = HashMap::new();
    let mut parts = Vec::new();
    let mut init = false;
    let mut pending: Option<&str> = None;
    for l in &lines {
        if l.starts_with("#EXT-X-MAP") {
            // ponytail: only the first init section is kept; a mid-stream codec switch is not followed.
            if init {
                continue;
            }
            let url = join(attr(l, "URI").ok_or_else(|| refuse("not_media", "재생 목록의 초기화 조각에 주소가 없습니다."))?)?.to_string();
            let range = attr(l, "BYTERANGE").map(|r| byte_range(r, &url, &mut ends)).transpose()?;
            parts.push(Part { url, range });
            init = true;
        } else if let Some(spec) = l.strip_prefix("#EXT-X-BYTERANGE:") {
            pending = Some(spec);
        } else if !l.starts_with('#') {
            let url = join(l)?.to_string();
            let range = pending.take().map(|r| byte_range(r, &url, &mut ends)).transpose()?;
            parts.push(Part { url, range });
        }
    }
    let segments = &parts[usize::from(init)..];
    if segments.is_empty() {
        return Err(refuse("not_media", "재생 목록에 조각이 없습니다."));
    }
    let first = segments[0].url.to_ascii_lowercase();
    let ext = if init || first.contains(".m4s") || first.contains(".mp4") || first.contains(".m4a") {
        "mp4"
    } else if first.contains(".aac") {
        "aac"
    } else if first.contains(".mp3") {
        "mp3"
    } else {
        "ts"
    };
    Ok(Source::Hls { parts, ext })
}

impl Source {
    pub fn ext(&self) -> &'static str {
        match self {
            Source::File { ext, .. } | Source::Hls { ext, .. } => ext,
        }
    }
}

/// Downloads `src` into `dest`, reporting a 0..=100 percent. Every request
/// (and redirect) is checked against the same URL policy as the probe.
pub fn download(src: &Source, dest: &Path, allow_loopback: bool, progress: &mut dyn FnMut(i64)) -> Result<(), String> {
    let mut out = std::io::BufWriter::new(std::fs::File::create(dest).map_err(|e| e.to_string())?);
    let mut buf = vec![0u8; 1 << 16];
    let get = |url: &str, range| get(url, range, allow_loopback, 60_000).map_err(|r| format!("{}: {}", r.code, r.message));
    match src {
        Source::File { url, .. } => {
            let mut r = get(url, None)?;
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
        Source::Hls { parts, .. } => {
            let io = |e: std::io::Error| e.to_string();
            for (i, part) in parts.iter().enumerate() {
                let mut r = get(&part.url, part.range)?;
                match part.range {
                    Some((off, len)) => {
                        // A server that ignores Range answers 200 with the whole file.
                        if r.status != 206 {
                            std::io::copy(&mut (&mut r).take(off), &mut std::io::sink()).map_err(io)?;
                        }
                        let n = std::io::copy(&mut (&mut r).take(len), &mut out).map_err(io)?;
                        if n < len {
                            return Err(format!("range of {} ended at {n} of {len} bytes", part.url));
                        }
                    }
                    None => {
                        std::io::copy(&mut r, &mut out).map_err(io)?;
                    }
                }
                progress(((i + 1) * 100 / parts.len()) as i64);
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
        assert_eq!(probe_hls(&base(), drm, true, 0).unwrap_err().code, "drm");
        let aes = "#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"k.bin\"\n#EXTINF:4,\na.ts\n#EXT-X-ENDLIST";
        assert_eq!(probe_hls(&base(), aes, true, 0).unwrap_err().code, "encrypted");
        let live = "#EXTM3U\n#EXTINF:4,\na.ts\n";
        assert_eq!(probe_hls(&base(), live, true, 0).unwrap_err().code, "live");
    }

    #[test]
    fn vod_hls_lists_absolute_segments() {
        let vod = "#EXTM3U\n#EXTINF:4,\nseg0.aac\n#EXTINF:4,\n/x/seg1.aac\n#EXT-X-ENDLIST";
        let part = |u: &str| Part { url: u.into(), range: None };
        assert_eq!(
            probe_hls(&base(), vod, true, 0).unwrap(),
            Source::Hls { parts: vec![part("https://cdn.example.org/v/seg0.aac"), part("https://cdn.example.org/x/seg1.aac")], ext: "aac" }
        );
    }

    #[test]
    fn byte_ranges_are_kept_and_continue_from_the_previous_one() {
        let vod = "#EXTM3U\n#EXT-X-MAP:URI=\"media.mp4\",BYTERANGE=\"720@0\"\n#EXTINF:4,\n#EXT-X-BYTERANGE:1000@720\nmedia.mp4\n\
                   #EXTINF:4,\n#EXT-X-BYTERANGE:500\nmedia.mp4\n#EXT-X-ENDLIST";
        let Source::Hls { parts, ext } = probe_hls(&base(), vod, true, 0).unwrap() else { panic!("not HLS") };
        assert_eq!(ext, "mp4");
        let ranges: Vec<_> = parts.iter().map(|p| p.range).collect();
        assert_eq!(ranges, [Some((0, 720)), Some((720, 1000)), Some((1720, 500))]);
    }

    #[test]
    fn a_long_vod_playlist_is_not_mistaken_for_live() {
        let mut vod = String::from("#EXTM3U\n");
        for i in 0..5_000 {
            vod.push_str(&format!("#EXTINF:4.000,\nsegment-{i:06}.aac\n"));
        }
        vod.push_str("#EXT-X-ENDLIST\n");
        assert!(vod.len() > 65_536, "longer than the probe's first read");
        let Source::Hls { parts, .. } = probe_hls(&base(), &vod, true, 0).unwrap() else { panic!("not HLS") };
        assert_eq!(parts.len(), 5_000);
    }

    #[test]
    fn private_addresses_are_refused_wherever_they_appear() {
        for u in ["http://127.0.0.1/a.wav", "http://10.1.2.3/a", "http://192.168.0.2/a", "http://[::1]/a", "http://[::ffff:127.0.0.1]/a", "http://localhost/a", "http://x.internal/a"] {
            assert_eq!(check_media_url(u, false).unwrap_err().code, "private_address", "{u}");
        }
        assert!(check_media_url("http://127.0.0.1/a.wav", true).is_ok());
        assert_eq!(check_media_url("file:///etc/passwd", true).unwrap_err().code, "bad_url");
        // A public playlist cannot point a segment, an init section or a variant at the private network.
        for sneaky in [
            "#EXTM3U\n#EXTINF:4,\nhttp://127.0.0.1:8080/a.aac\n#EXT-X-ENDLIST",
            "#EXTM3U\n#EXT-X-MAP:URI=\"http://10.0.0.1/init.mp4\"\n#EXTINF:4,\na.m4s\n#EXT-X-ENDLIST",
            "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1\nhttp://192.168.1.1/v.m3u8",
        ] {
            assert_eq!(probe_hls(&base(), sneaky, false, 0).unwrap_err().code, "private_address", "{sneaky}");
        }
    }
}
