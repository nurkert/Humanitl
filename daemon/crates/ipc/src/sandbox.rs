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
//! Es prüft die drei Garantien nicht (HUM-041), es hängt kein Terminal an
//! (HUM-042) und es schreibt keine Konfiguration (`SetConfig`, HUM-069). Das
//! Projektverzeichnis, das die Oberfläche wählt, reist deshalb in
//! [`v1::sandbox_request::Plan`] und in [`v1::sandbox_request::Start`] mit und
//! gilt für die laufende Sitzung; dauerhaft wird es erst mit dem
//! Einstellungs-Bildschirm.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

use humanitl_config::{Config, WorkMode};
use humanitl_core::Severity as CoreSeverity;
use humanitl_core::diagnostics::codes;
use humanitl_core::ids::SessionId;
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_sandbox::agent::opencode;
use humanitl_sandbox::{
    AdapterRegistry, AgentContext, BwrapBackend, KILL_GRACE, LaunchInputs, MIN_BWRAP_VERSION,
    MountPolicy, SANDBOX_SHELL, SandboxBackend, SandboxFile, SandboxHandle, SandboxProfile,
    SessionContext, StdioMode, shell_line,
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::server_stub::BoxStream;
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
];

/// Die Variable, die allein die Sitzung kennt.
const ENV_SESSION: &str = "HUMANITL_SESSION";

/// So viele Ereignisse darf ein Sandbox-Strom puffern, bevor der Erzeuger
/// wartet. Ein Start schickt eine Handvoll; die Reserve fängt einen Client
/// ab, der langsamer liest, als der Start meldet.
const EVENT_BUFFER: usize = 32;

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
    started_at: SystemTime,
    profile: String,
    work_dir: PathBuf,
    work_mode: WorkMode,
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
/// Es steht hier und nicht in `config.toml`, weil `SetConfig` erst mit dem
/// Einstellungs-Bildschirm kommt (HUM-069). Die Wahl gilt deshalb für diese
/// Sitzung; ohne sie fiele die Momentaufnahme beim nächsten `Status` still
/// auf das Verzeichnis der Konfiguration zurück, und der gewählte Ordner
/// verschwände, ohne dass jemand etwas angefasst hätte.
#[derive(Debug, Default)]
struct Pending {
    profile: Option<String>,
    work_dir: Option<PathBuf>,
    work_mode: Option<WorkMode>,
}

#[derive(Debug)]
struct Inner {
    config: Config,
    paths: humanitl_config::Paths,
    session: SessionId,
    running: Mutex<Option<Running>>,
    pending: Mutex<Pending>,
}

