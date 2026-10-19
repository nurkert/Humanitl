//! Die Prüfung hinter dem Fuzz-Target `audit_verify` (HUM-056).
//!
//! Eine Quelle für beide Seiten: `main.rs` bindet die Datei als Modul ein, das
//! Target unter `daemon/fuzz/fuzz_targets/audit_verify.rs` über `#[path]`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use humanitl_audit::{AuditVerifier, VerifyStatus};

/// Der HMAC-Schlüssel der Prüfung, derselbe wie in `tests/common/mod.rs`.
///
/// Die Spezifikation nennt `Key::test()` unter `cfg(feature = "fuzzing")`.
/// Der Prüfer nimmt den Schlüssel als `&[u8; 32]`; ein fester Wert im Test
/// genügt dafür, und kein Produktions-Build trägt ihn.
pub const KEY: [u8; 32] = [7; 32];

/// Eine gültige Kette aus fünf Records, mit [`KEY`] geschrieben.
pub const VALID_CHAIN: &[u8] = include_bytes!("audit_verify-valid-five.bin");

/// Prüft `data` als `audit.jsonl` und kippt Bits in einer gültigen Kette.
///
/// Invarianten:
///
/// - Kein Panic, gleich was in `data` steht.
/// - Eine Kette, die hält, hat höchstens so viele Records, wie `data` Zeilen.
/// - Die gültige Kette [`VALID_CHAIN`] hält; dieselbe Kette mit einem
///   gekippten Byte hält nie. Welches Byte und welche Bits, bestimmen die
///   ersten drei Bytes von `data`.
pub fn audit_verify(data: &[u8]) {
    let report = AuditVerifier::verify_reader(data, Some(&KEY), &[]).unwrap();
    if report.status == VerifyStatus::Ok {
        let lines = data.split(|byte| *byte == b'\n').count() as u64;
        assert!(
            report.records <= lines,
            "{} records from {lines} lines",
            report.records
        );
    }

    if let [high, low, mask, ..] = *data {
        let position = usize::from(u16::from_be_bytes([high, low])) % VALID_CHAIN.len();
        let mask = if mask == 0 { 1 } else { mask };
        assert_flip_breaks(position, mask);
    }
}

/// Kippt in [`VALID_CHAIN`] an `position` die Bits aus `mask` und verlangt,
/// dass die Kette danach nicht hält.
pub fn assert_flip_breaks(position: usize, mask: u8) {
    let mut chain = VALID_CHAIN.to_vec();
    chain[position] ^= mask;
    let report = AuditVerifier::verify_reader(chain.as_slice(), Some(&KEY), &[]).unwrap();
    assert!(
        matches!(report.status, VerifyStatus::Broken { .. }),
        "flipping {mask:#04x} at byte {position} left the chain intact: {report:?}"
    );
}
