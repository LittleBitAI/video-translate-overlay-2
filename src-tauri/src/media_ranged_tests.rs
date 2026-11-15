//! A direct file comes down in ranges written in order, each checked against
//! the first piece's resource; a dropped piece resumes where it stopped, and a
//! server that ignores Range, or names no revision, still gives the whole file
//! in one response.

use super::*;
use std::io::{BufRead, BufReader};
use std::sync::{Arc, Mutex};

/// What the test server does with the n-th request and its `Range`.
enum Answer {
    /// 206 for the asked range with these validator header lines; `cut` sends only that many body bytes, then closes.
    Range { validators: &'static str, cut: Option<usize> },
    /// 200 with the whole file.
    Whole,
    /// 206 naming bytes that start one later than asked.
    Shifted,
}

/// A strong validator naming one revision.
const V1: &str = "ETag: \"v1\"\r\n";
const MODIFIED: &str = "Last-Modified: Fri, 13 Sep 2024 08:50:31 GMT\r\n";

type Seen = Arc<Mutex<Vec<Option<String>>>>;

/// Serves `file`; `plan(n)` decides the n-th answer. Returns the URL and the `Range` headers seen.
fn serve(file: Vec<u8>, plan: impl Fn(usize) -> Answer + Send + 'static) -> (String, Seen) {
    serve_revisions(vec![file], plan)
}

/// Like [`serve`], but the n-th request is answered from `revisions[n]` (the last one from then on).
fn serve_revisions(revisions: Vec<Vec<u8>>, plan: impl Fn(usize) -> Answer + Send + 'static) -> (String, Seen) {
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = server.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        for (n, s) in server.incoming().enumerate() {
            let Ok(s) = s else { continue };
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut range = None;
            loop {
                let mut l = String::new();
                if r.read_line(&mut l).unwrap_or(0) == 0 || l.trim().is_empty() {
                    break;
                }
                if let Some(v) = l.strip_prefix("Range: bytes=") {
                    range = Some(v.trim().to_string());
                }
            }
            log.lock().unwrap().push(range.clone());
            let file = &revisions[n.min(revisions.len() - 1)];
            let (a, b) = range
                .as_deref()
                .and_then(|r| r.split_once('-'))
                .map(|(a, b)| (a.parse::<usize>().unwrap(), b.parse::<usize>().unwrap().min(file.len() - 1)))
                .unwrap_or((0, file.len() - 1));
            let head = |status: &str, from: usize, len: usize, extra: String| {
                format!("HTTP/1.1 {status}\r\nContent-Type: audio/mp4\r\nContent-Length: {len}\r\n{extra}Connection: close\r\n\r\n")
                    .into_bytes()
                    .into_iter()
                    .chain(file[from..from + len].iter().copied())
                    .collect::<Vec<u8>>()
            };
            let total = file.len();
            let bytes = match plan(n) {
                Answer::Whole => head("200 OK", 0, total, String::new()),
                // A request without Range gets the whole file, as from any server.
                Answer::Range { validators, .. } if range.is_none() => head("200 OK", 0, total, validators.to_string()),
                Answer::Range { validators, cut } => {
                    let mut all = head("206 Partial Content", a, b - a + 1, format!("Content-Range: bytes {a}-{b}/{total}\r\n{validators}"));
                    if let Some(c) = cut {
                        all.truncate(all.len() - (b - a + 1) + c);
                    }
                    all
                }
                Answer::Shifted => head("206 Partial Content", a + 1, b - a, format!("Content-Range: bytes {}-{b}/{total}\r\n{V1}", a + 1)),
            };
            let _ = (&s).write_all(&bytes);
        }
    });
    (format!("http://127.0.0.1:{port}/a.m4a"), seen)
}

fn file(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i * 7 % 251) as u8).collect()
}

/// The file through [`download_file`] with `chunk`-byte pieces, as `download` fetches one.
fn fetch(url: &str, chunk: u64) -> Result<Vec<u8>, String> {
    let get = |u: &str, range| get(u, range, true, 10_000).map(|(r, _)| r).map_err(|r| format!("{}: {}", r.code, r.message));
    let mut out = Vec::new();
    let mut last = 0;
    download_file(url, chunk, &mut out, &get, &mut |p| last = p)?;
    assert_eq!(last, 100);
    Ok(out)
}

