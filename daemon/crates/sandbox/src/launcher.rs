//! Der Port [`SandboxBackend`] und sein Vokabular (CONVENTIONS.md 3.4).
//!
//! Ein Backend macht aus einer [`PlanRequest`] (Profil, Sitzung und
//! [`LaunchOptions`]) einen [`LaunchPlan`], startet ihn zu einem
//! [`SandboxHandle`] und liest aus der laufenden Sandbox die drei Garantien
//! als [`CheckResult`]. Daemon und Kommandozeile halten das Backend als
//! [`crate::Backend`], ein Enum über die Backends dieses Builds, gewählt zur
//! Laufzeit (HUM-428); was ein Start außer Profil und Sitzung braucht, steht
//! in den [`LaunchOptions`] und nicht mehr in Methoden eines konkreten Typs.
//! Die Mounttabelle der Oberfläche entsteht unter Linux noch aus den Flags
//! der Zeile des Sandbox-Backends (HUM-429 zieht sie in den Plan). Dass die
//! Kopplung an bwrap nicht weiter wächst, prüft `tools/check_coupling.py`
//! (HUM-145).
//!
//! Der Plan ist vollständig: `argv[0]` ist das Programm, alles danach kommt
//! vom Backend, und die Oberfläche zeigt die Liste wörtlich
//! ([`LaunchPlan::argv_line`]). Ein Plan startet genau eine Sandbox: er trägt
//! die Deskriptoren, die das Programm erbt, und die Leseseiten der Pipes, über
//! die die Sandbox zurückmeldet; beides ist beim zweiten Start nicht mehr da.

use std::ffi::OsString;
use std::io::PipeReader;
use std::os::fd::{OwnedFd, RawFd};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use humanitl_core::Diagnostic;
use humanitl_core::ids::SessionId;

use crate::bwrap_args::shell_line;
use crate::disposable::OverlayDirs;
use crate::handle::{OutputSink, SandboxHandle};
use crate::profile::{SandboxProfile, SessionContext};

/// Wie lange [`SandboxBackend::isolation_check`] auf den Bericht des Shims
/// wartet, wenn [`LaunchOptions::report_timeout`] nichts anderes sagt.
pub const REPORT_TIMEOUT: Duration = Duration::from_secs(5);

/// Ein Sandbox-Backend: plant, startet, prüft.
pub trait SandboxBackend: Send + Sync {
    /// Der Name des Backends, `"bwrap"` oder `"seatbelt"`.
    fn name(&self) -> &'static str;

    /// Wie stark dieses Backend die Sandbox vom Host trennt.
    fn isolation(&self) -> IsolationLevel;

