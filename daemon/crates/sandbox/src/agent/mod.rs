//! Der Erweiterungspunkt für Agenten (`backlog/CONVENTIONS.md` 3.10).
//!
//! Humanitl startet nicht irgendein Programm, sondern einen bekannten Agenten
//! mit bekannten Eigenheiten. Was ein Agent braucht, damit er in der Sandbox
//! anläuft — sein Kommando, seine Umgebung, seine Konfigurationsdateien —, und
//! was er von sich aus ins Netz tut, weiß nur sein Adapter. Der Daemon kennt
//! ihn als [`AgentAdapter`] und fügt seine Beiträge in den `LaunchPlan` ein.
//!
//! Die Zusagen, die kein Compiler prüft und die jeder Adapter einhalten muss:
//!
//! 1. **Keine neue Tür.** Ein Adapter fügt keine Bridge und keine
//!    seccomp-Familie hinzu. Beides kommt aus dem Profil
//!    ([`crate::profile`]), und das Profil kennt genau eine Bridge.
//! 2. **Nichts nach `/work`.** Jede Datei aus [`AgentAdapter::files`] liegt
//!    außerhalb des Projektverzeichnisses; sonst schriebe Humanitl in ein
//!    Repository, das ihm nicht gehört, und der Agent könnte seine eigene
//!    Konfiguration umschreiben. [`SandboxFile::is_outside_work`] hält das
//!    fest, [`files_inside_work`] prüft es.
//! 3. **Kein Netz in der Vorprüfung.** [`AgentAdapter::preflight`] läuft auf
//!    dem Host, bevor irgendetwas startet. Es darf den Dateibaum ansehen und
//!    sonst nichts. Ob der LLM-Endpunkt antwortet, beantwortet die Probe aus
//!    HUM-039, nicht der Adapter.
//!
//! Der einzige Adapter des MVP ist [`opencode::OpenCodeAdapter`]; weitere
//! (Aider, Codex, Claude Code) kommen nach dem MVP und berühren diesen Kern
//! nicht.

pub mod opencode;
pub mod opencode_models;

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use humanitl_config::{Language, LlmConfig};
use humanitl_core::rule::Rule;
use humanitl_core::{Diagnostic, SessionId};

pub use crate::agent::opencode::OpenCodeAdapter;

/// Der Ort in der Sandbox, an dem das Projekt liegt.
///
/// Dasselbe wie [`crate::profile::WORK_DST`]; hier noch einmal als Pfad, damit
/// [`AgentContext::work_dir_sandbox`] einen Vorgabewert hat.
pub const WORK_DIR_SANDBOX: &str = crate::profile::WORK_DST;

/// Das Heimatverzeichnis des Agenten in der Sandbox.
///
/// Eine tmpfs, damit `~/.config` und `~/.local/share` des Agenten nicht auf dem
/// Host landen. Derselbe Wert wie [`crate::bwrap_args::DEFAULT_HOME`].
pub const AGENT_HOME: &str = crate::bwrap_args::DEFAULT_HOME;

/// Das Verzeichnis, in dem die Dateien der Adapter in der Sandbox liegen.
///
/// Unter `/etc`, nicht unter `/work` und nicht unter [`AGENT_HOME`]: der Agent
/// soll seine eigene Konfiguration lesen und nicht ändern können.
pub const AGENT_CONFIG_DIR: &str = "/etc/humanitl";

/// Rechte einer Datei, die der Agent nur lesen darf.
///
/// Der Wert dokumentiert die Absicht und wird nicht angewandt: die Dateien
/// kommen als `--ro-bind-data` in die Sandbox, und `bwrap` bestimmt den Modus
/// selbst (gemessen 0600, nur lesbar eingehängt). Wer den Modus wirklich
/// setzen will, braucht einen anderen Mechanismus als einen Deskriptor.
pub const MODE_READ_ONLY: u32 = 0o444;

