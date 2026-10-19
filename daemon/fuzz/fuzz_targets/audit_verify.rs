//! Fuzz-Target `audit_verify` (HUM-056). Die Prüfung und ihre Invarianten stehen in
//! `daemon/crates/audit/tests/regressions/check.rs`; dieselbe Funktion rufen die
//! Regressionstests der Crate auf.

#![no_main]

#[allow(dead_code)]
#[path = "../../crates/audit/tests/regressions/check.rs"]
mod check;

libfuzzer_sys::fuzz_target!(|data: &[u8]| check::audit_verify(data));