#[test]
fn ranges_are_fetched_in_order_and_joined_exactly() {
    let want = file(25_000);
    let (url, seen) = serve(want.clone(), |_| Answer::Range { validators: V1, cut: None });
    assert_eq!(fetch(&url, 10_000).unwrap(), want);
    let ranges: Vec<_> = seen.lock().unwrap().iter().map(|r| r.clone().unwrap()).collect();
    assert_eq!(ranges, ["0-9999", "10000-19999", "20000-29999"]);
}

#[test]
fn a_server_that_ignores_range_gives_the_whole_file_in_one_answer() {
    let want = file(25_000);
    let (url, seen) = serve(want.clone(), |_| Answer::Whole);
    assert_eq!(fetch(&url, 10_000).unwrap(), want);
    assert_eq!(seen.lock().unwrap().len(), 1);
}

#[test]
fn a_dropped_piece_resumes_from_the_byte_it_reached() {
    let want = file(25_000);
    let (url, seen) = serve(want.clone(), |n| Answer::Range { validators: V1, cut: (n == 1).then_some(4_000) });
    assert_eq!(fetch(&url, 10_000).unwrap(), want);
    let ranges: Vec<_> = seen.lock().unwrap().iter().map(|r| r.clone().unwrap()).collect();
    assert_eq!(ranges, ["0-9999", "10000-19999", "14000-23999", "24000-33999"]);
}

/// Ranges are joined only under validators that name one revision: a strong
/// `ETag` or a `Last-Modified`. Otherwise (no validator, or only a weak
/// `ETag`) the file comes from one whole-file response, never a mix of two
/// equal-length revisions.
#[test]
fn pieces_are_joined_only_under_a_validator_that_names_one_revision() {
    let (old, new) = (file(25_000), file(25_000).iter().map(|b| b ^ 0xff).collect::<Vec<u8>>());
    let weak_and_modified = "ETag: W/\"v1\"\r\nLast-Modified: Fri, 13 Sep 2024 08:50:31 GMT\r\n";
    for (validators, ranged) in [(V1, true), (MODIFIED, true), (weak_and_modified, true), ("ETag: W/\"v1\"\r\n", false), ("", false)] {
        // The first request is answered from one revision, every later one from another.
        let (url, seen) = serve_revisions(vec![old.clone(), new.clone()], move |_| Answer::Range { validators, cut: None });
        let got = fetch(&url, 10_000).unwrap();
        let seen = seen.lock().unwrap().clone();
        if ranged {
            // This server claims one revision throughout, so the ranged path is taken.
            assert_eq!(seen.len(), 3, "{validators:?}");
            assert!(seen.iter().all(Option::is_some), "{validators:?}");
        } else {
            assert_eq!(got, new, "{validators:?}: pieces of two revisions were joined");
            assert_eq!(seen, [Some("0-9999".to_string()), None], "{validators:?}");
        }
    }
}

#[test]
fn pieces_that_do_not_continue_the_same_file_fail_the_download() {
    let later_modified = "Last-Modified: Sat, 14 Sep 2024 08:50:31 GMT\r\n";
    for (first, later) in [(V1, "ETag: \"v2\"\r\n"), (MODIFIED, later_modified)] {
        let changed = serve(file(25_000), move |n| Answer::Range { validators: if n == 0 { first } else { later }, cut: None }).0;
        assert!(fetch(&changed, 10_000).unwrap_err().contains("changed"), "{later:?}");
    }
    let shifted = serve(file(25_000), |n| if n == 0 { Answer::Range { validators: V1, cut: None } } else { Answer::Shifted }).0;
    assert!(fetch(&shifted, 10_000).unwrap_err().contains("asked for bytes from 10000"));
    let stopped = serve(file(25_000), |n| if n == 0 { Answer::Range { validators: V1, cut: None } } else { Answer::Whole }).0;
    assert!(fetch(&stopped, 10_000).unwrap_err().contains("stopped answering ranges"));
}

#[test]
fn content_range_is_read_strictly() {
    assert_eq!(content_range("bytes 0-9/10"), Some((0, 9, 10)));
    for bad in ["bytes 0-9/*", "bytes */10", "bytes 5-4/10", "bytes 0-10/10", "items 0-9/10", "bytes x-9/10"] {
        assert_eq!(content_range(bad), None, "{bad}");
    }
}
