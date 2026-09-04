//! Werttypen der Aufzeichnung: was hineingeht und was herauskommt.
//!
//! Die Zeilen der Datenbank tauchen hier als Werte auf, in derselben Form wie
//! in `V1__init.sql`. Zustand, Entscheidung und Grund bleiben absichtlich Text
//! und werden nicht in die Aufzählungen des Kerns zurückverwandelt: die
//! Aufzeichnung ist ein Archiv, und ein Archiv muss auch eine Zeile lesen
//! können, die eine ältere Fassung geschrieben hat.

use std::time::{SystemTime, UNIX_EPOCH};

use humanitl_core::{BodyRef, FlowId, SessionId};

/// So viele Zeilen liefert [`FlowQuery`] ohne eigene Angabe.
pub const DEFAULT_LIMIT: u32 = 200;

/// Mehr Zeilen als das liefert eine Seite nie.
pub const MAX_LIMIT: u32 = 1000;

/// So weit zählt [`FlowPage::total_estimate`] höchstens.
pub const COUNT_CEILING: u64 = 10_000;

/// Millisekunden seit der Epoche, wie sie in jeder Zeitspalte stehen.
///
/// Ein Zeitpunkt vor 1970 wird zu einer negativen Zahl; ein Zeitpunkt, der
/// nicht mehr in `i64` passt, wird auf [`i64::MAX`] begrenzt. Beides ist eine
/// Uhr, die falsch geht, und keine Lage, in der die Aufzeichnung stehenbleiben
/// soll.
#[must_use]
pub fn millis(at: SystemTime) -> i64 {
    match at.duration_since(UNIX_EPOCH) {
        Ok(delta) => i64::try_from(delta.as_millis()).unwrap_or(i64::MAX),
        Err(err) => -i64::try_from(err.duration().as_millis()).unwrap_or(i64::MAX),
    }
}

/// Richtung einer aufgezeichneten Nachricht, Spalte `messages.dir`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Dir {
    /// Die Anfrage, wie sie ankam.
    Request,
    /// Die Anfrage, wie der Mensch sie bearbeitet hat.
    RequestEdited,
    /// Die Antwort des Ziels.
    Response,
}

impl Dir {
    /// Kurzname in `snake_case`, wie er in der Spalte steht.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Request => "request",
            Self::RequestEdited => "request_edited",
            Self::Response => "response",
        }
    }

    /// Liest eine Richtung aus der Spalte zurück.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "request" => Some(Self::Request),
            "request_edited" => Some(Self::RequestEdited),
            "response" => Some(Self::Response),
            _ => None,
        }
    }
}

impl core::fmt::Display for Dir {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Kopfdaten einer Sitzung, Zeile in `sessions`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionMeta {
    /// Id der Sitzung.
    pub id: SessionId,
    /// Wann die Sitzung begann.
    pub started_at: SystemTime,
    /// Name des Sandbox-Profils.
    pub sandbox_profile: String,
    /// Der `LLM`-Endpunkt, falls einer gesetzt war.
    pub llm_endpoint: Option<String>,
    /// Das Projektverzeichnis auf dem Host.
    pub work_dir: String,
    /// Der Agent-Adapter, zum Beispiel `opencode`.
    pub agent: String,
}

/// Wonach eine Seite sortiert wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SortKey {
    /// Nach Ankunftszeit (Vorgabe).
    #[default]
    Ts,
    /// Nach Ziel-Host.
    Host,
    /// Nach Dauer.
    Duration,
    /// Nach Größe von Anfrage und Antwort zusammen.
    Size,
}