/// Ziele, die kein Adapter belegen darf: die Sandbox setzt sie selbst.
///
/// Der Proxy-Socket, die CA, ihr Bündel, der Shim und die drei
/// Identitätsdateien. Eine Adapter-Datei mit demselben Ziel stünde in der
/// Argumentliste dahinter und verdeckte sie.
pub const RESERVED_FILE_TARGETS: &[&str] = &[
    crate::profile::PROXY_SOCKET_DST,
    crate::profile::CA_CERT_DST,
    crate::profile::CA_BUNDLE_DST,
    crate::profile::SHIM_DST,
    crate::bwrap_args::PASSWD_DST,
    crate::bwrap_args::GROUP_DST,
    crate::bwrap_args::HOSTS_DST,
];

/// Bäume, die kein Adapter belegen darf.
///
/// `/proc`, `/sys` und `/dev` gehören dem Kern, `/run/humanitl` der Sandbox
/// selbst.
pub const RESERVED_FILE_TREES: &[&str] = &["/proc", "/sys", "/dev", "/run/humanitl"];

/// Alles, was ein Adapter über die Sitzung wissen muss, für die er vorbereiten
/// soll.
///
/// `backlog/sprint-3.md` (HUM-037) nennt diesen Typ `SessionContext`. Der Name
/// ist in dieser Crate schon vergeben: [`crate::profile::SessionContext`] ist
/// der Kontext des Launchers (Host-Pfade von Socket, CA und Shim,
/// `backlog/CONVENTIONS.md` 4.12), und der Adapter braucht etwas anderes. Beide
/// Kontexte gehören zur selben Sitzung und entstehen nebeneinander im Daemon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentContext {
    /// Die Sitzung, zu der diese Sandbox gehört.
    pub session: SessionId,
    /// Das Projektverzeichnis auf dem Host.
    pub work_dir_host: PathBuf,
    /// Das Projektverzeichnis in der Sandbox, immer [`WORK_DIR_SANDBOX`].
    pub work_dir_sandbox: PathBuf,
    /// Der LLM-Endpunkt und seine Modelle aus der Konfiguration.
    ///
    /// `llm.models` leer heißt „kein Modell konfiguriert" und führt zu
    /// `LLM_004`; der Agent bekommt dann ein Platzhalter-Modell.
    pub llm: LlmConfig,
    /// Ersetzt die Kommandozeile des Adapters vollständig (`agent.command`).
    pub agent_command_override: Option<Vec<OsString>>,
    /// Der Port, auf dem die Bridge in der Sandbox lauscht
    /// ([`crate::profile::PROXY_PORT`]).
    pub proxy_port: u16,
    /// Das CA-Zertifikat in der Sandbox
    /// ([`crate::profile::CA_CERT_DST`]).
    pub ca_path_sandbox: PathBuf,
    /// Die Sprache des Nutzers, für Texte, die der Agent zu sehen bekommt.
    pub language: Language,
    /// Der Suchpfad des Hosts (`$PATH`), aus dem [`AgentAdapter::preflight`]
    /// das Kommando sucht.
    ///
    /// Der Wert wird hereingereicht, statt ihn hier aus der Prozessumgebung zu
    /// lesen: `humanitl_config::Env` ist die einzige Stelle, die die Umgebung
    /// liest (`backlog/CONVENTIONS.md` 4.11). Nebenbei kann ein Test damit
    /// einen Pfad ohne den Agenten vorgeben, ohne die Umgebung des Prozesses
    /// anzufassen.
    ///
    /// `None` heißt „Suchpfad unbekannt", nicht „Suchpfad leer". Die
    /// Vorprüfung sucht dann nicht und meldet nichts; ein Befund über ein
    /// fehlendes Programm braucht einen Beleg. Wer die Prüfung will, setzt den
    /// Pfad mit [`AgentContext::with_host_path`].
    pub host_path: Option<OsString>,
    /// Das Heimatverzeichnis des Agenten in der Sandbox.
    ///
    /// Aus `[env].HOME` des Profils beziehungsweise `sandbox.env`; Vorgabe
    /// [`AGENT_HOME`]. Der Adapter setzt `HOME` und die `XDG_*`-Variablen
    /// darauf und legt seine zweite Kopie der Konfiguration darunter ab; ein
    /// anderes `HOME` im Profil liefe sonst ins Leere.
    pub home: PathBuf,
    /// Host-Pfade, die die Sandbox unter demselben Pfad nur lesbar einhängt.
    ///
    /// Aus `[mounts].ro` und `[mounts].extra_ro` des Profils. Ein Programm des
    /// Hosts ist in der Sandbox nur erreichbar, wenn es darunter liegt; sonst
    /// scheitert das `exec` erst nach dem Start, und `AGENT_004` sagt das
    /// vorher. Leer heißt „nicht bekannt": die Vorprüfung schweigt dann,
    /// genau wie bei [`AgentContext::host_path`].
    pub sandbox_ro_paths: Vec<PathBuf>,
}

