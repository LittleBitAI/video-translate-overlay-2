//! Media the app may download: URL policy, probing (direct file, HLS, or a
//! reason it cannot be had), and the download itself.
//!
//! Every URL fetched here (the media URL, each redirect hop, each variant
//! playlist, init section and segment) passes [`check_media_url`] first, so a
//! page can never make the app request this device or its private network.

use crate::http::{self, Request};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
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
    checked(raw, allow_loopback).map(|(url, _)| url)
}

fn private_address() -> Refusal {
    refuse("private_address", "기기 자신이나 사설망을 가리키는 주소는 받지 않습니다.")
}

/// [`check_media_url`], plus the public address a host name resolved to, so
/// the request can go to exactly the address that was checked.
fn checked(raw: &str, allow_loopback: bool) -> Result<(Url, Option<SocketAddr>), Refusal> {
    let url = Url::parse(raw).map_err(|_| refuse("bad_url", "영상 주소를 읽을 수 없습니다."))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(refuse("bad_url", "http나 https 주소만 받을 수 있습니다."));
    }
    if allow_loopback {
        return Ok((url, None));
    }
    let port = url.port_or_known_default().unwrap_or(443);
    let addr = match url.host() {
        Some(url::Host::Ipv4(ip)) => (!is_private(IpAddr::V4(ip))).then_some(None),
        Some(url::Host::Ipv6(ip)) => (!is_private(IpAddr::V6(ip))).then_some(None),
        Some(url::Host::Domain(d)) => {
            let d = d.to_ascii_lowercase();
            if d == "localhost" || [".localhost", ".local", ".internal"].iter().any(|s| d.ends_with(s)) {
                None
            } else {
                match (d.as_str(), port).to_socket_addrs() {
                    // Any private answer refuses the name: the connection could pick that one.
                    Ok(addrs) => {
                        let addrs: Vec<SocketAddr> = addrs.collect();
                        (!addrs.iter().any(|a| is_private(a.ip()))).then(|| addrs.first().copied())
                    }
                    // A name that does not resolve cannot be connected to either.
                    Err(_) => Some(None),
                }
            }
        }
        None => None,
    };
    addr.map(|a| (url, a)).ok_or_else(private_address)
}