impl SortKey {
    /// Kurzname in `snake_case`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ts => "ts",
            Self::Host => "host",
            Self::Duration => "duration",
            Self::Size => "size",
        }
    }

    /// Liest einen Sortierschlüssel aus Text.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "ts" => Some(Self::Ts),
            "host" => Some(Self::Host),
            "duration" => Some(Self::Duration),
            "size" => Some(Self::Size),
            _ => None,
        }
    }

    /// Der Ausdruck, nach dem sortiert wird.
    ///
    /// Für [`SortKey::Ts`] ist er leer: dann sind `ts` und `id` bereits der
    /// vollständige Schlüssel, und genau diese Reihenfolge liegt im Index
    /// `flows_ts`. Die anderen Schlüssel setzen einen Ausdruck davor, der nie
    /// `NULL` wird, damit die Ordnung total bleibt und der Keyset-Cursor
    /// dieselbe Ordnung vergleichen kann wie das `ORDER BY`.
    pub(crate) const fn expr(self) -> Option<&'static str> {
        match self {
            Self::Ts => None,
            Self::Host => Some("host"),
            Self::Duration => Some("COALESCE(duration_ms, -1)"),
            Self::Size => Some("(request_size + COALESCE(response_size, 0))"),
        }
    }
}

/// Der Wert des Sortierschlüssels am Ende einer Seite.
#[derive(Debug, Clone, PartialEq)]
pub enum CursorKey {
    /// Ein Zahlenschlüssel (Dauer, Größe).
    Int(i64),
    /// Ein Textschlüssel (Host).
    Text(String),
}

/// Wo die nächste Seite beginnt (Keyset, kein Offset).
///
/// `ts` und `id` sind der Schlüssel aus dem Index `flows_ts`; `sort` trägt
/// zusätzlich den Wert des gewählten Sortierschlüssels, wenn nicht nach `ts`
/// sortiert wird. Ohne dieses dritte Feld ließe sich eine nach Host oder Größe
/// sortierte Liste nicht lückenlos weiterblättern (Abweichung von der Skizze
/// in `backlog/sprint-2.md`, dort hat der Cursor nur zwei Felder).
#[derive(Debug, Clone, PartialEq)]
pub struct Cursor {
    /// Ankunftszeit der letzten Zeile der Seite, Unix-Millisekunden.
    pub ts: i64,
    /// Id der letzten Zeile der Seite.
    pub id: String,
    /// Der Wert des Sortierschlüssels, falls nicht nach `ts` sortiert wird.
    pub sort: Option<CursorKey>,
}

impl Cursor {
    /// Ein Cursor für die Sortierung nach Ankunftszeit.
    #[must_use]
    pub fn new(ts: i64, id: impl Into<String>) -> Self {
        Self {
            ts,
            id: id.into(),
            sort: None,
        }
    }
}

/// Eine Anfrage an die Flow-Liste.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowQuery {
    /// Der Filterausdruck, siehe `filter`-Modul. Leer heißt „alles".
    pub filter: String,
    /// Wonach sortiert wird.
    pub sort: SortKey,
    /// Absteigend (Vorgabe) oder aufsteigend.
    pub desc: bool,
    /// Höchstens so viele Zeilen, begrenzt auf [`MAX_LIMIT`].
    pub limit: u32,
    /// Wo die Seite beginnt.
    pub cursor: Option<Cursor>,
}

impl Default for FlowQuery {
    fn default() -> Self {
        Self {
            filter: String::new(),
            sort: SortKey::Ts,
            desc: true,
            limit: DEFAULT_LIMIT,
            cursor: None,
        }
    }
}

impl FlowQuery {
    /// Eine Anfrage mit diesem Filter, sonst Vorgaben.
    #[must_use]
    pub fn new(filter: impl Into<String>) -> Self {
        Self {
            filter: filter.into(),
            ..Self::default()
        }
    }

    /// Die tatsächlich benutzte Zeilenzahl: mindestens 1, höchstens [`MAX_LIMIT`].
    #[must_use]
    pub const fn effective_limit(&self) -> u32 {
        if self.limit == 0 {
            DEFAULT_LIMIT
        } else if self.limit > MAX_LIMIT {
            MAX_LIMIT
        } else {
            self.limit
        }
    }
}

