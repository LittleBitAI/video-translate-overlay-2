//! Chunked bodies off Windows: streamed, and a malformed one is an error rather
//! than a panic, an allocation of the server's choosing, or a silent end.

use super::portable::Chunked;
use std::io::{Cursor, Read};

fn body(raw: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut buf = [0u8; 3]; // smaller than the chunks, so one chunk takes several reads
    let mut c = Chunked { r: Cursor::new(raw.to_vec()), left: 0, done: false };
    loop {
        match c.read(&mut buf)? {
            0 => return Ok(out),
            n => out.extend_from_slice(&buf[..n]),
        }
    }
}

#[test]
fn chunks_stream_and_trailers_are_skipped() {
    let raw = b"5;ext=1\r\nhello\r\n7\r\n, world\r\n0\r\nX-Trailer: 1\r\n\r\nNEXT";
    assert_eq!(body(raw).unwrap(), b"hello, world");
}

#[test]
fn a_huge_chunk_size_is_an_error_not_a_panic_or_an_allocation() {
    let e = body(b"8000000000000000\r\nRIFF....WAVE").unwrap_err();
    assert_eq!(e.kind(), std::io::ErrorKind::UnexpectedEof, "{e}");
    let e = body(b"fffffffffffffffff\r\nx").unwrap_err();
    assert_eq!(e.kind(), std::io::ErrorKind::InvalidData, "{e}");
}

#[test]
fn malformed_framing_is_an_error_not_the_end() {
    for raw in [&b"zz\r\nabc"[..], b"3\r\nabcX\r\n0\r\n\r\n", b"3\r\nabc\r\n", b"3", &[b'1'; 2000]] {
        assert!(body(raw).is_err(), "{:?}", String::from_utf8_lossy(raw));
    }
}
