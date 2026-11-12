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
    /// The pieces in playlist order, each init section (`EXT-X-MAP`) just before
    /// the first segment it applies to. `sections` has one section from the
    /// first segment, and one more at each `EXT-X-DISCONTINUITY` or init
    /// section change.
    Hls { parts: Vec<Part>, ext: &'static str, sections: Vec<Section> },
}

/// Where a playlist's section starts: at `part` (an index into `parts`), and
/// `at` seconds into the playlist (the `EXTINF`s before it), decoded after the
/// init section `init` (an index into `parts`). Its timestamps begin again
/// there, so players place it by the playlist, and so does the decoder
/// ([`sections_path`]). A section ends where the next one or its init begins.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Section {
    pub part: usize,
    pub at: f64,
    pub init: Option<usize>,
}

/// Beside a downloaded HLS file with sections, one line per section:
/// `start end seconds init_start init_end`, byte offsets into the audio file
/// (the init range empty when the section has none).
pub fn sections_path(audio: &Path) -> std::path::PathBuf {
    let mut s = audio.as_os_str().to_owned();
    s.push(".sections");
    s.into()
}

/// Whole playlists are read (a 64 KiB head can cut a long VOD before `#EXT-X-ENDLIST`).
const MAX_PLAYLIST: usize = 8 << 20;
const MAX_REDIRECTS: usize = 5;

/// Only http(s), and never this device or a private network unless allowed
/// (the contract host serves media from loopback). Host names are resolved and
/// refused if any address they resolve to is private, or if they do not
/// resolve at all (`unreachable`, which a job retries).
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
        Some(url::Host::Ipv4(ip)) => (!is_private(IpAddr::V4(ip))).then_some(Some(SocketAddr::new(ip.into(), port))),
        Some(url::Host::Ipv6(ip)) => (!is_private(IpAddr::V6(ip))).then_some(Some(SocketAddr::new(ip.into(), port))),
        Some(url::Host::Domain(d)) => {
            let d = d.to_ascii_lowercase();
            if d == "localhost" || [".localhost", ".local", ".internal"].iter().any(|s| d.ends_with(s)) {
                None
            } else {
                match (d.as_str(), port).to_socket_addrs().map(|a| a.collect::<Vec<SocketAddr>>()) {
                    // Any private answer refuses the name: the connection could pick that one.
                    Ok(addrs) if !addrs.is_empty() => (!addrs.iter().any(|a| is_private(a.ip()))).then(|| addrs.first().copied()),
                    // No checked address, no request: the HTTP client would resolve the
                    // name again itself, and that answer could be this device.
                    Ok(_) => return Err(unreachable(format!("{d} has no address"))),
                    Err(e) => return Err(unreachable(format!("{d}: {e}"))),
                }
            }
        }
        None => None,
    };
    addr.map(|a| (url, a)).ok_or_else(private_address)
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
        let mut req = Request::get(current.as_str());
        req.timeout_ms = timeout_ms;
        req.follow_redirects = false;
        // Sent to exactly the address that was checked, http and https alike: a
        // name that resolves differently the second time (DNS rebinding) cannot
        // move it. The name still goes in Host, SNI and the certificate check.
        req.connect_to = addr;
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
        let src = probe_hls(&at, &String::from_utf8_lossy(&whole), allow_loopback, 0)?;
        pieces_answer(&src, allow_loopback)?;
        hls_has_sound(&src, allow_loopback)?;
        return Ok(src);
    }
    if head.len() > 8 && &head[4..8] == b"ftyp" {
        let moov = mp4_moov(url.as_str(), &head, allow_loopback)?;
        if moov.as_deref().is_some_and(marked) {
            return Err(drm());
        }
        // No moov (nothing a player could play either), or no sound track in it.
        if !moov.as_deref().is_some_and(mp4_has_sound) {
            return Err(no_sound());
        }
    }
    if ct.contains("dash+xml") || text.contains("<MPD") {
        return Err(refuse("dash", "DASH 방식 영상은 아직 내려받을 수 없습니다."));
    }
    if ct.starts_with("text/html") || text.trim_start().starts_with('<') {
        return Err(refuse("not_media", "이 주소는 영상 파일이 아닌 웹 페이지입니다."));
    }
    // Refused here, before a job, rather than failing it once downloaded (R20).
    let ext = file_ext(&head).ok_or_else(|| refuse("not_audio", "이 주소는 오디오나 영상 파일이 아니라 소리를 꺼낼 수 없습니다."))?;
    let more: Vec<u8>;
    let silent = match ext {
        "ts" => crate::decode::ts_has_audio(&head) == Some(false),
        "webm" => {
            // `Tracks` follows the header, SeekHead and Info; a large one is read on, up to 1 MiB.
            let tracks = match webm_tracks(&head) {
                Some(t) => t,
                None if head.len() >= 65_536 => {
                    more = fetch(url.as_str(), Some((0, 1 << 20)), 1 << 20, allow_loopback)?.1;
                    webm_tracks(&more).ok_or_else(|| refuse("unverifiable", "영상의 트랙 정보를 찾지 못해 DRM 보호 여부를 확인할 수 없습니다."))?
                }
                // The whole file, and no tracks in it: nothing a decoder could read.
                None => return Err(no_sound()),
            };
            if webm_encrypted(tracks) {
                return Err(drm());
            }
            !webm_has_audio(tracks)
        }
        _ => false,
    };
    if silent {
        return Err(no_sound());
    }
    Ok(Source::File { url: url.to_string(), ext })
}

