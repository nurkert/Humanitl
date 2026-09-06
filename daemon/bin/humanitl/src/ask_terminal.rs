//! Der Prompt von `humanitl run --ask terminal` (HUM-067).
//!
//! Hier steht, wie eine gehaltene Anfrage im Terminal eines Menschen aussieht
//! und welche Taste was bedeutet. Entschieden wird nichts davon hier: Die
//! Tasten werden zu einer [`Choice`], und wer sie ausführt, spricht mit dem
//! Daemon (ADR-018).
//!
//! # Fremde Bytes in einem rohen Terminal
//!
//! Jedes Feld dieses Kastens kommt aus der Anfrage eines Agenten: Methode,
//! Adresse, Werkzeugname, Fundtext, Katalogzeile. Der Kasten wird in ein
//! Terminal geschrieben, das die Kommandozeile selbst in den Rohmodus gesetzt
//! hat, und in dem eine Steuerfolge deshalb unmittelbar wirkt. Zwei Riegel
//! stehen davor, und beide sitzen in [`field`]:
//!
//! 1. [`humanitl_core::block::sanitize_note`] über [`crate::render::plain`]:
//!    kein `ESC`, kein `CSI`, keine Zeilenumbrüche, keine C1-Bytes.
//! 2. Eine Breitenklemme je Zeile. Ohne sie schöbe eine lange Adresse die
//!    rechte Kante des Kastens vom Schirm, und die Tastenzeile gleich mit --
//!    ein Kasten, dessen Rand woanders steht, sieht aus wie ein anderer
//!    Kasten, und genau das ist der Trick, gegen den `CLI_002` bei
//!    Vollbild-TUIs steht (`backlog/CONVENTIONS.md` 4.10).
//!
//! **Der Kasten ist ASCII, und das ist die dritte Sperre.** Eine Breite in
//! Zeichen ist nicht dieselbe wie eine Breite in Spalten: `한` und die meisten
//! Emoji belegen zwei Spalten, und eine Zeile, die in Zeichen genau passt,
//! ist auf dem Schirm dann breiter als das Fenster. Sie bricht um, die
//! folgende Zeile steht verschoben, und das Löschen vor dem nächsten Zeichnen
//! (`ESC [ n A`) geht um zu wenige Zeilen hinauf -- der Kasten zerfällt genau
//! so, wie er es mit einer Steuerfolge täte. Statt eine Bibliothek für
//! Spaltenbreiten mitzunehmen, ersetzt [`field`] jeden Skalar außerhalb von
//! ASCII durch `?`: Danach ist eine Spalte ein Zeichen, und die Klemme rechnet
//! richtig. Der Preis steht in `docs/cli.md`: Ein Pfad in einer anderen
//! Schrift ist im Kasten nicht zu lesen. Wer ihn lesen will, sieht die Anfrage
//! mit `humanitl flows show`, wo kein Rahmen zu halten ist.

use std::fmt::Write as _;

use humanitl_ipc::v1;

use crate::render::plain;

/// Die Zeichen des Rahmens.
const TOP_LEFT: char = '┌';
const TOP_RIGHT: char = '┐';
const BOTTOM_LEFT: char = '└';
const BOTTOM_RIGHT: char = '┘';
const HORIZONTAL: char = '─';
const VERTICAL: char = '│';

/// Die Breite, mit der gezeichnet wird, wenn das Terminal keine nennt.
pub const FALLBACK_WIDTH: usize = 80;

/// Die schmalste Breite, mit der gezeichnet wird.
///
/// Ein schmaleres Fenster bekommt trotzdem diese Breite: Der Kasten bräche
/// dann in jeder Zeile um, und das Löschen vor dem nächsten Zeichnen ginge um
/// zu wenige Zeilen hinauf. Ein Fenster unter 40 Spalten ist kein Ort für
/// diese Frage; `docs/cli.md` sagt es, und wer dort moderieren will,
/// vergrößert das Fenster oder nimmt `--ask ui`.
pub const MIN_BOX_WIDTH: usize = 40;

/// Was ein Mensch am Prompt gewählt hat.
///
/// Die Werte sind das, was die Kommandozeile danach tut, nicht das, was sie
/// entscheidet: `Allow` und `Block` reisen als `Decide`, `Rule` öffnet die
/// zweite Frage, und `Next` lässt die Anfrage gehalten stehen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// `a`: einmal erlauben.
    AllowOnce,
    /// `s`: erlauben und für diese Sitzung merken.
    AllowSession,
    /// `b`: blocken.
    Block,
    /// `r`: Regel bauen, in zwei Schritten.
    Rule,
    /// `e`: die Anfrage im `$EDITOR` ändern.
    Edit,
    /// `v`: den Rumpf ansehen.
    View,
    /// `n`: die nächste gehaltene Anfrage, ohne zu entscheiden.
    Next,
    /// `Esc` oder `Ctrl+C`: Prompt schließen, Anfrage bleibt gehalten.
    Close,
}

/// Die Taste zu einer Wahl, oder `None` für alles andere.
///
/// Groß und klein gelten gleich: Wer die Umschalttaste hält, meint dasselbe.
/// `Ctrl+C` (0x03) und `Esc` (0x1b) schließen den Prompt, statt ihn zu
/// beenden -- die Anfrage bleibt gehalten, und der Agent wartet weiter.
#[must_use]
pub fn choice_of(byte: u8) -> Option<Choice> {
    match byte.to_ascii_lowercase() {
        b'a' => Some(Choice::AllowOnce),
        b's' => Some(Choice::AllowSession),
        b'b' => Some(Choice::Block),
        b'r' => Some(Choice::Rule),
        b'e' => Some(Choice::Edit),
        b'v' => Some(Choice::View),
        b'n' => Some(Choice::Next),
        0x03 | 0x1b => Some(Choice::Close),
        _ => None,
    }
}