    /// Ob diese Sitzung eine Wegwerf-Sandbox bekommen kann (HUM-357):
    /// dieselben Bedingungen, die der Start einer Wegwerf-Sandbox prüft, ohne
    /// etwas anzulegen. Die Auflösung von `sandbox.mode = "auto"` fragt sie
    /// vor dem Start ([`crate::resolve_mode`]).
    ///
    /// # Errors
    ///
    /// `SANDBOX_030` mit dem ersten fehlenden Teil und seinem Fix.
    fn disposable_support(&self, request: &PlanRequest<'_>) -> Result<(), Diagnostic>;

    /// Baut aus der Anfrage die vollständige Kommandozeile samt allem, was
    /// der Start braucht, und prüft dabei jede Vorbedingung, die sich ohne
    /// Start prüfen lässt.
    ///
    /// # Errors
    ///
    /// Ein [`Diagnostic`] mit registriertem Code; welche, sagt das Backend.
    fn plan(&self, request: &PlanRequest<'_>) -> Result<LaunchPlan, Diagnostic>;

    /// Startet den Plan. Kehrt zurück, sobald die Sandbox läuft oder sicher
    /// nicht läuft.
    ///
    /// # Errors
    ///
    /// Ein [`Diagnostic`], wenn das Programm nicht startet oder endet, bevor
    /// der Befehl in der Sandbox lief.
    fn launch(&self, plan: &LaunchPlan) -> Result<SandboxHandle, Diagnostic>;

    /// Liest aus der laufenden Sandbox, ob die drei Garantien gelten.
    ///
    /// Ein Ergebnis je [`IsolationCheck`], in der Reihenfolge der Varianten.
    /// Gewartet wird so lange, wie [`LaunchOptions::report_timeout`] beim
    /// Planen sagte: Die Frist reist mit dem Plan in das Handle, damit jede
    /// Prüfung unter den Bedingungen des Starts läuft, auch mit einer
    /// anderen Instanz des Backends.
    fn isolation_check(&self, handle: &SandboxHandle) -> Vec<CheckResult>;
}

/// Wie stark ein Backend die Sandbox vom Host trennt, geordnet: Eine
/// Untergrenze vergleicht mit `>=`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum IsolationLevel {
    /// Eine Richtlinie des Kerns über einem Prozess, der die echten Pfade
    /// sieht (Seatbelt unter macOS).
    Policy,
    /// Eigene Namensräume über dem geteilten Kern des Hosts (bwrap).
    Namespaces,
    /// Ein eigener Gastkern (geplant: Podman mit `krun`, HUM-444).
    Kernel,
}

impl IsolationLevel {
    /// Die drei, von schwach nach stark.
    pub const ALL: [Self; 3] = [Self::Policy, Self::Namespaces, Self::Kernel];

    /// Der Name in `snake_case`, wie ihn Konfiguration, Protokoll und
    /// Oberfläche führen. Stabil: Er wird ein Wert von
    /// `sandbox.min_isolation` (HUM-431).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Policy => "policy",
            Self::Namespaces => "namespaces",
            Self::Kernel => "kernel",
        }
    }
}

/// Die Schicht einer Wegwerf-Sandbox, in der Form, die das Backend nimmt
/// (HUM-350, HUM-374).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisposableLayer {
    /// Obere Schicht und Arbeitsverzeichnis eines Overlays über dem Projekt
    /// (bwrap).
    Overlay(OverlayDirs),
    /// Ein Klon des Projekts, der an dessen Stelle das Projekt der Sandbox
    /// ist (Seatbelt).
    Clone(PathBuf),
}

