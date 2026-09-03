//! Budget, Prozent-Dekodierung und Vorbereitung des Bodys.
//!
//! Alles hier arbeitet auf Bytes und legt nie mehr in den Speicher, als das
//! Budget erlaubt. Die drei Werkzeuge:
//!
//! - [`Budget`] begrenzt jede Ausgabe doppelt: absolut über
//!   `limits.preview_cap_bytes` und relativ über `limits.max_decompress_ratio`.
//!   So kann eine Bombe von 1 KB keine 1 GB erzeugen: der Entpacker schiebt
//!   seine Ausgabe stückweise durch das Budget und hört auf, sobald es leer
//!   ist.
//! - [`percent_decode`] liefert neben der dekodierten Kopie eine Tabelle, mit
//!   der ein Bereich der Kopie auf den Rohtext zurückzeigt.
//! - [`printable_only`] baut die „strings"-Sicht auf einen Body, der kein Text
//!   ist: Läufe aus mindestens [`MIN_PRINTABLE_RUN`] druckbaren Bytes bleiben
//!   stehen, alles andere wird zu `\n`. Die Länge bleibt gleich, deshalb
//!   zeigen alle Bereiche unverändert auf den echten Body.

use core::fmt;
use std::io::Read;

use flate2::read::{DeflateDecoder, GzDecoder, ZlibDecoder};
use humanitl_core::diagnostics::codes::FINDINGS_002;
use humanitl_core::{Diagnostic, Severity};

/// So viel liest der Entpacker auf einmal.
const DECOMPRESS_CHUNK: usize = 16 * 1024;

/// So viele druckbare Bytes am Stück gelten als Text (`strings`-Modus).
pub const MIN_PRINTABLE_RUN: usize = 8;

/// Die Kodierung eines Bodys, aus `Content-Encoding` gelesen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentEncoding {
    /// Keine Kodierung oder ausdrücklich `identity`.
    Identity,
    /// `gzip` oder `x-gzip`.
    Gzip,
    /// `deflate`.
    Deflate,
    /// `br`.
    Brotli,
    /// Etwas anderes; der Wert steht darin, wie er in der Kopfzeile stand.
    Other(String),
}

impl ContentEncoding {
    /// Liest den Kopfzeilen-Wert, auch als Liste (`gzip, br`).
    ///
    /// `identity` und leere Glieder fallen weg. Bleibt genau eine bekannte
    /// Kodierung übrig, ist sie das Ergebnis; bleiben mehrere übrig, ist das
    /// [`ContentEncoding::Other`] mit der ganzen Kette.
    #[must_use]
    pub fn parse(value: &str) -> Self {
        let parts: Vec<&str> = value
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty() && !part.eq_ignore_ascii_case("identity"))
            .collect();
        match parts.as_slice() {
            [] => Self::Identity,
            [one] => match one.to_ascii_lowercase().as_str() {
                "gzip" | "x-gzip" => Self::Gzip,
                "deflate" => Self::Deflate,
                "br" => Self::Brotli,
                other => Self::Other(other.to_owned()),
            },
            many => Self::Other(many.join(", ").to_ascii_lowercase()),
        }
    }

    /// Der Name, wie er in einer Meldung erscheint.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Identity => "identity",
            Self::Gzip => "gzip",
            Self::Deflate => "deflate",
            Self::Brotli => "br",
            Self::Other(name) => name,
        }
    }

    /// Wahr, wenn der Body ohne Entpacker durchsucht werden kann.
    #[must_use]
    pub const fn is_identity(&self) -> bool {
        matches!(self, Self::Identity)
    }
}

/// Ein doppeltes Budget: absolut und relativ zur Eingabe.
///
/// `allowance` ist das Kleinere aus `cap_bytes` und
/// `input_len * max_decompress_ratio`. Ein Entpacker schiebt seine Ausgabe
/// stückweise durch [`Budget::take`] und hört auf, sobald `0` zurückkommt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    allowance: usize,
    produced: usize,
}