/// Eine Seite der Flow-Liste.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowPage {
    /// Die Zeilen dieser Seite.
    pub rows: Vec<FlowSummary>,
    /// Wo die nächste Seite beginnt, sonst `None`.
    pub next: Option<Cursor>,
    /// Wie viele Zeilen der Filter trifft.
    ///
    /// Gezählt wird höchstens bis [`COUNT_CEILING`], damit ein Filter über eine
    /// lange History nicht die ganze Tabelle zählt. Ob die Zahl exakt ist oder
    /// nur eine Untergrenze, sagt [`FlowPage::capped`]; wer sie anzeigt, muss
    /// beides unterscheiden (`backlog/CONVENTIONS.md` 4.13: eine geschätzte
    /// Zahl wird als geschätzt gekennzeichnet).
    pub total_estimate: u64,
    /// Wahr, wenn [`FlowPage::total_estimate`] an der Obergrenze abgeschnitten
    /// wurde und die wahre Zahl größer ist.
    ///
    /// Dann heißt `total_estimate = 10000` „mindestens 10000", und die
    /// Oberfläche schreibt `10000+`, nie `10000`.
    pub capped: bool,
}

impl FlowPage {
    /// Die Trefferzahl als Text, mit `+`, wo sie nur eine Untergrenze ist.
    #[must_use]
    pub fn total_text(&self) -> String {
        if self.capped {
            format!("{}+", self.total_estimate)
        } else {
            self.total_estimate.to_string()
        }
    }
}

/// Eine Zeile der Flow-Liste: alle Spalten von `flows`, kein Body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowSummary {
    /// Id des Flows.
    pub id: FlowId,
    /// Sitzung, zu der er gehört.
    pub session: SessionId,
    /// Laufende Nummer innerhalb der Sitzung, 1-basiert.
    pub seq: i64,
    /// Ankunftszeit in Unix-Millisekunden.
    pub ts: i64,
    /// Die Methode der Anfrage.
    pub method: String,
    /// Das Schema der Anfrage.
    pub scheme: String,
    /// Der Ziel-Host als A-Label, lowercase.
    pub host: String,
    /// Der Ziel-Host in der Schreibweise für Menschen (U-Label).
    pub host_display: String,
    /// Der Ziel-Port.
    pub port: u16,
    /// Pfad samt Query.
    pub path: String,
    /// Der angefragte Protokollwechsel, sonst `None`.
    pub upgrade: Option<String>,
    /// Der Zustand, benannt wie `FlowState::name`.
    ///
    /// `received`, `analyzed`, `held`, `decided`, `forwarded`, `responded`,
    /// `failed` oder `recorded`. Der Spaltenkommentar in `V1__init.sql` nennt
    /// `failed` nicht, weil das `SQL` dort wortgleich aus der Spezifikation
    /// stammt; der Zustand kam mit `backlog/CONVENTIONS.md` 4.10 dazu und wird
    /// geschrieben wie jeder andere (siehe 4.14).
    pub state: String,
    /// Die Entscheidung, sobald es eine gibt.
    pub decision: Option<String>,
    /// Der Grund des Blocks, sobald es einen gibt.
    pub block_reason: Option<String>,
    /// Die Regel, die entschied.
    pub rule_id: Option<String>,
    /// Wahr für `LLM`-Passthrough.
    pub passthrough: bool,
    /// Der Status der Antwort.
    pub status: Option<u16>,
    /// Ankunft bis Abschluss in Millisekunden.
    pub duration_ms: Option<i64>,
    /// Beginn des Wartens bis Entscheidung in Millisekunden.
    pub held_ms: Option<i64>,
    /// Wahr, wenn der Mensch die Anfrage bearbeitet hat.
    pub edited: bool,
    /// Wie viele Funde die Detektoren meldeten.
    pub findings_count: u32,
    /// Größe des Anfrage-Bodys in Bytes.
    pub request_size: u64,
    /// Größe des Antwort-Bodys in Bytes, sobald sie feststeht.
    pub response_size: Option<u64>,
    /// Der Apex nach der Public Suffix List.
    pub apex: Option<String>,
    /// Die Kennung im Domain-Katalog.
    pub catalog_id: Option<String>,
    /// Woran der Flow gescheitert ist, sonst `None`.
    ///
    /// Ein kurzer, fester Bezeichner: `upstream_dns`, `upstream_connect`,
    /// `upstream_tls`, `upstream_private_address:<ip>`, `upstream_timeout` für
    /// einen gescheiterten Weg nach draußen, `tls_handshake_failed` für einen
    /// Client in der Sandbox, der den Handschlag zum Proxy abgebrochen hat
    /// (HUM-045). Nicht dasselbe wie [`FlowSummary::block_reason`]: dort steht,
    /// warum jemand geblockt hat, hier, woran es gescheitert ist.
    pub error: Option<String>,
}