/// Was ein Start außer Profil und Sitzung braucht (HUM-428).
///
/// Bis HUM-428 waren das Builder-Methoden der konkreten Backends, und ein
/// Aufrufer musste deshalb den Typ kennen. Jetzt ist es ein Wert, den jedes
/// Backend gleich bekommt. Eine Wegwerf-Schicht in einer Form, die es nicht
/// nimmt, lehnt ein Backend ab, statt sie still zu übergehen.
#[derive(Debug, Clone, Default)]
pub struct LaunchOptions {
    /// Wohin Ein- und Ausgabe der Sandbox gehen; Standard
    /// [`StdioMode::Inherit`].
    pub stdio: StdioMode,
    /// Wohin jedes gelesene Stück Ausgabe zusätzlich geht, während die Sandbox
    /// läuft.
    ///
    /// Wirkt nur, wo es Ausgabe zu lesen gibt ([`StdioMode::Capture`],
    /// [`StdioMode::Pty`]). Der Sender steht vor dem Start fest und nicht
    /// danach, damit kein Byte zwischen Start und Anmeldung verloren geht.
    /// Wer auf das Ende des Kanals wartet, gibt ihn nur den Optionen des
    /// Starts und keiner Kopie, die gehalten wird: Ein gehaltener Sender
    /// schlösse den Kanal nie.
    pub output_sink: Option<OutputSink>,
    /// Wie lange [`SandboxBackend::isolation_check`] auf den Bericht des
    /// Shims wartet; ohne Angabe [`REPORT_TIMEOUT`].
    pub report_timeout: Option<Duration>,
    /// Der aufgelöste Ort, an dem der Aufrufer den Projektordner geprüft und
    /// freigegeben hat (HUM-294).
    ///
    /// Der Plan öffnet den Projektordner einmal (HUM-263) und nimmt ihn nur,
    /// wenn der Ort des Geöffneten genau dieser ist. Der Aufrufer prüft mehr
    /// als die Politik des Backends, etwa ob der Ordner unter dem
    /// Heimatverzeichnis liegt oder erklärt ist; ersetzt ein Verweis zwischen
    /// seiner Prüfung und dem Öffnen einen Teil des Pfades, führte derselbe
    /// Text sonst an einen Ort, den nur die Denylist gesehen hat. Ohne Angabe
    /// gilt allein die Politik.
    pub approved_work_dir: Option<PathBuf>,
    /// Gerät und Inode des Projektordners, den der Aufrufer schon geöffnet
    /// hält (HUM-350): Der Plan bindet den Ordner nur, wenn der dort
    /// geöffnete dieselben trägt. Der Aufrufer schreibt später durch seinen
    /// Deskriptor; ein Umbenennen und ein neuer Ordner unter demselben Pfad
    /// dazwischen hängte sonst einen anderen Ordner ein als den, in den
    /// übernommen wird. Seatbelt hängt nichts ein und liest das Feld nicht;
    /// dort ist der Klon der Ordner der Sandbox (HUM-374).
    pub expected_work_inode: Option<(u64, u64)>,
    /// Die Schicht, wenn dies eine Wegwerf-Sandbox ist. Ob ein Backend eine
    /// kann, sagt [`SandboxBackend::disposable_support`].
    pub disposable: Option<DisposableLayer>,
    /// Nur lesbare Binds, die der Aufrufer zur Sitzung beiträgt und nicht das
    /// Profil, als `(Quelle, Ziel)`: das Testverzeichnis der Escape-Tests
    /// (`--tests-dir`).
    ///
    /// Sie gehen denselben Weg wie die Einträge aus `mounts.extra_ro`
    /// (HUM-263): Der Plan öffnet die Quelle einmal, prüft den Ort ihres
    /// Deskriptors mit [`crate::MountPolicy::check_session_dir`] und bindet
    /// den Deskriptor, hinter den Erweiterungen des Profils und vor dem
    /// Projektordner. Geprüft wird wie beim Projektordner, denn auch diese
    /// Quelle kommt aus dem Aufruf und darf unter dem Heimatverzeichnis
    /// liegen; das Heimatverzeichnis selbst und die geschützten Bäume nicht.
    pub session_ro: Vec<(PathBuf, PathBuf)>,
}

impl LaunchOptions {
    /// Die Frist für den Bericht des Shims, mit der Vorgabe
    /// [`REPORT_TIMEOUT`].
    #[must_use]
    pub fn effective_report_timeout(&self) -> Duration {
        self.report_timeout.unwrap_or(REPORT_TIMEOUT)
    }

    /// Die Schicht eines Overlays, wenn eine gewählt ist.
    #[must_use]
    pub fn overlay(&self) -> Option<&OverlayDirs> {
        match &self.disposable {
            Some(DisposableLayer::Overlay(dirs)) => Some(dirs),
            Some(DisposableLayer::Clone(_)) | None => None,
        }
    }
}

/// Was ein Backend zum Planen bekommt.
#[derive(Debug, Clone, Copy)]
pub struct PlanRequest<'a> {
    /// Das geprüfte Profil.
    pub profile: &'a SandboxProfile,
    /// Die Sitzung, für die geplant wird.
    pub session: &'a SessionContext,
    /// Was der Start außerdem braucht.
    pub options: &'a LaunchOptions,
}

impl<'a> PlanRequest<'a> {
    /// Eine Anfrage aus ihren drei Teilen.
    #[must_use]
    pub const fn new(
        profile: &'a SandboxProfile,
        session: &'a SessionContext,
        options: &'a LaunchOptions,
    ) -> Self {
        Self {
            profile,
            session,
            options,
        }
    }
}