impl AgentContext {
    /// Ein Kontext mit den Vorgaben der Sandbox: `/work`, der Proxy-Port und
    /// die CA aus dem Profil.
    ///
    /// Alles Weitere setzt der Aufrufer; ohne `llm.endpoint` gibt es keine
    /// Passthrough-Regel, und ohne `models` läuft das Platzhalter-Modell.
    #[must_use]
    pub fn new(session: SessionId, work_dir_host: PathBuf, llm: LlmConfig) -> Self {
        Self {
            session,
            work_dir_host,
            work_dir_sandbox: PathBuf::from(WORK_DIR_SANDBOX),
            llm,
            agent_command_override: None,
            proxy_port: crate::profile::PROXY_PORT,
            ca_path_sandbox: PathBuf::from(crate::profile::CA_CERT_DST),
            language: Language::En,
            home: PathBuf::from(AGENT_HOME),
            host_path: None,
            sandbox_ro_paths: Vec::new(),
        }
    }

    /// Setzt `llm.models`.
    #[must_use]
    pub fn with_models(mut self, models: Vec<String>) -> Self {
        self.llm.models = models;
        self
    }

    /// Setzt den Override aus `agent.command`.
    #[must_use]
    pub fn with_command_override(mut self, command: Option<Vec<OsString>>) -> Self {
        self.agent_command_override = command;
        self
    }

    /// Setzt den Suchpfad des Hosts, aus dem die Vorprüfung das Kommando sucht.
    #[must_use]
    pub fn with_host_path(mut self, path: Option<OsString>) -> Self {
        self.host_path = path;
        self
    }

    /// Setzt die Sprache des Nutzers.
    #[must_use]
    pub const fn with_language(mut self, language: Language) -> Self {
        self.language = language;
        self
    }

    /// Setzt das Heimatverzeichnis des Agenten in der Sandbox.
    ///
    /// Ein leerer oder relativer Pfad wird abgelehnt und [`AGENT_HOME`]
    /// beibehalten: eine Konfiguration an einem relativen Ort fände der Agent
    /// nirgends wieder.
    #[must_use]
    pub fn with_home(mut self, home: PathBuf) -> Self {
        if home.is_absolute() {
            self.home = home;
        }
        self
    }

    /// Setzt die Host-Pfade, die die Sandbox nur lesbar einhängt.
    #[must_use]
    pub fn with_sandbox_ro_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.sandbox_ro_paths = paths;
        self
    }

    /// Wahr, wenn dieser Host-Pfad in der Sandbox unter demselben Pfad liegt.
    ///
    /// Verglichen wird der aufgelöste Pfad; ein Symlink, der aus einer
    /// eingehängten Quelle herausführt, zählt nicht als erreichbar.
    #[must_use]
    pub fn is_visible_in_sandbox(&self, host: &Path) -> bool {
        let host = crate::profile::normalize(host);
        self.sandbox_ro_paths
            .iter()
            .any(|root| host.starts_with(crate::profile::normalize(root)))
    }
}

