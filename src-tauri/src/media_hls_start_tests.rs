//! An HLS timeline starts at its first segment, whatever the segment's own
//! timestamps say (R4), and a playlist with no sound track is refused before a
//! job, as a file is (R20, P6's 422). Fixtures: see `media_tracks_tests.rs`;
//! `tracks/video-init.mp4` + `video0.m4s` are a 2 s video-only fMP4 rendition.

use super::tracks_tests::{download_and_decode, fixtures, rms, serve_dir};
use super::*;

fn served() -> String {
    let maps = fixtures("hls_maps");
    let tracks = fixtures("tracks");
    let file = |dir: &Path, n: &str| (format!("/{n}"), std::fs::read(dir.join(n)).unwrap());
    let text = |n: &str, body: &str| (format!("/{n}"), body.as_bytes().to_vec());
    let head = "#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-TARGETDURATION:4\n#EXT-X-PLAYLIST-TYPE:VOD\n";
    serve_dir(
        "tracks",
        vec![
            file(&maps, "init-a.mp4"),
            file(&maps, "a0.m4s"),
            file(&maps, "init-b.mp4"),
            file(&maps, "b0.m4s"),
            text("late.m3u8", &format!("{head}#EXT-X-MAP:URI=\"init-b.mp4\"\n#EXTINF:4.0,\nb0.m4s\n#EXT-X-ENDLIST\n")),
            text("ts-video.m3u8", &format!("{head}#EXTINF:1.0,\nvideo.ts\n#EXT-X-ENDLIST\n")),
            text("fmp4-video.m3u8", &format!("{head}#EXT-X-MAP:URI=\"video-init.mp4\"\n#EXTINF:2.0,\nvideo0.m4s\n#EXT-X-ENDLIST\n")),
            text("a.m3u8", &format!("{head}#EXT-X-MAP:URI=\"init-a.mp4\"\n#EXTINF:4.0,\na0.m4s\n#EXT-X-ENDLIST\n")),
            text(
                "master.m3u8",
                "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=100,CODECS=\"mp4v.20.9\"\nfmp4-video.m3u8\n\
                 #EXT-X-STREAM-INF:BANDWIDTH=200,CODECS=\"mp4v.20.9,mp4a.40.2\"\na.m3u8\n",
            ),
        ]
        .into_iter()
        .chain(std::iter::once(file(&tracks, "video.ts")))
        .collect(),
    )
}

#[test]
fn an_fmp4_rendition_stamped_from_ten_seconds_plays_from_zero() {
    let base = served();
    let src = probe(&Url::parse(&format!("{base}/late.m3u8")).unwrap(), true).unwrap();
    let pcm = download_and_decode(&src, "late").unwrap();
    let secs = pcm.len() as f64 / 16_000.0;
    assert!((secs - 4.0).abs() < 0.15, "{secs:.2} s long, not the playlist's 4 s");
    assert!(rms(&pcm, 0.2, 0.8) < 50.0, "the first second is not silent");
    let onset = pcm.iter().position(|s| s.unsigned_abs() > 3_000).unwrap() as f64 / 16_000.0;
    assert!((onset - 1.0).abs() < 0.1, "the tone starts at {onset:.2} s, not 1 s");
}

#[test]
fn a_playlist_with_no_sound_track_is_refused_before_a_job() {
    let base = served();
    let probe_at = |path: &str| probe(&Url::parse(&format!("{base}{path}")).unwrap(), true);
    assert_eq!(probe_at("/ts-video.m3u8").unwrap_err().code, "not_audio", "MPEG-TS with video only");
    assert_eq!(probe_at("/fmp4-video.m3u8").unwrap_err().code, "not_audio", "fMP4 with video only");
    // A master playlist passes over a cheaper variant whose CODECS names no audio.
    let Source::Hls { parts, .. } = probe_at("/master.m3u8").unwrap() else { panic!("not HLS") };
    assert!(parts.iter().any(|p| p.url.ends_with("/a0.m4s")), "{parts:?}");
}
