//! Der Port [`SandboxBackend`] und sein Vokabular (CONVENTIONS.md 3.4).
//!
//! Ein Backend macht aus Profil und Sitzung einen [`LaunchPlan`], startet ihn
//! zu einem [`SandboxHandle`] und liest aus der laufenden Sandbox die drei
//! Garantien als [`CheckResult`]. Der MVP hat ein Backend, `bwrap`
//! ([`crate::BwrapBackend`]); ein zweites (Docker, Seatbelt) berührt nichts
//! außerhalb dieser Crate.
//!
//! Der Plan ist vollständig: `argv[0]` ist das Programm, alles danach kommt
//! aus [`crate::SandboxProfile::to_bwrap_args`], und die Oberfläche zeigt die
//! Liste wörtlich ([`LaunchPlan::argv_line`]). Ein Plan startet genau eine
//! Sandbox: er trägt die Deskriptoren, die `bwrap` erbt, und die Leseseiten
//! der Pipes, über die die Sandbox zurückmeldet; beides ist beim zweiten
//! Start nicht mehr da.

use std::ffi::OsString;
use std::io::PipeReader;
use std::os::fd::{OwnedFd, RawFd};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use humanitl_core::Diagnostic;
use humanitl_core::ids::SessionId;

use crate::bwrap_args::shell_line;
use crate::handle::SandboxHandle;
use crate::profile::{SandboxProfile, SessionContext};

/// Ein Sandbox-Backend: plant, startet, prüft.
pub trait SandboxBackend: Send + Sync {
    /// Der Name des Backends, `"bwrap"`, später `"docker"`.
    fn name(&self) -> &'static str;

    /// Baut aus Profil und Sitzung die vollständige Kommandozeile samt allem,
    /// was der Start braucht, und prüft dabei jede Vorbedingung, die sich ohne
    /// Start prüfen lässt.
    ///
    /// # Errors
    ///
    /// Ein [`Diagnostic`] mit registriertem Code; welche, sagt das Backend.
    fn plan(
        &self,
        profile: &SandboxProfile,
        session: &SessionContext,
    ) -> Result<LaunchPlan, Diagnostic>;

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
    fn isolation_check(&self, handle: &SandboxHandle) -> Vec<CheckResult>;
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
    /// ([`SandboxHandle::output`]). Für Prüfläufe, Tests und kurze Befehle;
    /// ein Agent, der Stunden läuft, gehört an ein Terminal (HUM-042).
    Capture,
}

/// Was beim Start einmal aus dem Plan genommen wird.
#[derive(Debug)]
pub(crate) struct LaunchOnce {
    /// Die Deskriptoren, die `bwrap` erbt: das memfd der Masken und die
    /// Schreibseiten der Pipes. Werden nach dem Start geschlossen.
    pub(crate) inherit: Vec<OwnedFd>,
    /// Die Leseseite der Berichts-Pipe des Shims.
    pub(crate) report: PipeReader,
    /// Die Leseseite der Status-Pipe von `bwrap`.
    pub(crate) status: PipeReader,
}

/// Die vollständige Kommandozeile samt allem, was der Start braucht.
#[derive(Debug)]
pub struct LaunchPlan {
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
