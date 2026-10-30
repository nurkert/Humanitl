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

use flate2::bufread::{DeflateDecoder, GzDecoder, ZlibDecoder};
use humanitl_core::diagnostics::codes::{FINDINGS_002, LIMIT_003};
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
    /// Wahr, wenn das Verhältnis die engere Grenze ist und nicht das Cap.
    by_ratio: bool,
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
            by_ratio: relative < cap_bytes,
        }
    }

    /// Wahr, wenn `limits.max_decompress_ratio` die Grenze zieht und nicht
    /// `limits.preview_cap_bytes`.
    ///
    /// Bei Gleichstand gilt das Cap: Dann hätte auch ein größeres
    /// Verhältnis nicht mehr Bytes erlaubt, und der Befund soll nicht auf
    /// eine Bombe zeigen, wo nur die Vorschau-Grenze erreicht ist (HUM-057).
    #[must_use]
    pub const fn limited_by_ratio(&self) -> bool {
        self.by_ratio
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
/// Fünf Fälle melden `FINDINGS_002` und setzen `truncated`: der Body liegt über
/// dem Cap, das Cap hat das Entpacken abgebrochen, der Body ließ sich nicht
/// entpacken (unbekannte Kodierung, abgeschnittener oder kaputter Strom),
/// hinter dem Ende des Stroms liegen weitere Bytes (HUM-247), oder ein
/// `deflate`-Body ergibt als zlib-Rahmen und als nackter Strom Verschiedenes
/// (HUM-266). Im vierten Fall wird durchsucht, was vor dem Ende stand; der Rest
/// bleibt eine benannte Lücke, weil ein anderer Entpacker ihn anders lesen
/// kann. Im fünften wird die zlib-Lesart durchsucht, und die nackte Lesart
/// bleibt die benannte Lücke.
/// Ein sechster meldet stattdessen `LIMIT_003`: Das Entpacken brach am
/// Verhältnis ab, `limits.max_decompress_ratio` war die engere Grenze
/// ([`Budget::limited_by_ratio`], HUM-057).
/// Bei einem Body, der sich nicht entpacken ließ, wird gar nichts durchsucht,
/// statt in gepackten Bytes nach Mustern zu suchen und Bereiche zu liefern, die
/// auf niemanden zeigen.
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
            ambiguity: None,
        }) => DecodedBody {
            bytes,
            truncated: false,
            diagnostic: None,
        },
        Ok(Inflated {
            bytes,
            stopped: false,
            ambiguity: Some(ambiguity),
        }) => {
            let scanned = bytes.len();
            DecodedBody {
                bytes,
                truncated: true,
                diagnostic: Some(ambiguous_frame(ambiguity, raw.len(), scanned)),
            }
        }
        Ok(Inflated {
            bytes,
            stopped: true,
            ..
        }) if budget.limited_by_ratio() => {
            let produced = bytes.len();
            DecodedBody {
                bytes,
                truncated: true,
                diagnostic: Some(ratio_exceeded(encoding, raw.len(), produced, max_ratio)),
            }
        }
        Ok(Inflated {
            bytes,
            stopped: true,
            ..
        }) => {
            let produced = bytes.len();
            DecodedBody {
                bytes,
                truncated: true,
                diagnostic: Some(cap_exceeded(
                    encoding,
                    raw.len(),
                    produced,
                    cap_bytes,
                    max_ratio,
                )),
            }
        }
        Err(InflateError::TrailingBytes { decoded }) => {
            let scanned = decoded.len();
            DecodedBody {
                bytes: decoded,
                truncated: true,
                diagnostic: Some(trailing_gap(encoding, raw.len(), scanned)),
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
    ///
    /// `partial` trägt, was bis zum Abbruch entstanden ist. Der Scan wirft es
    /// weg und meldet `FINDINGS_002`: Ein halber Body führt zu halben Funden,
    /// und ein Fund ohne Ende ist keiner. `GetBody` dagegen darf den Anfang
    /// ausliefern, wenn die Aufzeichnung ohnehin nur einen Präfix hat
    /// (HUM-119); ein Mensch liest lieber den Anfang als eine Wand aus Hex.
    #[error("der Strom ist unvollständig oder beschädigt ({source})")]
    BrokenStream {
        /// Der Fehler des Entpackers.
        #[source]
        source: std::io::Error,
        /// Was bis zum Abbruch entpackt wurde.
        partial: Vec<u8>,
    },
    /// Der Strom endet sauber, aber dahinter liegen noch Bytes (HUM-247).
    ///
    /// Weder `deflate` noch `br` kennen ein zweites Stück; was hinter dem
    /// Ende steht, ignorieren die Entpacker dieser Crate. Ein nachsichtiger
    /// Entpacker auf der Gegenseite kann es aber als weiteren Inhalt lesen.
    /// `decoded` trägt den vollständig entpackten Teil davor; der Scan
    /// durchsucht ihn und meldet den Rest als Lücke (`FINDINGS_002`).
    #[error("hinter dem Ende des Stroms liegen weitere Bytes")]
    TrailingBytes {
        /// Was bis zum Ende des Stroms entpackt wurde.
        decoded: Vec<u8>,
    },
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
    /// Gesetzt, wenn dieselben Bytes eine Lesart haben, die der Scan nicht
    /// geprüft hat (HUM-266).
    ///
    /// Nur bei `deflate`. `GetBody` liefert weiter `bytes` aus, denn genau
    /// darauf zeigen die Fundstellen des Scans; der Scan meldet die andere
    /// Lesart als Lücke (`FINDINGS_002`).
    pub ambiguity: Option<Ambiguity>,
}

/// Warum ein `deflate`-Body eine Lesart hat, die der Scan nicht geprüft hat
/// (HUM-266).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ambiguity {
    /// zlib-Rahmen (RFC 1950) und nackter Strom (RFC 1951) ergeben
    /// Verschiedenes. Durchsucht ist die zlib-Lesart.
    Readings,
    /// Der zlib-Kopf ist gültig, verlangt aber ein voreingestelltes Wörterbuch
    /// (`FDICT`, RFC 1950 §2.2), das der Scan nicht hat. Durchsucht ist die
    /// nackte Lesart; ein Empfänger mit dem Wörterbuch liest etwas anderes.
    PresetDictionary,
}