/// Eine gehaltene Anfrage, so wie der Prompt sie zeigt.
///
/// Die Felder sind schon Text und keine Struktur: Was hier steht, ist das
/// Ergebnis von [`from_detail`], und der Kasten selbst rechnet nichts mehr
/// aus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    /// Die Kennung, mit der `Decide` diese Anfrage anspricht.
    pub flow_id: String,
    /// `POST https://api.github.com/graphql`.
    pub request: String,
    /// `opencode · webfetch`, oder leer.
    pub origin: String,
    /// `2.1 KB · json`, oder leer.
    pub size: String,
    /// `1 · GITHUB_TOKEN in header Authorization`, oder leer.
    pub findings: String,
    /// `GitHub API - source hosting`, oder leer.
    pub catalog: String,
    /// Der Host, wie der Dienst ihn nennt -- nicht aus der Zeile gelesen.
    ///
    /// Eine Regel entsteht aus diesem Feld und nicht aus dem Text des
    /// Kastens: `[::1]:8080` zerfiele beim Zerlegen der Zeile in `[`, und eine
    /// Regel auf `[` trifft nie etwas.
    pub host: String,
    /// Der Pfad ohne Abfrage, wie der Dienst ihn nennt.
    ///
    /// Ohne Abfrage, weil der Regelvergleich sie abschneidet
    /// (`daemon/crates/rules/src/path.rs`): Eine Regel mit `?query=1` im Pfad
    /// träfe die nächste Anfrage nicht.
    pub path: String,
    /// Die registrierbare Domain laut Public Suffix List, oder leer.
    ///
    /// Aus `DomainInfo.apex` des Dienstes und nicht aus den letzten beiden
    /// Marken geraten: `a.b.github.io` hat den Apex `b.github.io`, und eine
    /// Regel auf `**.github.io` gälte für jede fremde Seite dort.
    pub apex: String,
    /// Die Methode als Zahl des Vertrags.
    pub method: i32,
}

/// Die Anfrage, wie der Dienst sie beschreibt.
///
/// Jedes Feld läuft durch [`plain`]; was der Agent geschickt hat, ist damit
/// Text und keine Steuerfolge mehr.
#[must_use]
pub fn from_detail(detail: &v1::FlowDetail) -> Held {
    let summary = detail.summary.as_ref();
    let flow_id = summary.map(|s| s.flow_id.clone()).unwrap_or_default();
    let request = summary.map_or_else(String::new, |s| {
        let method = if s.method_raw.is_empty() {
            method_name(s.method)
        } else {
            s.method_raw.clone()
        };
        let authority = s
            .authority
            .as_ref()
            .map_or_else(String::new, authority_text);
        let scheme = if s.scheme == i32::from(v1::Scheme::Http) {
            "http"
        } else {
            "https"
        };
        format!("{method} {scheme}://{authority}{}", s.path)
    });
    let origin = summary.map_or_else(String::new, |s| s.origin_tool.clone());
    let size = summary.map_or_else(String::new, |s| {
        if s.request_size == 0 {
            String::new()
        } else {
            format!("{}", ByteSize(s.request_size))
        }
    });
    let findings = findings_text(detail);
    let catalog = detail
        .domain
        .as_ref()
        .map_or_else(String::new, catalog_text);
    let host = summary
        .and_then(|s| s.authority.as_ref())
        .map_or_else(String::new, |authority| authority.host.clone());
    let path = summary.map_or_else(String::new, |s| {
        s.path
            .split_once('?')
            .map_or_else(|| s.path.clone(), |(path, _)| path.to_owned())
    });
    let apex = detail
        .domain
        .as_ref()
        .map_or_else(String::new, |domain| domain.apex.clone());
    let method_number = summary.map_or(0, |s| s.method);
    Held {
        flow_id: plain(&flow_id),
        request: plain(&request),
        origin: plain(&origin),
        size: plain(&size),
        findings: plain(&findings),
        catalog: plain(&catalog),
        // Host, Pfad, Apex und Methode reisen ungesäubert: Sie stehen in
        // keiner Zeile des Kastens, sondern gehen in eine Regel, und der
        // Daemon prüft sie.
        host,
        path,
        apex,
        method: method_number,
    }
}

/// `1 · GITHUB_TOKEN in header Authorization`, oder leer ohne Funde.
///
/// Genannt wird der erste Fund und die Zahl aller; eine Liste im Kasten
/// verschöbe die Tastenzeile nach unten aus dem Blick, und wer alle sehen
/// will, drückt `v`.
fn findings_text(detail: &v1::FlowDetail) -> String {
    let count = detail.findings.len();
    if count == 0 {
        return String::new();
    }
    let first = &detail.findings[0];
    let mut out = format!("{count} - {}", first.kind);
    if !first.header_name.is_empty() {
        let _ = write!(out, " in header {}", first.header_name);
    }
    out
}

/// `github.api · rank #37`, oder was davon da ist.
///
/// **Der Name des Dienstes steht hier nicht, und das ist keine Auslassung.**
/// Der Vertrag schickt die Kennung des Katalogeintrags und nicht seinen Namen
/// (`humanitl.proto`, `DomainInfo.catalog_id`: „Name, Beschreibung, Quelle und
/// Symbol schlägt die Oberfläche in ihrer eigenen Kopie derselben Datei
/// nach"). Die Kommandozeile trägt diese Kopie nicht, und den
/// Verzeichnis-Suchweg des Daemons hier zu wiederholen, hieße dieselbe
/// Entscheidung an zwei Stellen zu treffen. Die Kennung ist der kürzeste Weg
/// von dem, was ein Mensch sieht, zu dem Eintrag, der es entschieden hat --
/// dieselbe Begründung wie beim Diagnostic-Code.
fn catalog_text(domain: &v1::DomainInfo) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(2);
    if !domain.catalog_id.is_empty() {
        parts.push(domain.catalog_id.clone());
    }
    if domain.tranco_rank > 0 {
        parts.push(format!("rank #{}", domain.tranco_rank));
    }
    parts.join(" - ")
}