/// Eine Datei, die der Daemon vor dem `exec` in die Sandbox schreibt.
///
/// Der Inhalt steht im Speicher und kommt über einen Deskriptor in die Sandbox
/// (`bwrap --file FD DST`), nie über einen Pfad auf dem Host. Damit liegt die
/// Datei auf einer tmpfs, überlebt die Sitzung nicht und lässt sich vom Agenten
/// nicht durch einen Symlink umlenken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxFile {
    /// Der Ort in der Sandbox, immer absolut.
    pub dst: PathBuf,
    /// Der Inhalt.
    pub content: Vec<u8>,
    /// Die Rechte, üblicherweise [`MODE_READ_ONLY`].
    pub mode: u32,
}

impl SandboxFile {
    /// Eine Datei, die der Agent nur lesen darf.
    #[must_use]
    pub fn read_only(dst: impl Into<PathBuf>, content: impl Into<Vec<u8>>) -> Self {
        Self {
            dst: dst.into(),
            content: content.into(),
            mode: MODE_READ_ONLY,
        }
    }

    /// Wahr, wenn die Datei außerhalb des Projektverzeichnisses liegt.
    ///
    /// Die zweite Zusage dieses Moduls. Ein Adapter, der nach `/work` schriebe,
    /// änderte ein Repository, das ihm nicht gehört, und der Agent könnte seine
    /// eigene Konfiguration mit einem Commit überschreiben.
    ///
    /// Verglichen wird der normalisierte Pfad: `/etc/../work/x` liegt unter
    /// `/work`, auch wenn es nicht danach aussieht.
    #[must_use]
    pub fn is_outside_work(&self, work_dir_sandbox: &Path) -> bool {
        let dst = crate::profile::normalize(&self.dst);
        dst.is_absolute() && !dst.starts_with(crate::profile::normalize(work_dir_sandbox))
    }

    /// Wahr, wenn die Datei ein Ziel überdeckt, das die Sandbox selbst setzt.
    ///
    /// Die Adapter-Dateien stehen in der Argumentliste nach dem Proxy-Socket,
    /// der CA und dem Shim; eine Datei mit demselben Ziel verdeckte sie. Heute
    /// kann das nicht vorkommen, weil es einen eingebauten Adapter mit
    /// Konstanten gibt — genau deshalb ist es billig, den Weg jetzt zu
    /// schließen, statt ihn dem zweiten Adapter zu überlassen.
    #[must_use]
    pub fn overlays_a_sandbox_path(&self) -> bool {
        let dst = crate::profile::normalize(&self.dst);
        RESERVED_FILE_TARGETS
            .iter()
            .any(|reserved| dst == crate::profile::normalize(Path::new(reserved)))
            || RESERVED_FILE_TREES
                .iter()
                .any(|tree| dst.starts_with(crate::profile::normalize(Path::new(tree))))
    }
}

/// Der Port, über den ein Agent in Humanitl hineinwächst.
///
/// Ein Adapter ist zustandslos: jede Methode bekommt den [`AgentContext`] und
/// gibt einen Wert zurück. Was er tut, ist damit für einen Test vollständig
/// beobachtbar, ohne dass eine Sandbox startet.
pub trait AgentAdapter: Send + Sync {
    /// Die Kennung, unter der der Adapter in `agent.adapter` steht.
    fn id(&self) -> &'static str;

    /// Das Kommando, das der Shim nach seccomp startet.
    ///
    /// Bei gesetztem [`AgentContext::agent_command_override`] genau dieses.
    fn command(&self, ctx: &AgentContext) -> Vec<OsString>;

    /// Die Umgebungsvariablen des Agenten, zusätzlich zum Env-Kit, das der
    /// Launcher setzt (`humanitl_proxy::ca::ENV_KIT`, HUM-014).
    fn env(&self, ctx: &AgentContext) -> Vec<(String, String)>;

    /// Die Dateien, die vor dem `exec` in der Sandbox liegen müssen.
    ///
    /// # Errors
    ///
    /// Ein [`Diagnostic`], wenn eine mitgelieferte Vorlage nicht die erwartete
    /// Form hat (`AGENT_003`). Das ist ein Fehler im Build, keine
    /// Nutzereingabe: die Vorlagen liegen unter `agents/` und sind
    /// einkompiliert.
    fn files(&self, ctx: &AgentContext) -> Result<Vec<SandboxFile>, Diagnostic>;

