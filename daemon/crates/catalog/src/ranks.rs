//! Die gebündelte Rangliste der Verbreitung: laden und nachschlagen.
//!
//! Ausgeliefert wird ein Ausschnitt der Majestic Million (CC BY 3.0). Der Rang
//! misst dort, von wie vielen verschiedenen Netzen aus auf eine Domain
//! verwiesen wird.
//!
//! Der Rang ist Reichweite, kein Urteil. Ein niedriger Rang macht eine Anfrage
//! nicht sicher, ein fehlender macht sie nicht verdächtig; verbreitete Dienste
//! tragen Schadsoftware aus, und der interne Dienst einer Firma steht in keiner
//! öffentlichen Liste. Die Oberfläche schreibt das an die Zahl, und dieser Text
//! hält es fest, damit niemand den Rang später als Bewertung liest.
//!
//! Die Liste liegt als `catalog/ranks-top100k.csv.gz` neben dem Katalog;
//! Herkunft, Datum, Prüfsummen und Lizenz stehen in `catalog/RANKS-LICENSE`.
//! Zur Laufzeit wird nichts geholt (ADR-006); eine neue Liste ist ein
//! `chore`-Commit pro Release.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use flate2::read::GzDecoder;
use humanitl_core::diagnostics::codes::CATALOG_002;
use humanitl_core::{Diagnostic, Severity};

/// Höchste entpackte Größe, die beim Laden akzeptiert wird.
///
/// Die Datei gehört zur Auslieferung, aber sie liegt als Gzip auf der Platte,
/// und eine Datei auf der Platte kann ausgetauscht werden. Ohne Grenze wäre ein
/// paar Kilobyte großer Strom genug, um den Daemon beim Start den Speicher
/// auffressen zu lassen. 16 MiB sind gut das Achtfache der heutigen Liste.
pub const MAX_DECOMPRESSED_BYTES: u64 = 16 * 1024 * 1024;

/// Höchste Zeilenzahl, die beim Laden akzeptiert wird.
pub const MAX_LINES: usize = 1_000_000;

/// Wie viel Speicher die geladene Liste belegt.
///
/// Gemessen, nicht geschätzt: Der Resident-Set des Testprozesses wächst beim
/// Laden der ausgelieferten Datei (100 000 Einträge) um 6 848 512 Byte, also
/// 6,53 MiB, dreimal hintereinander mit demselben Wert. Davon sind 3 280 896
/// Byte das Bucket-Array der Hashtabelle und rund 3,23 MB die 100 000 einzelnen
/// Schlüssel; die Differenz zur reinen Summe der Namen (1 325 131 Byte) ist der
/// Verwaltungsaufwand von `malloc` je Block.
///
/// Die Zahl steht hier, weil sie beim Start jedes Daemons anfällt und ein
/// Wachstum der Liste sie unmittelbar erhöht. Gemessen wurde über
/// `/proc/self/statm` vor und nach [`Ranks::load`].
pub const MEASURED_RESIDENT_BYTES: u64 = 6_848_512;

/// Ränge, nach Apex nachschlagbar.
///
/// Der Schlüssel ist der Name, wie er in der Liste steht: klein geschrieben,
/// ohne abschließenden Punkt. Nachgeschlagen wird der Apex aus [`crate::psl`],
/// nie der volle Host: die Liste führt registrierbare Domains, und
/// `api.github.com` stünde dort nie.
#[derive(Debug, Clone, Default)]
pub struct Ranks {
    map: HashMap<Box<str>, u32>,
}

impl Ranks {
    /// Eine leere Rangliste.
    ///
    /// Damit läuft der Daemon weiter, wenn die Datei fehlt: jeder Rang ist dann
    /// unbekannt, und unbekannt steht als unbekannt da.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Liest die Rangliste aus einer Datei.
    ///
    /// Ein Gzip-Strom wird am Magic (`1f 8b`) erkannt und entpackt; eine
    /// unverpackte CSV-Datei wird genauso gelesen. So kann ein Test dieselbe
    /// Funktion mit einer Handvoll Zeilen aufrufen wie der Daemon mit der
    /// ausgelieferten Datei.
    ///
    /// # Errors
    ///
    /// [`CATALOG_002`], wenn die Datei fehlt, nicht lesbar ist, der Gzip-Strom
    /// beschädigt ist, eine Zeile nicht die Form `rang,domain` hat oder die
    /// Grenzen aus [`MAX_DECOMPRESSED_BYTES`] und [`MAX_LINES`] überschritten
    /// werden.
    pub fn load(path: &Path) -> Result<Self, Diagnostic> {
        let file = File::open(path).map_err(|err| {
            Diagnostic::builder(CATALOG_002, Severity::Warning)
                .why(format!(
                    "the ranking list {} could not be opened: {err}",
                    path.display()
                ))
                .build()
        })?;
        let mut head = [0_u8; 2];
        let mut reader = BufReader::new(file);
        let read = fill(&mut reader, &mut head).map_err(|err| Self::read_error(path, &err))?;
        let stream: Box<dyn Read> = if read == 2 && head == [0x1f, 0x8b] {
            Box::new(GzDecoder::new(Chained::new(head, reader)))
        } else {
            Box::new(Chained::new_partial(head, read, reader))
        };
        Self::parse(BufReader::new(stream.take(MAX_DECOMPRESSED_BYTES + 1)))
            .map_err(|err| Self::wrap(path, &err))
    }

