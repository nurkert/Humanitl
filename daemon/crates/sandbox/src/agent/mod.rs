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

pub mod briefing;
pub mod opencode;
pub mod opencode_models;

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use humanitl_config::{AgentBriefing, HoldConfig, Language, LlmConfig};
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
    /// Woher dieses Kommando kommt: aus `agent.command` oder von der
    /// Kommandozeile des Aufrufers.
    ///
    /// Ein Befund über ein Kommando, das der Aufrufer selbst hinter `--`
    /// geschrieben hat, darf nicht `agent.command` anfassen -- die Einstellung
    /// hat damit nichts zu tun, und ein Vorschlag, sie zu ändern, änderte auch
    /// nichts (HUM-135).
    pub agent_command_from_caller: bool,
    /// Der Port, auf dem die Bridge in der Sandbox lauscht
    /// ([`crate::profile::PROXY_PORT`]).
    pub proxy_port: u16,
    /// Das CA-Zertifikat in der Sandbox
    /// ([`crate::profile::CA_CERT_DST`]).
    pub ca_path_sandbox: PathBuf,
    /// Die Sprache des Nutzers, für Texte, die der Agent zu sehen bekommt.
    pub language: Language,
    /// Wie eine Anfrage angehalten und beantwortet wird (`hold`).
    ///
    /// Das Briefing nennt die Frist (`hold.timeout_secs`) und den Ask-Modus
    /// (`hold.ask_mode`), und beide Angaben müssen die sein, nach denen der
    /// Proxy tatsächlich arbeitet: ein Agent, dem eine falsche Frist genannt
    /// wird, bricht zu früh ab oder wartet auf eine Frage, die niemand stellt
    /// (HUM-071).
    pub hold: HoldConfig,
    /// Ob die Instruktionsdatei angelegt wird (`agent.briefing`).
    pub briefing: AgentBriefing,
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
    /// Das Konfigurationsverzeichnis, das der Agent wirklich sieht.
    ///
    /// `None` heißt: aus [`AgentContext::home`] abgeleitet, also
    /// `<home>/.config`. Etwas anderes steht hier, wenn `sandbox.env` ein
    /// eigenes `XDG_CONFIG_HOME` setzt — und das ist kein Sonderfall, sondern
    /// der Fall, der sonst still schiefgeht: `sandbox.env` wird **nach** dem
    /// Beitrag des Adapters in die Umgebung gelegt und gewinnt. Der Agent
    /// suchte seine Konfiguration und seine Einweisung dann in einem
    /// Verzeichnis, in das die Sandbox nichts eingehängt hat; die Dateien
    /// lägen da, würden aber nie gelesen, und niemand merkte es. Deshalb
    /// entscheidet dieser Wert, wohin [`AgentAdapter::files`] schreibt.
    ///
    /// Lies ihn über [`AgentContext::config_home`], nicht aus dem Feld.
    pub config_home: Option<PathBuf>,
    /// Was die Sandbox vom Dateibaum des Hosts sieht: die Einhängungen des
    /// Profils, seine Verweise und was es überdeckt
    /// ([`SandboxView::of_profile`]).
    ///
    /// Ob ein Programm des Hosts drinnen erreichbar ist, beantwortet
    /// [`SandboxView::resolve`] und nicht ein Namensvergleich: Ein Verweis des
    /// Profils führt in eine Einhängung hinein, ein tmpfs überdeckt eine, und
    /// der Projektbaum liegt drinnen unter einem anderen Namen. Ohne
    /// Einhängungen heißt es „nicht bekannt": die Vorprüfung schweigt dann,
    /// genau wie bei [`AgentContext::host_path`].
    pub sandbox_view: SandboxView,
    /// Der Suchpfad, der in der Sandbox gilt (`[env].PATH` des Profils).
    ///
    /// Ein nacktes Kommando löst der Agent in der Sandbox gegen diesen Pfad
    /// auf und nicht gegen den des Hosts. Die Vorprüfung muss deshalb dieselbe
    /// Frage stellen: Bis zum 2026-09-07 suchte sie nur in
    /// [`AgentContext::host_path`], fand dort einen Wrapper unter `$HOME`, den
    /// keine Einhängung deckt, und meldete `AGENT_004` für einen Start, der
    /// funktioniert hätte — das gemountete Verzeichnis stand im PATH der
    /// Sandbox (HUM-139).
    ///
    /// `None` heißt „Suchpfad der Sandbox unbekannt", nicht „leer": dann wird
    /// dort nicht gesucht, und es bleibt beim Weg über den Host.
    pub sandbox_path: Option<OsString>,
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
            agent_command_from_caller: false,
            proxy_port: crate::profile::PROXY_PORT,
            ca_path_sandbox: PathBuf::from(crate::profile::CA_CERT_DST),
            language: Language::En,
            hold: HoldConfig::default(),
            briefing: AgentBriefing::default(),
            home: PathBuf::from(AGENT_HOME),
            config_home: None,
            host_path: None,
            sandbox_view: SandboxView::default(),
            sandbox_path: None,
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
        self.agent_command_from_caller = false;
        self
    }

    /// Setzt das Kommando, das der Aufrufer hinter `--` genannt hat.
    ///
    /// Dasselbe Feld wie [`AgentContext::with_command_override`], aber mit der
    /// Herkunft: Die Vorprüfung prüft dann den Pfad, der wirklich startet, und
    /// ihre Befunde reden nicht von einer Einstellung, die niemand gesetzt hat.
    #[must_use]
    pub fn with_command_from_caller(mut self, command: Vec<OsString>) -> Self {
        self.agent_command_override = Some(command);
        self.agent_command_from_caller = true;
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

    /// Setzt die Halte-Einstellungen, aus denen das Briefing Frist und
    /// Ask-Modus nimmt.
    #[must_use]
    pub fn with_hold(mut self, hold: HoldConfig) -> Self {
        self.hold = hold;
        self
    }

    /// Setzt die Einstellungen der Instruktionsdatei (`agent.briefing`).
    #[must_use]
    pub fn with_briefing(mut self, briefing: AgentBriefing) -> Self {
        self.briefing = briefing;
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

    /// Setzt das Konfigurationsverzeichnis, das der Agent wirklich sieht.
    ///
    /// Ein relativer Pfad wird verworfen und der abgeleitete beibehalten: in
    /// der Sandbox gibt es kein Arbeitsverzeichnis, auf das sich ein relativer
    /// `XDG_CONFIG_HOME` verlässlich bezöge.
    #[must_use]
    pub fn with_config_home(mut self, config_home: Option<PathBuf>) -> Self {
        self.config_home = config_home.filter(|path| path.is_absolute());
        self
    }

    /// Das Konfigurationsverzeichnis des Agenten, abgeleitet oder gesetzt.
    #[must_use]
    pub fn config_home(&self) -> PathBuf {
        self.config_home
            .clone()
            .unwrap_or_else(|| self.home.join(".config"))
    }

    /// Setzt, was die Sandbox vom Dateibaum sieht
    /// ([`SandboxView::of_profile`]).
    #[must_use]
    pub fn with_sandbox_view(mut self, view: SandboxView) -> Self {
        self.sandbox_view = view;
        self
    }

    /// Setzt den Suchpfad, der in der Sandbox gilt (`[env].PATH` des Profils).
    #[must_use]
    pub fn with_sandbox_path(mut self, path: Option<OsString>) -> Self {
        self.sandbox_path = path;
        self
    }

    /// Setzt das Arbeitsverzeichnis, das die Sandbox wirklich hat
    /// (`[mounts].work.dst`, Vorgabe [`WORK_DIR_SANDBOX`]).
    ///
    /// Der Launcher setzt `--chdir` auf diesen Pfad; ein relativer Eintrag im
    /// Suchpfad der Sandbox zeigt dorthin. Ein leerer oder relativer Wert wird
    /// abgelehnt: Ein Arbeitsverzeichnis ohne Wurzel gäbe es drinnen nicht.
    #[must_use]
    pub fn with_work_dir_sandbox(mut self, dst: PathBuf) -> Self {
        if dst.is_absolute() {
            self.work_dir_sandbox = dst;
        }
        self
    }

    /// Der Suchpfad der Sandbox, wie er wirklich gilt, ohne jede Erklärung.
    ///
    /// Für alles, was jemand kopiert und einträgt: In einem
    /// [`humanitl_core::FixAction`] stünde sonst der erklärende Zusatz aus
    /// [`AgentContext::sandbox_path_display`] mitten in der Konfiguration des
    /// Nutzers.
    #[must_use]
    pub fn sandbox_path_value(&self) -> String {
        self.sandbox_path.as_ref().map_or_else(
            || DEFAULT_SANDBOX_PATH.to_owned(),
            |path| path.to_string_lossy().into_owned(),
        )
    }

    /// Der Suchpfad der Sandbox als Text, für Befunde.
    ///
    /// `unknown`, wenn der Aufrufer ihn nicht hereingereicht hat: Ein Befund
    /// nennt, was er weiß, und behauptet keinen leeren Pfad.
    #[must_use]
    pub fn sandbox_path_display(&self) -> String {
        self.sandbox_path.as_ref().map_or_else(
            || format!("{DEFAULT_SANDBOX_PATH} (no PATH set, the C library falls back to this)"),
            |path| path.to_string_lossy().into_owned(),
        )
    }

    /// Sucht ein nacktes Kommando im Suchpfad der Sandbox und gibt den Pfad
    /// zurück, unter dem die Datei auf dem **Host** liegt.
    ///
    /// Die Kurzform von [`AgentContext::look_up_in_sandbox_path`] für den
    /// Fall, dass nur der startbare Treffer zählt; `None` heißt „nicht
    /// startbar", gleich aus welchem der drei Gründe. Die Vorprüfung selbst
    /// nimmt die lange Form, weil sie die Gründe auseinanderhalten muss.
    #[must_use]
    pub fn resolve_in_sandbox_path(&self, command: &OsStr) -> Option<PathBuf> {
        match self.look_up_in_sandbox_path(command) {
            SandboxLookup::Startable(path) => Some(path),
            SandboxLookup::NotExecutable(_) | SandboxLookup::Missing | SandboxLookup::Unknown => {
                None
            }
        }
    }

    /// Schlägt ein Kommando so nach, wie die Sandbox es nachschlägt.
    ///
    /// Drei Arten von Einträgen, am 2026-09-13 an einer echten Sandbox
    /// gemessen (`bwrap … --chdir /work`, `execvp` im Shim):
    ///
    /// - Ein absoluter Eintrag ist ein Pfad **der Sandbox** und wird über
    ///   [`AgentContext::search_view`] aufgelöst: Er zählt, wenn eine
    ///   Einhängung ihn deckt oder ein Verweis des Profils dorthin führt.
    ///   Gemessen: mit `PATH=/bin` und dem Verweis `usr/bin /bin` des Profils
    ///   startete `/bin/env` in der Sandbox.
    /// - Ein relativer Eintrag wird beim `exec` gegen das Arbeitsverzeichnis
    ///   der Sandbox aufgelöst ([`AgentContext::work_dir_sandbox`], `--chdir`).
    ///   Gemessen: `PATH=bin` startete `/work/bin/opencode`.
    /// - Ein leerer Eintrag ist nach POSIX das Arbeitsverzeichnis. Gemessen:
    ///   `PATH=:/usr/bin` startete `/work/opencode`.
    ///
    /// Ein Kommando **mit** Trennzeichen wird nicht im Suchpfad gesucht,
    /// sondern relativ zum Arbeitsverzeichnis (`agent.command =
    /// ["./bin/opencode"]` startet drinnen `/work/bin/opencode`); ein
    /// absolutes gar nicht, denn dessen Pfad gilt drinnen wie draußen, und
    /// darüber entscheidet [`AgentContext::reaches_program`].
    ///
    /// Die Reihenfolge der Ergebnisse ist Absicht: Ein startbarer Treffer
    /// gewinnt, dann zählt Unentscheidbares, dann „da, aber nicht ausführbar",
    /// zuletzt „nicht da". Wer über einem unlesbaren Eintrag einen Befund
    /// erhöbe, behauptete etwas über ein Verzeichnis, in das er nicht sehen
    /// konnte.
    #[must_use]
    pub fn look_up_in_sandbox_path(&self, command: &OsStr) -> SandboxLookup {
        let named = Path::new(command);
        if named.is_absolute() {
            return SandboxLookup::Missing;
        }
        let view = self.search_view();
        if named.components().count() > 1 {
            return view.look_up(&self.work_dir_sandbox.join(named));
        }
        // Ohne `[env].PATH` im Profil hat die Sandbox keinen Suchpfad, und
        // `execvp` nimmt die Vorgabe der C-Bibliothek. Am 2026-09-13 gemessen:
        // `bwrap … --clearenv -- env echo` läuft, und `getconf PATH` drinnen
        // sagt `/bin:/usr/bin`.
        let path = self
            .sandbox_path
            .clone()
            .unwrap_or_else(|| OsString::from(DEFAULT_SANDBOX_PATH));
        let mut unknown = false;
        let mut dead: Option<PathBuf> = None;
        for entry in std::env::split_paths(&path) {
            match view.look_up(&self.entry_in_sandbox(&entry).join(command)) {
                SandboxLookup::Startable(found) => return SandboxLookup::Startable(found),
                SandboxLookup::Unknown => unknown = true,
                SandboxLookup::NotExecutable(found) => dead = dead.or(Some(found)),
                SandboxLookup::Missing => {}
            }
        }
        if unknown {
            return SandboxLookup::Unknown;
        }
        dead.map_or(SandboxLookup::Missing, SandboxLookup::NotExecutable)
    }

    /// Der Eintrag des Suchpfads als Pfad in der Sandbox.
    ///
    /// Leer und relativ zeigen auf das Arbeitsverzeichnis, absolut auf sich
    /// selbst.
    fn entry_in_sandbox(&self, entry: &Path) -> PathBuf {
        if entry.as_os_str().is_empty() {
            self.work_dir_sandbox.clone()
        } else if entry.is_absolute() {
            entry.to_path_buf()
        } else {
            self.work_dir_sandbox.join(entry)
        }
    }

    /// Die Sicht für die Suche: die des Profils und zusätzlich der
    /// Projektbaum an seinem Ort in der Sandbox.
    ///
    /// Der Projektbaum steht nicht in [`AgentContext::sandbox_view`], weil er
    /// drinnen einen anderen Namen trägt (`/work`); jene Sicht beantwortet die
    /// Frage, ob ein Pfad des Hosts drinnen derselbe ist.
    #[must_use]
    pub fn search_view(&self) -> SandboxView {
        self.sandbox_view.clone().with_mount(Mount::new(
            self.work_dir_host.clone(),
            self.work_dir_sandbox.clone(),
        ))
    }

    /// Wahr, wenn die Sandbox dieses Programm unter demselben Pfad erreicht.
    ///
    /// Entschieden wird Schritt für Schritt, wie in der Sandbox selbst
    /// ([`SandboxView::resolve`]), nicht am kanonischen Pfad des Hosts: Eine
    /// Kette von Verweisen, deren Zwischenschritt die Einhängungen verlässt,
    /// landet auf dem Host wieder drinnen und drinnen im Nichts. Am 2026-09-13
    /// gemessen: `readlink -f` endet unter der Einhängung, `exec` in der
    /// Sandbox endet mit 127.
    ///
    /// Unentscheidbares zählt als erreichbar; ein Befund braucht einen Beleg.
    #[must_use]
    pub fn reaches_program(&self, host: &Path) -> bool {
        self.sandbox_view.reaches(host)
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

    /// Das Programm, das dieser Adapter begleitet, ohne Pfad.
    ///
    /// Daran wird entschieden, ob ein genanntes Kommando der Agent dieses
    /// Adapters ist ([`AgentAdapter::is_agent_command`]).
    fn program(&self) -> &'static str;

    /// Wahr, wenn dieses Kommando der Agent dieses Adapters ist.
    ///
    /// Ein leeres Kommando ist er immer: Dann setzt der Adapter es selbst.
    /// Sonst entscheidet der Dateiname, nicht der Pfad -- `opencode`,
    /// `/usr/local/bin/opencode` und `./opencode` sind derselbe Agent, und wer
    /// `bash` startet, ist keiner.
    ///
    /// **Warum am Kommando und nicht daran, ob eines genannt wurde.** Bis zum
    /// 2026-09-07 trug der Adapter nur bei, wenn niemand ein Kommando nannte;
    /// `humanitl run -- opencode` -- die Zeile, die ein Mensch schreibt --
    /// startete den Agenten damit ohne seine Konfiguration: ohne den Anbieter
    /// für die Durchreiche, ohne den mitgelieferten Modellkatalog, ohne die
    /// abgeschaltete Sitzungsfreigabe (HUM-135).
    fn is_agent_command(&self, command: &[std::ffi::OsString]) -> bool {
        let Some(first) = command.first() else {
            return true;
        };
        std::path::Path::new(first)
            .file_name()
            .is_some_and(|name| name == self.program())
    }

    /// Wahr, wenn der Agent ein Vollbild-TUI ist.
    ///
    /// `humanitl run --ask terminal` verweigert dann den Dienst mit `CLI_002`
    /// und schlägt `--ask ui` oder `--ask none` vor: in einem Vollbild-TUI wäre
    /// die Frage nach einer Entscheidung nicht zu sehen
    /// (`backlog/CONVENTIONS.md` 4.10, HUM-067).
    fn is_fullscreen_tui(&self) -> bool;
}

/// Wahr, wenn dieses Kommando der Agent dieser Konfiguration ist.
///
/// Eine Stelle für beide Seiten: Der Daemon entscheidet damit, ob der Adapter
/// beiträgt, und `humanitl sandbox argv` zeigt dasselbe. Standen die beiden
/// auseinander, zeigte die Vorschau eine Umgebung, die der Start nicht baut
/// (HUM-135).
///
/// Zwei Namen zählen: der des Adapters (`opencode`) und der aus
/// `agent.command`, denn wer sein Programm dort benennt, startet mit
/// `humanitl run` genau dieses.
#[must_use]
pub fn command_is_the_agent(config: &humanitl_config::Config, command: &[OsString]) -> bool {
    if AdapterRegistry::builtin()
        .get(&config.agent.adapter)
        .is_some_and(|adapter| adapter.is_agent_command(command))
    {
        return true;
    }
    let Some(own) = config
        .agent
        .command
        .as_ref()
        .and_then(|parts| parts.first())
    else {
        return false;
    };
    let Some(named) = command.first() else {
        return false;
    };
    std::path::Path::new(named).file_name() == std::path::Path::new(own).file_name()
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

/// Wahr, wenn dieser Pfad auf eine Datei zeigt, die dieser Prozess starten
/// kann.
///
/// Gefragt wird mit `access(EXEC_OK)` und nicht am Modus: `mode & 0o111` hieße
/// „irgendwer darf", und eine Datei, die nur das Bit für andere trägt, ließe
/// sich von diesem Nutzer nicht starten. Dieselbe Prüfung wie in
/// `BwrapBackend::find_program` und im Doctor. Ein Verzeichnis zählt nicht:
/// `exec` braucht eine Datei, und ein Verzeichnis trägt das Bit fast immer.
#[must_use]
pub fn is_executable_file(path: &Path) -> bool {
    path.is_file() && rustix::fs::access(path, rustix::fs::Access::EXEC_OK).is_ok()
}

/// Der Suchpfad, den die C-Bibliothek nimmt, wenn keiner gesetzt ist.
///
/// `confstr(_CS_PATH)`; am 2026-09-13 in der Sandbox gemessen (`getconf PATH`
/// nach `--clearenv`). Ein Profil ohne `[env].PATH` ist gültig, und `execvp`
/// sucht dann trotzdem — über den Verweis `/bin` des Profils in die
/// Einhängung `/usr` hinein.
pub const DEFAULT_SANDBOX_PATH: &str = "/bin:/usr/bin";

/// Wie viele Verweise eine Auflösung folgen darf, bevor sie aufgibt.
///
/// Derselbe Deckel wie `ELOOP` im Kern (40).
const MAX_SYMLINK_HOPS: usize = 40;

/// Ein Baum des Hosts an seinem Ort in der Sandbox.
///
/// Quelle und Ziel sind bei den Nur-Lese-Einhängungen derselbe Pfad; beim
/// Projektbaum nicht: Er liegt auf dem Host irgendwo und in der Sandbox unter
/// `/work`. Die Unterscheidung ist keine Feinheit, sondern der Unterschied
/// zwischen zwei Verweisen, die gleich aussehen: `bin/opencode` auf
/// `/work/tools/opencode` startet in der Sandbox, `bin/opencode` auf
/// `/home/u/proj/tools/opencode` nicht, weil es `/home/u/proj` drinnen nicht
/// gibt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mount {
    /// Der Baum auf dem Host.
    pub src: PathBuf,
    /// Sein Ort in der Sandbox.
    pub dst: PathBuf,
}

impl Mount {
    /// Eine Einhängung, die auf dem Host und in der Sandbox denselben Pfad hat.
    #[must_use]
    pub fn same_path(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        Self {
            src: path.clone(),
            dst: path,
        }
    }

    /// Eine Einhängung mit eigenem Ziel.
    #[must_use]
    pub fn new(src: impl Into<PathBuf>, dst: impl Into<PathBuf>) -> Self {
        Self {
            src: src.into(),
            dst: dst.into(),
        }
    }
}

/// Was aus einem Pfad in der Sandbox wird.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reach {
    /// Die Sandbox erreicht ihn; hier liegt er auf dem Host.
    Host(PathBuf),
    /// In der Sandbox gibt es ihn nicht.
    Absent,
    /// Unentscheidbar: Der Host gibt über eine Stelle des Weges keine Auskunft.
    ///
    /// Nicht dasselbe wie [`Reach::Absent`], und deshalb ein eigener Fall: Wer
    /// aus einer unlesbaren Stelle einen blockierenden Befund macht, verbietet
    /// einen Start wegen einer Behauptung, die er nicht belegen kann
    /// (`backlog/CONVENTIONS.md` 4.13).
    Unknown,
}

/// Was das Nachschlagen einer Datei ergeben hat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxLookup {
    /// Es gibt sie in der Sandbox nicht.
    Missing,
    /// Die Sandbox kann sie starten; hier liegt sie auf dem Host.
    Startable(PathBuf),
    /// Sie ist da, und dieser Nutzer darf sie nicht ausführen.
    NotExecutable(PathBuf),
    /// Unentscheidbar; es wird nichts behauptet.
    Unknown,
}

