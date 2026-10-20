//! Die Prüfung hinter dem Fuzz-Target `body_decoder` (HUM-056).
//!
//! Eine Quelle für beide Seiten: `main.rs` bindet die Datei als Modul ein, das
//! Target unter `daemon/fuzz/fuzz_targets/body_decoder.rs` über `#[path]`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use humanitl_findings::decode::{ContentEncoding, decode_body};

/// Das absolute Budget des Targets: `limits.preview_cap_bytes` wie im Daemon.
pub const CAP: usize = 8 * 1024 * 1024;

/// Das relative Budget des Targets: `limits.max_decompress_ratio` wie im Daemon.
pub const RATIO: u32 = 100;

/// Die Kodierung, die das erste Byte einer Eingabe wählt.
#[must_use]
pub fn encoding(selector: u8) -> ContentEncoding {
    match selector % 5 {
        0 => ContentEncoding::Identity,
        1 => ContentEncoding::Gzip,
        2 => ContentEncoding::Deflate,
        3 => ContentEncoding::Brotli,
        _ => ContentEncoding::Other("zstd".to_owned()),
    }
}

/// Entpackt `data[1..]` mit der Kodierung aus `data[0]` durch das Budget.
///
/// Invarianten:
///
/// - Kein Panic.
/// - Die Ausgabe ist nie größer als [`CAP`] und nie größer als
///   `Eingabe * RATIO`; das ist die Ratio-Grenze, die greift, während
///   entpackt wird, nicht danach.
/// - Ein Body, der nicht als gekürzt gemeldet wird, trägt keinen Befund.
/// - Ein gekürzter Body trägt immer einen Befund; eine Lücke ohne Erklärung
///   gibt es nicht. Es ist `LIMIT_003`, wenn das Verhältnis die engere Grenze
///   war und die Ausgabe genau dort endet, sonst `FINDINGS_002` (HUM-057).
pub fn body_decoder(data: &[u8]) {
    let Some((&selector, raw)) = data.split_first() else {
        return;
    };
    let decoded = decode_body(raw, &encoding(selector), CAP, RATIO);
    let relative = raw.len().saturating_mul(usize::try_from(RATIO).unwrap());
    assert!(
        decoded.bytes.len() <= CAP,
        "decoded {} bytes over the cap {CAP}",
        decoded.bytes.len()
    );
    assert!(
        decoded.bytes.len() <= relative,
        "decoded {} bytes from {} input bytes, over the ratio {RATIO}",
        decoded.bytes.len(),
        raw.len()
    );
    if decoded.truncated {
        let code = decoded.diagnostic.as_ref().map(|found| found.code.as_str());
        match code {
            // Das Verhältnis zog: Es muss die engere Grenze gewesen sein, und
            // die Ausgabe endet genau an ihr.
            Some("LIMIT_003") => {
                assert!(
                    relative < CAP,
                    "LIMIT_003 although the cap {CAP} was the narrower limit ({relative})"
                );
                assert_eq!(
                    decoded.bytes.len(),
                    relative,
                    "LIMIT_003 before the ratio was reached"
                );
            }
            Some("FINDINGS_002") => {}
            other => panic!("a truncated body without FINDINGS_002 or LIMIT_003: {other:?}"),
        }
    } else {
        assert!(
            decoded.diagnostic.is_none(),
            "a complete body with a diagnostic"
        );
    }
}