impl SandboxService {
    /// Der Dienst für die Sitzung [`session`](SessionId) über [`config`](Config).
    #[must_use]
    pub fn new(config: Config, paths: humanitl_config::Paths, session: SessionId) -> Self {
        Self {
            inner: Arc::new(Inner {
                config,
                paths,
                session,
                running: Mutex::new(None),
                pending: Mutex::new(Pending::default()),
            }),
        }
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
            // `isolation_check` und `status` beantworten beide den
            // Schnappschuss; die drei Garantien kommen mit HUM-041 als eigene
            // Ereignisse dazu, und bis dahin ist eine leere Antwort ehrlicher
            // als ein erfundenes Ergebnis.
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
    /// Prüft den Wunsch und merkt ihn sich für den nächsten Start.
    ///
    /// Leere Felder ändern nichts: `Status` schickt einen leeren Wunsch und
    /// darf die Wahl nicht löschen.
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
    fn remember(&self, plan: &v1::sandbox_request::Plan) -> Result<(), Diagnostic> {
        let profile = match plan.profile.trim() {
            "" => None,
            name => {
                check_profile_name(name)?;
                Some(name.to_owned())
            }
        };
        let work_dir = match plan.work_dir.trim() {
            "" => None,
            dir => Some(self.check_work_dir(Path::new(dir))?),
        };
        let work_mode = match plan.work_mode.trim().to_ascii_lowercase().as_str() {
            "ro" => Some(WorkMode::Ro),
            "rw" => Some(WorkMode::Rw),
            _ => None,
        };

        let mut pending = lock(&self.pending);
        if let Some(profile) = profile {
            pending.profile = Some(profile);
        }
        if let Some(dir) = work_dir {
            pending.work_dir = Some(dir);
        }
        if let Some(mode) = work_mode {
            pending.work_mode = Some(mode);
        }
        Ok(())
    }

    /// Prüft ein Projektverzeichnis, das über die RPC hereinkommt.
    ///
    /// Drei Stufen, und alle drei müssen halten:
    ///
    /// 1. **Es ist ein Verzeichnis.** Absolut, ohne `..`, vorhanden, und nach
    ///    dem Auflösen der Symlinks immer noch ein Verzeichnis. Der aufgelöste
    ///    Pfad ist der, der weiterreist: Sonst zeigte der Bildschirm den
    ///    geschriebenen und die Sandbox hängte den aufgelösten ein.
    /// 2. **Die Politik der Sandbox** ([`MountPolicy::check_work_dir`]) — die
    ///    Denylist, das Heimatverzeichnis selbst, die Verzeichnisse von
    ///    Humanitl, alles, was darüber liegt.
    /// 3. **Es liegt dort, wo Projekte liegen**: unter dem Heimatverzeichnis,
    ///    oder es ist genau das Verzeichnis, das in `sandbox.work_dir` steht.
    ///    Stufe 2 kennt `/etc`, `/usr` und `/var/lib` nicht, und ein
    ///    Bildschirm, der „der Agent sieht nur dein Projekt" verspricht und
    ///    `/etc` zeigt, hat die Zusage gebrochen. Wer ein Projekt außerhalb
    ///    des Heimatverzeichnisses hat, schreibt es in `config.toml`; dort
    ///    steht die Erklärung eines Menschen und nicht der Wunsch eines
    ///    Clients.
    ///
    /// # Errors
    ///
    /// `SANDBOX_006` mit dem Grund, in der Sprache des Nutzers.
    fn check_work_dir(&self, dir: &Path) -> Result<PathBuf, Diagnostic> {
        if !dir.is_absolute() {
            return Err(work_dir_refused(
                dir,
                "a project directory is an absolute path",
            ));
        }
        if dir
            .components()
            .any(|part| part == std::path::Component::ParentDir)
        {
            return Err(work_dir_refused(
                dir,
                "a project directory is written out, without `..`",
            ));
        }
        let resolved = dir.canonicalize().map_err(|error| {
            work_dir_refused(dir, &format!("cannot be read as a directory: {error}"))
        })?;
        if !resolved.is_dir() {
            return Err(work_dir_refused(dir, "is not a directory"));
        }
        MountPolicy::from_paths(&self.paths).check_work_dir(&resolved)?;

        let home = self.paths.home();
        let configured = self
            .config
            .sandbox
            .work_dir
            .as_ref()
            .and_then(|dir| dir.canonicalize().ok());
        let allowed = resolved.starts_with(&home)
            || configured.is_some_and(|configured| configured == resolved);
        if !allowed {
            return Err(work_dir_refused(
                dir,
                &format!(
                    "lies outside {} and is not the directory named in sandbox.work_dir; a                      project directory comes from your home directory, and anything else is                      declared in config.toml",
                    home.display()
                ),
            ));
        }
        Ok(resolved)
    }

    /// Der Schnappschuss zu diesem Wunsch, oder der Befund, der ihn verhindert.
    fn snapshot_or_diagnostic(
        &self,
        plan: &v1::sandbox_request::Plan,
        tx: &mpsc::Sender<v1::SandboxEvent>,
    ) {
        if let Err(diagnostic) = self.remember(plan) {
            let _ = tx.blocking_send(diagnostic_event(&diagnostic));
            // Der abgelehnte Wunsch steht in keiner Momentaufnahme: Sie zeigte
            // sonst das Verzeichnis, das gerade verweigert wurde.
            let _ = tx.blocking_send(status_event(
                self.failed_status(&v1::sandbox_request::Plan::default(), &diagnostic),
            ));
            return;
        }
        // Erst die Befunde, dann der Zustand: Der Client trägt die Befunde des
        // laufenden Vorgangs am Zustand mit, und einer, der nach dem Zustand
        // käme, stünde ohne ihn da.
        for diagnostic in self.preflight() {
            let _ = tx.blocking_send(diagnostic_event(&diagnostic));
        }
        match self.snapshot(plan) {
            Ok(status) => {
                let _ = tx.blocking_send(status_event(status));
            }
            Err(diagnostic) => {
                let _ = tx.blocking_send(diagnostic_event(&diagnostic));
                let _ = tx.blocking_send(status_event(self.failed_status(plan, &diagnostic)));
            }
        }
    }

    /// Was einem Start im Weg steht, bevor jemand ihn versucht.
    ///
    /// Heute genau eine Frage: Gibt es `bwrap`, und ist es neu genug
    /// ([`BwrapBackend::detect`], `SANDBOX_001` bis `SANDBOX_003`)? Der
    /// Schnappschuss selbst kommt auch ohne `bwrap` zustande — er zeigt, was
    /// starten *würde* —, aber die Schaltfläche gehört dann aus, und der Grund
    /// dazu (`docs/UX.md` 5.3). Die drei Garantien prüft HUM-041, und sie
    /// lassen sich ohnehin erst an einer laufenden Sandbox prüfen.
    fn preflight(&self) -> Vec<Diagnostic> {
        match BwrapBackend::detect(self.paths.clone()) {
            Ok(_) => Vec::new(),
            Err(diagnostic) => vec![diagnostic],
        }
    }

    /// Die Kommandozeile, Argument für Argument, als eigene Ereignisse.
    fn argv(&self, plan: &v1::sandbox_request::Plan, tx: &mpsc::Sender<v1::SandboxEvent>) {
        match self.prepare(plan) {
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
    async fn start(
        self: Arc<Self>,
        start: v1::sandbox_request::Start,
        tx: mpsc::Sender<v1::SandboxEvent>,
    ) {
        let plan = v1::sandbox_request::Plan {
            profile: start.profile.clone(),
            work_dir: start.work_dir.clone(),
            work_mode: start.work_mode.clone(),
        };
        if let Err(diagnostic) = self.remember(&plan) {
            let _ = tx.send(diagnostic_event(&diagnostic)).await;
            let _ = tx
                .send(status_event(self.failed_status(
                    &v1::sandbox_request::Plan::default(),
                    &diagnostic,
                )))
                .await;
            return;
        }
        if self.is_running() {
            let this = Arc::clone(&self);
            let plan = plan.clone();
            let tx = tx.clone();
            let _ =
                tokio::task::spawn_blocking(move || this.snapshot_or_diagnostic(&plan, &tx)).await;
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

        let this = Arc::clone(&self);
        let launch_plan = plan.clone();
        let command: Vec<OsString> = start.command.iter().map(OsString::from).collect();
        let launched =
            tokio::task::spawn_blocking(move || this.launch(&launch_plan, &command)).await;
        match launched {
            Ok(Ok(())) => {
                // Die Zeile, die der Log-Reiter zeigt. Der leere Zustand dort
                // verspricht sie („ein Start und ein Stopp schreiben je eine
                // Zeile"), also muss es sie geben; das Terminal des Agenten
                // ist etwas anderes und kommt mit HUM-042.
                if let Some(line) = self.started_line() {
                    let _ = tx.send(log_event(line)).await;
                }
                let this = Arc::clone(&self);
                let plan = plan.clone();
                let running = tokio::task::spawn_blocking(move || this.snapshot(&plan)).await;
                if let Ok(Ok(status)) = running {
                    let _ = tx.send(status_event(status)).await;
                }
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

    /// Beendet die laufende Sandbox und meldet `stopping`, dann `stopped`.
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
                && tx.send(status_event(status)).await.is_err()
            {
                return;
            }
        }

        let _ = tokio::task::spawn_blocking(move || handle.terminate(KILL_GRACE)).await;
        let stopped = self.stopped_line();
        self.clear_running();
        if let Some(line) = stopped {
            let _ = tx.send(log_event(line)).await;
        }

        let this = Arc::clone(&self);
        if let Ok(Ok(status)) = tokio::task::spawn_blocking(move || this.snapshot(&plan)).await {
            let _ = tx.send(status_event(status)).await;
        }
    }

    /// Startet die geplante Sandbox und merkt sie sich.
    fn launch(
        &self,
        plan: &v1::sandbox_request::Plan,
        command: &[OsString],
    ) -> Result<(), Diagnostic> {
        let prepared = self.prepare_with_command(plan, command)?;
        // Gesammelt, nicht durchgereicht: Der Daemon hat kein Terminal, und
        // die Ausgabe des Agenten gehört ohnehin in das Terminal von HUM-042.
        let backend = prepared.backend.clone().with_stdio(StdioMode::Capture);
        let launch = backend.plan(&prepared.profile, &prepared.session)?;
        let handle = backend.launch(&launch)?;
        let mut running = lock(&self.running);
        *running = Some(Running {
            handle: Arc::new(handle),
            started_at: SystemTime::now(),
            profile: prepared.profile.name.clone(),
            work_dir: prepared.session.work_src.clone(),
            work_mode: prepared.session.work_mode,
        });
        Ok(())
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
        let prepared = self.prepare(plan)?;
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
        let state = state.unwrap_or(if held {
            v1::SandboxState::Running
        } else {
            v1::SandboxState::Stopped
        });
        Ok(v1::sandbox_event::Status {
            state: state.into(),
            sandbox_id: id.unwrap_or_default(),
            session_id: self.session.to_string(),
            backend: prepared.backend.name().to_owned(),
            llm_endpoint: self
                .config
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
            llm_endpoint: self
                .config
                .llm
                .endpoint
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default(),
            work_dir: self.work_dir(plan).display().to_string(),
            work_mode: work_mode_name(self.work_mode(plan)).to_owned(),
            started_at: started_at.map(crate::convert::timestamp),
            profile: self.profile_name(plan),
            mounts: Vec::new(),
            env: Vec::new(),
            argv_preview: String::new(),
            agent_running,
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

    fn is_running(&self) -> bool {
        lock(&self.running)
            .as_ref()
            .is_some_and(|running| running.handle.try_wait().is_none())
    }

    fn running_handle(&self) -> Option<Arc<SandboxHandle>> {
        lock(&self.running)
            .as_ref()
            .map(|running| Arc::clone(&running.handle))
    }

    fn clear_running(&self) {
        *lock(&self.running) = None;
    }

    /// Profil, Sitzung und Backend für diesen Wunsch.
    fn prepare(&self, plan: &v1::sandbox_request::Plan) -> Result<Prepared, Diagnostic> {
        self.prepare_with_command(plan, &[])
    }

    fn prepare_with_command(
        &self,
        plan: &v1::sandbox_request::Plan,
        command: &[OsString],
    ) -> Result<Prepared, Diagnostic> {
        let policy = MountPolicy::from_paths(&self.paths);
        let name = self.profile_name(plan);
        let (profile_path, profile_origin) = self.profile_path(&name)?;
        let profile = SandboxProfile::load_validated(&profile_path, &policy)?;

        let work_src = self.work_dir(plan);
        let work_mode = self.work_mode(plan);
        let agent = if command.is_empty() {
            self.agent_contribution(&work_src, &profile)?
        } else {
            AgentContribution::default()
        };

        let mut session_env = vec![(ENV_SESSION.to_owned(), self.session.to_string())];
        session_env.extend(agent.env.iter().cloned());
        session_env.extend(
            self.config
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
        for key in self.config.sandbox.env.keys() {
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
            command: self.command(command, &agent.command),
            files: agent.files,
        };

        // Ein Schnappschuss ist eine Anzeige, kein Start: Ohne `bwrap` steht
        // die Zeile trotzdem da, und der Start scheitert später mit dem
        // Befund, der dazu gehört (`SANDBOX_001`).
        let backend = BwrapBackend::detect(self.paths.clone()).unwrap_or_else(|_| {
            BwrapBackend::unchecked("bwrap", MIN_BWRAP_VERSION, self.paths.clone())
        });

        Ok(Prepared {
            env_origin,
            adapter_files: session.files.iter().map(|file| file.dst.clone()).collect(),
            profile,
            session,
            backend,
        })
    }

    /// Was der Agent-Adapter zu dieser Sitzung beiträgt.
    fn agent_contribution(
        &self,
        work_src: &Path,
        profile: &SandboxProfile,
    ) -> Result<AgentContribution, Diagnostic> {
        let registry = AdapterRegistry::builtin();
        let adapter = registry.get(&self.config.agent.adapter).ok_or_else(|| {
            Diagnostic::builder(codes::CONFIG_003, Severity::Blocking)
                .why(format!(
                    "agent.adapter is {:?}, and no adapter of that name exists; known: {}",
                    self.config.agent.adapter,
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

        let ctx = AgentContext::new(
            self.session,
            work_src.to_path_buf(),
            self.config.llm.clone(),
        )
        .with_command_override(
            self.config
                .agent
                .command
                .as_ref()
                .map(|parts| parts.iter().map(OsString::from).collect()),
        )
        .with_host_path(self.paths.env().non_empty("PATH").map(OsString::from))
        .with_language(self.config.ui.language)
        .with_hold(self.config.hold.clone())
        .with_briefing(self.config.agent.briefing.clone())
        .with_home(
            self.config
                .sandbox
                .env
                .get("HOME")
                .or_else(|| profile.env.get("HOME"))
                .map_or_else(
                    || PathBuf::from(humanitl_sandbox::DEFAULT_HOME),
                    PathBuf::from,
                ),
        )
        .with_config_home(
            self.config
                .sandbox
                .env
                .get("XDG_CONFIG_HOME")
                .map(PathBuf::from),
        )
        .with_sandbox_ro_paths(
            profile
                .mounts
                .ro
                .iter()
                .chain(&profile.mounts.extra_ro)
                .cloned()
                .collect(),
        );

        Ok(AgentContribution {
            command: adapter.command(&ctx),
            env: adapter.env(&ctx),
            files: adapter.files(&ctx)?,
        })
    }

    /// Der Befehl in der Sandbox: der des Aufrufers, sonst der des Adapters,
    /// sonst der aus `agent.command`, sonst die Shell.
    fn command(&self, requested: &[OsString], from_adapter: &[OsString]) -> Vec<OsString> {
        if !requested.is_empty() {
            return requested.to_vec();
        }
        if !from_adapter.is_empty() {
            return from_adapter.to_vec();
        }
        self.config
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
    fn profile_name(&self, plan: &v1::sandbox_request::Plan) -> String {
        if !plan.profile.trim().is_empty() {
            return plan.profile.trim().to_owned();
        }
        if let Some(running) = lock(&self.running).as_ref() {
            return running.profile.clone();
        }
        if let Some(chosen) = lock(&self.pending).profile.clone() {
            return chosen;
        }
        self.config.sandbox.profile.clone()
    }

    /// Das Projektverzeichnis dieses Wunsches, sonst das der laufenden
    /// Sandbox, sonst das zuletzt gewählte, sonst das der Konfiguration.
    fn work_dir(&self, plan: &v1::sandbox_request::Plan) -> PathBuf {
        if !plan.work_dir.trim().is_empty() {
            return PathBuf::from(plan.work_dir.trim());
        }
        if let Some(running) = lock(&self.running).as_ref() {
            return running.work_dir.clone();
        }
        if let Some(chosen) = lock(&self.pending).work_dir.clone() {
            return chosen;
        }
        self.config
            .sandbox
            .work_dir
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }

    /// Der Modus dieses Wunsches, sonst der der laufenden Sandbox, sonst der
    /// zuletzt gewählte, sonst der der Konfiguration.
    fn work_mode(&self, plan: &v1::sandbox_request::Plan) -> WorkMode {
        match plan.work_mode.trim().to_ascii_lowercase().as_str() {
            "ro" => WorkMode::Ro,
            "rw" => WorkMode::Rw,
            _ => {
                if let Some(running) = lock(&self.running).as_ref() {
                    return running.work_mode;
                }
                lock(&self.pending)
                    .work_mode
                    .unwrap_or(self.config.sandbox.work_mode)
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
    env: Vec<(String, String)>,
    files: Vec<SandboxFile>,
}

/// Profil, Sitzung und Backend eines Schnappschusses.
#[derive(Debug)]
struct Prepared {
    profile: SandboxProfile,
    session: SessionContext,
    backend: BwrapBackend,
    /// Wer den Wert jeder Umgebungsvariablen zuletzt geschrieben hat.
    ///
    /// Dieselbe Reihenfolge, in der [`SandboxProfile::effective_env`]
    /// zusammenführt; der letzte Schreiber gewinnt. Ein Schlüssel, der hier
    /// fehlt, gilt als von Hand geschrieben und wird zurückgehalten.
    env_origin: BTreeMap<String, v1::ValueOrigin>,
    /// Die Ziele der Dateien des Adapters; für die Herkunftsspalte.
    adapter_files: BTreeSet<PathBuf>,
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
        args.extend(self.profile.to_bwrap_args(
            &self.session,
            &LaunchInputs::preview_with_agent_files(
                self.session.files.iter().map(|file| file.dst.clone()),
            ),
        ));
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
    while index < argv.len() {
        let flag = argv[index].to_string_lossy().into_owned();
        if flag == "--" {
            break;
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

/// Der Befund für ein Projektverzeichnis, das keines sein darf.
fn work_dir_refused(dir: &Path, why: &str) -> Diagnostic {
    Diagnostic::builder(codes::SANDBOX_006, CoreSeverity::Blocking)
        .why(format!("{} {why}", dir.display()))
        .fix(FixAction::ChangeSetting {
            key: "sandbox.work_dir".to_owned(),
            value: String::new(),
        })
        .build()
}

/// `ro` oder `rw`, wie das Protokoll den Modus schreibt.
const fn work_mode_name(mode: WorkMode) -> &'static str {
    match mode {
        WorkMode::Ro => "ro",
        WorkMode::Rw => "rw",
    }
}

/// Der Shim auf dem Host: neben dem Daemon, sonst in einer Installation.
fn shim_path() -> PathBuf {
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        candidates.push(dir.join(SHIM_BINARY));
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

/// Der Befund, wenn der Arbeitsfaden selbst nicht durchkam.
fn joined_failed(error: &tokio::task::JoinError) -> Diagnostic {
    Diagnostic::builder(codes::SANDBOX_012, Severity::Blocking)
        .why(format!("the thread preparing the sandbox failed: {error}"))
        .build()
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
        };
        let paths =
            humanitl_config::Paths::new(humanitl_config::Env::from_pairs([("HOME", "/home/u")]));
        Prepared {
            profile,
            session,
            backend: BwrapBackend::unchecked("bwrap", MIN_BWRAP_VERSION, paths),
            env_origin: BTreeMap::new(),
            adapter_files: BTreeSet::new(),
        }
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
}