impl Budget {
    /// Baut das Budget für eine Eingabe dieser Länge.
    #[must_use]
    pub fn new(input_len: usize, cap_bytes: usize, max_ratio: u32) -> Self {
        let ratio = usize::try_from(max_ratio.max(1)).unwrap_or(usize::MAX);
        let relative = input_len.saturating_mul(ratio);
        Self {
            allowance: cap_bytes.min(relative),
            produced: 0,
        }
    }

    /// So viele Bytes dürfen insgesamt entstehen.
    #[must_use]
    pub const fn allowance(&self) -> usize {
        self.allowance
    }

    /// So viele Bytes sind schon entstanden.
    #[must_use]
    pub const fn produced(&self) -> usize {
        self.produced
    }

    /// Nimmt so viel vom Budget, wie noch übrig ist, und meldet die Menge.
    ///
    /// Ist die Rückgabe kleiner als `wanted`, ist das Budget erschöpft und der
    /// Aufrufer bricht ab; der Scan gilt dann als unvollständig.
    pub const fn take(&mut self, wanted: usize) -> usize {
        let left = self.allowance - self.produced;
        let granted = if wanted < left { wanted } else { left };
        self.produced += granted;
        granted
    }

    /// So viele Bytes dürfen noch entstehen.
    #[must_use]
    pub const fn remaining(&self) -> usize {
        self.allowance - self.produced
    }

    /// Wahr, wenn nichts mehr übrig ist.
    #[must_use]
    pub const fn is_exhausted(&self) -> bool {
        self.produced >= self.allowance
    }
}

/// Ein Body, so wie der Scan ihn sieht.
///
/// Ohne eigenes [`fmt::Debug`]: Ein abgeleitetes `Debug` würde den ganzen Body
/// als Zahlenliste ausgeben, und damit stünde jedes Geheimnis wieder im Log,
/// nur in einer anderen Schreibweise. Gedruckt werden Länge und Zustand.
#[derive(Clone, PartialEq)]
pub struct DecodedBody {
    /// Die Bytes, auf die sich alle Bereiche beziehen.
    pub bytes: Vec<u8>,
    /// Wahr, wenn nicht der ganze Body durchsucht wurde.
    pub truncated: bool,
    /// Der Befund, der die Lücke erklärt.
    pub diagnostic: Option<Diagnostic>,
}

impl fmt::Debug for DecodedBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecodedBody")
            .field("bytes", &Bytes(self.bytes.len()))
            .field("truncated", &self.truncated)
            .field(
                "diagnostic",
                &self.diagnostic.as_ref().map(|found| found.code.as_str()),
            )
            .finish()
    }
}

/// Eine Länge in Bytes, gedruckt als `<n Bytes>`.
///
/// Die eine Stelle, an der ein Inhalt zu einer Zahl wird. Wer einen Wert
/// braucht, schneidet ihn selbst aus dem Body; ein `Debug` gibt ihn nie her.
pub(crate) struct Bytes(pub usize);

impl fmt::Debug for Bytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{} Bytes>", self.0)
    }
}