/// Was die Sandbox vom Dateibaum des Hosts sieht.
///
/// Zwei Quellen, beide aus dem Profil: die Bäume, die eingehängt sind
/// (`[mounts].ro`, `[mounts].extra_ro`, dazu der Projektbaum der Sitzung), und
/// die Verweise, die die Sandbox selbst anlegt (`[mounts].symlinks`). Mehr
/// braucht die Frage nicht, die dieser Typ beantwortet: Findet ein `exec` in
/// der Sandbox diesen Pfad, und wo liegt er dann auf dem Host?
///
/// **Gerechnet wird in Pfaden der Sandbox.** Ein Verweis trägt den Text seines
/// Ziels, und den liest der Kern drinnen, nicht draußen; erst nach der
/// Auflösung wird auf den Host abgebildet.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SandboxView {
    /// Die Bäume, die in der Sandbox ankommen.
    pub mounts: Vec<Mount>,
    /// Die Verweise des Profils als Paare aus Ort und Ziel, beide in Pfaden
    /// der Sandbox; das Ziel ist relativ zum Elternverzeichnis des Verweises
    /// oder absolut.
    pub symlinks: Vec<(PathBuf, PathBuf)>,
    /// Orte in der Sandbox, die etwas anderes überdeckt: `[mounts].tmpfs` und
    /// die Masken aus `[mounts].masked_files`.
    ///
    /// Sie liegen **über** den Einhängungen. Was der Host darunter hat, ist
    /// drinnen nicht zu sehen: Das ausgelieferte Profil legt ein tmpfs über
    /// `/work/.direnv` und `/work/.git/hooks`, und ein Kommando von dort
    /// scheiterte drinnen mit 127, während der Host es findet (HUM-043,
    /// HUM-139).
    pub covered: Vec<PathBuf>,
}

