//! Fuzz-Target `chunked_decoder` (HUM-056). Die Prüfung und ihre Invarianten stehen in
//! `daemon/crates/proxy/tests/regressions/check.rs`; dieselbe Funktion rufen die
//! Regressionstests der Crate auf.

#![no_main]

#[allow(dead_code)]
#[path = "../../crates/proxy/tests/regressions/check.rs"]
mod check;

libfuzzer_sys::fuzz_target!(|data: &[u8]| check::chunked_decoder(data));
