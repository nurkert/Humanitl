//! Was `GetBody` ausliefert: entpacken, wenn der Klient darum bittet, und in
//! Stücke zerlegen.
//!
//! Der echte Dienst ([`crate::server`]) und der Fake ([`crate::fake`]) gehen
//! hier durch dieselbe Funktion. Vorher stand die Stückelung zweimal im Code,
//! einmal je Seite; eine Oberfläche, die gegen den Fake ein anderes
//! `encoding_left` sähe als gegen den Daemon, prüfte nichts.
//!
//! Entpackt wird über [`humanitl_findings::decode::inflate`] und niemals mit
//! einem eigenen Entpacker: Der Scan sucht in genau diesen Bytes, und nur
//! deshalb zeigt die Stelle eines Fundes in der Oberfläche auf denselben Wert
//! (HUM-119). Das Budget ist dasselbe wie beim Scan, also gilt
//! `limits.preview_cap_bytes` zusammen mit `limits.max_decompress_ratio` auch
//! hier: Eine Bombe von einem Kilobyte füllt den Speicher des Daemons nicht,
//! nur weil jemand `decoded = true` gesetzt hat.

use bytes::Bytes;
use humanitl_findings::decode::{Budget, ContentEncoding, InflateError, inflate};

use crate::v1;

/// So groß ist ein Stück von `GetBody`.
pub const BODY_CHUNK_BYTES: usize = 64 * 1024;

/// Die Bytes, die `GetBody` zu [`v1::BodyRef`] ausliefert, und was danach noch
/// auf ihnen liegt.
///
/// Ohne `decoded` bleibt alles, wie es aufgezeichnet wurde, und
/// `encoding_left` ist leer: Der Klient hat nicht nach dem Auspacken gefragt,
/// also ist auch nichts übrig geblieben, was er nicht erwartet hätte. Mit
/// `decoded` und einer Kodierung, die diese Crate kennt (`gzip`, `deflate`,
/// `br`), kommen die entpackten Bytes zurück und `encoding_left` ist leer. Mit
/// einer Kodierung, die sie nicht kennt (`zstd`, eine Kette wie `gzip, br`),
/// oder mit einem Strom, der sich nicht entpacken lässt, kommen die rohen
/// Bytes zurück und `encoding_left` nennt die Kodierung.
///
/// Bricht das Budget das Entpacken ab, ist das Ergebnis der entpackte Anfang
/// mit leerem `encoding_left`: Diese Bytes sind echt entpackt, es sind nur
/// weniger, und jede Fundstelle darin stimmt weiterhin.
///
/// Ein Body, von dem die Aufzeichnung nur einen Präfix hat
/// (`BodyRef.truncated`), endet mitten im Strom. Dann kommt zurück, was bis
/// dahin entpackt wurde, wieder mit leerem `encoding_left`: Der Anfang eines
/// Textes ist mehr wert als eine Wand aus Hex, und dass er nur ein Anfang ist,
/// steht schon in `truncated`. Ein Strom, der vollständig sein müsste und
/// trotzdem bricht, bekommt diese Nachsicht nicht — dort ist etwas anderes
/// falsch, und die rohen Bytes sind die ehrlichere Antwort.
#[must_use]
pub fn deliver(
    wire: &v1::BodyRef,
    bytes: Bytes,
    cap_bytes: usize,
    max_decompress_ratio: u32,
) -> (Bytes, String) {
    if !wire.decoded {
        return (bytes, String::new());
    }
    let encoding = ContentEncoding::parse(&wire.content_encoding);
    if encoding.is_identity() || bytes.is_empty() {
        return (bytes, String::new());
    }
    let mut budget = Budget::new(bytes.len(), cap_bytes, max_decompress_ratio);
    match inflate(&bytes, &encoding, &mut budget) {
        Ok(inflated) => (Bytes::from(inflated.bytes), String::new()),
        Err(InflateError::BrokenStream { source, partial })
            if wire.truncated && !partial.is_empty() =>
        {
            tracing::debug!(
                encoding = encoding.as_str(),
                why = %source,
                "GetBody returns the prefix that could be unpacked from a truncated body"
            );
            (Bytes::from(partial), String::new())
        }
        Err(why) => {
            tracing::debug!(
                encoding = encoding.as_str(),
                why = %why,
                "GetBody returns the recorded bytes; they were not unpacked"
            );
            (bytes, encoding.as_str().to_owned())
        }
    }
}