/// Bereitet den Body für den Scan vor.
///
/// Ohne Kodierung wird der Body auf `cap_bytes` gekürzt. Mit `gzip`, `deflate`
/// oder `br` wird er entpackt, und zwar durch dasselbe [`Budget`]: Es hält
/// gleichzeitig `limits.preview_cap_bytes` und `limits.max_decompress_ratio`
/// ein, damit eine Bombe nicht den Speicher des Daemons füllt. Alle Bereiche
/// der Funde zeigen danach auf die entpackten Bytes.
///
/// Drei Fälle melden `FINDINGS_002` und setzen `truncated`: der Body liegt über
/// dem Cap, das Budget hat das Entpacken abgebrochen, oder der Body ließ sich
/// nicht entpacken (unbekannte Kodierung, abgeschnittener oder kaputter Strom).
/// Im letzten Fall wird gar nichts durchsucht, statt in gepackten Bytes nach
/// Mustern zu suchen und Bereiche zu liefern, die auf niemanden zeigen.
#[must_use]
pub fn decode_body(
    raw: &[u8],
    encoding: &ContentEncoding,
    cap_bytes: usize,
    max_ratio: u32,
) -> DecodedBody {
    if raw.is_empty() {
        // Eine Anfrage ohne Body ist vollständig durchsucht, auch wenn eine
        // Kopfzeile eine Kodierung nennt. Ohne diese Zeile meldete jeder leere
        // Body mit `Content-Encoding` einen beschädigten Strom.
        return DecodedBody {
            bytes: Vec::new(),
            truncated: false,
            diagnostic: None,
        };
    }

    let mut budget = Budget::new(raw.len(), cap_bytes, max_ratio);
    if encoding.is_identity() {
        let granted = budget.take(raw.len());
        if granted < raw.len() {
            return DecodedBody {
                bytes: raw[..granted].to_vec(),
                truncated: true,
                diagnostic: Some(over_cap(raw.len(), granted, cap_bytes)),
            };
        }
        return DecodedBody {
            bytes: raw.to_vec(),
            truncated: false,
            diagnostic: None,
        };
    }

    match inflate(raw, encoding, &mut budget) {
        Ok(Inflated {
            bytes,
            stopped: false,
        }) => DecodedBody {
            bytes,
            truncated: false,
            diagnostic: None,
        },
        Ok(Inflated {
            bytes,
            stopped: true,
        }) => {
            let produced = bytes.len();
            DecodedBody {
                bytes,
                truncated: true,
                diagnostic: Some(
                    Diagnostic::builder(FINDINGS_002, Severity::Warning)
                        .why(format!(
                            "der Body ist mit \"{}\" kodiert und beim Entpacken über das Budget \
                             gelaufen; durchsucht wurden die ersten {produced} Bytes \
                             (limits.preview_cap_bytes = {cap_bytes}, \
                             limits.max_decompress_ratio = {max_ratio}, gepackt {} Bytes)",
                            encoding.as_str(),
                            raw.len()
                        ))
                        .fix(humanitl_core::FixAction::ChangeSetting {
                            key: "limits.max_decompress_ratio".to_owned(),
                            value: max_ratio.saturating_mul(2).to_string(),
                        })
                        .build(),
                ),
            }
        }
        Err(why) => DecodedBody {
            bytes: Vec::new(),
            truncated: true,
            diagnostic: Some(
                Diagnostic::builder(FINDINGS_002, Severity::Warning)
                    .why(format!(
                        "der Body ist mit \"{}\" kodiert und wurde nicht durchsucht: {why}",
                        encoding.as_str()
                    ))
                    .build(),
            ),
        },
    }
}

/// Warum ein Body nicht entpackt werden konnte.
///
/// Ein Fehler ist ein Wert mit Bedeutung, kein Text (ADR-012,
/// `scripts/ci/lint-no-string-errors.sh`). [`decode_body`] macht daraus den
/// Befund `FINDINGS_002`; der Wortlaut des Fehlers steht dort im `why`.
#[derive(Debug, thiserror::Error)]
pub enum InflateError {
    /// Für diese Kodierung gibt es in dieser Crate keinen Entpacker.
    #[error("für \"{0}\" gibt es keinen Entpacker")]
    UnsupportedEncoding(String),
    /// Der Strom endet zu früh oder ist beschädigt.
    #[error("der Strom ist unvollständig oder beschädigt ({0})")]
    BrokenStream(#[from] std::io::Error),
}

/// Was beim Entpacken herauskam.
///
/// Ohne abgeleitetes `Debug`, aus demselben Grund wie [`DecodedBody`].
#[derive(Clone, PartialEq, Eq)]
pub struct Inflated {
    /// Die entpackten Bytes, höchstens so viele wie das Budget erlaubt.
    pub bytes: Vec<u8>,
    /// Wahr, wenn das Budget vor dem Ende des Stroms zugemacht hat.
    pub stopped: bool,
}

impl fmt::Debug for Inflated {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Inflated")
            .field("bytes", &Bytes(self.bytes.len()))
            .field("stopped", &self.stopped)
            .finish()
    }
}

