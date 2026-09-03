//! Zerlegung einer Anfrage in Suchziele.
//!
//! Ein Suchziel ist ein Stück Bytes mit einem Ort: jeder Header einzeln, die
//! Query roh, die Query prozent-dekodiert und der Body. Kein Header ist
//! ausgenommen; gerade `authorization` und `cookie` sind der Grund, warum es
//! Detektoren gibt.
//!
//! Die dekodierte Query trägt eine [`SpanMap`], die einen Bereich der Kopie auf
//! den Rohtext zurückrechnet. Nach außen zeigt ein Fund damit immer auf das,
//! was wirklich auf der Leitung stand.

use core::fmt;
use core::ops::Range;

use humanitl_core::http::{HeaderName, HttpRequest};
use humanitl_core::{Diagnostic, FindingLocation};

use crate::content_type::ContentType;
use crate::decode::{
    Bytes, ContentEncoding, MIN_PRINTABLE_RUN, decode_body, percent_decode, printable_only,
};

/// Was ein Detektor zu sehen bekommt.
///
/// Ohne abgeleitetes `Debug`: Die Bytes sind der Inhalt der Anfrage, und ein
/// abgeleitetes `Debug` würde sie als Zahlenliste ausgeben. Ein Geheimnis
/// verlässt diese Crate auch nicht als Zahlenreihe. Gedruckt werden Ort, Länge
/// und Typ.
#[derive(Clone)]
pub struct ScanInput<'a> {
    /// Wo diese Bytes in der Anfrage stehen.
    pub location: FindingLocation,
    /// Die Bytes selbst; alle Bereiche sind relativ dazu.
    pub bytes: &'a [u8],
    /// Der `Content-Type` des Bodys, sonst `None`.
    pub content_type: Option<&'a ContentType>,
}

impl fmt::Debug for ScanInput<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScanInput")
            .field("location", &self.location.to_string())
            .field("bytes", &Bytes(self.bytes.len()))
            .field("content_type", &self.content_type.map(ContentType::essence))
            .finish()
    }
}

/// Wie ein Bereich des Suchziels auf den Rohtext zurückzeigt.
#[derive(Clone, PartialEq, Eq)]
pub enum SpanMap {
    /// Die Bytes sind das Original; der Bereich gilt unverändert.
    Identity,
    /// Die Bytes sind eine Kopie; die Tabelle hält je Byte den Rohindex und am
    /// Ende die Länge des Rohtexts.
    Table(Vec<usize>),
}

impl fmt::Debug for SpanMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identity => f.write_str("Identity"),
            Self::Table(table) => write!(f, "Table({} Stellen)", table.len()),
        }
    }
}

impl SpanMap {
    /// Rechnet einen Bereich der Kopie auf den Rohtext zurück.
    ///
    /// `None`, wenn der Bereich nicht in die Tabelle passt; das kann nur ein
    /// Fehler in einem Detektor sein und wird verworfen, statt einen Bereich zu
    /// liefern, der ins Leere zeigt.
    #[must_use]
    pub fn map(&self, span: Range<usize>) -> Option<Range<usize>> {
        match self {
            Self::Identity => Some(span),
            Self::Table(table) => {
                let start = *table.get(span.start)?;
                let end = *table.get(span.end)?;
                (start <= end).then_some(start..end)
            }
        }
    }
}

/// Ein Suchziel mit seiner Rückrechnung.
#[derive(Clone)]
pub struct ScanTarget<'a> {
    /// Ort und Bytes.
    pub input: ScanInput<'a>,
    /// Die Rückrechnung der Bereiche.
    pub map: &'a SpanMap,
}

impl fmt::Debug for ScanTarget<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScanTarget")
            .field("input", &self.input)
            .field("map", self.map)
            .finish()
    }
}

#[derive(Clone)]
struct Entry {
    location: FindingLocation,
    bytes: Vec<u8>,
    map: SpanMap,
    content_type: Option<ContentType>,
}

impl fmt::Debug for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Entry")
            .field("location", &self.location.to_string())
            .field("bytes", &Bytes(self.bytes.len()))
            .field("map", &self.map)
            .field(
                "content_type",
                &self.content_type.as_ref().map(ContentType::essence),
            )
            .finish()
    }
}

/// Alle Suchziele einer Anfrage, mitsamt den Kopien, auf die sie zeigen.
///
/// Ohne abgeleitetes `Debug`, aus demselben Grund wie [`ScanInput`].
#[derive(Clone)]
pub struct ScanTargets {
    entries: Vec<Entry>,
    truncated: bool,
    diagnostics: Vec<Diagnostic>,
}

