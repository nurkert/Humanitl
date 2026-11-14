//! Die Prüfung hinter dem Fuzz-Target `control_stream_reader` (HUM-443).
//!
//! Eine Quelle für beide Seiten: `main.rs` bindet die Datei als Modul ein, das
//! Target unter `daemon/fuzz/fuzz_targets/control_stream_reader.rs` über
//! `#[path]`. Geprüft wird der Leser, mit dem der Daemon den Steuerkanal aus
//! der Sandbox liest: `Decoder` und `Message::parse` aus
//! `daemon/bin/humanitl-shim/src/frame.rs`, derselben Datei, die
//! `humanitl_sandbox::control_frame` und der Shim übersetzen.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[allow(dead_code)]
#[path = "../../../../bin/humanitl-shim/src/frame.rs"]
pub mod frame;

use frame::{Decoder, Dropped, Event, HEADER, MAX_PAYLOAD, Message};

/// Liest `data` als Strom aus der Sandbox, am Stück und in Stücken.
///
/// Invarianten:
///
/// - Kein Panic, gleich was in `data` steht.
/// - Der Leser hält nie mehr als `HEADER` + `MAX_PAYLOAD` Bytes, gleich
///   was ein Kopf ankündigt. Das ist die Längenprüfung: Ohne sie sammelte er
///   die angekündigten Bytes eines übergroßen Rahmens.
/// - Kein Rahmen trägt mehr als `MAX_PAYLOAD` Bytes; ein Kopf, der mehr
///   ankündigt, endet als `Dropped::Oversized`, nie als Rahmen.
/// - Was herauskommt, hängt nicht davon ab, wie der Strom zerschnitten
///   ankommt.
pub fn control_stream_reader(data: &[u8]) {
    let whole = read(data, data.len().max(1));
    for cut in [1, 7, HEADER, 4096] {
        assert_eq!(read(data, cut), whole, "cut {cut} reads differently");
    }
}

/// Die Ereignisse eines Lesers, der `data` in Stücken von `cut` Bytes
/// bekommt; jedes Ereignis geprüft.
fn read(data: &[u8], cut: usize) -> Vec<Event> {
    let mut decoder = Decoder::new();
    let mut events = Vec::new();
    for chunk in data.chunks(cut) {
        decoder.push(chunk, |event| {
            match &event {
                Event::Frame(frame) => {
                    assert!(
                        frame.payload.len() <= MAX_PAYLOAD,
                        "a frame of {} bytes came through",
                        frame.payload.len()
                    );
                    // Die Form wird geprüft; das Ergebnis ist hier gleich.
                    let _ = Message::parse(frame);
                }
                Event::Dropped(Dropped::Oversized { len, .. }) => {
                    assert!(
                        usize::try_from(*len).unwrap_or(usize::MAX) > MAX_PAYLOAD,
                        "dropped as oversized at {len} bytes"
                    );
                }
                Event::Dropped(Dropped::UnknownKind { .. }) => {}
            }
            events.push(event);
        });
        assert!(
            decoder.buffered() <= HEADER + MAX_PAYLOAD,
            "the reader holds {} bytes",
            decoder.buffered()
        );
    }
    events
}
