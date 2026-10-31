//! Die `Sandbox`-RPC: was der Agent bekommt, und Start und Stopp (HUM-040).
//!
//! Der Bildschirm „Sandbox" beantwortet eine einzige Frage: bekommt der Agent
//! die ganze Platte? Alles, was er dafür zeigt, kommt von hier, und zwar aus
//! genau der Kommandozeile, die auch startet — die Liste der Einhängungen wird
//! aus dem Argumentvektor gelesen und nicht daneben ein zweites Mal geführt.
//! Ein Mount, der in der Zeile steht, steht damit auch in der Tabelle; einen
//! zu vergessen, ist strukturell nicht möglich.
//!
//! Die Umgebung geht denselben Weg: [`SandboxProfile::effective_env`] liefert
//! genau die Paare, die `--setenv` in die Zeile schreibt.
//!
//! # Warum die Werte nach einer Erlaubnisliste gehen
//!
//! Zurückgehalten wird nach [`VISIBLE_ENV`], nicht nach einer Liste
//! verdächtiger Namen. Eine Liste verdächtiger Namen ist prinzipiell
//! unvollständig, und die Lücken sind genau die gefährlichen:
//! `AWS_ACCESS_KEY_ID` endet auf `_ID`, `DATABASE_URL` trägt das Passwort in
//! der URL, `GH_PAT` und `AUTHORIZATION` heißen nach gar nichts. Zwei der drei
//! Quellen der Umgebung sind offen — `[env]` eines eigenen Profils und
//! `sandbox.env` aus `config.toml`, und genau dort landet später die
//! Zugangsdaten-Injektion —, also kann keine Namensregel sie ausschöpfen.
//!
//! Gezeigt wird deshalb nur, was der Bildschirm zum Beweis braucht: wohin der
//! Agent darf (Proxy), wem er glaubt (Zertifikate), wo er steht (Pfade,
//! Sprache) und was der Adapter zur Steuerung setzt. Jeder andere Wert bleibt
//! zurück, auch ein harmloser. Die Vorgabe steht damit auf der sicheren Seite:
//! eine neue Variable ist stumm, nicht versehentlich sichtbar (CONVENTIONS
//! 4.13, „Voreinstellungen stehen auf der sicheren Seite").
//!
//! Dieselben Werte stehen auch in der angezeigten Kommandozeile nicht: dort
//! ersetzt [`WITHHELD_PLACEHOLDER`] sie. Sonst wäre die Zeile die Hintertür,
//! durch die ein Geheimnis den Bildschirm doch verlässt — und sie ist das eine
//! Stück dieses Bildschirms, das man in die Zwischenablage legt und in ein
//! Ticket klebt.
//!
//! # Was dieses Modul nicht tut
//!
//! Es hängt kein Terminal an (HUM-042) und es schreibt keine Konfiguration
//! (`SetConfig`, HUM-069). Das Projektverzeichnis, das die Oberfläche wählt,
//! reist deshalb in [`v1::sandbox_request::Plan`] und in
//! [`v1::sandbox_request::Start`] mit und gilt für die laufende Sitzung;
//! dauerhaft wird es erst mit dem Einstellungs-Bildschirm.
//!
//! # Die drei Garantien
//!
//! Gemessen wird in der Sandbox, die läuft: Der Shim schreibt seine Prüfzeilen
//! von innen, [`SandboxBackend::isolation_check`] faltet sie zu den drei
//! Garantien, und dieses Modul reicht sie als [`v1::CheckResult`] weiter. Es
//! prüft nichts selbst; eine zweite Prüfpipeline neben der vorhandenen wäre
//! eine zweite Wahrheit über dieselbe Sandbox.
//!
//! Fail-closed: Ist eine Garantie rot oder fehlt der Bericht, wird die Sandbox
//! beendet und der Zustand ist `failed` — wie `enforce_isolation` es auf der
//! Kommandozeile tut (HUM-041).
//!
//! **Was das nicht heißt.** Der Shim erzwingt nichts: Er schreibt seine fünf
//! Zeilen und `exec`t den Agenten unmittelbar danach (HUM-012, CONVENTIONS
//! 4.12). Der Wirt liest den Bericht erst danach. Zwischen dem `exec` und dem
//! `SIGKILL` liegt deshalb ein Fenster, in dem der Agent läuft, die Brücke
//! steht und der Proxy annimmt — Millisekunden im Normalfall, im Fall
//! `SANDBOX_015` mit genau dem zweiten Socket, den die Prüfung beanstandet.
//! Die obere Schranke ist die Summe zweier Fristen: `REPORT_TIMEOUT` (5 s),
//! bis ein ausbleibender Bericht als ausgeblieben gilt, plus `KILL_GRACE`
//! (5 s), die `terminate` nach dem `SIGKILL` auf das Einsammeln wartet —
//! zusammen **bis zu 10 s**.
//!
//! Das Fenster wird hier so klein wie möglich gehalten (der Dienst tötet ohne
//! Gnadenfrist und wartet nur einmal), aber es wird hier nicht geschlossen:
//! Dazu müsste der Proxy Verbindungen ablehnen, solange die Isolation nicht
//! belegt ist, und `humanitl-proxy` liegt innerhalb dieser Crate
//! (Abhängigkeitsrichtung, `tools/deps-allow.toml`). Es steht als eigenes
//! Issue offen und ist in `docs/SECURITY.md` und `docs/THREAT-MODEL.md`
//! benannt.
//!
//! **Derselbe Bau steht auf jedem Weg in die Sandbox.** `humanitl sandbox
//! run` und `escape-launch` starten ebenso erst und prüfen danach; sie töten
//! sogar mit Gnadenfrist (`SandboxHandle::kill`), ihre Schranke ist also
//! 5 s + 5 s + 5 s = bis zu 15 s. Keiner der drei Wege ist fail-closed in dem
//! Sinn, dass der Befehl nicht liefe.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError, RwLock};
use std::time::{Duration, Instant, SystemTime};