/// `example.com` oder `example.com:8443`.
fn authority_text(authority: &v1::Authority) -> String {
    if authority.port == 0 || authority.port == 443 || authority.port == 80 {
        authority.host.clone()
    } else {
        format!("{}:{}", authority.host, authority.port)
    }
}

/// Der Name einer Methode aus dem Enum, für den Fall ohne `method_raw`.
fn method_name(method: i32) -> String {
    match v1::Method::try_from(method) {
        Ok(v1::Method::Get) => "GET",
        Ok(v1::Method::Post) => "POST",
        Ok(v1::Method::Put) => "PUT",
        Ok(v1::Method::Patch) => "PATCH",
        Ok(v1::Method::Delete) => "DELETE",
        Ok(v1::Method::Head) => "HEAD",
        Ok(v1::Method::Options) => "OPTIONS",
        Ok(v1::Method::Connect) => "CONNECT",
        Ok(v1::Method::Trace) => "TRACE",
        _ => "?",
    }
    .to_owned()
}

/// Eine Zahl Bytes, wie ein Mensch sie liest.
struct ByteSize(u64);

impl std::fmt::Display for ByteSize {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let bytes = self.0;
        if bytes < 1024 {
            return write!(formatter, "{bytes} B");
        }
        #[expect(
            clippy::cast_precision_loss,
            reason = "eine Anzeige, keine Rechnung: die Ziffern hinter dem Komma \
                      interessieren hier niemanden"
        )]
        let kib = bytes as f64 / 1024.0;
        if kib < 1024.0 {
            write!(formatter, "{kib:.1} KB")
        } else {
            write!(formatter, "{:.1} MB", kib / 1024.0)
        }
    }
}

/// Die Zeilen des Prompts, fertig zum Schreiben auf `stderr`.
///
/// `position` ist `(diese, alle)`, beide von eins an gezählt; `left` sind die
/// Sekunden bis zur Frist. `width` ist die Breite des Fensters.
#[must_use]
pub fn prompt_lines(held: &Held, position: (usize, usize), left: u64, width: usize) -> Vec<String> {
    let width = width.max(MIN_BOX_WIDTH);
    let inner = width - 4;
    let mut lines = Vec::with_capacity(9);
    lines.push(head_line(position, left, width));
    lines.push(field(&held.request, inner));
    if !held.origin.is_empty() {
        lines.push(field(&format!("from: {}", held.origin), inner));
    }
    if !held.size.is_empty() {
        lines.push(field(&format!("size: {}", held.size), inner));
    }
    if !held.findings.is_empty() {
        lines.push(field(&format!("findings: {}", held.findings), inner));
    }
    if !held.catalog.is_empty() {
        lines.push(field(&format!("catalog: {}", held.catalog), inner));
    }
    lines.push(field("", inner));
    for row in key_rows(inner) {
        lines.push(field(row, inner));
    }
    lines.push(format!(
        "{BOTTOM_LEFT}{}{BOTTOM_RIGHT}",
        String::from(HORIZONTAL).repeat(width - 2)
    ));
    lines
}

/// Die Tastenzeilen, so breit wie das Fenster sie trägt.
///
/// Eine Taste, die geklemmt wird, ist eine Taste, die niemand findet: In einem
/// Fenster von 60 Spalten stünde von `[b] block [r] rule [e] edit` nichts mehr
/// da, und der Kasten böte drei Wege an, von denen er zwei verschweigt.
fn key_rows(inner: usize) -> Vec<&'static str> {
    const WIDE: [&str; 2] = [
        "[a] allow once   [s] allow this session   [b] block   [r] rule   [e] edit",
        "[v] view body    [n] next                 [Esc] close",
    ];
    const NARROW: [&str; 4] = [
        "[a] allow once   [s] allow this session",
        "[b] block        [r] rule",
        "[e] edit         [v] view body",
        "[n] next         [Esc] close",
    ];
    if WIDE.iter().all(|row| row.chars().count() <= inner) {
        WIDE.to_vec()
    } else {
        NARROW.to_vec()
    }
}

/// `┌─ humanitl · request held (1 of 2) ──── 04:52 left ─┐`.
///
/// Der Kopf trägt zwei Zahlen und sonst nichts: die Stelle in der
/// Warteschlange und die Zeit, die bleibt. Beides entsteht hier und nicht aus
/// der Anfrage, ist also nicht zu fälschen.
fn head_line(position: (usize, usize), left: u64, width: usize) -> String {
    let clock = format!(" {} left ", countdown(left));
    // Vier Zeichen gehören dem Rahmen: die beiden Ecken und der Strich neben
    // jeder von ihnen. Was übrig bleibt, teilen sich Titel und Füllung, und die
    // Uhr steht rechts, weil sie sich jede Sekunde ändert und ein Auge sie dort
    // wiederfindet.
    let room = width.saturating_sub(clock.chars().count() + 4);
    let title = clamp(
        &format!(
            " humanitl · request held ({} of {}) ",
            position.0, position.1
        ),
        room,
    );
    let fill = room - title.chars().count();
    format!(
        "{TOP_LEFT}{HORIZONTAL}{title}{}{clock}{HORIZONTAL}{TOP_RIGHT}",
        String::from(HORIZONTAL).repeat(fill)
    )
}

/// `04:52`, aus Sekunden.
///
/// Über einer Stunde bleibt es bei Minuten: `90:00` ist eine Zahl, die ein
/// Mensch liest, `1:30:00` eine, die er erst zerlegen muss.
#[must_use]
pub fn countdown(left: u64) -> String {
    format!("{:02}:{:02}", left / 60, left % 60)
}

