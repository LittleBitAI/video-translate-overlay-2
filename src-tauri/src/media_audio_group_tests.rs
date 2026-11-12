//! A master playlist's audio is the one a player plays with the chosen
//! variant: its AUDIO group's `DEFAULT=YES` rendition (else `AUTOSELECT=YES`,
//! else the first), not the first audio listed, which may be a commentary
//! track (R1, R17). A variant with no group, or a rendition with no URI of its
//! own, carries its audio itself.
//!
//! The renditions are `tests/fixtures/hls_maps`: "commentary" is `init-a.mp4` +
//! `a0.m4s`, "lecture" is `init-b.mp4` + `b0.m4s`.

use super::tracks_tests::serve_dir;
use super::*;

const COMMENTARY: &str = "#EXTM3U\n#EXT-X-MAP:URI=\"init-a.mp4\"\n#EXTINF:4.0,\na0.m4s\n#EXT-X-ENDLIST\n";
const LECTURE: &str = "#EXTM3U\n#EXT-X-MAP:URI=\"init-b.mp4\"\n#EXTINF:4.0,\nb0.m4s\n#EXT-X-ENDLIST\n";

/// The pieces `master` leads to, served beside the two renditions.
fn pieces(master: &str) -> Vec<String> {
    let base = serve_dir(
        "hls_maps",
        vec![
            ("/master.m3u8".into(), master.as_bytes().to_vec()),
            ("/commentary.m3u8".into(), COMMENTARY.as_bytes().to_vec()),
            ("/lecture.m3u8".into(), LECTURE.as_bytes().to_vec()),
        ],
    );
    match probe(&Url::parse(&format!("{base}/master.m3u8")).unwrap(), true).unwrap() {
        Source::Hls { parts, .. } => parts.iter().map(|p| p.url.rsplit('/').next().unwrap().to_string()).collect(),
        _ => panic!("not HLS"),
    }
}

#[test]
fn the_default_rendition_is_taken_not_the_first() {
    let master = "#EXTM3U\n\
        #EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",NAME=\"Commentary\",DEFAULT=NO,AUTOSELECT=NO,URI=\"commentary.m3u8\"\n\
        #EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",NAME=\"Lecture\",DEFAULT=YES,AUTOSELECT=YES,URI=\"lecture.m3u8\"\n\
        #EXT-X-STREAM-INF:BANDWIDTH=800000,CODECS=\"avc1.42e01e,mp4a.40.2\",AUDIO=\"a\"\n\
        video.m3u8\n";
    assert_eq!(pieces(master), ["init-b.mp4", "b0.m4s"]);
}

#[test]
fn the_group_of_the_chosen_variant_is_the_one_used() {
    // The lowest-bandwidth variant plays group "lo", whose default is the lecture;
    // group "hi" lists the commentary first and as its default.
    let master = "#EXTM3U\n\
        #EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"hi\",NAME=\"Commentary\",DEFAULT=YES,URI=\"commentary.m3u8\"\n\
        #EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"lo\",NAME=\"Commentary\",DEFAULT=NO,URI=\"commentary.m3u8\"\n\
        #EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"lo\",NAME=\"Lecture\",DEFAULT=YES,URI=\"lecture.m3u8\"\n\
        #EXT-X-STREAM-INF:BANDWIDTH=5000000,CODECS=\"avc1.640028\",AUDIO=\"hi\"\n\
        hi.m3u8\n\
        #EXT-X-STREAM-INF:BANDWIDTH=500000,CODECS=\"avc1.42e01e\",AUDIO=\"lo\"\n\
        lo.m3u8\n";
    assert_eq!(pieces(master), ["init-b.mp4", "b0.m4s"]);
}

#[test]
fn a_variant_with_no_audio_group_plays_its_own_audio() {
    // The EXT-X-MEDIA belongs to no variant here.
    let master = "#EXTM3U\n\
        #EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"x\",NAME=\"Commentary\",DEFAULT=YES,URI=\"commentary.m3u8\"\n\
        #EXT-X-STREAM-INF:BANDWIDTH=800000,CODECS=\"mp4a.40.2\"\n\
        lecture.m3u8\n";
    assert_eq!(pieces(master), ["init-b.mp4", "b0.m4s"]);
}

#[test]
fn renditions_are_picked_as_players_pick_them() {
    let lines = |m: &'static str| m.lines().collect::<Vec<_>>();
    let pick = |m: &'static str| group_audio(&lines(m), "a");
    // No DEFAULT: the AUTOSELECT one.
    assert_eq!(
        pick("#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",NAME=\"C\",URI=\"c.m3u8\"\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",NAME=\"L\",AUTOSELECT=YES,URI=\"l.m3u8\""),
        Some("l.m3u8")
    );
    // Neither: the first.
    assert_eq!(pick("#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",NAME=\"L\",URI=\"l.m3u8\"\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",NAME=\"C\",URI=\"c.m3u8\""), Some("l.m3u8"));
    // The default has no URI: its audio is in the variant.
    assert_eq!(pick("#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",NAME=\"C\",URI=\"c.m3u8\"\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",NAME=\"L\",DEFAULT=YES"), None);
    // Subtitles and other groups are not audio of this group.
    assert_eq!(pick("#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID=\"a\",NAME=\"S\",DEFAULT=YES,URI=\"s.m3u8\"\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"b\",NAME=\"C\",DEFAULT=YES,URI=\"c.m3u8\""), None);
}