/// Entpackt einen Body durch das Budget.
///
/// # Errors
///
/// [`InflateError`], wenn kein Entpacker zuständig ist oder der Strom kaputt
/// ist. Der Aufrufer macht daraus `FINDINGS_002`.
pub fn inflate(
    raw: &[u8],
    encoding: &ContentEncoding,
    budget: &mut Budget,
) -> Result<Inflated, InflateError> {
    match encoding {
        ContentEncoding::Identity => Ok(Inflated {
            bytes: raw.to_vec(),
            stopped: false,
        }),
        ContentEncoding::Gzip => read_limited(GzDecoder::new(raw), budget),
        // "deflate" ist im Netz zweideutig: die meisten Server schicken den
        // zlib-Rahmen aus RFC 1950, manche den nackten Strom aus RFC 1951.
        // Erst der Rahmen, dann der nackte Strom.
        ContentEncoding::Deflate => {
            let mut framed = *budget;
            match read_limited(ZlibDecoder::new(raw), &mut framed) {
                Ok(result) => {
                    *budget = framed;
                    Ok(result)
                }
                Err(_) => read_limited(DeflateDecoder::new(raw), budget),
            }
        }
        ContentEncoding::Brotli => {
            read_limited(brotli::Decompressor::new(raw, DECOMPRESS_CHUNK), budget)
        }
        ContentEncoding::Other(name) => Err(InflateError::UnsupportedEncoding(name.clone())),
    }
}

/// Liest einen Strom, bis er endet oder das Budget zumacht.
fn read_limited<R: Read>(mut reader: R, budget: &mut Budget) -> Result<Inflated, InflateError> {
    let mut bytes = Vec::new();
    let mut chunk = [0u8; DECOMPRESS_CHUNK];
    loop {
        let left = budget.remaining();
        if left == 0 {
            // Ein Byte mehr lesen, um "genau voll" von "zu viel" zu trennen.
            return Ok(Inflated {
                stopped: reader.read(&mut chunk[..1])? > 0,
                bytes,
            });
        }
        // Ein Byte über das Budget hinaus anfragen: kommt es, war der Strom
        // länger als erlaubt, und der Scan ist unvollständig.
        let want = DECOMPRESS_CHUNK.min(left.saturating_add(1));
        let read = reader.read(&mut chunk[..want])?;
        if read == 0 {
            return Ok(Inflated {
                bytes,
                stopped: false,
            });
        }
        if read > left {
            bytes.extend_from_slice(&chunk[..left]);
            budget.take(left);
            return Ok(Inflated {
                bytes,
                stopped: true,
            });
        }
        bytes.extend_from_slice(&chunk[..read]);
        budget.take(read);
    }
}

/// Der Befund für einen Body über `limits.preview_cap_bytes`.
fn over_cap(size: usize, scanned: usize, cap_bytes: usize) -> Diagnostic {
    Diagnostic::builder(FINDINGS_002, Severity::Warning)
        .why(format!(
            "der Body ist {size} Bytes groß; durchsucht wurden die ersten {scanned} \
             (limits.preview_cap_bytes = {cap_bytes})"
        ))
        .fix(humanitl_core::FixAction::ChangeSetting {
            key: "limits.preview_cap_bytes".to_owned(),
            value: size.to_string(),
        })
        .build()
}

