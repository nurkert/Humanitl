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
//! von innen, [`BwrapBackend::isolation_check`] faltet sie zu den drei
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
use std::time::{Duration, SystemTime};

use humanitl_config::{Config, WorkMode};
use humanitl_core::Severity as CoreSeverity;
use humanitl_core::diagnostics::codes;
use humanitl_core::ids::SessionId;
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_proxy::rules_store::RulesStore;
use humanitl_proxy::session::{SessionSettings, SessionState};
use humanitl_sandbox::agent::opencode;
use humanitl_sandbox::{
    AdapterRegistry, AgentContext, BwrapBackend, CheckResult, IsolationCheck, KILL_GRACE,
    LaunchInputs, MIN_BWRAP_VERSION, MountPolicy, SANDBOX_SHELL, SandboxBackend, SandboxFile,
    SandboxHandle, SandboxProfile, SessionContext, StdioMode, shell_line,
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::server_stub::BoxStream;
use crate::session::{
    SessionRequest, SessionResolver, ask_mode_name, bundled_rules, parse_ask_mode, parse_work_mode,
    work_mode_name,
};
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
    /// Das Backend, das diese Sandbox gestartet hat.
    ///
    /// Es wird gehalten und nicht neu gesucht, weil die Isolationsprüfung an
    /// derselben Instanz hängt, die auch gestartet hat: In ihr steht die
    /// Frist, innerhalb deren der Bericht des Shims eintreffen muss
    /// (`BwrapBackend::with_report_timeout`). Ein zweites, frisch gefundenes
    /// Backend hätte eine andere Frist und prüfte damit unter anderen
    /// Bedingungen als es gestartet hat.
    backend: BwrapBackend,
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
        lock(&self.inner.pending).claimed = false;
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
        Self {
            inner: Arc::new(Inner {
                config: RwLock::new(config),
                resolver,
                paths,
                session,
                running: Mutex::new(None),
                pending: Mutex::new(Pending::default()),
                rules: ports.rules,
                settings: ports.settings,
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
            // Die drei Garantien werden an der laufenden Sandbox gemessen.
            // Das ist blockierende Arbeit — der Bericht des Shims wird mit
            // Frist gelesen — und gehört deshalb auf einen eigenen Faden.
            Some(Op::IsolationCheck(())) => {
                tokio::task::spawn_blocking(move || inner.isolation_check(&tx));
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
    /// Dies ist die einzige Stelle, an der `pending` und `running` ineinander
    /// gehalten werden, und die Reihenfolge ist `pending` vor `running`. Jede
    /// andere Stelle nimmt sie nacheinander (`work_dir`, `work_mode`,
    /// `profile_name`), also gibt es keinen Weg, sie andersherum zu halten.
    fn claim_start(self: &Arc<Self>) -> Option<StartClaim> {
        let mut pending = lock(&self.pending);
        if pending.claimed || self.is_running() {
            return None;
        }
        pending.claimed = true;
        Some(StartClaim {
            inner: Arc::clone(self),
        })
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
    fn remember(&self, plan: &v1::sandbox_request::Plan) -> Result<Option<PathBuf>, Diagnostic> {
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
        // Das Verzeichnis, das für **diesen** Wunsch gilt, gelesen unter
        // demselben Schloss, unter dem es geschrieben wurde. Der Aufrufer
        // trägt es weiter, statt es später noch einmal aus `pending` zu holen:
        // Dazwischen könnte ein anderer Aufruf ein anderes hineingelegt haben,
        // und die Sitzung löste dann gegen ein Verzeichnis auf, das niemand
        // für sie gewählt hat.
        Ok(pending.work_dir.clone())
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
            .config()
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
        let plan = v1::sandbox_request::Plan {
            profile: start.profile.clone(),
            work_dir: start.work_dir.clone(),
            work_mode: start.work_mode.clone(),
        };
        // Der Anspruch zuerst, vor jeder Prüfung, die etwas merkt: Ein
        // zweiter Start soll `pending` nicht anfassen, während der erste
        // daraus seine Sitzung auflöst.
        let Some(_claim) = self.claim_start() else {
            // Der Befund zuerst, dann der Zustand. Die Oberfläche schaltet die
            // Schaltfläche ohnehin ab und sieht ihn selten; `humanitl run`
            // dagegen bekäme sonst nur einen laufenden Zustand ohne Ausgabe
            // und ohne Exit-Code und wüsste nicht, warum (HUM-067).
            let _ = tx
                .send(diagnostic_event(&already_running(&self.running_facts())))
                .await;
            let this = Arc::clone(&self);
            let plan = plan.clone();
            let tx = tx.clone();
            let _ =
                tokio::task::spawn_blocking(move || this.snapshot_or_diagnostic(&plan, &tx)).await;
            return;
        };

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

        let this = Arc::clone(&self);
        let launch_plan = plan.clone();
        let command: Vec<OsString> = start.command.iter().map(OsString::from).collect();
        // Die Ausgabe des Agenten läuft über einen eigenen Kanal, nicht über
        // den gesammelten Puffer: Der Puffer gibt erst nach dem Ende etwas
        // heraus und ist gedeckelt, und ein Mensch, der `humanitl run` tippt,
        // will sehen, was gerade geschieht.
        let (output_tx, output_rx) = std::sync::mpsc::channel();
        let launched = tokio::task::spawn_blocking(move || {
            this.launch(&launch_plan, &command, Some(output_tx))
        })
        .await;
        match launched {
            Ok(Ok(())) => {
                // Die Zeile, die der Log-Reiter zeigt. Der leere Zustand dort
                // verspricht sie („ein Start und ein Stopp schreiben je eine
                // Zeile"), also muss es sie geben; das Terminal des Agenten
                // ist etwas anderes und kommt mit HUM-042.
                if let Some(line) = self.started_line() {
                    let _ = tx.send(log_event(line)).await;
                }
                if !self.check_isolation_or_kill(&plan, &tx).await {
                    return;
                }
                let this = Arc::clone(&self);
                let running_plan = plan.clone();
                let running =
                    tokio::task::spawn_blocking(move || this.snapshot(&running_plan)).await;
                if let Ok(Ok(status)) = running {
                    let _ = tx.send(status_event(status)).await;
                }
                self.stream_output(output_rx, &tx).await;
                self.report_exit(&tx).await;
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
        Ok(diagnostics)
    }

    /// Reicht die Ausgabe des Agenten weiter, bis sein Strom endet.
    ///
    /// Die Bytes laufen durch [`humanitl_core::TerminalFilter`], je Strom
    /// einen: Die Ausgabe eines Agenten erreicht ein Terminal, und ein
    /// Terminal führt aus, was in ihr steht (`BACKLOG.md` 4.2). Ein Filter für
    /// beide Ströme zusammen zerschnitte ihre Folgen gegenseitig.
    ///
    /// Blockierend, denn der Kanal des Lesers ist einer der Standardbibliothek;
    /// das Warten gehört deshalb auf einen eigenen Faden.
    async fn stream_output(
        &self,
        rx: std::sync::mpsc::Receiver<humanitl_sandbox::OutputChunk>,
        tx: &mpsc::Sender<v1::SandboxEvent>,
    ) {
        let tx = tx.clone();
        let _ = tokio::task::spawn_blocking(move || {
            let mut stdout = humanitl_core::TerminalFilter::new();
            let mut stderr = humanitl_core::TerminalFilter::new();
            while let Ok(chunk) = rx.recv() {
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
                    return;
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
                if !rest.is_empty() {
                    let _ = tx.blocking_send(output_event(stream, rest));
                }
            }
        })
        .await;
    }

    /// Meldet den Exit-Code des Agenten, sobald er beendet ist.
    ///
    /// Ein Signal wird nach POSIX-Sitte auf `128 + n` abgebildet; so liest es
    /// jede Shell, und so gibt `humanitl run` es weiter.
    async fn report_exit(&self, tx: &mpsc::Sender<v1::SandboxEvent>) {
        let Some(handle) = self.running_handle() else {
            return;
        };
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
    ///
    /// `output` bekommt jedes Stück Ausgabe, sobald es gelesen ist. Der
    /// Daemon selbst hat kein Terminal; die Bytes gehen über den Ereignisstrom
    /// an den Client, der eines hat, und laufen dabei durch
    /// [`humanitl_core::TerminalFilter`]. Ein PTY ist das nicht — es gibt
    /// keine Eingabe und keine Geometrie, das kommt mit HUM-042.
    fn launch(
        &self,
        plan: &v1::sandbox_request::Plan,
        command: &[OsString],
        output: Option<humanitl_sandbox::OutputSink>,
    ) -> Result<(), Diagnostic> {
        let prepared = self.prepare_with_command(plan, command)?;
        let backend = prepared.backend.clone().with_stdio(StdioMode::Capture);
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
        let handle = launcher.launch(&launch)?;
        drop(launcher);
        let mut running = lock(&self.running);
        *running = Some(Running {
            handle: Arc::new(handle),
            backend,
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
                .config()
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
        let config = self.config();
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
            work_dir: self.work_dir(&config, plan).display().to_string(),
            work_mode: work_mode_name(self.work_mode(&config, plan)).to_owned(),
            started_at: started_at.map(crate::convert::timestamp),
            profile: self.profile_name(&config, plan),
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

    /// Das Backend, das gestartet hat, und sein Handle; beides oder nichts.
    fn running_parts(&self) -> Option<(BwrapBackend, Arc<SandboxHandle>)> {
        lock(&self.running)
            .as_ref()
            .map(|running| (running.backend.clone(), Arc::clone(&running.handle)))
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
        // Eine Aufnahme der Konfiguration für den ganzen Aufbau: Ein Start,
        // der dazwischenkommt, darf nicht die Hälfte einer Kommandozeile
        // ersetzen.
        let config = self.config();
        let policy = MountPolicy::from_paths(&self.paths);
        let name = self.profile_name(&config, plan);
        let (profile_path, profile_origin) = self.profile_path(&name)?;
        let profile = SandboxProfile::load_validated(&profile_path, &policy)?;

        let work_src = self.work_dir(&config, plan);
        let work_mode = self.work_mode(&config, plan);
        let agent = if command.is_empty() {
            self.agent_contribution(&config, &work_src, &profile)?
        } else {
            AgentContribution::default()
        };

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
            command: Self::command(&config, command, &agent.command),
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
        config: &Config,
        work_src: &Path,
        profile: &SandboxProfile,
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

        let ctx = AgentContext::new(self.session, work_src.to_path_buf(), config.llm.clone())
            .with_command_override(
                config
                    .agent
                    .command
                    .as_ref()
                    .map(|parts| parts.iter().map(OsString::from).collect()),
            )
            .with_host_path(self.paths.env().non_empty("PATH").map(OsString::from))
            .with_language(config.ui.language)
            .with_hold(config.hold.clone())
            .with_briefing(config.agent.briefing.clone())
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
    fn profile_name(&self, config: &Config, plan: &v1::sandbox_request::Plan) -> String {
        if !plan.profile.trim().is_empty() {
            return plan.profile.trim().to_owned();
        }
        if let Some(running) = lock(&self.running).as_ref() {
            return running.profile.clone();
        }
        if let Some(chosen) = lock(&self.pending).profile.clone() {
            return chosen;
        }
        config.sandbox.profile.clone()
    }

    /// Das Projektverzeichnis dieses Wunsches, sonst das der laufenden
    /// Sandbox, sonst das zuletzt gewählte, sonst das der Konfiguration.
    fn work_dir(&self, config: &Config, plan: &v1::sandbox_request::Plan) -> PathBuf {
        // Der Wunsch dieses Aufrufs, aber in der Fassung, die `check_work_dir`
        // aufgelöst hat: `remember` hat ihn unmittelbar davor abgelegt. Die
        // rohe Zeichenkette der Leitung wird nie eingehängt — sonst prüfte
        // der Dienst den aufgelösten Pfad und hängte den geschriebenen ein,
        // und die Prüfung sagte über die Einhängung nichts aus.
        if !plan.work_dir.trim().is_empty()
            && let Some(checked) = lock(&self.pending).work_dir.clone()
        {
            return checked;
        }
        if let Some(running) = lock(&self.running).as_ref() {
            return running.work_dir.clone();
        }
        if let Some(chosen) = lock(&self.pending).work_dir.clone() {
            return chosen;
        }
        config
            .sandbox
            .work_dir
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }

    /// Der Modus dieses Wunsches, sonst der der laufenden Sandbox, sonst der
    /// zuletzt gewählte, sonst der der Konfiguration.
    fn work_mode(&self, config: &Config, plan: &v1::sandbox_request::Plan) -> WorkMode {
        match plan.work_mode.trim().to_ascii_lowercase().as_str() {
            "ro" => WorkMode::Ro,
            "rw" => WorkMode::Rw,
            _ => {
                if let Some(running) = lock(&self.running).as_ref() {
                    return running.work_mode;
                }
                lock(&self.pending)
                    .work_mode
                    .unwrap_or(config.sandbox.work_mode)
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

/// Der Befund für einen Start, während schon eine Sitzung läuft.
///
/// Der Daemon führt genau eine; eine zweite bekäme eine Sandbox, die einem
/// anderen Projektverzeichnis gehört. Weder ein Befehl zum Anhängen noch einer
/// zum Beheben steht im Text: `attach` gibt es nicht, und es gibt auch keinen
/// Befehl, der eine fremde Sitzung beendet. Wer sie gestartet hat, beendet sie
/// dort (HUM-067, Nicht-Ziel).
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
/// [`BwrapBackend::isolation_check`] legt zu jedem roten Ergebnis einen an;
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
    /// [`BwrapBackend::isolation_check`] zu jedem roten Ergebnis einen Befund
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
}