    /// Der mitgelieferte Regelsatz des Adapters, im Format von `rules.yaml`.
    ///
    /// Der Adapter liefert den Text, nicht die fertigen [`Rule`]-Werte:
    /// `humanitl-sandbox` darf laut `tools/deps-allow.toml` nicht von
    /// `humanitl-rules` abhängen, und ein zweiter YAML-Leser für Regeln wäre
    /// genau die Doppelung, die `docs/ARCHITECTURE.md` Abschnitt 4 verbietet.
    /// Der Daemon liest den Text mit `humanitl_rules::parse_rules` und stellt
    /// das Ergebnis vor die Regeln des Nutzers.
    fn default_rules(&self) -> &'static str;

    /// Die Passthrough-Regel für den LLM-Endpunkt des Nutzers (HUM-039).
    ///
    /// `None`, wenn `llm.endpoint` nicht gesetzt oder sein Host unlesbar ist:
    /// ohne Endpunkt gibt es nichts durchzulassen, und eine Regel auf einen
    /// erfundenen Host wäre schlimmer als keine.
    fn llm_passthrough(&self, llm: &LlmConfig) -> Option<Rule>;

    /// Vorprüfung auf dem Host, vor dem Start. Leer heißt: nichts gefunden.
    ///
    /// Fasst das Netz nicht an. Ein Befund mit
    /// [`humanitl_core::Severity::Blocking`] verhindert den Start.
    fn preflight(&self, ctx: &AgentContext) -> Vec<Diagnostic>;

    /// Wahr, wenn der Agent ein Vollbild-TUI ist.
    ///
    /// `humanitl run --ask terminal` verweigert dann den Dienst mit `CLI_002`
    /// und schlägt `--ask ui` oder `--ask none` vor: in einem Vollbild-TUI wäre
    /// die Frage nach einer Entscheidung nicht zu sehen
    /// (`backlog/CONVENTIONS.md` 4.10, HUM-067).
    fn is_fullscreen_tui(&self) -> bool;
}

/// Die eingebauten Adapter.
///
/// Eine Liste statt einer Map: es sind wenige, die Reihenfolge ist die der
/// Anzeige, und `agent.adapter` wird einmal pro Sitzung aufgelöst.
pub struct AdapterRegistry {
    adapters: Vec<Box<dyn AgentAdapter>>,
}

impl AdapterRegistry {
    /// Die Adapter, die mit Humanitl ausgeliefert werden.
    #[must_use]
    pub fn builtin() -> Self {
        Self {
            adapters: vec![Box::new(OpenCodeAdapter::new())],
        }
    }

    /// Eine Registry aus vorgegebenen Adaptern; für Tests.
    #[must_use]
    pub fn from_adapters(adapters: Vec<Box<dyn AgentAdapter>>) -> Self {
        Self { adapters }
    }

    /// Der Adapter zu einer Kennung.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&dyn AgentAdapter> {
        self.adapters
            .iter()
            .find(|adapter| adapter.id() == id)
            .map(AsRef::as_ref)
    }

    /// Die Kennungen aller Adapter, in der Reihenfolge der Liste.
    #[must_use]
    pub fn ids(&self) -> Vec<&'static str> {
        self.adapters.iter().map(|adapter| adapter.id()).collect()
    }

    /// Wahr, wenn die Registry keinen Adapter kennt.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.adapters.is_empty()
    }

    /// Die Anzahl der Adapter.
    #[must_use]
    pub fn len(&self) -> usize {
        self.adapters.len()
    }
}

impl Default for AdapterRegistry {
    /// Wie [`AdapterRegistry::builtin`].
    fn default() -> Self {
        Self::builtin()
    }
}

impl core::fmt::Debug for AdapterRegistry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AdapterRegistry")
            .field("ids", &self.ids())
            .finish()
    }
}

