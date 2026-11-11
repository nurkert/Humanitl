//! Wie viel Speicher der Entpacker auf einmal anfordert (HUM-425).
//!
//! Das Budget aus `decode.rs` begrenzt die Ausgabe, nicht die Puffer der
//! Entpacker. Ein Brotli-Strom mit Large-Window-Kennung ließ
//! `brotli::Decompressor` aus fünf Bytes Eingabe einen Ringpuffer von 512 MiB
//! anfordern; der nächtliche Fuzzer brach daran mit `-rss_limit_mb=512` ab.
//! Weil der Puffer nullinitialisiert angefordert wird, wächst der residente
//! Speicher dabei kaum. Gemessen wird deshalb die größte einzelne Anforderung
//! über einen zählenden Allokator, je Thread, damit parallel laufende Tests
//! sich nicht stören.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::io::Write as _;
use std::path::Path;

use humanitl_findings::decode::{ContentEncoding, decode_body};

/// Der Ringpuffer des größten Fensters aus RFC 7932 (16 MiB) samt Spielraum
/// für die Ausgabe, die bis zum Cap von 8 MiB wächst.
const LARGEST_ALLOWED: usize = 32 * 1024 * 1024;

/// Das Cap und das Verhältnis wie im Fuzz-Target `body_decoder`.
const CAP: usize = 8 * 1024 * 1024;
const RATIO: u32 = 100;

thread_local! {
    static LARGEST: Cell<usize> = const { Cell::new(0) };
}

/// Leitet an den System-Allokator weiter und merkt sich je Thread die größte
/// Anforderung.
struct Counting;

fn note(size: usize) {
    // `try_with`: Beim Abbau des Threads ist der Wert schon weg; dann wird
    // nichts mehr gemessen.
    let _ = LARGEST.try_with(|largest| largest.set(largest.get().max(size)));
}

// SAFETY: Jede Methode reicht Zeiger und Layout unverändert an `System`
// weiter; `note` fordert selbst keinen Speicher an.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        // SAFETY: Vertrag von `GlobalAlloc::alloc`, unverändert weitergereicht.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        // SAFETY: wie oben.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note(new_size);
        // SAFETY: wie oben.
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: wie oben.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// Die größte einzelne Anforderung, während `run` läuft.
fn largest_allocation(run: impl FnOnce()) -> usize {
    LARGEST.with(|largest| largest.set(0));
    run();
    LARGEST.with(Cell::get)
}

/// Die minimierte Eingabe des Fuzzers ohne das Auswahl-Byte: Kennung `0x11`,
/// dann `WBITS` 29.
#[test]
fn a_large_window_brotli_stream_allocates_no_window() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/regressions/body_decoder-brotli-large-window.bin");
    let input = std::fs::read(path).unwrap();
    let (&selector, body) = input.split_first().unwrap();
    assert_eq!(selector % 5, 3, "the fixture selects br");

    let mut code = None;
    let largest = largest_allocation(|| {
        let decoded = decode_body(body, &ContentEncoding::Brotli, CAP, RATIO);
        assert!(decoded.truncated);
        code = decoded
            .diagnostic
            .map(|found| found.code.as_str().to_owned());
    });
    assert_eq!(code.as_deref(), Some("FINDINGS_002"));
    assert!(
        largest <= LARGEST_ALLOWED,
        "{} bytes of br asked for a single allocation of {largest} bytes",
        body.len()
    );
}

/// Gegenprobe: Das größte Fenster aus RFC 7932 entpackt weiter und bleibt
/// unter derselben Grenze, auch wenn die Ausgabe bis zum Cap wächst.
#[test]
fn the_widest_rfc_7932_window_stays_under_the_same_bound() {
    let plain: Vec<u8> = (0..12 * 1024 * 1024u32)
        .map(|index| index.wrapping_mul(2_654_435_761).to_be_bytes()[0])
        .collect();
    let mut packed = Vec::new();
    {
        let mut encoder = brotli::CompressorWriter::new(&mut packed, 4096, 1, 24);
        encoder.write_all(&plain).unwrap();
    }

    let mut produced = 0;
    let largest = largest_allocation(|| {
        let decoded = decode_body(&packed, &ContentEncoding::Brotli, CAP, RATIO);
        produced = decoded.bytes.len();
    });
    assert_eq!(produced, CAP, "the output grows up to the cap");
    assert!(largest <= LARGEST_ALLOWED, "largest allocation {largest}");
}