impl fmt::Debug for ScanTargets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScanTargets")
            .field("entries", &self.entries)
            .field("truncated", &self.truncated)
            .field(
                "diagnostics",
                &self
                    .diagnostics
                    .iter()
                    .map(|found| found.code.as_str())
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl ScanTargets {
    /// Zerlegt eine Anfrage.
    ///
    /// `cap_bytes` gilt je Suchziel, nicht für die Summe: ein großer Body kann
    /// die Header nicht verdrängen. `max_ratio` begrenzt zusätzlich, wie weit
    /// ein kodierter Body beim Entpacken wachsen darf.
    #[must_use]
    pub fn from_request(
        request: &HttpRequest,
        body: &[u8],
        cap_bytes: usize,
        max_ratio: u32,
    ) -> Self {
        let mut entries = Vec::new();
        let mut truncated = false;
        let mut diagnostics = Vec::new();

        for (name, value) in &request.headers {
            let bytes = value.as_bytes();
            let cut = bytes.len().min(cap_bytes);
            truncated |= cut < bytes.len();
            entries.push(Entry {
                location: FindingLocation::Header(name.clone()),
                bytes: bytes[..cut].to_vec(),
                map: SpanMap::Identity,
                content_type: None,
            });
        }

        if let Some((_, query)) = request.path_and_query.split_once('?') {
            let raw = query.as_bytes();
            let cut = raw.len().min(cap_bytes);
            truncated |= cut < raw.len();
            let raw = &raw[..cut];
            entries.push(Entry {
                location: FindingLocation::Query,
                bytes: raw.to_vec(),
                map: SpanMap::Identity,
                content_type: None,
            });
            let (decoded, table) = percent_decode(raw);
            if decoded != raw {
                entries.push(Entry {
                    location: FindingLocation::Query,
                    bytes: decoded,
                    map: SpanMap::Table(table),
                    content_type: None,
                });
            }
        }

        let content_type =
            header_value(request, "content-type").map(|value| ContentType::parse(&value));
        let encoding = header_value(request, "content-encoding")
            .map_or(ContentEncoding::Identity, |value| {
                ContentEncoding::parse(&value)
            });
        let decoded = decode_body(body, &encoding, cap_bytes, max_ratio);
        truncated |= decoded.truncated;
        if let Some(diagnostic) = decoded.diagnostic {
            diagnostics.push(diagnostic);
        }
        if !decoded.bytes.is_empty() {
            let bytes = if is_textual(content_type.as_ref(), &decoded.bytes) {
                decoded.bytes
            } else {
                printable_only(&decoded.bytes, MIN_PRINTABLE_RUN)
            };
            entries.push(Entry {
                location: FindingLocation::Body,
                bytes,
                map: SpanMap::Identity,
                content_type,
            });
        }

        Self {
            entries,
            truncated,
            diagnostics,
        }
    }

    /// Wahr, wenn nicht alles durchsucht wurde.
    #[must_use]
    pub const fn truncated(&self) -> bool {
        self.truncated
    }

    /// Die Befunde, die die Lücken erklären.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Die Suchziele in fester Reihenfolge: Header, Query, Body.
    pub fn iter(&self) -> impl Iterator<Item = ScanTarget<'_>> {
        self.entries.iter().map(|entry| ScanTarget {
            input: ScanInput {
                location: entry.location.clone(),
                bytes: &entry.bytes,
                content_type: entry.content_type.as_ref(),
            },
            map: &entry.map,
        })
    }
}

/// Entscheidet, ob der Body vollständig oder im „strings"-Modus durchsucht wird.
///
/// Mit `Content-Type` entscheidet der Typ. Ohne `Content-Type` entscheidet der
/// Inhalt: gültiges UTF-8 ist Text, alles andere ist binär. Ein Body ohne
/// Kopfzeile wäre sonst als binär behandelt worden, und ein kurzer Text wie
/// `ACME` wäre unter die Mindestlänge eines Laufs gefallen.
fn is_textual(content_type: Option<&ContentType>, bytes: &[u8]) -> bool {
    content_type.map_or_else(
        || core::str::from_utf8(bytes).is_ok(),
        ContentType::is_textual,
    )
}