/// Eine Zeile im Kasten, gesäubert, auf ASCII gebracht und auf `inner`
/// Spalten geklemmt.
fn field(text: &str, inner: usize) -> String {
    let text = ascii_only(&plain(text));
    let clamped = clamp(&text, inner);
    let pad = inner.saturating_sub(clamped.chars().count());
    format!("{VERTICAL} {clamped}{} {VERTICAL}", " ".repeat(pad))
}

/// Jeder Skalar außerhalb von ASCII wird `?`.
///
/// Damit ist eine Spalte des Terminals ein Zeichen dieser Zeichenkette, und
/// die Breitenklemme rechnet in derselben Einheit, in der der Rahmen steht.
/// Die Begründung steht oben im Modul.
fn ascii_only(text: &str) -> String {
    text.chars()
        .map(|scalar| if scalar.is_ascii() { scalar } else { '?' })
        .collect()
}

/// Höchstens `width` Skalare, mit `…` am Ende, wenn etwas wegfällt.
fn clamp(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    let mut out: String = text.chars().take(width - 1).collect();
    // `~` und nicht `…`: Der Kasten rechnet in ASCII-Spalten, und ein Zeichen,
    // das eine Spalte breit sein soll, muss eines sein.
    out.push('~');
    out
}

/// Der Deckel des Puffers, der die Ausgabe des Agenten anhält.
///
/// Solange der Prompt steht, geht die Ausgabe des Agenten nicht auf den
/// Schirm: Sie überschriebe den Kasten, und ein halb überschriebener Kasten
/// ist eine Frage, deren Tasten nicht mehr dort stehen, wo sie stehen. Voll
/// wird der Puffer trotzdem irgendwann -- ein Agent, der weiterläuft, schreibt
/// weiter --, und dann wird er durchgelassen und der Kasten neu gezeichnet.
/// 256 KiB ist die Zahl der Spezifikation.
pub const OUTPUT_BUFFER_CAP: usize = 256 * 1024;

/// Was die Kommandozeile nach einer Taste tun muss.
///
/// Der Moderator entscheidet nichts selbst; er sagt, was zu tun ist, und der
/// Aufrufer spricht mit dem Daemon (ADR-018).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Nichts zu tun; der Kasten steht neu gezeichnet da.
    Draw,
    /// Die Sitzung beenden (`Ctrl+C` ohne stehenden Kasten).
    Stop,
    /// Den Kasten wegnehmen; es steht keine Frage mehr.
    Close,
    /// Nichts zu tun **und nichts am Schirm ändern**.
    ///
    /// Der Unterschied zu [`Step::Close`] ist der Grund, warum es beide gibt:
    /// Über den Ereignisstrom kommt weit mehr als „gehalten" und
    /// „entschieden" -- jede Antwort, jedes Stück einer Antwort, jede
    /// Regeländerung. Würde `Idle` den Kasten wegnehmen, verschwände er bei
    /// jedem Stück einer streamenden Antwort und käme erst mit dem nächsten
    /// Sekundentakt zurück.
    Idle,
    /// `Decide` mit dieser Entscheidung schicken.
    Decide {
        /// Der Fluss, um den es geht.
        flow_id: String,
        /// Was entschieden wurde.
        verdict: Verdict,
    },
    /// Die Einzelheiten dieses Flusses holen (`GetFlow`), dann zeichnen.
    Fetch(String),
    /// Den Rumpf zeigen (`GetBody`).
    ViewBody(String),
    /// Die Anfrage im `$EDITOR` öffnen.
    Edit(String),
    /// Die zweite Frage der Regel stellen.
    AskRule(String),
}

/// Die Entscheidung, die aus einer Taste wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Einmal erlauben.
    Allow,
    /// Erlauben und für diese Sitzung merken.
    AllowForSession,
    /// Blocken.
    Block,
}

/// Die gehaltenen Anfragen dieser Sitzung und der Kasten, der eine davon zeigt.
///
/// Reine Zustandslogik: keine Ein- und Ausgabe, keine Uhr, kein Netz. Der
/// Aufrufer schiebt Ereignisse hinein ([`Moderator::held`],
/// [`Moderator::decided`], [`Moderator::output`], [`Moderator::key`]) und führt
/// aus, was als [`Step`] zurückkommt.
#[derive(Debug, Default)]
pub struct Moderator {
    queue: Vec<String>,
    detail: Option<Held>,
    cursor: usize,
    buffer: Vec<u8>,
    dropped: bool,
}