impl fmt::Debug for Inflated {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Inflated")
            .field("bytes", &Bytes(self.bytes.len()))
            .field("stopped", &self.stopped)
            .field("ambiguity", &self.ambiguity)
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
            ambiguity: None,
        }),
        // RFC 1952 erlaubt beliebig viele Member hintereinander, und `gzip -d`
        // oder Go `compress/gzip` auf der Gegenseite lesen sie alle. Deshalb
        // liest der Scan sie auch alle, durch ein einziges Budget (HUM-247).
        ContentEncoding::Gzip => read_limited(GzipMembers::new(raw), budget),
        // "deflate" ist im Netz zweideutig: die meisten Server schicken den
        // zlib-Rahmen aus RFC 1950, manche den nackten Strom aus RFC 1951.
        // Erst der Rahmen, dann der nackte Strom. Ein kaputter Rahmen führt
        // zum zweiten Versuch; ein sauberer Rahmen mit Bytes dahinter bleibt
        // eine Lücke und wird nicht anders gedeutet. Ist der Rahmen vollständig
        // gelesen, wird der nackte Strom zusätzlich gegen ihn verglichen:
        // Dieselben Bytes können beides sein und Verschiedenes sagen (HUM-266).
        // Ein gültiger Kopf mit `FDICT` scheitert hier immer, weil der Scan das
        // Wörterbuch nicht hat; die zlib-Lesart bleibt dann ungeprüft, auch
        // wenn der nackte Strom sauber endet.
        ContentEncoding::Deflate => {
            let before = *budget;
            let mut framed = before;
            match read_limited(ZlibDecoder::new(raw), &mut framed) {
                Err(InflateError::BrokenStream { .. }) => {
                    let mut naked = read_limited(DeflateDecoder::new(raw), budget);
                    if let Ok(inflated) = &mut naked
                        && !inflated.stopped
                        && wants_preset_dictionary(raw)
                    {
                        inflated.ambiguity = Some(Ambiguity::PresetDictionary);
                    }
                    naked
                }
                Ok(mut inflated) => {
                    *budget = framed;
                    if !inflated.stopped && raw_reading_differs(raw, before, &inflated.bytes) {
                        inflated.ambiguity = Some(Ambiguity::Readings);
                    }
                    Ok(inflated)
                }
                other => {
                    *budget = framed;
                    other
                }
            }
        }
        ContentEncoding::Brotli => {
            read_limited(brotli::Decompressor::new(raw, DECOMPRESS_CHUNK), budget)
        }
        ContentEncoding::Other(name) => Err(InflateError::UnsupportedEncoding(name.clone())),
    }
}

/// Ein Entpacker, der nach dem Ende seines Stroms sagt, ob dahinter noch
/// Eingabe liegt (HUM-247).
///
/// Die Prüfung entpackt nichts: Sie schaut nur, ob von der gepackten Eingabe
/// etwas übrig ist. Deshalb kann sie das [`Budget`] nicht umgehen.
trait StreamEnd: Read {
    /// Wahr, wenn hinter dem Ende des Stroms noch Bytes liegen.
    ///
    /// Gilt nur, nachdem `read` das Ende mit `0` gemeldet hat.
    fn has_trailing_input(&mut self) -> bool;
}

/// Liest gzip-Member nacheinander, bis die Eingabe zu Ende ist (HUM-247).
///
/// Dasselbe, was `flate2::bufread::MultiGzDecoder` tut, mit einem
/// Unterschied: Dieser Leser weiß, ob schon ein Member vollständig gelesen
/// war, als ein Fehler kam. Scheitert erst ein späteres Member (Bytes nach dem
/// letzten Member, die kein Member sind, oder ein kaputtes zweites Member),
/// ist der Anfang echt entpackt; er wird durchsucht, und der Rest wird zur
/// Lücke statt den ganzen Body unlesbar zu machen. `MultiGzDecoder` meldet in
/// diesem Fall nur den Fehler, und der Scan sähe gar nichts.
///
/// Alle Member laufen durch denselben Aufrufer von [`read_limited`] und damit
/// durch dasselbe [`Budget`].
struct GzipMembers<'a> {
    member: GzDecoder<&'a [u8]>,
    /// Wahr, sobald ein Member samt Prüfsumme vollständig gelesen ist.
    complete: bool,
    /// Wahr, wenn nach einem vollständigen Member etwas folgte, das sich nicht
    /// als Member lesen ließ.
    broken_tail: bool,
}

impl<'a> GzipMembers<'a> {
    fn new(raw: &'a [u8]) -> Self {
        Self {
            member: GzDecoder::new(raw),
            complete: false,
            broken_tail: false,
        }
    }
}

impl Read for GzipMembers<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() || self.broken_tail {
            return Ok(0);
        }
        loop {
            match self.member.read(buf) {
                // Das Member ist zu Ende und seine Prüfsumme stimmt. Jedes
                // Member verbraucht mindestens seinen Kopf, die Schleife endet
                // also mit der Eingabe.
                Ok(0) => {
                    self.complete = true;
                    let rest = *self.member.get_ref();
                    if rest.is_empty() {
                        return Ok(0);
                    }
                    self.member = GzDecoder::new(rest);
                }
                Ok(read) => return Ok(read),
                Err(_) if self.complete => {
                    self.broken_tail = true;
                    return Ok(0);
                }
                Err(error) => return Err(error),
            }
        }
    }
}