/// Dekodiert `%XX` und liefert die Rückzeigetabelle.
///
/// Die Tabelle hat ein Feld mehr als die Kopie: `table[i]` ist der Index im
/// Rohtext, an dem das dekodierte Byte `i` beginnt, und `table[len]` ist die
/// Länge des Rohtexts. Ein Bereich `a..b` der Kopie wird damit zu
/// `table[a]..table[b]`. Die Tabelle wächst monoton, auch wenn ein `%00` in der
/// Kopie steht: sie zählt Rohindizes, nicht Zeichen.
#[must_use]
pub fn percent_decode(raw: &[u8]) -> (Vec<u8>, Vec<usize>) {
    let mut decoded = Vec::with_capacity(raw.len());
    let mut table = Vec::with_capacity(raw.len() + 1);
    let mut index = 0usize;
    while index < raw.len() {
        if raw[index] == b'%'
            && index + 2 < raw.len()
            && let (Some(high), Some(low)) =
                (hex_nibble(raw[index + 1]), hex_nibble(raw[index + 2]))
        {
            decoded.push((high << 4) | low);
            table.push(index);
            index += 3;
            continue;
        }
        decoded.push(raw[index]);
        table.push(index);
        index += 1;
    }
    table.push(raw.len());
    (decoded, table)
}

const fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Baut die „strings"-Sicht: nur Läufe druckbaren Texts bleiben stehen.
///
/// Druckbar ist ASCII `0x20..=0x7E`, Tabulator, Zeilenumbruch, Wagenrücklauf
/// sowie jedes gültige UTF-8-Zeichen über `0x7F`, das kein Steuerzeichen ist.
/// Alles andere und jeder Lauf unter `min_run` Bytes wird zu `\n`, einem Byte,
/// das keine Wortgrenze überbrückt und in keinem Muster vorkommt. Die Länge der
/// Ausgabe ist die der Eingabe, deshalb bleiben alle Bereiche gültig.
#[must_use]
pub fn printable_only(bytes: &[u8], min_run: usize) -> Vec<u8> {
    let mut keep = vec![false; bytes.len()];
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte < 0x80 {
            keep[index] = matches!(byte, 0x20..=0x7E | b'\t' | b'\n' | b'\r');
            index += 1;
            continue;
        }
        match utf8_len(byte) {
            Some(len) if index + len <= bytes.len() => {
                match core::str::from_utf8(&bytes[index..index + len]) {
                    Ok(text) if !text.chars().any(char::is_control) => {
                        for slot in &mut keep[index..index + len] {
                            *slot = true;
                        }
                        index += len;
                    }
                    _ => index += 1,
                }
            }
            _ => index += 1,
        }
    }

    let mut out = bytes.to_vec();
    let mut run_start = 0usize;
    let mut position = 0usize;
    while position <= keep.len() {
        let inside = position < keep.len() && keep[position];
        if !inside {
            if position - run_start < min_run {
                for slot in &mut out[run_start..position] {
                    *slot = b'\n';
                }
            }
            if position < out.len() {
                out[position] = b'\n';
            }
            run_start = position + 1;
        }
        position += 1;
    }
    out
}