fn header_value(request: &HttpRequest, name: &'static str) -> Option<String> {
    request
        .headers
        .get(HeaderName::from_static(name))
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use core::fmt::Write as _;

    use humanitl_core::http::{HeaderMap, HeaderName, HeaderValue};
    use humanitl_core::{Authority, FindingLocation, HostName, HttpRequest, Method, Scheme};

    use super::{ContentEncoding, ScanTargets, SpanMap};

    fn request(path_and_query: &str, headers: &[(&'static str, &str)]) -> HttpRequest {
        let mut map = HeaderMap::new();
        for (name, value) in headers {
            map.insert(
                HeaderName::from_static(name),
                HeaderValue::from_str(value).unwrap(),
            );
        }
        HttpRequest::new(
            Method::POST,
            Scheme::Https,
            Authority::with_scheme(HostName::Dns("api.example.com".to_owned()), Scheme::Https),
            path_and_query,
        )
        .with_headers(map)
    }

    #[test]
    fn every_header_becomes_its_own_target() {
        let request = request("/v1", &[("authorization", "Bearer x"), ("cookie", "a=b")]);
        let targets = ScanTargets::from_request(&request, b"", 4096, 100);
        let locations: Vec<String> = targets
            .iter()
            .map(|target| target.input.location.to_string())
            .collect();
        assert!(locations.contains(&"header:authorization".to_owned()));
        assert!(locations.contains(&"header:cookie".to_owned()));
    }

    #[test]
    fn the_query_appears_raw_and_decoded() {
        let request = request("/v1?q=user%40example.com", &[]);
        let targets = ScanTargets::from_request(&request, b"", 4096, 100);
        let query: Vec<_> = targets
            .iter()
            .filter(|target| target.input.location == FindingLocation::Query)
            .collect();
        assert_eq!(query.len(), 2);
        assert_eq!(query[0].input.bytes, b"q=user%40example.com");
        assert_eq!(query[1].input.bytes, b"q=user@example.com");
        assert!(matches!(query[1].map, SpanMap::Table(_)));
        assert_eq!(query[1].map.map(2..18), Some(2..20));
    }

    #[test]
    fn a_binary_body_is_reduced_to_its_strings() {
        let mut body = vec![0u8; 16];
        body.extend_from_slice(b"AKIAIOSFODNN7EXAMPLE");
        let request = request("/v1", &[("content-type", "application/octet-stream")]);
        let targets = ScanTargets::from_request(&request, &body, 4096, 100);
        let target = targets
            .iter()
            .find(|target| target.input.location == FindingLocation::Body)
            .unwrap();
        assert_eq!(target.input.bytes.len(), body.len());
        assert_eq!(&target.input.bytes[16..], b"AKIAIOSFODNN7EXAMPLE");
        assert!(target.input.bytes[..16].iter().all(|byte| *byte == b'\n'));
    }

    #[test]
    fn a_json_body_is_kept_as_it_is() {
        let body = b"{\"note\":\"a b\"}";
        let request = request("/v1", &[("content-type", "application/json")]);
        let targets = ScanTargets::from_request(&request, body, 4096, 100);
        let target = targets
            .iter()
            .find(|target| target.input.location == FindingLocation::Body)
            .unwrap();
        assert_eq!(target.input.bytes, body);
    }

    #[test]
    fn debug_never_renders_the_scanned_bytes() {
        // Ein `Debug` landet in Logs, in Panics und in fehlgeschlagenen
        // Zusicherungen. Es darf den Inhalt der Anfrage weder als Text noch als
        // Zahlenreihe hergeben.
        let secret = "AKIAIOSFODNN7EXAMPLE";
        let request = request(
            &format!("/v1?key={secret}"),
            &[("authorization", &format!("Bearer {secret}"))],
        );
        let body = format!("aws_access_key_id = {secret}");
        let targets = ScanTargets::from_request(&request, body.as_bytes(), 4096, 100);

        let mut rendered = format!("{targets:?}");
        for target in targets.iter() {
            let _ = write!(rendered, "{target:?}{:?}{:?}", target.input, target.map);
        }
        let _ = write!(
            rendered,
            "{:?}",
            crate::decode::decode_body(body.as_bytes(), &ContentEncoding::Identity, 4096, 100)
        );

        assert!(!rendered.contains(secret), "{rendered}");
        assert!(!rendered.contains("AKIA"), "{rendered}");
        // Dieselben Bytes dezimal ("65, 75, 73, 65") und hexadezimal.
        let decimal = secret
            .bytes()
            .map(|byte| byte.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        assert!(!rendered.contains(&decimal[..11]), "{rendered}");
        let hex = secret.bytes().fold(String::new(), |mut acc, byte| {
            let _ = write!(acc, "{byte:02x}");
            acc
        });
        assert!(!rendered.contains(&hex[..8]), "{rendered}");
        // Und die Zahlen, die stehen dürfen, stehen auch wirklich da.
        assert!(rendered.contains("<40 Bytes>"), "{rendered}");
    }

    #[test]
    fn a_body_over_the_cap_marks_the_scan_truncated() {
        let request = request("/v1", &[("content-type", "text/plain")]);
        let targets = ScanTargets::from_request(&request, &[b'a'; 100], 10, 100);
        assert!(targets.truncated());
        assert_eq!(targets.diagnostics().len(), 1);
        assert_eq!(targets.diagnostics()[0].code.as_str(), "FINDINGS_002");
    }
}