/// Where a request for `url` actually goes. Plain HTTP is sent to the checked
/// address with the name in `Host`, so a name that resolves differently the
/// second time (DNS rebinding) cannot redirect it. HTTPS keeps the name: the
/// certificate must prove that name, so a rebound connection fails its
/// handshake before any request is sent, and [`get`] also checks the address
/// actually connected to.
fn pinned(url: &Url, addr: Option<SocketAddr>) -> (Url, Option<String>) {
    match addr {
        Some(a) if url.scheme() == "http" => {
            let mut to = url.clone();
            let _ = to.set_ip_host(a.ip());
            let host = match url.port() {
                Some(p) => format!("{}:{p}", url.host_str().unwrap_or("")),
                None => url.host_str().unwrap_or("").to_string(),
            };
            (to, Some(host))
        }
        _ => (url.clone(), None),
    }
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
/// Also returns the URL that finally answered: relative references in what it
/// serves resolve against that URL, not the one first asked for.
fn get(url: &str, range: Option<(u64, u64)>, allow_loopback: bool, timeout_ms: i32) -> Result<(http::Response, Url), Refusal> {
    let (mut current, mut addr) = checked(url, allow_loopback)?;
    for _ in 0..=MAX_REDIRECTS {
        let (to, host) = pinned(&current, addr);
        let mut req = Request::get(to.as_str());
        req.timeout_ms = timeout_ms;
        req.follow_redirects = false;
        if let Some(h) = &host {
            req = req.header("Host", h);
        }
        if let Some((off, len)) = range {
            req = req.header("Range", &format!("bytes={off}-{}", off + len.max(1) - 1));
        }
        let r = http::send(req).map_err(unreachable)?;
        // The address the stack really connected to, before any of the answer is used.
        if !allow_loopback && r.remote_ip.is_some_and(is_private) {
            return Err(private_address());
        }
        if (300..400).contains(&r.status) {
            let to = r.location.clone().ok_or_else(|| unreachable(format!("HTTP {} without Location", r.status)))?;
            let to = current.join(to.trim()).map_err(|_| unreachable("bad redirect"))?;
            (current, addr) = checked(to.as_str(), allow_loopback)?;
            continue;
        }
        if !(200..300).contains(&r.status) {
            return Err(unreachable(format!("HTTP {}", r.status)));
        }
        return Ok((r, current));
    }
    Err(unreachable("too many redirects"))
}

/// `(content type, body up to limit, final URL)`.
fn fetch(url: &str, range: Option<(u64, u64)>, limit: usize, allow_loopback: bool) -> Result<(Option<String>, Vec<u8>, Url), Refusal> {
    let (r, at) = get(url, range, allow_loopback, 30_000)?;
    let ct = r.content_type.clone();
    Ok((ct, r.bytes(limit).map_err(unreachable)?, at))
}

fn drm() -> Refusal {
    refuse("drm", "DRM으로 보호된 영상이라 오디오를 내려받을 수 없습니다.")
}

/// Looks at what the URL serves and decides whether and how to download it.
pub fn probe(url: &Url, allow_loopback: bool) -> Result<Source, Refusal> {
    let (ct, head, _) = fetch(url.as_str(), Some((0, 65_536)), 65_536, allow_loopback)?;
    let ct = ct.unwrap_or_default().to_ascii_lowercase();
    let text = String::from_utf8_lossy(&head[..head.len().min(4096)]).to_string();
    if ct.contains("mpegurl") || text.trim_start().starts_with("#EXTM3U") {
        let (_, whole, at) = fetch(url.as_str(), None, MAX_PLAYLIST, allow_loopback)?;
        return probe_hls(&at, &String::from_utf8_lossy(&whole), allow_loopback, 0);
    }
    if head.len() > 8 && &head[4..8] == b"ftyp" && mp4_protected(url.as_str(), &head, allow_loopback) {
        return Err(drm());
    }
    if ct.contains("dash+xml") || text.contains("<MPD") {
        return Err(refuse("dash", "DASH 방식 영상은 아직 내려받을 수 없습니다."));
    }
    if ct.starts_with("text/html") || text.trim_start().starts_with('<') {
        return Err(refuse("not_media", "이 주소는 영상 파일이 아닌 웹 페이지입니다."));
    }
    Ok(Source::File { url: url.to_string(), ext: file_ext(&ct, url, &head) })
}

/// Whether an MP4's `moov` marks a track as protected (Common Encryption puts
/// `sinf`/`tenc` under an `enca`/`encv` sample entry, and `pssh` beside it).
/// The top-level boxes are walked by size; a `moov` past the first read (a file
/// that is not "fast start") is fetched by range.
fn mp4_protected(url: &str, head: &[u8], allow_loopback: bool) -> bool {
    const MARKS: [&[u8; 4]; 5] = [b"sinf", b"tenc", b"enca", b"encv", b"pssh"];
    // Bytes `[off, off + n)`, from the head when it has them. A failed or
    // non-ranged answer past the head means the walk cannot go on: `None`.
    let read = |off: u64, n: u64| -> Option<Vec<u8>> {
        let end = (off + n) as usize;
        if end <= head.len() {
            return Some(head[off as usize..end].to_vec());
        }
        let (r, _) = get(url, Some((off, n)), allow_loopback, 30_000).ok()?;
        (r.status == 206).then(|| r.bytes(n as usize).ok()).flatten()
    };
    let mut off = 0u64;
    for _ in 0..64 {
        let Some(h) = read(off, 16).or_else(|| read(off, 8)).filter(|h| h.len() >= 8) else { return false };
        let (mut size, mut hdr) = (u64::from(u32::from_be_bytes([h[0], h[1], h[2], h[3]])), 8);
        if size == 1 && h.len() >= 16 {
            size = u64::from_be_bytes(h[8..16].try_into().unwrap());
            hdr = 16;
        }
        if &h[4..8] == b"moov" {
            // size 0: the box runs to the end of the file.
            let n = if size == 0 { MAX_PLAYLIST as u64 } else { size.min(MAX_PLAYLIST as u64) };
            let body = read(off, n).or_else(|| head.get(off as usize..).map(<[u8]>::to_vec));
            // ponytail: a byte scan of `moov`, not a full box parse; a chunk offset that spells a mark refuses a clear file.
            return body.is_some_and(|b| b.windows(4).any(|w| MARKS.iter().any(|m| w == *m)));
        }
        if size < hdr {
            return false;
        }
        off += size;
    }
    false
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
                return Err(drm());
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
        let (_, body, at) = fetch(next.as_str(), None, MAX_PLAYLIST, allow_loopback)?;
        return probe_hls(&at, &String::from_utf8_lossy(&body), allow_loopback, depth + 1);
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
    let get = |url: &str, range| get(url, range, allow_loopback, 60_000).map(|(r, _)| r).map_err(|r| format!("{}: {}", r.code, r.message));
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
#[path = "media_probe_tests.rs"]
mod probe_tests;

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
    fn plain_http_goes_to_the_checked_address() {
        let addr: SocketAddr = "93.184.215.14:8080".parse().unwrap();
        let (to, host) = pinned(&Url::parse("http://media.example.org:8080/v/a.m4a?x=1").unwrap(), Some(addr));
        assert_eq!(to.as_str(), "http://93.184.215.14:8080/v/a.m4a?x=1");
        assert_eq!(host.as_deref(), Some("media.example.org:8080"));
        let (to, host) = pinned(&Url::parse("http://media.example.org/a.m4a").unwrap(), Some("[2606:2800::1]:80".parse().unwrap()));
        assert_eq!((to.as_str(), host.as_deref()), ("http://[2606:2800::1]/a.m4a", Some("media.example.org")));
        // HTTPS keeps the name: its certificate has to prove it.
        let https = Url::parse("https://media.example.org/a.m4a").unwrap();
        assert_eq!(pinned(&https, Some(addr)), (https.clone(), None));
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