/// Eine aufgezeichnete Nachricht: Kopfzeilen plus Verweis auf den Body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageRecord {
    /// Richtung.
    pub dir: Dir,
    /// Die Kopfzeilen in Originalreihenfolge.
    pub headers: Vec<(String, String)>,
    /// Der Wert aus `Content-Type`.
    pub content_type: Option<String>,
    /// Der Wert aus `Content-Encoding`.
    pub content_encoding: Option<String>,
    /// Der Body: inline, wenn er klein genug war, sonst nur als Verweis.
    pub body: BodyRef,
}

/// Ein aufgezeichneter Fund.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindingRecord {
    /// Laufende Nummer innerhalb des Flows, 0-basiert.
    pub idx: u32,
    /// Art des Fundes, zum Beispiel `api_key:github`.
    pub kind: String,
    /// Wo der Fund liegt, zum Beispiel `header:authorization`.
    pub location: String,
    /// Beginn des Fundes im Ort, in Bytes.
    pub span_start: u64,
    /// Ende des Fundes im Ort, in Bytes.
    pub span_end: u64,
    /// Wie sicher der Fund ist.
    pub tier: String,
    /// `SHA-256` über den gefundenen Wert.
    pub value_hash: [u8; 32],
    /// Die ersten Zeichen des Werts für die Anzeige.
    pub display_prefix: String,
    /// Was aus dem Fund wurde: `replaced`, `ignored` oder nichts.
    pub resolved: Option<String>,
}

/// Ein Flow mit allem, was zu ihm aufgezeichnet wurde.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowDetail {
    /// Die Zeile aus `flows`.
    pub summary: FlowSummary,
    /// Die Nachrichten, sortiert nach Richtung.
    pub messages: Vec<MessageRecord>,
    /// Die Funde, sortiert nach `idx`.
    pub findings: Vec<FindingRecord>,
}

/// Was ein Aufräumlauf gelöscht hat.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PurgeReport {
    /// Gelöschte Zeilen in `flows`.
    pub flows: u64,
    /// Gelöschte Zeilen in `messages`.
    pub messages: u64,
    /// Gelöschte Zeilen in `findings`.
    pub findings: u64,
    /// Gelöschte Zeilen in `sessions`.
    pub sessions: u64,
    /// Gelöschte Dateien im Blob-Speicher.
    pub blobs: u64,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::time::{Duration, UNIX_EPOCH};

    use super::{Dir, FlowQuery, MAX_LIMIT, SortKey, millis};

    #[test]
    fn millis_counts_from_the_epoch() {
        assert_eq!(millis(UNIX_EPOCH), 0);
        assert_eq!(millis(UNIX_EPOCH + Duration::from_millis(1500)), 1500);
        assert_eq!(millis(UNIX_EPOCH - Duration::from_millis(250)), -250);
    }

    #[test]
    fn dir_round_trips_through_its_column_form() {
        for dir in [Dir::Request, Dir::RequestEdited, Dir::Response] {
            assert_eq!(Dir::parse(dir.as_str()), Some(dir));
        }
        assert_eq!(Dir::parse("nonsense"), None);
    }

    #[test]
    fn sort_key_round_trips_and_names_its_expression() {
        for key in [SortKey::Ts, SortKey::Host, SortKey::Duration, SortKey::Size] {
            assert_eq!(SortKey::parse(key.as_str()), Some(key));
        }
        assert_eq!(SortKey::Ts.expr(), None);
        assert_eq!(SortKey::Host.expr(), Some("host"));
    }

    #[test]
    fn limit_is_capped_and_never_zero() {
        let query = FlowQuery {
            limit: 0,
            ..FlowQuery::default()
        };
        assert_eq!(query.effective_limit(), super::DEFAULT_LIMIT);
        let query = FlowQuery {
            limit: 5_000,
            ..FlowQuery::default()
        };
        assert_eq!(query.effective_limit(), MAX_LIMIT);
    }
}