fn no_sound() -> Refusal {
    refuse("not_audio", "이 영상에는 소리 트랙이 없어 자막을 만들 수 없습니다.")
}

/// Whether a `moov` has a sound track: a handler (`hdlr`) of type `soun`, or an
/// audio sample entry.
fn mp4_has_sound(moov: &[u8]) -> bool {
    const ENTRIES: [&[u8; 4]; 7] = [b"mp4a", b"Opus", b"fLaC", b"alac", b"ac-3", b"ec-3", b".mp3"];
    // ponytail: a byte scan like `marked`; the handler type sits 8 bytes after the `hdlr` type.
    moov.windows(4).enumerate().any(|(i, w)| w == b"hdlr" && moov.get(i + 12..i + 16) == Some(b"soun") || ENTRIES.iter().any(|e| w == *e))
}

/// The body of the EBML element whose size (a variable-length integer) starts at `at`, if all of it is in `b`.
fn ebml_body(b: &[u8], at: usize) -> Option<&[u8]> {
    let first = *b.get(at)?;
    let len = first.leading_zeros() as usize + 1;
    if len > 8 {
        return None;
    }
    let mut size = u64::from(first) & (0xFF >> len);
    for x in b.get(at + 1..at + len)? {
        size = size << 8 | u64::from(*x);
    }
    let from = at + len;
    b.get(from..from.checked_add(usize::try_from(size).ok()?)?)
}

/// A WebM's `Tracks` body, when the whole element is in `head`.
fn webm_tracks(head: &[u8]) -> Option<&[u8]> {
    const TRACKS: [u8; 4] = [0x16, 0x54, 0xAE, 0x6B];
    // The element itself, not its ID listed in the SeekHead (SeekID 0x53AB, size 4).
    let at = (3..head.len().saturating_sub(4)).find(|&i| head[i..i + 4] == TRACKS && head[i - 3..i] != [0x53, 0xAB, 0x84])? + 4;
    ebml_body(head, at)
}

/// Whether `Tracks` has an audio track (`TrackType` 2).
fn webm_has_audio(tracks: &[u8]) -> bool {
    // ponytail: a byte scan for TrackType (0x83), size 1, value 2.
    tracks.windows(3).any(|w| w == [0x83, 0x81, 0x02])
}

