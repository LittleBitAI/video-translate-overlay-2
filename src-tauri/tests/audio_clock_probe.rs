//! Opt-in real compressed-audio clock check; the generated markers are at
//! 1, 300 and 899 seconds, not concatenated speech-relative positions.
use std::path::PathBuf;

#[test]
#[ignore = "needs VTO_CLOCK_AUDIO, a 902-second AAC recording with tones at 1, 300 and 899 s"]
fn compressed_audio_keeps_late_markers_on_the_source_clock() {
    let path = PathBuf::from(std::env::var_os("VTO_CLOCK_AUDIO").expect("VTO_CLOCK_AUDIO"));
    let pcm = vto::audio::decode_file(&path).unwrap();
    let rate = vto::audio::RATE;
    let seconds = pcm.len() as f64 / rate as f64;
    println!("decoded duration: {seconds:.6} s");
    assert!((seconds - 902.0).abs() < 0.1);
    for expected in [1usize, 300, 899] {
        let from = (expected - 1) * rate;
        let until = (expected + 2) * rate;
        let offset = pcm[from..until].iter().position(|s| s.unsigned_abs() > 3_000).expect("marker");
        let actual = (from + offset) as f64 / rate as f64;
        println!("marker {expected}: {actual:.6} s");
        assert!((actual - expected as f64).abs() < 0.05);
    }
}