// Die `bufread`-Entpacker von flate2 verbrauchen von ihrer Eingabe genau die
// Bytes, die zum Strom gehören (`consume` mit `total_in`). Was nach dem Ende im
// Slice steht, liegt also hinter dem Strom. Beim gzip-Leser bleibt dort nach
// einem sauberen Ende nie etwas stehen; ein Rest, der kein Member ist, steht
// in `broken_tail`.
impl StreamEnd for GzipMembers<'_> {
    fn has_trailing_input(&mut self) -> bool {
        self.broken_tail || !self.member.get_ref().is_empty()
    }
}

impl StreamEnd for ZlibDecoder<&[u8]> {
    fn has_trailing_input(&mut self) -> bool {
        !self.get_ref().is_empty()
    }
}

impl StreamEnd for DeflateDecoder<&[u8]> {
    fn has_trailing_input(&mut self) -> bool {
        !self.get_ref().is_empty()
    }
}

// Der brotli-Leser puffert seine Eingabe selbst. Das erste `read` nach dem Ende
// liefert `0`, auch wenn im Puffer noch Bytes liegen (der Test
// `brotli_reader_ignores_trailing_bytes` hält das fest). Erst ein zweites
// `read` meldet einen Rest im Puffer als Fehler; was er noch nicht aus dem
// Slice geholt hat, steht in `get_ref`. Beides zusammen ist der Rest. Liefert
// das zweite `read` wider Erwarten Bytes, gilt auch das als Rest; sie werden
// nicht angehängt.
impl StreamEnd for brotli::Decompressor<&[u8]> {
    fn has_trailing_input(&mut self) -> bool {
        let mut probe = [0u8; 1];
        match self.read(&mut probe) {
            Ok(0) => !self.get_ref().is_empty(),
            Ok(_) | Err(_) => true,
        }
    }
}

/// Liest einen Strom, bis er endet oder das Budget zumacht.
///
/// Der eine Weg für alle Kodierungen: Die Ausgabe aller gzip-Member läuft
/// durch dasselbe Budget, und nach dem Ende eines Stroms prüft
/// [`StreamEnd::has_trailing_input`], ob dahinter noch etwas liegt.
fn read_limited<R: StreamEnd>(
    mut reader: R,
    budget: &mut Budget,
) -> Result<Inflated, InflateError> {
    let mut bytes = Vec::new();
    let mut chunk = [0u8; DECOMPRESS_CHUNK];
    loop {
        let left = budget.remaining();
        if left == 0 {
            // Ein Byte mehr lesen, um "genau voll" von "zu viel" zu trennen.
            let more = match reader.read(&mut chunk[..1]) {
                Ok(more) => more,
                Err(source) => return Err(broken(source, bytes)),
            };
            if more > 0 {
                return Ok(Inflated {
                    stopped: true,
                    bytes,
                    ambiguity: None,
                });
            }
            return finish(reader, bytes);
        }
        // Ein Byte über das Budget hinaus anfragen: kommt es, war der Strom
        // länger als erlaubt, und der Scan ist unvollständig.
        let want = DECOMPRESS_CHUNK.min(left.saturating_add(1));
        let read = match reader.read(&mut chunk[..want]) {
            Ok(read) => read,
            Err(source) => return Err(broken(source, bytes)),
        };
        if read == 0 {
            return finish(reader, bytes);
        }
        if read > left {
            bytes.extend_from_slice(&chunk[..left]);
            budget.take(left);
            return Ok(Inflated {
                bytes,
                stopped: true,
                ambiguity: None,
            });
        }
        bytes.extend_from_slice(&chunk[..read]);
        budget.take(read);
    }
}

/// Das Ende eines Stroms: vollständig, oder mit Bytes dahinter (HUM-247).
fn finish<R: StreamEnd>(mut reader: R, bytes: Vec<u8>) -> Result<Inflated, InflateError> {
    if reader.has_trailing_input() {
        return Err(InflateError::TrailingBytes { decoded: bytes });
    }
    Ok(Inflated {
        bytes,
        stopped: false,
        ambiguity: None,
    })
}