/// Whether a track is encrypted (WebM's Common Encryption): a `ContentEncryption`
/// (0x5035) element under its `ContentEncodings`, holding `ContentEncAlgo`
/// (0x47E1) or `ContentEncKeyID` (0x47E2).
fn webm_encrypted(tracks: &[u8]) -> bool {
    // ponytail: a byte scan for the element, checked by its size and children, like `marked` for MP4.
    (0..tracks.len().saturating_sub(2)).any(|i| {
        tracks[i..i + 2] == [0x50, 0x35] && ebml_body(tracks, i + 2).is_some_and(|b| b.windows(2).any(|w| w == [0x47, 0xE1] || w == [0x47, 0xE2]))
    })
}

/// An HLS playlist lists its pieces; they must also answer, or the audio cannot
/// be downloaded and no job is made (R20). The first piece (the init section
/// when there is one), the first segment and the last are asked for one byte.
/// ponytail: segments in between are not asked; one that fails later fails the job as `download_failed`.
fn pieces_answer(src: &Source, allow_loopback: bool) -> Result<(), Refusal> {
    let Source::Hls { parts, .. } = src else { return Ok(()) };
    let mut asked: Vec<&Part> = Vec::new();
    for p in parts.iter().take(2).chain(parts.last()) {
        if asked.contains(&p) {
            continue;
        }
        asked.push(p);
        let at = p.range.map_or(0, |(off, _)| off);
        let got = fetch(&p.url, Some((at, 1)), 1, allow_loopback);
        match got {
            Ok((_, b, _)) if !b.is_empty() => {}
            Err(e) if e.code == "private_address" => return Err(e),
            other => {
                crate::log(format!("an hls piece does not answer: {:?}", other.err().map(|e| e.message)));
                return Err(refuse("segment_unavailable", "재생 목록의 오디오 조각을 받을 수 없습니다."));
            }
        }
    }
    Ok(())
}

/// The first section's audio, as with a file: an fMP4 init section must list a
/// sound track, and an MPEG-TS segment's PMT an audio stream, or the playlist
/// is refused as `not_audio` before a job is made (R20). ADTS and MP3 segments
/// are audio by what they are.
fn hls_has_sound(src: &Source, allow_loopback: bool) -> Result<(), Refusal> {
    let Source::Hls { parts, ext, sections } = src else { return Ok(()) };
    let Some(first) = sections.first() else { return Ok(()) };
    let silent = if let Some(init) = first.init.map(|i| &parts[i]) {
        let (_, body, _) = fetch(&init.url, init.range, MAX_PLAYLIST, allow_loopback)?;
        !mp4_has_sound(&body)
    } else if *ext == "ts" {
        let seg = &parts[first.part];
        let (off, len) = seg.range.map_or((0, 65_536), |(o, l)| (o, l.min(65_536)));
        let (_, head, _) = fetch(&seg.url, Some((off, len)), len as usize, allow_loopback)?;
        crate::decode::ts_has_audio(&head) == Some(false)
    } else {
        false
    };
    if silent {
        return Err(no_sound());
    }
    Ok(())
}

/// An MP4's `moov`, for its protection marks ([`marked`]) and its tracks
/// ([`mp4_has_sound`]); `None` when the file has none. The top-level boxes are
/// walked by size; a `moov` past the first read (a file that is not "fast
/// start") is fetched by range, or, from a server that ignores Range, read off
/// one plain GET ([`mp4_moov_streamed`]). A file whose `moov` cannot be
/// reached is refused, never taken as unprotected.
fn mp4_moov(url: &str, head: &[u8], allow_loopback: bool) -> Result<Option<Vec<u8>>, Refusal> {
    // Bytes `[off, off + n)`, from the head when it has them. `None`: the
    // server answered a range with the whole file. Past the end: what is there.
    let read = |off: u64, n: u64| -> Result<Option<Vec<u8>>, Refusal> {
        let end = (off + n) as usize;
        if end <= head.len() {
            return Ok(Some(head[off as usize..end].to_vec()));
        }
        match get(url, Some((off, n)), allow_loopback, 30_000) {
            Ok((r, _)) if r.status == 206 => Ok(Some(r.bytes(n as usize).map_err(unreachable)?)),
            Ok(_) => Ok(None),
            // A range starting at the end of the file: the walk is over.
            Err(e) if e.message.contains("HTTP 416") => Ok(Some(Vec::new())),
            Err(e) => Err(e),
        }
    };
    let mut off = 0u64;
    for _ in 0..64 {
        let Some(h) = read(off, 16)? else { return mp4_moov_streamed(url, allow_loopback) };
        let Some((size, hdr)) = box_header(&h) else { return Ok(None) };
        if &h[4..8] == b"moov" {
            // size 0: the box runs to the end of the file.
            let n = if size == 0 { MAX_PLAYLIST as u64 } else { size.min(MAX_PLAYLIST as u64) };
            let Some(body) = read(off, n)? else { return mp4_moov_streamed(url, allow_loopback) };
            return Ok(Some(body));
        }
        if size < hdr {
            return Ok(None);
        }
        off += size;
    }
    Ok(None)
}