/// Wie die Umgebung des Befehls in die Sandbox kommt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EnvDelivery {
    /// Sie steht in `argv` (bwrap mit `--setenv`); das Programm selbst
    /// startet mit leerer Umgebung.
    Argv,
    /// Sie ist die Umgebung des Programms, das sie an den Befehl weiterreicht
    /// (`sandbox-exec`, HUM-323).
    Program,
}

/// Wie eine gestartete Sandbox beendet wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Teardown {
    /// `SIGTERM`, nach der Frist `SIGKILL` an den gestarteten Prozess. Mit
    /// ihm endet die ganze Sandbox: bwrap nimmt mit `--die-with-parent` den
    /// PID-Namensraum mit, und unter `sandbox-exec` ist der Prozess selbst
    /// die Sandbox.
    Process,
}

/// Die drei Garantien (BACKLOG.md 4.1, `docs/SECURITY.md` Abschnitt 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum IsolationCheck {
    /// Außer `lo` gibt es kein Netzwerk-Interface.
    NoNetworkInterface,
    /// Genau ein Unix-Socket führt hinein, der Proxy.
    SingleSocket,
    /// Der seccomp-Filter ist geladen und antwortet.
    SeccompActive,
}

impl IsolationCheck {
    /// Die drei, in der Reihenfolge des Berichts.
    pub const ALL: [Self; 3] = [
        Self::NoNetworkInterface,
        Self::SingleSocket,
        Self::SeccompActive,
    ];

    /// Der Name in `snake_case`, wie ihn Protokoll und Oberfläche führen.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoNetworkInterface => "no_network_interface",
            Self::SingleSocket => "single_socket",
            Self::SeccompActive => "seccomp_active",
        }
    }
}

/// Das Ergebnis einer Garantie-Prüfung.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckResult {
    /// Welche Garantie.
    pub check: IsolationCheck,
    /// Ob sie gilt.
    pub passed: bool,
    /// Was gesehen wurde, für die Anzeige.
    pub evidence: String,
    /// Der Befund, wenn sie nicht gilt oder nicht geprüft werden konnte.
    pub diagnostic: Option<Diagnostic>,
}

/// Wohin Ein- und Ausgabe der Sandbox gehen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StdioMode {
    /// Die drei Deskriptoren des Aufrufers, unverändert: die Kommandozeile,
    /// ein Terminal. Die Meldungen von `bwrap` landen dann dort, nicht im
    /// Befund.
    #[default]
    Inherit,
    /// Eingabe aus `/dev/null`, Ausgabe und Fehlerausgabe gesammelt
    /// ([`SandboxHandle::output`]). Für Prüfläufe, Tests und kurze Befehle.
    Capture,
    /// Alle drei Deskriptoren an einem Pseudoterminal, dessen Herrscherseite
    /// der Daemon behält (HUM-042).
    ///
    /// Das ist der Modus einer Sitzung, in der ein Mensch mit dem Agenten
    /// arbeitet: Er gibt dem Agenten ein Terminal — `isatty` ist wahr,
    /// `TERM` gilt, ein Vollbild-TUI läuft —, und der Daemon liest und
    /// schreibt die andere Seite ([`SandboxHandle::pty_master`],
    /// [`SandboxHandle::write_input`], [`SandboxHandle::resize`]).
    ///
    /// Was das gegenüber [`StdioMode::Capture`] ändert, und was daran hängt:
    ///
    /// - **Ein Strom statt zwei.** Ein Terminal hat keine getrennte
    ///   Fehlerausgabe; alles kommt als [`crate::OutputStream::Stdout`].
    /// - **Kein `EOF` auf der Eingabe.** Ein Agent, der von stdin liest,
    ///   wartet, statt sofort zu enden.
    /// - **`\n` wird zu `\r\n`.** Das ist die Zeilendisziplin des Terminals
    ///   und nicht der Agent.
    /// - **Die Startdiagnostik kommt aus demselben Strom.** Ohne getrennte
    ///   Fehlerausgabe fände `SandboxHandle::wait` die Meldung von `bwrap`
    ///   nicht; die ersten [`crate::handle::PTY_MIRROR_BYTES`] werden deshalb
    ///   gespiegelt.
    Pty {
        /// Spalten beim Start; der Client ändert sie später mit
        /// [`SandboxHandle::resize`].
        cols: u16,
        /// Zeilen beim Start.
        rows: u16,
    },
}