/// Wahr, wenn der nackte Strom aus RFC 1951 eine gültige Lesart von `raw` ist
/// und etwas anderes ergibt als die zlib-Lesart `framed` (HUM-266).
///
/// Der Kopf eines zlib-Rahmens mit `CM = 8` beginnt nackt gelesen immer einen
/// Stored-Block. Bei einem gewöhnlichen zlib-Body passen dessen `LEN` und
/// `NLEN` nicht zusammen, der nackte Strom bricht sofort ab, und es bleibt bei
/// der zlib-Lesart. Ein gezielt gebauter Body besteht beide Prüfungen und
/// trägt nackt gelesen einen anderen Inhalt; ein Empfänger, der nur den
/// nackten Strom kennt, bekäme dann etwas, das der Scan nie sah.
///
/// Die zweite Lesart bekommt ein eigenes Budget mit derselben Grenze wie die
/// erste (`budget` ist der Stand vor der zlib-Lesart). Ihre Ausgabe wird nicht
/// aufbewahrt, sondern stückweise mit `framed` verglichen und verworfen: Im
/// Speicher liegt nie mehr als ein Stück von [`DECOMPRESS_CHUNK`] Bytes
/// zusätzlich, und entpackt werden höchstens `allowance + 1` Bytes. Nach der
/// ersten Abweichung liest sie ohne Vergleich bis zum Ende weiter.
///
/// Ein Strom mit ungültigem Inhalt (falsches `NLEN`, unbekannter Blocktyp,
/// Abstand vor den Anfang) ist keine Lesart und lässt es bei der zlib-Lesart.
/// Zwei Enden zählen dagegen als Lesart, weil ein Empfänger, der stückweise
/// entpackt, bis dahin schon ausgeliefert hat: Bytes hinter dem Ende des
/// nackten Stroms (Python `zlib.decompressobj(-15)` legt sie in
/// `unused_data` ab) und eine Eingabe, die mitten im Strom aufhört
/// (`UnexpectedEof`; `decompressobj(-15)` liefert, was bis dahin entstand,
/// ohne Fehler). Im zweiten Fall zählt nur, was schon abweicht oder über die
/// zlib-Lesart hinausgeht; ein abgeschnittener Anfang derselben Bytes zeigt
/// nichts, was der Scan nicht sah.
///
/// Läuft die zweite Lesart über das Budget, gilt sie als verschieden: Die
/// zlib-Lesart hat vollständig ins Budget gepasst, der nackte Strom ist also
/// länger. Ob er danach noch bricht, sieht der Scan nicht, und was er nicht
/// sieht, meldet er (`backlog/CONVENTIONS.md` 4.13).
fn raw_reading_differs(raw: &[u8], mut budget: Budget, framed: &[u8]) -> bool {
    let mut reader = DeflateDecoder::new(raw);
    let mut chunk = [0u8; DECOMPRESS_CHUNK];
    let mut seen = 0usize;
    let mut differs = false;
    loop {
        let left = budget.remaining();
        let want = DECOMPRESS_CHUNK.min(left.saturating_add(1));
        let read = match reader.read(&mut chunk[..want]) {
            Ok(read) => read,
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
                return differs || seen > framed.len();
            }
            Err(_) => return false,
        };
        if read == 0 {
            return differs || seen != framed.len();
        }
        if read > left {
            return true;
        }
        if !differs {
            differs = framed.get(seen..seen + read) != Some(&chunk[..read]);
        }
        seen += read;
        budget.take(read);
    }
}

/// Wahr, wenn `raw` mit einem gültigen zlib-Kopf beginnt, der ein
/// voreingestelltes Wörterbuch verlangt (`FDICT`, RFC 1950 §2.2).
///
/// Gültig heißt: `CM = 8`, `CINFO` höchstens 7 und `FCHECK` passend. Der
/// Entpacker dieser Crate hat kein Wörterbuch und bricht an so einem Kopf ab;
/// ein Server, der das Wörterbuch kennt, liest den Rahmen trotzdem.
///
/// Beginnt der Rahmen hinter Kopf und Wörterbuch-Kennung mit dem reservierten
/// Blocktyp 3, ist er mit keinem Wörterbuch lesbar; dann gibt es keine zweite
/// Lesart und keine Lücke.
fn wants_preset_dictionary(raw: &[u8]) -> bool {
    let [cmf, flg, ..] = *raw else {
        return false;
    };
    let reserved_block = raw.get(6).is_some_and(|first| (first >> 1) & 0b11 == 0b11);
    cmf & 0x0f == 8
        && cmf >> 4 <= 7
        && (u16::from(cmf) << 8 | u16::from(flg)) % 31 == 0
        && flg & 0x20 != 0
        && !reserved_block
}

/// Der Abbruch eines Stroms samt dem, was bis dahin entstanden ist.
fn broken(source: std::io::Error, partial: Vec<u8>) -> InflateError {
    InflateError::BrokenStream { source, partial }
}

/// Der Befund [`LIMIT_003`] für einen Body, dessen Entpacken an
/// `limits.max_decompress_ratio` abgebrochen wurde (HUM-057).
///
/// Ohne `fix`: Ein Verhältnis über der Grenze ist das Merkmal einer
/// Entpack-Bombe, und der Rat, die Grenze anzuheben, wäre der falsche. Bis
/// HUM-057 stand hier `FINDINGS_002` mit genau diesem Rat.
fn ratio_exceeded(
    encoding: &ContentEncoding,
    packed: usize,
    produced: usize,
    max_ratio: u32,
) -> Diagnostic {
    Diagnostic::builder(LIMIT_003, Severity::Warning)
        .why(format!(
            "der Body ist mit \"{}\" kodiert und wäre beim Entpacken über das Verhältnis \
             {max_ratio}:1 gewachsen (limits.max_decompress_ratio); aus {packed} gepackten Bytes \
             wurden die ersten {produced} entpackt und durchsucht, dann brach das Entpacken ab",
            encoding.as_str(),
        ))
        .build()
}

/// Der Befund `FINDINGS_002` für einen Body, dessen Entpacken am Cap
/// `limits.preview_cap_bytes` abgebrochen wurde.
fn cap_exceeded(
    encoding: &ContentEncoding,
    packed: usize,
    produced: usize,
    cap_bytes: usize,
    max_ratio: u32,
) -> Diagnostic {
    Diagnostic::builder(FINDINGS_002, Severity::Warning)
        .why(format!(
            "der Body ist mit \"{}\" kodiert und beim Entpacken über das Budget \
             gelaufen; durchsucht wurden die ersten {produced} Bytes \
             (limits.preview_cap_bytes = {cap_bytes}, \
             limits.max_decompress_ratio = {max_ratio}, gepackt {packed} Bytes)",
            encoding.as_str(),
        ))
        // Hier zog das Cap und nicht das Verhältnis (das meldet `LIMIT_003`),
        // also zeigt der Rat auf das Cap.
        .fix(humanitl_core::FixAction::ChangeSetting {
            key: "limits.preview_cap_bytes".to_owned(),
            value: cap_bytes.saturating_mul(2).to_string(),
        })
        .build()
}

