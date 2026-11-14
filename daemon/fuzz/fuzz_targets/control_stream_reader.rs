//! Fuzz-Target `control_stream_reader` (HUM-443). Die Prüfung und ihre
//! Invarianten stehen in `daemon/crates/sandbox/tests/regressions/check.rs`;
//! dieselbe Funktion rufen die Regressionstests der Crate auf.

#![no_main]

#[allow(dead_code)]
#[path = "../../crates/sandbox/tests/regressions/check.rs"]
mod check;

libfuzzer_sys::fuzz_target!(|data: &[u8]| check::control_stream_reader(data));
