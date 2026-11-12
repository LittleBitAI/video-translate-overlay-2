//! An HLS playlist under `EXT-X-KEY:METHOD=AES-128` with an identity key (no
//! DRM; the key served beside the segments) is taken (R17, design A3): each
//! piece is decrypted as it is downloaded, from the tag's IV or its media
//! sequence number, and decodes to what the same playlist in the clear does.
//! A key that cannot be had is refused before a job is made (R20).
//!
//! The pieces are `tests/fixtures/hls_disc` (MPEG-TS) and `tests/fixtures/hls_maps` (fMP4).

use super::tracks_tests::{download_and_decode, fixtures, serve_dir};
use super::*;

const KEY: [u8; 16] = *b"0123456789abcdef";

fn read(dir: &str, name: &str) -> Vec<u8> {
    std::fs::read(fixtures(dir).join(name)).unwrap()
}

/// `plain` under AES-128-CBC with PKCS#7 padding, as an HLS packager writes it.
fn encrypt(plain: &[u8], key: &[u8; 16], iv: [u8; 16]) -> Vec<u8> {
    use aes::cipher::{generic_array::GenericArray, BlockEncrypt, KeyInit};
    let c = aes::Aes128::new(GenericArray::from_slice(key));
    let pad = 16 - plain.len() % 16;
    let mut data = plain.to_vec();
    data.extend(std::iter::repeat_n(pad as u8, pad));
    let mut prev = iv;
    for b in data.chunks_exact_mut(16) {
        for i in 0..16 {
            b[i] ^= prev[i];
        }
        c.encrypt_block(GenericArray::from_mut_slice(b));
        prev.copy_from_slice(b);
    }
    data
}

fn seq_iv(n: u128) -> [u8; 16] {
    n.to_be_bytes()
}

fn probe_at(base: &str, path: &str) -> Result<Source, Refusal> {
    probe(&Url::parse(&format!("{base}{path}")).unwrap(), true)
}

/// The bytes `download` writes for `src`.
fn downloaded(src: &Source, name: &str) -> Vec<u8> {
    let dest = std::env::temp_dir().join(format!("vto-aes-{}-{name}.ts", std::process::id()));
    download(src, &dest, true, &mut |_| {}).unwrap();
    let b = std::fs::read(&dest).unwrap();
    let _ = std::fs::remove_file(sections_path(&dest));
    let _ = std::fs::remove_file(&dest);
    b
}

fn plain(dir: &str) -> Vec<i16> {
    let base = serve_dir(dir, vec![]);
    download_and_decode(&probe_at(&base, "/index.m3u8").unwrap(), &format!("{dir}-plain")).unwrap()
}

#[test]
fn ts_segments_decrypt_with_their_media_sequence_numbers() {
    let playlist = std::fs::read_to_string(fixtures("hls_disc").join("index.m3u8"))
        .unwrap()
        .replace("#EXT-X-PLAYLIST-TYPE:VOD\n", "#EXT-X-PLAYLIST-TYPE:VOD\n#EXT-X-MEDIA-SEQUENCE:41\n#EXT-X-KEY:METHOD=AES-128,URI=\"k.bin\"\n")
        .replace("seg", "enc");
    let base = serve_dir(
        "hls_disc",
        vec![
            ("/k.bin".into(), KEY.to_vec()),
            ("/enc0.ts".into(), encrypt(&read("hls_disc", "seg0.ts"), &KEY, seq_iv(41))),
            ("/enc1.ts".into(), encrypt(&read("hls_disc", "seg1.ts"), &KEY, seq_iv(42))),
            ("/enc.m3u8".into(), playlist.into_bytes()),
        ],
    );
    let src = probe_at(&base, "/enc.m3u8").expect("an AES-128 playlist was refused");
    // Byte for byte the plain segments (a wrong IV spoils only the first block, which a decoder may get past).
    assert_eq!(downloaded(&src, "aes-ts"), [read("hls_disc", "seg0.ts"), read("hls_disc", "seg1.ts")].concat());
    assert_eq!(download_and_decode(&src, "aes-ts").unwrap(), plain("hls_disc"));
}

#[test]
fn an_fmp4_playlist_decrypts_its_init_sections_and_segments_with_the_tags_iv() {
    let iv = seq_iv(0x00ff_1234_5678_9abc_def0_0011_2233_4455);
    let playlist = std::fs::read_to_string(fixtures("hls_maps").join("index.m3u8"))
        .unwrap()
        .replace("#EXT-X-PLAYLIST-TYPE:VOD\n", "#EXT-X-PLAYLIST-TYPE:VOD\n#EXT-X-KEY:METHOD=AES-128,URI=\"k.bin\",IV=0x00FF123456789ABCDEF0001122334455\n")
        .replace("init-", "enc-")
        .replace("0.m4s", "0e.m4s");
    let mut extra: Vec<(String, Vec<u8>)> = vec![("/k.bin".into(), KEY.to_vec()), ("/enc.m3u8".into(), playlist.into_bytes())];
    for (from, to) in [("init-a.mp4", "enc-a.mp4"), ("init-b.mp4", "enc-b.mp4"), ("a0.m4s", "a0e.m4s"), ("b0.m4s", "b0e.m4s")] {
        extra.push((format!("/{to}"), encrypt(&read("hls_maps", from), &KEY, iv)));
    }
    let base = serve_dir("hls_maps", extra);
    let src = probe_at(&base, "/enc.m3u8").expect("an AES-128 playlist was refused");
    let clear: Vec<u8> = ["init-a.mp4", "a0.m4s", "init-b.mp4", "b0.m4s"].iter().flat_map(|n| read("hls_maps", n)).collect();
    assert_eq!(downloaded(&src, "aes-mp4"), clear);
    assert_eq!(download_and_decode(&src, "aes-mp4").unwrap(), plain("hls_maps"));
}