/// Der Befund `FINDINGS_002` für einen `deflate`-Body mit einer Lesart, die
/// der Scan nicht geprüft hat (HUM-266).
///
/// Ohne `fix`: Keine Einstellung macht aus zwei Lesarten eine, und das
/// Wörterbuch eines fremden Servers kennt der Scan nicht.
fn ambiguous_frame(ambiguity: Ambiguity, packed: usize, scanned: usize) -> Diagnostic {
    let why = match ambiguity {
        Ambiguity::Readings => format!(
            "der Body ist mit \"deflate\" kodiert und ergibt als zlib-Rahmen (RFC 1950) und als \
             nackter Strom (RFC 1951) Verschiedenes; durchsucht wurden die {scanned} Bytes der \
             zlib-Lesart, die nackte Lesart nicht (gepackt {packed} Bytes). Ein Empfänger, der \
             nur den nackten Strom liest, bekommt Inhalt, den der Scan nicht gesehen hat; ein \
             gewöhnlicher Server erzeugt so einen Body nicht"
        ),
        Ambiguity::PresetDictionary => format!(
            "der Body ist mit \"deflate\" kodiert und beginnt mit einem zlib-Kopf (RFC 1950), der \
             ein voreingestelltes Wörterbuch verlangt (FDICT); das Wörterbuch hat der Scan nicht. \
             Durchsucht wurden die {scanned} Bytes der Lesart als nackter Strom (RFC 1951), die \
             zlib-Lesart nicht (gepackt {packed} Bytes). Ein Empfänger mit dem Wörterbuch \
             bekommt Inhalt, den der Scan nicht gesehen hat"
        ),
    };
    Diagnostic::builder(FINDINGS_002, Severity::Warning)
        .why(why)
        .build()
}