/// What a server that ignores Range lets the walk do: one GET from the start,
/// box bodies skipped as they stream by. Up to 64 MiB may be skipped (the
/// command waits on this); past that the file is refused as unverifiable.
fn mp4_moov_streamed(url: &str, allow_loopback: bool) -> Result<Option<Vec<u8>>, Refusal> {
    const SKIP_CAP: u64 = 64 << 20;
    let (mut r, _) = get(url, None, allow_loopback, 30_000)?;
    let mut skipped = 0u64;
    loop {
        let mut h = Vec::with_capacity(16);
        (&mut r).take(8).read_to_end(&mut h).map_err(unreachable)?;
        if h.len() == 8 && h[..4] == [0, 0, 0, 1] {
            (&mut r).take(8).read_to_end(&mut h).map_err(unreachable)?;
        }
        let Some((size, hdr)) = box_header(&h) else { return Ok(None) }; // the end, and no moov
        if &h[4..8] == b"moov" {
            let n = if size == 0 { MAX_PLAYLIST as u64 } else { (size - hdr).min(MAX_PLAYLIST as u64) };
            let mut body = Vec::new();
            (&mut r).take(n).read_to_end(&mut body).map_err(unreachable)?;
            return Ok(Some(body));
        }
        if size == 0 || size < hdr {
            return Ok(None); // the last box, and not moov
        }
        skipped += size - hdr;
        if skipped > SKIP_CAP {
            return Err(refuse("unverifiable", "이 서버는 영상을 나눠 받지 못해 DRM 보호 여부를 확인할 수 없습니다."));
        }
        let gone = std::io::copy(&mut (&mut r).take(size - hdr), &mut std::io::sink()).map_err(unreachable)?;
        if gone < size - hdr {
            return Ok(None);
        }
    }
}

/// `(size, header length)` of the box at the start of `h`; size 0 runs to the end.
pub(crate) fn box_header(h: &[u8]) -> Option<(u64, u64)> {
    let size = u64::from(u32::from_be_bytes(h.get(..4)?.try_into().ok()?));
    h.get(4..8)?;
    if size == 1 {
        return Some((u64::from_be_bytes(h.get(8..16)?.try_into().ok()?), 16));
    }
    Some((size, 8))
}

/// Common Encryption's marks in a `moov`.
fn marked(moov: &[u8]) -> bool {
    const MARKS: [&[u8; 4]; 5] = [b"sinf", b"tenc", b"enca", b"encv", b"pssh"];
    // ponytail: a byte scan of `moov`, not a full box parse; a chunk offset that spells a mark refuses a clear file.
    moov.windows(4).any(|w| MARKS.iter().any(|m| w == *m))
}