/// Prüft die zweite Zusage dieses Moduls für eine Liste von Dateien.
///
/// Gibt die Ziele zurück, die im Projektverzeichnis lägen. Eine leere Liste ist
/// der Normalfall; alles andere ist ein Fehler im Adapter, kein Zustand der
/// Sitzung, und wird im Test aufgedeckt statt zur Laufzeit stillschweigend
/// geschrieben.
#[must_use]
pub fn files_inside_work<'a>(files: &'a [SandboxFile], work_dir_sandbox: &Path) -> Vec<&'a Path> {
    files
        .iter()
        .filter(|file| !file.is_outside_work(work_dir_sandbox))
        .map(|file| file.dst.as_path())
        .collect()
}

/// Die Ziele, die ein Adapter nicht belegen darf.
///
/// Siehe [`SandboxFile::overlays_a_sandbox_path`]. Eine leere Liste ist der
/// Normalfall; alles andere ist ein Fehler im Adapter.
#[must_use]
pub fn files_on_reserved_targets(files: &[SandboxFile]) -> Vec<&Path> {
    files
        .iter()
        .filter(|file| file.overlays_a_sandbox_path())
        .map(|file| file.dst.as_path())
        .collect()
}

/// Sucht ein Kommando im Suchpfad des Hosts.
///
/// Ein Name ohne `/` wird in den Einträgen von `path` gesucht, ein Name mit `/`
/// unverändert geprüft. Zurück kommt der erste Treffer, der eine Datei ist.
/// Ohne `path` wird nur der Fall mit `/` beantwortet: diese Crate liest die
/// Prozessumgebung nicht.
#[must_use]
pub fn find_in_path(command: &OsStr, path: Option<&OsStr>) -> Option<PathBuf> {
    let candidate = Path::new(command);
    if candidate.components().count() > 1 {
        return candidate.is_file().then(|| candidate.to_path_buf());
    }

    let path = path?;
    std::env::split_paths(path)
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| dir.join(candidate))
        .find(|full| full.is_file())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::ffi::OsString;
    use std::path::{Path, PathBuf};

    use super::{SandboxFile, files_inside_work, find_in_path};

    #[test]
    fn files_outside_work_are_accepted() {
        let files = vec![
            SandboxFile::read_only("/etc/humanitl/opencode/opencode.json", b"{}".to_vec()),
            SandboxFile::read_only("/home/agent/.config/opencode/.keep", Vec::new()),
        ];
        assert!(files_inside_work(&files, Path::new("/work")).is_empty());
    }

    #[test]
    fn a_file_in_work_is_reported() {
        let files = vec![SandboxFile::read_only(
            "/work/opencode.json",
            b"{}".to_vec(),
        )];
        assert_eq!(
            files_inside_work(&files, Path::new("/work")),
            vec![Path::new("/work/opencode.json")]
        );
    }

    #[test]
    fn find_in_path_needs_a_path_for_a_bare_name() {
        assert_eq!(find_in_path(&OsString::from("opencode"), None), None);
    }

    #[test]
    fn find_in_path_finds_an_entry() {
        let dir = tempfile::tempdir().unwrap();
        let binary = dir.path().join("opencode");
        std::fs::write(&binary, b"#!/bin/sh\n").unwrap();
        let path = OsString::from(format!("/nonexistent:{}", dir.path().display()));
        assert_eq!(
            find_in_path(&OsString::from("opencode"), Some(&path)),
            Some(binary)
        );
    }

    #[test]
    fn find_in_path_takes_an_explicit_path_as_is() {
        let dir = tempfile::tempdir().unwrap();
        let binary = dir.path().join("opencode");
        std::fs::write(&binary, b"#!/bin/sh\n").unwrap();
        assert_eq!(
            find_in_path(binary.as_os_str(), None),
            Some(PathBuf::from(&binary))
        );
        assert_eq!(
            find_in_path(dir.path().join("missing").as_os_str(), None),
            None
        );
    }
}