#[test]
fn encrypted_byte_ranges_and_a_key_that_ends_decrypt_piece_by_piece() {
    // Both segments in one file by byte range; the first encrypted, the second
    // after `METHOD=NONE`, in the clear.
    let (seg0, seg1) = (encrypt(&read("hls_disc", "seg0.ts"), &KEY, seq_iv(0)), read("hls_disc", "seg1.ts"));
    let playlist = format!(
        "#EXTM3U\n#EXT-X-VERSION:4\n#EXT-X-PLAYLIST-TYPE:VOD\n#EXT-X-KEY:METHOD=AES-128,URI=\"k.bin\"\n#EXTINF:4.0,\n#EXT-X-BYTERANGE:{}@0\nall.ts\n\
         #EXT-X-DISCONTINUITY\n#EXT-X-KEY:METHOD=NONE\n#EXTINF:4.0,\n#EXT-X-BYTERANGE:{}\nall.ts\n#EXT-X-ENDLIST\n",
        seg0.len(),
        seg1.len()
    );
    let base = serve_dir("hls_disc", vec![("/k.bin".into(), KEY.to_vec()), ("/all.ts".into(), [seg0, seg1].concat()), ("/r.m3u8".into(), playlist.into_bytes())]);
    let src = probe_at(&base, "/r.m3u8").expect("an AES-128 playlist was refused");
    let Source::Hls { parts, .. } = &src else { panic!("not HLS") };
    assert!(parts[0].aes.is_some() && parts[1].aes.is_none(), "{parts:?}");
    assert_eq!(downloaded(&src, "aes-ranges"), [read("hls_disc", "seg0.ts"), read("hls_disc", "seg1.ts")].concat());
    assert_eq!(download_and_decode(&src, "aes-ranges").unwrap(), plain("hls_disc"));
}

#[test]
fn a_key_that_cannot_be_had_or_does_not_fit_is_refused_before_a_job() {
    let one = "#EXTM3U\n#EXT-X-PLAYLIST-TYPE:VOD\n#EXT-X-KEY:METHOD=AES-128,URI=\"k.bin\"\n#EXT-X-MAP:URI=\"enc-a.mp4\"\n#EXTINF:4.0,\na0e.m4s\n#EXT-X-ENDLIST\n";
    let pieces = |key: &[u8; 16]| -> Vec<(String, Vec<u8>)> {
        vec![
            ("/one.m3u8".into(), one.into()),
            ("/enc-a.mp4".into(), encrypt(&read("hls_maps", "init-a.mp4"), key, seq_iv(0))),
            ("/a0e.m4s".into(), encrypt(&read("hls_maps", "a0.m4s"), key, seq_iv(0))),
        ]
    };
    // No key at its URL.
    let base = serve_dir("hls_maps", pieces(&KEY));
    assert_eq!(probe_at(&base, "/one.m3u8").unwrap_err().code, "key_unavailable");
    // A key that is not 16 bytes.
    let mut files = pieces(&KEY);
    files.push(("/k.bin".into(), b"not a key".to_vec()));
    assert_eq!(probe_at(&serve_dir("hls_maps", files), "/one.m3u8").unwrap_err().code, "key_unavailable");
    // A key that is not the one the pieces were encrypted with.
    let mut files = pieces(b"fedcba9876543210");
    files.push(("/k.bin".into(), KEY.to_vec()));
    assert_eq!(probe_at(&serve_dir("hls_maps", files), "/one.m3u8").unwrap_err().code, "bad_key");
    // And the right one is taken.
    let mut files = pieces(&KEY);
    files.push(("/k.bin".into(), KEY.to_vec()));
    assert!(probe_at(&serve_dir("hls_maps", files), "/one.m3u8").is_ok());
}

#[test]
fn a_key_on_the_private_network_is_refused_as_any_url_there_is() {
    let one = "#EXTM3U\n#EXT-X-PLAYLIST-TYPE:VOD\n#EXT-X-KEY:METHOD=AES-128,URI=\"http://10.0.0.1/k.bin\"\n#EXTINF:4,\nhttps://cdn.example.org/a.ts\n#EXT-X-ENDLIST";
    let base = Url::parse("https://cdn.example.org/v/index.m3u8").unwrap();
    assert_eq!(probe_hls(&base, one, false, 0).unwrap_err().code, "private_address");
}