/// Der Befund `FINDINGS_002` für Bytes hinter dem Ende eines Stroms (HUM-247).
///
/// Ohne `fix`: Kein Wert einer Einstellung macht den Rest lesbar, er gehört
/// zu keinem Strom, den der Scan kennt.
fn trailing_gap(encoding: &ContentEncoding, packed: usize, scanned: usize) -> Diagnostic {
    Diagnostic::builder(FINDINGS_002, Severity::Warning)
        .why(format!(
            "der Body ist mit \"{}\" kodiert, und hinter dem Ende des Stroms liegen weitere \
             Bytes; durchsucht wurden die {scanned} entpackten Bytes davor, der Rest nicht \
             (gepackt {packed} Bytes). Ein anderer Entpacker kann den Rest als weiteren Inhalt \
             lesen",
            encoding.as_str(),
        ))
        .build()
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

    use std::io::Read as _;

    use super::{
        Ambiguity, Budget, ContentEncoding, DECOMPRESS_CHUNK, DecodedBody, DeflateDecoder,
        InflateError, MIN_PRINTABLE_RUN, StreamEnd as _, ZlibDecoder, decode_body, inflate,
        percent_decode, printable_only, raw_reading_differs, read_limited,
    };

    fn gzip(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }

    fn zlib(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }

    fn raw_deflate(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }

    fn brotli(bytes: &[u8]) -> Vec<u8> {
        let mut packed = Vec::new();
        {
            let mut encoder = brotli::CompressorWriter::new(&mut packed, 4096, 5, 22);
            encoder.write_all(bytes).unwrap();
        }
        packed
    }

    fn code(decoded: &DecodedBody) -> Option<&str> {
        decoded.diagnostic.as_ref().map(|found| found.code.as_str())
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
        // Ein Strom, der genau endet, ist vollständig: Die Prüfung auf Bytes
        // hinter dem Ende meldet hier nichts (HUM-247).
        for packed in [zlib(plain), raw_deflate(plain)] {
            let decoded = decode_body(&packed, &ContentEncoding::Deflate, 4096, 100);
            assert_eq!(decoded.bytes, plain);
            assert!(!decoded.truncated);
            assert!(decoded.diagnostic.is_none());
        }
    }

    /// Ein `deflate`-Body mit zwei gültigen Lesarten (HUM-266), dieselbe Datei
    /// wie im Corpus von `body_decoder`; das erste Byte wählt dort `deflate`.
    ///
    /// Der Kopf `08 1d` ist ein gültiger zlib-Kopf. Als zlib gelesen folgt ein
    /// Stored-Block mit `LEN` 65250 (`01 e2 fe 1d 01`) samt passender
    /// Adler-32-Summe; der Inhalt besteht fast nur aus Nullbytes. Nackt
    /// gelesen ist `08` selbst ein nicht letzter Stored-Block mit `LEN`
    /// `0x011d` (`1d 01 e2 fe`), dahinter ein letzter Block mit festen
    /// Huffman-Codes, der `password=hum-266-polyglot-secret` trägt. Der Rest
    /// liegt hinter dem Ende des nackten Stroms.
    fn polyglot() -> &'static [u8] {
        let file = include_bytes!("../tests/regressions/body_decoder-deflate-polyglot.bin");
        &file[1..]
    }

    const POLYGLOT_SECRET: &[u8] = b"password=hum-266-polyglot-secret";

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }

    #[test]
    fn the_polyglot_has_two_valid_readings() {
        // Die Datei ist, was sie sein soll: Beide Lesarten enden sauber, und
        // nur die nackte trägt das Geheimnis. Ohne diese Probe könnte der
        // folgende Test auf einer kaputten Datei grün sein.
        let mut framed = Vec::new();
        ZlibDecoder::new(polyglot())
            .read_to_end(&mut framed)
            .unwrap();
        assert_eq!(framed.len(), 65_250);
        assert!(!contains(&framed, POLYGLOT_SECRET));
        let mut naked = Vec::new();
        let mut reader = DeflateDecoder::new(polyglot());
        reader.read_to_end(&mut naked).unwrap();
        assert!(contains(&naked, POLYGLOT_SECRET));
        assert!(reader.has_trailing_input());
    }

    #[test]
    fn a_deflate_polyglot_is_reported_and_not_called_complete() {
        let decoded = decode_body(polyglot(), &ContentEncoding::Deflate, 1 << 20, 100);
        assert!(decoded.truncated);
        assert_eq!(code(&decoded), Some("FINDINGS_002"));
        let why = decoded.diagnostic.unwrap().why;
        assert!(why.contains("RFC 1951"), "{why}");
        // Durchsucht wird die zlib-Lesart, auf die auch `GetBody` zeigt.
        assert_eq!(decoded.bytes.len(), 65_250);
        assert!(!contains(&decoded.bytes, POLYGLOT_SECRET));
        let mut budget = Budget::new(polyglot().len(), 1 << 20, 100);
        let inflated = inflate(polyglot(), &ContentEncoding::Deflate, &mut budget).unwrap();
        assert_eq!(inflated.ambiguity, Some(Ambiguity::Readings));
        assert_eq!(inflated.bytes, decoded.bytes);
    }

    /// Wie [`polyglot`], aber der nackte Strom hört nach dem Geheimnis mitten
    /// in einem Stored-Block auf (`UnexpectedEof`). Python
    /// `zlib.decompressobj(-15)` liefert das Geheimnis trotzdem.
    fn cut_polyglot() -> &'static [u8] {
        let file =
            include_bytes!("../tests/regressions/body_decoder-deflate-truncated-divergence.bin");
        &file[1..]
    }

    #[test]
    fn a_naked_reading_that_diverges_and_then_stops_short_is_a_reading() {
        let secret = b"password=hum-266-truncated-secret";
        let mut naked = Vec::new();
        let error = DeflateDecoder::new(cut_polyglot())
            .read_to_end(&mut naked)
            .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::UnexpectedEof);
        let decoded = decode_body(cut_polyglot(), &ContentEncoding::Deflate, 1 << 20, 100);
        assert!(!contains(&decoded.bytes, secret));
        assert!(decoded.truncated);
        assert_eq!(code(&decoded), Some("FINDINGS_002"));
        assert!(decoded.diagnostic.unwrap().why.contains("RFC 1951"));
    }

    /// Ein gültiger zlib-Kopf `08 3c` mit `FDICT` und der Wörterbuch-Kennung
    /// `ff c3 00 03`. Mit dem passenden Wörterbuch ergibt der Rahmen
    /// `password=hum-266-dictionary-secret`; nackt gelesen ist derselbe Body
    /// ein Stored-Block von 65340 Bytes ohne das Geheimnis und ein leerer
    /// letzter Block, sauber zu Ende.
    fn dictionary_body() -> &'static [u8] {
        let file =
            include_bytes!("../tests/regressions/body_decoder-deflate-preset-dictionary.bin");
        &file[1..]
    }

    #[test]
    fn a_zlib_header_with_a_preset_dictionary_is_a_gap() {
        assert!(super::wants_preset_dictionary(dictionary_body()));
        let mut naked = Vec::new();
        DeflateDecoder::new(dictionary_body())
            .read_to_end(&mut naked)
            .unwrap();
        assert_eq!(naked.len(), 65_340);
        let decoded = decode_body(dictionary_body(), &ContentEncoding::Deflate, 1 << 20, 100);
        assert_eq!(decoded.bytes, naked);
        assert!(decoded.truncated);
        assert_eq!(code(&decoded), Some("FINDINGS_002"));
        assert!(decoded.diagnostic.unwrap().why.contains("FDICT"));
        // Ohne gesetztes `FDICT` oder mit falscher Prüfsumme ist es kein
        // zlib-Kopf mit Wörterbuch; dann gilt der nackte Strom allein.
        assert!(!super::wants_preset_dictionary(&[0x08, 0x1d]));
        assert!(!super::wants_preset_dictionary(&[0x08, 0x3d]));
        // Codex-Beispiel: Kopf mit Wörterbuch, aber der Rahmen beginnt mit dem
        // reservierten Blocktyp; nackt ist der Body vollständig lesbar.
        let mut reserved = vec![0x08, 0x3c, 0x00, 0xc3, 0xff, 0x00, 0x06];
        reserved.extend([0u8; 58]);
        reserved.extend([0x03, 0x00]);
        assert!(!super::wants_preset_dictionary(&reserved));
        let decoded = decode_body(&reserved, &ContentEncoding::Deflate, 1 << 20, 100);
        assert!(!decoded.truncated);
        assert_eq!(code(&decoded), None);
        assert!(!super::wants_preset_dictionary(&[0x08]));
        let plain = b"ein gewoehnlicher nackter Strom";
        let decoded = decode_body(&raw_deflate(plain), &ContentEncoding::Deflate, 4096, 100);
        assert!(!decoded.truncated);
    }

    #[test]
    fn ordinary_zlib_and_raw_deflate_have_one_reading() {
        // Alle Stufen, auch `none`: Dort ist der zlib-Inhalt selbst ein
        // Stored-Block, der nackt gelesen am ehesten durchrutschen könnte.
        let plain: Vec<u8> = (0..20_000u32)
            .flat_map(|index| format!("Zeile {index} mit etwas Text\n").into_bytes())
            .collect();
        for level in [0, 1, 6, 9] {
            let level = Compression::new(level);
            let mut framed = ZlibEncoder::new(Vec::new(), level);
            framed.write_all(&plain).unwrap();
            let mut naked = DeflateEncoder::new(Vec::new(), level);
            naked.write_all(&plain).unwrap();
            for packed in [framed.finish().unwrap(), naked.finish().unwrap()] {
                let decoded = decode_body(&packed, &ContentEncoding::Deflate, 1 << 21, 100);
                assert_eq!(decoded.bytes, plain, "{level:?}");
                assert!(!decoded.truncated, "{level:?}");
                assert!(decoded.diagnostic.is_none(), "{level:?}");
            }
        }
    }

    #[test]
    fn the_second_reading_stays_inside_the_same_budget() {
        let plain = b"derselbe Inhalt in beiden Lesarten";
        let naked = raw_deflate(plain);
        // Gleiche Ausgabe, sauberes Ende: eine Lesart.
        assert!(!raw_reading_differs(
            &naked,
            Budget::new(naked.len(), 4096, 100),
            plain
        ));
        // Ein nackter Strom, der bricht, lässt es bei der zlib-Lesart.
        assert!(!raw_reading_differs(
            &naked[..naked.len() / 2],
            Budget::new(naked.len(), 4096, 100),
            plain
        ));
        // Eine Abweichung im Inhalt oder in der Länge ist eine zweite Lesart.
        assert!(raw_reading_differs(
            &naked,
            Budget::new(naked.len(), 4096, 100),
            b"derselbe Inhalt in beiden LesartEN"
        ));
        assert!(raw_reading_differs(
            &naked,
            Budget::new(naked.len(), 4096, 100),
            &plain[..10]
        ));
        // Die zweite Lesart entpackt nicht über das Budget hinaus: Schon das
        // erste Byte über `allowance` beendet sie, und ein Strom, der weiter
        // reicht als die vollständige zlib-Lesart, ist eine andere Lesart.
        let budget = Budget::new(naked.len(), plain.len() - 1, 100);
        assert!(raw_reading_differs(
            &naked,
            budget,
            &plain[..plain.len() - 1]
        ));
        // Mit einem Budget, an dem schon die zlib-Lesart endet, gibt es keine
        // zweite Lesart: Der Body ist ohnehin als gekürzt gemeldet.
        let decoded = decode_body(polyglot(), &ContentEncoding::Deflate, 1024, 100);
        assert!(decoded.truncated);
        assert_eq!(decoded.bytes.len(), 1024);
        assert!(decoded.diagnostic.unwrap().why.contains("Budget"));
    }

    #[test]
    fn a_brotli_body_is_decoded_whole() {
        let plain = b"Kontakt: vorname.nachname@kunde.de, bitte antworten";
        let decoded = decode_body(&brotli(plain), &ContentEncoding::Brotli, 4096, 100);
        assert_eq!(decoded.bytes, plain);
        assert!(!decoded.truncated);
        assert!(decoded.diagnostic.is_none());
    }

    #[test]
    fn gzip_members_share_the_budget() {
        // Jedes Member für sich bleibt unter dem Budget, beide zusammen
        // nicht. Mit einem Budget je Member käme der Body vollständig durch.
        let len = 64 * 1024;
        let member = gzip(&vec![0u8; len]);
        let body = [member.clone(), member].concat();

        // Das Verhältnis ist die engere Grenze, irgendwo zwischen einem und
        // zwei Membern Ausgabe. Seit HUM-057 meldet das `LIMIT_003`.
        let ratio = u32::try_from(len * 3 / 2 / body.len()).unwrap();
        let allowance = body.len() * usize::try_from(ratio).unwrap();
        assert!(len < allowance && allowance < 2 * len, "{allowance}");
        let decoded = decode_body(&body, &ContentEncoding::Gzip, 8 * 1024 * 1024, ratio);
        assert!(decoded.truncated);
        assert_eq!(decoded.bytes.len(), allowance);
        assert_eq!(code(&decoded), Some("LIMIT_003"));

        // Das Cap ist die engere Grenze: `FINDINGS_002`.
        let cap = len * 3 / 2;
        let decoded = decode_body(&body, &ContentEncoding::Gzip, cap, 1_000_000);
        assert!(decoded.truncated);
        assert_eq!(decoded.bytes.len(), cap);
        assert_eq!(code(&decoded), Some("FINDINGS_002"));

        // Gegenprobe: Mit genug Budget kommen beide Member vollständig durch.
        let decoded = decode_body(&body, &ContentEncoding::Gzip, 2 * len, 1_000_000);
        assert!(!decoded.truncated);
        assert_eq!(decoded.bytes.len(), 2 * len);
    }

    #[test]
    fn gzip_garbage_after_the_last_member_is_a_gap() {
        // Bytes nach einem Member, die kein weiteres Member sind, liegen nicht
        // still, und sie machen das vollständige Member davor nicht unlesbar:
        // Es wird durchsucht, der Rest wird zur Lücke.
        let plain = b"iban GB82 WEST 1234 5698 7654 32";
        let mut broken_member = gzip(b"zweites Member");
        broken_member.truncate(broken_member.len() - 3);
        for tail in [&b"x"[..], &[0x1f][..], &[0u8; 4][..], &broken_member[..]] {
            let body = [gzip(plain), tail.to_vec()].concat();
            let decoded = decode_body(&body, &ContentEncoding::Gzip, 4096, 100);
            assert!(decoded.truncated, "{tail:?}");
            assert_eq!(code(&decoded), Some("FINDINGS_002"), "{tail:?}");
            assert!(decoded.bytes.starts_with(plain), "{tail:?}");
            let why = &decoded.diagnostic.as_ref().unwrap().why;
            assert!(why.contains("hinter dem Ende des Stroms"), "{why}");
        }
    }

    #[test]
    fn trailing_bytes_after_deflate_are_a_gap() {
        let plain = b"AKIAIOSFODNN7EXAMPLE und dahinter noch etwas";
        let hidden = zlib(b"versteckt");
        for packed in [zlib(plain), raw_deflate(plain)] {
            for tail in [&b"x"[..], &[0u8; 4][..], &hidden[..]] {
                let body = [packed.clone(), tail.to_vec()].concat();
                let decoded = decode_body(&body, &ContentEncoding::Deflate, 4096, 100);
                assert!(decoded.truncated, "{tail:?}");
                assert_eq!(code(&decoded), Some("FINDINGS_002"), "{tail:?}");
                // Was vor dem Ende stand, ist entpackt und wird durchsucht.
                assert_eq!(decoded.bytes, plain);
                let why = &decoded.diagnostic.as_ref().unwrap().why;
                assert!(why.contains("hinter dem Ende des Stroms"), "{why}");
            }
        }
    }

    #[test]
    fn brotli_reader_ignores_trailing_bytes() {
        // Festgestellt für HUM-247: `brotli::Decompressor` meldet das Ende
        // eines Stroms mit `Ok(0)`, auch wenn dahinter noch Bytes liegen. Wer
        // nur bis zum ersten `0` liest, bekommt einen still vollständigen Body.
        let plain = b"Kontakt: vorname.nachname@kunde.de";
        let body = [brotli(plain), b"und noch etwas".to_vec()].concat();
        let mut reader = brotli::Decompressor::new(&body[..], DECOMPRESS_CHUNK);
        let mut out = Vec::new();
        reader.read_to_end(&mut out).unwrap();
        assert_eq!(out, plain);
    }

    #[test]
    fn trailing_bytes_after_br_are_a_gap() {
        let plain = b"Kontakt: vorname.nachname@kunde.de, bitte antworten";
        let packed = brotli(plain);
        for tail in [
            b"x".to_vec(),
            brotli(b"versteckt"),
            // Länger als der Puffer des Lesers: Ein Teil bleibt im Slice.
            vec![b'y'; 2 * DECOMPRESS_CHUNK],
        ] {
            let body = [packed.clone(), tail].concat();
            let decoded = decode_body(&body, &ContentEncoding::Brotli, 4096, 100);
            assert!(decoded.truncated);
            assert_eq!(code(&decoded), Some("FINDINGS_002"));
            assert_eq!(decoded.bytes, plain);
        }
    }

    #[test]
    fn trailing_bytes_behind_the_brotli_buffer_are_a_gap() {
        // Der Puffer des Lesers ist genau so groß wie der Strom: Nach dem Ende
        // ist der Puffer leer, und der Rest steht nur noch im Slice.
        let plain = b"Kontakt: vorname.nachname@kunde.de, bitte antworten";
        let packed = brotli(plain);
        let body = [packed.clone(), b"rest".to_vec()].concat();

        let reader = brotli::Decompressor::new(&body[..], packed.len());
        let mut budget = Budget::new(body.len(), 4096, 100);
        match read_limited(reader, &mut budget) {
            Err(InflateError::TrailingBytes { decoded }) => assert_eq!(decoded, plain),
            Err(other) => panic!("expected trailing bytes, got {other}"),
            Ok(inflated) => panic!("expected trailing bytes, got {inflated:?}"),
        }

        let reader = brotli::Decompressor::new(&packed[..], packed.len());
        let mut budget = Budget::new(packed.len(), 4096, 100);
        let inflated = read_limited(reader, &mut budget).unwrap();
        assert_eq!(inflated.bytes, plain);
        assert!(!inflated.stopped);
    }

    #[test]
    fn trailing_bytes_are_seen_at_an_exactly_full_budget() {
        // Der Strom füllt das Budget genau. Ohne Rest ist der Body vollständig,
        // mit Rest eine Lücke; die Prüfung entpackt dabei kein Byte mehr.
        let plain = b"genau so lang wie das Cap, kein Byte mehr";
        let cap = plain.len();
        let encodings = [
            (ContentEncoding::Gzip, gzip(plain)),
            (ContentEncoding::Deflate, zlib(plain)),
            (ContentEncoding::Deflate, raw_deflate(plain)),
            (ContentEncoding::Brotli, brotli(plain)),
        ];
        for (encoding, packed) in encodings {
            let decoded = decode_body(&packed, &encoding, cap, 1_000_000);
            assert!(!decoded.truncated, "{encoding:?}");
            assert!(decoded.diagnostic.is_none(), "{encoding:?}");
            assert_eq!(decoded.bytes, plain);

            let body = [packed, b"rest".to_vec()].concat();
            let decoded = decode_body(&body, &encoding, cap, 1_000_000);
            assert!(decoded.truncated, "{encoding:?}");
            assert_eq!(code(&decoded), Some("FINDINGS_002"), "{encoding:?}");
            assert!(decoded.bytes.len() <= cap);
        }
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
        // Das Verhältnis zog, nicht das Cap: `LIMIT_003`, mit dem Verhältnis im
        // Text und ohne den Rat, die Grenze anzuheben (HUM-057).
        let diagnostic = decoded.diagnostic.unwrap();
        assert_eq!(diagnostic.code.as_str(), "LIMIT_003");
        assert!(diagnostic.why.contains("100:1"), "{}", diagnostic.why);
        assert!(diagnostic.why.contains("max_decompress_ratio"));
        assert!(diagnostic.fix.is_none());
    }

    #[test]
    fn a_bomb_stopped_by_the_cap_keeps_findings_002_with_the_cap_as_fix() {
        // Dasselbe Archiv, aber das Cap ist enger als das Verhältnis: Das ist
        // keine Aussage über eine Bombe, nur über die Vorschau-Grenze.
        let bomb = gzip(&vec![0u8; 1024 * 1024]);
        let decoded = decode_body(&bomb, &ContentEncoding::Gzip, 4096, 100);
        assert!(decoded.truncated);
        let diagnostic = decoded.diagnostic.unwrap();
        assert_eq!(diagnostic.code.as_str(), "FINDINGS_002");
        assert_eq!(
            diagnostic.fix,
            Some(humanitl_core::FixAction::ChangeSetting {
                key: "limits.preview_cap_bytes".to_owned(),
                value: "8192".to_owned(),
            })
        );
    }

    #[test]
    fn the_budget_knows_which_limit_is_the_narrower_one() {
        assert!(Budget::new(1024, 1024 * 1024, 100).limited_by_ratio());
        assert!(!Budget::new(1024, 4096, 100).limited_by_ratio());
        // Gleichstand: das Cap.
        assert!(!Budget::new(1024, 102_400, 100).limited_by_ratio());
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