use humanitl_config::secret::{KeyRef, KeyStore};
use humanitl_config::{Config, SandboxMode, WorkMode};
use humanitl_core::diagnostics::codes;
use humanitl_core::ids::{SandboxId, SessionId};
use humanitl_core::{Diagnostic, FixAction, FlowEvent, Severity};
use humanitl_proxy::rules_store::RulesStore;
use humanitl_proxy::session::{SessionSettings, SessionState};
use humanitl_recorder::Recorder;
use humanitl_sandbox::agent::{opencode, pi};
use humanitl_sandbox::summary::SessionSummary;
use humanitl_sandbox::{
    AdapterRegistry, AgentContext, AgentKey, CheckResult, IsolationCheck, KILL_GRACE, LaunchInputs,
    MountPolicy, PlatformBackend, SANDBOX_SHELL, SandboxBackend, SandboxFile, SandboxHandle,
    SandboxProfile, SessionContext, StdioMode, Termination, WorkDirSource, check_project_dir,
    shell_line,
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::exec::ShellRegistry;
use crate::llm_reach::LlmReach;
use crate::server_stub::BoxStream;
use crate::session::{
    SessionRequest, SessionResolver, ask_mode_name, bundled_rules, parse_ask_mode, parse_work_mode,
    work_mode_name,
};
use crate::summary::SummaryWatch;
use crate::terminal::{HeldNotices, TerminalHub};
use crate::v1;

/// Der Name des Shims, wie er neben dem Daemon oder im Systempfad liegt.
const SHIM_BINARY: &str = "humanitl-shim";

/// Wo ein installiertes Humanitl den Shim ablegt, in dieser Reihenfolge.
const SHIM_DIRS: &[&str] = &[
    "/usr/lib/humanitl",
    "/usr/libexec/humanitl",
    "/usr/local/lib/humanitl",
];

/// Wo ein installiertes Humanitl die Sandbox-Profile ablegt.
const PROFILE_DIRS: &[&str] = &["/usr/share/humanitl", "/usr/local/share/humanitl"];

/// Das Unterverzeichnis der Sandbox-Profile.
const PROFILE_SUBDIR: &str = "profiles/sandbox";

/// Wie viele Elternverzeichnisse des Binaries nach `profiles/sandbox`
/// abgesucht werden.
const TREE_DEPTH: usize = 6;

/// Der Name des CA-Bundles im CA-Verzeichnis; dieselbe Datei wie
/// `humanitl_proxy::ca::BUNDLE_FILE`.
const CA_BUNDLE_FILE: &str = "ca-bundle.crt";

/// Was in der angezeigten Kommandozeile anstelle eines zurückgehaltenen Werts
/// steht.
///
/// Die Zeile ist Anzeige und nie Ausführung — gestartet wird immer die Liste
/// aus [`SandboxProfile::to_bwrap_args`] —, und sie ist das eine Stück dieses
/// Bildschirms, das ein Mensch in die Zwischenablage legt. Ein Wert, den die
/// Umgebungstabelle zurückhält und die Zeile daneben ausschreibt, wäre
/// zurückgehalten nur dem Namen nach.
pub const WITHHELD_PLACEHOLDER: &str = "<withheld>";

/// Die Variablen, deren **Wert** die Oberfläche sehen darf.
///
/// Vier Gruppen, und jede beantwortet eine Frage des Bildschirms:
///
/// 1. **Wohin darf der Agent** — die Proxy-Variablen. Sie sind der Beleg, dass
///    aller Verkehr auf den Loopback zum Shim zeigt.
/// 2. **Wem glaubt er** — die Zertifikatspfade. Sie belegen, dass er der CA
///    dieser Sitzung glaubt und keiner anderen.
/// 3. **Wo steht er** — Heimat, Pfad, Sprache, Terminal, die XDG-Verzeichnisse.
/// 4. **Was steuert ihn** — die Variablen des Shims und die des Adapters.
///
/// Verglichen wird ohne Rücksicht auf Groß- und Kleinschreibung: das Profil
/// setzt `HTTP_PROXY` und `http_proxy` nebeneinander, und beide meinen
/// dasselbe. Ein Name, der hier fehlt, wird zurückgehalten — auch ein
/// harmloser. Das ist der Preis dafür, dass ein gefährlicher nie durchrutscht.
///
/// Die Namen des Shims und des Adapters stehen als Konstanten und nicht als
/// Text: Wer eine davon umbenennt, bricht den Bau, statt sie still von der
/// Liste zu nehmen.
pub static VISIBLE_ENV: &[&str] = &[
    // 1. Wohin darf der Agent.
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "NO_PROXY",
    "FTP_PROXY",
    // Lenkt Nodes eingebautes `fetch` auf dieselben Proxy-Variablen (HUM-304).
    "NODE_USE_ENV_PROXY",
    // 2. Wem glaubt er.
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "CURL_CA_BUNDLE",
    "REQUESTS_CA_BUNDLE",
    "NODE_EXTRA_CA_CERTS",
    "DENO_CERT",
    "GIT_SSL_CAINFO",
    "CARGO_HTTP_CAINFO",
    "PIP_CERT",
    "NPM_CONFIG_CAFILE",
    "NIX_SSL_CERT_FILE",
    "AWS_CA_BUNDLE",
    // 3. Wo steht er.
    "HOME",
    "USER",
    "LOGNAME",
    "SHELL",
    "PATH",
    "PWD",
    "TMPDIR",
    "TERM",
    "COLORTERM",
    "LANG",
    "LC_ALL",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "XDG_CACHE_HOME",
    "XDG_STATE_HOME",
    "XDG_RUNTIME_DIR",
    // 4a. Was steuert ihn: die Sitzung und der Shim.
    "HUMANITL",
    ENV_SESSION,
    humanitl_sandbox::ENV_BRIDGES,
    humanitl_sandbox::ENV_SECCOMP_FAMILIES,
    humanitl_sandbox::ENV_SECCOMP_TYPES,
    humanitl_sandbox::ENV_SECCOMP_DENY,
    humanitl_sandbox::ENV_REPORT_FD,
    // 4b. Was steuert ihn: der Adapter. Keine dieser Variablen trägt einen
    // Wert, den der Mensch nicht sehen soll; sie schalten Selbstaktualisierung,
    // Telemetrie und Rechte ab, und genau das will der Bildschirm belegen.
    // Kein Präfix, sondern Namen: `OPENCODE_*` als Muster hieße, dass ein
    // künftiges `OPENCODE_API_KEY` von selbst sichtbar wäre.
    opencode::ENV_DISABLE_AUTOUPDATE,
    opencode::ENV_MODELS_PATH,
    opencode::ENV_DISABLE_MODELS_FETCH,
    opencode::ENV_CONFIG,
    opencode::ENV_AUTO_SHARE,
    opencode::ENV_DISABLE_SHARE,
    opencode::ENV_ENABLE_EXA,
    opencode::ENV_ENABLE_PARALLEL,
    opencode::ENV_DISABLE_LSP_DOWNLOAD,
    opencode::ENV_PERMISSION,
    opencode::ENV_BASH_DEFAULT_TIMEOUT_MS,
    // The pi adapter (HUM-274), by name for the same reason.
    pi::ENV_AGENT_DIR,
    pi::ENV_OFFLINE,
    pi::ENV_SKIP_VERSION_CHECK,
    pi::ENV_TELEMETRY,
];

/// Die Variable, die allein die Sitzung kennt.
const ENV_SESSION: &str = "HUMANITL_SESSION";

/// So viele Ereignisse darf ein Sandbox-Strom puffern, bevor der Erzeuger
/// wartet. Ein Start schickt eine Handvoll; die Reserve fängt einen Client
/// ab, der langsamer liest, als der Start meldet.
const EVENT_BUFFER: usize = 32;

/// Wie lange der Faden der verweigerten Versuche auf einen neuen Stand
/// wartet, bevor er nachsieht, ob noch jemand zuhört (HUM-138).
const REFUSALS_POLL: Duration = Duration::from_secs(1);

/// Wie lange nach dem Ende der Sandbox noch auf die letzten Zeilen über
/// verweigerte Versuche gewartet wird. Der Shim schreibt sie, nachdem der
/// Agent gegangen ist; die Pipe schließt kurz danach mit der Sandbox.
const REFUSALS_DRAIN: Duration = Duration::from_secs(2);

/// Die Geometrie, mit der eine Sitzung startet, bevor ein Client seine eigene
/// nennt.
///
/// Achtzig mal vierundzwanzig ist die Größe, die jedes Terminal kennt und die
/// jedes TUI ohne Nachfrage bedienen kann. Der erste Schreiber ersetzt sie mit
/// seinem `Open` (HUM-042).
pub const DEFAULT_TERMINAL_SIZE: (u16, u16) = (80, 24);

/// Der Code, mit dem ein Terminal endet, dessen Sitzung keinen Status mehr
/// hergab.
///
/// Erreichbar nur, wenn der wartende Faden den Status nicht mehr findet;
/// `128` ist die Stelle, an der die Signalcodes beginnen, und heißt hier
/// „beendet, Grund unbekannt".
const EXIT_UNKNOWN: i32 = 128;

/// Ob der Name auf der Erlaubnisliste steht ([`VISIBLE_ENV`]).
///
/// Die Frage lautet bewusst nicht „ist das ein Geheimnis?", sondern „brauche
/// ich das als Beleg?" — die erste ist nicht beantwortbar, die zweite schon.
#[must_use]
pub fn is_visible_env_name(key: &str) -> bool {
    VISIBLE_ENV
        .iter()
        .any(|visible| visible.eq_ignore_ascii_case(key))
}

/// Ob der **Wert** dieser Variablen den Daemon verlassen darf.
///
/// Zwei Bedingungen, und beide müssen gelten: Der Name steht auf
/// [`VISIBLE_ENV`], **und** den Wert hat nicht ein Mensch geschrieben.
///
/// Der Name allein genügt nicht. `HTTP_PROXY` steht auf der Liste, weil der
/// Daemon dort `http://127.0.0.1:3128` hinschreibt und das der Beleg dafür
/// ist, dass aller Verkehr durch den Shim geht — aber derselbe Name in
/// `sandbox.env` oder in einem eigenen Profil kann
/// `http://nutzer:passwort@host` lauten, und dann stünde ein Zugangsdatum
/// unter einem erlaubten Namen auf dem Schirm und in der kopierbaren Zeile.
/// Ein erlaubter Name sagt nichts über den Wert, der darunter steht; deshalb
/// entscheidet die Herkunft mit.
///
/// [`v1::ValueOrigin::User`] ist alles, was ein Mensch geschrieben hat: `[env]`
/// eines eigenen Profils und `sandbox.env` aus `config.toml`. Eine Herkunft,
/// die der Dienst nicht zuordnen konnte, zählt ebenfalls als `User` — eine
/// unbekannte Quelle ist der Fall, in dem die Vorgabe zurückhalten muss.
#[must_use]
pub fn shows_env_value(key: &str, origin: v1::ValueOrigin) -> bool {
    matches!(
        origin,
        v1::ValueOrigin::Profile | v1::ValueOrigin::Adapter | v1::ValueOrigin::Session
    ) && is_visible_env_name(key)
}

/// Die laufende Sandbox dieser Sitzung.
#[derive(Debug)]
struct Running {
    handle: Arc<SandboxHandle>,
    /// Das Backend, das diese Sandbox gestartet hat.
    ///
    /// Es wird gehalten und nicht neu gesucht, weil die Isolationsprüfung an
    /// derselben Instanz hängt, die auch gestartet hat: In ihr steht die
    /// Frist, innerhalb deren der Bericht des Shims eintreffen muss
    /// (`BwrapBackend::with_report_timeout`). Ein zweites, frisch gefundenes
    /// Backend hätte eine andere Frist und prüfte damit unter anderen
    /// Bedingungen als es gestartet hat.
    backend: PlatformBackend,
    started_at: SystemTime,
    profile: String,
    work_dir: PathBuf,
    /// Woher `work_dir` kam (HUM-293), für jeden Befund über ihn.
    work_dir_source: WorkDirSource,
    work_mode: WorkMode,
    /// Das Terminal dieser Sitzung: der eine Filter, der Ringpuffer und die
    /// Clients, die zusehen (HUM-042).
    terminal: TerminalHub,
    /// Die Terminals der weiteren Shells dieser Sitzung (HUM-299).
    shells: ShellRegistry,
    /// Was ein Befund braucht, wenn der Agent nie läuft (HUM-137).
    agent: AgentFacts,
}

/// Das Kommando dieser Sitzung und der `PATH`, unter dem es gesucht wird.
///
/// Beides wird beim Start festgehalten und nicht beim Ende neu gebaut: Am Ende
/// kann die Konfiguration schon eine andere sein, und der Befund
/// [`humanitl_sandbox::did_not_start`] muss über das Kommando reden, das
/// wirklich gestartet wurde.
#[derive(Debug, Clone)]
struct AgentFacts {
    /// Das erste Wort hinter dem Shim.
    command: String,
    /// Der `PATH` der Sandbox, wie der Bildschirm ihn zeigen darf.
    path: String,
}

/// Was die `Sandbox`-RPC beantwortet.
///
/// Billig zu klonen; der Zustand liegt hinter einem `Arc`. Der Dienst hält
/// höchstens eine laufende Sandbox: eine Sitzung, ein Agent.
#[derive(Debug, Clone)]
pub struct SandboxService {
    inner: Arc<Inner>,
}

/// Was der Mensch für den nächsten Start gewählt hat.
///
/// Profil und Modus stehen hier und nicht in `config.toml`; sie gelten für
/// diese Sitzung des Daemons.
///
/// **Der Projektordner steht nicht hier** (HUM-293). Er hat genau eine Quelle,
/// `sandbox.work_dir` in `config.toml`, und die Oberfläche schreibt ihn mit
/// `SetConfig`. Bis HUM-293 merkte sich der Daemon den Ordner aus
/// `Sandbox(Plan)` an dieser Stelle; die Oberfläche startete dann in ihm,
/// `humanitl sandbox run` aber im aktuellen Verzeichnis, und `SANDBOX_006`
/// nannte `sandbox.work_dir` für einen Pfad, der in keiner Datei stand. Ein
/// `work_dir` in `Plan` gilt nur für diesen einen Plan, eines in `Start` nur
/// für diesen einen Start ([`Pending::starting_work_dir`]).
#[derive(Debug, Default)]
struct Pending {
    profile: Option<String>,
    /// Der geprüfte Ordner, den der Start nennt, der gerade den Anspruch
    /// hält (`--work` oder das aktuelle Verzeichnis von `humanitl run`).
    ///
    /// Mit ihm steht seine Quelle, damit ein Befund während des Starts
    /// `--work` oder die Datei nennt und nicht „die Anfrage".
    ///
    /// Er lebt, bis die Sandbox steht: [`Inner::launch`] leert ihn, sobald
    /// `Running::work_dir` ihn nennt, und [`StartClaim`] beim Loslassen,
    /// falls der Start vorher scheitert. Ein `Plan` ohne Ordner, der danach
    /// kommt, auch im Fenster der Zusammenfassung nach dem Ende, zeigt den
    /// laufenden oder den gespeicherten Ordner; ein Ordner eines Aufrufs wird
    /// nie zum Ordner des nächsten.
    starting_work_dir: Option<(PathBuf, WorkDirSource)>,
    work_mode: Option<WorkMode>,
    /// Ob gerade ein Start läuft, der noch kein Handle hat.
    ///
    /// `self.running` wird erst gesetzt, wenn `bwrap` steht; zwischen der
    /// Frage „läuft schon eine?" und dieser Zuweisung liegen die Auflösung der
    /// Sitzung und der Start selbst. Zwei gleichzeitige `Start` kämen ohne
    /// diesen Anspruch beide daran vorbei, beide starteten, der zweite
    /// überschriebe `running` — und der erste Prozess liefe weiter, ohne dass
    /// ihn noch jemand beenden könnte. Der Anspruch wird unter demselben
    /// Schloss gesetzt, unter dem er geprüft wird.
    claimed: bool,
}

/// Was ein geglückter Start dem Aufrufer mitgibt.
///
/// Alles drei überlebt den Start und wird danach gebraucht: die Befunde des
/// Plans gehören in den Ereignisstrom, der Schnappschuss in die
/// Zusammenfassung am Ende, und die Kennung des Laufs braucht beide. Sie
/// stehen hier und nicht in [`Running`]: Ein Stopp räumt `running` weg, bevor
/// die begleitende Aufgabe fertig ist, und die Zusammenfassung wäre dann
/// verloren — mitsamt dem einzigen Schnappschuss, den es von vorher gibt.
#[derive(Debug)]
struct Launched {
    /// Befunde, die den Start nicht verhindert haben: eine aufgehobene Maske
    /// (`SANDBOX_020`), ein Kernel ohne `openat2` (`SANDBOX_021`), ein
    /// Projektverzeichnis, das sich nicht lesen ließ (`SANDBOX_011`).
    warnings: Vec<Diagnostic>,
    /// Der Blick auf `/work`, sobald der Lauf endet; `None`, wenn das Projekt
    /// nur lesbar eingehängt ist oder sich nicht lesen ließ.
    watch: Option<SummaryWatch>,
    /// Die Kennung dieses Laufs, zugleich der Schlüssel der Zusammenfassung.
    sandbox: SandboxId,
    /// Die Schicht einer Wegwerf-Sandbox (HUM-350); `None` für jeden anderen
    /// Lauf. Sie steht hier und nicht in [`Running`], aus demselben Grund
    /// wie der Schnappschuss: Ein Stopp räumt `running` weg, und die
    /// Änderungen dürfen dabei nicht verloren gehen.
    disposable: Option<crate::disposable::Waiting>,
}

/// Der Zuhörer für den Stand der Durchreiche eines Laufs (HUM-251).
///
/// Er entsteht vor dem Start des Agenten ([`Inner::listen_llm`]), beginnt zu
/// melden mit [`LlmWatch::run`] und endet mit [`LlmWatch::finish`]. Fällt er
/// ohne `finish` weg (ein früher Ausstieg aus `attend`, ein Start, der
/// scheitert, eine Aufgabe, die abgebrochen wird), bricht er seine Aufgabe
/// beim Fallenlassen selbst ab und leert den Stand: Ein Befund aus einem Lauf,
/// der nicht mehr läuft, wäre eine Behauptung über nichts.
#[derive(Debug)]
struct LlmWatch {
    inner: Arc<Inner>,
    /// Der Empfänger, bis [`LlmWatch::run`] ihn in die Aufgabe übergibt.
    events: Option<tokio::sync::broadcast::Receiver<FlowEvent>>,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl LlmWatch {
    /// Liest den Ereignisstrom, auch was seit der Anmeldung schon wartet, und
    /// schickt eine neue Momentaufnahme in den Strom dieses Starts, sobald
    /// sich `Status.llm` ändert.
    ///
    /// Wie bei den verweigerten Versuchen bekommt nur dieser eine Strom die
    /// Änderung; jeder andere Client liest den Befund, wenn er die
    /// Momentaufnahme holt.
    fn run(&mut self, plan: &v1::sandbox_request::Plan, tx: &mpsc::Sender<v1::SandboxEvent>) {
        let Some(mut events) = self.events.take() else {
            return;
        };
        let this = Arc::clone(&self.inner);
        let plan = plan.clone();
        let tx = tx.clone();
        self.task = Some(tokio::spawn(async move {
            loop {
                let event = match events.recv().await {
                    Ok(event) => event,
                    // Verpasste Ereignisse lassen den Stand, wie er ist; die
                    // nächste Durchreiche entscheidet ihn neu.
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
                };
                if !this.llm_reach.observe(&event) || tx.is_closed() {
                    continue;
                }
                send_snapshot(&this, &plan, &tx).await;
            }
        }));
    }

    /// Beendet den Zuhörer am Ende des Agenten. Stand ein Befund, geht eine
    /// Momentaufnahme ohne ihn in den Strom, damit die Oberfläche die Zeile
    /// nicht rot stehen lässt, nachdem der Lauf vorbei ist.
    async fn finish(
        mut self,
        plan: &v1::sandbox_request::Plan,
        tx: &mpsc::Sender<v1::SandboxEvent>,
    ) {
        if let Some(task) = self.task.take() {
            task.abort();
            // Bis der Abbruch gegriffen hat; danach schreibt die Aufgabe
            // nichts mehr in den Strom. Den Stand schützt ohnehin `end`.
            let _ = task.await;
        }
        if self.inner.llm_reach.end() {
            send_snapshot(&self.inner, plan, tx).await;
        }
    }
}

impl Drop for LlmWatch {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
        self.inner.llm_reach.end();
    }
}

/// Schickt die Momentaufnahme von `plan` in `tx`; scheitert ihr Bau, geht
/// nichts, denn die nächste Momentaufnahme sagt dasselbe.
async fn send_snapshot(
    inner: &Arc<Inner>,
    plan: &v1::sandbox_request::Plan,
    tx: &mpsc::Sender<v1::SandboxEvent>,
) {
    let inner = Arc::clone(inner);
    let wanted = plan.clone();
    let status = tokio::task::spawn_blocking(move || inner.snapshot(&wanted)).await;
    if let Ok(Ok(status)) = status {
        let _ = tx.send(status_event(status)).await;
    }
}

/// Der Anspruch auf den nächsten Start, solange er gehalten wird.
///
/// Er gibt sich beim Fallenlassen selbst frei, damit kein Fehlerpfad ihn
/// vergisst: Ein Anspruch, der nach einem gescheiterten Start liegen bliebe,
/// verweigerte jeden weiteren Start bis zum Ende des Daemons.
#[derive(Debug)]
struct StartClaim {
    inner: Arc<Inner>,
}

impl Drop for StartClaim {
    fn drop(&mut self) {
        let mut pending = lock(&self.inner.pending);
        pending.claimed = false;
        pending.starting_work_dir = None;
        drop(pending);
        self.inner.bump_generation();
    }
}

#[derive(Debug)]
struct Inner {
    /// Die Konfiguration, die gerade gilt.
    ///
    /// Sie steht hinter einem Schloss und nicht als Wert, weil ein Start sie
    /// ersetzt: `humanitl run --profile llm-only --ask none --llm …` löst für
    /// seine Sitzung neu auf, und was der Bildschirm danach zeigt, ist die
    /// Sitzung, die läuft, und nicht die Auflösung des Daemon-Starts
    /// (HUM-067).
    config: RwLock<Arc<Config>>,
    /// Whether the slot holds the configuration a start published for its
    /// session (HUM-277). Set by [`Inner::apply_session`], cleared when the
    /// next start takes its claim. A `Plan` answers from the slot only while
    /// this is set and that session still holds claim or handle; otherwise it
    /// resolves the configuration afresh ([`Inner::plan_config`]).
    session_config: std::sync::atomic::AtomicBool,
    /// Counts every change of who holds the sandbox and which configuration
    /// is published: claim, release, publication, handle set and cleared
    /// (HUM-277). A `Plan` built across such a change is built again
    /// ([`Inner::consistent`]).
    generation: std::sync::atomic::AtomicU64,
    /// Woher eine neue Auflösung kommt.
    resolver: SessionResolver,
    paths: humanitl_config::Paths,
    session: SessionId,
    running: Mutex<Option<Running>>,
    pending: Mutex<Pending>,
    /// Der Regelspeicher der Sitzung, falls einer geführt wird.
    ///
    /// Ein Start ersetzt seine mitgelieferte Gruppe: die Durchreiche zum
    /// Sprachmodell dieser Sitzung und die Regeln ihrer Profile. Ohne
    /// Speicher — im Fake-Modus und in Tests — bleibt die Auflösung ohne
    /// Wirkung auf die Regeln.
    rules: Option<Arc<RulesStore>>,
    /// Der Stand, den Proxy und Meta-Endpunkt lesen.
    settings: Option<Arc<SessionSettings>>,
    /// Woher die Hinweiszeilen im Terminal kommen (HUM-042).
    notices: Option<HeldNotices>,
    /// Wohin die Zusammenfassung eines Laufs geht (HUM-043).
    ///
    /// Ohne Aufzeichnung entsteht sie trotzdem und reist als Ereignis; nur
    /// nachschlagen lässt sie sich später nicht. Das ist der Fall im
    /// Fake-Modus und in Tests.
    recorder: Option<Recorder>,
    /// Wo der Zugangsschlüssel des Modells liegt (HUM-140).
    ///
    /// Ein Start holt ihn hier, gibt ihn dem Adapter und nimmt ihn vorher in
    /// die Masken der Aufzeichnung auf; ein Plan liest ihn nie.
    keys: KeyStore,
    /// Ob dieser Dienst seinen Abschied schon begonnen hat (HUM-142).
    ///
    /// Ab da startet nichts Neues mehr: Eine Sandbox, die nach dem Abschied
    /// anfinge, bekäme keinen geordneten mehr, sondern nur noch das `SIGKILL`,
    /// das `--die-with-parent` an das Ende des Daemons hängt. Gesetzt wird das
    /// in [`Inner::shutdown`], gelesen in [`Inner::claim_start`].
    leaving: std::sync::atomic::AtomicBool,
    /// Ob sich der Dienst ohne Sitzung verabschiedet hat (HUM-284).
    ///
    /// Gesetzt in [`Inner::retire_if_idle`], gelesen von
    /// [`SandboxService::retired`]: Der Daemon wartet darauf wie auf ein
    /// Signal und geht dann denselben Abschied.
    retired: tokio::sync::watch::Sender<bool>,
    /// Ob die Durchreiche an das Modell im laufenden Lauf zuletzt scheiterte
    /// (HUM-251); `Status.llm` liest es.
    llm_reach: LlmReach,
    /// Die obere Schicht einer beendeten Wegwerf-Sandbox, deren Änderungen
    /// auf den Menschen warten (HUM-350). Höchstens eine; ein neuer Start
    /// mit wartenden Änderungen wird abgelehnt.
    disposable: Mutex<Option<crate::disposable::Waiting>>,
}

/// Woran der Dienst hängt, wenn eine Sitzung startet.
///
/// Beide sind wahlfrei, weil beide fehlen dürfen: Im Fake-Modus und in Tests
/// gibt es keinen Regelspeicher und keinen laufenden Proxy, und ein Start
/// bleibt dort ein Start ohne Wirkung auf sie. Sie stehen zusammen in einem
/// Typ und nicht als zwei Argumente, damit ein dritter Anschluss die Signatur
/// nicht wieder ändert.
#[derive(Debug, Clone, Default)]
pub struct SandboxPorts {
    /// Der Regelspeicher, dessen mitgelieferte Gruppe ein Start ersetzt: die
    /// Durchreiche zum Sprachmodell dieser Sitzung und die Regeln ihrer
    /// Profile.
    pub rules: Option<Arc<RulesStore>>,
    /// Der Stand, den Proxy und Meta-Endpunkt lesen: Frage-Modus, Haltefrist
    /// und Sprachmodell.
    pub settings: Option<Arc<SessionSettings>>,
    /// Woher die Hinweiszeilen im Terminal kommen: der Ereignisstrom und die
    /// Registry, aus der sich ein gehaltener Fluss auflösen lässt (HUM-042).
    ///
    /// Ohne diesen Anschluss läuft die Sitzung ohne Hinweise. Das ist der
    /// Fall im Fake-Modus und in Tests; ein Daemon mit Proxy hat beide.
    pub notices: Option<HeldNotices>,
    /// Wohin die Zusammenfassung eines Laufs geht (HUM-043).
    ///
    /// Ohne Aufzeichnung entsteht sie trotzdem und reist als Ereignis; nur
    /// `GetSessionSummary` findet sie später nicht mehr.
    pub recorder: Option<Recorder>,
    /// Wo der Zugangsschlüssel des Modells liegt (HUM-140). Ohne Angabe der
    /// Speicher der Pfade des Resolvers; Tests setzen eine Attrappe ein.
    pub keys: Option<KeyStore>,
}

impl SandboxPorts {
    /// Ohne Anschlüsse: ein Dienst, dessen Start nichts außerhalb der Sandbox
    /// ändert.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Dieselben Anschlüsse mit einem Regelspeicher.
    #[must_use]
    pub fn with_rules(mut self, rules: Arc<RulesStore>) -> Self {
        self.rules = Some(rules);
        self
    }

    /// Dieselben Anschlüsse mit dem Stand der Sitzung.
    #[must_use]
    pub fn with_settings(mut self, settings: Arc<SessionSettings>) -> Self {
        self.settings = Some(settings);
        self
    }

    /// Dieselben Anschlüsse mit der Quelle der Terminal-Hinweise.
    #[must_use]
    pub fn with_notices(mut self, notices: HeldNotices) -> Self {
        self.notices = Some(notices);
        self
    }

    /// Dieselben Anschlüsse mit der Aufzeichnung, in der die Zusammenfassung
    /// eines Laufs liegen bleibt (HUM-043).
    #[must_use]
    pub fn with_recorder(mut self, recorder: Recorder) -> Self {
        self.recorder = Some(recorder);
        self
    }

    /// Dieselben Anschlüsse mit diesem Speicher für den Zugangsschlüssel.
    #[must_use]
    pub fn with_keys(mut self, keys: KeyStore) -> Self {
        self.keys = Some(keys);
        self
    }
}

impl SandboxService {
    /// Der Dienst für die Sitzung [`session`](SessionId).
    ///
    /// Der Resolver ersetzt die eingefrorene [`Config`] der ersten Fassung:
    /// Jeder Start löst für seine Sitzung neu auf, sonst hätten `--profile`,
    /// `--ask` und `--llm` keinen Weg in den Daemon.
    #[must_use]
    pub fn new(resolver: SessionResolver, session: SessionId, ports: SandboxPorts) -> Self {
        let paths = resolver.paths().clone();
        let config = Arc::new(resolver.base().config.clone());
        let keys = ports.keys.unwrap_or_else(|| KeyStore::for_paths(&paths));
        Self {
            inner: Arc::new(Inner {
                config: RwLock::new(config),
                session_config: std::sync::atomic::AtomicBool::new(false),
                generation: std::sync::atomic::AtomicU64::new(0),
                resolver,
                paths,
                session,
                running: Mutex::new(None),
                pending: Mutex::new(Pending::default()),
                rules: ports.rules,
                settings: ports.settings,
                notices: ports.notices,
                recorder: ports.recorder,
                keys,
                leaving: std::sync::atomic::AtomicBool::new(false),
                retired: tokio::sync::watch::Sender::new(false),
                llm_reach: LlmReach::default(),
                disposable: Mutex::new(None),
            }),
        }
    }

    /// Das Terminal der laufenden Sitzung.
    ///
    /// `sandbox_id` ist leer („die Sitzung, die läuft") oder nennt genau sie;
    /// ein Daemon führt eine. Jede andere Kennung ist eine Anfrage nach einer
    /// Sitzung, die es hier nicht gibt.
    ///
    /// # Errors
    ///
    /// `IPC_006`, wenn keine Sitzung läuft oder eine andere gemeint ist —
    /// derselbe Befund, mit dem `Sandbox` antwortet, wenn dieser Daemon keine
    /// Sandbox hat.
    pub fn terminal(&self, sandbox_id: &str) -> Result<TerminalHub, Diagnostic> {
        self.inner.terminal(sandbox_id)
    }

    /// Das Terminal, das ein `Open` meint: mit `shell_id` das einer weiteren
    /// Shell (HUM-299), sonst das des Agenten.
    ///
    /// # Errors
    ///
    /// `IPC_006` wie [`SandboxService::terminal`], und für eine Kennung,
    /// unter der keine Shell (mehr) lebt.
    pub fn terminal_for(&self, open: &v1::terminal_input::Open) -> Result<TerminalHub, Diagnostic> {
        let hub = self.inner.terminal(&open.sandbox_id)?;
        let wanted = open.shell_id.trim();
        if wanted.is_empty() {
            return Ok(hub);
        }
        let Some(shells) = self.inner.running_shells() else {
            return Err(crate::exec::no_such_shell(wanted, hub.sandbox(), &[]));
        };
        shells
            .get(wanted)
            .ok_or_else(|| crate::exec::no_such_shell(wanted, hub.sandbox(), &shells.ids()))
    }

    /// Öffnet eine weitere Shell in der laufenden Sandbox (HUM-299).
    ///
    /// Die Shell bekommt ein Terminal wie das des Agenten und, wie dieses,
    /// die Hinweiszeilen zu gehaltenen Flüssen. Ihr Leser läuft, bis sie
    /// endet; dann nimmt er ihr Terminal aus der Ablage.
    ///
    /// # Errors
    ///
    /// `IPC_006`, wenn keine Sitzung läuft oder eine andere gemeint ist;
    /// `TERM_002`, wenn die Sandbox keine Tür hat, beendet ist, schon vier
    /// weitere Shells offen sind oder der Shim ablehnt.
    pub async fn exec(&self, request: v1::ExecRequest) -> Result<v1::ExecResponse, Diagnostic> {
        let inner = Arc::clone(&self.inner);
        let opened = tokio::task::spawn_blocking(move || inner.open_shell(&request))
            .await
            .map_err(|err| {
                Diagnostic::builder(codes::TERM_002, Severity::Error)
                    .why(format!("opening the shell did not finish: {err}"))
                    .build()
            })??;
        let response = opened.response.clone();
        let notices = self
            .inner
            .notices
            .clone()
            .map(|notices| tokio::spawn(notices.run(opened.hub.clone())));
        tokio::spawn(async move {
            let _ = tokio::task::spawn_blocking(move || opened.run()).await;
            if let Some(task) = notices {
                task.abort();
            }
        });
        Ok(response)
    }

    /// Beendet die laufende Sandbox dieser Sitzung, mit Frist, und sagt, was
    /// es dazu gebraucht hat.
    ///
    /// Das ist der Abschied des Daemons (HUM-142): Er geht denselben Weg wie
    /// `Sandbox(Stop)` — `SIGTERM`, nach [`KILL_GRACE`] `SIGKILL` —, nur ohne
    /// Ereignisstrom, weil ihm niemand mehr zuhört. Die Reihenfolge ist
    /// festgelegt: **erst das Kind, dann der eigene Abschied.** Das Backend
    /// startet mit `--die-with-parent`; ein Daemon, der zuerst geht, ließe ein
    /// Kind zurück, das an ihm hing.
    ///
    /// Blockierend und nach oben beschränkt: zweimal [`KILL_GRACE`], also
    /// höchstens zehn Sekunden. Läuft keine Sandbox, geschieht nichts.
    ///
    /// Zurück kommt ein `SANDBOX_029`, wenn `SIGTERM` nicht gereicht hat oder
    /// der Prozess auch nach `SIGKILL` nicht eingesammelt war; sonst `None`.
    /// Der Aufrufer protokolliert ihn — ein Abschied, den niemand nennt, ist
    /// derselbe Hänger ohne Meldung wie vorher, nur kürzer.
    #[must_use]
    pub fn shutdown(&self) -> Option<Diagnostic> {
        self.inner.shutdown()
    }

    /// Schiebt den Riegel vor, ohne schon zu beenden (HUM-142).
    ///
    /// Ab hier bekommt kein `Start` mehr einen Anspruch: Eine Sandbox, die
    /// jetzt noch anfinge, bekäme keinen geordneten Abschied mehr, sondern nur
    /// das `SIGKILL`, das `--die-with-parent` an das Ende des Daemons hängt.
    ///
    /// Der Riegel gehört an das Signal und nicht in den Abschied selbst:
    /// Zwischen dem Signal und dem ersten Griff von [`SandboxService::shutdown`]
    /// läuft der Ausklang des gRPC-Dienstes, und eine Verbindung, die dort noch
    /// offen ist, könnte in genau dieser Lücke starten.
    ///
    /// Mehrfach zu rufen kostet nichts; [`SandboxService::shutdown`] setzt
    /// denselben Riegel noch einmal.
    pub fn begin_leaving(&self) {
        self.inner
            .leaving
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Ob der Abschied schon begonnen hat.
    ///
    /// Für den Aufrufer, der das Signal verdrahtet, und für seine Tests.
    #[must_use]
    pub fn is_leaving(&self) -> bool {
        self.inner.is_leaving()
    }

    /// Schiebt den Riegel vor, aber nur, wenn keine Sitzung läuft (HUM-284).
    ///
    /// Das ist der Abschied eines Daemons, dessen Programm ein Paket-Update
    /// ersetzt hat: Er geht, sobald niemand mehr in einer Sitzung arbeitet,
    /// und die Socket-Aktivierung startet bei der nächsten Verbindung das
    /// neue Programm. Eine Sitzung hält ihn, solange sie einen Anspruch oder
    /// ein Handle hat; auch eine, deren Agent sich beendet hat, denn ihr
    /// Terminal und ihre Zusammenfassung sieht ein Mensch noch an.
    ///
    /// Prüfen und Riegeln geschehen unter demselben Schloss, unter dem ein
    /// `Start` seinen Anspruch nimmt: Ein `Start`, der gleichzeitig anklopft,
    /// bekommt entweder seinen Anspruch, und dann geht der Daemon nicht, oder
    /// er findet den Riegel vor. Zurück kommt, ob der Riegel jetzt steht.
    #[must_use]
    pub fn retire_if_idle(&self) -> bool {
        self.inner.retire_if_idle()
    }

    /// Führt `work` nur aus, solange keine Sitzung die Sandbox hält oder
    /// startet, und hält während der Arbeit jeden neuen Start an derselben
    /// Stelle zurück wie [`SandboxService::retire_if_idle`] (HUM-289): Das
    /// Entfernen des aktiven Modellservers kann so nie zwischen die Auflösung
    /// eines Starts und dessen Sitzung geraten. `false`, wenn nicht
    /// ausgeführt.
    ///
    /// Blockiert, solange `work` läuft; ein Start wartet so lange.
    pub fn while_idle(&self, work: &mut dyn FnMut()) -> bool {
        self.inner.while_idle(work)
    }

    /// Wie [`SandboxService::while_idle`], aber nur ein Start mit Anspruch
    /// hält die Arbeit auf, eine laufende Sitzung nicht (HUM-289): Sie hat
    /// Konfiguration und Schlüssel schon, ein Start dagegen löst gerade auf
    /// und holt den Schlüssel erst noch.
    pub fn while_no_start(&self, work: &mut dyn FnMut()) -> bool {
        self.inner.while_no_start(work)
    }

    /// Wartet, bis [`SandboxService::retire_if_idle`] den Dienst ohne Sitzung
    /// verabschiedet hat (HUM-284), über `SandboxRequest.retire_if_idle` oder
    /// den Wächter über ein ersetztes Programm. Der Daemon wartet darauf neben
    /// `SIGTERM`; danach ist der Abschied derselbe.
    pub async fn retired(&self) {
        let mut retired = self.inner.retired.subscribe();
        let _ = retired.wait_for(|retired| *retired).await;
    }

    /// Der Ereignisstrom einer Sandbox-Operation.
    #[must_use]
    pub fn stream(&self, request: v1::SandboxRequest) -> BoxStream<v1::SandboxEvent> {
        use v1::sandbox_request::Op;

        let inner = Arc::clone(&self.inner);
        let (tx, rx) = mpsc::channel(EVENT_BUFFER);
        match request.op {
            Some(Op::Start(start)) => {
                tokio::spawn(async move { inner.start(start, tx).await });
            }
            Some(Op::Stop(())) => {
                tokio::spawn(async move { inner.stop(tx).await });
            }
            // Ein Schloss und ein Ereignis: Das blockiert nicht, und die
            // Antwort passt in den leeren Puffer (HUM-284).
            Some(Op::RetireIfIdle(())) => {
                let retired = inner.retire_if_idle();
                let _ = tx.try_send(retire_event(retired));
            }
            // Lesen heisst hier: ein Profil von der Platte lesen und eine
            // Kommandozeile bauen. Das ist blockierende Arbeit und gehoert
            // deshalb auf einen eigenen Faden, nicht in die Ereignisschleife.
            Some(Op::Argv(())) => {
                tokio::task::spawn_blocking(move || {
                    inner.argv(&v1::sandbox_request::Plan::default(), &tx);
                });
            }
            Some(Op::Plan(plan)) => {
                tokio::task::spawn_blocking(move || inner.snapshot_or_diagnostic(&plan, &tx));
            }
            // Die drei Garantien werden an der laufenden Sandbox gemessen.
            // Das ist blockierende Arbeit — der Bericht des Shims wird mit
            // Frist gelesen — und gehört deshalb auf einen eigenen Faden.
            Some(Op::IsolationCheck(())) => {
                tokio::task::spawn_blocking(move || inner.isolation_check(&tx));
            }
            // Die Wegwerf-Sandbox (HUM-350): Lesen, Prüfen und Schreiben im
            // Projekt sind blockierende Arbeit.
            Some(Op::Changes(())) => {
                tokio::task::spawn_blocking(move || inner.disposable_changes(&tx));
            }
            Some(Op::ApplyChanges(request)) => {
                tokio::task::spawn_blocking(move || inner.apply_changes(&request, &tx));
            }
            Some(Op::DiscardChanges(request)) => {
                tokio::task::spawn_blocking(move || inner.discard_changes(&request, &tx));
            }
            _ => {
                tokio::task::spawn_blocking(move || {
                    inner.snapshot_or_diagnostic(&v1::sandbox_request::Plan::default(), &tx);
                });
            }
        }
        Box::pin(ReceiverStream::new(rx))
    }
}

impl Inner {
    /// Nimmt den Anspruch auf den nächsten Start, wenn er frei ist.
    ///
    /// Prüfen und Setzen geschehen unter demselben Schloss; deshalb gewinnt
    /// von zwei gleichzeitigen Startversuchen genau einer, und der andere
    /// bekommt `CLI_005`. Ein laufender Agent hält den Anspruch ebenso wie
    /// ein Start, der gerade erst beginnt.
    ///
    /// Dies und [`Inner::retire_if_idle`] sind die einzigen Stellen, an denen
    /// `pending` und `running` ineinander gehalten werden, und die Reihenfolge
    /// ist beide Male `pending` vor `running`. Jede andere Stelle nimmt sie
    /// nacheinander (`work_dir`, `work_mode`, `profile_name`), also gibt es
    /// keinen Weg, sie andersherum zu halten.
    fn claim_start(self: &Arc<Self>) -> Option<StartClaim> {
        let mut pending = lock(&self.pending);
        if pending.claimed || self.is_running() || self.is_leaving() {
            return None;
        }
        pending.claimed = true;
        // The slot still holds the previous session's configuration; until
        // this start publishes its own, a `Plan` resolves afresh.
        self.session_config
            .store(false, std::sync::atomic::Ordering::SeqCst);
        self.bump_generation();
        Some(StartClaim {
            inner: Arc::clone(self),
        })
    }

    /// Ob ein Start gerade seinen Anspruch hält. Nimmt nur `pending`.
    fn start_claimed(&self) -> bool {
        lock(&self.pending).claimed
    }

    /// Siehe [`SandboxService::retire_if_idle`].
    ///
    /// Dieselbe Reihenfolge der Schlösser wie in [`Inner::claim_start`]:
    /// `pending` vor `running`.
    fn while_no_start(&self, work: &mut dyn FnMut()) -> bool {
        let pending = lock(&self.pending);
        if pending.claimed {
            return false;
        }
        work();
        drop(pending);
        true
    }

    fn while_idle(&self, work: &mut dyn FnMut()) -> bool {
        let pending = lock(&self.pending);
        if pending.claimed || lock(&self.running).is_some() {
            return false;
        }
        work();
        drop(pending);
        true
    }

    fn retire_if_idle(&self) -> bool {
        let pending = lock(&self.pending);
        if self.is_leaving() {
            return true;
        }
        if pending.claimed || lock(&self.running).is_some() {
            return false;
        }
        // Wartende Änderungen einer Wegwerf-Sandbox halten den Daemon
        // (HUM-350): Sein Abschied verwürfe sie, und eine Paketaktualisierung
        // ist kein Grund dafür.
        if lock(&self.disposable).is_some() {
            return false;
        }
        self.leaving
            .store(true, std::sync::atomic::Ordering::SeqCst);
        drop(pending);
        self.retired.send_replace(true);
        true
    }

    /// Die Konfiguration, die gerade gilt.
    ///
    /// Ein Aufrufer nimmt sie einmal und liest daraus alles, was er braucht:
    /// Zwei Zugriffe in derselben Antwort könnten sonst zwei verschiedene
    /// Sitzungen zeigen, wenn dazwischen eine startet.
    fn config(&self) -> Arc<Config> {
        self.config.read().map_or_else(
            |poisoned| Arc::clone(&poisoned.into_inner()),
            |slot| Arc::clone(&slot),
        )
    }

    /// The configuration a `Plan` or `Argv` answers from, taken once
    /// (HUM-277).
    ///
    /// While a session holds claim or handle and has published its
    /// configuration, that one: the snapshot shows what runs. Otherwise the
    /// configuration as it stands on disk now, so a value `SetConfig` or
    /// `LlmKey` wrote since the daemon started shows without a restart; a
    /// start that holds its claim but has not published yet resolves the
    /// same file. The whole snapshot is built from this one value, never from
    /// a second read of the slot, so endpoint and preview cannot come from two
    /// configurations. A resolution that fails falls back to the slot; the
    /// start reports the same diagnostic itself.
    fn plan_config(&self) -> Arc<Config> {
        let held = {
            let pending = lock(&self.pending);
            let held = pending.claimed || lock(&self.running).is_some();
            drop(pending);
            held
        };
        if held
            && self
                .session_config
                .load(std::sync::atomic::Ordering::SeqCst)
        {
            return self.config();
        }
        match self.resolver.current() {
            Ok(now) => Arc::new(now.config),
            Err(diagnostic) => {
                tracing::debug!(
                    code = %diagnostic.code,
                    why = %diagnostic.why,
                    "configuration not read afresh; the previous one answers"
                );
                self.config()
            }
        }
    }

    /// Puts the configuration as it stands on disk into the slot, for the
    /// start that holds the claim (HUM-277).
    ///
    /// The check of the project directory in [`Inner::remember`] reads the
    /// slot; without this it would still see the configuration of the
    /// daemon's start. A handle whose agent has exited does not stop it: the
    /// claim was granted because nothing runs, and this start replaces that
    /// session ([`start_may_refresh`]). A resolution that fails keeps the
    /// slot, and the start reports it.
    fn refresh_for_start(&self) {
        if !start_may_refresh(&self.running_facts()) {
            return;
        }
        if let Ok(now) = self.resolver.current() {
            let mut slot = self.config.write().unwrap_or_else(PoisonError::into_inner);
            *slot = Arc::new(now.config);
        }
    }

    /// Prüft den Wunsch eines Starts und merkt ihn sich für diesen Start.
    ///
    /// Leere Felder ändern nichts: `Status` schickt einen leeren Wunsch und
    /// darf die Wahl nicht löschen. Ein Ordner gilt nur für den Start, der
    /// den Anspruch hält ([`Pending::starting_work_dir`], HUM-293).
    ///
    /// **Geprüft wird vor dem Merken, und gemerkt wird nur ein Wunsch, der
    /// ganz durchkommt.** Der Socket ist die Vertrauensgrenze, nicht die
    /// Oberfläche: Was hier hereinkommt, entscheidet, welches Profil die
    /// Sandbox stellt und welches Verzeichnis sie als `/work` sieht. Ein
    /// halb übernommener Wunsch hinterließe einen Zustand, den niemand
    /// gewählt hat.
    ///
    /// # Errors
    ///
    /// `CONFIG_003`, wenn der Profilname kein Name ist, `SANDBOX_006`, wenn
    /// das Verzeichnis keines ist, das ein Projekt sein darf.
    fn remember(&self, plan: &v1::sandbox_request::Plan) -> Result<Option<PathBuf>, Diagnostic> {
        self.remember_with(&self.config(), plan)
    }

    /// [`Inner::remember`] against this configuration.
    fn remember_with(
        &self,
        config: &Config,
        plan: &v1::sandbox_request::Plan,
    ) -> Result<Option<PathBuf>, Diagnostic> {
        let wish = self.plan_choice(config, plan)?;
        self.commit(&wish);
        let mut pending = lock(&self.pending);
        pending.starting_work_dir = wish.work_dir.clone().map(|dir| {
            (
                dir,
                wish.work_dir_source
                    .clone()
                    .unwrap_or(WorkDirSource::Request),
            )
        });
        drop(pending);
        Ok(wish.work_dir)
    }

    /// What this wish chooses, checked against `config`, without writing
    /// anything (HUM-277). Fields the wish leaves empty stay `None`.
    fn plan_choice(
        &self,
        config: &Config,
        plan: &v1::sandbox_request::Plan,
    ) -> Result<PlanChoice, Diagnostic> {
        let profile = match plan.profile.trim() {
            "" => None,
            name => {
                check_profile_name(name)?;
                Some(name.to_owned())
            }
        };
        let (work_dir, work_dir_source) = match plan.work_dir.trim() {
            "" => (None, None),
            dir => {
                let from = crate::work_dir::source_from_proto(plan.work_dir_source.as_ref());
                (
                    Some(self.check_work_dir(config, Path::new(dir), &from)?),
                    Some(from),
                )
            }
        };
        let work_mode = match plan.work_mode.trim().to_ascii_lowercase().as_str() {
            "ro" => Some(WorkMode::Ro),
            "rw" => Some(WorkMode::Rw),
            _ => None,
        };
        Ok(PlanChoice {
            profile,
            work_dir,
            work_dir_source,
            work_mode,
        })
    }

    /// Writes the profile and mode of a checked choice into `pending`, in
    /// one locked step. The folder of a choice is never kept (HUM-293); the
    /// caller carries the checked folder of **this** wish on, rather than
    /// fetching it from `pending` later, when another call could have put a
    /// different one there.
    fn commit(&self, wish: &PlanChoice) {
        let mut pending = lock(&self.pending);
        if let Some(profile) = &wish.profile {
            pending.profile = Some(profile.clone());
        }
        if let Some(mode) = wish.work_mode {
            pending.work_mode = Some(mode);
        }
    }

    /// The choices in `pending` now.
    fn pending_choice(&self) -> PlanChoice {
        let pending = lock(&self.pending);
        let (work_dir, work_dir_source) = pending
            .starting_work_dir
            .clone()
            .map_or((None, None), |(dir, from)| (Some(dir), Some(from)));
        PlanChoice {
            profile: pending.profile.clone(),
            work_dir,
            work_dir_source,
            work_mode: pending.work_mode,
        }
    }

    /// `pending` as it would stand after committing `wish`, without writing.
    fn staged(&self, wish: &PlanChoice) -> PlanChoice {
        let now = self.pending_choice();
        PlanChoice {
            profile: wish.profile.clone().or(now.profile),
            work_dir_source: if wish.work_dir.is_some() {
                wish.work_dir_source.clone()
            } else {
                now.work_dir_source
            },
            work_dir: wish.work_dir.clone().or(now.work_dir),
            work_mode: wish.work_mode.or(now.work_mode),
        }
    }

    /// Prüft ein Projektverzeichnis, das über die RPC hereinkommt, gegen
    /// `config` ([`crate::work_dir::check_requested`]). `from` ist die
    /// Quelle, die der Client nennt; sie steht in jedem Befund (HUM-293).
    ///
    /// # Errors
    ///
    /// `SANDBOX_006` mit dem Grund, in der Sprache des Nutzers.
    fn check_work_dir(
        &self,
        config: &Config,
        dir: &Path,
        from: &WorkDirSource,
    ) -> Result<PathBuf, Diagnostic> {
        crate::work_dir::check_requested(&self.paths, config.sandbox.work_dir.as_deref(), dir, from)
    }

    /// Der Schnappschuss zu diesem Wunsch, oder der Befund, der ihn verhindert.
    fn snapshot_or_diagnostic(
        &self,
        plan: &v1::sandbox_request::Plan,
        tx: &mpsc::Sender<v1::SandboxEvent>,
    ) {
        for event in self.consistent_plan(plan, || {}) {
            let _ = tx.blocking_send(event);
        }
    }

    /// The events of a `Plan`, built until claim, publication and handle held
    /// still (HUM-277).
    ///
    /// No build writes `pending`: each stages its choice and builds the
    /// snapshot from it, and only the answered build commits it. A directory
    /// one configuration allowed and the next refuses is never left for a
    /// later start to inherit, however many `Plan`s run at once.
    /// `between` runs after each build; tests use it to change the session at
    /// exactly that point.
    fn consistent_plan(
        &self,
        plan: &v1::sandbox_request::Plan,
        mut between: impl FnMut(),
    ) -> Vec<v1::SandboxEvent> {
        let (events, wish) = self.consistent(
            |config| {
                let built = self.plan_events(config, plan);
                between();
                built
            },
            drop,
        );
        if let Some(wish) = wish {
            self.commit(&wish);
        }
        events
    }

    /// Builds with one configuration, and again when claim, publication or
    /// handle changed meanwhile (HUM-277).
    ///
    /// Without this a `Plan` could take its configuration before a start
    /// publishes and its running facts after: a new sandbox id beside the old
    /// endpoint. At most [`SNAPSHOT_ATTEMPTS`] builds; a daemon that changes
    /// its session that often answers with the last one.
    fn consistent<T>(
        &self,
        mut build: impl FnMut(&Arc<Config>) -> T,
        mut discard: impl FnMut(T),
    ) -> T {
        let mut attempt = 1;
        loop {
            let before = self.generation();
            let config = self.plan_config();
            let answer = build(&config);
            if self.generation() == before || attempt >= SNAPSHOT_ATTEMPTS {
                return answer;
            }
            discard(answer);
            attempt += 1;
        }
    }

    fn generation(&self) -> u64 {
        self.generation.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn bump_generation(&self) {
        self.generation
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    /// The events of a `Plan`, from this one configuration.
    fn plan_events(
        &self,
        config: &Arc<Config>,
        plan: &v1::sandbox_request::Plan,
    ) -> (Vec<v1::SandboxEvent>, Option<PlanChoice>) {
        let mut events = Vec::new();
        let wish = match self.plan_choice(config, plan) {
            Ok(wish) => wish,
            Err(diagnostic) => {
                events.push(diagnostic_event(&diagnostic));
                // Der abgelehnte Wunsch steht in keiner Momentaufnahme: Sie zeigte
                // sonst das Verzeichnis, das gerade verweigert wurde.
                events.push(status_event(self.plan_failed_status(
                    config,
                    &self.pending_choice(),
                    &v1::sandbox_request::Plan::default(),
                    &diagnostic,
                )));
                return (events, None);
            }
        };
        let staged = self.staged(&wish);
        // Erst die Befunde, dann der Zustand: Der Client trägt die Befunde des
        // laufenden Vorgangs am Zustand mit, und einer, der nach dem Zustand
        // käme, stünde ohne ihn da.
        events.extend(self.preflight().iter().map(diagnostic_event));
        match self.snapshot_with_config(config, &staged, plan, None) {
            Ok(status) => events.push(status_event(status)),
            Err(diagnostic) => {
                events.push(diagnostic_event(&diagnostic));
                events.push(status_event(self.plan_failed_status(
                    config,
                    &staged,
                    plan,
                    &diagnostic,
                )));
            }
        }
        (events, Some(wish))
    }

    /// [`Inner::failed_status_with`] für einen `Plan` oder `Status`.
    ///
    /// Hält gerade ein Start seinen Anspruch und noch kein Handle, ist der
    /// Zustand `starting`, auch wenn der Plan dieses Aufrufers nicht aufgeht
    /// (HUM-284): Wer fragt, ob eine Sitzung läuft, darf in diesem Fenster
    /// nicht „keine" lesen. Der Start selbst meldet sein Scheitern weiter mit
    /// `failed` ([`Inner::failed_status`]).
    fn plan_failed_status(
        &self,
        config: &Config,
        choice: &PlanChoice,
        plan: &v1::sandbox_request::Plan,
        diagnostic: &Diagnostic,
    ) -> v1::sandbox_event::Status {
        let mut status = self.failed_status_with(config, choice, plan, diagnostic);
        if status.sandbox_id.is_empty() && self.start_claimed() {
            status.state = v1::SandboxState::Starting.into();
        }
        status
    }

    /// Was einem Start im Weg steht, bevor jemand ihn versucht.
    ///
    /// Heute genau eine Frage: Gibt es `bwrap`, und ist es neu genug
    /// (`PlatformBackend::detect`, `SANDBOX_001` bis `SANDBOX_003`)? Der
    /// Schnappschuss selbst kommt auch ohne `bwrap` zustande — er zeigt, was
    /// starten *würde* —, aber die Schaltfläche gehört dann aus, und der Grund
    /// dazu (`docs/UX.md` 5.3). Die drei Garantien prüft HUM-041, und sie
    /// lassen sich ohnehin erst an einer laufenden Sandbox prüfen.
    fn preflight(&self) -> Vec<Diagnostic> {
        match PlatformBackend::detect(self.paths.clone()) {
            Ok(_) => Vec::new(),
            Err(diagnostic) => vec![diagnostic],
        }
    }

    /// Die Kommandozeile, Argument für Argument, als eigene Ereignisse.
    fn argv(&self, plan: &v1::sandbox_request::Plan, tx: &mpsc::Sender<v1::SandboxEvent>) {
        match self.consistent(
            |config| {
                self.prepare_keyed_with(config, &self.pending_choice(), plan, &[], KeyNeed::Plan)
            },
            drop,
        ) {
            Ok(prepared) => {
                for arg in prepared.argv() {
                    let line = arg.to_string_lossy().into_owned();
                    if tx
                        .blocking_send(v1::SandboxEvent {
                            event: Some(v1::sandbox_event::Event::ArgvLine(line)),
                        })
                        .is_err()
                    {
                        return;
                    }
                }
            }
            Err(diagnostic) => {
                let _ = tx.blocking_send(diagnostic_event(&diagnostic));
            }
        }
    }

    /// Startet die Sandbox und meldet `starting`, dann `running`.
    ///
    /// Läuft schon eine, ist der Start keine Änderung: Der Strom meldet den
    /// laufenden Zustand und endet. Die Oberfläche schaltet die Schaltfläche
    /// ohnehin ab; ein Befund an dieser Stelle wäre ein Fehler ohne Ursache.
    ///
    /// **Die Reihenfolge ist eine Aussage.** Geprüft wird, bevor irgendetwas
    /// gilt: erst der Profilname und das Projektverzeichnis (der Socket ist
    /// die Vertrauensgrenze, `backlog/CONVENTIONS.md` 4.17), dann die
    /// Konfiguration dieser Sitzung. Erst wenn beides durchkommt, gelten die
    /// Regeln der Sitzung, ihre Haltefrist und ihre Durchreiche; ein halb
    /// übernommener Wunsch hinterließe einen Zustand, den niemand gewählt hat.
    async fn start(
        self: Arc<Self>,
        start: v1::sandbox_request::Start,
        tx: mpsc::Sender<v1::SandboxEvent>,
    ) {
        let plan = plan_of(&start);
        // `direct` or `disposable` (HUM-350); a word that is neither ends the
        // start before it claims anything. The buffer is empty here.
        let Ok(mode) = crate::disposable::parse_mode(&start.sandbox_mode)
            .map_err(|diagnostic| tx.try_send(diagnostic_event(&diagnostic)))
        else {
            return;
        };
        // Der Anspruch zuerst, vor jeder Prüfung, die etwas merkt: Ein
        // zweiter Start soll `pending` nicht anfassen, während der erste
        // daraus seine Sitzung auflöst.
        let Some(_claim) = self.claim_start() else {
            // Der Befund zuerst, dann der Zustand. Die Oberfläche schaltet die
            // Schaltfläche ohnehin ab und sieht ihn selten; `humanitl run`
            // dagegen bekäme sonst nur einen laufenden Zustand ohne Ausgabe
            // und ohne Exit-Code und wüsste nicht, warum (HUM-067).
            let refused = if self.is_leaving() {
                leaving_daemon()
            } else {
                already_running(&self.running_facts())
            };
            let _ = tx.send(diagnostic_event(&refused)).await;
            let this = Arc::clone(&self);
            let plan = plan.clone();
            let tx = tx.clone();
            let _ =
                tokio::task::spawn_blocking(move || this.snapshot_or_diagnostic(&plan, &tx)).await;
            return;
        };

        // What `SetConfig` wrote since the daemon started counts for the
        // check of the project directory, too (HUM-277).
        {
            let this = Arc::clone(&self);
            let _ = tokio::task::spawn_blocking(move || this.refresh_for_start()).await;
        }
        let work_dir = match self.remember(&plan) {
            Ok(work_dir) => work_dir,
            Err(diagnostic) => {
                let _ = tx.send(diagnostic_event(&diagnostic)).await;
                let _ = tx
                    .send(status_event(self.failed_status(
                        &v1::sandbox_request::Plan::default(),
                        &diagnostic,
                    )))
                    .await;
                return;
            }
        };

        if !self.settle_session(&start, work_dir, &plan, &tx).await {
            return;
        }

        let starting = {
            let this = Arc::clone(&self);
            let plan = plan.clone();
            tokio::task::spawn_blocking(move || {
                this.snapshot_with(&plan, Some(v1::SandboxState::Starting))
            })
            .await
        };
        match starting {
            Ok(Ok(status)) => {
                if tx.send(status_event(status)).await.is_err() {
                    return;
                }
            }
            Ok(Err(diagnostic)) => {
                let _ = tx.send(diagnostic_event(&diagnostic)).await;
                let _ = tx
                    .send(status_event(self.failed_status(&plan, &diagnostic)))
                    .await;
                return;
            }
            Err(error) => {
                let diagnostic = joined_failed(&error);
                let _ = tx.send(diagnostic_event(&diagnostic)).await;
                return;
            }
        }

        // Der Zuhörer der Durchreiche vor dem Agenten, damit keine erste
        // Anfrage an ihm vorbeigeht (HUM-251).
        let llm = self.listen_llm();
        let this = Arc::clone(&self);
        let launch_plan = plan.clone();
        let command: Vec<OsString> = start.command.iter().map(OsString::from).collect();
        // Die Ausgabe des Agenten läuft über einen eigenen Kanal, nicht über
        // den gesammelten Puffer: Der Puffer gibt erst nach dem Ende etwas
        // heraus und ist gedeckelt, und ein Mensch, der `humanitl run` tippt,
        // will sehen, was gerade geschieht.
        let (output_tx, output_rx) = std::sync::mpsc::channel();
        let launched = tokio::task::spawn_blocking(move || {
            this.launch(&launch_plan, &command, mode, Some(output_tx))
        })
        .await;
        match launched {
            Ok(Ok(run)) => {
                // Die Befunde des Plans zuerst: eine aufgehobene Maske
                // (`SANDBOX_020`) und ein Kernel ohne `openat2`
                // (`SANDBOX_021`) gehören zu dem Lauf, der gerade beginnt, und
                // nicht hinter seine Ausgabe.
                for diagnostic in &run.warnings {
                    if tx.send(diagnostic_event(diagnostic)).await.is_err() {
                        break;
                    }
                }
                self.accompany(&plan, run, output_rx, llm, &tx).await;
            }
            Ok(Err(diagnostic)) => {
                let _ = tx.send(diagnostic_event(&diagnostic)).await;
                let _ = tx
                    .send(status_event(self.failed_status(&plan, &diagnostic)))
                    .await;
            }
            Err(error) => {
                let diagnostic = joined_failed(&error);
                let _ = tx.send(diagnostic_event(&diagnostic)).await;
            }
        }
    }

    /// Begleitet die gestartete Sitzung bis zum Ende des Agenten und danach
    /// bis zur Zusammenfassung.
    ///
    /// Die Reihenfolge ist eine Aussage: erst die Zeile für das Log, dann die
    /// drei Garantien (eine rote beendet die Sitzung hier), dann `running`,
    /// und erst danach die Ausgabe. Wer `running` sieht, hat die drei
    /// Ergebnisse schon gesehen.
    ///
    /// **Dies ist der Wach-Task auf den Lauf**, und es braucht keinen zweiten:
    /// [`SandboxService::stream`] startet [`Inner::start`] in einer eigenen
    /// Aufgabe, die nicht am Ereignisstrom des Clients hängt. Wer seinen Strom
    /// schließt, beendet damit weder das Lesen der Ausgabe
    /// ([`Inner::stream_output`] merkt es sich in `forward`) noch diese
    /// Aufgabe. Die Zusammenfassung entsteht deshalb auch dann, wenn niemand
    /// mehr zusieht, und sie entsteht auf **beiden** Wegen hinaus: nach einem
    /// gewöhnlichen Ende ebenso wie nach einer roten Garantie. Was der Agent in
    /// den Millisekunden bis zum `SIGKILL` geschrieben hat, gehört zu dem, was
    /// ein Mensch danach sehen muss.
    async fn accompany(
        self: &Arc<Self>,
        plan: &v1::sandbox_request::Plan,
        run: Launched,
        output: std::sync::mpsc::Receiver<humanitl_sandbox::OutputChunk>,
        llm: Option<LlmWatch>,
        tx: &mpsc::Sender<v1::SandboxEvent>,
    ) {
        let watch = run.watch;
        let sandbox = run.sandbox;
        let disposable = run.disposable;
        self.attend(plan, output, llm, tx).await;
        match disposable {
            #[cfg(target_os = "linux")]
            Some(waiting) => self.settle_disposable(waiting, tx).await,
            // Ohne Overlay entsteht nie eine Schicht (HUM-350).
            #[cfg(not(target_os = "linux"))]
            Some(waiting) => match waiting {},
            None => self.summarize(watch, sandbox, tx).await,
        }
    }

    /// Nach dem Ende einer Wegwerf-Sandbox: die Grenze anhalten, die Schicht
    /// ablegen und die Änderungen einmal schicken, ohne dass ein Client
    /// fragen muss (HUM-350).
    #[cfg(target_os = "linux")]
    async fn settle_disposable(
        self: &Arc<Self>,
        mut waiting: crate::disposable::Waiting,
        tx: &mpsc::Sender<v1::SandboxEvent>,
    ) {
        if let Some(guard) = waiting.guard.take() {
            guard.stop();
            if let Some(tripped) = guard.tripped() {
                let _ = tx.send(diagnostic_event(&tripped)).await;
            }
        }
        let this = Arc::clone(self);
        let listed = tokio::task::spawn_blocking(move || {
            let changes = waiting.changes();
            // Ein Lauf ohne Änderungen hinterlässt nichts zu entscheiden: Die
            // Schicht geht sofort, die Aufzeichnung sagt `no_changes`, und
            // der nächste Start mit der Vorgabe `auto` wartet auf nichts
            // (HUM-357).
            if let Ok(listed) = &changes
                && crate::disposable::nothing_to_decide(listed)
            {
                tracing::info!(
                    session = %this.session,
                    sandbox = %waiting.sandbox,
                    "a disposable sandbox ended without changes; its layer is gone"
                );
                return Err(this.drop_layer(waiting, crate::disposable::NO_CHANGES));
            }
            tracing::info!(
                session = %this.session,
                sandbox = %waiting.sandbox,
                layer = %waiting.layer.dir().display(),
                "the changes of a disposable sandbox wait for apply or discard"
            );
            *lock(&this.disposable) = Some(waiting);
            Ok(changes)
        })
        .await;
        let event = match listed {
            Ok(Ok(Ok(changes))) => v1::SandboxEvent {
                event: Some(v1::sandbox_event::Event::Changes(changes)),
            },
            Ok(Ok(Err(diagnostic)) | Err(Some(diagnostic))) => diagnostic_event(&diagnostic),
            // Nichts zu entscheiden und die Schicht ist weg: kein Panel.
            Ok(Err(None)) => return,
            Err(error) => diagnostic_event(&joined_failed(&error)),
        };
        let _ = tx.send(event).await;
    }

    /// Der Lauf selbst: Log-Zeile, Garantien, `running`, Ausgabe, Exit-Code.
    ///
    /// `llm` ist der Zuhörer der Durchreiche, angemeldet vor dem Start des
    /// Agenten; ein früher Ausstieg lässt ihn fallen, und das leert den Stand.
    async fn attend(
        self: &Arc<Self>,
        plan: &v1::sandbox_request::Plan,
        output: std::sync::mpsc::Receiver<humanitl_sandbox::OutputChunk>,
        mut llm: Option<LlmWatch>,
        tx: &mpsc::Sender<v1::SandboxEvent>,
    ) {
        // Die Zeile, die der Log-Reiter zeigt. Der leere Zustand dort
        // verspricht sie („ein Start und ein Stopp schreiben je eine Zeile"),
        // also muss es sie geben; das Terminal des Agenten ist etwas anderes
        // und läuft über `Terminal`.
        if let Some(line) = self.started_line() {
            let _ = tx.send(log_event(line)).await;
        }
        if !self.check_isolation_or_kill(plan, tx).await {
            return;
        }
        let this = Arc::clone(self);
        let running_plan = plan.clone();
        let running = tokio::task::spawn_blocking(move || this.snapshot(&running_plan)).await;
        if let Ok(Ok(status)) = running {
            let _ = tx.send(status_event(status)).await;
        }
        // Das Terminal dieser Sitzung, und die Hinweise darin. Der Zuhörer der
        // Warteschlange wird hier angemeldet und nicht im Start-Faden: Er
        // gehört in die Ereignisschleife und nicht auf den blockierenden
        // Faden, der `bwrap` startet.
        let Ok(hub) = self.terminal("") else {
            // Zwischen Start und hier hat jemand die Sitzung beendet.
            return;
        };
        // Die Aufgabe hängt an der Warteschlange, und die lebt länger als
        // diese Sitzung. Ohne den Abbruch bliebe je beendeter Sitzung eine
        // Aufgabe stehen, die einen `TerminalHub` hält — und mit ihm den
        // `SandboxHandle` und die Herrscherseite des Pseudoterminals. Sie
        // könnte außerdem noch in den Ring einer Sitzung schreiben, die es
        // nicht mehr gibt.
        let notices = self
            .notices
            .clone()
            .map(|notices| tokio::spawn(notices.run(hub.clone())));
        let refusals = self.forward_refusals(tx);
        if let Some(llm) = llm.as_mut() {
            llm.run(plan, tx);
        }
        let first = self.stream_output(output, hub, tx).await;
        if let Some(task) = notices {
            task.abort();
        }
        // Vor dem Exit-Code: Ein Client, der beim Exit aufhört zu lesen, soll
        // die Zeile „LLM“ nicht rot stehen lassen (HUM-251).
        if let Some(llm) = llm {
            llm.finish(plan, tx).await;
        }
        // Der letzte Stand der verweigerten Versuche vor dem Exit-Code, damit
        // ein Client, der beim Exit aufhört zu lesen, ihn noch hat (HUM-138).
        if let Some(task) = refusals {
            let _ = tokio::time::timeout(REFUSALS_DRAIN + REFUSALS_POLL, task).await;
        }
        self.report_exit(&first, tx).await;
    }

    /// Löst die Konfiguration dieser Sitzung auf, sendet ihre Befunde und sagt,
    /// ob der Start weitergehen darf.
    ///
    /// Auflösen liest Dateien und gehört deshalb auf einen eigenen Faden.
    async fn settle_session(
        self: &Arc<Self>,
        start: &v1::sandbox_request::Start,
        work_dir: Option<PathBuf>,
        plan: &v1::sandbox_request::Plan,
        tx: &mpsc::Sender<v1::SandboxEvent>,
    ) -> bool {
        let this = Arc::clone(self);
        let wish = start.clone();
        let applied =
            tokio::task::spawn_blocking(move || this.apply_session(&wish, work_dir)).await;
        match applied {
            Ok(Ok(diagnostics)) => {
                for diagnostic in &diagnostics {
                    if tx.send(diagnostic_event(diagnostic)).await.is_err() {
                        return false;
                    }
                }
                true
            }
            Ok(Err(diagnostic)) => {
                let _ = tx.send(diagnostic_event(&diagnostic)).await;
                let _ = tx
                    .send(status_event(self.failed_status(plan, &diagnostic)))
                    .await;
                false
            }
            Err(error) => {
                let diagnostic = joined_failed(&error);
                let _ = tx.send(diagnostic_event(&diagnostic)).await;
                false
            }
        }
    }

    /// Löst die Konfiguration dieser Sitzung auf und lässt sie gelten.
    ///
    /// Drei Dinge ändern sich damit für die Dauer der Sitzung, und alle drei
    /// hängen an derselben Auflösung:
    ///
    /// 1. **Die Konfiguration**, aus der Kommandozeile, Umgebung, Profilen und
    ///    `config.toml` (`backlog/CONVENTIONS.md` 4.23). Alles, was der Dienst
    ///    danach liest — Adapter, Befehl, Umgebung, Sprachmodell —, kommt aus
    ///    ihr.
    /// 2. **Die mitgelieferte Gruppe des Regelspeichers**: die Durchreiche zum
    ///    Sprachmodell dieser Sitzung und die Regeln ihrer Profile (Rang 4).
    ///    Ohne diesen Schritt brächte `--profile llm-only` seine Blockregel
    ///    nicht mit, und `--llm` zeigte auf ein Modell, zu dem keine
    ///    Durchreiche führt.
    /// 3. **Frage-Modus und Haltefrist**, die der Proxy je gehaltenem Fluss
    ///    liest, und der Endpunkt, den `http://humanitl.internal/` nennt.
    ///
    /// Zurück kommen die Befunde, die die Auflösung überlebt haben: ein
    /// verdecktes Profil, ein Projekt-Profil aus fremdem Besitz, eine
    /// Regeldatei, die sich nicht lesen ließ. Sie halten den Start nicht auf,
    /// aber sie werden gesagt.
    ///
    /// # Errors
    ///
    /// `CONFIG_001` bis `CONFIG_003`, wenn Profil, Frage-Modus oder ein
    /// Konfigurationswert des Clients nicht durchkommen.
    fn apply_session(
        &self,
        start: &v1::sandbox_request::Start,
        work_dir: Option<PathBuf>,
    ) -> Result<Vec<Diagnostic>, Diagnostic> {
        let request = SessionRequest {
            profile: non_empty(&start.session_profile),
            // Das Verzeichnis ist zu diesem Zeitpunkt geprüft: `remember` hat
            // es aufgelöst und unter demselben Schloss zurückgegeben, unter dem
            // es abgelegt wurde. Der geprüfte Pfad reist weiter, nie der
            // geschriebene, und nie einer, den inzwischen jemand anders
            // hineingelegt hat.
            work_dir,
            work_mode: parse_work_mode(&start.work_mode),
            ask_mode: parse_ask_mode(&start.ask_mode)?,
            overrides: start
                .cli_overrides
                .iter()
                .map(|entry| (entry.path.trim().to_owned(), entry.value.clone()))
                .collect(),
        };
        let resolved = self.resolver.resolve(&request)?;
        let mut diagnostics = resolved.diagnostics.clone();

        if let Some(store) = self.rules.as_ref() {
            let (rules, found) = bundled_rules(&resolved.config, &resolved.profiles, self.session);
            diagnostics.extend(found);
            store.set_bundled(&rules.all());
            tracing::info!(
                session = %self.session,
                rules = rules.len(),
                profiles = resolved.profiles.len(),
                "session rules installed"
            );
        }
        // Was `--llm` aufmacht, wird gesagt, bevor der Agent läuft: Die
        // Durchreiche steht in Rang 1, wird nicht gehalten und überholt die
        // eigenen Block-Regeln des Nutzers. Geprüft wird ohne Auflösung — ein
        // Name verlässt den Rechner erst, wenn eine Anfrage freigegeben ist
        // (ADR-006).
        if let Some(endpoint) = resolved.config.llm.endpoint.as_ref()
            && let Some(diagnostic) = humanitl_proxy::not_private_by_name(endpoint)
        {
            diagnostics.push(diagnostic);
        }
        if let Some(settings) = self.settings.as_ref() {
            settings.set(SessionState::for_config(
                resolved.config.hold.ask_mode,
                resolved.config.hold.timeout_secs,
                llm_authority(&resolved.config),
            ));
        }
        tracing::info!(
            session = %self.session,
            ask_mode = ask_mode_name(resolved.config.hold.ask_mode),
            timeout_secs = resolved.config.hold.timeout_secs,
            profiles = ?resolved.profiles.iter().map(|profile| profile.name.as_str()).collect::<Vec<_>>(),
            "session configuration resolved"
        );

        let mut slot = self.config.write().unwrap_or_else(PoisonError::into_inner);
        *slot = Arc::new(resolved.config);
        drop(slot);
        self.session_config
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.bump_generation();
        Ok(diagnostics)
    }

    /// Reicht die Ausgabe des Agenten weiter, bis sein Strom endet.
    ///
    /// Der Faden liest die rohen Stücke **einmal** und bedient damit zwei
    /// Wege mit zwei Zusagen:
    ///
    /// - Das Terminal ([`TerminalHub::feed`]) bekommt jedes Stück, solange die
    ///   Sitzung läuft. Es filtert mit
    ///   [`humanitl_core::TerminalPolicy::FullScreen`], legt das Ergebnis in
    ///   seinen Ringpuffer und schickt es an jeden, der zusieht.
    /// - Der Ereignisstrom dieses Starts bekommt dieselben Bytes durch
    ///   [`humanitl_core::TerminalPolicy::ColourOnly`], je Strom einen Filter:
    ///   Wer `humanitl run` tippt, schreibt sie unverändert in sein eigenes
    ///   Terminal, und dort darf keine Folge landen, die eine Zeile
    ///   überschreibt, die schon steht (`backlog/CONVENTIONS.md` 4.26).
    ///
    /// Ein Client, der seinen Ereignisstrom schließt, beendet deshalb **nicht**
    /// das Lesen: Das Terminal hängt daran, und es gehört nicht dem, der als
    /// erster gestartet hat.
    ///
    /// Blockierend, denn der Kanal des Lesers ist einer der Standardbibliothek;
    /// das Warten gehört deshalb auf einen eigenen Faden.
    ///
    /// Zurück kommt, ob überhaupt ein Byte kam. Daran entscheidet
    /// [`Inner::report_exit`] den Fall ohne Bericht des Shims (HUM-137);
    /// gelesen wird es hier, weil nur dieser Faden jedes Stück sieht.
    async fn stream_output(
        &self,
        rx: std::sync::mpsc::Receiver<humanitl_sandbox::OutputChunk>,
        hub: TerminalHub,
        tx: &mpsc::Sender<v1::SandboxEvent>,
    ) -> humanitl_sandbox::FirstOutput {
        let tx = tx.clone();
        let handle = self.running_handle();
        let read = tokio::task::spawn_blocking(move || {
            let mut first = humanitl_sandbox::FirstOutput::default();
            let mut stdout = humanitl_core::TerminalFilter::new();
            let mut stderr = humanitl_core::TerminalFilter::new();
            let mut forward = true;
            while let Ok(chunk) = rx.recv() {
                first.observe(&chunk.bytes);
                // Am Pseudoterminal gibt es einen Strom; die Unterscheidung
                // bleibt für einen Aufrufer, der Pipes benutzt.
                hub.feed(&chunk.bytes);
                if !forward {
                    continue;
                }
                let (stream, filtered) = match chunk.stream {
                    humanitl_sandbox::OutputStream::Stdout => {
                        (v1::OutputStream::Stdout, stdout.push(&chunk.bytes))
                    }
                    humanitl_sandbox::OutputStream::Stderr => {
                        (v1::OutputStream::Stderr, stderr.push(&chunk.bytes))
                    }
                };
                if filtered.is_empty() {
                    continue;
                }
                if tx.blocking_send(output_event(stream, filtered)).is_err() {
                    forward = false;
                }
            }
            // Was am Ende noch zurückgehalten wird: eine angefangene Folge,
            // die nie geschlossen wurde, gehört dem Menschen und nicht dem
            // Filter — außer sie ist eine der gesperrten.
            for (stream, mut filter) in [
                (v1::OutputStream::Stdout, stdout),
                (v1::OutputStream::Stderr, stderr),
            ] {
                let rest = filter.flush();
                if !rest.is_empty() && forward {
                    let _ = tx.blocking_send(output_event(stream, rest));
                }
            }
            // Das Ende der Sitzung gehört auch denen, die am Terminal hängen.
            // Es kommt aus derselben Quelle wie `SandboxEvent.exit`, damit
            // beide Wege dieselbe Zahl nennen.
            if let Some(handle) = handle {
                let code = match handle.wait() {
                    Ok(status) => exit_code_of(status),
                    Err(diagnostic) => {
                        hub.diagnostic(&diagnostic);
                        handle.try_wait().map_or(EXIT_UNKNOWN, exit_code_of)
                    }
                };
                hub.finish(code);
            }
            first
        })
        .await;
        // Ein Faden, der nicht zurückkam, hat nichts gesehen, das sich belegen
        // ließe. Gezählt wird das als geschrieben: Ohne Beleg, dass der Agent
        // schwieg, steht kein Befund, der das behauptet.
        read.unwrap_or_else(|_| humanitl_sandbox::FirstOutput::unknown())
    }

    /// Meldet den Exit-Code des Agenten, sobald er beendet ist.
    ///
    /// Ein Signal wird nach POSIX-Sitte auf `128 + n` abgebildet; so liest es
    /// jede Shell, und so gibt `humanitl run` es weiter.
    ///
    /// **Ein Agent, der nie gelaufen ist, wird gesagt** (HUM-137). Meldet der
    /// Shim auf dem Berichtskanal ein gescheitertes `exec`, oder endet der
    /// Agent mit `127` oder `126`, ohne dass [`Inner::stream_output`] ein
    /// einziges Byte gesehen hat, geht vor dem Exit-Code `AGENT_005` hinaus,
    /// denselben Weg wie jeder andere Befund des Starts. Der Zustand bleibt,
    /// was er ist: Die Sandbox steht, und ihre Garantien gelten.
    async fn report_exit(
        &self,
        first: &humanitl_sandbox::FirstOutput,
        tx: &mpsc::Sender<v1::SandboxEvent>,
    ) {
        let Some((handle, agent)) = self.running_agent() else {
            return;
        };
        let report = Arc::clone(&handle);
        let waited = tokio::task::spawn_blocking(move || handle.wait()).await;
        let code = match waited {
            Ok(Ok(status)) => exit_code_of(status),
            Ok(Err(diagnostic)) => {
                let _ = tx.send(diagnostic_event(&diagnostic)).await;
                return;
            }
            Err(error) => {
                let diagnostic = joined_failed(&error);
                let _ = tx.send(diagnostic_event(&diagnostic)).await;
                return;
            }
        };
        // Der ganze Bericht, bis die Pipe zu ist: Die Zeile `EXEC fail` kommt
        // nach den Prüfungen, und nur sie belegt, dass das `exec` scheiterte.
        let exec = tokio::task::spawn_blocking(move || {
            report
                .report_after_exit(humanitl_sandbox::STATUS_DRAIN)
                .exec_failed
        })
        .await
        .unwrap_or_default();
        if let Some(diagnostic) =
            humanitl_sandbox::did_not_start(code, exec, first, &agent.command, &agent.path)
        {
            tracing::warn!(
                session = %self.session,
                code,
                command = %agent.command,
                "the agent ended before it wrote anything"
            );
            let _ = tx.send(diagnostic_event(&diagnostic)).await;
        }
        let _ = tx
            .send(v1::SandboxEvent {
                event: Some(v1::sandbox_event::Event::Exit(v1::sandbox_event::Exit {
                    code,
                })),
            })
            .await;
    }

    /// Misst die drei Garantien der gerade gestarteten Sandbox, sendet sie und
    /// sagt, ob der Start weitergehen darf.
    ///
    /// Die drei Ereignisse stehen zwischen `Status(starting)` und
    /// `Status(running)`: Wer `running` sieht, hat die drei Ergebnisse schon
    /// gesehen. Ist eines rot oder fehlt der Bericht, endet der Start hier —
    /// die Sandbox wird beendet, der Befund reist als eigenes Ereignis, und
    /// der Zustand ist `failed`. Ein „trotzdem starten" gibt es nicht
    /// (BACKLOG.md 4.1, `docs/SECURITY.md` Abschnitt 1); dieselbe Regel wie
    /// `enforce_isolation` auf der Kommandozeile. Zum Fenster zwischen `exec`
    /// und `SIGKILL` siehe die Modulbeschreibung.
    ///
    /// Antwortet `false`, wenn der Aufrufer nichts mehr senden soll.
    async fn check_isolation_or_kill(
        self: &Arc<Self>,
        plan: &v1::sandbox_request::Plan,
        tx: &mpsc::Sender<v1::SandboxEvent>,
    ) -> bool {
        let this = Arc::clone(self);
        let measured = tokio::task::spawn_blocking(move || this.measure_isolation()).await;
        let results = match measured {
            // Zwischen Start und Messung hat jemand gestoppt. Der Stopp hat
            // seinen eigenen Zustand gesendet; daraus einen blockierenden
            // Befund zu machen hieße, dem Menschen einen Fehler zu melden,
            // den er selbst ausgelöst hat.
            Ok(None) => return false,
            Ok(Some(results)) => results,
            // Der Faden selbst kam nicht durch. Gemessen ist damit nichts, und
            // nichts gemessen heißt hier nicht bestanden.
            Err(error) => {
                self.stop_after_failed_check(plan, &joined_failed(&error), tx)
                    .await;
                return false;
            }
        };
        for result in &results {
            if tx.send(check_event(result)).await.is_err() {
                return false;
            }
        }
        let Some(diagnostic) = first_failure(&results) else {
            return true;
        };
        self.stop_after_failed_check(plan, &diagnostic, tx).await;
        false
    }

    /// Beendet die Sandbox nach einer roten Prüfung und meldet den Befund.
    async fn stop_after_failed_check(
        self: &Arc<Self>,
        plan: &v1::sandbox_request::Plan,
        diagnostic: &Diagnostic,
        tx: &mpsc::Sender<v1::SandboxEvent>,
    ) {
        let this = Arc::clone(self);
        let owned_plan = plan.clone();
        let owned_diagnostic = diagnostic.clone();
        let closed =
            tokio::task::spawn_blocking(move || this.kill_and_fail(&owned_plan, &owned_diagnostic))
                .await;
        // Erst der Befund, dann der Zustand: Der Client trägt die Befunde des
        // laufenden Vorgangs am Zustand mit.
        let _ = tx.send(diagnostic_event(diagnostic)).await;
        match closed {
            Ok((stopped, status)) => {
                if let Some(line) = stopped {
                    let _ = tx.send(log_event(line)).await;
                }
                let _ = tx.send(status_event(status)).await;
            }
            Err(_) => {
                let _ = tx
                    .send(status_event(self.failed_status(plan, diagnostic)))
                    .await;
            }
        }
    }

    /// Beendet die Sandbox und liefert ihre Protokollzeile und den Zustand
    /// `failed` dazu.
    ///
    /// Der Zustand behält Einhängungen, Umgebung und Kommandozeile: Sie kommen
    /// aus dem Profil und nicht aus dem toten Prozess, und genau sie braucht,
    /// wer verstehen will, warum eine Garantie nicht galt. Erst wenn sich das
    /// Profil nicht mehr lesen lässt, bleibt der karge Zustand aus
    /// [`Inner::failed_status`].
    fn kill_and_fail(
        &self,
        plan: &v1::sandbox_request::Plan,
        diagnostic: &Diagnostic,
    ) -> (Option<String>, v1::sandbox_event::Status) {
        if let Some(handle) = self.running_handle() {
            // Ohne Gnadenfrist. `kill()` schickt `SIGTERM` und wartet
            // [`KILL_GRACE`] auf ein geordnetes Ende — fünf Sekunden, in denen
            // der Agent weiterläuft, die Brücke steht und der Proxy annimmt.
            // Eine Sandbox, deren Isolation nicht belegt ist, hat nichts
            // aufzuräumen, das diese Zeit wert wäre; `terminate(ZERO)`
            // eskaliert sofort auf `SIGKILL`.
            // Und nur einmal warten: `terminate` kehrt zurück, wenn der
            // Prozess weg ist oder [`KILL_GRACE`] nach dem `SIGKILL`
            // abgelaufen ist. Ein zweites, unbegrenztes `wait()` verdoppelte
            // die obere Schranke des Fensters und nähme dem Client bei einem
            // Prozess im D-Zustand auch noch den Befund.
            handle.terminate(Duration::ZERO);
        }
        let stopped = self.stopped_line();
        self.clear_running();
        let status = self
            .snapshot_with(plan, Some(v1::SandboxState::Failed))
            .unwrap_or_else(|_| self.failed_status(plan, diagnostic));
        (stopped, status)
    }

    /// `Op::IsolationCheck`: die drei Garantien der laufenden Sandbox.
    ///
    /// Gemessen wird in der Sandbox, die läuft, und nicht auf dem Wirt: Die
    /// Prüfzeilen schreibt der Shim von innen, `BwrapBackend::isolation_check`
    /// faltet sie zu den drei Garantien.
    ///
    /// Läuft keine Sandbox, sendet dieser Zweig **kein** Ergebnis, sondern den
    /// Zustand. Drei graue Ergebnisse zu schicken hieße, „nicht gemessen" als
    /// Messung auszugeben, und die Oberfläche sähe den Unterschied zu einem
    /// bestandenen Durchlauf nicht mehr (CONVENTIONS 4.13, „Nie mehr behaupten
    /// als bewiesen ist").
    fn isolation_check(&self, tx: &mpsc::Sender<v1::SandboxEvent>) {
        let Some((backend, handle)) = self.running_parts() else {
            self.snapshot_or_diagnostic(&v1::sandbox_request::Plan::default(), tx);
            return;
        };
        for result in &backend.isolation_check(&handle) {
            if tx.blocking_send(check_event(result)).is_err() {
                return;
            }
        }
    }

    /// Die drei Garantien der laufenden Sandbox, oder `None`, wenn keine mehr
    /// gehalten wird.
    ///
    /// `None` und nicht die leere Liste: Die leere Liste heißt „gemessen und
    /// nichts bekommen" und ist [`first_failure`] ein Grund, den Start zu
    /// beenden. Dass zwischen Start und Messung jemand gestoppt hat, ist
    /// etwas anderes und darf nicht als Fehlschlag der Sandbox erscheinen.
    fn measure_isolation(&self) -> Option<Vec<CheckResult>> {
        self.running_parts()
            .map(|(backend, handle)| backend.isolation_check(&handle))
    }

    /// Beendet die laufende Sandbox und meldet `stopping`, dann `stopped`.
    ///
    /// **Das Beenden hängt an nichts, was ein Client tut.** Bis HUM-142 kehrte
    /// diese Funktion zurück, sobald der Ereignisstrom weg war — und genau das
    /// ist der Alltagsfall: `humanitl run` schickt an seiner Zeitschranke sein
    /// `Sandbox(Stop)` und endet, der Empfänger fällt weg, `tx.send`
    /// scheitert, und die Sandbox lief weiter. Wer den Strom fallen lässt,
    /// bekommt seither keine Ereignisse mehr, aber der Abschied läuft zu Ende.
    async fn stop(self: Arc<Self>, tx: mpsc::Sender<v1::SandboxEvent>) {
        let plan = v1::sandbox_request::Plan::default();
        let handle = self.running_handle();
        let Some(handle) = handle else {
            let this = Arc::clone(&self);
            let tx = tx.clone();
            let _ =
                tokio::task::spawn_blocking(move || this.snapshot_or_diagnostic(&plan, &tx)).await;
            return;
        };

        {
            let this = Arc::clone(&self);
            let plan = plan.clone();
            if let Ok(Ok(status)) = tokio::task::spawn_blocking(move || {
                this.snapshot_with(&plan, Some(v1::SandboxState::Stopping))
            })
            .await
            {
                let _ = tx.send(status_event(status)).await;
            }
        }

        let ended = tokio::task::spawn_blocking(move || farewell(&handle)).await;
        let stopped = self.stopped_line();
        self.clear_running();
        match &ended {
            Ok(Some(diagnostic)) => {
                let _ = tx.send(diagnostic_event(diagnostic)).await;
            }
            // Ein Faden, der nicht zurückkam, ist derselbe Befund wie überall
            // sonst in dieser Datei; verschwiegen wäre er ein Stopp, der ohne
            // Grund nichts gesagt hat.
            Err(error) => {
                let _ = tx.send(diagnostic_event(&joined_failed(error))).await;
            }
            Ok(None) => {}
        }
        if let Some(line) = stopped {
            let _ = tx.send(log_event(line)).await;
        }

        let this = Arc::clone(&self);
        if let Ok(Ok(status)) = tokio::task::spawn_blocking(move || this.snapshot(&plan)).await {
            let _ = tx.send(status_event(status)).await;
        }
    }

    /// Beendet die laufende Sandbox dieser Sitzung, ohne Ereignisstrom.
    ///
    /// Der Weg des Daemons, der selbst endet; siehe
    /// [`SandboxService::shutdown`].
    fn shutdown(&self) -> Option<Diagnostic> {
        // Zuerst der Riegel, dann das Beenden: Ein `Start`, der nach dieser
        // Zeile anklopft, bekommt keinen Anspruch mehr und läuft deshalb nicht
        // in einen Abschied hinein, der ihn nicht mehr sieht. Im Daemon steht
        // er längst ([`SandboxService::begin_leaving`] am Signal); dieser
        // zweite Griff kostet nichts und deckt jeden anderen Aufrufer.
        self.leaving
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let ended = self.running_handle().and_then(|handle| {
            let ended = farewell(&handle);
            self.clear_running();
            ended
        });
        // Die obere Schicht liegt im Arbeitsspeicher und gehört zu diesem
        // Daemon; mit ihm geht sie (HUM-350). Die Aufzeichnung sagt es.
        if let Some(waiting) = lock(&self.disposable).take() {
            #[cfg(target_os = "linux")]
            self.drop_layer(waiting, "discarded");
            #[cfg(not(target_os = "linux"))]
            match waiting {}
        }
        ended
    }

    /// Baut die Zusammenfassung dieses Laufs, legt sie ab und schickt sie.
    ///
    /// Sie entsteht **nach** dem Ende des Prozesses: [`Inner::attend`] kehrt
    /// erst zurück, wenn der Strom der Ausgabe zu Ende ist und `wait` den
    /// Status hat, und `kill_and_fail` wartet auf das Einsammeln. Ein zweiter
    /// Schnappschuss über einen Baum, in den noch geschrieben wird, wäre eine
    /// Aussage über einen Zeitpunkt, der schon vorbei ist.
    ///
    /// Die Befunde reisen zweimal, aus einer Quelle: als eigene
    /// `Diagnostic`-Ereignisse, damit jeder Client sie sieht, der Befunde
    /// ohnehin liest (`humanitl run`, der Sandbox-Bildschirm), und in der
    /// Nachricht selbst, weil `GetSessionSummary` keinen Ereignisstrom hat und
    /// dieselbe Auskunft geben muss. Gerechnet werden sie an einer Stelle,
    /// [`SessionSummary::diagnostics`].
    async fn summarize(
        self: &Arc<Self>,
        watch: Option<SummaryWatch>,
        sandbox: SandboxId,
        tx: &mpsc::Sender<v1::SandboxEvent>,
    ) {
        let Some(watch) = watch else {
            return;
        };
        let settings = match crate::summary::findings_settings(&self.config()) {
            Ok(settings) => settings,
            Err(diagnostic) => {
                let _ = tx.send(diagnostic_event(&diagnostic)).await;
                return;
            }
        };
        let session = self.session;
        // Der zweite Schnappschuss läuft über den Baum und liest Dateien; das
        // gehört auf einen eigenen Faden.
        let built =
            tokio::task::spawn_blocking(move || watch.finish(session, sandbox, &settings)).await;
        let summary = match built {
            Ok(Ok(summary)) => summary,
            Ok(Err(diagnostic)) => {
                let _ = tx.send(diagnostic_event(&diagnostic)).await;
                return;
            }
            Err(error) => {
                let _ = tx.send(diagnostic_event(&joined_failed(&error))).await;
                return;
            }
        };
        // Erst ablegen, dann senden: Ein Client, der das Ereignis sieht und
        // sofort `GetSessionSummary` ruft, soll die Zeile schon finden.
        self.store_summary(&summary, tx).await;
        for diagnostic in &summary.diagnostics() {
            if tx.send(diagnostic_event(diagnostic)).await.is_err() {
                return;
            }
        }
        let _ = tx
            .send(v1::SandboxEvent {
                event: Some(v1::sandbox_event::Event::Summary(
                    crate::convert::session_summary_to_proto(&summary, SystemTime::now()),
                )),
            })
            .await;
    }

    /// Legt die Zusammenfassung in der Aufzeichnung ab, wenn es eine gibt.
    ///
    /// Ohne Aufzeichnung geschieht nichts: Der Lauf ist trotzdem
    /// zusammengefasst, nur nicht nachschlagbar. Ein Fehler beim Serialisieren
    /// ist ein Befund und kein stiller Verlust — sonst hielte ein Mensch ein
    /// leeres `humanitl sessions summary` für einen Lauf ohne Änderungen.
    async fn store_summary(&self, summary: &SessionSummary, tx: &mpsc::Sender<v1::SandboxEvent>) {
        let Some(recorder) = self.recorder.as_ref() else {
            return;
        };
        match serde_json::to_string(summary) {
            Ok(json) => recorder.store_session_summary(self.session, summary.sandbox, &json),
            Err(error) => {
                let diagnostic = Diagnostic::builder(codes::RECORDER_003, Severity::Warning)
                    .why(format!(
                        "the summary of sandbox {} could not be written to the recording \
                         ({error}); it is in this event stream and nowhere else",
                        summary.sandbox
                    ))
                    .build();
                let _ = tx.send(diagnostic_event(&diagnostic)).await;
            }
        }
    }

    /// Startet die geplante Sandbox und merkt sie sich.
    ///
    /// Die Sitzung läuft an einem Pseudoterminal (HUM-042): Der Agent bekommt
    /// ein Terminal, und der Daemon hält dessen andere Seite. Das ist die
    /// Voraussetzung für alles, was `Terminal` kann — ohne Terminal gibt es
    /// keine Eingabe, keine Geometrie und für ein Vollbild-TUI keinen Grund
    /// zu starten.
    ///
    /// `output` bekommt jedes Stück Ausgabe, sobald es gelesen ist. Es ist
    /// **roh**: Wer es weitergibt, filtert es für seinen Weg selbst. Der
    /// Ereignisstrom der Sandbox nimmt
    /// [`humanitl_core::TerminalPolicy::ColourOnly`]
    /// ([`Inner::stream_output`]), das Terminal
    /// [`humanitl_core::TerminalPolicy::FullScreen`] ([`TerminalHub`]). Zwei
    /// Wege, zwei Zusagen, ein Strom.
    ///
    /// Blockierend, und mit dem Schnappschuss des Projektverzeichnisses darin:
    /// Der Aufrufer ruft es auf einem eigenen Faden.
    fn launch(
        &self,
        plan: &v1::sandbox_request::Plan,
        command: &[OsString],
        mode: Option<SandboxMode>,
        output: Option<humanitl_sandbox::OutputSink>,
    ) -> Result<Launched, Diagnostic> {
        let prepared = self.prepare_keyed(plan, command, KeyNeed::Start)?;
        // Der Wunsch des Starts, sonst `sandbox.mode` aus derselben Aufnahme
        // der Konfiguration, die den Plan gebaut hat (HUM-357).
        let wanted = mode.unwrap_or(prepared.wanted_mode);
        // Warten Änderungen eines früheren Laufs, entscheidet erst der Mensch
        // über sie, und zwar vor der Auflösung: Auch eine Vorgabe, die
        // zurückfiele, ließe den Agenten sonst direkt in einen Ordner
        // schreiben, über dem Änderungen liegen, die niemand gesehen hat. Nur
        // ein ausdrückliches `direct` geht.
        self.pending_guard(wanted)?;
        // Aufgelöst gegen das, was das System kann. Ein ausdrückliches
        // `disposable`, das nicht geht, endet hier mit `SANDBOX_030`; die
        // Vorgabe `auto` fällt auf `direct` zurück und sagt es mit
        // `SANDBOX_033`.
        let mut choice = self.mode_choice(wanted, &prepared)?;
        if choice.is_disposable() {
            match self.launch_disposable(&prepared, plan, output.clone()) {
                Ok(launched) => return Ok(launched),
                Err(failure) => match crate::disposable::fallback_after(wanted, failure) {
                    Ok(notice) => {
                        choice = humanitl_sandbox::ModeChoice {
                            mode: SandboxMode::Direct,
                            notice: Some(notice),
                        }
                    }
                    Err(diagnostic) => return Err(diagnostic),
                },
            }
        }
        let mut launched = self
            .launch_with(&prepared, plan, None, None, output)
            .map(|(run, _)| run)?;
        if let Some(notice) = choice.notice {
            tracing::info!(why = %notice.why, "sandbox.mode auto runs direct");
            launched.warnings.push(notice);
        }
        Ok(launched)
    }

    /// Wartende Änderungen sperren jeden Start außer einem ausdrücklichen
    /// `direct` (HUM-357); siehe [`crate::disposable::pending_guard`].
    #[cfg(target_os = "linux")]
    fn pending_guard(&self, wanted: SandboxMode) -> Result<(), Diagnostic> {
        let pending = lock(&self.disposable)
            .as_ref()
            .map(|waiting| waiting.sandbox);
        crate::disposable::pending_guard(wanted, pending)
    }

    /// Ohne Overlay wartet nie eine Schicht.
    #[cfg(not(target_os = "linux"))]
    #[allow(clippy::unused_self, clippy::unnecessary_wraps)]
    fn pending_guard(&self, _wanted: SandboxMode) -> Result<(), Diagnostic> {
        Ok(())
    }

    /// Löst `wanted` für diesen Plan auf (HUM-357); siehe
    /// [`humanitl_sandbox::resolve_mode`].
    fn mode_choice(
        &self,
        wanted: SandboxMode,
        prepared: &Prepared,
    ) -> Result<humanitl_sandbox::ModeChoice, Diagnostic> {
        humanitl_sandbox::resolve_mode(wanted, || self.disposable_support(prepared))
    }

    /// Ob dieser Plan eine Wegwerf-Sandbox bekommen kann: ein Sandbox-Backend
    /// mit Overlay, das Projekt schreibbar, das Laufzeitverzeichnis auf tmpfs
    /// (HUM-357).
    #[cfg(target_os = "linux")]
    fn disposable_support(&self, prepared: &Prepared) -> Result<(), Diagnostic> {
        prepared.backend.disposable_support(
            &prepared.profile,
            &prepared.session,
            &self.paths.runtime_dir().path,
        )
    }

    /// Ohne Overlay gibt es keine Wegwerf-Sandbox (HUM-350).
    #[cfg(not(target_os = "linux"))]
    #[allow(clippy::unused_self)]
    fn disposable_support(&self, _prepared: &Prepared) -> Result<(), Diagnostic> {
        Err(humanitl_sandbox::disposable::unsupported())
    }

    /// Der Start einer Wegwerf-Sandbox: die obere Schicht vor dem Plan, damit
    /// der Plan sie nennt, und jeder Weg hinaus vor dem Handle räumt sie
    /// wieder weg (HUM-350).
    ///
    /// Ein Fehler trägt, ob die Sandbox schon lief
    /// ([`crate::disposable::SetupFailure`]): Nur davor darf die Vorgabe
    /// `auto` noch auf `direct` zurückfallen (HUM-357).
    #[cfg(target_os = "linux")]
    fn launch_disposable(
        &self,
        prepared: &Prepared,
        plan: &v1::sandbox_request::Plan,
        output: Option<humanitl_sandbox::OutputSink>,
    ) -> Result<Launched, crate::disposable::SetupFailure> {
        use crate::disposable::SetupFailure::{After, Before};
        // Der Projektordner, einmal geöffnet (C1): Grundlage, Diff und
        // Übernahme arbeiten an diesem Deskriptor, nie wieder am Pfad, und
        // das Backend muss genau diesen Ort einhängen.
        let lower =
            humanitl_sandbox::worktree::open_root(&prepared.session.work_src).map_err(Before)?;
        let resolved = pinned_path(&lower).map_err(Before)?;
        if let Some(approved) = prepared.backend.approved_work_dir()
            && approved != resolved.as_path()
        {
            return Err(Before(moved_work_dir(approved, &resolved)));
        }
        // Die Grundlage vor dem Start (C3, O5): Was der Mensch ab jetzt
        // ändert, ist ein Konflikt, auch in den ersten Augenblicken des Laufs.
        let baseline = humanitl_sandbox::disposable::baseline_at(std::os::fd::AsFd::as_fd(&lower));
        let layer =
            humanitl_sandbox::Layer::create(&self.paths.runtime_dir().path).map_err(Before)?;
        let meta = lower
            .try_clone()
            .map(std::fs::File::from)
            .and_then(|file| file.metadata())
            .map_err(|err| {
                Before(
                    Diagnostic::builder(codes::SANDBOX_030, Severity::Blocking)
                        .why(format!(
                            "the opened project folder cannot be inspected ({err})"
                        ))
                        .build(),
                )
            })?;
        let pin = PinnedWork {
            path: &resolved,
            dev: std::os::unix::fs::MetadataExt::dev(&meta),
            ino: std::os::unix::fs::MetadataExt::ino(&meta),
        };
        match self.launch_with(prepared, plan, Some(layer.dirs()), Some(pin), output) {
            Ok((mut run, handle)) => {
                // Ab hier läuft die Sandbox: Was jetzt scheitert, fällt nie
                // mehr auf `direct` zurück.
                let pinned = Pinned { lower, baseline };
                run.disposable = Some(
                    self.watch_layer(layer, handle, prepared, pinned)
                        .map_err(After)?,
                );
                Ok(run)
            }
            Err(diagnostic) => {
                if let Err((layer, failed)) = layer.discard() {
                    tracing::warn!(
                        layer = %layer.dir().display(),
                        why = %failed.why,
                        "the upper layer of a failed disposable start stays until the next daemon start"
                    );
                }
                // `SANDBOX_030` kommt aus `launch_with` nur aus der Prüfung
                // des Plans, vor dem Start von bubblewrap; jeder andere
                // Befund zählt, als liefe die Sandbox schon.
                if diagnostic.code == codes::SANDBOX_030 {
                    Err(Before(diagnostic))
                } else {
                    Err(After(diagnostic))
                }
            }
        }
    }

    /// Ohne Overlay gibt es keine Wegwerf-Sandbox (HUM-350).
    #[cfg(not(target_os = "linux"))]
    #[allow(clippy::unused_self)]
    fn launch_disposable(
        &self,
        _prepared: &Prepared,
        _plan: &v1::sandbox_request::Plan,
        _output: Option<humanitl_sandbox::OutputSink>,
    ) -> Result<Launched, crate::disposable::SetupFailure> {
        Err(crate::disposable::SetupFailure::Before(
            humanitl_sandbox::disposable::unsupported(),
        ))
    }

    /// Die Grundlage und die Grenze einer gestarteten Wegwerf-Sandbox.
    ///
    /// Die Grundlage entsteht hier, nach dem Start: Der Agent schreibt nie in
    /// das Projekt, sie hält nur fest, was der **Mensch** währenddessen
    /// ändert. Lässt sich das Projekt nicht lesen oder die Schicht nicht
    /// messen, endet die Sandbox, denn ohne Grundlage gäbe es keine sichere
    /// Übernahme und ohne Messung keine Grenze.
    #[cfg(target_os = "linux")]
    fn watch_layer(
        &self,
        layer: humanitl_sandbox::Layer,
        handle: Arc<SandboxHandle>,
        prepared: &Prepared,
        pinned: Pinned,
    ) -> Result<crate::disposable::Waiting, Diagnostic> {
        let stop = Arc::clone(&handle);
        // Gemessen gegen den Stand beim Anlegen der Schicht (C4, O4).
        let guarded = humanitl_sandbox::SizeGuard::start(&layer, move || {
            let _ = stop.kill();
        });
        let guard = match guarded {
            Ok(guard) => guard,
            Err(diagnostic) => {
                let _ = handle.kill();
                self.clear_running();
                if let Err((layer, failed)) = layer.discard() {
                    tracing::warn!(
                        layer = %layer.dir().display(),
                        why = %failed.why,
                        "the upper layer of a failed disposable start stays until the next daemon start"
                    );
                }
                return Err(diagnostic);
            }
        };
        // Die Startzeit des ersten Prozesses (O11): Eine wiederverwendete
        // Nummer ist später nicht mehr diese Sandbox.
        let child_start = handle.child_pid().and_then(|pid| {
            humanitl_sandbox::disposable::start_ticks(std::path::Path::new("/proc"), pid)
        });
        Ok(crate::disposable::Waiting {
            sandbox: handle.id,
            layer,
            work_dir: prepared.session.work_src.clone(),
            lower: pinned.lower,
            baseline: pinned.baseline,
            handle,
            child_start,
            guard: Some(guard),
        })
    }

    /// Der Start selbst, mit der Schicht, wenn es eine gibt.
    fn launch_with(
        &self,
        prepared: &Prepared,
        plan: &v1::sandbox_request::Plan,
        overlay: Option<humanitl_sandbox::OverlayDirs>,
        pinned: Option<PinnedWork<'_>>,
        output: Option<humanitl_sandbox::OutputSink>,
    ) -> Result<(Launched, Arc<SandboxHandle>), Diagnostic> {
        let backend = prepared.backend.clone().with_stdio(StdioMode::Pty {
            cols: DEFAULT_TERMINAL_SIZE.0,
            rows: DEFAULT_TERMINAL_SIZE.1,
        });
        let backend = match &pinned {
            Some(pin) => backend.with_approved_work_dir(pin.path),
            None => backend,
        };
        let disposable = overlay.is_some();
        // Das Backend hängt nur den Ordner ein, den der Daemon schon offen
        // hält (Review Runde 2): gleicher Ort und gleicher Inode.
        #[cfg(target_os = "linux")]
        let backend = match (overlay, &pinned) {
            (Some(dirs), Some(pin)) => backend
                .with_overlay(dirs)
                .with_expected_work_inode(pin.dev, pin.ino),
            (Some(dirs), None) => backend.with_overlay(dirs),
            (None, _) => backend,
        };
        #[cfg(not(target_os = "linux"))]
        if let Some(dirs) = overlay {
            drop(dirs);
            return Err(humanitl_sandbox::disposable::unsupported());
        }
        // Der Zuhörer hängt nur an dem Backend, das startet, und nicht an dem,
        // das gehalten wird: Ein gehaltener Sender schlösse seinen Kanal nie,
        // und wer auf dessen Ende wartet, wartete für immer. Beide tragen
        // dieselbe Frist für den Bericht des Shims, gemessen wird also unter
        // denselben Bedingungen wie gestartet.
        let mut launcher = backend.clone();
        if let Some(sink) = output {
            launcher = launcher.with_output_sink(sink);
        }
        let launch = launcher.plan(&prepared.profile, &prepared.session)?;
        let mut warnings = prepared.warnings.clone();
        warnings.extend(launch.warnings.iter().cloned());
        // **Der Schnappschuss steht zwischen Planen und Starten.** Vorher gibt
        // es `LaunchPlan::unprotected` noch nicht, und nachher wäre er falsch:
        // Was der Agent in der Zwischenzeit schreibt, stünde als „war schon
        // da" im Baum und fehlte am Ende im Unterschied. Er kostet einen Lauf
        // über das Projekt und verzögert den Start um genau diesen Lauf; die
        // Budgets aus `SnapshotLimits` decken ihn nach oben (HUM-043).
        // Eine Wegwerf-Sandbox schreibt nicht in das Projekt; was sie
        // hinterlässt, zeigt die obere Schicht (HUM-350).
        let watched = if disposable {
            Ok(None)
        } else {
            crate::summary::SummaryWatch::start(
                &prepared.session.work_src,
                prepared.session.work_mode,
                &launch.unprotected,
            )
        };
        let watch = match watched {
            Ok(watch) => watch,
            // Ohne ersten Schnappschuss gibt es später keinen Vergleich. Der
            // Start geht trotzdem weiter: Ein Projektverzeichnis, das sich
            // nicht lesen lässt, ist ein Grund für einen Befund und keiner,
            // dem Menschen die Sitzung zu verweigern.
            Err(diagnostic) => {
                warnings.push(diagnostic);
                None
            }
        };
        if let Some(watch) = watch.as_ref() {
            tracing::info!(
                session = %self.session,
                entries = watch.entries(),
                work_dir = %prepared.session.work_src.display(),
                "project snapshot taken before the sandbox starts"
            );
        }
        let agent = AgentFacts {
            command: humanitl_sandbox::agent::opencode::shown_program(&prepared.session.command)
                .map(|command| command.to_string_lossy().into_owned())
                .unwrap_or_default(),
            path: sandbox_path_of(prepared),
        };
        // Die Quelle, solange `running` noch leer ist: danach nennte
        // [`Inner::work_dir_source`] die der laufenden Sitzung.
        let work_dir_source = self.work_dir_source(&self.config(), &self.pending_choice(), plan);
        let handle = launcher.launch(&launch)?;
        drop(launcher);
        let sandbox = handle.id;
        let handle = Arc::new(handle);
        let started = Arc::clone(&handle);
        let terminal = TerminalHub::new(
            Arc::clone(&handle),
            DEFAULT_TERMINAL_SIZE.0,
            DEFAULT_TERMINAL_SIZE.1,
            self.config().ui.terminal_notices,
        );
        let mut running = lock(&self.running);
        self.bump_generation();
        *running = Some(Running {
            handle,
            backend,
            started_at: SystemTime::now(),
            profile: prepared.profile.name.clone(),
            work_dir: prepared.session.work_src.clone(),
            work_dir_source,
            work_mode: prepared.session.work_mode,
            terminal,
            shells: ShellRegistry::for_sandbox(sandbox),
            agent,
        });
        drop(running);
        // Ab hier nennt die laufende Sandbox den Ordner; der des Starts gilt
        // für nichts mehr (HUM-293). `pending` erst nach `running`
        // losgelassen: Die Reihenfolge der Schlösser ist sonst überall
        // `pending` vor `running`.
        lock(&self.pending).starting_work_dir = None;
        Ok((
            Launched {
                warnings,
                watch,
                sandbox,
                disposable: None,
            },
            started,
        ))
    }

    /// Die Schicht, die die Zeile einer Wegwerf-Sandbox zeigt (HUM-350): an
    /// der Stelle der Kennung, die erst der Start vergibt, steht `LAYER`.
    fn overlay_preview(&self, disposable: bool) -> Option<humanitl_sandbox::OverlayDirs> {
        // Ohne Overlay zeigt die Zeile keines; der Start lehnt ab (HUM-350).
        let possible = cfg!(target_os = "linux");
        (possible && disposable).then(|| {
            let layer = self
                .paths
                .runtime_dir()
                .path
                .join(humanitl_sandbox::disposable::DISPOSABLE_DIR)
                .join("LAYER");
            humanitl_sandbox::OverlayDirs {
                upper: layer.join("upper"),
                work: layer.join("work"),
            }
        })
    }

    /// `Op::Changes`: die wartenden Änderungen einer Wegwerf-Sandbox
    /// (HUM-350).
    #[cfg(target_os = "linux")]
    fn disposable_changes(&self, tx: &mpsc::Sender<v1::SandboxEvent>) {
        let slot = lock(&self.disposable);
        let event = match slot.as_ref().map(crate::disposable::Waiting::changes) {
            None => diagnostic_event(&crate::disposable::nothing_pending()),
            Some(Ok(changes)) => v1::SandboxEvent {
                event: Some(v1::sandbox_event::Event::Changes(changes)),
            },
            Some(Err(diagnostic)) => diagnostic_event(&diagnostic),
        };
        drop(slot);
        let _ = tx.blocking_send(event);
    }

    /// `Op::ApplyChanges`: schreibt die gewählten Änderungen und verwirft den
    /// Rest (HUM-350). Scheitert eine Prüfung vor dem ersten Schreiben, bleibt
    /// alles, wie es war.
    #[cfg(target_os = "linux")]
    fn apply_changes(
        &self,
        request: &v1::sandbox_request::ApplyChanges,
        tx: &mpsc::Sender<v1::SandboxEvent>,
    ) {
        let mut slot = lock(&self.disposable);
        let Some(waiting) = slot.as_ref() else {
            drop(slot);
            let _ = tx.blocking_send(diagnostic_event(&crate::disposable::nothing_pending()));
            return;
        };
        let applied = match waiting.apply(request) {
            Ok(applied) => applied,
            Err(diagnostic) => {
                drop(slot);
                let _ = tx.blocking_send(diagnostic_event(&diagnostic));
                return;
            }
        };
        let mut events = Vec::new();
        if let Some(failed) = &applied.failed {
            events.push(diagnostic_event(failed));
        }
        if let Some(record) = applied.record
            && let Some(waiting) = slot.take()
        {
            tracing::info!(
                session = %self.session,
                sandbox = %waiting.sandbox,
                applied = record.applied.len(),
                discarded = record.discarded.len(),
                "the changes of a disposable sandbox were applied"
            );
            if let Some(diagnostic) = self.finish_layer(waiting, record) {
                events.push(diagnostic_event(&diagnostic));
            }
        }
        drop(slot);
        events.push(v1::SandboxEvent {
            event: Some(v1::sandbox_event::Event::ChangesOutcome(applied.outcome)),
        });
        for event in events {
            let _ = tx.blocking_send(event);
        }
    }

    /// `Op::DiscardChanges`: verwirft alle wartenden Änderungen (HUM-350).
    #[cfg(target_os = "linux")]
    fn discard_changes(
        &self,
        request: &v1::sandbox_request::DiscardChanges,
        tx: &mpsc::Sender<v1::SandboxEvent>,
    ) {
        let mut slot = lock(&self.disposable);
        let checked = match slot.as_ref() {
            None => Err(crate::disposable::nothing_pending()),
            Some(waiting) => {
                crate::disposable::check_id(waiting, &request.sandbox_id).and_then(|()| {
                    humanitl_sandbox::disposable::ensure_ended(&waiting.handle, waiting.child_start)
                })
            }
        };
        if let Err(diagnostic) = checked {
            drop(slot);
            let _ = tx.blocking_send(diagnostic_event(&diagnostic));
            return;
        }
        let Some(waiting) = slot.take() else {
            return;
        };
        drop(slot);
        let sandbox = waiting.sandbox;
        let discarded = waiting.discarded_paths();
        let failed = self.drop_layer(waiting, "discarded");
        if let Some(diagnostic) = &failed {
            let _ = tx.blocking_send(diagnostic_event(diagnostic));
        }
        let _ = tx.blocking_send(v1::SandboxEvent {
            event: Some(v1::sandbox_event::Event::ChangesOutcome(
                crate::disposable::outcome(sandbox, "discarded", Vec::new(), discarded),
            )),
        });
    }

    /// Verwirft eine Schicht und hält es in der Aufzeichnung fest.
    #[cfg(target_os = "linux")]
    fn drop_layer(&self, waiting: crate::disposable::Waiting, outcome: &str) -> Option<Diagnostic> {
        let record = humanitl_sandbox::DisposableRecord {
            outcome: outcome.to_owned(),
            applied: Vec::new(),
            discarded: waiting.discarded_paths(),
        };
        tracing::info!(
            session = %self.session,
            sandbox = %waiting.sandbox,
            discarded = record.discarded.len(),
            "the changes of a disposable sandbox were discarded"
        );
        self.finish_layer(waiting, record)
    }

    /// Entfernt die Schicht und legt die Entscheidung in der Aufzeichnung
    /// ab. Eine Schicht, die sich nicht entfernen lässt, ist ein Befund; sie
    /// verschwindet spätestens beim nächsten Start des Daemons.
    #[cfg(target_os = "linux")]
    fn finish_layer(
        &self,
        waiting: crate::disposable::Waiting,
        record: humanitl_sandbox::DisposableRecord,
    ) -> Option<Diagnostic> {
        if let Some(recorder) = self.recorder.as_ref() {
            let summary = waiting.summary(self.session, record);
            if let Ok(json) = serde_json::to_string(&summary) {
                recorder.store_session_summary(self.session, summary.sandbox, &json);
            }
        }
        waiting
            .layer
            .discard()
            .err()
            .map(|(_, diagnostic)| diagnostic)
    }

    /// `Op::Changes` ohne Overlay: Es gibt keine Wegwerf-Sandbox (HUM-350).
    #[cfg(not(target_os = "linux"))]
    #[allow(clippy::unused_self)]
    fn disposable_changes(&self, tx: &mpsc::Sender<v1::SandboxEvent>) {
        let _ = tx.blocking_send(diagnostic_event(
            &humanitl_sandbox::disposable::unsupported(),
        ));
    }

    /// `Op::ApplyChanges` ohne Overlay (HUM-350).
    #[cfg(not(target_os = "linux"))]
    #[allow(clippy::unused_self)]
    fn apply_changes(
        &self,
        _request: &v1::sandbox_request::ApplyChanges,
        tx: &mpsc::Sender<v1::SandboxEvent>,
    ) {
        self.disposable_changes(tx);
    }

    /// `Op::DiscardChanges` ohne Overlay (HUM-350).
    #[cfg(not(target_os = "linux"))]
    #[allow(clippy::unused_self)]
    fn discard_changes(
        &self,
        _request: &v1::sandbox_request::DiscardChanges,
        tx: &mpsc::Sender<v1::SandboxEvent>,
    ) {
        self.disposable_changes(tx);
    }

    /// Der Schnappschuss, wie ihn die Oberfläche zeigt.
    fn snapshot(
        &self,
        plan: &v1::sandbox_request::Plan,
    ) -> Result<v1::sandbox_event::Status, Diagnostic> {
        self.snapshot_with(plan, None)
    }

    /// Derselbe Schnappschuss, mit einem Zustand, den der Aufrufer kennt und
    /// der Prozessliste noch nicht ansieht (`starting`, `stopping`).
    fn snapshot_with(
        &self,
        plan: &v1::sandbox_request::Plan,
        state: Option<v1::SandboxState>,
    ) -> Result<v1::sandbox_event::Status, Diagnostic> {
        self.snapshot_with_config(&self.config(), &self.pending_choice(), plan, state)
    }

    /// [`Inner::snapshot_with`] from this one configuration: preview and
    /// endpoint come from the same value (HUM-277).
    fn snapshot_with_config(
        &self,
        config: &Arc<Config>,
        choice: &PlanChoice,
        plan: &v1::sandbox_request::Plan,
        state: Option<v1::SandboxState>,
    ) -> Result<v1::sandbox_event::Status, Diagnostic> {
        let prepared = self.prepare_keyed_with(config, choice, plan, &[], KeyNeed::Plan)?;
        let argv = prepared.argv();
        let Facts {
            id,
            started_at,
            held,
            agent_running,
        } = self.running_facts();
        // Der Zustand kommt vom gehaltenen Handle, nicht von der Lebendigkeit
        // des Kindes: Eine Sitzung, deren Agent sich beendet hat, läuft weiter
        // — sie hat eine Kennung, eine Startzeit und einen Stopp, der noch
        // etwas tut. `agent_running` sagt allein, ob darin noch jemand
        // arbeitet. Beides zu vermengen hieße, `stopped` mit einer Startzeit
        // zu melden (Review Codex, Befund 4).
        //
        // Ein Start, der seinen Anspruch hält, aber noch kein Handle abgelegt
        // hat, ist `starting` und nicht `stopped` (HUM-284): Ein zweiter
        // Client, der in diesem Fenster fragt (`postinst` vor einem Neustart,
        // die Statusleiste), sähe sonst keine Sitzung, wo eine anläuft.
        let state = state.unwrap_or_else(|| {
            if held {
                v1::SandboxState::Running
            } else if self.start_claimed() {
                v1::SandboxState::Starting
            } else {
                v1::SandboxState::Stopped
            }
        });
        Ok(v1::sandbox_event::Status {
            state: state.into(),
            sandbox_id: id.unwrap_or_default(),
            session_id: self.session.to_string(),
            backend: prepared.backend.name().to_owned(),
            llm_endpoint: config
                .llm
                .endpoint
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default(),
            work_dir: prepared.session.work_src.display().to_string(),
            work_mode: work_mode_name(prepared.session.work_mode).to_owned(),
            started_at: started_at.map(crate::convert::timestamp),
            profile: prepared.profile.name.clone(),
            mounts: mounts_of(&argv, &prepared),
            env: env_of(&prepared),
            argv_preview: shell_line(&argv),
            agent_running,
            refusals: self.running_refusals(),
            llm: self
                .llm_reach
                .current()
                .as_ref()
                .map(crate::convert::diagnostic_to_proto),
            sandbox_mode: prepared.plan_mode.mode.as_str().to_owned(),
            sandbox_mode_notice: prepared
                .plan_mode
                .notice
                .as_ref()
                .map(crate::convert::diagnostic_to_proto),
        })
    }

    /// Der Schnappschuss, den ein Befund übrig lässt: alles, was ohne Profil
    /// noch bekannt ist, und der Zustand `failed`.
    ///
    /// Ein leerer Zustand wäre hier falsch. Die Oberfläche muss sagen können,
    /// welches Projektverzeichnis und welches Profil gemeint waren, sonst
    /// steht der Befund über einem Bildschirm ohne Bezug.
    fn failed_status(
        &self,
        plan: &v1::sandbox_request::Plan,
        diagnostic: &Diagnostic,
    ) -> v1::sandbox_event::Status {
        self.failed_status_with(&self.config(), &self.pending_choice(), plan, diagnostic)
    }

    /// [`Inner::failed_status`] from this configuration.
    fn failed_status_with(
        &self,
        config: &Config,
        choice: &PlanChoice,
        plan: &v1::sandbox_request::Plan,
        _diagnostic: &Diagnostic,
    ) -> v1::sandbox_event::Status {
        let Facts {
            id,
            started_at,
            agent_running,
            ..
        } = self.running_facts();
        v1::sandbox_event::Status {
            state: v1::SandboxState::Failed.into(),
            sandbox_id: id.unwrap_or_default(),
            session_id: self.session.to_string(),
            backend: "bwrap".to_owned(),
            llm_endpoint: config
                .llm
                .endpoint
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default(),
            work_dir: self.work_dir(config, choice, plan).display().to_string(),
            work_mode: work_mode_name(self.work_mode(config, choice, plan)).to_owned(),
            started_at: started_at.map(crate::convert::timestamp),
            profile: self.profile_name(config, choice, plan),
            mounts: Vec::new(),
            env: Vec::new(),
            argv_preview: String::new(),
            agent_running,
            refusals: self.running_refusals(),
            llm: self
                .llm_reach
                .current()
                .as_ref()
                .map(crate::convert::diagnostic_to_proto),
            // Ohne Plan kein aufgelöster Modus (HUM-357).
            sandbox_mode: String::new(),
            sandbox_mode_notice: None,
        }
    }

    /// Die Protokollzeile eines Starts; `None`, wenn nichts läuft.
    fn started_line(&self) -> Option<String> {
        let running = lock(&self.running);
        running.as_ref().map(|running| {
            format!(
                "sandbox {} started, pid {}, profile {}, work dir {}",
                running.handle.id,
                running.handle.pid,
                running.profile,
                running.work_dir.display()
            )
        })
    }

    /// Die Protokollzeile eines Stopps; `None`, wenn nichts lief.
    fn stopped_line(&self) -> Option<String> {
        let running = lock(&self.running);
        running.as_ref().map(|running| {
            let exit = running
                .handle
                .try_wait()
                .and_then(|status| status.code())
                .map_or_else(|| "signal".to_owned(), |code| code.to_string());
            format!("sandbox {} stopped, exit {}", running.handle.id, exit)
        })
    }

    /// Id, Startzeit, ob eine Sandbox gehalten wird und ob darin noch etwas
    /// läuft.
    fn running_facts(&self) -> Facts {
        let running = lock(&self.running);
        running
            .as_ref()
            .map_or_else(Facts::default, |running| Facts {
                id: Some(running.handle.id.to_string()),
                started_at: Some(running.started_at),
                held: true,
                agent_running: running.handle.try_wait().is_none(),
            })
    }

    /// Ob der Abschied schon begonnen hat; siehe [`Inner::leaving`].
    fn is_leaving(&self) -> bool {
        self.leaving.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn is_running(&self) -> bool {
        lock(&self.running)
            .as_ref()
            .is_some_and(|running| running.handle.try_wait().is_none())
    }

    /// Die Ablage der weiteren Shells der laufenden Sitzung (HUM-299).
    fn running_shells(&self) -> Option<ShellRegistry> {
        lock(&self.running)
            .as_ref()
            .map(|running| running.shells.clone())
    }

    /// Öffnet eine weitere Shell; siehe [`SandboxService::exec`]. Blockiert.
    fn open_shell(&self, request: &v1::ExecRequest) -> Result<crate::exec::Opened, Diagnostic> {
        // The same answer as `Terminal`: no session, or another one meant.
        let hub = self.terminal(&request.sandbox_id)?;
        let Some((handle, shells)) = lock(&self.running)
            .as_ref()
            .map(|running| (Arc::clone(&running.handle), running.shells.clone()))
        else {
            return Err(no_terminal(
                "no sandbox is running in this session, so there is nothing to open a shell in"
                    .to_owned(),
            ));
        };
        if handle.id != hub.sandbox() {
            return Err(no_terminal(
                "the session changed while the shell was being opened".to_owned(),
            ));
        }
        crate::exec::open(&handle, hub.sandbox(), &shells, request, hub.notices())
    }

    /// Das Terminal der laufenden Sitzung; siehe [`SandboxService::terminal`].
    fn terminal(&self, sandbox_id: &str) -> Result<TerminalHub, Diagnostic> {
        let Some(running) = lock(&self.running)
            .as_ref()
            .map(|running| running.terminal.clone())
        else {
            return Err(no_terminal(
                "no sandbox is running in this session, so there is no terminal to attach to"
                    .to_owned(),
            ));
        };
        let wanted = sandbox_id.trim();
        if !wanted.is_empty() && wanted != running.sandbox().to_string() {
            return Err(no_terminal(format!(
                "this daemon runs sandbox {}, not {wanted}",
                running.sandbox()
            )));
        }
        Ok(running)
    }

    /// Das Handle der laufenden Sandbox und was ein Befund über ihren Agenten
    /// braucht, unter einem Schloss.
    fn running_agent(&self) -> Option<(Arc<SandboxHandle>, AgentFacts)> {
        lock(&self.running)
            .as_ref()
            .map(|running| (Arc::clone(&running.handle), running.agent.clone()))
    }

    fn running_handle(&self) -> Option<Arc<SandboxHandle>> {
        lock(&self.running)
            .as_ref()
            .map(|running| Arc::clone(&running.handle))
    }

    /// Der Stand der verweigerten Versuche der laufenden Sandbox, für die
    /// Momentaufnahme (HUM-138); `None`, wenn keine läuft.
    fn running_refusals(&self) -> Option<v1::sandbox_event::Refusals> {
        self.running_handle()
            .map(|handle| crate::convert::refusals_to_proto(&handle.report().refusals))
    }

    /// Reicht jeden neuen Stand der verweigerten Versuche in den Strom dieses
    /// Starts, bis der Bericht der Sandbox zu ist (HUM-138).
    ///
    /// **Nur dieser eine Strom bekommt die laufenden Stände**, der des Starts,
    /// der die Sitzung begonnen hat. Ein zweites Fenster oder `humanitl run`
    /// neben der Oberfläche sieht den Stand nur in der Momentaufnahme
    /// (`Status.refusals`), wenn es sie holt; ihn über den Hub an jeden
    /// Zuhörer zu verteilen, ist HUM-202.
    ///
    /// Jede Nachricht trägt den ganzen Stand; der Shim fasst höchstens alle
    /// zwei Sekunden zusammen, also kommt auch hier keine Flut an. Meldet die
    /// Sandbox nicht (`REFUSALS off`), geht einmal `SANDBOX_019` voraus, damit
    /// eine leere Liste nicht als „nichts versucht" gelesen wird.
    ///
    /// Der Faden endet, wenn die Pipe des Berichts zu ist, wenn niemand mehr
    /// zuhört, oder [`REFUSALS_DRAIN`] nach dem Ende der Sandbox, falls die
    /// Pipe dann noch jemand hält.
    fn forward_refusals(
        &self,
        tx: &mpsc::Sender<v1::SandboxEvent>,
    ) -> Option<tokio::task::JoinHandle<()>> {
        let handle = self.running_handle()?;
        let tx = tx.clone();
        Some(tokio::task::spawn_blocking(move || {
            let mut seen = 0;
            let mut warned = false;
            let mut ended: Option<Instant> = None;
            loop {
                let (refusals, closed) = handle.wait_refusals(seen, REFUSALS_POLL);
                if refusals.generation > seen {
                    seen = refusals.generation;
                    if !warned && let Some(diagnostic) = refusals.unreported() {
                        warned = true;
                        let _ = tx.blocking_send(diagnostic_event(&diagnostic));
                    }
                    let event = v1::SandboxEvent {
                        event: Some(v1::sandbox_event::Event::Refusals(
                            crate::convert::refusals_to_proto(&refusals),
                        )),
                    };
                    if tx.blocking_send(event).is_err() {
                        return;
                    }
                }
                if closed || tx.is_closed() {
                    return;
                }
                if handle.try_wait().is_some() {
                    let since = *ended.get_or_insert_with(Instant::now);
                    if since.elapsed() >= REFUSALS_DRAIN {
                        return;
                    }
                }
            }
        }))
    }

    /// Meldet den Zuhörer für den Stand der Durchreiche an und beginnt den
    /// Lauf ohne Befund (HUM-251); `None` ohne Warteschlange (Fake-Modus,
    /// Tests), denn dann gibt es keine Durchreiche.
    ///
    /// **Vor dem Start des Agenten zu rufen.** Ein Agent, der sofort ein
    /// totes Modell fragt, erzeugt `Decided` und `Failed` in den ersten
    /// Millisekunden; ein Zuhörer, der sich erst nach der Prüfung der
    /// Garantien anmeldet, verpasste genau diese und meldete nie `LLM_001`.
    /// Der Empfänger puffert, was bis zu [`LlmWatch::run`] eintrifft.
    fn listen_llm(self: &Arc<Self>) -> Option<LlmWatch> {
        // Erst der Zuhörer, dann der neue Stand: Ein Ereignis dazwischen
        // gehörte sonst zu keinem Lauf.
        let events = self.notices.as_ref()?.subscribe();
        let endpoint = self
            .config()
            .llm
            .endpoint
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default();
        self.llm_reach.begin(&endpoint);
        Some(LlmWatch {
            inner: Arc::clone(self),
            events: Some(events),
            task: None,
        })
    }

    /// Das Backend, das gestartet hat, und sein Handle; beides oder nichts.
    fn running_parts(&self) -> Option<(PlatformBackend, Arc<SandboxHandle>)> {
        lock(&self.running)
            .as_ref()
            .map(|running| (running.backend.clone(), Arc::clone(&running.handle)))
    }

    fn clear_running(&self) {
        *lock(&self.running) = None;
        self.bump_generation();
    }

    /// Profil, Sitzung und Backend für diesen Wunsch.
    #[cfg(test)]
    fn prepare_with_command(
        &self,
        plan: &v1::sandbox_request::Plan,
        command: &[OsString],
    ) -> Result<Prepared, Diagnostic> {
        self.prepare_keyed(plan, command, KeyNeed::Plan)
    }

    /// Wie `prepare_with_command`, und für einen Start mit dem
    /// Zugangsschlüssel des Modells ([`KeyNeed::Start`]).
    fn prepare_keyed(
        &self,
        plan: &v1::sandbox_request::Plan,
        command: &[OsString],
        need: KeyNeed,
    ) -> Result<Prepared, Diagnostic> {
        // Eine Aufnahme der Konfiguration für den ganzen Aufbau: Ein Start,
        // der dazwischenkommt, darf nicht die Hälfte einer Kommandozeile
        // ersetzen.
        self.prepare_keyed_with(&self.config(), &self.pending_choice(), plan, command, need)
    }

    /// [`Inner::prepare_keyed`] from this one configuration (HUM-277).
    fn prepare_keyed_with(
        &self,
        config: &Arc<Config>,
        choice: &PlanChoice,
        plan: &v1::sandbox_request::Plan,
        command: &[OsString],
        need: KeyNeed,
    ) -> Result<Prepared, Diagnostic> {
        let config = Arc::clone(config);
        let policy = MountPolicy::from_paths(&self.paths);
        let name = self.profile_name(&config, choice, plan);
        let (profile_path, profile_origin) = self.profile_path(&name)?;
        let profile = SandboxProfile::load_validated(&profile_path, &policy)?;

        let work_src = self.work_dir(&config, choice, plan);
        // Dieselbe Regel, die das Sandbox-Backend beim Start anlegt (HUM-262),
        // und schon hier, damit `Sandbox(Plan)` und `Sandbox(Status)` einen Ordner nie
        // grün zeigen, den der Start danach verweigert. Ein Wunsch über die
        // RPC ist in `check_work_dir` schon geprüft; ein Ordner aus
        // `config.toml` kam bisher ungeprüft bis zum Start durch; seit
        // HUM-294 geht er unten durch alle drei Stufen, auch beim Plan, und
        // ein gespeicherter Ordner, den es nicht mehr gibt, ist dann schon im
        // Plan `SANDBOX_006`. Kein Ordner
        // ist für einen Plan kein Befund, sondern eine offene Wahl: Die
        // Momentaufnahme nennt dann keinen, und das Setup verlangt einen.
        // Der Start prüft dazu, dass der Ordner da ist. Der Befund nennt die
        // Quelle des Pfades (HUM-293); gesucht wird sie erst, wenn es einen
        // Befund gibt, denn für einen Ordner aus der Konfiguration heißt das,
        // sie noch einmal zu lesen.
        if need == KeyNeed::Start {
            with_source(
                |from| check_project_dir(&policy, &work_src, from),
                || self.work_dir_source(&config, choice, plan),
            )?;
        } else if !work_src.as_os_str().is_empty() {
            with_source(
                |from| policy.check_work_dir_from(&work_src, from),
                || self.work_dir_source(&config, choice, plan),
            )?;
        }
        // Der gespeicherte Ordner bekommt bei jedem Plan und Start die ganze
        // Prüfung eines Ordners über die RPC, auch die dritte Stufe (unter
        // dem Heimatverzeichnis oder erklärt), und das gegen seinen heutigen
        // aufgelösten Pfad (HUM-294). Sonst hängte ein Start, nachdem ein
        // Elternverzeichnis durch einen Verweis nach außerhalb ersetzt wurde,
        // den neuen Ort ein, den nie jemand gewählt hat.
        // Der aufgelöste Ort, der hier freigegeben ist, reist zum Backend: Es
        // bindet den Ordner nur, wenn sein geöffneter Deskriptor genau dort
        // liegt. Ein Verweis, der zwischen dieser Prüfung und dem Öffnen in
        // den Pfad kommt, erreicht die Sandbox so nicht (HUM-294, Review). Ein
        // Ordner aus der Anfrage ist schon der aufgelöste, den
        // `check_work_dir` freigegeben hat; die Vorgabe aus dem aktuellen
        // Verzeichnis prüft nur die Politik, wie bisher.
        let approved = if self.is_stored_work_dir(&config, choice, plan, &work_src) {
            Some(with_source(
                |from| {
                    crate::work_dir::check_requested(&self.paths, Some(&work_src), &work_src, from)
                },
                || self.work_dir_source(&config, choice, plan),
            )?)
        } else {
            choice
                .work_dir
                .as_ref()
                .filter(|checked| **checked == work_src)
                .cloned()
        };
        let work_mode = self.work_mode(&config, choice, plan);
        // Every session gets the adapter's contribution, whatever its command
        // (HUM-280). `humanitl run -- opencode` starts the same agent as
        // `humanitl run` (HUM-135), and since HUM-280 `humanitl run -- bash`
        // is a shell in which the person starts that agent by hand: with the
        // same model, key file, CA and configuration files, and with the
        // configured agent bound on `PATH`. The caller's own command is still
        // never bound.
        //
        // An adapter that does not exist is `CONFIG_003` for every command;
        // without that, an empty command would silently start a shell.
        let agent = self.agent_contribution(&config, &work_src, &profile, command, need)?;

        let mut session_env = vec![(ENV_SESSION.to_owned(), self.session.to_string())];
        session_env.extend(agent.env.iter().cloned());
        session_env.extend(
            config
                .sandbox
                .env
                .iter()
                .map(|(key, value)| (key.clone(), value.clone())),
        );

        // Wer welchen Wert geschrieben hat, in der Reihenfolge, in der
        // `effective_env` zusammenführt: Profil, dann die Sitzung (Kennung,
        // Adapter, `sandbox.env`), dann der Shim. Der letzte gewinnt, hier wie
        // dort. Zwei Läufe über dieselbe Reihenfolge sind der Preis dafür, dass
        // die Werte weiter aus genau einer Quelle kommen
        // (`SandboxProfile::effective_env`) statt aus einer zweiten Kopie.
        let mut env_origin: BTreeMap<String, v1::ValueOrigin> = BTreeMap::new();
        for key in profile.env.keys() {
            env_origin.insert(key.clone(), profile_origin);
        }
        env_origin.insert(ENV_SESSION.to_owned(), v1::ValueOrigin::Session);
        for (key, _) in &agent.env {
            env_origin.insert(key.clone(), v1::ValueOrigin::Adapter);
        }
        // `sandbox.env` steht in `config.toml` und ist von Hand geschrieben;
        // es überschreibt Profil und Adapter, also überschreibt es auch deren
        // Herkunft. Genau hier landet später die Zugangsdaten-Injektion.
        for key in config.sandbox.env.keys() {
            env_origin.insert(key.clone(), v1::ValueOrigin::User);
        }
        for key in humanitl_sandbox::RESERVED_ENV {
            env_origin.insert((*key).to_owned(), v1::ValueOrigin::Session);
        }

        let session = SessionContext {
            session: self.session,
            work_src,
            work_mode,
            proxy_socket_src: self.paths.proxy_socket(),
            ca_cert_src: self.paths.ca_cert_path(),
            ca_bundle_src: self.paths.ca_dir().join(CA_BUNDLE_FILE),
            shim_src: shim_path(),
            session_env,
            // A bound program rewrote the adapter's command; the caller's own
            // words would name the host path the sandbox does not have. A
            // caller's command is never rewritten: it comes back unchanged
            // from the adapter, and the bind beside it serves `PATH` only.
            command: if agent.program.is_some() {
                agent.command.clone()
            } else {
                Self::command(&config, command, &agent.command)
            },
            files: agent.files,
            agent_program: agent.program,
        };
        let warnings = agent.warnings;

        // Ein Schnappschuss ist eine Anzeige, kein Start: Ohne `bwrap` steht
        // die Zeile trotzdem da, und der Start scheitert später mit dem
        // Befund, der dazu gehört (`SANDBOX_001`).
        let backend = PlatformBackend::detect(self.paths.clone())
            .unwrap_or_else(|_| PlatformBackend::fallback(self.paths.clone()));
        let backend = match approved {
            Some(dir) => backend.with_approved_work_dir(dir),
            None => backend,
        };

        Ok(self.with_plan_mode(Prepared {
            env_origin,
            adapter_files: session.files.iter().map(|file| file.dst.clone()).collect(),
            warnings,
            profile,
            session,
            backend,
            overlay_preview: None,
            wanted_mode: config.sandbox.mode,
            plan_mode: humanitl_sandbox::ModeChoice {
                mode: SandboxMode::Direct,
                notice: None,
            },
        }))
    }

    /// Der Modus, den ein Start ohne eigenen Wunsch bekäme, und die Schicht,
    /// die die Zeile dann zeigt (HUM-357). Ein ausdrückliches `disposable`,
    /// das hier nicht geht, zeigt die Zeile weiter mit Overlay und nennt den
    /// Befund, mit dem der Start ablehnen wird.
    fn with_plan_mode(&self, mut prepared: Prepared) -> Prepared {
        prepared.plan_mode = self
            .mode_choice(prepared.wanted_mode, &prepared)
            .unwrap_or_else(|refusal| humanitl_sandbox::ModeChoice {
                mode: SandboxMode::Disposable,
                notice: Some(refusal),
            });
        // Warten Änderungen, lehnt der nächste Start ohne `direct` ab; die
        // Momentaufnahme sagt das, statt einen Modus ohne Hinweis zu nennen
        // (HUM-357, Review).
        if let Err(pending) = self.pending_guard(prepared.wanted_mode) {
            prepared.plan_mode.notice = Some(pending);
        }
        prepared.overlay_preview = self.overlay_preview(prepared.plan_mode.is_disposable());
        prepared
    }

    /// Was der Agent-Adapter zu dieser Sitzung beiträgt.
    fn agent_contribution(
        &self,
        config: &Config,
        work_src: &Path,
        profile: &SandboxProfile,
        command: &[OsString],
        need: KeyNeed,
    ) -> Result<AgentContribution, Diagnostic> {
        let registry = AdapterRegistry::builtin();
        let adapter = registry.get(&config.agent.adapter).ok_or_else(|| {
            Diagnostic::builder(codes::CONFIG_003, Severity::Blocking)
                .why(format!(
                    "agent.adapter is {:?}, and no adapter of that name exists; known: {}",
                    config.agent.adapter,
                    registry.ids().join(", ")
                ))
                .fix(FixAction::ChangeSetting {
                    key: "agent.adapter".to_owned(),
                    value: registry
                        .ids()
                        .first()
                        .map_or_else(String::new, |id| (*id).to_owned()),
                })
                .build()
        })?;

        // A shell from the caller does not wait for a locked or missing key
        // (HUM-280): it starts without one and says so. The agent the daemon
        // starts itself still stops with `LLM_009`, because without the key
        // it would only fail at its first request.
        let mut warnings = Vec::new();
        let key = match self.agent_key(config, need) {
            Ok(key) => key,
            Err(found) if !command.is_empty() && found.code == codes::LLM_009 => {
                warnings.push(started_without_key(found));
                AgentKey::None
            }
            Err(found) => return Err(found),
        };

        let ctx = AgentContext::new(self.session, work_src.to_path_buf(), config.llm.clone())
            // Das Kommando, das wirklich läuft: das genannte, sonst das der
            // Konfiguration. Der Adapter prüft damit den Pfad, der gestartet
            // wird, und nicht einen anderen, den er auf dem Host fände
            // (`AGENT_004` nannte sonst eine Datei, die niemand aufrufen
            // wollte). Die Herkunft geht mit: Ein Befund über ein Kommando von
            // der Kommandozeile darf nicht `agent.command` anfassen.
            .with_host_path(self.paths.env().non_empty("PATH").map(OsString::from))
            .with_host_home(self.paths.env().non_empty("HOME").map(PathBuf::from))
            .with_language(config.ui.language)
            .with_hold(config.hold.clone())
            .with_briefing(config.agent.briefing.clone())
            .with_pi_web_access(config.agent.pi.web_access)
            .with_home(
                config
                    .sandbox
                    .env
                    .get("HOME")
                    .or_else(|| profile.env.get("HOME"))
                    .map_or_else(
                        || PathBuf::from(humanitl_sandbox::DEFAULT_HOME),
                        PathBuf::from,
                    ),
            )
            .with_config_home(config.sandbox.env.get("XDG_CONFIG_HOME").map(PathBuf::from))
            // Was die Sandbox sieht, kommt aus dem Profil: Einhängungen, Verweise
            // und Überdeckungen in einem Stück (HUM-139). Der Suchpfad ist der
            // wirksame, also `sandbox.env` vor `[env]` des Profils — dieselbe
            // Reihenfolge, die der Start selbst anwendet; eine Vorprüfung gegen
            // einen anderen PATH als den, unter dem das `exec` läuft, beantwortet
            // die falsche Frage.
            .with_sandbox_view(humanitl_sandbox::SandboxView::of_profile(profile))
            .with_sandbox_path(
                config
                    .sandbox
                    .env
                    .get("PATH")
                    .or_else(|| profile.env.get("PATH"))
                    .map(OsString::from),
            )
            .with_work_dir_sandbox(profile.mounts.work.dst.clone())
            .with_llm_key(key);
        let ctx = if command.is_empty() {
            ctx.with_command_override(
                config
                    .agent
                    .command
                    .as_ref()
                    .map(|parts| parts.iter().map(OsString::from).collect()),
            )
        } else {
            ctx.with_command_from_caller(command.to_vec())
        };

        // An agent outside the sandbox's mounts, for example in
        // `~/.opencode/bin`, is bound read-only as one file and the command
        // points at it (HUM-278). A command from the caller stays as it is
        // and is never bound; the configured agent is bound beside it, so a
        // shell finds it on `PATH` (HUM-280).
        let mut command = adapter.command(&ctx);
        let program = if ctx.agent_command_from_caller {
            humanitl_sandbox::agent::plan_configured_agent(
                &ctx,
                adapter,
                config
                    .agent
                    .command
                    .as_ref()
                    .map(|parts| parts.iter().map(OsString::from).collect()),
            )
        } else {
            humanitl_sandbox::agent::bind_agent_program(&ctx, &mut command)
        };
        if cfg!(target_os = "macos") && !ctx.agent_command_from_caller {
            command = adapter.on_shared_loopback(command);
        }
        Ok(AgentContribution {
            command,
            program,
            env: adapter.env(&ctx),
            files: adapter.files(&ctx)?,
            warnings,
        })
    }

    /// Was der Adapter vom Zugangsschlüssel des Modells bekommt (HUM-140).
    ///
    /// Ein Plan bekommt nur die Ansage, ein Start den Schlüssel selbst. Bevor
    /// ihn der Adapter sieht, steht er in den Masken der Aufzeichnung: Die
    /// erste Anfrage des Agenten an das Modell trägt ihn schon.
    ///
    /// # Errors
    ///
    /// `LLM_009`, wenn der Speicher den Schlüssel nicht herausgibt, und
    /// `CONFIG_003`, wenn `llm.api_key_ref` kein Verweis ist (die Prüfung der
    /// Konfiguration lässt das nicht zu; ein Resolver ohne sie schon).
    fn agent_key(&self, config: &Config, need: KeyNeed) -> Result<AgentKey, Diagnostic> {
        let Some(text) = config.llm.api_key_ref.as_deref() else {
            return Ok(AgentKey::None);
        };
        let key_ref = KeyRef::parse(text).ok_or_else(|| {
            Diagnostic::builder(codes::CONFIG_003, Severity::Error)
                .why(format!(
                    "llm.api_key_ref = {text:?} is not a reference to an access key; it takes \
                     secret-service:humanitl/NAME, keychain:humanitl/NAME or file:NAME"
                ))
                .fix(FixAction::CopyCommand("humanitl llm key set".to_owned()))
                .build()
        })?;
        // Die Regel des Ladens, noch einmal an der Stelle, die den Schlüssel
        // holt (HUM-289): Er geht nur mit dem Endpunkt desselben Eintrags.
        if !config.llm.key_goes_with_endpoint() {
            tracing::warn!(
                key_ref = %key_ref,
                "the access key does not belong to the configured endpoint; it is not given to \
                 the agent"
            );
            return Ok(AgentKey::None);
        }
        if need == KeyNeed::Plan {
            return Ok(AgentKey::Withheld);
        }
        let key = self.keys.lookup(&key_ref)?;
        if let Some(recorder) = &self.recorder {
            recorder.mask_secret(key.expose(), endpoint_authority(config).as_deref());
        }
        Ok(AgentKey::Key(key))
    }

    /// Der Befehl in der Sandbox: der des Aufrufers, sonst der des Adapters,
    /// sonst der aus `agent.command`, sonst die Shell.
    fn command(
        config: &Config,
        requested: &[OsString],
        from_adapter: &[OsString],
    ) -> Vec<OsString> {
        if !requested.is_empty() {
            return requested.to_vec();
        }
        if !from_adapter.is_empty() {
            return from_adapter.to_vec();
        }
        config
            .agent
            .command
            .as_ref()
            .filter(|command| !command.is_empty())
            .map_or_else(
                || vec![OsString::from(SANDBOX_SHELL)],
                |command| command.iter().map(OsString::from).collect(),
            )
    }

    /// Der Profilname dieses Wunsches, sonst der der laufenden Sandbox, sonst
    /// der zuletzt gewählte, sonst der der Konfiguration.
    ///
    /// Die Reihenfolge ist eine Aussage: Was läuft, gilt vor dem, was gewählt
    /// ist, und das vor dem, was konfiguriert ist. Sonst zeigte der Bildschirm
    /// während einer Sitzung das Profil, das beim nächsten Start gälte, und
    /// nicht das, unter dem der Agent gerade arbeitet.
    fn profile_name(
        &self,
        config: &Config,
        choice: &PlanChoice,
        plan: &v1::sandbox_request::Plan,
    ) -> String {
        if !plan.profile.trim().is_empty() {
            return plan.profile.trim().to_owned();
        }
        if let Some(running) = lock(&self.running).as_ref() {
            return running.profile.clone();
        }
        if let Some(chosen) = choice.profile.clone() {
            return chosen;
        }
        config.sandbox.profile.clone()
    }

    /// Das Projektverzeichnis dieses Wunsches, sonst das der laufenden
    /// Sandbox, sonst das des Starts, der gerade anläuft, sonst
    /// `sandbox.work_dir` der Konfiguration, sonst [`Self::default_work_dir`].
    fn work_dir(
        &self,
        config: &Config,
        choice: &PlanChoice,
        plan: &v1::sandbox_request::Plan,
    ) -> PathBuf {
        self.work_dir_with_cwd(
            config,
            choice,
            plan,
            std::env::current_dir().ok().as_deref(),
        )
    }

    /// [`Self::work_dir`] mit dem Arbeitsverzeichnis als Argument statt aus dem
    /// Prozess, damit ein Test die Vorgabe mit einem abgelehnten Verzeichnis
    /// prüfen kann, ohne das Arbeitsverzeichnis aller Tests zu verstellen.
    fn work_dir_with_cwd(
        &self,
        config: &Config,
        choice: &PlanChoice,
        plan: &v1::sandbox_request::Plan,
        cwd: Option<&Path>,
    ) -> PathBuf {
        // Der Wunsch dieses Aufrufs, aber in der Fassung, die `check_work_dir`
        // aufgelöst hat: `remember` hat ihn unmittelbar davor abgelegt. Die
        // rohe Zeichenkette der Leitung wird nie eingehängt — sonst prüfte
        // der Dienst den aufgelösten Pfad und hängte den geschriebenen ein,
        // und die Prüfung sagte über die Einhängung nichts aus.
        if !plan.work_dir.trim().is_empty()
            && let Some(checked) = choice.work_dir.clone()
        {
            return checked;
        }
        if let Some(running) = lock(&self.running).as_ref() {
            return running.work_dir.clone();
        }
        if let Some(starting) = choice.work_dir.clone() {
            return starting;
        }
        config
            .sandbox
            .work_dir
            .clone()
            .unwrap_or_else(|| self.default_work_dir(cwd))
    }

    /// Ob `work_src` der Ordner aus `sandbox.work_dir` ist (HUM-294): kein
    /// Ordner in der Anfrage, keiner eines laufenden Starts, keine laufende
    /// Sitzung, und die Konfiguration nennt genau ihn. Einen Ordner aus der
    /// Anfrage hat [`crate::work_dir::check_requested`] schon geprüft, den
    /// einer laufenden Sitzung prüfte ihr Start.
    fn is_stored_work_dir(
        &self,
        config: &Config,
        choice: &PlanChoice,
        plan: &v1::sandbox_request::Plan,
        work_src: &Path,
    ) -> bool {
        plan.work_dir.trim().is_empty()
            && choice.work_dir.is_none()
            && lock(&self.running).is_none()
            && config.sandbox.work_dir.as_deref() == Some(work_src)
    }

    /// Woher der Ordner kommt, den [`Self::work_dir`] für diesen Wunsch
    /// nennt (HUM-293): die Quelle, die der Client zu seinem `work_dir`
    /// angibt, sonst die Herkunft von `sandbox.work_dir`, sonst das aktuelle
    /// Verzeichnis des Daemons.
    ///
    /// Für `sandbox.work_dir` liest das die Konfiguration noch einmal; die
    /// Aufnahme im Dienst kennt Werte, aber nicht ihre Herkunft. Scheitert
    /// das Lesen, ist die Quelle die Datei, die `SetConfig` schreibt.
    fn work_dir_source(
        &self,
        config: &Config,
        choice: &PlanChoice,
        plan: &v1::sandbox_request::Plan,
    ) -> WorkDirSource {
        if !plan.work_dir.trim().is_empty() && choice.work_dir.is_some() {
            return choice.work_dir_source.clone().unwrap_or_else(|| {
                crate::work_dir::source_from_proto(plan.work_dir_source.as_ref())
            });
        }
        if let Some(running) = lock(&self.running).as_ref() {
            return running.work_dir_source.clone();
        }
        if choice.work_dir.is_some() {
            return choice
                .work_dir_source
                .clone()
                .unwrap_or(WorkDirSource::Request);
        }
        if config.sandbox.work_dir.is_none() {
            return WorkDirSource::CurrentDir;
        }
        let file = self.paths.config_path();
        match self.resolver.current() {
            Ok(now) if now.config.sandbox.work_dir.is_some() => {
                crate::work_dir::configured_source(&now, &file)
            }
            Ok(_) | Err(_) => WorkDirSource::ConfigFile(file),
        }
    }

    /// Die Vorgabe, wenn niemand einen Ordner genannt hat: das
    /// Arbeitsverzeichnis des Daemons, aber nur, wenn ein Start es annähme.
    ///
    /// Der Daemon läuft als Nutzerdienst, und systemd startet ihn im
    /// Heimatverzeichnis; eine Anwendung aus dem Menü des Desktops ebenso.
    /// Ungeprüft wurde daraus der Projektordner `$HOME`, das Setup zeigte ihn
    /// grün, und erst der Start verweigerte ihn mit `SANDBOX_006` (HUM-262).
    /// Die Vorgabe geht deshalb durch dieselbe Regel, die der Start anlegt
    /// ([`MountPolicy::check_work_dir`]), und keine strengere: Eine neue Regel
    /// ist nicht das Ziel, nur dieselbe früher. Hält sie nicht, gibt es keine
    /// Vorgabe: Der leere Pfad heißt „niemand hat gewählt", und das Setup
    /// verlangt eine Auswahl (`CONFIG_013`), statt einen Ordner anzubieten,
    /// den der Start ablehnt.
    fn default_work_dir(&self, cwd: Option<&Path>) -> PathBuf {
        let policy = MountPolicy::from_paths(&self.paths);
        cwd.filter(|dir| dir.is_dir() && policy.check_work_dir(dir).is_ok())
            .map(Path::to_path_buf)
            .unwrap_or_default()
    }

    /// Der Modus dieses Wunsches, sonst der der laufenden Sandbox, sonst der
    /// zuletzt gewählte, sonst der der Konfiguration.
    fn work_mode(
        &self,
        config: &Config,
        choice: &PlanChoice,
        plan: &v1::sandbox_request::Plan,
    ) -> WorkMode {
        match plan.work_mode.trim().to_ascii_lowercase().as_str() {
            "ro" => WorkMode::Ro,
            "rw" => WorkMode::Rw,
            _ => {
                if let Some(running) = lock(&self.running).as_ref() {
                    return running.work_mode;
                }
                choice.work_mode.unwrap_or(config.sandbox.work_mode)
            }
        }
    }

    /// Die Datei des Sandbox-Profils samt der Herkunft ihrer Werte; dieselbe
    /// Suche wie in der Kommandozeile, damit beide dieselbe Politik lesen.
    ///
    /// Der eigene Ordner des Nutzers steht vorn und gilt als von Hand
    /// geschrieben ([`v1::ValueOrigin::User`]); die Verzeichnisse einer
    /// Installation und der Arbeitsbaum liefern das mitgelieferte Profil.
    ///
    /// # Errors
    ///
    /// `CONFIG_003`, wenn der Name kein Name ist ([`check_profile_name`]),
    /// `CONFIG_001`, wenn es die Datei nirgends gibt.
    fn profile_path(&self, name: &str) -> Result<(PathBuf, v1::ValueOrigin), Diagnostic> {
        check_profile_name(name)?;
        let file = format!("{name}.toml");
        let own = self.paths.profiles_dir().join("sandbox").join(&file);
        if own.is_file() {
            return Ok((own, v1::ValueOrigin::User));
        }
        let mut candidates = vec![own];
        candidates.extend(tree_dirs().map(|dir| dir.join(&file)));
        candidates.extend(
            PROFILE_DIRS
                .iter()
                .map(|dir| Path::new(dir).join(PROFILE_SUBDIR).join(&file)),
        );
        candidates
            .iter()
            .find(|path| path.is_file())
            .map(|path| (path.clone(), v1::ValueOrigin::Profile))
            .ok_or_else(|| {
                let looked = candidates
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                Diagnostic::builder(codes::CONFIG_001, Severity::Blocking)
                    .why(format!(
                        "no sandbox profile {name}; looked at {looked}. A profile from the \
                         working directory is not searched"
                    ))
                    .fix(FixAction::ChangeSetting {
                        key: "sandbox.profile".to_owned(),
                        value: "default".to_owned(),
                    })
                    .build()
            })
    }
}

/// Was von der laufenden Sandbox bekannt ist.
#[derive(Debug, Default)]
struct Facts {
    /// Die Kennung der Sandbox, solange eine gehalten wird.
    id: Option<String>,
    /// Wann sie gestartet wurde.
    started_at: Option<SystemTime>,
    /// Ob überhaupt eine gehalten wird. Das ist der Zustand.
    held: bool,
    /// Ob in ihr noch ein Prozess lebt. Das ist etwas anderes.
    agent_running: bool,
}

/// Was der Agent-Adapter beisteuert.
#[derive(Debug, Default)]
struct AgentContribution {
    command: Vec<OsString>,
    /// The agent program bound from outside the mounts (HUM-278).
    program: Option<humanitl_sandbox::agent::AgentProgram>,
    env: Vec<(String, String)>,
    files: Vec<SandboxFile>,
    /// Findings that do not stop the start, such as a shell without the key
    /// (HUM-280).
    warnings: Vec<Diagnostic>,
}

/// `LLM_009` as a warning: the caller's command starts without the key.
fn started_without_key(mut found: Diagnostic) -> Diagnostic {
    found.severity = Severity::Warning;
    found.why = format!(
        "{}; the session starts anyway, without the key, and an agent started in it has no          access key for the model",
        found.why
    );
    found
}

/// Profil, Sitzung und Backend eines Schnappschusses.
#[derive(Debug)]
struct Prepared {
    profile: SandboxProfile,
    session: SessionContext,
    backend: PlatformBackend,
    /// Wer den Wert jeder Umgebungsvariablen zuletzt geschrieben hat.
    ///
    /// Dieselbe Reihenfolge, in der [`SandboxProfile::effective_env`]
    /// zusammenführt; der letzte Schreiber gewinnt. Ein Schlüssel, der hier
    /// fehlt, gilt als von Hand geschrieben und wird zurückgehalten.
    env_origin: BTreeMap<String, v1::ValueOrigin>,
    /// Die Ziele der Dateien des Adapters; für die Herkunftsspalte.
    adapter_files: BTreeSet<PathBuf>,
    /// Befunde der Vorbereitung, die den Start nicht aufhalten (HUM-280).
    warnings: Vec<Diagnostic>,
    /// Die Schicht, die die Zeile zeigt, wenn der Plan eine Wegwerf-Sandbox
    /// ist (HUM-350, HUM-357). Die echte entsteht erst beim Start und trägt
    /// dann eine eigene Kennung an der Stelle von `LAYER`.
    overlay_preview: Option<humanitl_sandbox::OverlayDirs>,
    /// `sandbox.mode` aus der Aufnahme der Konfiguration, die diesen Plan
    /// gebaut hat (HUM-357).
    wanted_mode: SandboxMode,
    /// [`Prepared::wanted_mode`], aufgelöst gegen das System: der Modus eines
    /// Starts ohne eigenen Wunsch, mit dem Grund einer Abweichung (HUM-357).
    plan_mode: humanitl_sandbox::ModeChoice,
}

impl Prepared {
    /// Wer den Wert dieser Variablen geschrieben hat; unbekannt zählt als
    /// von Hand geschrieben, damit die Vorgabe zurückhält.
    fn origin_of(&self, key: &str) -> v1::ValueOrigin {
        self.env_origin
            .get(key)
            .copied()
            .unwrap_or(v1::ValueOrigin::User)
    }

    /// Ob der Wert dieser Variablen angezeigt werden darf.
    fn shows(&self, key: &str) -> bool {
        shows_env_value(key, self.origin_of(key))
    }

    /// Die vollständige Kommandozeile, `argv[0]` ist das Programm, mit
    /// [`WITHHELD_PLACEHOLDER`] anstelle jedes Werts, den auch die
    /// Umgebungstabelle zurückhält.
    ///
    /// **Nur zur Anzeige.** Gestartet wird die Liste, die
    /// [`SandboxProfile::to_bwrap_args`] beim Start selbst baut; diese hier
    /// geht an die Oberfläche und über sie in die Zwischenablage.
    fn argv(&self) -> Vec<OsString> {
        let mut args = vec![self.backend.program().as_os_str().to_owned()];
        // The daemon starts every session at its own pseudo-terminal, so the
        // shim gets the controlling terminal there (HUM-280), and the line
        // says so.
        args.extend(
            self.profile.to_bwrap_args(
                &self.session,
                &LaunchInputs::preview_with_agent_files(
                    self.session.files.iter().map(|file| file.dst.clone()),
                )
                .with_controlling_terminal(true)
                .with_overlay(self.overlay_preview.clone()),
            ),
        );
        self.redact_hidden_values(&mut args);
        args
    }

    /// Ersetzt in `--setenv <KEY> <VALUE>` jeden Wert, den auch die
    /// Umgebungstabelle zurückhält, durch [`WITHHELD_PLACEHOLDER`].
    ///
    /// Dieselbe Entscheidung wie dort ([`Prepared::shows`]) und keine zweite
    /// daneben: Wo die Tabelle Punkte zeigt und die Zeile den Wert, wäre die
    /// Zusage gebrochen.
    ///
    /// Die Einhängungen bleiben unberührt: `--setenv` steht hinter
    /// `--clearenv` und damit hinter allem, was die Tabelle liest.
    fn redact_hidden_values(&self, args: &mut [OsString]) {
        let setenv = OsString::from("--setenv");
        let mut index = 0;
        while index + 2 < args.len() {
            if args[index] == setenv && !self.shows(&args[index + 1].to_string_lossy()) {
                args[index + 2] = OsString::from(WITHHELD_PLACEHOLDER);
            }
            index += 1;
        }
    }

    /// Die Ziele, die aus der Sitzung stammen und nicht aus dem Profil.
    fn session_dsts(&self) -> BTreeSet<PathBuf> {
        let mut dsts = BTreeSet::new();
        dsts.insert(self.profile.mounts.work.dst.clone());
        dsts.insert(self.profile.network.proxy_socket_dst.clone());
        dsts.insert(self.profile.network.ca_cert_dst.clone());
        dsts.insert(self.profile.network.shim_dst.clone());
        dsts.insert(PathBuf::from(humanitl_sandbox::CA_BUNDLE_DST));
        dsts
    }

    /// Die Ziele, die der Nutzer selbst ins Profil geschrieben hat.
    ///
    /// Sie stehen getrennt, weil der Satz im Einhänge-Reiter sie namentlich
    /// nennen muss: Er behauptet, der Agent sehe nur das Projekt, und jede
    /// dieser Erweiterungen ist eine Ausnahme davon.
    fn user_dsts(&self) -> BTreeSet<PathBuf> {
        self.profile
            .mounts
            .extra_ro
            .iter()
            .chain(&self.profile.mounts.extra_rw)
            .cloned()
            .collect()
    }
}

/// Die Einhängungen, wie sie in der Kommandozeile stehen.
///
/// Gelesen wird der Argumentvektor selbst und nur bis zum ersten `--`; was
/// danach kommt, ist der Shim mit dem Befehl des Agenten und hängt nichts ein.
/// Damit kann die Tabelle keinen Mount verlieren, den die Zeile hat.
fn mounts_of(argv: &[OsString], prepared: &Prepared) -> Vec<v1::Mount> {
    let session_dsts = prepared.session_dsts();
    let user_dsts = prepared.user_dsts();
    let mut mounts = Vec::new();
    let mut index = 0;
    // `--overlay-src LOWER` gehört zum nächsten `--overlay` (HUM-350).
    let mut lower = String::new();
    while index < argv.len() {
        let flag = argv[index].to_string_lossy().into_owned();
        if flag == "--" {
            break;
        }
        if flag == "--overlay-src" && index + 1 < argv.len() {
            lower = argv[index + 1].to_string_lossy().into_owned();
            index += 2;
            continue;
        }
        // Das Projekt einer Wegwerf-Sandbox: Auf diesem Rechner ist es nur
        // lesbar eingehängt; was der Agent schreibt, landet in der oberen
        // Schicht. Die Zeile nennt deshalb den Ordner des Projekts mit `ro`.
        if flag == "--overlay" && index + 3 < argv.len() {
            let dst = argv[index + 3].to_string_lossy().into_owned();
            mounts.push(v1::Mount {
                origin: v1::ValueOrigin::Session.into(),
                dst,
                src: std::mem::take(&mut lower),
                mode: v1::MountMode::Ro.into(),
                link_target: String::new(),
            });
            index += 4;
            continue;
        }
        let (mode, arity) = match flag.as_str() {
            "--ro-bind" | "--ro-bind-try" => (v1::MountMode::Ro, 2),
            "--bind" | "--bind-try" | "--dev-bind" | "--dev-bind-try" => (v1::MountMode::Rw, 2),
            "--ro-bind-data" | "--bind-data" | "--file" => (v1::MountMode::Masked, 2),
            "--symlink" => (v1::MountMode::Symlink, 2),
            "--tmpfs" => (v1::MountMode::Tmpfs, 1),
            "--proc" => (v1::MountMode::Proc, 1),
            "--dev" => (v1::MountMode::Dev, 1),
            _ => {
                index += 1;
                continue;
            }
        };
        if index + arity >= argv.len() {
            break;
        }
        let (first, dst) = if arity == 2 {
            (
                argv[index + 1].to_string_lossy().into_owned(),
                argv[index + 2].to_string_lossy().into_owned(),
            )
        } else {
            (
                String::new(),
                argv[index + 1].to_string_lossy().into_owned(),
            )
        };
        // Was vor dem Ziel steht, ist je nach Flagge etwas anderes, und nur in
        // einem Fall ein Wirtspfad. Der Deskriptor einer Datei aus dem Speicher
        // ist keiner — eine Nummer in der Spalte „auf diesem Rechner" hielte
        // jemand für einen Pfad —, und das Ziel eines Verweises liegt in der
        // Sandbox und bekommt deshalb ein eigenes Feld (Review Codex,
        // Befund 5).
        let (src, link_target) = match mode {
            v1::MountMode::Masked => (String::new(), String::new()),
            v1::MountMode::Symlink => (String::new(), first),
            _ => (first, String::new()),
        };
        let dst_path = PathBuf::from(&dst);
        let origin = if prepared.adapter_files.contains(&dst_path) {
            v1::ValueOrigin::Adapter
        } else if session_dsts.contains(&dst_path) {
            v1::ValueOrigin::Session
        } else if user_dsts.contains(&dst_path) {
            v1::ValueOrigin::User
        } else {
            v1::ValueOrigin::Profile
        };
        mounts.push(v1::Mount {
            dst,
            src,
            mode: mode.into(),
            origin: origin.into(),
            link_target,
        });
        index += arity + 1;
    }
    mounts
}

/// Die Umgebung, die `--setenv` setzt, alphabetisch, zurückgehaltene ohne Wert.
/// Der `PATH` der Sandbox, so wie der Bildschirm ihn zeigen darf.
///
/// Dieselbe Quelle wie die Umgebungstabelle ([`env_of`]) und dieselbe Regel:
/// Ein Wert, den die Tabelle zurückhält, steht auch in keinem Befund. Setzt
/// niemand `PATH`, gilt der Rückfall der C-Bibliothek, und der Satz sagt das.
fn sandbox_path_of(prepared: &Prepared) -> String {
    let path = prepared
        .profile
        .effective_env(&prepared.session, Some(0))
        .into_iter()
        .find(|(key, _)| key == "PATH");
    match path {
        None => format!(
            "{} (no PATH is set, the C library falls back to this)",
            humanitl_sandbox::agent::DEFAULT_SANDBOX_PATH
        ),
        Some((_, value)) if prepared.shows("PATH") => value,
        Some(_) => {
            format!("{WITHHELD_PLACEHOLDER} (set by hand in sandbox.env or a profile of your own)")
        }
    }
}

fn env_of(prepared: &Prepared) -> Vec<v1::EnvVar> {
    prepared
        .profile
        .effective_env(&prepared.session, Some(0))
        .into_iter()
        .map(|(key, value)| {
            let origin = prepared.origin_of(&key);
            let withheld = !prepared.shows(&key);
            v1::EnvVar {
                key,
                value: if withheld { String::new() } else { value },
                origin: origin.into(),
                withheld,
            }
        })
        .collect()
}

/// Ein Profilname ist ein Name, kein Pfad.
///
/// Ohne diese Prüfung machte `format!("{name}.toml")` aus `/tmp/evil` den Pfad
/// `/tmp/evil.toml` — `Path::join` ersetzt die Basis, sobald das Angehängte
/// absolut ist —, und `..` liefe aus dem Suchpfad heraus. Wer den Namen setzt,
/// bestimmt Einhängungen und Umgebung der Sandbox; geprüft wird deshalb an der
/// Stelle, die aus dem Namen einen Pfad macht, und nicht erst dort, wo ein
/// Mensch tippt. Dieselbe Regel wie für die Profile der Sitzung
/// (`humanitl_config::profile::check_name`).
///
/// # Errors
///
/// `CONFIG_003` mit dem beanstandeten Namen.
fn check_profile_name(name: &str) -> Result<(), Diagnostic> {
    humanitl_config::profile::check_name(name, "the Sandbox request")
}

/// Der Befund für einen Start, während schon eine Sitzung läuft.
///
/// Der Daemon führt genau eine; eine zweite bekäme eine Sandbox, die einem
/// anderen Projektverzeichnis gehört. Weder ein Befehl zum Anhängen noch einer
/// zum Beheben steht im Text: `attach` gibt es nicht, und es gibt auch keinen
/// Befehl, der eine fremde Sitzung beendet. Wer sie gestartet hat, beendet sie
/// dort (HUM-067, Nicht-Ziel).
/// Der Befund, wenn es kein Terminal gibt, an das man sich hängen könnte.
///
/// Derselbe Code wie bei einer `Sandbox`-Anfrage an einen Daemon ohne
/// Sandbox: Die Frage ist dieselbe — „diese Fähigkeit gibt es hier gerade
/// nicht" —, und ein eigener Code dafür wäre eine Unterscheidung ohne
/// Unterschied.
fn no_terminal(why: String) -> Diagnostic {
    Diagnostic::builder(codes::IPC_006, Severity::Error)
        .why(why)
        .build()
}

fn already_running(facts: &Facts) -> Diagnostic {
    let id = facts.id.as_deref().unwrap_or("unknown");
    Diagnostic::builder(codes::CLI_005, Severity::Blocking)
        .why(format!(
            "a session is already running in this daemon (sandbox {id}); it keeps its own \
             project directory, and a second one would not get it. End it where it was started, \
             or watch it in the app."
        ))
        .build()
}

/// Dieser Daemon endet gerade und startet nichts mehr (HUM-142).
///
/// `IPC_006` und nicht `CLI_005`: Es steht keine Sitzung im Weg, sondern
/// dieser Daemon kann es nicht mehr — eine Aussage ueber ihn und nicht ueber
/// die Anfrage. Eine Sandbox, die nach dem Abschied anfinge, bekaeme keinen
/// geordneten mehr.
fn leaving_daemon() -> Diagnostic {
    Diagnostic::builder(codes::IPC_006, Severity::Blocking)
        .why(
            "this daemon is shutting down and starts no new sandbox; a session that began now \
             would not get an orderly end. Start it again once the daemon runs."
                .to_owned(),
        )
        .build()
}

/// Ein Feld der Leitung als Wunsch: leer heißt „kein Wunsch".
fn non_empty(text: &str) -> Option<String> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// Der Sprachmodell-Endpunkt als `host:port`, wie ihn `humanitl.internal`
/// zeigt.
///
/// Ohne `llm.endpoint` und bei einer Adresse ohne Host gibt es nichts zu
/// zeigen; `/` schreibt dann `llm=none`, statt einen Endpunkt zu erfinden.
fn llm_authority(config: &Config) -> Option<String> {
    let endpoint = config.llm.endpoint.as_ref()?;
    let host = endpoint.host_str()?;
    Some(match endpoint.port_or_known_default() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_owned(),
    })
}

/// Beendet diese Sandbox mit Frist und sagt, wenn es dazu Gewalt gebraucht hat.
///
/// **Der eine Weg hinaus.** `Sandbox(Stop)` und der Abschied des Daemons gehen
/// beide hier hindurch, damit es keinen zweiten Weg gibt, auf dem die Frist
/// fehlt (HUM-142): `SIGTERM`, nach [`KILL_GRACE`] `SIGKILL`, obere Schranke
/// zweimal [`KILL_GRACE`]. Der einzige Weg ohne Gnadenfrist bleibt
/// [`Inner::kill_and_fail`], und der hat seinen Grund (eine rote Garantie).
///
/// Bleibt der Exit-Status aus, wird **gefragt statt behauptet**: Erst
/// [`SandboxHandle::process_alive`] entscheidet, ob hier ein Prozess
/// überlebt hat oder ob nur sein Status ausblieb (siehe [`Termination::Stuck`]).
///
/// Blockierend: `terminate` wartet auf das Einsammeln durch den Faden, der die
/// Sandbox gestartet hat.
fn farewell(handle: &SandboxHandle) -> Option<Diagnostic> {
    let started = Instant::now();
    let ended = handle.terminate(KILL_GRACE);
    // `u64` und nicht `u128`: `tracing` kennt keinen `u128` und schriebe die
    // Zahl als Zeichenkette ins JSON, während jeder Nachbar eine Zahl ist.
    // Dieselbe Umrechnung wie in `humanitl_proxy::llm_probe`.
    let took = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let alive = (!ended.ended()).then(|| handle.process_alive()).flatten();
    log_farewell(handle.id, handle.pid, ended, took, alive);
    farewell_finding(handle.id, handle.pid, ended, took, alive)
}

/// Die eine Zeile, die jeder Abschied im Protokoll hinterlässt.
///
/// Sie steht im Protokoll des Daemons auch dann, wenn niemand mehr am
/// Ereignisstrom hängt — und genau dann ist sie die einzige Spur. Ohne
/// Exit-Status bekommt sie die andere Stufe und den anderen Satz: Beendet ist
/// die Sandbox dann nicht nachweislich, und das darf keine Zeile behaupten.
///
/// `took` ist `u64` und nicht `u128`, weil `tracing` keinen `u128` kennt und
/// die Zahl sonst als Zeichenkette im JSON stünde, während jeder Nachbar eine
/// Zahl ist. Der Test dazu liest die erzeugte Zeile und nicht den Typ.
fn log_farewell(sandbox: SandboxId, pid: u32, ended: Termination, took: u64, alive: Option<bool>) {
    if ended.ended() {
        tracing::info!(
            sandbox = %sandbox,
            pid = pid,
            outcome = %ended,
            took_ms = took,
            "sandbox ended"
        );
    } else {
        tracing::warn!(
            sandbox = %sandbox,
            pid = pid,
            outcome = %ended,
            took_ms = took,
            alive = ?alive,
            "sandbox without an exit status after SIGKILL"
        );
    }
}

/// Der Befund zu einem Abschied, ohne die Sandbox dazu.
///
/// Steht für sich, damit die Zuordnung von Ausgang zu Code, Stufe und Text
/// geprüft werden kann, ohne eine Sandbox zu starten: Die Verzweigung ist die
/// Aussage, nicht das Beenden.
///
/// `alive` ist, was [`SandboxHandle::process_alive`] nach einem ausbleibenden
/// Exit-Status gesagt hat: `Some(true)` ein Prozess, der noch da ist,
/// `Some(false)` einer, der weg oder ein Zombie ist, `None` keine Auskunft.
fn farewell_finding(
    sandbox: SandboxId,
    pid: u32,
    ended: Termination,
    took: u64,
    alive: Option<bool>,
) -> Option<Diagnostic> {
    if !ended.escalated() {
        return None;
    }
    if ended.ended() {
        return Some(
            Diagnostic::builder(codes::SANDBOX_029, Severity::Warning)
                .why(format!(
                    "sandbox {sandbox} (pid {pid}) did not answer SIGTERM within {} s, so it was \
                     killed; the farewell took {took} ms in total. An agent that catches the \
                     signal gets that deadline and not a second longer",
                    KILL_GRACE.as_secs()
                ))
                .build(),
        );
    }
    // Kein Exit-Status. Ob das an einem überlebenden Prozess liegt oder an
    // einem Leser, der noch an einem Deskriptor hängt, entscheidet die Frage
    // nach dem Prozess — nicht die ausgebliebene Antwort.
    let survived = alive == Some(true);
    let severity = if survived {
        Severity::Blocking
    } else {
        Severity::Warning
    };
    let state = match alive {
        Some(true) => "and it is still there",
        Some(false) => "but it is gone; only its status never arrived",
        None => "and whether it is still there could not be read from /proc",
    };
    let finding = Diagnostic::builder(codes::SANDBOX_029, severity).why(format!(
        "sandbox {sandbox} (pid {pid}) reported no exit status {took} ms after SIGTERM and \
         SIGKILL, {state}"
    ));
    // Ein Fix nur, wenn es etwas zu sehen gibt. `kill -9` wäre keiner: Genau
    // das hat `terminate` schon geschickt, und einen Prozess, den es nicht
    // beendet hat, beendet ein zweites auch nicht.
    Some(if survived {
        finding
            .fix(FixAction::CopyCommand(format!("ps -o stat= -p {pid}")))
            .build()
    } else {
        finding.build()
    })
}

/// Der Exit-Code eines Prozesses: sein eigener, oder `128 + Signal`.
///
/// Über 255 gibt es keinen; `ExitStatus::code` liefert nie mehr, und ein
/// Signal wird nach POSIX-Sitte auf `128 + n` abgebildet. Dieselbe Zuordnung
/// wie in `humanitl sandbox run`.
fn exit_code_of(status: std::process::ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt as _;

    if let Some(code) = status.code() {
        return code;
    }
    status.signal().map_or(1, |signal| 128 + signal)
}

/// Ein Stück gefilterte Ausgabe als Ereignis.
fn output_event(stream: v1::OutputStream, data: Vec<u8>) -> v1::SandboxEvent {
    v1::SandboxEvent {
        event: Some(v1::sandbox_event::Event::Output(
            v1::sandbox_event::OutputChunk {
                stream: stream.into(),
                data,
            },
        )),
    }
}

/// Der Shim auf dem Host: neben dem Daemon, sonst in einer Installation.
fn shim_path() -> PathBuf {
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        candidates.push(dir.join(SHIM_BINARY));
        // Und ein Verzeichnis darüber. Ein Cargo-Testbinary liegt in
        // `target/<profil>/deps/`, der Shim daneben in `target/<profil>/`;
        // ohne diesen Kandidaten findet ein Integrationstest den Shim nie und
        // könnte den Start nur gegen einen erfundenen prüfen.
        if let Some(up) = dir.parent() {
            candidates.push(up.join(SHIM_BINARY));
        }
    }
    candidates.extend(SHIM_DIRS.iter().map(|dir| Path::new(dir).join(SHIM_BINARY)));
    candidates
        .iter()
        .find(|path| is_executable(path))
        .cloned()
        .or_else(|| candidates.first().cloned())
        .unwrap_or_else(|| Path::new(SHIM_DIRS[0]).join(SHIM_BINARY))
}

/// Ob dieser Pfad eine Datei ist, die sich ausführen lässt.
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;

    std::fs::metadata(path)
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

/// Die Kandidaten für `profiles/sandbox` im Baum über dem Binary.
fn tree_dirs() -> impl Iterator<Item = PathBuf> {
    std::env::current_exe().ok().into_iter().flat_map(|exe| {
        exe.ancestors()
            .skip(1)
            .take(TREE_DEPTH)
            .map(|dir| dir.join(PROFILE_SUBDIR))
            .collect::<Vec<PathBuf>>()
    })
}

/// Ein Schnappschuss als Ereignis.
/// Runs `check` without a source first and, only when it refuses, again with
/// the source `from` finds (HUM-293). Finding the source of a configured
/// folder reads the configuration once more; a folder that passes needs none.
///
/// # Errors
///
/// The refusal of `check` with the source in it.
fn with_source<T>(
    check: impl Fn(&WorkDirSource) -> Result<T, Diagnostic>,
    from: impl FnOnce() -> WorkDirSource,
) -> Result<T, Diagnostic> {
    match check(&WorkDirSource::Request) {
        Ok(passed) => Ok(passed),
        Err(_) => check(&from()),
    }
}

/// Profile, project directory and mode a `Plan` chooses, staged before it is
/// committed to `pending` (HUM-277). The directory is never committed; it
/// belongs to this one call (HUM-293).
#[derive(Debug, Clone, Default)]
struct PlanChoice {
    profile: Option<String>,
    work_dir: Option<PathBuf>,
    /// Where `work_dir` came from (HUM-293); set exactly when it is.
    work_dir_source: Option<WorkDirSource>,
    work_mode: Option<WorkMode>,
}

/// How often a `Plan` is built before it answers with what it has
/// ([`Inner::consistent`], HUM-277).
const SNAPSHOT_ATTEMPTS: usize = 3;

/// Whether a start that holds the claim reads the configuration afresh
/// (HUM-277).
///
/// Yes unless an agent still runs, which the claim already rules out. A
/// handle that is still held after its agent exited is a finished session
/// this start replaces; checking a new project directory against that
/// session's configuration would refuse what `SetConfig` just allowed.
const fn start_may_refresh(facts: &Facts) -> bool {
    !facts.agent_running
}

/// Die Antwort auf `SandboxRequest.retire_if_idle` (HUM-284).
/// Der geöffnete Projektordner einer Wegwerf-Sandbox und seine Grundlage,
/// beide vor dem Start genommen (HUM-350).
#[cfg(target_os = "linux")]
struct Pinned {
    lower: std::os::fd::OwnedFd,
    baseline: humanitl_sandbox::worktree::TreeSnapshot,
}

/// Wo der geöffnete Projektordner liegt.
#[cfg(target_os = "linux")]
fn pinned_path(lower: &std::os::fd::OwnedFd) -> Result<PathBuf, Diagnostic> {
    use std::os::fd::AsRawFd as _;

    std::fs::read_link(format!("/proc/self/fd/{}", lower.as_raw_fd())).map_err(|err| {
        Diagnostic::builder(codes::SANDBOX_030, Severity::Blocking)
            .why(format!(
                "where the opened project folder lies cannot be read ({err}); the disposable \
                 sandbox needs it to mount exactly that folder"
            ))
            .build()
    })
}

/// Der Projektordner liegt nicht mehr dort, wo er geprüft wurde.
#[cfg(target_os = "linux")]
fn moved_work_dir(approved: &std::path::Path, now: &std::path::Path) -> Diagnostic {
    Diagnostic::builder(codes::SANDBOX_006, Severity::Blocking)
        .why(format!(
            "the project folder was checked at {}, but once opened it lies at {}; a symbolic \
             link in its path leads elsewhere now, so it is not mounted",
            approved.display(),
            now.display()
        ))
        .fix(humanitl_sandbox::choose_folder_fix())
        .build()
}

/// Der Projektordner, den der Daemon vor dem Start geöffnet hält: sein Ort
/// und sein Inode, damit das Backend genau ihn einhängt (HUM-350).
#[derive(Debug, Clone, Copy)]
struct PinnedWork<'a> {
    path: &'a std::path::Path,
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    dev: u64,
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    ino: u64,
}

/// Der Plan, den ein Start mit sich trägt.
fn plan_of(start: &v1::sandbox_request::Start) -> v1::sandbox_request::Plan {
    v1::sandbox_request::Plan {
        profile: start.profile.clone(),
        work_dir: start.work_dir.clone(),
        work_mode: start.work_mode.clone(),
        work_dir_source: start.work_dir_source.clone(),
    }
}

fn retire_event(retired: bool) -> v1::SandboxEvent {
    v1::SandboxEvent {
        event: Some(v1::sandbox_event::Event::Retire(
            v1::sandbox_event::Retire { retired },
        )),
    }
}

fn status_event(status: v1::sandbox_event::Status) -> v1::SandboxEvent {
    v1::SandboxEvent {
        event: Some(v1::sandbox_event::Event::Status(status)),
    }
}

/// Eine Protokollzeile als Ereignis.
fn log_event(line: String) -> v1::SandboxEvent {
    v1::SandboxEvent {
        event: Some(v1::sandbox_event::Event::Log(v1::sandbox_event::LogLine {
            at: Some(crate::convert::timestamp(SystemTime::now())),
            line,
        })),
    }
}

/// Ein Befund als Ereignis.
fn diagnostic_event(diagnostic: &Diagnostic) -> v1::SandboxEvent {
    v1::SandboxEvent {
        event: Some(v1::sandbox_event::Event::Diagnostic(
            crate::convert::diagnostic_to_proto(diagnostic),
        )),
    }
}

/// Ein Ergebnis einer Isolationsprüfung als Ereignis.
fn check_event(result: &CheckResult) -> v1::SandboxEvent {
    v1::SandboxEvent {
        event: Some(v1::sandbox_event::Event::Check(
            crate::convert::check_result_to_proto(result),
        )),
    }
}

/// Der Befund, der einen Start beendet, oder `None`, wenn alle drei Garantien
/// belegt sind.
///
/// **Kein Ergebnis ist nicht dasselbe wie ein gutes Ergebnis.** Eine leere
/// Liste heißt, dass nichts gemessen wurde — keine laufende Sandbox, kein
/// Bericht —, und das ist `SANDBOX_013` und kein Durchlauf.
///
/// Ein rotes Ergebnis ohne eigenen `Diagnostic` bleibt rot.
/// [`SandboxBackend::isolation_check`] legt zu jedem roten Ergebnis einen an;
/// ein zweites Backend (ADR-005 nennt Docker als späteren Kandidaten) muss das
/// nicht tun, und diese Stelle darf aus einem fehlenden Befund keinen
/// bestandenen Start machen. Geprüft wird deshalb `passed`, nie das
/// Vorhandensein des Befunds.
fn first_failure(results: &[CheckResult]) -> Option<Diagnostic> {
    let missing: Vec<&str> = IsolationCheck::ALL
        .iter()
        .filter(|check| !results.iter().any(|result| result.check == **check))
        .map(|check| check.as_str())
        .collect();
    if !missing.is_empty() {
        return Some(
            Diagnostic::builder(codes::SANDBOX_013, Severity::Blocking)
                .why(format!(
                    "the sandbox reported no result for {}; nothing about {} is proven",
                    missing.join(", "),
                    if missing.len() == IsolationCheck::ALL.len() {
                        "its isolation"
                    } else {
                        "those guarantees"
                    }
                ))
                .build(),
        );
    }
    results.iter().find(|result| !result.passed).map(|result| {
        result.diagnostic.clone().unwrap_or_else(|| {
            Diagnostic::builder(code_of(result.check), Severity::Blocking)
                .why(format!(
                    "{}: {} (the backend reported no diagnostic of its own)",
                    result.check.as_str(),
                    result.evidence
                ))
                .build()
        })
    })
}

/// Der Diagnostic-Code einer Garantie, als Boden für ein Backend, das keinen
/// eigenen Befund mitschickt.
///
/// Die Zuordnung steht in `daemon/crates/sandbox/src/bwrap.rs` und in
/// `codes.rs`; hier steht sie nur, damit ein rotes Ergebnis auch ohne Befund
/// den Code trägt, der zu seiner Garantie gehört (CONVENTIONS 4.11).
const fn code_of(check: IsolationCheck) -> humanitl_core::DiagnosticCode {
    match check {
        IsolationCheck::NoNetworkInterface => codes::SANDBOX_014,
        IsolationCheck::SingleSocket => codes::SANDBOX_015,
        IsolationCheck::SeccompActive => codes::SANDBOX_016,
    }
}

/// Der Befund, wenn der Arbeitsfaden selbst nicht durchkam.
fn joined_failed(error: &tokio::task::JoinError) -> Diagnostic {
    Diagnostic::builder(codes::SANDBOX_012, Severity::Blocking)
        .why(format!("the thread preparing the sandbox failed: {error}"))
        .build()
}

/// `host:port` von `llm.endpoint`, wie `Authority` es schreibt: das Ziel, an
/// das der Zugangsschlüssel gehört (HUM-140).
fn endpoint_authority(config: &Config) -> Option<String> {
    config
        .llm
        .endpoint_authority()
        .map(|authority| authority.to_string())
}

/// Ob eine Vorbereitung den Zugangsschlüssel des Modells braucht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyNeed {
    /// Eine Momentaufnahme oder ein Plan: nichts startet, also wird kein
    /// Geheimnis gelesen.
    Plan,
    /// Ein Start: Der Agent bekommt den Schlüssel.
    Start,
}

/// Ein vergifteter Mutex ist kein Grund, den Zustand zu verlieren: Alles
/// darin sind Werte, kein halb geschriebener Zustand.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// Ein Argument der Testzeile.
    fn as_os(text: &str) -> OsString {
        OsString::from(text)
    }

    #[test]
    fn only_the_values_on_the_allow_list_are_shown() {
        for visible in [
            "HTTP_PROXY",
            "http_proxy",
            "SSL_CERT_FILE",
            "PATH",
            "HOME",
            "HUMANITL_SESSION",
            "OPENCODE_PERMISSION",
            "OPENCODE_EXPERIMENTAL_BASH_DEFAULT_TIMEOUT_MS",
        ] {
            assert!(
                shows_env_value(visible, v1::ValueOrigin::Profile),
                "{visible} from the bundled profile is evidence"
            );
            // Derselbe Name, von Hand geschrieben: der Wert bleibt zurück.
            // Ein erlaubter Name sagt nichts über den Wert darunter.
            assert!(
                !shows_env_value(visible, v1::ValueOrigin::User),
                "{visible} written by a person is not evidence"
            );
        }
        // Die Namen, an denen eine Liste verdächtiger Endungen scheitert. Kein
        // einziger endet auf `_TOKEN`, `_KEY`, `_SECRET` oder `PASSWORD`, und
        // alle tragen ein Geheimnis. Sie sind der Grund für die Richtung
        // dieser Regel und dürfen nie sichtbar werden.
        for withheld in [
            "AWS_ACCESS_KEY_ID",
            "OPENAI_API_KEY_BASE",
            "DATABASE_URL",
            "AUTHORIZATION",
            "GH_PAT",
            "APIKEY",
            "TOKEN",
            "KEY",
            "SECRET",
            "GITHUB_TOKEN",
        ] {
            assert!(
                !shows_env_value(withheld, v1::ValueOrigin::Profile),
                "{withheld} must stay withheld"
            );
        }
    }

    /// Die Mutationsprobe zur Richtung der Regel.
    ///
    /// Sie schlägt fehl, sobald jemand die Erlaubnisliste wieder in eine Liste
    /// verdächtiger Endungen dreht: Die vier Namen hier tragen ein Geheimnis
    /// und passen auf keine der Endungen, die eine solche Liste hätte.
    #[test]
    fn a_deny_list_of_suffixes_would_miss_these_and_the_allow_list_does_not() {
        const SUFFIXES: &[&str] = &["_TOKEN", "_KEY", "_SECRET", "PASSWORD"];
        for name in [
            "AWS_ACCESS_KEY_ID",
            "DATABASE_URL",
            "GH_PAT",
            "AUTHORIZATION",
        ] {
            let upper = name.to_ascii_uppercase();
            assert!(
                !SUFFIXES.iter().any(|suffix| upper.ends_with(suffix)),
                "{name} would slip through a suffix rule"
            );
            assert!(
                !shows_env_value(name, v1::ValueOrigin::Profile),
                "{name} is withheld anyway"
            );
        }
    }

    #[test]
    fn the_mount_table_reads_the_command_line_and_stops_at_the_double_dash() {
        let argv: Vec<OsString> = [
            "bwrap",
            "--unshare-all",
            "--ro-bind",
            "/usr",
            "/usr",
            "--tmpfs",
            "/tmp",
            "--proc",
            "/proc",
            "--dev",
            "/dev",
            "--symlink",
            "usr/bin",
            "/bin",
            "--bind",
            "/home/u/proj",
            "/work",
            "--ro-bind-data",
            "15",
            "/work/.envrc",
            "--clearenv",
            "--setenv",
            "HOME",
            "/home/agent",
            "--",
            "/run/humanitl/humanitl-shim",
            "--proxy-port",
            "3128",
            "--",
            "--ro-bind",
            "/etc/shadow",
            "/etc/shadow",
        ]
        .into_iter()
        .map(as_os)
        .collect();
        let mounts = mounts_of(&argv, &preview_prepared());
        let seen: Vec<(&str, i32)> = mounts
            .iter()
            .map(|mount| (mount.dst.as_str(), mount.mode))
            .collect();
        assert_eq!(
            seen,
            vec![
                ("/usr", v1::MountMode::Ro as i32),
                ("/tmp", v1::MountMode::Tmpfs as i32),
                ("/proc", v1::MountMode::Proc as i32),
                ("/dev", v1::MountMode::Dev as i32),
                ("/bin", v1::MountMode::Symlink as i32),
                ("/work", v1::MountMode::Rw as i32),
                ("/work/.envrc", v1::MountMode::Masked as i32),
            ]
        );
        // Der Deskriptor einer maskierten Datei ist kein Host-Pfad.
        let masked = mounts.last().expect("the mask is the last entry");
        assert_eq!(masked.src, "");
        // Und nichts hinter dem ersten `--` zählt.
        assert!(mounts.iter().all(|mount| mount.dst != "/etc/shadow"));
    }

    /// Ein Dienst über einem Wegwerf-XDG mit dem mitgelieferten Profil.
    fn service_for(home: &tempfile::TempDir, adapter: &str) -> SandboxService {
        service_with(home, adapter, None, SandboxPorts::none())
    }

    /// Der Endpunkt, zu dem der Schlüssel in [`service_with`] gehört.
    const KEYED_ENDPOINT: &str = "https://api.example.com/v1";

    /// Wie [`service_for`], mit einem Verweis auf den Zugangsschlüssel und
    /// eigenen Anschlüssen (HUM-140).
    fn service_with(
        home: &tempfile::TempDir,
        adapter: &str,
        api_key_ref: Option<&str>,
        ports: SandboxPorts,
    ) -> SandboxService {
        service_configured(home, adapter, api_key_ref, ports, |_| {})
    }

    /// Wie [`service_with`], und `tweak` darf die Konfiguration danach noch
    /// ändern.
    fn service_configured(
        home: &tempfile::TempDir,
        adapter: &str,
        api_key_ref: Option<&str>,
        ports: SandboxPorts,
        tweak: impl FnOnce(&mut humanitl_config::Config),
    ) -> SandboxService {
        let config_home = home.path().join("config");
        let profile_dir = config_home.join("humanitl/profiles/sandbox");
        std::fs::create_dir_all(&profile_dir).unwrap();
        let shipped = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../profiles/sandbox");
        std::fs::copy(shipped.join("default.toml"), profile_dir.join("test.toml")).unwrap();

        let env = humanitl_config::Env::from_pairs([
            ("HOME", home.path().display().to_string()),
            ("XDG_CONFIG_HOME", config_home.display().to_string()),
            (
                "XDG_DATA_HOME",
                home.path().join("data").display().to_string(),
            ),
            (
                "XDG_RUNTIME_DIR",
                home.path().join("run").display().to_string(),
            ),
        ]);
        let mut config = humanitl_config::Config::default();
        "test".clone_into(&mut config.sandbox.profile);
        adapter.clone_into(&mut config.agent.adapter);
        config.sandbox.work_dir = Some(home.path().join("project"));
        config.llm.api_key_ref = api_key_ref.map(ToOwned::to_owned);
        // Ein Schlüssel gehört zu einem Endpunkt: An dessen Host und Port ist
        // er in `Authorization` kein Fund (HUM-140).
        if api_key_ref.is_some() {
            config.llm.endpoint = Some(url::Url::parse(KEYED_ENDPOINT).unwrap());
        }
        std::fs::create_dir_all(home.path().join("project")).unwrap();
        tweak(&mut config);

        SandboxService::new(
            crate::session::SessionResolver::for_config(humanitl_config::Paths::new(env), config),
            SessionId::new(),
            ports,
        )
    }

    /// Eine Attrappe von `secret-tool`, die ihre Einträge unter `dir/keyring`
    /// ablegt. Der Schlüsselbund des Menschen bleibt unberührt.
    fn fake_secret_tool(dir: &Path) -> PathBuf {
        let store = dir.join("keyring");
        std::fs::create_dir_all(&store).unwrap();
        let script = dir.join("secret-tool");
        let body = format!(
            "#!/bin/sh\nfor last; do :; done\ncase \"$1\" in\n  store) cat > '{0}'/\"$last\" ;;\n  \
             lookup) [ -f '{0}'/\"$last\" ] || exit 1; cat '{0}'/\"$last\" ;;\n  clear) rm -f \
             '{0}'/\"$last\" ;;\nesac\n",
            store.display()
        );
        crate::test_support::write_executable(&script, &body);
        script
    }

    /// Ein `tracing`-Schreiber in einen geteilten Puffer.
    #[derive(Clone, Default)]
    struct LogBuffer(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for LogBuffer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            lock(&self.0).extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// The start gets the stored key only together with the endpoint of the
    /// same saved server (HUM-289, review A): an endpoint that is not the
    /// active server's leaves the agent without a key.
    #[test]
    fn the_start_withholds_a_key_that_belongs_to_another_endpoint() {
        const KEY: &str = "sk-sandbox-0123456789abcdef";
        let home = tempfile::TempDir::new().unwrap();
        let paths = humanitl_config::Paths::new(humanitl_config::Env::from_pairs([(
            "XDG_DATA_HOME",
            home.path().join("data").display().to_string(),
        )]));
        let keys =
            KeyStore::for_paths(&paths).with_secret_tool(Some(fake_secret_tool(home.path())));
        let stored = keys
            .store(&humanitl_config::secret::ApiKey::new(KEY).unwrap())
            .unwrap();
        let key_ref = stored.key_ref.to_string();
        let service = service_with(
            &home,
            "opencode",
            Some(&key_ref),
            SandboxPorts::none().with_keys(keys),
        );
        let mut config = (*service.inner.config()).clone();
        config.llm.servers.insert(
            "cloud".to_owned(),
            humanitl_config::LlmServer {
                endpoint: Some(KEYED_ENDPOINT.to_owned()),
                models: Vec::new(),
                api_key_ref: Some(key_ref.clone()),
            },
        );
        config.llm.active = Some("cloud".to_owned());
        assert!(matches!(
            service.inner.agent_key(&config, KeyNeed::Start).unwrap(),
            AgentKey::Key(_)
        ));
        config.llm.endpoint = Some(url::Url::parse("http://192.168.1.20:11434/").unwrap());
        assert!(
            matches!(
                service.inner.agent_key(&config, KeyNeed::Start).unwrap(),
                AgentKey::None
            ),
            "the key went to the agent beside an endpoint of another origin"
        );
    }

    /// Der Zugangsschlüssel kommt aus der Attrappe des Speichers in eine
    /// Datei der Sandbox, auf die `opencode.json` verweist, und sonst
    /// nirgendwohin: nicht in `argv`, nicht in die Umgebung, nicht ins Log und
    /// nicht im Klartext in die Aufzeichnung (HUM-140).
    #[tokio::test]
    async fn the_key_reaches_a_sandbox_file_and_nothing_else() {
        const KEY: &str = "sk-sandbox-0123456789abcdef";
        let home = tempfile::TempDir::new().unwrap();
        let paths = humanitl_config::Paths::new(humanitl_config::Env::from_pairs([(
            "XDG_DATA_HOME",
            home.path().join("data").display().to_string(),
        )]));
        let keys =
            KeyStore::for_paths(&paths).with_secret_tool(Some(fake_secret_tool(home.path())));
        let stored = keys
            .store(&humanitl_config::secret::ApiKey::new(KEY).unwrap())
            .unwrap();
        let recorder = Recorder::open(
            &home.path().join("data/humanitl.db"),
            &home.path().join("data/blobs"),
            humanitl_recorder::RecorderSettings::default(),
        )
        .unwrap();
        let key_ref = stored.key_ref.to_string();
        let service = service_with(
            &home,
            "opencode",
            Some(&key_ref),
            SandboxPorts::none()
                .with_keys(keys)
                .with_recorder(recorder.clone()),
        );

        let log = LogBuffer::default();
        let writer = log.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_writer(move || writer.clone())
            .finish();
        let prepared = tracing::subscriber::with_default(subscriber, || {
            tracing::info!("preparing the sandbox");
            service
                .inner
                .prepare_keyed(&v1::sandbox_request::Plan::default(), &[], KeyNeed::Start)
        })
        .expect("the session is prepared");

        // In der Sandbox: eine Datei mit dem Schlüssel, und die Konfiguration
        // des Agenten verweist nur auf sie.
        let secret = prepared
            .session
            .files
            .iter()
            .find(|file| file.secret)
            .expect("the key travels as a file");
        assert_eq!(secret.dst, Path::new(opencode::API_KEY_DST));
        assert_eq!(secret.content, KEY.as_bytes());
        let config = prepared
            .session
            .files
            .iter()
            .find(|file| file.dst == Path::new(opencode::CONFIG_DST))
            .expect("opencode.json is there");
        let config = String::from_utf8_lossy(&config.content);
        assert!(
            config.contains("{file:/etc/humanitl/opencode/llm-api-key}"),
            "{config}"
        );
        assert!(!config.contains(KEY), "{config}");

        // Nicht in `argv`, nicht in der Umgebung, nicht in einer Debug-Form.
        let argv = shell_line(&prepared.argv());
        assert!(!argv.contains(KEY), "the key reached argv: {argv}");
        for (name, value) in &prepared.session.session_env {
            assert!(
                !value.contains(KEY),
                "the key reached the environment as {name}"
            );
        }
        let debug = format!("{:?}", prepared.session);
        assert!(!debug.contains(KEY), "the key shows in a Debug form");

        // Nicht im Log.
        let logged = String::from_utf8_lossy(&lock(&log.0)).into_owned();
        assert!(
            logged.contains("preparing the sandbox"),
            "the log was captured"
        );
        assert!(!logged.contains(KEY), "the key reached the log: {logged}");

        // Nicht im Klartext in der Aufzeichnung: Der Start hat ihn in die
        // Masken gelegt, bevor der Agent ihn sieht.
        assert_eq!(
            recorded_authorization(&recorder, KEY).await,
            vec![("authorization".to_owned(), "Bearer ••••".to_owned())]
        );

        // Kein Fund ist er nur an seinem eigenen Ziel: Host und Port von
        // `llm.endpoint`, mit dem Standardport des Schemas.
        let hash = humanitl_core::http::sha256(KEY.as_bytes());
        let masks = recorder.masks();
        assert!(masks.expected_at(&hash, "api.example.com:443"));
        assert!(!masks.expected_at(&hash, "evil.example:443"));
    }

    /// With `agent.adapter = "pi"` the key takes the same way: a file of the
    /// sandbox, read by the command `models.json` names, and nothing else
    /// (HUM-274, HUM-140).
    #[test]
    fn the_key_reaches_pi_through_a_sandbox_file_only() {
        const KEY: &str = "sk-pi-sandbox-0123456789abcdef";
        let home = tempfile::TempDir::new().unwrap();
        let paths = humanitl_config::Paths::new(humanitl_config::Env::from_pairs([(
            "XDG_DATA_HOME",
            home.path().join("data").display().to_string(),
        )]));
        let keys =
            KeyStore::for_paths(&paths).with_secret_tool(Some(fake_secret_tool(home.path())));
        let stored = keys
            .store(&humanitl_config::secret::ApiKey::new(KEY).unwrap())
            .unwrap();
        let key_ref = stored.key_ref.to_string();
        let service = service_with(
            &home,
            "pi",
            Some(&key_ref),
            SandboxPorts::none().with_keys(keys),
        );
        let prepared = service
            .inner
            .prepare_keyed(&v1::sandbox_request::Plan::default(), &[], KeyNeed::Start)
            .expect("the session is prepared");

        let secret = prepared
            .session
            .files
            .iter()
            .find(|file| file.secret)
            .expect("the key travels as a file");
        assert_eq!(secret.dst, Path::new(pi::API_KEY_DST));
        assert_eq!(secret.content, KEY.as_bytes());
        let models = prepared
            .session
            .files
            .iter()
            .find(|file| file.dst.ends_with(".pi/agent/models.json"))
            .expect("models.json is there");
        let models = String::from_utf8_lossy(&models.content);
        assert!(
            models.contains("\"apiKey\": \"!cat \\\"$HUMANITL_PI_KEY_FILE\\\"\""),
            "{models}"
        );
        assert!(
            models.contains("\"baseUrl\": \"https://api.example.com/v1\""),
            "{models}"
        );
        assert!(!models.contains(KEY), "{models}");

        let argv = shell_line(&prepared.argv());
        assert!(!argv.contains(KEY), "the key reached argv: {argv}");
        assert!(
            argv.contains("PI_OFFLINE"),
            "the adapter contributed: {argv}"
        );
        for (name, value) in &prepared.session.session_env {
            assert!(
                !value.contains(KEY),
                "the key reached the environment as {name}"
            );
        }
    }

    /// Zeichnet eine Anfrage mit `Authorization: Bearer <key>` auf und liest
    /// ihre Kopfzeilen aus der Aufzeichnung zurück.
    async fn recorded_authorization(recorder: &Recorder, key: &str) -> Vec<(String, String)> {
        recorder.start_session(&humanitl_recorder::SessionMeta {
            id: SessionId::new(),
            started_at: SystemTime::now(),
            sandbox_profile: "test".to_owned(),
            llm_endpoint: None,
            work_dir: "/work".to_owned(),
            agent: "opencode".to_owned(),
        });
        let flow = humanitl_core::FlowId::new();
        let request = humanitl_core::HttpRequest::new(
            humanitl_core::Method::POST,
            humanitl_core::Scheme::Http,
            humanitl_core::Authority::new(
                humanitl_core::HostName::parse("192.168.1.20").unwrap(),
                11434,
            ),
            "/v1/chat/completions",
        );
        recorder.apply(&humanitl_core::FlowEvent::Received {
            flow_id: flow,
            at: SystemTime::now(),
            request: Box::new(request),
        });
        let mut headers = humanitl_core::HeaderMap::new();
        headers.insert(
            "authorization",
            humanitl_core::http::HeaderValue::from_str(&format!("Bearer {key}")).unwrap(),
        );
        recorder
            .store_message(
                flow,
                humanitl_recorder::Dir::Request,
                &headers,
                bytes::Bytes::new(),
            )
            .await
            .unwrap();
        recorder.flush().await;
        let detail = recorder.get_flow(flow).await.unwrap().unwrap();
        detail
            .messages
            .first()
            .expect("the request is recorded")
            .headers
            .clone()
    }

    /// Ein Plan liest den Schlüssel nie: Die Datei steht da, damit Plan und
    /// Start dieselbe Kommandozeile haben, aber leer (HUM-140). Ein Start ohne
    /// Schlüssel im Speicher bricht mit `LLM_009` samt Vorschlag ab.
    #[test]
    fn a_plan_announces_the_key_file_but_reads_no_key() {
        let home = tempfile::TempDir::new().unwrap();
        // Ein Schlüsselbund ohne Eintrag: Läse der Plan, bräche er mit
        // `LLM_009`.
        let paths = humanitl_config::Paths::new(humanitl_config::Env::from_pairs([(
            "XDG_DATA_HOME",
            home.path().join("data").display().to_string(),
        )]));
        let keys =
            KeyStore::for_paths(&paths).with_secret_tool(Some(fake_secret_tool(home.path())));
        let service = service_with(
            &home,
            "opencode",
            Some("secret-service:humanitl/llm"),
            SandboxPorts::none().with_keys(keys),
        );
        let prepared = service
            .inner
            .prepare_with_command(&v1::sandbox_request::Plan::default(), &[])
            .expect("a plan needs no key");
        let secret = prepared
            .session
            .files
            .iter()
            .find(|file| file.secret)
            .expect("the plan shows the key file");
        assert!(secret.content.is_empty());

        let refused = service
            .inner
            .prepare_keyed(&v1::sandbox_request::Plan::default(), &[], KeyNeed::Start)
            .expect_err("a start without the key stops");
        assert_eq!(refused.code, codes::LLM_009);
        assert_eq!(
            refused.fix,
            Some(FixAction::CopyCommand("humanitl llm key set".to_owned()))
        );
    }

    /// A caller's command starts without the key when the store does not
    /// give it out, and says so with `LLM_009` as a warning; the agent the
    /// daemon starts itself still stops (HUM-280).
    #[test]
    fn a_shell_starts_without_a_key_the_store_does_not_give() {
        let home = tempfile::TempDir::new().unwrap();
        let paths = humanitl_config::Paths::new(humanitl_config::Env::from_pairs([(
            "XDG_DATA_HOME",
            home.path().join("data").display().to_string(),
        )]));
        let keys =
            KeyStore::for_paths(&paths).with_secret_tool(Some(fake_secret_tool(home.path())));
        let service = service_with(
            &home,
            "opencode",
            Some("secret-service:humanitl/llm"),
            SandboxPorts::none().with_keys(keys),
        );
        let shell = service
            .inner
            .prepare_keyed(
                &v1::sandbox_request::Plan::default(),
                &[OsString::from("bash")],
                KeyNeed::Start,
            )
            .expect("a shell starts without the key");
        assert!(
            !shell.session.files.iter().any(|file| file.secret),
            "no key file without a key"
        );
        let warning = shell
            .warnings
            .iter()
            .find(|found| found.code == codes::LLM_009)
            .expect("the start says it has no key");
        assert_eq!(warning.severity, Severity::Warning);
        assert!(warning.why.contains("without the key"), "{}", warning.why);

        let refused = service
            .inner
            .prepare_keyed(&v1::sandbox_request::Plan::default(), &[], KeyNeed::Start)
            .expect_err("the agent itself still stops");
        assert_eq!(refused.code, codes::LLM_009);
    }

    /// HUM-262: Die Vorgabe ist nie ein Ordner, den der Start ablehnt.
    ///
    /// Der Daemon läuft als Nutzerdienst im Heimatverzeichnis. Das enthält die
    /// Konfiguration von Humanitl, also gibt es dann keine Vorgabe, und das
    /// Setup verlangt eine Auswahl. Ein Arbeitsverzeichnis, das die Regel
    /// annimmt, bleibt die Vorgabe wie bisher.
    #[test]
    fn the_default_folder_is_never_one_the_start_refuses() {
        let home = tempfile::TempDir::new().unwrap();
        let service = service_for(&home, "opencode");
        let inner = &service.inner;
        for refused in [home.path(), Path::new("/"), &home.path().join("config")] {
            assert_eq!(
                inner.default_work_dir(Some(refused)),
                PathBuf::new(),
                "{}",
                refused.display()
            );
        }
        assert_eq!(inner.default_work_dir(None), PathBuf::new());
        assert_eq!(
            inner.default_work_dir(Some(&home.path().join("missing"))),
            PathBuf::new(),
            "a folder that is not there is no default"
        );
        let project = home.path().join("project");
        assert_eq!(inner.default_work_dir(Some(&project)), project);

        // Und ohne `sandbox.work_dir` landet genau diese Vorgabe im Plan: Ein
        // Daemon, der im Heimatverzeichnis läuft, nennt keinen Ordner, einer
        // in einem Projektordner diesen.
        let mut config = (*inner.config()).clone();
        config.sandbox.work_dir = None;
        let plan = v1::sandbox_request::Plan::default();
        assert_eq!(
            inner.work_dir_with_cwd(&config, &inner.pending_choice(), &plan, Some(home.path())),
            PathBuf::new()
        );
        assert_eq!(
            inner.work_dir_with_cwd(&config, &inner.pending_choice(), &plan, Some(&project)),
            project
        );
    }

    /// Die Namen der Umgebung, die eine Sitzung mit diesem Kommando bekäme.
    fn env_keys_for(service: &SandboxService, command: &[&str]) -> Vec<String> {
        let command: Vec<OsString> = command.iter().map(|arg| OsString::from(*arg)).collect();
        let prepared = service
            .inner
            .prepare_with_command(&v1::sandbox_request::Plan::default(), &command)
            .expect("the session is prepared");
        prepared
            .session
            .session_env
            .iter()
            .map(|(key, _)| key.clone())
            .collect()
    }

    /// Der genannte Agent bekommt seine Umgebung, ein anderes Kommando nicht
    /// (HUM-135).
    ///
    /// Bis zum 2026-09-07 hing der Beitrag daran, ob **irgendein** Kommando
    /// genannt wurde; `humanitl run -- opencode` startete den Agenten damit
    /// ohne die Konfiguration, die dieses Produkt für ihn baut.
    #[test]
    fn a_named_agent_still_gets_its_environment() {
        let home = tempfile::TempDir::new().unwrap();
        let service = service_for(&home, "opencode");

        for named in [
            vec!["opencode"],
            vec!["opencode", "run", "say hello"],
            vec!["/usr/local/bin/opencode", "--version"],
            vec![],
        ] {
            let keys = env_keys_for(&service, &named);
            for wanted in [
                "OPENCODE_CONFIG",
                "OPENCODE_MODELS_PATH",
                "OPENCODE_DISABLE_MODELS_FETCH",
            ] {
                assert!(
                    keys.iter().any(|key| key == wanted),
                    "{named:?} is the agent, so it needs {wanted}: {keys:?}"
                );
            }
        }

        // Since HUM-280 a command of its own gets the same contribution: a
        // shell in which the person starts the agent by hand needs the same
        // configuration as the agent the daemon starts.
        for other in [vec!["bash"], vec!["sh", "-c", "opencode"]] {
            let keys = env_keys_for(&service, &other);
            assert!(
                keys.iter().any(|key| key == "OPENCODE_CONFIG"),
                "{other:?} is a shell for the agent: {keys:?}"
            );
        }
    }

    /// A native executable in a per-user bin directory, as the ELF check
    /// wants it (HUM-278).
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    fn native_program(dir: &Path, name: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::create_dir_all(dir).unwrap();
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        let path = dir.join(name);
        std::fs::copy("/bin/true", &path)
            .or_else(|_| std::fs::copy("/usr/bin/true", &path))
            .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    /// A session started with a caller's command binds the configured agent
    /// by HUM-278's rules and puts its directory first on `PATH`; the
    /// caller's own command is not bound and not rewritten (HUM-280).
    // Plant mit dem Linux-Sandbox-Backend (HUM-312).
    #[cfg(target_os = "linux")]
    #[test]
    fn a_callers_command_gets_the_configured_agent_on_its_path() {
        let home = tempfile::TempDir::new().unwrap();
        // A name no system has under `/usr`, so the sandbox cannot start it
        // as it is and the search goes to the per-user directories.
        let agent = native_program(&home.path().join(".opencode/bin"), "hum280-agent");
        let elsewhere = native_program(&home.path().join("tools"), "hum280-tool");
        let service = service_configured(&home, "opencode", None, SandboxPorts::none(), |c| {
            c.agent.command = Some(vec!["hum280-agent".to_owned()]);
        });

        for caller in [
            vec![OsString::from("bash")],
            vec![elsewhere.clone().into_os_string(), OsString::from("-v")],
        ] {
            let prepared = service
                .inner
                .prepare_with_command(&v1::sandbox_request::Plan::default(), &caller)
                .expect("the session is prepared");
            let program = prepared
                .session
                .agent_program
                .clone()
                .unwrap_or_else(|| panic!("{caller:?}: the configured agent is bound"));
            assert_eq!(program.dst, Path::new("/run/humanitl/agent/hum280-agent"));
            assert_eq!(program.host, agent.canonicalize().unwrap());
            assert_eq!(
                prepared.session.command, caller,
                "the caller's command is neither bound nor rewritten"
            );

            let env = prepared.profile.effective_env(&prepared.session, None);
            let path = env
                .iter()
                .find(|(key, _)| key == "PATH")
                .map(|(_, value)| value.clone())
                .expect("the sandbox has a PATH");
            assert!(
                path.starts_with("/run/humanitl/agent:"),
                "{caller:?}: the bind is first on PATH: {path}"
            );
            assert!(
                env.iter().any(|(key, _)| key == "OPENCODE_CONFIG"),
                "{caller:?}: the adapter's configuration is there"
            );

            let argv = prepared.argv();
            let elsewhere = elsewhere.as_os_str();
            assert!(
                !argv
                    .windows(2)
                    .any(|w| w[0].to_string_lossy().contains("bind") && w[1] == elsewhere),
                "{caller:?}: the caller's file is never bound: {argv:?}"
            );
            assert!(
                argv.iter()
                    .any(|arg| arg == humanitl_sandbox::SHIM_CONTROLLING_TERMINAL),
                "the screen's line asks for the controlling terminal, as the start does"
            );
        }
    }

    /// Ein Adapter, den es nicht gibt, bleibt ein Befund und wird nicht still
    /// zur Shell.
    #[test]
    fn an_unknown_adapter_is_still_config_003() {
        let home = tempfile::TempDir::new().unwrap();
        let service = service_for(&home, "there-is-no-such-adapter");
        let diagnostic = service
            .inner
            .prepare_with_command(&v1::sandbox_request::Plan::default(), &[])
            .expect_err("an adapter that does not exist stops the start");
        assert_eq!(diagnostic.code.as_str(), "CONFIG_003");
    }

    /// Der Plan eines Dienstes mit `tweak` und einem Sandbox-Backend der Fassung
    /// `version`, ohne ein echtes zu fragen (HUM-357).
    #[cfg(target_os = "linux")]
    fn plan_with(
        home: &tempfile::TempDir,
        version: humanitl_sandbox::Version,
        tweak: impl FnOnce(&mut humanitl_config::Config),
    ) -> (SandboxService, Prepared) {
        let service = service_configured(home, "opencode", None, SandboxPorts::none(), tweak);
        let mut prepared = service
            .inner
            .prepare_with_command(&v1::sandbox_request::Plan::default(), &[])
            .expect("the plan");
        let program = humanitl_sandbox::PlatformBackend::fallback(service.inner.paths.clone())
            .program()
            .to_owned();
        prepared.backend = humanitl_sandbox::PlatformBackend::unchecked(
            program,
            version,
            service.inner.paths.clone(),
        );
        (service, prepared)
    }

    /// Ob das Laufzeitverzeichnis der Testdienste auf tmpfs liegt.
    #[cfg(target_os = "linux")]
    fn runtime_on_tmpfs(home: &tempfile::TempDir) -> bool {
        humanitl_sandbox::disposable::require_tmpfs(&home.path().join("run")).is_ok()
    }

    /// Die Vorgabe `auto` löst der Daemon gegen das System auf (HUM-357):
    /// eine Wegwerf-Sandbox mit `--overlay`, schreibbarem Projekt und
    /// Laufzeitverzeichnis auf tmpfs, sonst `direct` mit `SANDBOX_033`, das
    /// den Grund nennt; ein altes bubblewrap und ein nur lesbares Projekt
    /// nennen je ihren eigenen.
    #[cfg(target_os = "linux")]
    #[test]
    fn the_default_mode_resolves_against_the_system() {
        use humanitl_sandbox::Version;

        let home = tempfile::TempDir::new().unwrap();
        let (service, prepared) = plan_with(&home, Version(0, 11, 0), |_| {});
        assert_eq!(prepared.wanted_mode, SandboxMode::Auto);
        let choice = service
            .inner
            .mode_choice(SandboxMode::Auto, &prepared)
            .unwrap();
        if runtime_on_tmpfs(&home) {
            assert_eq!(choice.mode, SandboxMode::Disposable, "{choice:?}");
            assert!(choice.notice.is_none());
        } else {
            assert_eq!(choice.mode, SandboxMode::Direct);
            let notice = choice.notice.expect("the notice");
            assert_eq!(notice.code.as_str(), "SANDBOX_033");
            assert!(notice.why.contains("not on tmpfs"), "{}", notice.why);
        }

        let (service, prepared) = plan_with(&home, Version(0, 9, 0), |_| {});
        let choice = service
            .inner
            .mode_choice(SandboxMode::Auto, &prepared)
            .unwrap();
        assert_eq!(choice.mode, SandboxMode::Direct);
        let notice = choice.notice.expect("old bubblewrap");
        assert_eq!(notice.code.as_str(), "SANDBOX_033");
        assert_eq!(notice.severity, Severity::Warning);
        assert!(notice.why.contains("bubblewrap 0.9.0"), "{}", notice.why);

        let (service, prepared) = plan_with(&home, Version(0, 11, 0), |config| {
            config.sandbox.work_mode = WorkMode::Ro;
        });
        let choice = service
            .inner
            .mode_choice(SandboxMode::Auto, &prepared)
            .unwrap();
        assert_eq!(choice.mode, SandboxMode::Direct);
        let notice = choice.notice.expect("read-only project");
        assert_eq!(notice.code.as_str(), "SANDBOX_033");
        assert!(notice.why.contains("read-only"), "{}", notice.why);
    }

    /// Ein ausdrückliches `disposable`, aus der Konfiguration oder dem Start,
    /// lehnt ab, wo es nicht geht (HUM-357); `direct` fragt nichts.
    #[cfg(target_os = "linux")]
    #[test]
    fn an_explicit_disposable_still_refuses_where_it_cannot_run() {
        use humanitl_sandbox::Version;

        let home = tempfile::TempDir::new().unwrap();
        let (service, prepared) = plan_with(&home, Version(0, 9, 0), |config| {
            config.sandbox.mode = SandboxMode::Disposable;
        });
        let refused = service
            .inner
            .mode_choice(prepared.wanted_mode, &prepared)
            .expect_err("explicit disposable on bubblewrap 0.9");
        assert_eq!(refused.code.as_str(), "SANDBOX_030");
        let direct = service
            .inner
            .mode_choice(SandboxMode::Direct, &prepared)
            .unwrap();
        assert_eq!(direct.mode, SandboxMode::Direct);
        assert!(direct.notice.is_none());
    }

    /// Die Momentaufnahme nennt den Modus des nächsten Starts und den Grund
    /// einer Abweichung (HUM-357): `direct` ohne Hinweis, wenn es gewählt
    /// ist; ein ausdrückliches `disposable` vor einem nur lesbaren Projekt
    /// bleibt `disposable` und nennt das `SANDBOX_030`, mit dem der Start
    /// ablehnen wird.
    #[test]
    fn the_snapshot_names_the_resolved_mode_and_its_reason() {
        let home = tempfile::TempDir::new().unwrap();
        let service = service_configured(&home, "opencode", None, SandboxPorts::none(), |config| {
            config.sandbox.mode = SandboxMode::Direct;
        });
        let status = service
            .inner
            .snapshot(&v1::sandbox_request::Plan::default())
            .unwrap();
        assert_eq!(status.sandbox_mode, "direct");
        assert!(status.sandbox_mode_notice.is_none());

        let service = service_configured(&home, "opencode", None, SandboxPorts::none(), |config| {
            config.sandbox.mode = SandboxMode::Disposable;
            config.sandbox.work_mode = WorkMode::Ro;
        });
        let status = service
            .inner
            .snapshot(&v1::sandbox_request::Plan::default())
            .unwrap();
        assert_eq!(status.sandbox_mode, "disposable");
        let notice = status.sandbox_mode_notice.expect("the refusal to come");
        assert_eq!(notice.code, "SANDBOX_030");

        // Die Vorgabe: was immer das System hier kann, nie `auto`, und ein
        // `direct` nur mit Grund.
        let service = service_configured(&home, "opencode", None, SandboxPorts::none(), |_| {});
        let status = service
            .inner
            .snapshot(&v1::sandbox_request::Plan::default())
            .unwrap();
        assert!(
            matches!(status.sandbox_mode.as_str(), "direct" | "disposable"),
            "{}",
            status.sandbox_mode
        );
        if cfg!(target_os = "linux") && status.sandbox_mode == "direct" {
            assert_eq!(
                status.sandbox_mode_notice.expect("the reason").code,
                "SANDBOX_033"
            );
        }
    }

    /// Ein Profil ohne Datei, nur für die Tabellenfunktionen.
    fn preview_prepared() -> Prepared {
        let profile = SandboxProfile::parse("version = 1\nname = \"test\"\n", Path::new("<test>"))
            .expect("the minimal profile parses");
        let session = SessionContext {
            session: SessionId::nil(),
            work_src: PathBuf::from("/home/u/proj"),
            work_mode: WorkMode::Rw,
            proxy_socket_src: PathBuf::from("/run/user/1000/humanitl/proxy/proxy.sock"),
            ca_cert_src: PathBuf::from("/home/u/.local/share/humanitl/ca/ca.crt"),
            ca_bundle_src: PathBuf::from("/home/u/.local/share/humanitl/ca/ca-bundle.crt"),
            shim_src: PathBuf::from("/usr/lib/humanitl/humanitl-shim"),
            session_env: Vec::new(),
            command: Vec::new(),
            files: Vec::new(),
            agent_program: None,
        };
        let paths =
            humanitl_config::Paths::new(humanitl_config::Env::from_pairs([("HOME", "/home/u")]));
        Prepared {
            profile,
            session,
            backend: PlatformBackend::fallback(paths),
            env_origin: BTreeMap::new(),
            adapter_files: BTreeSet::new(),
            warnings: Vec::new(),
            overlay_preview: None,
            wanted_mode: SandboxMode::Direct,
            plan_mode: humanitl_sandbox::ModeChoice {
                mode: SandboxMode::Direct,
                notice: None,
            },
        }
    }

    /// Der `PATH` im Befund `AGENT_005` folgt derselben Regel wie die
    /// Umgebungstabelle (HUM-137): gezeigt, wenn das mitgelieferte Profil ihn
    /// setzt, zurückgehalten, wenn ein Mensch ihn geschrieben hat, und der
    /// Rückfall der C-Bibliothek, wenn niemand ihn setzt.
    #[test]
    fn the_sandbox_path_in_a_finding_obeys_the_table() {
        let mut prepared = preview_prepared();
        assert!(
            sandbox_path_of(&prepared).starts_with(humanitl_sandbox::agent::DEFAULT_SANDBOX_PATH),
            "{}",
            sandbox_path_of(&prepared)
        );

        prepared
            .profile
            .env
            .insert("PATH".to_owned(), "/opt/agent/bin:/usr/bin".to_owned());
        prepared
            .env_origin
            .insert("PATH".to_owned(), v1::ValueOrigin::Profile);
        assert_eq!(sandbox_path_of(&prepared), "/opt/agent/bin:/usr/bin");

        prepared
            .env_origin
            .insert("PATH".to_owned(), v1::ValueOrigin::User);
        let withheld = sandbox_path_of(&prepared);
        assert!(withheld.starts_with(WITHHELD_PLACEHOLDER), "{withheld}");
        assert!(!withheld.contains("/opt/agent/bin"), "{withheld}");
    }

    #[test]
    fn the_work_mount_and_the_proxy_socket_come_from_the_session() {
        let prepared = preview_prepared();
        let argv: Vec<OsString> = [
            "bwrap",
            "--ro-bind",
            "/usr",
            "/usr",
            "--bind",
            "/home/u/proj",
            "/work",
            "--ro-bind",
            "/run/user/1000/humanitl/proxy/proxy.sock",
            "/run/humanitl/proxy.sock",
        ]
        .into_iter()
        .map(as_os)
        .collect();
        let mounts = mounts_of(&argv, &prepared);
        let origin = |dst: &str| {
            mounts
                .iter()
                .find(|mount| mount.dst == dst)
                .map(|mount| mount.origin)
                .expect("the mount is in the table")
        };
        assert_eq!(origin("/usr"), v1::ValueOrigin::Profile as i32);
        assert_eq!(origin("/work"), v1::ValueOrigin::Session as i32);
        assert_eq!(
            origin("/run/humanitl/proxy.sock"),
            v1::ValueOrigin::Session as i32
        );
    }

    /// Ein Ergebnis der Isolationsprüfung, wie es vom Backend käme.
    fn result(check: IsolationCheck, passed: bool) -> CheckResult {
        CheckResult {
            check,
            passed,
            evidence: format!("{} evidence", check.as_str()),
            diagnostic: (!passed).then(|| {
                Diagnostic::builder(code_of(check), Severity::Blocking)
                    .why(format!("{} failed", check.as_str()))
                    .build()
            }),
        }
    }

    /// Alle drei Garantien belegt: der Start darf weitergehen.
    #[test]
    fn three_green_checks_let_the_start_through() {
        let green: Vec<CheckResult> = IsolationCheck::ALL
            .iter()
            .map(|&check| result(check, true))
            .collect();
        assert!(first_failure(&green).is_none());
    }

    /// **Kein Ergebnis ist nicht dasselbe wie ein gutes Ergebnis.**
    ///
    /// Die Mutationsprobe dazu: Wer `first_failure` so schriebe, dass es über
    /// eine leere Liste iteriert und `None` findet, liefe hier auf. Eine
    /// Sandbox ohne Bericht startete dann mit drei unbelegten Garantien.
    #[test]
    fn a_report_that_never_arrived_is_never_a_pass() {
        let diagnostic = first_failure(&[]).expect("no measurement is not a pass");
        assert_eq!(diagnostic.code, codes::SANDBOX_013);
        assert_eq!(diagnostic.severity, Severity::Blocking);
        assert!(!diagnostic.why.is_empty(), "a finding without a why");
    }

    /// Zwei grüne Ergebnisse sind kein Durchlauf.
    ///
    /// Der Start prüft drei Garantien, nicht „so viele, wie ankamen". Ein
    /// abgerissener Strom oder ein Backend, das eine Zeile verschluckt, darf
    /// nicht als bestandener Start enden — gezählt wird gegen
    /// [`IsolationCheck::ALL`], nie gegen die Länge der Liste.
    #[test]
    fn two_green_results_out_of_three_are_not_a_pass() {
        for missing in IsolationCheck::ALL {
            let results: Vec<CheckResult> = IsolationCheck::ALL
                .iter()
                .filter(|check| *check != &missing)
                .map(|&check| result(check, true))
                .collect();
            assert_eq!(results.len(), 2);
            let diagnostic = first_failure(&results)
                .unwrap_or_else(|| panic!("{} was never measured", missing.as_str()));
            assert_eq!(diagnostic.code, codes::SANDBOX_013);
            assert!(
                diagnostic.why.contains(missing.as_str()),
                "the finding names the guarantee nobody measured: {}",
                diagnostic.why
            );
        }
    }

    /// Der erste rote Befund ist der, der den Start beendet, und er trägt den
    /// Code seiner Garantie.
    #[test]
    fn the_first_red_check_is_the_reason_the_start_ends() {
        let results = vec![
            result(IsolationCheck::NoNetworkInterface, true),
            result(IsolationCheck::SingleSocket, false),
            result(IsolationCheck::SeccompActive, false),
        ];
        let diagnostic = first_failure(&results).expect("a red check stops the start");
        assert_eq!(diagnostic.code, codes::SANDBOX_015);
    }

    /// Die zweite Mutationsprobe: Rot bleibt rot, auch ohne Befund daneben.
    ///
    /// Wer `passed` gegen `diagnostic.is_some()` tauschte — naheliegend, weil
    /// [`SandboxBackend::isolation_check`] zu jedem roten Ergebnis einen Befund
    /// legt —, ließe ein Backend, das keinen mitschickt, mit einer roten
    /// Garantie durchstarten.
    #[test]
    fn a_red_check_without_a_diagnostic_still_stops_the_start() {
        let results = vec![
            result(IsolationCheck::NoNetworkInterface, true),
            result(IsolationCheck::SingleSocket, true),
            CheckResult {
                check: IsolationCheck::SeccompActive,
                passed: false,
                evidence: "seccomp_applied FAIL: Seccomp:0".to_owned(),
                diagnostic: None,
            },
        ];
        let diagnostic = first_failure(&results).expect("passed decides, not the diagnostic");
        assert_eq!(diagnostic.code, codes::SANDBOX_016);
        assert_eq!(diagnostic.severity, Severity::Blocking);
    }

    /// Jede Garantie hat ihren eigenen Code; zwei Garantien teilen sich keinen.
    #[test]
    fn every_guarantee_has_its_own_code() {
        let codes: Vec<&str> = IsolationCheck::ALL
            .iter()
            .map(|&check| code_of(check).as_str())
            .collect();
        assert_eq!(codes, vec!["SANDBOX_014", "SANDBOX_015", "SANDBOX_016"]);
    }

    /// Jeder Ausgang eines Abschieds bekommt genau den Befund, der zu ihm
    /// gehört (`SANDBOX_029`, HUM-142).
    ///
    /// Geprüft wird die Verzweigung und nicht das Beenden: Ob aus einem
    /// Ausgang ein Befund wird, welche Stufe er trägt und was er behauptet,
    /// hängt an [`farewell_finding`] allein. Ein `SIGTERM`, das reichte, ist
    /// kein Befund; ein `SIGKILL`, das wirkte, ist eine Warnung; und ein
    /// ausbleibender Exit-Status ist nur dann blockierend, wenn der Prozess
    /// wirklich noch da ist.
    #[test]
    fn every_way_out_of_a_farewell_has_its_own_finding() {
        let sandbox = SandboxId::nil();

        assert!(
            farewell_finding(sandbox, 7, Termination::Term, 12, None).is_none(),
            "SIGTERM was enough; there is nothing to report"
        );
        assert!(
            farewell_finding(sandbox, 7, Termination::AlreadyEnded, 0, None).is_none(),
            "nothing was running; there is nothing to report"
        );

        let killed = farewell_finding(sandbox, 7, Termination::Kill, 5_012, None)
            .expect("an agent that had to be killed is a finding");
        assert_eq!(killed.code.as_str(), "SANDBOX_029");
        assert_eq!(killed.severity, Severity::Warning);
        assert!(killed.why.contains("pid 7"), "{}", killed.why);
        assert!(
            killed.why.contains("5012 ms in total"),
            "the finding names the measured time: {}",
            killed.why
        );
        assert!(killed.fix.is_none(), "a killed process needs no command");

        let survived = farewell_finding(sandbox, 7, Termination::Stuck, 10_004, Some(true))
            .expect("a process that outlived SIGKILL is a finding");
        assert_eq!(survived.severity, Severity::Blocking, "{}", survived.why);
        assert!(
            survived.why.contains("no exit status") && survived.why.contains("still there"),
            "the finding says what was measured and what follows from it: {}",
            survived.why
        );
        assert!(
            matches!(
                survived.fix.as_ref(),
                Some(FixAction::CopyCommand(command)) if command == "ps -o stat= -p 7"
            ),
            "the fix asks after the state of the process, it does not repeat the kill: {:?}",
            survived.fix
        );

        // Derselbe Ausgang, aber der Prozess ist weg: Dann ist nur der Status
        // ausgeblieben, und das ist keine blockierende Aussage über einen
        // Prozess, der stehen geblieben wäre.
        let only_the_status = farewell_finding(sandbox, 7, Termination::Stuck, 10_004, Some(false))
            .expect("a missing exit status is still worth saying");
        assert_eq!(
            only_the_status.severity,
            Severity::Warning,
            "{}",
            only_the_status.why
        );
        assert!(
            only_the_status.why.contains("it is gone"),
            "{}",
            only_the_status.why
        );
        assert!(
            !only_the_status.why.contains("still there"),
            "nothing claims a process that is not there: {}",
            only_the_status.why
        );
        assert!(only_the_status.fix.is_none());

        // Und ohne Auskunft aus `/proc` wird weder das eine noch das andere
        // behauptet.
        let unknown = farewell_finding(sandbox, 7, Termination::Stuck, 10_004, None)
            .expect("a missing exit status is still worth saying");
        assert_eq!(unknown.severity, Severity::Warning);
        assert!(unknown.why.contains("could not be read"), "{}", unknown.why);
    }

    /// Ein Protokoll, das sich nach dem Lauf lesen lässt.
    #[derive(Clone, Default)]
    struct Collected(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for Collected {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            lock(&self.0).extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Collected {
        type Writer = Self;

        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    /// Die Zeile, die `run` schreibt, als JSON.
    fn logged_json(run: impl FnOnce()) -> serde_json::Value {
        let sink = Collected::default();
        let subscriber = tracing_subscriber::fmt()
            .json()
            .with_writer(sink.clone())
            .finish();
        tracing::subscriber::with_default(subscriber, run);
        let written = lock(&sink.0).clone();
        let line = String::from_utf8(written).expect("the log is text");
        serde_json::from_str(line.trim()).unwrap_or_else(|error| {
            panic!("the daemon writes one JSON line per event ({error}): {line}")
        })
    }

    /// Die gemessene Zeit steht als **Zahl** im Protokoll (HUM-142).
    ///
    /// `tracing` kennt keinen `u128`: Wer die Millisekunden so übergibt,
    /// bekommt sie als Zeichenkette ins JSON, während jeder Nachbar eine Zahl
    /// ist — `"took_ms":"0"` neben `"pid":4711`. Wer das Protokoll auswertet,
    /// müsste dann genau dieses eine Feld anders lesen als alle anderen.
    /// Geprüft wird deshalb die erzeugte Zeile und nicht der Typ im Quelltext.
    #[test]
    fn the_measured_time_is_a_number_in_the_log() {
        let ended = logged_json(|| {
            log_farewell(SandboxId::nil(), 4_711, Termination::Kill, 5_012, None);
        });
        let fields = &ended["fields"];
        assert_eq!(fields["message"], "sandbox ended", "{ended}");
        assert!(
            fields["took_ms"].is_u64(),
            "took_ms is a number, like its neighbours: {ended}"
        );
        assert_eq!(fields["took_ms"].as_u64(), Some(5_012), "{ended}");
        assert!(fields["pid"].is_u64(), "{ended}");
        assert_eq!(fields["outcome"], "sigkill", "{ended}");

        // Und der andere Zweig behauptet kein Ende, sondern nennt den
        // fehlenden Status.
        let stuck = logged_json(|| {
            log_farewell(
                SandboxId::nil(),
                4_711,
                Termination::Stuck,
                10_004,
                Some(true),
            );
        });
        assert_eq!(
            stuck["fields"]["message"], "sandbox without an exit status after SIGKILL",
            "{stuck}"
        );
        assert_eq!(stuck["level"], "WARN", "{stuck}");
        assert!(stuck["fields"]["took_ms"].is_u64(), "{stuck}");
    }

    /// Ein Faden, der abgestürzt ist, wird zu einem Befund und nicht zu
    /// Schweigen (HUM-142).
    ///
    /// `Inner::stop` schickt ihn seit diesem Issue in den Ereignisstrom, wie
    /// es `start` und `report_exit` längst tun. Geprüft wird hier die Quelle
    /// des Befunds an einem echten `JoinError`; dass der Zweig in `stop` ihn
    /// sendet, steht als Form neben den beiden anderen Stellen.
    #[tokio::test]
    async fn a_thread_that_died_becomes_a_finding() {
        let error = tokio::task::spawn_blocking(|| panic!("simulated panic"))
            .await
            .expect_err("the task panicked");
        assert!(error.is_panic(), "a panic, not a cancellation");

        let finding = joined_failed(&error);

        assert_eq!(finding.code.as_str(), "SANDBOX_012");
        assert_eq!(finding.severity, Severity::Blocking);
        assert!(
            finding.why.contains("thread preparing the sandbox failed"),
            "the finding says which thread and that it failed: {}",
            finding.why
        );
    }

    /// Ein Daemon, dessen Programm ersetzt wurde, geht nur ohne Sitzung
    /// (HUM-284).
    ///
    /// Solange ein Start seinen Anspruch hält, bleibt er und startet weiter;
    /// ist der Anspruch frei, steht der Riegel, und kein späterer Start bekommt
    /// noch einen.
    #[test]
    fn a_replaced_daemon_retires_only_without_a_session() {
        let home = tempfile::tempdir().unwrap();
        let service = service_for(&home, "opencode");

        let claim = service.inner.claim_start().expect("the claim is free");
        assert!(
            !service.retire_if_idle(),
            "a start that holds its claim keeps the daemon"
        );
        assert!(!service.is_leaving(), "and nothing is barred");
        drop(claim);

        assert!(service.retire_if_idle(), "without a session it retires");
        assert!(service.is_leaving());
        assert!(
            service.inner.claim_start().is_none(),
            "after retiring no start gets a claim any more"
        );
        assert!(service.retire_if_idle(), "asking twice changes nothing");
    }

    /// Die Zustände aller Momentaufnahmen einer Antwort auf `Status`.
    async fn status_states(service: &SandboxService) -> Vec<v1::SandboxState> {
        use tokio_stream::StreamExt as _;

        let events: Vec<v1::SandboxEvent> = service
            .stream(v1::SandboxRequest {
                op: Some(v1::sandbox_request::Op::Status(())),
            })
            .collect()
            .await;
        events
            .iter()
            .filter_map(|event| match &event.event {
                Some(v1::sandbox_event::Event::Status(status)) => {
                    v1::SandboxState::try_from(status.state).ok()
                }
                _ => None,
            })
            .collect()
    }

    /// `Status.llm` aus der Antwort auf `Status`, wie sie jeder andere Client
    /// liest.
    async fn status_llm(service: &SandboxService) -> Option<v1::Diagnostic> {
        use tokio_stream::StreamExt as _;

        let events: Vec<v1::SandboxEvent> = service
            .stream(v1::SandboxRequest {
                op: Some(v1::sandbox_request::Op::Status(())),
            })
            .collect()
            .await;
        let mut last = None;
        for event in events {
            if let Some(v1::sandbox_event::Event::Status(status)) = event.event {
                last = Some(status.llm);
            }
        }
        last.expect("the answer carries a snapshot")
    }

    /// Die nächste Momentaufnahme im Strom des Starts.
    async fn next_status(rx: &mut mpsc::Receiver<v1::SandboxEvent>) -> v1::sandbox_event::Status {
        loop {
            let event = tokio::time::timeout(Duration::from_secs(10), rx.recv())
                .await
                .expect("a snapshot within ten seconds")
                .expect("the stream is open");
            if let Some(v1::sandbox_event::Event::Status(status)) = event.event {
                return status;
            }
        }
    }

    /// HUM-251: Scheitert die Durchreiche an der Verbindung, steht `LLM_001`
    /// mit `ChangeSetting` auf `llm.endpoint` am Status, im Strom des Starts
    /// wie in der Momentaufnahme jedes anderen Clients; die nächste Antwort
    /// einer Durchreiche nimmt ihn weg.
    ///
    /// Die Ereignisse gehen durch eine echte Warteschlange und in der Form,
    /// in der der Proxy sie für eine Durchreiche veröffentlicht
    /// (`Decided` mit `DecisionSource::Passthrough`, danach `Failed`
    /// beziehungsweise `ResponseHeaders`), und der Zuhörer ist derselbe, den
    /// `start` vor dem Agenten anmeldet und `attend` laufen lässt.
    ///
    /// Die erste Anfrage scheitert, **bevor** der Zuhörer liest: So sieht es
    /// aus, wenn ein Agent sofort ein totes Modell fragt, während der Dienst
    /// noch die Garantien prüft. Ein Zuhörer, der sich erst beim Lesen
    /// anmeldete, verpasste sie.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_failed_passthrough_sets_llm_001_on_the_status_and_an_answer_removes_it() {
        let home = tempfile::tempdir().unwrap();
        let notices = HeldNotices::detached();
        let service = service_with(
            &home,
            "opencode",
            None,
            SandboxPorts::none().with_notices(notices.clone()),
        );
        assert_eq!(status_llm(&service).await, None, "nothing failed yet");

        let (tx, mut rx) = mpsc::channel(16);
        let plan = v1::sandbox_request::Plan::default();
        let mut watch = service.inner.listen_llm().expect("a queue to listen to");

        let first = humanitl_core::FlowId::new();
        unreachable_passthrough(&notices, first);
        watch.run(&plan, &tx);
        let pushed = next_status(&mut rx).await;
        let llm = pushed.llm.expect("LLM_001 travels in the snapshot");
        assert_eq!(llm.code, "LLM_001");
        let endpoint = service
            .inner
            .config()
            .llm
            .endpoint
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default();
        assert_eq!(
            llm.fix.and_then(|fix| fix.action),
            Some(v1::fix_action::Action::ChangeSetting(
                v1::fix_action::ChangeSetting {
                    key: "llm.endpoint".to_owned(),
                    value: endpoint,
                }
            ))
        );
        assert_eq!(
            status_llm(&service).await.map(|llm| llm.code),
            Some("LLM_001".to_owned()),
            "every other client reads it from the snapshot"
        );

        let second = humanitl_core::FlowId::new();
        notices.publish(passthrough(second));
        notices.publish(FlowEvent::ResponseHeaders {
            flow_id: second,
            at: SystemTime::now(),
            status: 200,
        });
        let pushed = next_status(&mut rx).await;
        assert_eq!(pushed.llm, None, "an answer takes it away");
        assert_eq!(status_llm(&service).await, None);
        watch.finish(&plan, &tx).await;
    }

    /// Eine Freigabe der Durchreiche, wie der Proxy sie veröffentlicht.
    fn passthrough(flow_id: humanitl_core::FlowId) -> FlowEvent {
        FlowEvent::Decided {
            flow_id,
            at: SystemTime::now(),
            decision: humanitl_core::Decision::Allow,
            source: humanitl_core::DecisionSource::Passthrough,
            findings: humanitl_core::DecidedFindings::default(),
        }
    }

    /// Eine Durchreiche, an der niemand die Verbindung annahm.
    fn unreachable_passthrough(notices: &HeldNotices, flow_id: humanitl_core::FlowId) {
        notices.publish(passthrough(flow_id));
        notices.publish(FlowEvent::Failed {
            flow_id,
            at: SystemTime::now(),
            error: humanitl_core::UpstreamError::Connect,
        });
    }

    /// HUM-251: Endet der Agent, solange `LLM_001` steht, geht noch eine
    /// Momentaufnahme ohne ihn in den Strom des Starts; sonst bliebe die
    /// Zeile „LLM“ nach dem Ende rot. Auch jede spätere Momentaufnahme liest
    /// den geleerten Stand.
    #[tokio::test(flavor = "multi_thread")]
    async fn the_end_of_the_agent_sends_a_snapshot_without_llm_001() {
        let home = tempfile::tempdir().unwrap();
        let notices = HeldNotices::detached();
        let service = service_with(
            &home,
            "opencode",
            None,
            SandboxPorts::none().with_notices(notices.clone()),
        );
        let (tx, mut rx) = mpsc::channel(16);
        let plan = v1::sandbox_request::Plan::default();
        let mut watch = service.inner.listen_llm().expect("a queue to listen to");
        watch.run(&plan, &tx);
        unreachable_passthrough(&notices, humanitl_core::FlowId::new());
        assert!(
            next_status(&mut rx).await.llm.is_some(),
            "the finding stands"
        );

        watch.finish(&plan, &tx).await;
        let pushed = tokio::time::timeout(Duration::from_secs(1), next_status(&mut rx))
            .await
            .expect("a snapshot after the end");
        assert_eq!(pushed.llm, None, "the end clears the line");
        assert_eq!(status_llm(&service).await, None);

        // Ohne Befund schuldet das Ende niemandem eine Momentaufnahme.
        let mut quiet = service.inner.listen_llm().expect("a queue to listen to");
        quiet.run(&plan, &tx);
        quiet.finish(&plan, &tx).await;
        assert!(rx.try_recv().is_err(), "nothing to say, nothing sent");
    }

    /// HUM-251: Fällt der Zuhörer ohne `finish` weg (ein früher Ausstieg,
    /// eine abgebrochene Aufgabe), bricht er seine Aufgabe ab und leert den
    /// Stand; ein Ereignis danach setzt nichts mehr.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_dropped_watch_stops_listening_and_clears_the_finding() {
        let home = tempfile::tempdir().unwrap();
        let notices = HeldNotices::detached();
        let service = service_with(
            &home,
            "opencode",
            None,
            SandboxPorts::none().with_notices(notices.clone()),
        );
        let (tx, mut rx) = mpsc::channel(16);
        let plan = v1::sandbox_request::Plan::default();
        let mut watch = service.inner.listen_llm().expect("a queue to listen to");
        watch.run(&plan, &tx);
        unreachable_passthrough(&notices, humanitl_core::FlowId::new());
        assert!(next_status(&mut rx).await.llm.is_some());

        drop(watch);
        assert_eq!(
            service.inner.llm_reach.current(),
            None,
            "dropping clears it"
        );
        // Die abgebrochene Aufgabe lässt ihren Sender los, sobald der
        // Abbruch greift; danach erreicht kein Ereignis mehr den Stand.
        tokio::time::sleep(Duration::from_millis(100)).await;
        unreachable_passthrough(&notices, humanitl_core::FlowId::new());
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(
            service.inner.llm_reach.current(),
            None,
            "nobody listens any more"
        );
        assert!(rx.try_recv().is_err(), "and nothing is sent");
    }

    /// Ein Start, der seinen Anspruch hält und noch kein Handle hat, ist für
    /// jeden anderen Client `starting`, nie `stopped` (HUM-284).
    ///
    /// In genau diesem Fenster fragte `postinst` vor einem Neustart; las es
    /// `stopped`, startete es den Daemon mitten in einem Start neu.
    #[tokio::test]
    async fn a_held_claim_is_starting_for_every_other_client() {
        let home = tempfile::tempdir().unwrap();
        let service = service_for(&home, "opencode");

        let claim = service.inner.claim_start().expect("the claim is free");
        let held = status_states(&service).await;
        assert!(!held.is_empty(), "the answer carries a snapshot");
        assert!(
            held.iter()
                .all(|state| *state == v1::SandboxState::Starting),
            "a claimed start is starting: {held:?}"
        );
        drop(claim);

        let free = status_states(&service).await;
        assert!(
            free.iter()
                .all(|state| *state != v1::SandboxState::Starting),
            "without the claim nothing starts: {free:?}"
        );
    }

    /// `SandboxRequest.retire_if_idle` antwortet mit genau einem `Retire`:
    /// falsch, solange ein Start seinen Anspruch hält, wahr ohne ihn, und
    /// dann endet das Warten auf [`SandboxService::retired`] (HUM-284).
    #[tokio::test]
    async fn retire_if_idle_answers_over_the_wire_and_releases_the_daemon() {
        use tokio_stream::StreamExt as _;

        async fn ask(service: &SandboxService) -> Vec<bool> {
            let events: Vec<v1::SandboxEvent> = service
                .stream(v1::SandboxRequest {
                    op: Some(v1::sandbox_request::Op::RetireIfIdle(())),
                })
                .collect()
                .await;
            events
                .iter()
                .map(|event| match &event.event {
                    Some(v1::sandbox_event::Event::Retire(retire)) => retire.retired,
                    other => panic!("only a Retire answers, not {other:?}"),
                })
                .collect()
        }

        let home = tempfile::tempdir().unwrap();
        let service = service_for(&home, "opencode");
        let claim = service.inner.claim_start().expect("the claim is free");
        assert_eq!(ask(&service).await, vec![false], "a claimed start keeps it");
        let waiting = tokio::time::timeout(Duration::from_millis(50), service.retired()).await;
        assert!(waiting.is_err(), "a busy daemon is not released");
        drop(claim);

        assert_eq!(ask(&service).await, vec![true]);
        tokio::time::timeout(Duration::from_secs(5), service.retired())
            .await
            .expect("the retired daemon is released to its farewell");
    }

    /// Ein Daemon, der sich verabschiedet, startet keine neue Sandbox mehr
    /// (HUM-142).
    ///
    /// Der Riegel schließt das Fenster zwischen dem Abschied und dem Ende des
    /// Prozesses: Ein `Start`, der dort anklopfte, bekäme keinen geordneten
    /// Abschied mehr, sondern nur noch das `SIGKILL` am Ende des Daemons.
    #[test]
    fn a_daemon_that_is_leaving_starts_no_new_sandbox() {
        let home = tempfile::tempdir().unwrap();
        let service = service_for(&home, "opencode");

        let claim = service.inner.claim_start();
        assert!(claim.is_some(), "a fresh service takes the next start");
        drop(claim);

        assert!(
            service.shutdown().is_none(),
            "nothing was running, so there is nothing to report"
        );

        assert!(
            service.inner.is_leaving(),
            "the farewell has begun and says so"
        );
        assert!(
            service.inner.claim_start().is_none(),
            "after the farewell no start gets a claim any more"
        );
        let refused = leaving_daemon();
        assert_eq!(refused.code.as_str(), "IPC_006");
        assert!(
            refused.why.contains("shutting down"),
            "the caller is told why, and it is not CLI_005: {}",
            refused.why
        );
    }

    /// Ohne laufende Sandbox ist nichts gemessen — und nichts gemessen wird
    /// nicht als Ergebnis gesendet.
    ///
    /// Drei graue `CheckResult` wären auf der Leitung nicht von drei
    /// gemessenen zu unterscheiden; die Oberfläche bekommt stattdessen den
    /// Zustand, der sagt, dass nichts läuft (CONVENTIONS 4.13).
    #[tokio::test]
    async fn isolation_check_without_a_running_sandbox_reports_no_result() {
        use tokio_stream::StreamExt as _;

        let paths =
            humanitl_config::Paths::new(humanitl_config::Env::from_pairs([("HOME", "/home/u")]));
        let service = SandboxService::new(
            crate::session::SessionResolver::for_config(paths, Config::default()),
            SessionId::nil(),
            SandboxPorts::none(),
        );
        let events: Vec<v1::SandboxEvent> = service
            .stream(v1::SandboxRequest {
                op: Some(v1::sandbox_request::Op::IsolationCheck(())),
            })
            .collect()
            .await;
        assert!(
            !events
                .iter()
                .any(|event| matches!(event.event, Some(v1::sandbox_event::Event::Check(_)))),
            "no sandbox ran, so nothing was measured: {events:?}"
        );
        assert!(
            events
                .iter()
                .any(|event| matches!(event.event, Some(v1::sandbox_event::Event::Status(_)))),
            "the answer says what the state is: {events:?}"
        );
    }

    /// A service over a live resolver: the configuration comes from
    /// `config.toml` in `home`, as in the daemon, and not from a hand-made
    /// [`Config`] (HUM-277).
    fn live_service(
        home: &tempfile::TempDir,
        ports: SandboxPorts,
    ) -> (SandboxService, humanitl_config::Paths) {
        let config_home = home.path().join("config");
        let profile_dir = config_home.join("humanitl/profiles/sandbox");
        std::fs::create_dir_all(&profile_dir).unwrap();
        let shipped = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../profiles/sandbox");
        std::fs::copy(shipped.join("default.toml"), profile_dir.join("test.toml")).unwrap();
        let project = home.path().join("project");
        std::fs::create_dir_all(&project).unwrap();
        let env = humanitl_config::Env::from_pairs([
            ("HOME", home.path().display().to_string()),
            ("XDG_CONFIG_HOME", config_home.display().to_string()),
            (
                "XDG_DATA_HOME",
                home.path().join("data").display().to_string(),
            ),
            (
                "XDG_RUNTIME_DIR",
                home.path().join("run").display().to_string(),
            ),
        ]);
        let paths = humanitl_config::Paths::new(env);
        std::fs::write(
            paths.config_path(),
            format!(
                "[sandbox]\nprofile = \"test\"\nwork_dir = {:?}\n\n[agent]\nadapter = \"opencode\"\n",
                project.display().to_string()
            ),
        )
        .unwrap();
        // What the daemon read at its start: no endpoint, no key.
        let at_start = humanitl_config::resolve(
            &humanitl_config::ProfileSelection::default(),
            None,
            paths.env(),
            &[],
        )
        .unwrap();
        assert!(at_start.config.llm.endpoint.is_none());
        let service = SandboxService::new(
            crate::session::SessionResolver::new(paths.clone(), at_start),
            SessionId::new(),
            ports,
        );
        (service, paths)
    }

    /// The `llm_endpoint` of the snapshot a `Plan` returns.
    async fn planned_endpoint(service: &SandboxService) -> String {
        use tokio_stream::StreamExt as _;

        let events: Vec<v1::SandboxEvent> = service
            .stream(v1::SandboxRequest {
                op: Some(v1::sandbox_request::Op::Plan(
                    v1::sandbox_request::Plan::default(),
                )),
            })
            .collect()
            .await;
        events
            .into_iter()
            .find_map(|event| match event.event {
                Some(v1::sandbox_event::Event::Status(status)) => Some(status.llm_endpoint),
                _ => None,
            })
            .expect("a Plan answers with a status")
    }

    /// The owner's report of 2026-10-22 (HUM-277): endpoint and key saved
    /// through `SetConfig` and `LlmKey` reach the next `Plan` and the next
    /// start of the running daemon, without a restart. The start hands the
    /// new key to the sandbox file and expects its mask at the new endpoint.
    #[tokio::test]
    async fn saved_settings_reach_the_next_plan_and_start_without_a_restart() {
        const KEY: &str = "sk-saved-later-0123456789abcdef";
        let home = tempfile::TempDir::new().unwrap();
        let recorder = Recorder::open(
            &home.path().join("data/humanitl.db"),
            &home.path().join("data/blobs"),
            humanitl_recorder::RecorderSettings::default(),
        )
        .unwrap();
        let keys_for = |paths: &humanitl_config::Paths| {
            KeyStore::for_paths(paths).with_secret_tool(Some(fake_secret_tool(home.path())))
        };
        let probe_paths = humanitl_config::Paths::new(humanitl_config::Env::from_pairs([(
            "XDG_DATA_HOME",
            home.path().join("data").display().to_string(),
        )]));
        let (service, paths) = live_service(
            &home,
            SandboxPorts::none()
                .with_keys(keys_for(&probe_paths))
                .with_recorder(recorder.clone()),
        );
        assert_eq!(planned_endpoint(&service).await, "", "nothing is set yet");

        // The settings form: endpoint through `SetConfig`, key through `LlmKey`.
        crate::config_rpc::set(&paths, "llm.endpoint", "https://llm.example.net/api/v1/")
            .expect("SetConfig writes");
        crate::llm_key::answer(
            &paths,
            &keys_for(&probe_paths),
            v1::LlmKeyRequest {
                op: Some(v1::llm_key_request::Op::Set(v1::llm_key_request::Set {
                    key: KEY.to_owned(),
                })),
                server: String::new(),
            },
        )
        .expect("LlmKey stores");

        assert_eq!(
            planned_endpoint(&service).await,
            "https://llm.example.net/api/v1/",
            "the next Plan names the saved endpoint"
        );

        // The next start: its resolution sets the slot the sandbox is built from.
        service
            .inner
            .apply_session(
                &v1::sandbox_request::Start::default(),
                Some(home.path().join("project")),
            )
            .expect("the session resolves");
        let prepared = service
            .inner
            .prepare_keyed(&v1::sandbox_request::Plan::default(), &[], KeyNeed::Start)
            .expect("the session is prepared");
        let secret = prepared
            .session
            .files
            .iter()
            .find(|file| file.secret)
            .expect("the key file");
        assert_eq!(
            secret.content,
            KEY.as_bytes(),
            "the new key reaches the agent"
        );
        let hash = humanitl_core::http::sha256(KEY.as_bytes());
        assert!(
            recorder.masks().expected_at(&hash, "llm.example.net:443"),
            "the new key is masked at the new endpoint"
        );
    }

    /// The interleaving of the review (HUM-277): endpoint B is saved, a start
    /// takes its claim but has not published its configuration, and a `Plan`
    /// arrives. The `Plan` answers with B from one fresh read, not with the
    /// stale slot, and does not touch the slot. Once the start published its
    /// session, a `Plan` shows that session even when the file moved on; after
    /// the claim is gone it reads the file again.
    #[tokio::test]
    async fn a_plan_beside_a_claimed_start_answers_from_one_configuration() {
        let home = tempfile::TempDir::new().unwrap();
        let (service, paths) = live_service(&home, SandboxPorts::none());
        let claim = service.inner.claim_start().expect("the claim is free");
        crate::config_rpc::set(&paths, "llm.endpoint", "https://b.example.net/v1/")
            .expect("SetConfig writes");

        assert_eq!(
            planned_endpoint(&service).await,
            "https://b.example.net/v1/",
            "an unpublished start does not hide the saved endpoint"
        );
        assert!(
            service.inner.config().llm.endpoint.is_none(),
            "the Plan read afresh and left the slot to the start"
        );

        service
            .inner
            .apply_session(
                &v1::sandbox_request::Start::default(),
                Some(home.path().join("project")),
            )
            .expect("the session resolves");
        crate::config_rpc::set(&paths, "llm.endpoint", "https://c.example.net/v1/")
            .expect("SetConfig writes");
        assert_eq!(
            planned_endpoint(&service).await,
            "https://b.example.net/v1/",
            "a published session shows what it runs with"
        );

        drop(claim);
        assert_eq!(
            planned_endpoint(&service).await,
            "https://c.example.net/v1/",
            "without claim or handle the Plan reads the file again"
        );

        // The next start takes its claim: the previous session's slot no
        // longer answers, even before this start publishes its own.
        let again = service.inner.claim_start().expect("the claim is free");
        crate::config_rpc::set(&paths, "llm.endpoint", "https://d.example.net/v1/")
            .expect("SetConfig writes");
        assert_eq!(
            planned_endpoint(&service).await,
            "https://d.example.net/v1/",
            "a new claim retires the previous session's configuration"
        );
        drop(again);
    }

    /// A start that holds its claim blocks every switch and every removal
    /// that deletes a key (HUM-289, review): it has resolved the active
    /// server and fetches that server's key only later. Once it is gone, the
    /// same requests go through.
    #[tokio::test]
    async fn a_claimed_start_blocks_switching_and_key_deleting_removals() {
        let home = tempfile::TempDir::new().unwrap();
        let (service, paths) = live_service(&home, SandboxPorts::none());
        let keys = KeyStore::for_paths(&paths).with_secret_tool(None);
        let open = crate::llm_servers::OPEN;
        for (name, endpoint) in [
            ("home", "http://home.lan:11434/"),
            ("cloud", "https://api.example.com/v1"),
        ] {
            crate::llm_servers::answer(
                &paths,
                &keys,
                &open,
                &v1::LlmServersRequest {
                    op: Some(v1::llm_servers_request::Op::Add(
                        v1::llm_servers_request::Add {
                            name: name.to_owned(),
                            endpoint: endpoint.to_owned(),
                            models: Vec::new(),
                            activate: false,
                        },
                    )),
                },
            )
            .expect("the server is saved");
        }
        let idle = |work: &mut dyn FnMut()| service.while_idle(work);
        let no_start = |work: &mut dyn FnMut()| service.while_no_start(work);
        let gates = crate::llm_servers::Gates {
            idle: &idle,
            no_start: &no_start,
        };
        let activate = v1::LlmServersRequest {
            op: Some(v1::llm_servers_request::Op::Activate(
                v1::llm_servers_request::Activate {
                    name: "cloud".to_owned(),
                },
            )),
        };
        let remove_home = v1::LlmServersRequest {
            op: Some(v1::llm_servers_request::Op::Remove(
                v1::llm_servers_request::Remove {
                    name: "home".to_owned(),
                    delete_key: true,
                    replacement: String::new(),
                },
            )),
        };

        let claim = service.inner.claim_start().expect("the claim is free");
        let refused = crate::llm_servers::answer(&paths, &keys, &gates, &activate).unwrap_err();
        assert_eq!(refused.code, codes::CONFIG_021);
        let refused = crate::llm_servers::answer(&paths, &keys, &gates, &remove_home).unwrap_err();
        assert_eq!(refused.code, codes::CONFIG_021);
        let remove_cloud = v1::LlmServersRequest {
            op: Some(v1::llm_servers_request::Op::Remove(
                v1::llm_servers_request::Remove {
                    name: "cloud".to_owned(),
                    delete_key: true,
                    replacement: String::new(),
                },
            )),
        };
        let refused = crate::llm_servers::answer(&paths, &keys, &gates, &remove_cloud).unwrap_err();
        assert_eq!(
            refused.code,
            codes::CONFIG_021,
            "a key went away under a start"
        );
        let table = crate::llm_servers::read_table(&paths).unwrap();
        assert_eq!(
            table["llm"]["servers"].as_table().map(toml::Table::len),
            Some(2)
        );
        assert_eq!(
            humanitl_config::servers::active_name(&table).as_deref(),
            Some("home"),
            "the server the start resolved changed under it"
        );
        drop(claim);

        crate::llm_servers::answer(&paths, &keys, &gates, &activate).expect("no start, a switch");
        crate::llm_servers::answer(&paths, &keys, &gates, &remove_home)
            .expect("no start, a removal");
    }

    /// `while_idle` runs nothing while a start holds its claim, and runs the
    /// work once the claim is gone (HUM-289, review E).
    #[tokio::test]
    async fn while_idle_refuses_while_a_start_holds_its_claim() {
        let home = tempfile::TempDir::new().unwrap();
        let (service, _paths) = live_service(&home, SandboxPorts::none());
        let claim = service.inner.claim_start().expect("the claim is free");
        let mut ran = false;
        assert!(!service.while_idle(&mut || ran = true));
        assert!(!ran, "the work ran beside a claimed start");
        drop(claim);
        assert!(service.while_idle(&mut || ran = true));
        assert!(ran);
    }

    /// Switching the active model server while a session runs (HUM-289):
    /// the running session keeps the server, endpoint and key it started
    /// with; the next one gets the new server.
    #[tokio::test]
    async fn activating_another_server_leaves_the_running_session_alone() {
        let home = tempfile::TempDir::new().unwrap();
        let (service, paths) = live_service(&home, SandboxPorts::none());
        let keys = KeyStore::for_paths(&paths).with_secret_tool(None);
        let add = |name: &str, endpoint: &str| v1::LlmServersRequest {
            op: Some(v1::llm_servers_request::Op::Add(
                v1::llm_servers_request::Add {
                    name: name.to_owned(),
                    endpoint: endpoint.to_owned(),
                    models: Vec::new(),
                    activate: false,
                },
            )),
        };
        crate::llm_servers::answer(
            &paths,
            &keys,
            &crate::llm_servers::OPEN,
            &add("home", "http://home.lan:11434/"),
        )
        .expect("the first server is saved and active");
        crate::llm_servers::answer(
            &paths,
            &keys,
            &crate::llm_servers::OPEN,
            &add("cloud", "https://api.example.com/v1"),
        )
        .expect("the second server is saved");

        let claim = service.inner.claim_start().expect("the claim is free");
        service
            .inner
            .apply_session(
                &v1::sandbox_request::Start::default(),
                Some(home.path().join("project")),
            )
            .expect("the session resolves");
        crate::llm_servers::answer(
            &paths,
            &keys,
            &crate::llm_servers::OPEN,
            &v1::LlmServersRequest {
                op: Some(v1::llm_servers_request::Op::Activate(
                    v1::llm_servers_request::Activate {
                        name: "cloud".to_owned(),
                    },
                )),
            },
        )
        .expect("the switch is written");

        assert_eq!(
            service
                .inner
                .config()
                .llm
                .endpoint
                .as_ref()
                .map(url::Url::as_str),
            Some("http://home.lan:11434/"),
            "the running session switched its server"
        );
        assert_eq!(
            planned_endpoint(&service).await,
            "http://home.lan:11434/",
            "the snapshot of the running session shows its own server"
        );
        drop(claim);
        assert_eq!(
            planned_endpoint(&service).await,
            "https://api.example.com/v1",
            "the next session gets the new server"
        );
    }

    /// The second interleaving of the review (HUM-277): a `Plan` takes its
    /// configuration, then a start claims and publishes endpoint B before the
    /// `Plan` reads its running facts. The build is repeated, and the answer
    /// comes from B, not from the configuration taken before.
    #[tokio::test]
    async fn a_plan_across_a_publication_is_built_again() {
        let home = tempfile::TempDir::new().unwrap();
        let (service, paths) = live_service(&home, SandboxPorts::none());
        let inner = Arc::clone(&service.inner);
        let mut claim = None;
        let mut builds = 0;
        let endpoint = inner.consistent(
            |config| {
                builds += 1;
                if builds == 1 {
                    // In the middle of the first build a start claims and
                    // publishes its session with B.
                    crate::config_rpc::set(&paths, "llm.endpoint", "https://b.example.net/v1/")
                        .expect("SetConfig writes");
                    claim = inner.claim_start();
                    inner
                        .apply_session(
                            &v1::sandbox_request::Start::default(),
                            Some(home.path().join("project")),
                        )
                        .expect("the session resolves");
                }
                config.llm.endpoint.as_ref().map(ToString::to_string)
            },
            drop,
        );
        assert_eq!(builds, 2, "the build across the publication is repeated");
        assert_eq!(endpoint.as_deref(), Some("https://b.example.net/v1/"));
        drop(claim);

        // Bounded: a session that changes on every build gets the last one.
        let mut restless = 0;
        inner.consistent(
            |_| {
                restless += 1;
                inner.bump_generation();
            },
            drop,
        );
        assert_eq!(restless, SNAPSHOT_ATTEMPTS);
    }

    /// A start after an agent that exited, without a Stop in between, reads
    /// the saved configuration: the exited session is replaced, and only an
    /// agent that still runs keeps its configuration (HUM-277).
    #[test]
    fn a_start_after_an_exited_agent_reads_afresh() {
        let exited = Facts {
            id: Some("old".to_owned()),
            started_at: Some(SystemTime::now()),
            held: true,
            agent_running: false,
        };
        assert!(
            start_may_refresh(&exited),
            "the held handle of an exited agent"
        );
        let running = Facts {
            agent_running: true,
            ..exited
        };
        assert!(!start_may_refresh(&running), "an agent that still runs");
        assert!(start_may_refresh(&Facts::default()), "nothing held");
    }

    /// The start that holds the claim puts the saved configuration into the
    /// slot before it checks the project directory (HUM-277).
    #[test]
    fn the_claimed_start_refreshes_the_slot() {
        let home = tempfile::TempDir::new().unwrap();
        let (service, paths) = live_service(&home, SandboxPorts::none());
        let claim = service.inner.claim_start().expect("the claim is free");
        crate::config_rpc::set(&paths, "llm.endpoint", "https://b.example.net/v1/")
            .expect("SetConfig writes");
        service.inner.refresh_for_start();
        assert_eq!(
            service
                .inner
                .config()
                .llm
                .endpoint
                .as_ref()
                .map(url::Url::as_str),
            Some("https://b.example.net/v1/")
        );
        drop(claim);
    }

    /// Writes `sandbox.work_dir` the way a person declares it, past the check
    /// of `SetConfig` (`humanitl config set`, an editor): a folder outside the
    /// home directory gets into the file only this way (HUM-293).
    fn declare_work_dir(paths: &humanitl_config::Paths, dir: &Path) {
        humanitl_config::edit::set_value(
            &paths.config_path(),
            &["sandbox".to_owned(), "work_dir".to_owned()],
            Some(&toml::Value::String(dir.display().to_string())),
            |_| Ok(()),
        )
        .expect("the file is written");
    }

    /// A directory outside the home directory, and the `Plan` that names it.
    fn outside_dir() -> (tempfile::TempDir, PathBuf, v1::sandbox_request::Plan) {
        let outside = tempfile::TempDir::new().unwrap();
        let canonical = outside.path().canonicalize().unwrap();
        let plan = v1::sandbox_request::Plan {
            work_dir: canonical.display().to_string(),
            ..v1::sandbox_request::Plan::default()
        };
        (outside, canonical, plan)
    }

    /// The third and fourth interleavings of the review (HUM-277):
    /// configuration A allows a directory outside the home directory, two
    /// `Plan`s with different profiles build with A at the same time, and a
    /// start publishes B, which refuses the directory, before either build is
    /// answered. Both builds are thrown away; neither profile nor directory
    /// stays in `pending` for a later start to inherit.
    #[tokio::test]
    async fn discarded_plan_builds_leave_no_choice_behind() {
        let home = tempfile::TempDir::new().unwrap();
        let (service, paths) = live_service(&home, SandboxPorts::none());
        let (_outside, canonical, plan) = outside_dir();
        declare_work_dir(&paths, &canonical);
        let project = home.path().join("project");
        let inner = Arc::clone(&service.inner);
        let first = v1::sandbox_request::Plan {
            profile: "first".to_owned(),
            ..plan.clone()
        };
        let second = v1::sandbox_request::Plan {
            profile: "second".to_owned(),
            ..plan
        };
        let mut claim = None;
        let mut outer_builds = 0;
        let mut inner_builds = 0;
        let mut second_events = Vec::new();
        let first_events = inner.consistent_plan(&first, || {
            outer_builds += 1;
            if outer_builds > 1 {
                return;
            }
            // While the first build waits, the second Plan builds, and a
            // start publishes B in the middle of that build.
            second_events = inner.consistent_plan(&second, || {
                inner_builds += 1;
                if inner_builds == 1 {
                    crate::config_rpc::set(
                        &paths,
                        "sandbox.work_dir",
                        &project.display().to_string(),
                    )
                    .expect("SetConfig writes");
                    claim = inner.claim_start();
                    inner
                        .apply_session(
                            &v1::sandbox_request::Start::default(),
                            Some(project.clone()),
                        )
                        .expect("the session resolves");
                }
            });
        });
        assert_eq!(
            (outer_builds, inner_builds),
            (2, 2),
            "both builds are repeated"
        );
        for events in [&first_events, &second_events] {
            assert!(
                events.iter().any(|event| matches!(
                    event.event,
                    Some(v1::sandbox_event::Event::Diagnostic(_))
                )),
                "the answered build refuses the directory: {events:?}"
            );
        }
        let pending = inner.pending_choice();
        assert_eq!(pending.work_dir, None, "no thrown-away directory stays");
        assert_eq!(pending.profile, None, "no thrown-away profile stays");
        drop(claim);
    }

    /// The folder of a `Plan` belongs to that one call (HUM-293): it is not
    /// kept, and the next `Plan` without a folder shows `sandbox.work_dir`
    /// again. Before HUM-293 the daemon kept it, the app started in it and
    /// the command line did not, so two places picked the folder by two
    /// rules.
    #[tokio::test]
    async fn a_plans_folder_is_not_kept_for_the_next_call() {
        let home = tempfile::TempDir::new().unwrap();
        let (service, _paths) = live_service(&home, SandboxPorts::none());
        let inner = Arc::clone(&service.inner);
        let other = home.path().join("other");
        std::fs::create_dir_all(&other).unwrap();
        let plan = v1::sandbox_request::Plan {
            work_dir: other.display().to_string(),
            ..v1::sandbox_request::Plan::default()
        };
        let events = inner.consistent_plan(&plan, || {});
        let shown = |events: &[v1::SandboxEvent]| {
            events
                .iter()
                .find_map(|event| match &event.event {
                    Some(v1::sandbox_event::Event::Status(status)) => Some(status.work_dir.clone()),
                    _ => None,
                })
                .expect("a snapshot")
        };
        assert_eq!(
            shown(&events),
            other.canonicalize().unwrap().display().to_string()
        );
        assert_eq!(inner.pending_choice().work_dir, None, "nothing is kept");
        let events = inner.consistent_plan(&v1::sandbox_request::Plan::default(), || {});
        assert_eq!(
            shown(&events),
            home.path().join("project").display().to_string(),
            "the next Plan shows sandbox.work_dir"
        );
    }

    /// The folder a start names lives exactly as long as its claim
    /// (HUM-293): `--work` overrides the stored folder for one call.
    #[tokio::test]
    async fn a_starts_folder_lives_as_long_as_its_claim() {
        let home = tempfile::TempDir::new().unwrap();
        let (service, _paths) = live_service(&home, SandboxPorts::none());
        let inner = Arc::clone(&service.inner);
        let other = home.path().join("other");
        std::fs::create_dir_all(&other).unwrap();
        let start = v1::sandbox_request::Plan {
            work_dir: other.display().to_string(),
            ..v1::sandbox_request::Plan::default()
        };
        let claim = inner.claim_start().expect("the claim is free");
        let checked = inner.remember(&start).expect("a folder under home");
        assert_eq!(checked, Some(other.canonicalize().unwrap()));
        assert_eq!(inner.pending_choice().work_dir, checked, "while it starts");
        drop(claim);
        assert_eq!(inner.pending_choice().work_dir, None, "and not after");
    }

    /// While a start holds its folder, a finding about it names the source
    /// the client gave, not "the request" (review HUM-293).
    #[tokio::test]
    async fn a_starting_folder_keeps_its_source() {
        let home = tempfile::TempDir::new().unwrap();
        let (service, _paths) = live_service(&home, SandboxPorts::none());
        let inner = Arc::clone(&service.inner);
        let other = home.path().join("other");
        std::fs::create_dir_all(&other).unwrap();
        let start = v1::sandbox_request::Plan {
            work_dir: other.display().to_string(),
            work_dir_source: Some(crate::work_dir::source_to_proto(&WorkDirSource::Flag)),
            ..v1::sandbox_request::Plan::default()
        };
        let claim = inner.claim_start().expect("the claim is free");
        inner.remember(&start).expect("a folder under home");
        let config = inner.config();
        let choice = inner.pending_choice();
        assert_eq!(
            inner.work_dir_source(&config, &choice, &start),
            WorkDirSource::Flag
        );
        assert_eq!(
            inner.work_dir_source(&config, &choice, &v1::sandbox_request::Plan::default()),
            WorkDirSource::Flag,
            "a Plan without a folder during the start names it too"
        );
        drop(claim);
    }

    /// Every refusal of a folder over the socket names the source the client
    /// gave (HUM-293); a client without one gets "the request".
    #[tokio::test]
    async fn a_refused_folder_names_the_source_of_the_request() {
        let home = tempfile::TempDir::new().unwrap();
        let (service, _paths) = live_service(&home, SandboxPorts::none());
        let inner = Arc::clone(&service.inner);
        let plan = |source: Option<v1::WorkDirSource>| v1::sandbox_request::Plan {
            work_dir: home.path().display().to_string(),
            work_dir_source: source,
            ..v1::sandbox_request::Plan::default()
        };
        let current = crate::work_dir::source_to_proto(&WorkDirSource::CurrentDir);
        let refused = inner
            .remember(&plan(Some(current)))
            .expect_err("home is no project");
        assert_eq!(refused.code, codes::SANDBOX_006);
        assert!(
            refused.why.contains("from the current directory"),
            "{}",
            refused.why
        );
        assert!(!refused.why.contains("sandbox.work_dir"), "{}", refused.why);
        let refused = inner.remember(&plan(None)).expect_err("home is no project");
        assert!(
            refused.why.contains("from the work_dir of the request"),
            "{}",
            refused.why
        );
    }

    /// A configured folder the start refuses names the file it stands in
    /// (HUM-293).
    #[tokio::test]
    async fn a_refused_configured_folder_names_its_file() {
        let home = tempfile::TempDir::new().unwrap();
        let (service, paths) = live_service(&home, SandboxPorts::none());
        declare_work_dir(&paths, home.path());
        let inner = Arc::clone(&service.inner);
        inner.refresh_for_start();
        let Err(refused) =
            inner.prepare_keyed(&v1::sandbox_request::Plan::default(), &[], KeyNeed::Start)
        else {
            panic!("home is no project");
        };
        assert_eq!(refused.code, codes::SANDBOX_006);
        let named = format!("sandbox.work_dir in {}", paths.config_path().display());
        assert!(refused.why.contains(&named), "{}", refused.why);
    }

    /// A folder chosen in the app is stored resolved (HUM-293); when a parent
    /// of it is later replaced by a link to a place outside the home
    /// directory, the same text names another folder. The next start refuses
    /// it and names the config file (HUM-294).
    #[tokio::test]
    async fn a_stored_folder_moved_out_of_home_by_a_link_is_refused_at_start() {
        let home = tempfile::TempDir::new().unwrap();
        let (service, paths) = live_service(&home, SandboxPorts::none());
        let parent = home.path().join("projects");
        std::fs::create_dir_all(parent.join("x")).unwrap();
        crate::config_rpc::set(
            &paths,
            "sandbox.work_dir",
            &parent.join("x").display().to_string(),
        )
        .expect("SetConfig stores a folder under home");
        let inner = Arc::clone(&service.inner);
        inner.refresh_for_start();
        let prepared = inner
            .prepare_keyed(&v1::sandbox_request::Plan::default(), &[], KeyNeed::Start)
            .expect("the chosen folder starts");
        assert_eq!(
            prepared.backend.approved_work_dir(),
            Some(parent.join("x").canonicalize().unwrap().as_path()),
            "the backend mounts only the folder checked here"
        );

        let outside = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(outside.path().join("projects/x")).unwrap();
        std::fs::remove_dir_all(&parent).unwrap();
        std::os::unix::fs::symlink(outside.path().join("projects"), &parent).unwrap();
        inner.refresh_for_start();
        let Err(refused) =
            inner.prepare_keyed(&v1::sandbox_request::Plan::default(), &[], KeyNeed::Start)
        else {
            panic!("the link leads out of home");
        };
        assert_eq!(refused.code, codes::SANDBOX_006);
        let named = format!("sandbox.work_dir in {}", paths.config_path().display());
        assert!(refused.why.contains(&named), "{}", refused.why);
        assert!(
            refused
                .why
                .contains("a symbolic link in its path leads elsewhere"),
            "{}",
            refused.why
        );
        // The plan says the same, so Setup never shows the folder green.
        let Err(planned) =
            inner.prepare_keyed(&v1::sandbox_request::Plan::default(), &[], KeyNeed::Plan)
        else {
            panic!("the plan refuses it too");
        };
        assert_eq!(planned.code, codes::SANDBOX_006);
    }

    /// A folder outside the home directory that a person declared, with
    /// `humanitl config set` or by hand, under the path it really has, still
    /// starts (HUM-294); written through a link, it is refused with the path
    /// to declare instead.
    #[tokio::test]
    async fn a_declared_folder_outside_home_still_starts() {
        let home = tempfile::TempDir::new().unwrap();
        let (service, paths) = live_service(&home, SandboxPorts::none());
        let (outside, canonical, _plan) = outside_dir();
        declare_work_dir(&paths, &canonical);
        let inner = Arc::clone(&service.inner);
        inner.refresh_for_start();
        let prepared = inner
            .prepare_keyed(&v1::sandbox_request::Plan::default(), &[], KeyNeed::Start)
            .expect("a declared folder outside home starts");
        assert_eq!(prepared.session.work_src, canonical);

        let link_outside = tempfile::TempDir::new().unwrap();
        let through = link_outside.path().join("link");
        std::os::unix::fs::symlink(outside.path(), &through).unwrap();
        declare_work_dir(&paths, &through);
        inner.refresh_for_start();
        let Err(refused) =
            inner.prepare_keyed(&v1::sandbox_request::Plan::default(), &[], KeyNeed::Start)
        else {
            panic!("a declaration through a link names another path");
        };
        assert_eq!(refused.code, codes::SANDBOX_006);
        assert!(
            refused.why.contains(&format!(
                "humanitl config set sandbox.work_dir {}",
                canonical.display()
            )),
            "{}",
            refused.why
        );
    }

    /// `humanitl run --config other.toml` sends the folder of `other.toml`
    /// with that file as its source (HUM-296). Only the daemon's own config
    /// file declares a folder outside the home directory, so the start is
    /// refused, and the fix is the command that writes the folder into that
    /// file. After that declaration the same request starts.
    #[tokio::test]
    async fn a_folder_outside_home_from_a_named_config_file_names_the_daemons_file() {
        let home = tempfile::TempDir::new().unwrap();
        let (service, paths) = live_service(&home, SandboxPorts::none());
        let (_outside, canonical, mut start) = outside_dir();
        let named = home.path().join("other.toml");
        start.work_dir_source = Some(crate::work_dir::source_to_proto(
            &WorkDirSource::ConfigFile(named.clone()),
        ));
        let inner = Arc::clone(&service.inner);
        let refused = inner
            .remember(&start)
            .expect_err("a file the client names declares nothing");
        assert_eq!(refused.code, codes::SANDBOX_006);
        let daemon_file = paths.config_path();
        assert!(
            refused
                .why
                .contains(&format!("sandbox.work_dir in {}", named.display())),
            "{}",
            refused.why
        );
        assert!(
            refused.why.contains(&format!(
                "only in the daemon's config file {}",
                daemon_file.display()
            )),
            "{}",
            refused.why
        );
        assert_eq!(
            refused.fix,
            Some(FixAction::CopyCommand(format!(
                "humanitl --config {} config set sandbox.work_dir {}",
                daemon_file.display(),
                canonical.display()
            ))),
            "the fix names the daemon's file"
        );

        declare_work_dir(&paths, &canonical);
        inner.refresh_for_start();
        assert_eq!(
            inner
                .remember(&start)
                .expect("declared in the daemon's file"),
            Some(canonical)
        );
    }
}