    /// Liest die Rangliste aus einem entpackten Strom.
    ///
    /// # Errors
    ///
    /// [`CATALOG_002`] mit Zeilennummer, wenn eine Zeile nicht die Form
    /// `rang,domain` hat oder eine der Grenzen überschritten wird.
    pub fn parse(reader: impl BufRead) -> Result<Self, Diagnostic> {
        let mut map: HashMap<Box<str>, u32> = HashMap::new();
        let mut bytes: u64 = 0;
        for (index, line) in reader.lines().enumerate() {
            let number = index + 1;
            if number > MAX_LINES {
                return Err(Self::limit_error(format!(
                    "the ranking list has more than {MAX_LINES} lines"
                )));
            }
            let line = line.map_err(|err| Self::limit_error(format!("line {number}: {err}")))?;
            bytes += line.len() as u64 + 1;
            if bytes > MAX_DECOMPRESSED_BYTES {
                return Err(Self::limit_error(format!(
                    "the ranking list is larger than {MAX_DECOMPRESSED_BYTES} bytes once unpacked"
                )));
            }
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Some((rank, domain)) = line.split_once(',') else {
                return Err(Self::limit_error(format!(
                    "line {number} is not `rank,domain`: {line:?}"
                )));
            };
            let Ok(rank) = rank.trim().parse::<u32>() else {
                return Err(Self::limit_error(format!(
                    "line {number} has no rank: {line:?}"
                )));
            };
            let domain = domain.trim().trim_end_matches('.').to_ascii_lowercase();
            if domain.is_empty() {
                return Err(Self::limit_error(format!(
                    "line {number} has no domain: {line:?}"
                )));
            }
            // Kommt ein Name zweimal vor, gilt der bessere Rang. Zwei Ränge für
            // einen Namen wären sonst eine Frage der Lesereihenfolge.
            map.entry(domain.into_boxed_str())
                .and_modify(|known| *known = (*known).min(rank))
                .or_insert(rank);
        }
        Ok(Self { map })
    }

    /// Der Rang eines Apex, oder `None`.
    ///
    /// `None` heißt „steht nicht in den vorderen Rängen dieser Liste", nicht
    /// „unbekannter Betreiber" und schon gar nicht „verdächtig".
    #[must_use]
    pub fn rank(&self, apex: &str) -> Option<u32> {
        self.map.get(apex).copied()
    }

    /// Wie viele Namen die Liste kennt.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Wahr, wenn keine Liste geladen ist.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    fn read_error(path: &Path, err: &std::io::Error) -> Diagnostic {
        Diagnostic::builder(CATALOG_002, Severity::Warning)
            .why(format!(
                "the ranking list {} could not be read: {err}",
                path.display()
            ))
            .build()
    }

    fn limit_error(why: String) -> Diagnostic {
        Diagnostic::builder(CATALOG_002, Severity::Warning)
            .why(why)
            .build()
    }

    fn wrap(path: &Path, err: &Diagnostic) -> Diagnostic {
        Diagnostic::builder(CATALOG_002, err.severity)
            .why(format!("{}: {}", path.display(), err.why))
            .build()
    }
}

/// Liest so viele Bytes wie möglich in den Puffer.
fn fill(reader: &mut impl Read, buf: &mut [u8]) -> Result<usize, std::io::Error> {
    let mut filled = 0;
    while filled < buf.len() {
        match reader.read(&mut buf[filled..])? {
            0 => break,
            n => filled += n,
        }
    }
    Ok(filled)
}

/// Ein Strom, dem die schon gelesenen Magic-Bytes wieder vorangestellt sind.
struct Chained<R> {
    head: [u8; 2],
    len: usize,
    at: usize,
    rest: R,
}

impl<R: Read> Chained<R> {
    fn new(head: [u8; 2], rest: R) -> Self {
        Self {
            head,
            len: 2,
            at: 0,
            rest,
        }
    }

    fn new_partial(head: [u8; 2], len: usize, rest: R) -> Self {
        Self {
            head,
            len,
            at: 0,
            rest,
        }
    }
}

impl<R: Read> Read for Chained<R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        if self.at < self.len {
            let take = (self.len - self.at).min(buf.len());
            buf[..take].copy_from_slice(&self.head[self.at..self.at + take]);
            self.at += take;
            return Ok(take);
        }
        self.rest.read(buf)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::io::Cursor;

    use super::Ranks;

    fn ranks(text: &str) -> Ranks {
        Ranks::parse(Cursor::new(text.to_owned())).unwrap()
    }

    #[test]
    fn a_plain_list_is_read() {
        let ranks = ranks("1,google.com\n2,cloudflare.com\n42,github.com\n");
        assert_eq!(ranks.len(), 3);
        assert_eq!(ranks.rank("github.com"), Some(42));
        assert_eq!(ranks.rank("example.invalid"), None);
    }

    #[test]
    fn names_are_lowercased_and_trimmed() {
        let ranks = ranks("7,GitHub.COM.\n");
        assert_eq!(ranks.rank("github.com"), Some(7));
    }

    #[test]
    fn a_duplicate_keeps_the_better_rank() {
        let ranks = ranks("9,github.com\n3,github.com\n");
        assert_eq!(ranks.rank("github.com"), Some(3));
    }

    #[test]
    fn a_malformed_line_is_a_finding_with_its_number() {
        let err = Ranks::parse(Cursor::new("1,google.com\nnonsense\n".to_owned())).unwrap_err();
        assert_eq!(err.code.as_str(), "CATALOG_002");
        assert!(err.why.contains("line 2"), "{}", err.why);
    }

    #[test]
    fn an_empty_list_answers_none() {
        let ranks = Ranks::empty();
        assert!(ranks.is_empty());
        assert_eq!(ranks.rank("github.com"), None);
    }
}