/// The container a file is, by its first bytes, among those the decoder reads
/// (WAV, MP4/M4A, WebM/Matroska, MP3, ADTS AAC, MPEG-TS); `None` for anything
/// else (an image, a document, a page), which cannot be made into audio.
fn file_ext(head: &[u8]) -> Option<&'static str> {
    if head.len() >= 12 && head.starts_with(b"RIFF") && &head[8..12] == b"WAVE" {
        return Some("wav");
    }
    if head.len() > 8 && &head[4..8] == b"ftyp" {
        return Some("m4a");
    }
    if head.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        return Some("webm");
    }
    if head.starts_with(b"ID3") {
        return Some("mp3");
    }
    if head.len() > 1 && head[0] == 0xFF && head[1] & 0xE0 == 0xE0 {
        // ADTS has layer bits 00; MPEG audio layers I-III do not.
        return Some(if head[1] & 0x06 == 0 { "aac" } else { "mp3" });
    }
    if head.len() > 188 && head[0] == 0x47 && head[188] == 0x47 {
        return Some("ts");
    }
    None
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
        let variants: Vec<&[&str]> = lines.windows(2).filter(|w| w[0].starts_with("#EXT-X-STREAM-INF") && !w[1].starts_with('#')).collect();
        // A variant whose CODECS names no audio codec has no sound; one without CODECS may.
        const AUDIO: [&str; 6] = ["mp4a", "ac-3", "ec-3", "opus", "flac", "alac"];
        let sounds = |w: &&[&str]| attr(w[0], "CODECS").is_none_or(|c| AUDIO.iter().any(|a| c.to_ascii_lowercase().contains(a)));
        let pool: Vec<&[&str]> = if variants.iter().any(sounds) { variants.iter().copied().filter(sounds).collect() } else { variants };
        let variant = pool
            .iter()
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
    let mut pending: Option<&str> = None;
    // The init section in force (its part, once a segment has followed it), and one declared but not yet pushed.
    let (mut map, mut next_map): (Option<usize>, Option<Part>) = (None, None);
    let mut init = false;
    // The playlist's clock (the EXTINFs so far), and whether a new section starts at the next segment.
    let (mut clock, mut extinf, mut broken) = (0f64, 0f64, false);
    let mut first: Option<(usize, Option<usize>)> = None;
    let mut sections = Vec::new();
    for l in &lines {
        if let Some(d) = l.strip_prefix("#EXTINF:") {
            extinf = d.split(',').next().and_then(|s| s.trim().parse().ok()).filter(|s: &f64| s.is_finite() && *s >= 0.0).unwrap_or(0.0);
        } else if l.starts_with("#EXT-X-DISCONTINUITY") && !l.starts_with("#EXT-X-DISCONTINUITY-SEQUENCE") {
            broken = true;
        } else if l.starts_with("#EXT-X-MAP") {
            let url = join(attr(l, "URI").ok_or_else(|| refuse("not_media", "재생 목록의 초기화 조각에 주소가 없습니다."))?)?.to_string();
            let range = attr(l, "BYTERANGE").map(|r| byte_range(r, &url, &mut ends)).transpose()?;
            let part = Part { url, range };
            // The same init section again changes nothing; another one starts a section (a codec or track switch).
            if map.is_none_or(|m| parts[m] != part) {
                next_map = Some(part);
            } else {
                next_map = None;
            }
            init = true;
        } else if let Some(spec) = l.strip_prefix("#EXT-X-BYTERANGE:") {
            pending = Some(spec);
        } else if !l.starts_with('#') {
            let url = join(l)?.to_string();
            let range = pending.take().map(|r| byte_range(r, &url, &mut ends)).transpose()?;
            let switched = next_map.is_some() && first.is_some();
            if let Some(m) = next_map.take() {
                map = Some(parts.len());
                parts.push(m);
            }
            if (std::mem::take(&mut broken) || switched) && first.is_some() {
                if sections.is_empty() {
                    let (part, init) = first.unwrap();
                    sections.push(Section { part, at: 0.0, init });
                }
                sections.push(Section { part: parts.len(), at: clock, init: map });
            }
            first.get_or_insert((parts.len(), map));
            parts.push(Part { url, range });
            clock += std::mem::take(&mut extinf);
        }
    }
    let Some((first, first_init)) = first else {
        return Err(refuse("not_media", "재생 목록에 조각이 없습니다."));
    };
    // One section at least: players start an HLS timeline at its first segment, whatever
    // its timestamps say (fMP4 from 10 s plays from 0), and so does the decoder.
    if sections.is_empty() {
        sections.push(Section { part: first, at: 0.0, init: first_init });
    }
    let first = parts[first].url.to_ascii_lowercase();
    let ext = if init || first.contains(".m4s") || first.contains(".mp4") || first.contains(".m4a") {
        "mp4"
    } else if first.contains(".aac") {
        "aac"
    } else if first.contains(".mp3") {
        "mp3"
    } else {
        "ts"
    };
    Ok(Source::Hls { parts, ext, sections })
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
    // An index from an earlier attempt describes other bytes.
    let sidecar = sections_path(dest);
    match std::fs::remove_file(&sidecar) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(format!("stale sections index: {e}")),
        _ => {}
    }
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
        Source::Hls { parts, sections, .. } => {
            let io = |e: std::io::Error| e.to_string();
            // Where each part starts in the file, and where the last ends.
            let mut starts = vec![0u64];
            let mut written = 0u64;
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
                        written += n;
                    }
                    None => {
                        written += std::io::copy(&mut r, &mut out).map_err(io)?;
                    }
                }
                starts.push(written);
                progress(((i + 1) * 100 / parts.len()) as i64);
            }
            let mut index = String::new();
            for (i, s) in sections.iter().enumerate() {
                // A section ends where the next begins, or the next one's own init section before it.
                let end = sections.get(i + 1).map_or(parts.len(), |n| n.init.filter(|&m| m > s.part).unwrap_or(n.part));
                let (from, to) = s.init.map_or((0, 0), |m| (starts[m], starts[m + 1]));
                index.push_str(&format!("{} {} {} {from} {to}\n", starts[s.part], starts[end], s.at));
            }
            // Written after the audio, so a download cut short never leaves an index that outruns it.
            if !index.is_empty() {
                out.flush().map_err(|e| e.to_string())?;
                std::fs::write(&sidecar, index).map_err(|e| e.to_string())?;
            }
        }
    }
    out.flush().map_err(|e| e.to_string())
}