const fn utf8_len(lead: u8) -> Option<usize> {
    match lead {
        0xC2..=0xDF => Some(2),
        0xE0..=0xEF => Some(3),
        0xF0..=0xF4 => Some(4),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::io::Write as _;

    use flate2::Compression;
    use flate2::write::{DeflateEncoder, GzEncoder, ZlibEncoder};

    use super::{
        Budget, ContentEncoding, MIN_PRINTABLE_RUN, decode_body, percent_decode, printable_only,
    };

    fn gzip(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn encodings_are_read_from_the_header() {
        assert_eq!(ContentEncoding::parse(""), ContentEncoding::Identity);
        assert_eq!(
            ContentEncoding::parse("identity"),
            ContentEncoding::Identity
        );
        assert_eq!(ContentEncoding::parse("GZIP"), ContentEncoding::Gzip);
        assert_eq!(ContentEncoding::parse(" br "), ContentEncoding::Brotli);
        assert_eq!(
            ContentEncoding::parse("gzip, br"),
            ContentEncoding::Other("gzip, br".to_owned())
        );
    }

    #[test]
    fn the_budget_stops_a_bomb_at_the_ratio() {
        // 1 KB Eingabe, Ratio 100: mehr als 100 KB darf nicht entstehen, auch
        // wenn der Cap bei 1 GB liegt.
        let mut budget = Budget::new(1024, 1024 * 1024 * 1024, 100);
        assert_eq!(budget.allowance(), 102_400);
        let mut produced = 0usize;
        for _ in 0..1000 {
            let granted = budget.take(1024);
            produced += granted;
            if granted < 1024 {
                break;
            }
        }
        assert_eq!(produced, 102_400);
        assert!(budget.is_exhausted());
        assert_eq!(budget.take(1), 0);
    }

    #[test]
    fn the_budget_stops_at_the_cap_when_the_ratio_would_allow_more() {
        let mut budget = Budget::new(1024, 4096, 100);
        assert_eq!(budget.allowance(), 4096);
        assert_eq!(budget.take(usize::MAX), 4096);
    }

    #[test]
    fn a_body_over_the_cap_is_cut_and_reported() {
        let raw = vec![b'a'; 100];
        let decoded = decode_body(&raw, &ContentEncoding::Identity, 10, 100);
        assert_eq!(decoded.bytes.len(), 10);
        assert!(decoded.truncated);
        assert_eq!(
            decoded.diagnostic.as_ref().map(|d| d.code.as_str()),
            Some("FINDINGS_002")
        );
    }

    #[test]
    fn a_gzip_body_is_decoded_whole() {
        let plain = b"iban GB82 WEST 1234 5698 7654 32 und mehr Text";
        let decoded = decode_body(&gzip(plain), &ContentEncoding::Gzip, 4096, 100);
        assert_eq!(decoded.bytes, plain);
        assert!(!decoded.truncated);
        assert!(decoded.diagnostic.is_none());
    }

    #[test]
    fn deflate_is_read_with_and_without_the_zlib_frame() {
        let plain = b"AKIAIOSFODNN7EXAMPLE steht hier drin, mehrfach AKIAIOSFODNN7EXAMPLE";
        let mut framed = ZlibEncoder::new(Vec::new(), Compression::default());
        framed.write_all(plain).unwrap();
        let framed = framed.finish().unwrap();
        assert_eq!(
            decode_body(&framed, &ContentEncoding::Deflate, 4096, 100).bytes,
            plain
        );

        let mut bare = DeflateEncoder::new(Vec::new(), Compression::default());
        bare.write_all(plain).unwrap();
        let bare = bare.finish().unwrap();
        assert_eq!(
            decode_body(&bare, &ContentEncoding::Deflate, 4096, 100).bytes,
            plain
        );
    }

    #[test]
    fn a_brotli_body_is_decoded_whole() {
        let plain = b"Kontakt: vorname.nachname@kunde.de, bitte antworten";
        let mut packed = Vec::new();
        {
            let mut encoder = brotli::CompressorWriter::new(&mut packed, 4096, 5, 22);
            encoder.write_all(plain).unwrap();
        }
        let decoded = decode_body(&packed, &ContentEncoding::Brotli, 4096, 100);
        assert_eq!(decoded.bytes, plain);
        assert!(!decoded.truncated);
    }

    #[test]
    fn gzip_bomb_truncated() {
        // Die Spezifikation nennt 1 KB, die zu 1 GB werden. Der Test muss das
        // Archiv selbst herstellen, deshalb steht hier 1 MiB Nullbytes; das
        // Verhältnis ist dasselbe, und nur darauf kommt es an. Mit Ratio 100
        // darf aus 1 KB gepackt nur 100 KB entpackt werden.
        let bomb = gzip(&vec![0u8; 1024 * 1024]);
        assert!(bomb.len() < 16 * 1024, "gepackt sind {} Bytes", bomb.len());

        let decoded = decode_body(&bomb, &ContentEncoding::Gzip, 8 * 1024 * 1024, 100);
        assert!(decoded.truncated);
        assert_eq!(decoded.bytes.len(), bomb.len() * 100);
        let diagnostic = decoded.diagnostic.unwrap();
        assert_eq!(diagnostic.code.as_str(), "FINDINGS_002");
        assert!(diagnostic.why.contains("max_decompress_ratio"));
    }

    #[test]
    fn the_cap_also_stops_a_bomb_with_a_generous_ratio() {
        let bomb = gzip(&vec![0u8; 1024 * 1024]);
        let decoded = decode_body(&bomb, &ContentEncoding::Gzip, 4096, 1_000_000);
        assert!(decoded.truncated);
        assert_eq!(decoded.bytes.len(), 4096);
    }

    #[test]
    fn a_broken_stream_is_reported_and_not_scanned() {
        let decoded = decode_body(b"\x1f\x8b\x08", &ContentEncoding::Gzip, 4096, 100);
        assert!(decoded.bytes.is_empty());
        assert!(decoded.truncated);
        let diagnostic = decoded.diagnostic.unwrap();
        assert_eq!(diagnostic.code.as_str(), "FINDINGS_002");
        assert!(diagnostic.why.contains("gzip"));
    }

    #[test]
    fn an_empty_body_is_complete_even_with_an_encoding() {
        for encoding in [
            ContentEncoding::Identity,
            ContentEncoding::Gzip,
            ContentEncoding::Other("zstd".to_owned()),
        ] {
            let decoded = decode_body(b"", &encoding, 4096, 100);
            assert!(decoded.bytes.is_empty());
            assert!(!decoded.truncated, "{encoding:?}");
            assert!(decoded.diagnostic.is_none(), "{encoding:?}");
        }
    }

    #[test]
    fn an_unknown_encoding_is_reported_and_not_scanned() {
        let decoded = decode_body(
            b"irgendwas",
            &ContentEncoding::Other("zstd".to_owned()),
            4096,
            100,
        );
        assert!(decoded.bytes.is_empty());
        assert!(decoded.truncated);
        assert!(decoded.diagnostic.unwrap().why.contains("zstd"));
    }

    #[test]
    fn percent_decoding_keeps_the_table_monotone() {
        let (decoded, table) = percent_decode(b"q=user%40example.com&z=%00%zz");
        assert_eq!(
            core::str::from_utf8(&decoded[..20]).unwrap(),
            "q=user@example.com&z"
        );
        assert_eq!(table.len(), decoded.len() + 1);
        assert!(table.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(*table.last().unwrap(), 29);
        let start = decoded
            .windows(16)
            .position(|window| window == b"user@example.com")
            .unwrap();
        assert_eq!(table[start]..table[start + 16], 2..20);
    }

    #[test]
    fn strings_mode_keeps_long_runs_and_positions() {
        let mut bytes = vec![0u8; 32];
        bytes.extend_from_slice(b"AKIAIOSFODNN7EXAMPLE");
        bytes.extend_from_slice(&[0xff, 0xfe]);
        let filtered = printable_only(&bytes, MIN_PRINTABLE_RUN);
        assert_eq!(filtered.len(), bytes.len());
        assert_eq!(&filtered[32..52], b"AKIAIOSFODNN7EXAMPLE");
        assert!(filtered[..32].iter().all(|byte| *byte == b'\n'));
    }

    #[test]
    fn strings_mode_drops_short_runs_and_keeps_utf8() {
        let filtered = printable_only("ab\u{0}Müller-Projekt".as_bytes(), MIN_PRINTABLE_RUN);
        assert!(filtered.starts_with(b"\n\n\n"));
        assert!(
            core::str::from_utf8(&filtered)
                .unwrap()
                .contains("Müller-Projekt")
        );
    }
}