impl StdioMode {
    /// Die Geometrie, wenn dies ein Pseudoterminal ist.
    #[must_use]
    pub const fn pty_size(self) -> Option<(u16, u16)> {
        match self {
            Self::Pty { cols, rows } => Some((cols, rows)),
            Self::Inherit | Self::Capture => None,
        }
    }
}

/// Was beim Start einmal aus dem Plan genommen wird.
#[derive(Debug)]
pub(crate) struct LaunchOnce {
    /// Die Deskriptoren, die das Programm erbt, unter bwrap die memfds der
    /// Masken und die Schreibseiten der Pipes. Werden nach dem Start geschlossen.
    pub(crate) inherit: Vec<OwnedFd>,
    /// Die Leseseite der Berichts-Pipe des Shims.
    pub(crate) report: PipeReader,
    /// Die Leseseite der Status-Pipe des Programms, wenn es eine hat
    /// (`--json-status-fd` unter bwrap).
    pub(crate) status: Option<PipeReader>,
    /// Wer die Ausgabe mitliest ([`LaunchOptions::output_sink`]); geht mit
    /// dem einen Start mit.
    pub(crate) output_sink: Option<OutputSink>,
    /// Die Seite des Daemons an der Tür zu weiteren Shells (HUM-299), nur
    /// für einen Start am eigenen Pseudoterminal.
    pub(crate) shells: Option<crate::shell::DoorSeed>,
}

/// Die vollständige Kommandozeile samt allem, was der Start braucht.
#[derive(Debug)]
pub struct LaunchPlan {
    /// Das Verzeichnis der Sitzung, wenn das Backend eines anlegt (HUM-368).
    pub session_dir: Option<std::path::PathBuf>,
    /// Die ganze Liste, `argv[0]` ist das Programm. Wird wörtlich angezeigt.
    pub argv: Vec<OsString>,
    /// Die Umgebung des Befehls in der Sandbox, alphabetisch: dieselben Paare,
    /// die `--setenv` in `argv` trägt. Das Programm selbst startet mit leerer
    /// Umgebung.
    pub env: Vec<(String, String)>,
    /// Die Deskriptoren, die das Programm erbt, als `(host, ziel)`. Beide
    /// Nummern sind gleich: die Deskriptoren werden unter ihrer eigenen Nummer
    /// vererbt, nicht umnummeriert ([`crate::LaunchInputs`]); die Paarform
    /// bleibt, damit ein Backend, das umnummerieren muss, Platz hat.
    pub fds: Vec<(RawFd, RawFd)>,
    /// Die Dateien, die der Agent-Adapter in die Sandbox legt, in der
    /// Reihenfolge, in der sie in [`LaunchPlan::argv`] als `--ro-bind-data`
    /// stehen. Für Anzeige und Prüfung; der Inhalt liegt schon in den
    /// Deskriptoren aus [`LaunchPlan::fds`].
    pub files: Vec<crate::agent::SandboxFile>,
    /// Die Sitzung, für die der Plan gilt.
    pub session: SessionId,
    /// Der Name des Profils, für Protokoll und Thread-Namen.
    pub profile: String,
    /// Befunde, die den Start nicht verhindern, aber dazugehören: eine
    /// aufgehobene Maske (`SANDBOX_020`), ein Kernel ohne `openat2`
    /// (`SANDBOX_021`). Wer den Plan startet, gibt sie in den Ereignisstrom.
    pub warnings: Vec<Diagnostic>,
    /// Die Überdeckungen unter `/work`, die in diesem Lauf **fehlen**, weil es
    /// den Pfad auf dem Host nicht gibt (HUM-043).
    ///
    /// Beide Arten: die Verzeichnisse aus `mounts.tmpfs` und die Dateien aus
    /// `mounts.masked_files`. Die Pfade sind relativ zum Projektverzeichnis,
    /// also in demselben Raum wie die Einträge des Schnappschusses; nur so
    /// lässt sich eine Änderung einer fehlenden Überdeckung zuordnen
    /// (`crate::summary::SessionSummary::set_unprotected`).
    ///
    /// Was der Agent dort schreibt, landet im Projekt — als neue Datei, die im
    /// Diff des Laufs auftaucht. Die Zusammenfassung nennt die Liste, damit die
    /// Lücke benannt ist und nicht bloß vorhanden.
    pub unprotected: Vec<PathBuf>,
    /// Wie [`LaunchPlan::env`] in die Sandbox kommt.
    pub(crate) env_delivery: EnvDelivery,
    /// Wohin Ein- und Ausgabe gehen ([`LaunchOptions::stdio`]).
    pub(crate) stdio: StdioMode,
    /// Die Frist für den Bericht des Shims
    /// ([`LaunchOptions::effective_report_timeout`]).
    pub(crate) report_timeout: Duration,
    /// Wie die gestartete Sandbox beendet wird.
    pub(crate) teardown: Teardown,
    /// Das Programm, `argv[0]`, aufgelöst.
    pub(crate) program: PathBuf,
    /// Was der Start einmal entnimmt; danach `None`.
    pub(crate) once: Mutex<Option<LaunchOnce>>,
}