#[cfg(test)]
#[path = "media_probe_tests.rs"]
mod probe_tests;

#[cfg(test)]
#[path = "media_dns_tests.rs"]
mod dns_tests;

#[cfg(test)]
#[path = "media_norange_tests.rs"]
mod norange_tests;

#[cfg(test)]
#[path = "media_hls_pieces_tests.rs"]
mod hls_pieces_tests;

#[cfg(test)]
#[path = "media_not_audio_tests.rs"]
mod not_audio_tests;

#[cfg(test)]
#[path = "media_discontinuity_tests.rs"]
mod discontinuity_tests;

#[cfg(test)]
#[path = "media_tracks_tests.rs"]
mod tracks_tests;

#[cfg(test)]
#[path = "media_hls_start_tests.rs"]
mod hls_start_tests;

#[cfg(test)]
#[path = "media_webm_drm_tests.rs"]
mod webm_drm_tests;

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
            Source::Hls { parts: vec![part("https://cdn.example.org/v/seg0.aac"), part("https://cdn.example.org/x/seg1.aac")], ext: "aac", sections: vec![Section { part: 0, at: 0.0, init: None }] }
        );
    }

    #[test]
    fn byte_ranges_are_kept_and_continue_from_the_previous_one() {
        let vod = "#EXTM3U\n#EXT-X-MAP:URI=\"media.mp4\",BYTERANGE=\"720@0\"\n#EXTINF:4,\n#EXT-X-BYTERANGE:1000@720\nmedia.mp4\n\
                   #EXTINF:4,\n#EXT-X-BYTERANGE:500\nmedia.mp4\n#EXT-X-ENDLIST";
        let Source::Hls { parts, ext, .. } = probe_hls(&base(), vod, true, 0).unwrap() else { panic!("not HLS") };
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
