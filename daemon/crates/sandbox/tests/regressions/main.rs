//! Regressionstests des Fuzz-Targets `control_stream_reader` (HUM-443).
//!
//! Jede Datei `control_stream_reader-<kurzbeschreibung>.bin` in diesem
//! Verzeichnis läuft durch dieselbe Prüfung wie im Fuzzer (`check.rs`). Wie
//! eine Datei hierher kommt, steht in `daemon/fuzz/README.md`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod check;

use std::path::{Path, PathBuf};

use check::frame::{Decoder, Dropped, Event, Kind, Message};

/// Die Eingaben des Targets, nach Namen sortiert.
fn inputs(target: &str) -> Vec<(PathBuf, Vec<u8>)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/regressions");
    let prefix = format!("{target}-");
    let mut found: Vec<(PathBuf, Vec<u8>)> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension().is_some_and(|ext| ext == "bin")
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(&prefix))
        })
        .map(|path| {
            let bytes = std::fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect();
    found.sort();
    assert!(
        !found.is_empty(),
        "no regression input for {target} in {}",
        dir.display()
    );
    found
}

#[test]
fn regressions_replay_control_stream_reader() {
    for (path, bytes) in inputs("control_stream_reader") {
        eprintln!("replaying {}", path.display());
        check::control_stream_reader(&bytes);
    }
}

fn events(name: &str) -> Vec<Event> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/regressions")
        .join(name);
    let bytes = std::fs::read(&path).unwrap();
    let mut decoder = Decoder::new();
    let mut events = Vec::new();
    decoder.push(&bytes, |event| events.push(event));
    events
}

/// Die Seeds sagen, was ihr Name verspricht: Der übergroße Rahmen wird
/// übersprungen, und der Rahmen danach kommt an; die unbekannten Arten und
/// die falsch geformten Rahmen bleiben ohne Nachricht, der gültige Rahmen am
/// Ende nicht.
#[test]
fn the_seeds_are_what_they_claim() {
    let oversized = events("control_stream_reader-oversized-data.bin");
    assert!(matches!(
        oversized.first(),
        Some(Event::Dropped(Dropped::Oversized { kind: b'D', .. }))
    ));
    assert!(matches!(
        oversized.last(),
        Some(Event::Frame(frame)) if Message::parse(frame)
            == Ok(Message::Data { channel: 0, bytes: b"after".to_vec() })
    ));

    let hostile = events("control_stream_reader-unknown-and-malformed.bin");
    let messages: Vec<_> = hostile
        .iter()
        .filter_map(|event| match event {
            Event::Frame(frame) => Message::parse(frame).ok(),
            Event::Dropped(_) => None,
        })
        .collect();
    assert_eq!(
        messages,
        [Message::Exit {
            channel: 3,
            code: 7
        }],
        "only the last frame is a message"
    );

    let valid = events("control_stream_reader-every-kind.bin");
    let kinds: Vec<Kind> = valid
        .iter()
        .map(|event| match event {
            Event::Frame(frame) => {
                Message::parse(frame).unwrap();
                frame.kind
            }
            Event::Dropped(dropped) => panic!("{dropped:?}"),
        })
        .collect();
    assert_eq!(kinds.len(), 9);
}