impl LaunchPlan {
    /// Das Programm, das startet: `argv[0]` als Pfad.
    #[must_use]
    pub fn program(&self) -> &Path {
        &self.program
    }

    /// Die ganze Liste als eine Zeile, nach POSIX zitiert, mit Programm.
    ///
    /// `shlex::split` der Zeile ergibt wieder [`LaunchPlan::argv`]. Ausführbar
    /// ist die Zeile nur mit den Deskriptoren aus [`LaunchPlan::fds`], die ein
    /// Terminal nicht hat; sie ist Anzeige, nicht Ausführung.
    #[must_use]
    pub fn argv_line(&self) -> String {
        shell_line(&self.argv)
    }

    /// Ob der Plan noch nicht gestartet wurde.
    #[must_use]
    pub fn is_fresh(&self) -> bool {
        self.once
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some()
    }

    /// Nimmt, was der Start braucht; beim zweiten Mal `None`.
    pub(crate) fn take_once(&self) -> Option<LaunchOnce> {
        self.once
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isolation_levels_are_ordered_from_policy_to_kernel() {
        assert!(IsolationLevel::Policy < IsolationLevel::Namespaces);
        assert!(IsolationLevel::Namespaces < IsolationLevel::Kernel);
        let mut sorted = [
            IsolationLevel::Kernel,
            IsolationLevel::Policy,
            IsolationLevel::Namespaces,
        ];
        sorted.sort();
        assert_eq!(sorted, IsolationLevel::ALL);
    }

    /// Die Namen werden Werte in `config.toml` und im Protokoll; ein
    /// umbenannter Wert bräche jede gespeicherte Untergrenze.
    #[test]
    fn isolation_level_names_are_stable() {
        let names: Vec<&str> = IsolationLevel::ALL
            .iter()
            .map(|level| level.as_str())
            .collect();
        assert_eq!(names, ["policy", "namespaces", "kernel"]);
    }

    #[test]
    fn default_options_wait_the_default_report_timeout() {
        let options = LaunchOptions::default();
        assert_eq!(options.effective_report_timeout(), REPORT_TIMEOUT);
        assert_eq!(options.stdio, StdioMode::Inherit);
        assert!(options.disposable.is_none() && options.session_ro.is_empty());
        let options = LaunchOptions {
            report_timeout: Some(Duration::from_millis(300)),
            ..LaunchOptions::default()
        };
        assert_eq!(
            options.effective_report_timeout(),
            Duration::from_millis(300)
        );
    }
}