/// Zerlegt einen Body in die Stücke, die `GetBody` streamt.
///
/// Auch ein leerer Body ergibt genau ein Stück mit `last = true`: Der Klient
/// soll das Ende sehen und nicht auf ein nächstes warten. `encoding_left`
/// steht in jedem Stück, damit ein Klient, der nur das erste liest, die
/// Auskunft nicht verpasst.
#[must_use]
pub fn chunks(bytes: &Bytes, encoding_left: &str) -> Vec<v1::BodyChunk> {
    let total = bytes.len();
    if total == 0 {
        return vec![v1::BodyChunk {
            data: Vec::new(),
            offset: 0,
            last: true,
            encoding_left: encoding_left.to_owned(),
        }];
    }
    (0..total)
        .step_by(BODY_CHUNK_BYTES)
        .map(|offset| {
            let end = (offset + BODY_CHUNK_BYTES).min(total);
            v1::BodyChunk {
                data: bytes.slice(offset..end).to_vec(),
                offset: offset as u64,
                last: end == total,
                encoding_left: encoding_left.to_owned(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::io::Write as _;

    use bytes::Bytes;
    use flate2::write::GzEncoder;

    use super::{chunks, deliver};
    use crate::v1;

    fn reference(encoding: &str, decoded: bool) -> v1::BodyRef {
        v1::BodyRef {
            sha256: vec![0; 32],
            size: 0,
            truncated: false,
            content_type: String::new(),
            content_encoding: encoding.to_owned(),
            decoded,
        }
    }

    fn gzip(plain: &[u8]) -> Bytes {
        let mut encoder = GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(plain).unwrap();
        Bytes::from(encoder.finish().unwrap())
    }

    #[test]
    fn an_empty_body_is_one_last_chunk() {
        let pieces = chunks(&Bytes::new(), "zstd");
        assert_eq!(pieces.len(), 1);
        assert!(pieces[0].last);
        assert_eq!(pieces[0].encoding_left, "zstd");
    }

    #[test]
    fn every_chunk_carries_the_encoding_left() {
        let bytes = Bytes::from(vec![b'x'; super::BODY_CHUNK_BYTES + 1]);
        let pieces = chunks(&bytes, "zstd");
        assert_eq!(pieces.len(), 2);
        assert!(pieces.iter().all(|piece| piece.encoding_left == "zstd"));
        assert_eq!(pieces[1].offset, super::BODY_CHUNK_BYTES as u64);
        assert!(pieces[1].last);
    }

    #[test]
    fn without_the_flag_nothing_is_unpacked() {
        let packed = gzip(b"hello");
        let (bytes, left) = deliver(&reference("gzip", false), packed.clone(), 4096, 100);
        assert_eq!(bytes, packed);
        assert_eq!(left, "");
    }

    #[test]
    fn a_broken_stream_comes_back_raw_with_its_name() {
        let broken = Bytes::from_static(b"\x1f\x8b\x08");
        let (bytes, left) = deliver(&reference("gzip", true), broken.clone(), 4096, 100);
        assert_eq!(bytes, broken);
        assert_eq!(left, "gzip");
    }

    #[test]
    fn an_empty_body_is_never_called_encoded() {
        let (bytes, left) = deliver(&reference("gzip", true), Bytes::new(), 4096, 100);
        assert!(bytes.is_empty());
        assert_eq!(left, "");
    }
}