impl SandboxView {
    /// Eine Sicht aus Einhängungen und Verweisen.
    #[must_use]
    pub fn new(mounts: Vec<Mount>, symlinks: Vec<(PathBuf, PathBuf)>) -> Self {
        Self {
            mounts,
            symlinks,
            covered: Vec::new(),
        }
    }

    /// Eine Sicht, deren Einhängungen auf beiden Seiten denselben Pfad haben.
    #[must_use]
    pub fn same_path(roots: Vec<PathBuf>, symlinks: Vec<(PathBuf, PathBuf)>) -> Self {
        Self::new(roots.into_iter().map(Mount::same_path).collect(), symlinks)
    }

    /// Die Sicht, die dieses Profil beschreibt: seine Nur-Lese-Einhängungen
    /// und seine Verweise.
    ///
    /// Der Projektbaum gehört nicht dazu; er kommt aus der Sitzung und geht
    /// über [`SandboxView::with_mount`] dazu.
    #[must_use]
    pub fn of_profile(profile: &crate::profile::SandboxProfile) -> Self {
        Self::same_path(
            profile
                .mounts
                .ro
                .iter()
                .chain(&profile.mounts.extra_ro)
                // Schreibbar eingehängt ist genauso eingehängt: `extra_rw`
                // kommt als `--bind path path` in die Sandbox und ist dort
                // unter demselben Namen da. Fehlte es hier, verböte die
                // Vorprüfung einen Start, der läuft (HUM-139).
                .chain(&profile.mounts.extra_rw)
                .cloned()
                .collect(),
            profile
                .mounts
                .symlinks
                .iter()
                .map(|link| (link.link.clone(), PathBuf::from(&link.target)))
                .collect(),
        )
        .with_covered(
            profile
                .mounts
                .tmpfs
                .iter()
                .cloned()
                .chain(profile.effective_masked_files())
                .collect(),
        )
    }

