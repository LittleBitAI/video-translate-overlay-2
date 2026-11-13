//! Decoding a download never holds the file: an MPEG-TS stream whose video
//! dwarfs its audio (an HLS variant with video, hours long) is read from disk
//! as it is decoded, whole or as an HLS section. Its own test binary, so its
//! counting allocator sees only this.

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

struct Counting;
static NOW: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            PEAK.fetch_max(NOW.fetch_add(l.size(), Relaxed) + l.size(), Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) };
        NOW.fetch_sub(l.size(), Relaxed);
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

/// The most allocated at once while `f` runs, beyond what was allocated before it.
fn peak_of<T>(f: impl FnOnce() -> T) -> (T, usize) {
    let base = NOW.load(Relaxed);
    PEAK.store(base, Relaxed);
    let out = f();
    (out, PEAK.load(Relaxed) - base)
}

/// `tone.ts` (a 440 Hz tone) with `filler` null packets after each of its packets.
fn padded(filler: usize) -> PathBuf {
    let tone = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/audio/tone.ts")).unwrap();
    let path = std::env::temp_dir().join(format!("vto-memory-{}.ts", std::process::id()));
    let mut null = [0xFFu8; 188];
    null[..4].copy_from_slice(&[0x47, 0x1F, 0xFF, 0x10]);
    let mut out = std::io::BufWriter::new(std::fs::File::create(&path).unwrap());
    for p in tone.chunks_exact(188) {
        out.write_all(p).unwrap();
        for _ in 0..filler {
            out.write_all(&null).unwrap();
        }
    }
    out.flush().unwrap();
    path
}

#[test]
fn a_long_ts_download_is_decoded_without_holding_it() {
    const LIMIT: usize = 24 << 20;
    let tone = vto::audio::decode_file(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/audio/tone.ts")).unwrap();
    let path = padded(4_600);
    let size = std::fs::metadata(&path).unwrap().len();
    assert!(size > 90 << 20, "{size} bytes");

    let (pcm, peak) = peak_of(|| vto::audio::decode_file(&path).unwrap());
    assert_eq!(pcm, tone, "the filler changed the audio");
    assert!(peak < LIMIT, "decoding a {} MB file held {} MB at once", size >> 20, peak >> 20);

    // The same bytes as one HLS section.
    let index = vto::media::sections_path(&path);
    std::fs::write(&index, format!("0 {size} 0 0 0\n")).unwrap();
    let (pcm, peak) = peak_of(|| vto::audio::decode_file(&path).unwrap());
    assert_eq!(pcm.len(), tone.len());
    assert!(peak < LIMIT, "decoding a {} MB section held {} MB at once", size >> 20, peak >> 20);

    let _ = std::fs::remove_file(&index);
    let _ = std::fs::remove_file(&path);
}