impl Moderator {
    /// Ein Moderator ohne gehaltene Anfrage.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Wahr, solange ein Kasten steht.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.detail.is_some()
    }

    /// Die Anfrage, die der Kasten gerade zeigt.
    #[must_use]
    pub fn shown(&self) -> Option<&Held> {
        self.detail.as_ref()
    }

    /// Die Stelle in der Warteschlange, von eins an, und ihre Länge.
    #[must_use]
    pub fn position(&self) -> (usize, usize) {
        (self.cursor + 1, self.queue.len().max(1))
    }

    /// Eine neue gehaltene Anfrage.
    ///
    /// Steht noch kein Kasten, wird diese gezeigt; steht einer, reiht sie sich
    /// dahinter ein. Ein zweiter Kasten über dem ersten nähme dem Menschen die
    /// Frage weg, die er gerade beantwortet.
    pub fn held(&mut self, flow_id: &str) -> Step {
        if self.queue.iter().any(|id| id == flow_id) {
            return Step::Idle;
        }
        self.queue.push(flow_id.to_owned());
        if self.detail.is_none() {
            self.cursor = self.queue.len() - 1;
            return Step::Fetch(flow_id.to_owned());
        }
        Step::Draw
    }

    /// Die Einzelheiten, die [`Step::Fetch`] verlangt hat.
    pub fn show(&mut self, held: Held) {
        if let Some(at) = self.queue.iter().position(|id| *id == held.flow_id) {
            self.cursor = at;
        }
        self.detail = Some(held);
    }

    /// Über diesen Fluss ist entschieden worden -- hier oder anderswo.
    ///
    /// „Anderswo" ist der Normalfall mit laufender Oberfläche: Wer im Fenster
    /// entscheidet, nimmt dem Terminal die Frage ab, und der Kasten muss
    /// verschwinden, statt eine Entscheidung zu erfragen, die schon gefallen
    /// ist.
    pub fn decided(&mut self, flow_id: &str) -> Step {
        self.queue.retain(|id| id != flow_id);
        let shown = self
            .detail
            .as_ref()
            .is_some_and(|held| held.flow_id == flow_id);
        if !shown {
            // Der Zeiger zeigt auf eine Stelle der Warteschlange, und die
            // Warteschlange ist gerade kürzer geworden. Ohne diese Zeile stünde
            // im Kopf des Kastens „(2 of 1)", und `n` liefe im Kreis.
            if let Some(held) = self.detail.as_ref()
                && let Some(at) = self.queue.iter().position(|id| *id == held.flow_id)
            {
                self.cursor = at;
            }
            return if self.detail.is_some() {
                Step::Draw
            } else {
                Step::Idle
            };
        }
        self.detail = None;
        self.cursor = 0;
        match self.next_step() {
            // Nichts wartet mehr: Der Kasten geht weg, statt stehen zu
            // bleiben und über einen Fluss zu sprechen, über den entschieden
            // ist.
            Step::Idle => Step::Close,
            step => step,
        }
    }

    /// Nimmt die Warteschlange, die der Dienst führt, als die eigene.
    ///
    /// Nach verworfenen Ereignissen ist die eigene nicht mehr zu trauen: Sie
    /// kann eine gehaltene Anfrage vermissen und eine tragen, über die längst
    /// entschieden ist. Der Kasten bleibt stehen, wenn sein Fluss noch dabei
    /// ist, und geht sonst weg.
    pub fn resync(&mut self, held: &[String]) -> Step {
        self.queue = held.to_vec();
        let shown = self
            .detail
            .as_ref()
            .and_then(|held| self.queue.iter().position(|id| *id == held.flow_id));
        if let Some(at) = shown {
            self.cursor = at;
            return Step::Draw;
        }
        self.detail = None;
        self.cursor = 0;
        match self.next_step() {
            Step::Idle => Step::Close,
            step => step,
        }
    }

    /// Eine Taste.
    ///
    /// `Ctrl+C` ohne stehenden Kasten beendet die Sitzung: Im Rohmodus ist
    /// `ISIG` aus, der Kernel schickt also kein `SIGINT`, und ohne diesen Weg
    /// wäre `Ctrl+C` unter `--ask terminal` verschluckt -- entgegen dem, was
    /// `docs/cli.md` verspricht.
    pub fn key(&mut self, byte: u8) -> Step {
        let Some(choice) = choice_of(byte) else {
            return Step::Idle;
        };
        let Some(held) = self.detail.as_ref() else {
            return if byte == 0x03 { Step::Stop } else { Step::Idle };
        };
        let flow_id = held.flow_id.clone();
        match choice {
            Choice::AllowOnce => Step::Decide {
                flow_id,
                verdict: Verdict::Allow,
            },
            Choice::AllowSession => Step::Decide {
                flow_id,
                verdict: Verdict::AllowForSession,
            },
            Choice::Block => Step::Decide {
                flow_id,
                verdict: Verdict::Block,
            },
            Choice::Rule => Step::AskRule(flow_id),
            Choice::Edit => Step::Edit(flow_id),
            Choice::View => Step::ViewBody(flow_id),
            Choice::Next => self.skip(),
            Choice::Close => {
                self.detail = None;
                Step::Close
            }
        }
    }

    /// Die nächste gehaltene Anfrage, ohne zu entscheiden.
    fn skip(&mut self) -> Step {
        if self.queue.len() < 2 {
            return Step::Draw;
        }
        self.cursor = (self.cursor + 1) % self.queue.len();
        self.detail = None;
        Step::Fetch(self.queue[self.cursor].clone())
    }

    /// Die nächste Anfrage, wenn eine wartet.
    fn next_step(&mut self) -> Step {
        match self.queue.first() {
            Some(flow_id) => {
                self.cursor = 0;
                Step::Fetch(flow_id.clone())
            }
            None => Step::Idle,
        }
    }

    /// Ausgabe des Agenten.
    ///
    /// Steht kein Kasten, geht sie sofort hinaus. Steht einer, wird sie
    /// gehalten, bis der Deckel erreicht ist; dann geht sie hinaus und der
    /// Kasten wird darüber neu gezeichnet.
    pub fn output(&mut self, chunk: &[u8]) -> Option<Vec<u8>> {
        if self.detail.is_none() {
            return Some(chunk.to_vec());
        }
        self.buffer.extend_from_slice(chunk);
        if self.buffer.len() < OUTPUT_BUFFER_CAP {
            return None;
        }
        self.dropped = true;
        Some(std::mem::take(&mut self.buffer))
    }

    /// Alles, was der Puffer hält, und er ist danach leer.
    pub fn drain(&mut self) -> Vec<u8> {
        self.dropped = false;
        std::mem::take(&mut self.buffer)
    }

    /// Wahr, wenn der Puffer seit dem letzten [`Moderator::drain`] übergelaufen
    /// ist.
    #[must_use]
    pub fn overflowed(&self) -> bool {
        self.dropped
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn held() -> Held {
        Held {
            flow_id: "f1".to_owned(),
            request: "POST https://api.github.com/graphql".to_owned(),
            origin: "opencode - webfetch".to_owned(),
            size: "2.1 KB".to_owned(),
            findings: "1 - api_key.github in header Authorization".to_owned(),
            catalog: "GitHub API".to_owned(),
            host: "api.github.com".to_owned(),
            path: "/graphql".to_owned(),
            apex: "github.com".to_owned(),
            method: v1::Method::Post as i32,
        }
    }

    /// Jede Zeile ist genau so breit wie das Fenster.
    ///
    /// Der Kasten steht in einem Terminal im Rohmodus: Eine Zeile, die eine
    /// Spalte zu breit ist, bricht um, und jede folgende Zeile steht um eine
    /// Zeile verschoben -- der Zeilensalat, den das Kriterium ausschließt.
    #[test]
    fn every_line_is_exactly_the_width_of_the_window() {
        for width in [MIN_BOX_WIDTH, 60, 80, 132] {
            let lines = prompt_lines(&held(), (1, 2), 292, width);
            for line in &lines {
                assert_eq!(
                    line.chars().count(),
                    width,
                    "width {width}, line {line:?} in {lines:#?}"
                );
            }
        }
    }

    /// Eine Adresse, die länger ist als das Fenster, schiebt die Kante nicht.
    #[test]
    fn a_long_request_is_clamped_instead_of_pushing_the_edge() {
        let mut long = held();
        long.request = format!("GET https://example.com/{}", "a".repeat(400));
        let lines = prompt_lines(&long, (1, 1), 60, 60);
        assert!(lines.iter().all(|line| line.chars().count() == 60));
        assert!(
            lines.iter().any(|line| line.contains('~')),
            "the clamp says that something was left out: {lines:#?}"
        );
    }

    /// Was der Agent schickt, wird Text und keine Steuerfolge.
    #[test]
    fn an_escape_from_the_agent_never_reaches_the_terminal() {
        let mut evil = held();
        evil.request = "GET https://evil.test/\u{1b}]52;c;SGVsbG8=\u{7}".to_owned();
        evil.origin = "tool\u{1b}[2K\r[a] allow once".to_owned();
        let lines = prompt_lines(&evil, (1, 1), 10, 80);
        let text = lines.join("\n");
        assert!(!text.contains('\u{1b}'), "no escape: {text:?}");
        assert!(!text.contains('\r'), "no carriage return: {text:?}");
        assert!(!text.contains('\u{7}'), "no bell: {text:?}");
    }

    /// Ein Zeichen, das zwei Spalten breit wäre, steht nicht im Kasten.
    ///
    /// Sonst wäre eine Zeile in Zeichen genau richtig und auf dem Schirm zu
    /// breit; sie bräche um, und das Löschen vor dem nächsten Zeichnen ginge
    /// um zu wenige Zeilen hinauf.
    #[test]
    fn a_double_width_character_never_reaches_the_box() {
        let mut wide = held();
        wide.request = "GET https://例え.テスト/パス".to_owned();
        wide.origin = "🙂🙂🙂".to_owned();
        let lines = prompt_lines(&wide, (1, 1), 60, 80);
        let text = lines.join("\n");
        // Außer dem Rahmen selbst, dessen sechs Zeichen je eine Spalte
        // belegen, steht nichts außerhalb von ASCII im Kasten.
        let frame = [
            TOP_LEFT,
            TOP_RIGHT,
            BOTTOM_LEFT,
            BOTTOM_RIGHT,
            HORIZONTAL,
            VERTICAL,
        ];
        // Der Punkt der Kopfzeile gehört dieser Anwendung und belegt eine
        // Spalte; alles andere außerhalb von ASCII käme aus der Anfrage.
        for scalar in text.chars() {
            assert!(
                scalar.is_ascii() || frame.contains(&scalar) || scalar == '\n' || scalar == '·',
                "{scalar:?} is neither ASCII nor a part of the frame: {text:?}"
            );
        }
        assert!(lines.iter().all(|line| line.chars().count() == 80));
    }

    /// Die Uhr zählt in Minuten und Sekunden, mit führender Null.
    #[test]
    fn the_countdown_reads_like_a_clock() {
        assert_eq!(countdown(292), "04:52");
        assert_eq!(countdown(0), "00:00");
        assert_eq!(countdown(59), "00:59");
        assert_eq!(countdown(5400), "90:00");
    }

    /// Jede Taste der Spezifikation trifft ihre Wahl, groß wie klein.
    #[test]
    fn every_key_of_the_specification_has_its_choice() {
        assert_eq!(choice_of(b'a'), Some(Choice::AllowOnce));
        assert_eq!(choice_of(b'A'), Some(Choice::AllowOnce));
        assert_eq!(choice_of(b's'), Some(Choice::AllowSession));
        assert_eq!(choice_of(b'b'), Some(Choice::Block));
        assert_eq!(choice_of(b'r'), Some(Choice::Rule));
        assert_eq!(choice_of(b'e'), Some(Choice::Edit));
        assert_eq!(choice_of(b'v'), Some(Choice::View));
        assert_eq!(choice_of(b'n'), Some(Choice::Next));
        assert_eq!(choice_of(0x03), Some(Choice::Close));
        assert_eq!(choice_of(0x1b), Some(Choice::Close));
        assert_eq!(choice_of(b'x'), None);
        assert_eq!(choice_of(b'\n'), None);
    }

    /// Die Tastenzeile steht in jedem Kasten, auch im schmalsten.
    #[test]
    fn the_keys_stand_in_every_box() {
        let lines = prompt_lines(&held(), (1, 1), 10, MIN_BOX_WIDTH);
        let text = lines.join("\n");
        assert!(text.contains("[a] allow"), "{text}");
        assert!(text.contains("[b] block"), "{text}");
        assert!(text.contains("[r] rule"), "{text}");
        assert!(text.contains("[e] edit"), "{text}");
        assert!(text.contains("[v] view body"), "{text}");
        assert!(text.contains("[n] next"), "{text}");
    }

    fn held_with(flow_id: &str) -> Held {
        Held {
            flow_id: flow_id.to_owned(),
            ..held()
        }
    }

    /// Die erste gehaltene Anfrage öffnet den Kasten, die zweite nicht.
    ///
    /// Ein zweiter Kasten über dem ersten nähme einem Menschen die Frage weg,
    /// die er gerade beantwortet, und die Taste, die er gleich drückt, träfe
    /// eine andere Anfrage als die, die er gelesen hat.
    #[test]
    fn a_second_hold_waits_instead_of_taking_the_screen() {
        let mut moderator = Moderator::new();
        assert_eq!(moderator.held("f1"), Step::Fetch("f1".to_owned()));
        moderator.show(held_with("f1"));
        assert_eq!(moderator.held("f2"), Step::Draw);
        assert_eq!(moderator.shown().map(|h| h.flow_id.as_str()), Some("f1"));
        assert_eq!(moderator.position(), (1, 2));
    }

    /// Wer im Fenster entscheidet, nimmt dem Terminal die Frage ab.
    #[test]
    fn a_decision_elsewhere_closes_the_box_and_opens_the_next() {
        let mut moderator = Moderator::new();
        moderator.held("f1");
        moderator.show(held_with("f1"));
        moderator.held("f2");

        assert_eq!(moderator.decided("f1"), Step::Fetch("f2".to_owned()));
        assert!(!moderator.is_open(), "the box of the decided flow is gone");
        moderator.show(held_with("f2"));
        assert_eq!(moderator.position(), (1, 1));

        assert_eq!(moderator.decided("f2"), Step::Close);
        assert!(!moderator.is_open());
    }

    /// `n` geht weiter, ohne zu entscheiden.
    #[test]
    fn next_moves_on_without_deciding() {
        let mut moderator = Moderator::new();
        moderator.held("f1");
        moderator.show(held_with("f1"));
        moderator.held("f2");

        assert_eq!(moderator.key(b'n'), Step::Fetch("f2".to_owned()));
        moderator.show(held_with("f2"));
        assert_eq!(moderator.position(), (2, 2));
        // Und im Kreis zurück, statt am Ende stehen zu bleiben.
        assert_eq!(moderator.key(b'n'), Step::Fetch("f1".to_owned()));
    }

    /// Jede entscheidende Taste nennt den Fluss, den der Kasten zeigt.
    #[test]
    fn a_key_decides_the_flow_that_stands_in_the_box() {
        let mut moderator = Moderator::new();
        moderator.held("f1");
        moderator.show(held_with("f1"));
        assert_eq!(
            moderator.key(b'a'),
            Step::Decide {
                flow_id: "f1".to_owned(),
                verdict: Verdict::Allow
            }
        );
        assert_eq!(
            moderator.key(b's'),
            Step::Decide {
                flow_id: "f1".to_owned(),
                verdict: Verdict::AllowForSession
            }
        );
        assert_eq!(
            moderator.key(b'b'),
            Step::Decide {
                flow_id: "f1".to_owned(),
                verdict: Verdict::Block
            }
        );
        assert_eq!(moderator.key(b'v'), Step::ViewBody("f1".to_owned()));
        assert_eq!(moderator.key(b'e'), Step::Edit("f1".to_owned()));
        assert_eq!(moderator.key(b'r'), Step::AskRule("f1".to_owned()));
    }

    /// Der Zeiger bleibt in der Warteschlange, wenn woanders entschieden wird.
    ///
    /// Ohne das Nachführen stünde im Kopf „(2 of 1)", und `n` liefe im Kreis
    /// auf denselben Fluss.
    #[test]
    fn a_foreign_decision_keeps_the_position_true() {
        let mut moderator = Moderator::new();
        moderator.held("f1");
        moderator.show(held_with("f1"));
        moderator.held("f2");
        // Der zweite steht im Kasten, der erste wartet.
        moderator.key(b'n');
        moderator.show(held_with("f2"));
        assert_eq!(moderator.position(), (2, 2));

        // Jemand entscheidet den ersten im Fenster.
        assert_eq!(moderator.decided("f1"), Step::Draw);
        assert_eq!(
            moderator.position(),
            (1, 1),
            "the box shows the one that is left, and says so"
        );
    }

    /// Nach verworfenen Ereignissen gilt die Warteschlange des Dienstes.
    ///
    /// Der Rundfunk des Daemons hat einen Puffer, und dieser Befehl liest ihn
    /// nicht, solange er auf einen Editor oder eine zweite Taste wartet. Was
    /// dabei fällt, kann ein `Held` sein — eine Frage, die sonst niemand
    /// sähe, bis ihre Frist sie blockt.
    #[test]
    fn a_resync_takes_the_queue_of_the_daemon() {
        let mut moderator = Moderator::new();
        moderator.held("f1");
        moderator.show(held_with("f1"));

        // Der Dienst führt zwei, von denen einer neu ist, und der gezeigte
        // steht noch dabei.
        assert_eq!(
            moderator.resync(&["f1".to_owned(), "f2".to_owned()]),
            Step::Draw
        );
        assert_eq!(moderator.position(), (1, 2));

        // Und jetzt einer, der nicht mehr dabei ist: Der Kasten wechselt.
        assert_eq!(
            moderator.resync(&["f2".to_owned()]),
            Step::Fetch("f2".to_owned())
        );
        assert!(!moderator.is_open());

        // Nichts mehr gehalten: Der Kasten geht weg.
        moderator.show(held_with("f2"));
        assert_eq!(moderator.resync(&[]), Step::Close);
        assert!(!moderator.is_open());
    }

    /// Was der Kasten nicht angeht, lässt ihn stehen.
    ///
    /// Der Ereignisstrom trägt jede Antwort und jedes Stück einer Antwort.
    /// Nähme `Idle` den Kasten weg, verschwände er bei einer streamenden
    /// Antwort mehrmals je Sekunde.
    #[test]
    fn an_event_that_is_not_about_the_box_leaves_it_standing() {
        let mut moderator = Moderator::new();
        moderator.held("f1");
        moderator.show(held_with("f1"));
        // Eine Taste, die keine Wahl ist: derselbe Weg wie ein fremdes
        // Ereignis, und er zeichnet nichts.
        assert_eq!(moderator.key(b'x'), Step::Idle);
        assert!(moderator.is_open(), "the box is still there");
    }

    /// `Ctrl+C` ohne Kasten beendet die Sitzung.
    ///
    /// Im Rohmodus ist `ISIG` aus: Der Kernel schickt kein `SIGINT`, und ohne
    /// diesen Weg wäre `Ctrl+C` unter `--ask terminal` verschluckt.
    #[test]
    fn ctrl_c_without_a_box_ends_the_session() {
        let mut moderator = Moderator::new();
        assert_eq!(moderator.key(0x03), Step::Stop);
        // Steht ein Kasten, schließt dieselbe Taste nur ihn.
        moderator.held("f1");
        moderator.show(held_with("f1"));
        assert_eq!(moderator.key(0x03), Step::Close);
        assert!(!moderator.is_open());
    }

    /// Host und Pfad kommen aus der Struktur, nicht aus der Zeile.
    ///
    /// Aus der Zeile gelesen zerfiele `[::1]:8080` in `[`, und eine Abfrage im
    /// Pfad landete in einer Regel, die der Vergleich des Daemons nie trifft
    /// (`daemon/crates/rules/src/path.rs` schneidet sie ab).
    #[test]
    fn host_and_path_come_from_the_structure() {
        let detail = v1::FlowDetail {
            summary: Some(v1::FlowSummary {
                flow_id: "f1".to_owned(),
                method: v1::Method::Get as i32,
                scheme: v1::Scheme::Http as i32,
                authority: Some(v1::Authority {
                    host: "::1".to_owned(),
                    port: 8080,
                    ..v1::Authority::default()
                }),
                path: "/v1/models?page=2".to_owned(),
                ..v1::FlowSummary::default()
            }),
            ..v1::FlowDetail::default()
        };
        let held = from_detail(&detail);
        assert_eq!(held.host, "::1");
        assert_eq!(held.path, "/v1/models", "the query is not part of a rule");
        assert_eq!(held.method, v1::Method::Get as i32);
        assert!(held.apex.is_empty(), "an address has no apex");
    }

    /// Eine Taste ohne Kasten tut nichts.
    ///
    /// Ohne diesen Riegel entschiede ein `a`, das jemand tippt, während gerade
    /// nichts gehalten ist, die nächste Anfrage, die kommt.
    #[test]
    fn a_key_without_a_box_decides_nothing() {
        let mut moderator = Moderator::new();
        assert_eq!(moderator.key(b'a'), Step::Idle);
        moderator.held("f1");
        assert_eq!(
            moderator.key(b'a'),
            Step::Idle,
            "the box is not drawn before its detail is there"
        );
    }

    /// `Esc` schließt den Kasten, und die Anfrage bleibt gehalten.
    #[test]
    fn escape_closes_the_box_and_leaves_the_request_held() {
        let mut moderator = Moderator::new();
        moderator.held("f1");
        moderator.show(held_with("f1"));
        assert_eq!(moderator.key(0x1b), Step::Close);
        assert!(!moderator.is_open());
        // Der Fluss steht weiter in der Warteschlange: Niemand hat entschieden.
        assert_eq!(moderator.held("f1"), Step::Idle);
    }

    /// Ausgabe des Agenten geht durch, solange kein Kasten steht.
    #[test]
    fn output_passes_while_no_box_stands() {
        let mut moderator = Moderator::new();
        assert_eq!(moderator.output(b"hello"), Some(b"hello".to_vec()));
    }

    /// Steht ein Kasten, wartet die Ausgabe -- bis der Deckel erreicht ist.
    #[test]
    fn a_box_holds_the_output_until_the_cap() {
        let mut moderator = Moderator::new();
        moderator.held("f1");
        moderator.show(held_with("f1"));
        assert_eq!(moderator.output(b"quiet"), None);
        assert!(!moderator.overflowed());

        let flood = vec![b'x'; OUTPUT_BUFFER_CAP];
        let out = moderator.output(&flood).expect("the cap lets it through");
        assert_eq!(out.len(), OUTPUT_BUFFER_CAP + 5);
        assert!(
            moderator.overflowed(),
            "and it says so, because the box has to be drawn again"
        );
        assert!(moderator.drain().is_empty(), "the buffer is empty after");
        assert!(!moderator.overflowed());
    }

    /// Ein Feld ohne Inhalt steht nicht als leere Beschriftung da.
    #[test]
    fn an_empty_field_leaves_no_empty_label() {
        let bare = Held {
            flow_id: "f2".to_owned(),
            request: "GET https://example.com/".to_owned(),
            origin: String::new(),
            size: String::new(),
            findings: String::new(),
            catalog: String::new(),
            host: "example.com".to_owned(),
            path: "/".to_owned(),
            apex: "example.com".to_owned(),
            method: v1::Method::Get as i32,
        };
        let text = prompt_lines(&bare, (1, 1), 30, 80).join("\n");
        assert!(!text.contains("from:"), "{text}");
        assert!(!text.contains("findings:"), "{text}");
        assert!(!text.contains("catalog:"), "{text}");
    }
}