    /// Dieselbe Sicht mit einer Einhängung mehr.
    #[must_use]
    pub fn with_mount(mut self, mount: Mount) -> Self {
        self.mounts.push(mount);
        self
    }

    /// Dieselbe Sicht mit Orten, die etwas anderes überdeckt.
    #[must_use]
    pub fn with_covered(mut self, covered: Vec<PathBuf>) -> Self {
        self.covered = covered;
        self
    }

    /// Löst einen Pfad **der Sandbox** so auf, wie die Sandbox ihn auflöst.
    ///
    /// Schritt für Schritt, weil nur so die Zwischenschritte geprüft sind und
    /// `..` an der richtigen Stelle wirkt. Der kanonische Pfad des Hosts
    /// beantwortet die Frage nicht: Eine Kette wie `/mnt/opencode` auf
    /// `/außerhalb/link` auf `/mnt/echt` endet für `readlink -f` unter der
    /// Einhängung und in der Sandbox im Nichts. Am 2026-09-13 gemessen: `exec`
    /// endet mit 127.
    ///
    /// Die Fälle je Pfadstück:
    ///
    /// 1. `..` geht einen Schritt zurück — **nach** den Verweisen davor, wie
    ///    im Kern. `/m/link/../real` folgt erst `link`.
    /// 2. Ein Verweis des Profils wird ersetzt.
    /// 3. Ein Stück, das zu einer Einhängung hinführt oder ihr Ziel ist, gilt
    ///    als Verzeichnis: `bwrap` legt es selbst an. Ein Verweis des Hosts an
    ///    dieser Stelle (`/home` auf `/var/home`) zählt nicht.
    /// 4. Unterhalb einer Einhängung entscheidet der Host, auch über seine
    ///    Verweise; ihr Ziel gilt wieder in Pfaden der Sandbox.
    /// 5. Alles andere gibt es in der Sandbox nicht.
    #[must_use]
    pub fn resolve(&self, in_sandbox: &Path) -> Reach {
        if !in_sandbox.is_absolute() {
            return Reach::Absent;
        }
        let mut rest = parts_of(in_sandbox);
        let mut at = PathBuf::from("/");
        let mut hops = 0_usize;
        while let Some(part) = rest.pop() {
            if part == OsStr::new("..") {
                at.pop();
                continue;
            }
            let next = at.join(part);
            if let Some(target) = self.symlink_at(&next) {
                hops += 1;
                if hops > MAX_SYMLINK_HOPS {
                    return Reach::Unknown;
                }
                follow(&mut rest, &mut at, &target);
                continue;
            }
            // Überdeckt: Darunter liegt in der Sandbox ein leeres tmpfs oder
            // eine leere Datei, nicht das, was der Host hier hat.
            if self.is_covered(&next) {
                return Reach::Absent;
            }
            if self.leads_to_a_mount(&next) {
                at = next;
                continue;
            }
            let Some(host) = self.host_of(&next) else {
                return Reach::Absent;
            };
            match std::fs::symlink_metadata(&host) {
                Ok(meta) if meta.file_type().is_symlink() => {
                    hops += 1;
                    if hops > MAX_SYMLINK_HOPS {
                        return Reach::Unknown;
                    }
                    let Ok(target) = std::fs::read_link(&host) else {
                        return Reach::Unknown;
                    };
                    follow(&mut rest, &mut at, &target);
                }
                Ok(_) => at = next,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Reach::Absent;
                }
                // Keine Auskunft, keine Behauptung: Eine Einhängung, in die
                // dieser Prozess nicht hineinsehen darf, ist kein Beleg dafür,
                // dass der Agent fehlt.
                Err(_) => return Reach::Unknown,
            }
        }
        self.host_of(&at).map_or(Reach::Absent, Reach::Host)
    }

    /// Schlägt eine Datei nach und unterscheidet „nicht da", „da, aber nicht
    /// ausführbar" und „unentscheidbar".
    #[must_use]
    pub fn look_up(&self, in_sandbox: &Path) -> SandboxLookup {
        match self.resolve(in_sandbox) {
            Reach::Absent => SandboxLookup::Missing,
            Reach::Unknown => SandboxLookup::Unknown,
            Reach::Host(host) => {
                if is_executable_file(&host) {
                    SandboxLookup::Startable(host)
                } else if host.is_file() {
                    SandboxLookup::NotExecutable(host)
                } else {
                    SandboxLookup::Missing
                }
            }
        }
    }

    /// Der Ort auf dem Host, an dem diese Datei liegt, wenn die Sandbox sie
    /// starten kann.
    ///
    /// Zurück kommt der **aufgelöste** Pfad, nicht der genannte. Wer
    /// `/bin/opencode` fragt, bekommt `/usr/bin/opencode`: Nur diesen Pfad
    /// kann der Aufrufer auf dem Host auch selbst anfassen, etwa um
    /// `--version` zu lesen.
    #[must_use]
    pub fn startable(&self, in_sandbox: &Path) -> Option<PathBuf> {
        match self.look_up(in_sandbox) {
            SandboxLookup::Startable(host) => Some(host),
            SandboxLookup::Missing | SandboxLookup::NotExecutable(_) | SandboxLookup::Unknown => {
                None
            }
        }
    }

    /// Wahr, solange nicht feststeht, dass die Sandbox diesen Pfad **nicht**
    /// hat.
    ///
    /// Unentscheidbares zählt als erreichbar: Ein Befund über ein fehlendes
    /// Programm braucht einen Beleg.
    #[must_use]
    pub fn reaches(&self, in_sandbox: &Path) -> bool {
        !matches!(self.resolve(in_sandbox), Reach::Absent)
    }

    /// Wahr, wenn an dieser Stelle etwas anderes liegt als das, was der Host
    /// dort hat.
    fn is_covered(&self, in_sandbox: &Path) -> bool {
        let path = crate::profile::normalize(in_sandbox);
        self.covered
            .iter()
            .any(|over| path.starts_with(crate::profile::normalize(over)))
    }

    /// Wahr, wenn dieser Pfad der Sandbox zu einer Einhängung hinführt oder
    /// ihr Ziel ist.
    fn leads_to_a_mount(&self, in_sandbox: &Path) -> bool {
        let path = crate::profile::normalize(in_sandbox);
        self.mounts
            .iter()
            .any(|mount| crate::profile::normalize(&mount.dst).starts_with(&path))
    }

    /// Der Ort auf dem Host, an dem dieser Pfad der Sandbox liegt.
    ///
    /// Die längste passende Einhängung gewinnt, damit eine Einhängung in einer
    /// anderen die äußere nicht überstimmt.
    fn host_of(&self, in_sandbox: &Path) -> Option<PathBuf> {
        let path = crate::profile::normalize(in_sandbox);
        self.mounts
            .iter()
            .filter_map(|mount| {
                let dst = crate::profile::normalize(&mount.dst);
                path.strip_prefix(&dst)
                    .ok()
                    .map(|rest| (dst.components().count(), mount.src.join(rest)))
            })
            .max_by_key(|(depth, _)| *depth)
            .map(|(_, host)| host)
    }

    /// Das Ziel eines Verweises, den das Profil an dieser Stelle anlegt.
    fn symlink_at(&self, in_sandbox: &Path) -> Option<PathBuf> {
        let path = crate::profile::normalize(in_sandbox);
        self.symlinks
            .iter()
            .find(|(link, _)| crate::profile::normalize(link) == path)
            .map(|(_, target)| target.clone())
    }
}

/// Setzt die Auflösung hinter einem Verweis fort.
///
/// Ein absolutes Ziel beginnt wieder an der Wurzel der Sandbox, ein relatives
/// im Verzeichnis des Verweises. In beiden Fällen kommen die Stücke des Ziels
/// vor den Rest des Weges, damit ein `..` dahinter auf das Ergebnis wirkt.
fn follow(rest: &mut Vec<OsString>, at: &mut PathBuf, target: &Path) {
    let full = if target.is_absolute() {
        target.to_path_buf()
    } else {
        at.join(target)
    };
    rest.extend(parts_of(&full));
    *at = PathBuf::from("/");
}

/// Die Namen eines Pfades, das letzte Stück zuerst.
///
/// `.` fällt weg, `..` bleibt: Es wirkt erst beim Gehen, nach den Verweisen
/// davor. Wer es vorher wegrechnet, beantwortet eine andere Frage als der Kern
/// — `/m/link/../real` ist nicht `/m/real`, wenn `link` aus der Einhängung
/// herausführt.
fn parts_of(path: &Path) -> Vec<OsString> {
    path.components()
        .filter_map(|part| match part {
            std::path::Component::Normal(name) => Some(name.to_os_string()),
            std::path::Component::ParentDir => Some(OsString::from("..")),
            _ => None,
        })
        .rev()
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::ffi::{OsStr, OsString};
    use std::path::{Path, PathBuf};

    use humanitl_config::LlmConfig;
    use humanitl_core::SessionId;

    use super::{
        AgentContext, SandboxFile, SandboxLookup, SandboxView, files_inside_work, find_in_path,
        is_executable_file,
    };

    /// Ein Kontext mit einem Projektverzeichnis, das es nicht gibt.
    fn context() -> AgentContext {
        AgentContext::new(
            SessionId::nil(),
            PathBuf::from("/home/u/proj"),
            LlmConfig::default(),
        )
    }

    /// Ein ausführbares Programm in diesem Verzeichnis.
    fn program(dir: &Path, name: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt as _;

        let path = dir.join(name);
        std::fs::write(&path, b"#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

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

    /// Der Suchpfad der Sandbox entscheidet, nicht die Liste der Einhängungen
    /// (HUM-139).
    ///
    /// `mounted_without_path` liegt vorn in den Einhängungen und trägt
    /// dasselbe Programm; wer nur die Einhängungen durchsucht, gibt diesen
    /// Pfad zurück und liegt falsch: In der Sandbox findet `execvp` nur, was
    /// im PATH steht.
    #[test]
    fn resolve_in_sandbox_path_follows_the_sandbox_path() {
        let on_path = tempfile::tempdir().unwrap();
        let mounted_without_path = tempfile::tempdir().unwrap();
        program(on_path.path(), "opencode");
        program(mounted_without_path.path(), "opencode");

        let ctx = context()
            .with_sandbox_view(SandboxView::same_path(
                vec![
                    mounted_without_path.path().to_path_buf(),
                    on_path.path().to_path_buf(),
                ],
                Vec::new(),
            ))
            .with_sandbox_path(Some(OsString::from(on_path.path())));

        assert_eq!(
            ctx.resolve_in_sandbox_path(OsStr::new("opencode")),
            Some(on_path.path().join("opencode"))
        );
    }

    /// Ein Eintrag, den keine Einhängung deckt, ist drinnen nicht da — auch
    /// dann nicht, wenn die Datei darin in eine Einhängung zeigt.
    ///
    /// Der Fallstrick von HUM-139: Zählte so ein Eintrag als Treffer,
    /// verschwände der Befund, den HUM-135 gebaut hat. In der Sandbox gibt es
    /// das Verzeichnis nicht, also findet `execvp` dort nichts, egal was auf
    /// dem Host darin steht.
    #[test]
    fn resolve_in_sandbox_path_ignores_an_entry_without_a_mount() {
        let mounted = tempfile::tempdir().unwrap();
        let unmounted = tempfile::tempdir().unwrap();
        let target = program(mounted.path(), "opencode");
        std::os::unix::fs::symlink(&target, unmounted.path().join("opencode")).unwrap();

        let ctx = context()
            .with_sandbox_view(SandboxView::same_path(
                vec![mounted.path().to_path_buf()],
                Vec::new(),
            ))
            .with_sandbox_path(Some(OsString::from(unmounted.path())));

        assert_eq!(ctx.resolve_in_sandbox_path(OsStr::new("opencode")), None);
    }

    /// Relative und leere Einträge zeigen auf das Arbeitsverzeichnis der
    /// Sandbox, und dessen Host-Seite ist das Projektverzeichnis (HUM-139).
    ///
    /// Am 2026-09-13 an einer echten Sandbox gemessen: `bwrap … --chdir /work`
    /// mit `PATH=bin` startete `/work/bin/opencode`, mit `PATH=:/usr/bin`
    /// startete es `/work/opencode`.
    #[test]
    fn resolve_in_sandbox_path_resolves_relative_entries_against_work() {
        let work = tempfile::tempdir().unwrap();
        std::fs::create_dir(work.path().join("bin")).unwrap();
        program(&work.path().join("bin"), "opencode");
        program(work.path(), "opencode");

        let relative = AgentContext::new(
            SessionId::nil(),
            work.path().to_path_buf(),
            LlmConfig::default(),
        )
        .with_sandbox_path(Some(OsString::from("bin")));
        assert_eq!(
            relative.resolve_in_sandbox_path(OsStr::new("opencode")),
            Some(work.path().join("bin").join("opencode"))
        );

        let empty = AgentContext::new(
            SessionId::nil(),
            work.path().to_path_buf(),
            LlmConfig::default(),
        )
        .with_sandbox_path(Some(OsString::from(":/usr/bin")));
        assert_eq!(
            empty.resolve_in_sandbox_path(OsStr::new("opencode")),
            Some(work.path().join("opencode"))
        );
    }

    /// Ein Symlink, der aus der Einhängung herausführt, ist kein Treffer.
    ///
    /// Auf dem Host ist er ausführbar, in der Sandbox fehlt sein Ziel;
    /// gemessen endete dieses `exec` mit 127.
    #[test]
    fn resolve_in_sandbox_path_refuses_a_symlink_out_of_the_mount() {
        let mounted = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target = program(outside.path(), "opencode");
        std::os::unix::fs::symlink(&target, mounted.path().join("opencode")).unwrap();

        let ctx = context()
            .with_sandbox_view(SandboxView::same_path(
                vec![mounted.path().to_path_buf()],
                Vec::new(),
            ))
            .with_sandbox_path(Some(OsString::from(mounted.path())));

        assert_eq!(ctx.resolve_in_sandbox_path(OsStr::new("opencode")), None);
    }

    /// Eine Kette von Verweisen, deren Zwischenschritt die Einhängungen
    /// verlässt, ist kein Treffer (HUM-139).
    ///
    /// Am 2026-09-13 gemessen: `readlink -f` auf dem Host endet unter der
    /// Einhängung und der Host startet die Datei, während `exec` in der
    /// Sandbox am Zwischenschritt mit 127 endet. Ein kanonischer Pfad vom Host
    /// beantwortet diese Frage deshalb nicht.
    #[test]
    fn a_symlink_chain_through_the_outside_is_not_reachable() {
        let mounted = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let real = program(mounted.path(), "real-opencode");
        std::os::unix::fs::symlink(&real, outside.path().join("hop")).unwrap();
        std::os::unix::fs::symlink(outside.path().join("hop"), mounted.path().join("opencode"))
            .unwrap();

        let view = SandboxView::same_path(vec![mounted.path().to_path_buf()], Vec::new());
        assert_eq!(
            std::fs::canonicalize(mounted.path().join("opencode")).unwrap(),
            std::fs::canonicalize(&real).unwrap(),
            "the host resolves the chain to a file under the mount"
        );
        assert!(
            !view.reaches(&mounted.path().join("opencode")),
            "the hop in the middle is not in the sandbox"
        );
    }

    /// Ein Symlink auf dem Weg zu einer Einhängung zählt nicht: `bwrap` legt
    /// dort ein echtes Verzeichnis an (HUM-139).
    ///
    /// Auf Fedora Silverblue, Bazzite und `SteamOS` ist `/home` ein Verweis auf
    /// `/var/home`; ein Projekt auf einer zweiten Platte sieht genauso aus.
    /// Würde die Vorprüfung dem Verweis folgen, verglich sie `/var/home/…` mit
    /// der Wurzel `/home/…` und verweigerte einen Start, der läuft.
    #[test]
    fn a_symlink_above_a_mount_does_not_hide_it() {
        let base = tempfile::tempdir().unwrap();
        let real = base.path().join("var-home");
        std::fs::create_dir_all(real.join("u/proj/bin")).unwrap();
        let link = base.path().join("home");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let binary = program(&link.join("u/proj/bin"), "opencode");

        let root = link.join("u/proj");
        let view = SandboxView::same_path(vec![root], Vec::new());
        assert!(
            view.startable(&binary).is_some(),
            "the mount root lies behind a symlink of the host, and the sandbox has it as a \
             directory"
        );
    }

    /// Ein Verweis, den das Profil anlegt, führt in die Einhängung (HUM-139).
    ///
    /// `profiles/sandbox/default.toml` legt `/bin` auf `usr/bin` und hängt
    /// `/usr` ein; `[env].PATH` nennt `/bin`. Am 2026-09-13 gemessen: In einer
    /// Sandbox mit diesem Verweis startete `PATH=/bin` das Programm aus
    /// `/usr/bin`.
    #[test]
    fn a_symlink_of_the_profile_leads_into_the_mount() {
        let base = tempfile::tempdir().unwrap();
        let usr_bin = base.path().join("usr/bin");
        std::fs::create_dir_all(&usr_bin).unwrap();
        let binary = program(&usr_bin, "opencode");

        let view = SandboxView::same_path(
            vec![base.path().join("usr")],
            vec![(base.path().join("bin"), PathBuf::from("usr/bin"))],
        );
        assert_eq!(
            view.resolve(&base.path().join("bin/opencode")),
            super::Reach::Host(binary),
            "the link of the profile leads to the mounted directory"
        );
    }

    /// Das Arbeitsverzeichnis der Sandbox kommt aus dem Profil, nicht aus
    /// einer Konstanten (HUM-139).
    ///
    /// `[mounts].work.dst` ist einstellbar, und der Launcher setzt `--chdir`
    /// darauf. Gemessen werden beide Formen, die dorthin zeigen: der relative
    /// Eintrag `bin` über das Arbeitsverzeichnis und der absolute Eintrag
    /// `/workspace/bin`, der nur trägt, wenn das Profil wirklich gilt.
    #[test]
    fn the_work_directory_of_the_profile_carries_both_forms_of_an_entry() {
        let work = tempfile::tempdir().unwrap();
        std::fs::create_dir(work.path().join("bin")).unwrap();
        let binary = program(&work.path().join("bin"), "opencode");

        let with_dst = |entry: &str| {
            AgentContext::new(
                SessionId::nil(),
                work.path().to_path_buf(),
                LlmConfig::default(),
            )
            .with_work_dir_sandbox(PathBuf::from("/workspace"))
            .with_sandbox_path(Some(OsString::from(entry)))
            .resolve_in_sandbox_path(OsStr::new("opencode"))
        };

        assert_eq!(with_dst("bin"), Some(binary.clone()), "the relative entry");
        assert_eq!(
            with_dst("/workspace/bin"),
            Some(binary),
            "the absolute entry that names the work directory of the profile"
        );
    }

    /// Zurück kommt der aufgelöste Pfad, nicht der genannte (HUM-139).
    ///
    /// Wer `/bin/opencode` sucht, bekommt `/usr/bin/opencode`. Nur den kann
    /// der Aufrufer auf dem Host anfassen: `/bin` ist ein Verweis, den das
    /// Profil in der Sandbox anlegt, und auf diesem Rechner muss es ihn nicht
    /// geben.
    #[test]
    fn the_sandbox_path_gives_back_the_resolved_place_on_the_host() {
        let base = tempfile::tempdir().unwrap();
        let usr_bin = base.path().join("usr/bin");
        std::fs::create_dir_all(&usr_bin).unwrap();
        let binary = program(&usr_bin, "opencode");

        let ctx = context()
            .with_sandbox_view(SandboxView::same_path(
                vec![base.path().join("usr")],
                vec![(base.path().join("bin"), PathBuf::from("usr/bin"))],
            ))
            .with_sandbox_path(Some(OsString::from(base.path().join("bin"))));

        assert_eq!(
            ctx.resolve_in_sandbox_path(OsStr::new("opencode")),
            Some(binary),
            "the link of the profile is followed before the path is handed back"
        );
    }

    /// Ein relatives Kommando startet im Arbeitsverzeichnis der Sandbox
    /// (HUM-139).
    ///
    /// `agent.command = ["./bin/opencode"]` startet drinnen
    /// `/work/bin/opencode`, weil der Launcher `--chdir` auf `/work` setzt.
    /// Der Pfad des Hosts sagt darüber nichts: Dort hängt er am
    /// Arbeitsverzeichnis des Daemons.
    #[test]
    fn a_relative_command_is_looked_up_in_the_work_directory() {
        let work = tempfile::tempdir().unwrap();
        std::fs::create_dir(work.path().join("bin")).unwrap();
        let binary = program(&work.path().join("bin"), "opencode");

        let ctx = AgentContext::new(
            SessionId::nil(),
            work.path().to_path_buf(),
            LlmConfig::default(),
        );
        assert_eq!(
            ctx.resolve_in_sandbox_path(OsStr::new("./bin/opencode")),
            Some(binary),
            "a command with a separator is resolved against the work directory"
        );
        assert_eq!(
            ctx.resolve_in_sandbox_path(OsStr::new("/usr/bin/opencode")),
            None,
            "an absolute command is the question of reaches_program, not of the search"
        );
    }

    /// Eine Datei im Suchpfad der Sandbox ohne Ausführungsrecht ist da, nicht
    /// verschwunden (HUM-139).
    #[test]
    fn a_file_without_the_execute_bit_is_found_but_not_startable() {
        use std::os::unix::fs::PermissionsExt as _;

        let mounted = tempfile::tempdir().unwrap();
        let binary = mounted.path().join("opencode");
        std::fs::write(&binary, b"#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o644)).unwrap();

        let ctx = context()
            .with_sandbox_view(SandboxView::same_path(
                vec![mounted.path().to_path_buf()],
                Vec::new(),
            ))
            .with_sandbox_path(Some(OsString::from(mounted.path())));

        assert_eq!(
            ctx.look_up_in_sandbox_path(OsStr::new("opencode")),
            super::SandboxLookup::NotExecutable(binary)
        );
    }

    /// Die Sicht kommt aus dem Profil: Einhängungen, Verweise und
    /// Überdeckungen (HUM-139).
    ///
    /// `extra_rw` gehört dazu. Es kommt als `--bind path path` in die Sandbox
    /// und ist dort unter demselben Namen da; fehlte es, verböte die
    /// Vorprüfung einen Start, der läuft — dieselbe Fehlerart, für die es
    /// dieses Issue gibt.
    #[test]
    fn the_view_of_a_profile_carries_mounts_symlinks_and_covers() {
        let profile = crate::profile::SandboxProfile::parse(
            concat!(
                "version = 1\n",
                "name = \"test\"\n",
                "[mounts]\n",
                "ro = [\"/usr\"]\n",
                "extra_ro = [\"/opt/tools\"]\n",
                "extra_rw = [\"/opt/scratch\"]\n",
                "symlinks = [[\"usr/bin\", \"/bin\"]]\n",
                "tmpfs = [\"/tmp\", \"/var/tmp\", \"/dev/shm\", \"/home/agent\", \"/work/.direnv\"]\n",
            ),
            Path::new("<test>"),
        )
        .unwrap();

        let view = SandboxView::of_profile(&profile);
        assert_eq!(
            view.mounts,
            vec![
                super::Mount::same_path("/usr"),
                super::Mount::same_path("/opt/tools"),
                super::Mount::same_path("/opt/scratch"),
            ],
            "a read-write mount is a mount"
        );
        assert_eq!(
            view.symlinks,
            vec![(PathBuf::from("/bin"), PathBuf::from("usr/bin"))]
        );
        assert!(
            view.covered.contains(&PathBuf::from("/work/.direnv")),
            "a tmpfs covers what the host has there: {:?}",
            view.covered
        );
        assert!(
            view.covered.contains(&PathBuf::from("/work/.envrc")),
            "the mandatory masks cover single files: {:?}",
            view.covered
        );
    }

    /// Ein tmpfs überdeckt, was der Host an dieser Stelle hat (HUM-139).
    ///
    /// Das ausgelieferte Profil legt eines über `/work/.direnv`,
    /// `/work/.git/hooks` und `/work/.vscode` (HUM-043). Ein Kommando von dort
    /// findet der Host, und drinnen endet der Start mit 127.
    #[test]
    fn a_tmpfs_hides_what_the_host_has_under_it() {
        let work = tempfile::tempdir().unwrap();
        std::fs::create_dir(work.path().join(".direnv")).unwrap();
        program(&work.path().join(".direnv"), "opencode");

        let view = SandboxView::new(vec![super::Mount::new(work.path(), "/work")], Vec::new());
        assert!(
            view.startable(Path::new("/work/.direnv/opencode"))
                .is_some(),
            "without the cover the host decides"
        );

        let covered = view.with_covered(vec![PathBuf::from("/work/.direnv")]);
        assert_eq!(
            covered.look_up(Path::new("/work/.direnv/opencode")),
            SandboxLookup::Missing,
            "the tmpfs is empty inside, whatever the host has there"
        );
    }

    /// Ohne `[env].PATH` sucht die Sandbox im Pfad der C-Bibliothek
    /// (HUM-139).
    ///
    /// Ein Profil ohne `[env].PATH` ist gültig. Am 2026-09-13 gemessen:
    /// `bwrap … --clearenv -- env echo` läuft, und `getconf PATH` drinnen sagt
    /// `/bin:/usr/bin`. Wer hier `Missing` meldete, fiele auf den Host zurück
    /// und verböte mit `AGENT_004` einen Start, der funktioniert.
    #[test]
    fn without_a_path_the_sandbox_searches_the_default_of_the_c_library() {
        let base = tempfile::tempdir().unwrap();
        let usr_bin = base.path().join("usr/bin");
        std::fs::create_dir_all(&usr_bin).unwrap();
        let binary = program(&usr_bin, "opencode");

        // `/bin` ist der Verweis des Profils, `/usr` die Einhängung; beide
        // unter einer eigenen Wurzel, damit der Test nichts vom Rechner nimmt.
        let view = SandboxView::same_path(
            vec![base.path().join("usr")],
            vec![(PathBuf::from("/bin"), base.path().join("usr/bin"))],
        );
        let ctx = context().with_sandbox_view(view);

        assert_eq!(ctx.sandbox_path, None, "the profile names no PATH");
        assert_eq!(
            ctx.resolve_in_sandbox_path(OsStr::new("opencode")),
            Some(binary),
            "execvp falls back to /bin:/usr/bin"
        );
        assert!(
            ctx.sandbox_path_display()
                .contains(super::DEFAULT_SANDBOX_PATH),
            "the findings name the path that really applies: {}",
            ctx.sandbox_path_display()
        );
    }

    /// `..` wirkt nach dem Verweis davor, nicht davor (HUM-139).
    ///
    /// So arbeitet der Kern: `/m/link/../real` folgt erst `link`. Führt der
    /// Verweis aus der Einhängung heraus, landet der Schritt zurück draußen
    /// und nicht bei `/m/real`. Wer `..` vorher wegrechnet, beantwortet eine
    /// andere Frage und lässt einen Pfad durch, den die Sandbox nicht hat.
    #[test]
    fn a_parent_step_applies_after_the_symlink_before_it() {
        let mounted = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::create_dir(mounted.path().join("real")).unwrap();
        program(&mounted.path().join("real"), "opencode");
        std::os::unix::fs::symlink(outside.path(), mounted.path().join("link")).unwrap();

        let view = SandboxView::same_path(vec![mounted.path().to_path_buf()], Vec::new());
        assert_eq!(
            view.look_up(&mounted.path().join("link/../real/opencode")),
            SandboxLookup::Missing,
            "the link leaves the mount, so the step back lands outside"
        );
        assert!(
            view.startable(&mounted.path().join("real/opencode"))
                .is_some(),
            "the same file without the detour is there"
        );
    }

    /// Ein Verweis im Projektbaum gilt in Pfaden der Sandbox (HUM-139).
    ///
    /// `bin/opencode` auf `/work/tools/opencode` startet drinnen, weil es
    /// `/work` dort gibt. Derselbe Verweis auf den Host-Pfad desselben Baums
    /// ist drinnen kaputt: Diesen Pfad gibt es in der Sandbox nicht.
    #[test]
    fn a_symlink_in_the_work_tree_is_read_in_sandbox_paths() {
        let work = tempfile::tempdir().unwrap();
        std::fs::create_dir(work.path().join("bin")).unwrap();
        std::fs::create_dir(work.path().join("tools")).unwrap();
        let real = program(&work.path().join("tools"), "opencode");

        let view = SandboxView::new(vec![super::Mount::new(work.path(), "/work")], Vec::new());

        std::os::unix::fs::symlink("/work/tools/opencode", work.path().join("bin/opencode"))
            .unwrap();
        assert_eq!(
            view.startable(Path::new("/work/bin/opencode")),
            Some(real),
            "the target names the path the sandbox has"
        );

        std::fs::remove_file(work.path().join("bin/opencode")).unwrap();
        std::os::unix::fs::symlink(
            work.path().join("tools/opencode"),
            work.path().join("bin/opencode"),
        )
        .unwrap();
        assert_eq!(
            view.look_up(Path::new("/work/bin/opencode")),
            SandboxLookup::Missing,
            "the target names a path of the host, and the sandbox has no such place"
        );
    }

    /// Was sich nicht lesen lässt, wird nicht behauptet (HUM-139).
    ///
    /// Ein Verzeichnis ohne Leserecht ist kein Beleg dafür, dass der Agent
    /// fehlt. `backlog/CONVENTIONS.md` 4.13: Ein Befund braucht einen Beleg.
    #[test]
    fn a_directory_that_cannot_be_read_stays_unknown() {
        use std::os::unix::fs::PermissionsExt as _;

        let mounted = tempfile::tempdir().unwrap();
        let closed = mounted.path().join("closed");
        std::fs::create_dir(&closed).unwrap();
        std::fs::set_permissions(&closed, std::fs::Permissions::from_mode(0o000)).unwrap();

        let view = SandboxView::same_path(vec![mounted.path().to_path_buf()], Vec::new());
        let lookup = view.look_up(&closed.join("opencode"));
        // Als `root` ist alles lesbar; dann ist die Antwort „nicht da", und
        // der Test misst nichts. Das sagt er, statt grün zu behaupten.
        if rustix::process::getuid().is_root() {
            assert_eq!(lookup, SandboxLookup::Missing);
        } else {
            assert_eq!(lookup, SandboxLookup::Unknown);
        }
        std::fs::set_permissions(&closed, std::fs::Permissions::from_mode(0o700)).unwrap();
    }

    /// Ausführbar heißt: **dieser** Nutzer darf, nicht irgendwer.
    #[test]
    fn is_executable_file_asks_access_and_not_the_mode_bits() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = tempfile::tempdir().unwrap();
        let others_only = dir.path().join("others-only");
        std::fs::write(&others_only, b"#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&others_only, std::fs::Permissions::from_mode(0o601)).unwrap();
        assert!(
            std::fs::metadata(&others_only)
                .unwrap()
                .permissions()
                .mode()
                & 0o111
                != 0,
            "the fixture carries an x bit for others"
        );
        assert!(
            !is_executable_file(&others_only),
            "the owner of this file may not execute it"
        );

        assert!(is_executable_file(&program(dir.path(), "mine")));
        assert!(!is_executable_file(dir.path()), "a directory is not a file");
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
