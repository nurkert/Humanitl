//! Der echte gRPC-Dienst über Registry und Halte-Warteschlange (HUM-018).
//!
//! [`IpcServer`] ist die Oberfläche, die Sprint 1 braucht: `GetInfo`,
//! `Subscribe`, `Decide`, `ListFlows`. Jede weitere RPC des Vertrags antwortet
//! mit `UNIMPLEMENTED` und nennt das Issue, das sie bringt — ein leerer
//! Erfolg wäre schlimmer als ein klarer Fehler, weil ein Client ihn für eine
//! Antwort hielte.
//!
//! # Warum hier tonic steht und nicht [`DaemonApi`](crate::DaemonApi)
//!
//! Der Fake benutzt den generischen Dienst [`DaemonService`](crate::DaemonService)
//! über dem Port [`DaemonApi`](crate::DaemonApi). Dieser Dienst kann das nicht:
//! der Port hat für Ströme und für `GetInfo`/`Doctor` keinen Fehlerweg, und
//! genau den braucht `UNIMPLEMENTED`. Der echte Dienst erfüllt deshalb den
//! erzeugten tonic-Trait selbst. Die Token-Prüfung bleibt trotzdem an einer
//! Stelle: sie liegt in [`crate::auth`] und hängt hier als Interceptor vor
//! jeder RPC, damit kein Endpunkt vergessen werden kann.
//!
//! # Zustand
//!
//! Alles im Speicher, nichts persistent (der Recorder kommt mit HUM-026). Der
//! Dienst hält keinen eigenen Zustand: er liest die
//! [`FlowRegistry`] und entscheidet über die
//! [`HoldQueue`]. Beide teilen sich einen einzigen
//! Rundfunk-Kanal; der Dienst öffnet keinen zweiten, sondern nimmt die
//! Registry der Warteschlange ([`HoldQueue::registry`]).

use std::fs::Permissions;
use std::future::Future;
use std::net::Ipv4Addr;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use bytes::Bytes;
use dashmap::DashMap;
use humanitl_audit::RecordKind;
use humanitl_audit::kinds::LlmDiscover;
use humanitl_audit_log::AuditHandle;
use humanitl_config::Config;
use humanitl_config::secret::{ApiKey, KeyRef, KeyStore};
use humanitl_core::diagnostics::codes;
use humanitl_core::ids::SandboxId;
use humanitl_core::{
    BodyRef, Decision, DecisionSource, Diagnostic, FixAction, FlowEvent, FlowId, FlowState,
    HostName, SessionId, Severity,
};
use humanitl_proxy::egress::Egress;
use humanitl_proxy::hold::NotHeld;
use humanitl_proxy::rules_store::RulesStore;
use humanitl_proxy::{
    CONNECT_TIMEOUT, ClientTls, Direct, FlowFilter, FlowRegistry, HoldQueue, LlmProbe, Resolver,
    ResolverPort, Scan, Subnet, Upstream,
};
use humanitl_recorder::{
    Cursor, CursorKey, Dir, FlowDetail as RecordedDetail, FlowQuery, Recorder, SortKey,
};
use humanitl_sandbox::doctor::Probe as DoctorProbe;
use tokio::net::UnixListener;
use tokio::sync::oneshot;
use tokio_stream::StreamExt as _;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::{BroadcastStream, UnixListenerStream};
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use zeroize::Zeroizing;

use crate::audit::AuditService;
use crate::domains::DomainTable;
use crate::rules::RulesService;
use crate::sandbox::SandboxService;
use crate::server_stub::{BoxStream, diagnostic_to_status};
use crate::{
    PROTO_MAJOR, PROTO_MINOR, auth, config_rpc, convert, llm_key, llm_servers, rules, v1, validate,
};

/// Was dieser Daemon in M1 kann.
///
/// Die Liste ist eine Zusage, keine Absichtserklärung: `findings.regex` fehlt,
/// solange die zugehörige RPC `UNIMPLEMENTED` liefert (HUM-025). Ein Client,
/// der danach schaltet, soll nicht in einen Fehler laufen. `sandbox.bwrap`
/// kommt erst mit [`IpcServer::with_sandbox`] dazu, also genau dann, wenn
/// dieser Daemon wirklich eine Sandbox planen und starten kann (HUM-040).
pub const CAPABILITIES: &[&str] = &["hold", "proxy.h1"];

/// Vorgabe für `ListFlowsRequest.limit`, wie im Vertrag beschrieben.
pub const DEFAULT_PAGE_LIMIT: usize = 200;

/// Obergrenze für `ListFlowsRequest.limit`, wie im Vertrag beschrieben.
pub const MAX_PAGE_LIMIT: u32 = 1000;

/// So viele Bytes trägt ein Stück eines Bodys über die Leitung (`GetBody`).
///
/// Groß genug, damit ein Body von einigen Megabyte nicht in tausend Nachrichten
/// zerfällt, klein genug, damit der erste Teil sofort beim Client ist. Die
/// Zahl steht in [`crate::body`], weil der Fake dieselbe braucht.
pub use crate::body::BODY_CHUNK_BYTES;

/// So lange darf eine offene Verbindung den Abbau nach dem Signal aufhalten.
///
/// Nach `SIGTERM` nimmt der Dienst nichts Neues mehr an und lässt laufende
/// Aufrufe zu Ende gehen. Ohne Frist hinge er an einem Client, der seine
/// Verbindung offen lässt — `systemctl stop humanitld` würde dann warten, bis
/// die Oberfläche sich schließt. Nach dieser Frist endet der Dienst, und
/// Socket und Token verschwinden in jedem Fall.
pub const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

/// Der gRPC-Dienst des echten Daemons.
///
/// Billig zu bauen und an eine Laufzeit zu hängen; alles Geteilte liegt hinter
/// `Arc`.
#[derive(Debug)]
pub struct IpcServer {
    registry: Arc<FlowRegistry>,
    queue: Arc<HoldQueue>,
    info: v1::Info,
    body_cap_bytes: u64,
    /// `limits.preview_cap_bytes` und `limits.max_decompress_ratio`: das
    /// Budget, mit dem `GetBody` einen Body für `decoded = true` entpackt.
    /// Dasselbe wie beim Scan, damit ein Rumpf, den die Detektoren gesehen
    /// haben, in der Oberfläche genauso weit reicht (HUM-119).
    decode_cap_bytes: usize,
    max_decompress_ratio: u32,
    rules: Option<RulesService>,
    recorder: Option<Recorder>,
    domains: Option<Arc<DomainTable>>,
    /// Die host-seitige Probe des LLM-Endpunkts (HUM-039).
    ///
    /// Sie hängt am Dienst und nicht am Proxy, weil sie nichts mit einem Fluss
    /// zu tun hat: Sie beantwortet eine Frage, die ein Mensch im Setup stellt,
    /// bevor überhaupt eine Sandbox läuft. `None` nur, wenn sich der
    /// Verbindungsstapel aus der Konfiguration nicht bauen ließ; der Grund
    /// steht dann in `llm_probe_error` und geht als Antwort an den Aufrufer,
    /// statt still zu verschwinden.
    llm_probe: Option<Arc<LlmProbe>>,
    llm_probe_error: Option<Diagnostic>,
    /// Die Sandbox dieser Sitzung: Einhängungen, Umgebung, Kommandozeile,
    /// Start und Stopp (HUM-040). `None` in einem Daemon, der ohne Sandbox
    /// läuft; `Sandbox` antwortet dann mit `IPC_006` statt mit einer
    /// erfundenen Momentaufnahme.
    sandbox: Option<SandboxService>,
    /// Wohin Regeländerungen und Netzsuchen als Audit-Records gehen (HUM-050).
    /// `None` im Fake und in Tests ohne Audit-Log.
    audit: Option<AuditHandle>,
    /// Wer die `Audit`-RPC beantwortet (HUM-156). `None` in einem Daemon ohne
    /// Audit-Log; `Audit` antwortet dann mit `IPC_006` wie der Fake, statt
    /// eine leere, heile Kette zu behaupten.
    audit_log: Option<AuditService>,
    /// Woraus `Doctor` seinen Bericht baut (HUM-075).
    doctor: DoctorSetup,
    bodies: BodyIndex,
    /// Wo der Zugangsschlüssel des Modells liegt (HUM-140). `None` heißt:
    /// der Speicher der Pfade dieses Dienstes ([`KeyStore::for_paths`]);
    /// Tests setzen eine Attrappe ein.
    keys: Option<KeyStore>,
}

/// Was der Doctor über diese Installation wissen muss.
///
/// Drei Werte aus der Konfiguration und die Pfade dieses Prozesses. Sie liegen
/// hier und nicht in der Prüfung, weil die Prüfung nichts über Konfiguration
/// wissen soll: Sie liest die Maschine, und wonach sie sucht, sagt ihr der,
/// der sie ruft.
#[derive(Debug, Clone)]
struct DoctorSetup {
    /// Die Pfade und die Umgebung dieses Daemons.
    paths: humanitl_config::Paths,
    /// `agent.adapter`.
    adapter: String,
    /// Das erste Wort von `agent.command`, wenn es die Kommandozeile ersetzt.
    agent_command: Option<String>,
    /// `llm.endpoint`, als Text.
    llm_endpoint: Option<String>,
    /// Die Befunde der letzten Ladung; darunter der eines `llm.endpoint`,
    /// den sie beiseitegelegt hat (HUM-283).
    load_diagnostics: Vec<Diagnostic>,
}

impl DoctorSetup {
    /// The same setup with the configuration as it stands on disk now
    /// (HUM-277).
    ///
    /// The values above are those of the daemon's start; after `SetConfig`
    /// the `llm` line still said "llm.endpoint is not set" until the daemon
    /// restarted. Only when the configuration cannot be resolved do the start
    /// values stay. Blocking: it reads files.
    fn refreshed(mut self) -> Self {
        if let Ok(now) = humanitl_config::resolve(
            &humanitl_config::ProfileSelection::default(),
            None,
            self.paths.env(),
            &[],
        ) {
            self.adapter = now.config.agent.adapter;
            self.agent_command = now
                .config
                .agent
                .command
                .and_then(|words| words.into_iter().next());
            self.llm_endpoint = now.config.llm.endpoint.as_ref().map(ToString::to_string);
            self.load_diagnostics = now.diagnostics;
        }
        self
    }

    /// Was der Daemon über den Endpunkt des Sprachmodells sagen darf.
    ///
    /// Nie mehr als „er steht in der Konfiguration": Gemessen wird er über
    /// `ProbeLlm`, auf ausdrücklichen Wunsch eines Menschen.
    fn llm_facts(&self) -> humanitl_sandbox::doctor::LlmFacts {
        humanitl_sandbox::doctor::LlmFacts::from_config(
            self.llm_endpoint.clone(),
            &self.load_diagnostics,
            humanitl_sandbox::doctor::PROBE_LLM_COMMAND,
        )
    }
}

/// Wo ein Body liegt, den dieser Dienst schon einmal ausgeliefert hat.
///
/// `GetBody` bekommt nur einen [`v1::BodyRef`], also Prüfsumme und Größe. Ein
/// großer Body steht damit fest: Er liegt unter seiner Prüfsumme im
/// Blob-Speicher. Ein kleiner steht als Spalte in der Zeile seiner Nachricht,
/// und die findet man ohne Flow und Richtung nicht wieder — die Aufzeichnung
/// kennt keinen Weg von der Prüfsumme zur Nachricht (siehe Bericht zu
/// HUM-026).
///
/// Deshalb merkt sich der Dienst beim Ausliefern eines `FlowDetail`, zu
/// welchem Flow und welcher Richtung jede Prüfsumme gehört. Der Weg des
/// Klienten ist ohnehin `GetFlow` und dann `GetBody`: Vorher kennt er keinen
/// [`v1::BodyRef`], den er verlangen könnte. Gespeichert werden nur die
/// Kennungen, nie Inhalt; ein Eintrag kostet gut fünfzig Bytes.
type BodyIndex = DashMap<[u8; 32], (FlowId, Dir)>;

impl IpcServer {
    /// Der Dienst über einer Warteschlange und ihrer Registry.
    ///
    /// Die Registry kommt aus [`HoldQueue::registry`]: Warteschlange und
    /// Verzeichnis teilen sich einen Rundfunk-Kanal, und ein zweiter Kanal
    /// würde die Reihenfolge der Ereignisse je Flow zerreißen (HUM-016).
    #[must_use]
    pub fn new(queue: Arc<HoldQueue>, config: &Config, session: Option<SessionId>) -> Self {
        let registry = Arc::clone(queue.registry());
        let probe = build_llm_probe(config).map(Arc::new);
        if let Err(diagnostic) = probe.as_ref() {
            tracing::warn!(
                code = %diagnostic.code,
                why = %diagnostic.why,
                "the LLM endpoint probe is not available in this daemon"
            );
        }
        Self {
            registry,
            queue,
            info: v1::Info {
                daemon_version: env!("CARGO_PKG_VERSION").to_owned(),
                proto_major: PROTO_MAJOR,
                proto_minor: PROTO_MINOR,
                capabilities: CAPABILITIES.iter().map(|name| (*name).to_owned()).collect(),
                session_id: session.map(|id| id.to_string()).unwrap_or_default(),
            },
            body_cap_bytes: config.limits.hold_body_cap_bytes,
            decode_cap_bytes: usize::try_from(config.limits.preview_cap_bytes)
                .unwrap_or(usize::MAX),
            max_decompress_ratio: config.limits.max_decompress_ratio,
            rules: None,
            recorder: None,
            domains: None,
            llm_probe: probe.as_ref().ok().map(Arc::clone),
            llm_probe_error: probe.err(),
            sandbox: None,
            audit: None,
            audit_log: None,
            doctor: DoctorSetup {
                paths: humanitl_config::Paths::from_process(),
                adapter: config.agent.adapter.clone(),
                agent_command: config
                    .agent
                    .command
                    .as_ref()
                    .and_then(|words| words.first())
                    .cloned(),
                llm_endpoint: config.llm.endpoint.as_ref().map(ToString::to_string),
                load_diagnostics: Vec::new(),
            },
            bodies: BodyIndex::new(),
            keys: None,
        }
    }

    /// Derselbe Dienst mit diesem Speicher für den Zugangsschlüssel; für
    /// Tests, die den Schlüsselbund des Menschen nicht anfassen dürfen.
    #[must_use]
    pub fn with_key_store(mut self, keys: KeyStore) -> Self {
        self.keys = Some(keys);
        self
    }

    /// Der Speicher des Zugangsschlüssels, gesetzt oder aus den Pfaden.
    fn key_store(&self) -> KeyStore {
        self.keys
            .clone()
            .unwrap_or_else(|| KeyStore::for_paths(&self.doctor.paths))
    }

    /// Der Schlüssel, den eine Probe mitschickt (HUM-140).
    ///
    /// Ein Schlüssel im Auftrag geht an genau den Endpunkt des Auftrags: Der
    /// Mensch hat ihn eben eingetippt, und gespeichert wird er hier nicht. Der
    /// hinterlegte geht nur an einen Endpunkt mit derselben Herkunft wie
    /// `llm.endpoint`, also wie der aktive Server; ein Client, der einen
    /// anderen Host nennt, bekommt die Probe ohne Schlüssel, und `key_sent`
    /// sagt es. Nennt der Auftrag einen gespeicherten Server (`server`,
    /// HUM-289), gilt dasselbe mit dessen Endpunkt und dessen Schlüssel: Ein
    /// Schlüssel geht nie an die Herkunft eines anderen Servers.
    ///
    /// # Errors
    ///
    /// `CONFIG_020` für einen Schlüssel im Auftrag, der keiner ist,
    /// `CONFIG_021` für einen Server, den es nicht gibt, und `LLM_009`, wenn
    /// der hinterlegte Schlüssel verlangt, aber nicht zu holen ist.
    async fn probe_key(
        &self,
        endpoint: &url::Url,
        typed: &str,
        use_stored_key: bool,
        server: &str,
    ) -> Result<Option<ApiKey>, Diagnostic> {
        if !typed.is_empty() {
            return ApiKey::from_pasted(typed).map(Some);
        }
        if !use_stored_key {
            return Ok(None);
        }
        let paths = self.doctor.paths.clone();
        let keys = self.key_store();
        let endpoint = endpoint.clone();
        let server = server.trim().to_owned();
        tokio::task::spawn_blocking(move || {
            let Ok(resolved) = humanitl_config::resolve(
                &humanitl_config::ProfileSelection::default(),
                None,
                paths.env(),
                &[],
            ) else {
                return Ok(None);
            };
            let llm = resolved.config.llm;
            // Welcher Endpunkt und welcher Verweis zusammengehören: ohne
            // Namen der Server, den diese Auflösung wählt (`llm.server` aus
            // `config.toml` oder dem Profil `default`, sonst der aktive; die
            // flachen Schlüssel sind seine Ansicht, HUM-400), mit Namen
            // genau dieser Eintrag. Eine Datei ohne
            // Einträge kennt nur den abgeleiteten Server `default`.
            let (configured, reference) = if server.is_empty()
                || (llm.servers.is_empty() && server == humanitl_config::servers::MIGRATED_NAME)
            {
                // Dieselbe Regel wie beim Start (HUM-289): Der Verweis wirkt
                // nur mit dem Endpunkt desselben Eintrags.
                if !llm.key_goes_with_endpoint() {
                    return Ok(None);
                }
                (llm.endpoint, llm.api_key_ref)
            } else {
                let Some(entry) = llm.servers.get(&server) else {
                    return Err(Diagnostic::builder(
                        humanitl_core::diagnostics::codes::CONFIG_021,
                        humanitl_core::Severity::Error,
                    )
                    .why(format!(
                        "the probe sent no key: there is no server named {server}"
                    ))
                    .fix(humanitl_core::FixAction::CopyCommand(
                        "humanitl llm server ls".to_owned(),
                    ))
                    .build());
                };
                // Ein Verweis, den ein anderer Eintrag auch trägt, wirkt für
                // keinen (HUM-289, Review C).
                let shared = llm
                    .servers
                    .values()
                    .filter(|other| {
                        other.api_key_ref.is_some() && other.api_key_ref == entry.api_key_ref
                    })
                    .count()
                    > 1;
                let endpoint = entry
                    .endpoint
                    .as_deref()
                    .and_then(|text| url::Url::parse(text).ok());
                (endpoint, entry.api_key_ref.clone().filter(|_| !shared))
            };
            let same_origin = configured
                .as_ref()
                .is_some_and(|configured| configured.origin() == endpoint.origin());
            let key_ref = reference.as_deref().and_then(KeyRef::parse);
            match key_ref {
                Some(key_ref) if same_origin => keys.lookup(&key_ref).map(Some),
                _ => Ok(None),
            }
        })
        .await
        .unwrap_or_else(|error| Err(unfinished("ProbeLlm", &error)))
    }

    /// Derselbe Dienst über einer Warteschlange, in der nie etwas hängt.
    ///
    /// Für einen Aufrufer, der nur aus der Aufzeichnung antwortet — `GetFlow`,
    /// `GetBody`, `ListFlows` — und keinen laufenden Proxy hat. Die
    /// Warteschlange gehört sonst dem Proxy, und ohne ihn gibt es nichts zu
    /// halten. Sie steht hier, damit ein solcher Aufrufer sie nicht selbst
    /// bauen und dafür `humanitl-proxy` kennen muss: Die Kopplung an dieses
    /// Crate wächst nicht mehr (`tools/coupling-baseline.toml`, HUM-145).
    #[must_use]
    pub fn over_the_recording(config: &Config, session: Option<SessionId>) -> Self {
        let registry = Arc::new(FlowRegistry::new(&config.limits));
        let queue = Arc::new(HoldQueue::with_registry(&config.limits, registry));
        Self::new(queue, config, session)
    }

    /// Derselbe Dienst über der Sandbox dieser Sitzung (HUM-040).
    ///
    /// Mit ihr beantwortet `Sandbox` die Momentaufnahme aus dem Profil, das
    /// auch startet, und startet und stoppt sie. Ohne sie bleibt es bei
    /// `IPC_006`: ein Daemon, der keine Sandbox hat, soll das sagen, statt
    /// eine leere Tabelle zu zeigen, die wie „nichts eingehängt" aussieht.
    #[must_use]
    pub fn with_sandbox(mut self, sandbox: SandboxService) -> Self {
        self.info.capabilities.push("sandbox.bwrap".to_owned());
        // Further shells in the running sandbox (HUM-299): a client asks
        // before it offers the action.
        self.info.capabilities.push("sandbox.exec".to_owned());
        // The disposable sandbox (HUM-350): a client asks before it offers
        // the switch and the changes panel. Only where the overlay exists.
        if cfg!(target_os = "linux") {
            self.info.capabilities.push("sandbox.disposable".to_owned());
        }
        self.sandbox = Some(sandbox);
        self
    }

    /// Derselbe Dienst über anderen Pfaden als denen des Prozesses.
    ///
    /// Gegen sie prüft `Doctor`, und in ihre `config.toml` schreibt
    /// `SetConfig`. Der Daemon braucht das nicht; Tests setzen hier ein
    /// Wegwerf-Verzeichnis, damit keiner die Konfiguration eines Menschen
    /// berührt.
    #[must_use]
    pub fn with_paths(mut self, paths: humanitl_config::Paths) -> Self {
        self.doctor.paths = paths;
        self
    }

    /// Derselbe Dienst mit einer anderen Probe; für Tests.
    ///
    /// Der Daemon braucht das nicht: [`IpcServer::new`] baut die Probe aus
    /// derselben Konfiguration, aus der auch der Proxy seine Ports baut.
    #[must_use]
    pub fn with_llm_probe(mut self, probe: LlmProbe) -> Self {
        self.llm_probe = Some(Arc::new(probe));
        self.llm_probe_error = None;
        self
    }

    /// Derselbe Dienst, der aus der Aufzeichnung liest.
    ///
    /// Mit ihr beantworten `ListFlows`, `GetFlow` und `GetBody` aus der
    /// Datenbank statt aus dem Speicher: Die Historie überlebt damit einen
    /// Neustart, und die Oberfläche hält nie mehr als eine Seite (ADR-008).
    /// Ohne sie bleibt es bei der Registry der laufenden Sitzung — der Weg,
    /// den der Fake und die Tests gehen.
    #[must_use]
    pub fn with_recorder(mut self, recorder: Recorder) -> Self {
        self.recorder = Some(recorder);
        self
    }

    /// Derselbe Dienst mit dem Domain-Katalog (HUM-031).
    ///
    /// Dieselbe Tabelle, die der Proxy beim Eintreffen eines Flows füllt; hier
    /// wird sie nur gelesen.
    #[must_use]
    pub fn with_domains(mut self, domains: Arc<DomainTable>) -> Self {
        self.domains = Some(domains);
        self
    }

    /// Die Aufzeichnung, sofern eine verdrahtet ist.
    #[must_use]
    pub const fn recorder(&self) -> Option<&Recorder> {
        self.recorder.as_ref()
    }

    /// Derselbe Dienst mit einem Regelspeicher.
    ///
    /// Ohne ihn antwortet `Rules` mit `IPC_005` und `Decide` lehnt ein
    /// `remember` ab, statt es stillschweigend zu verwerfen. Der Recorder ist
    /// die Quelle des Probelaufs; fehlt er, prüft `dry_run` null Flows.
    #[must_use]
    pub fn with_rules(mut self, store: Arc<RulesStore>, recorder: Option<Recorder>) -> Self {
        let service = RulesService::new(store, recorder);
        self.rules = Some(match self.audit.clone() {
            Some(audit) => service.with_audit(audit),
            None => service,
        });
        self
    }

    /// Derselbe Dienst mit dem Audit-Log (HUM-050).
    ///
    /// Dorthin gehen die Änderungen des Regel-RPC (`rule.*`) und jede Suche
    /// nach Sprachmodellen im eigenen Netz (`llm.discover`, HUM-076). Die
    /// Reihenfolge der Bausteine ist gleich: Ein schon verdrahteter
    /// Regel-Dienst bekommt das Log nachgereicht.
    #[must_use]
    pub fn with_audit(mut self, audit: AuditHandle) -> Self {
        if let Some(rules) = self.rules.take() {
            self.rules = Some(rules.with_audit(audit.clone()));
        }
        self.audit = Some(audit);
        self
    }

    /// Derselbe Dienst, der `Audit` beantwortet (HUM-156): Prüfung mit
    /// Schlüssel und Ankern, Ende, Seiten und Export der Kette. Die Fähigkeit
    /// `audit` in `Info` kommt erst hier dazu, wie die Fähigkeit der Sandbox
    /// mit [`IpcServer::with_sandbox`].
    #[must_use]
    pub fn with_audit_log(mut self, service: AuditService) -> Self {
        self.info.capabilities.push("audit".to_owned());
        self.audit_log = Some(service);
        self
    }

    /// Der Regel-Dienst, sobald einer verdrahtet ist.
    #[must_use]
    pub const fn rules_service(&self) -> Option<&RulesService> {
        self.rules.as_ref()
    }

    /// Der Regel-Dienst oder der Befund, dass dieser Daemon keinen hat.
    fn rules_or_refuse(&self) -> Result<&RulesService, Diagnostic> {
        self.rules.as_ref().ok_or_else(rules::no_store)
    }

    /// Die Selbstauskunft, die `GetInfo` liefert.
    #[must_use]
    pub const fn info(&self) -> &v1::Info {
        &self.info
    }

    /// Setzt in ein aufgezeichnetes Detail die Kopfzeilen der Anfrage aus der
    /// Live-Registry, solange über den Flow noch nicht entschieden ist
    /// (HUM-250, aus dem Review von HUM-058).
    ///
    /// Die Aufzeichnung eines gehaltenen Flows kann unvollständig sein: Die
    /// Zeile steht, die Nachricht mit den Kopfzeilen fehlt, weil ihr Schreiben
    /// scheiterte oder noch im Schreib-Thread wartet. Eine Anfrage ohne
    /// Kopfzeilen verschwiege ein `Upgrade: websocket`, und die Oberfläche
    /// hielte sie deshalb auf allen Wegen gehalten (`requestIsReadable`). Die
    /// Registry kennt die Anfrage, über die entschieden wird, vollständig.
    ///
    /// Sie verlässt den Daemon in derselben Form, in der sie aus der
    /// Aufzeichnung käme: jedes Geheimnis der Masken als `••••`
    /// ([`Recorder::masked_headers`], HUM-140). Ziel und Pfad bleiben die der
    /// Zeile, die schon maskiert sind; ein fehlender Body-Verweis kommt aus
    /// der Registry.
    fn with_live_request(
        &self,
        recorder: &Recorder,
        id: FlowId,
        mut detail: v1::FlowDetail,
    ) -> v1::FlowDetail {
        let Some(record) = self.registry.get(id) else {
            return detail;
        };
        if !matches!(
            record.state,
            FlowState::Received | FlowState::Analyzed { .. } | FlowState::Held { .. }
        ) {
            return detail;
        }
        let live = masked_request(recorder, &record.request);
        let request = detail.request.get_or_insert_with(Default::default);
        request.headers = live.headers;
        if request.body.is_none() {
            request.body = live.body;
        }
        detail
    }

    /// Das Detail eines Flows, den nur die Registry kennt, weil seine Zeile
    /// nie geschrieben wurde: der Rückfall von `GetFlow`, und während einer
    /// Störung der Aufzeichnung der übliche Weg (HUM-250, Review).
    ///
    /// Mit Aufzeichnung verlässt es den Daemon in derselben Form, in der es
    /// aus der Aufzeichnung käme: Ziel, Pfad, Kopfzeilen und die Angaben zum
    /// Body gehen durch die Masken (HUM-140), ebenso das `why` jedes Befunds
    /// (HUM-256). Ohne Aufzeichnung gibt es keine
    /// Masken, und das Detail bleibt, wie die Registry es kennt.
    fn registry_detail(
        &self,
        mut detail: v1::FlowDetail,
        request: &humanitl_core::HttpRequest,
        diagnostics: &[Diagnostic],
    ) -> v1::FlowDetail {
        if let Some(recorder) = self.recorder.as_ref() {
            detail.request = Some(masked_request(recorder, request));
            // Die Befunde wie aus der Aufzeichnung, `why` maskiert (HUM-256).
            detail.diagnostics = diagnostics
                .iter()
                .map(|diagnostic| masked_diagnostic(recorder, diagnostic))
                .collect();
            let masks = recorder.masks();
            if let Some(summary) = detail.summary.as_mut() {
                summary.path = masks.apply(&summary.path).into_owned();
                // Die registrierbare Domain entsteht aus dem Hostnamen; ein
                // Schlüssel als Label stünde dort sonst im Klartext (Review).
                summary.apex = masks.apply_host(&summary.apex).into_owned();
                if let Some(authority) = summary.authority.as_mut() {
                    mask_authority(&masks, authority);
                }
            }
            if let Some(domain) = detail.domain.as_mut() {
                domain.apex = masks.apply_host(&domain.apex).into_owned();
            }
        }
        detail
    }

    /// Führt die Befunde der Aufzeichnung mit denen der Registry zusammen
    /// (HUM-057, HUM-256).
    ///
    /// Seit HUM-256 stehen die Befunde eines Flusses in der Aufzeichnung und
    /// kommen nach einem Neustart des Daemons von dort. Die Registry bleibt
    /// der Rückfall für die laufende Sitzung: Der Schreib-Thread bündelt, eine
    /// Zeile kann scheitern, und ein Befund kann in der einen Liste stehen und
    /// in der anderen nicht. Die Länge allein sagt deshalb nicht, welche
    /// Liste die vollständige ist; zwei gleich lange Listen können
    /// Verschiedenes enthalten. Zusammengeführt wird nach Gleichheit der
    /// Befunde in der Wire-Form ([`merge_diagnostics`]).
    ///
    /// Aus der Registry verlassen sie den Daemon in der Form, in der sie aus
    /// der Aufzeichnung kämen: `why` maskiert ([`Recorder::masked_why`]).
    fn with_live_diagnostics(
        &self,
        recorder: &Recorder,
        id: FlowId,
        mut detail: v1::FlowDetail,
    ) -> v1::FlowDetail {
        // Die Masken von jetzt auch über die aufgezeichneten Befunde: Ein
        // Schlüssel, der nach dem Schreiben dazukam, stünde sonst dort im
        // Klartext, und dieselbe Meldung aus der Registry wäre nicht mehr
        // gleich und erschiene doppelt (HUM-256, Review).
        for diagnostic in &mut detail.diagnostics {
            diagnostic.why = humanitl_core::sanitize_note(&recorder.masked_why(&diagnostic.why));
        }
        if let Some(record) = self.registry.get(id) {
            let live: Vec<v1::Diagnostic> = record
                .diagnostics
                .iter()
                .map(|diagnostic| masked_diagnostic(recorder, diagnostic))
                .collect();
            detail.diagnostics = merge_diagnostics(&detail.diagnostics, &live);
        }
        detail
    }

    /// Der Ereignisstrom eines Abonnenten.
    ///
    /// Ein zu langsamer Zuhörer verliert die ältesten Ereignisse; der
    /// Rundfunk meldet das als `Lagged`. Daraus wird ein reguläres
    /// [`FlowEvent::Lagged`] — dasselbe, was
    /// [`humanitl_proxy::hold::next_event`] einem Zuhörer ohne Strom liefert.
    /// Der Strom endet dabei nicht: der Client lädt mit `ListFlows` nach.
    fn event_stream(
        &self,
        request: &v1::SubscribeRequest,
    ) -> BoxStream<Result<v1::FlowEvent, Status>> {
        let registry = Arc::clone(&self.registry);
        let domains = self.domains.clone();
        let include_passthrough = request.include_passthrough;
        let recorder = self.recorder.clone();
        let live = BroadcastStream::new(self.registry.subscribe()).map(move |item| {
            let event = match item {
                Ok(event) => event,
                Err(BroadcastStreamRecvError::Lagged(n)) => {
                    log_lagged(n);
                    // Unter den verlorenen Ereignissen kann der eine Befund
                    // sein, mit dem eine Störung der Aufzeichnung begann; er
                    // kommt nur einmal. Hinter `Lagged` folgt deshalb, was
                    // noch gestört ist, damit kein Client grün bleibt, während
                    // nichts aufgezeichnet wird (HUM-250, Review). War der
                    // erste Befund noch nicht verloren, kommt er so doppelt;
                    // das ist gewollt und harmlos, denn die Oberfläche hält
                    // die Störung als Zustand, und ein zweites
                    // `RECORDER_003` ändert ihn nicht (`recorder_health.dart`).
                    let mut events = vec![Ok(convert::flow_event_to_proto(
                        &FlowEvent::Lagged { n },
                        &registry,
                        domains.as_deref(),
                    ))];
                    events.extend(outage_events(
                        recorder.as_ref(),
                        &registry,
                        domains.as_deref(),
                    ));
                    return events;
                }
            };
            // Ereignisse eines Durchreich-Flows (LLM-Passthrough) sieht nur,
            // wer sie ausdruecklich verlangt; `ListFlows` und der Rueckstand
            // filtern genauso, damit der Strom nicht stiller ist als die
            // Liste.
            //
            // Ein Befund ist davon ausgenommen, und zwar immer. `LLM_005`
            // warnt vor genau der Anfrage, die hier versteckt wird; ihn
            // mitzuverstecken kehrte die Zusage aus `docs/SECURITY.md` 3.1 um
            // („ein Treffer erzeugt eine Warnung") und liesse den Menschen
            // ohne die eine Meldung zurueck, die er sehen muss. Eingeklappt
            // heisst nicht stumm (HUM-039).
            let hidden = !include_passthrough
                && !matches!(event, FlowEvent::Diagnostic { .. })
                && event.flow_id().is_some_and(|id| {
                    registry.get(id).is_some_and(|record| {
                        convert::record_to_summary(&record, domains.as_deref()).passthrough
                    })
                });
            if hidden {
                return Vec::new();
            }
            vec![Ok(convert::flow_event_to_proto(
                &event,
                &registry,
                domains.as_deref(),
            ))]
        });
        // Erst zusammenführen, dann auflösen: `Lagged` und die nachgelieferte
        // Störung sind ein Stück und gehen als eines durch `merge`. Würde
        // vorher aufgelöst, könnte eine Regeländerung zwischen beide fallen
        // (HUM-250, Review Runde 2).
        let merged: Pin<Box<dyn tokio_stream::Stream<Item = Vec<_>> + Send>> =
            match self.rules.as_ref().map(RulesService::store) {
                None => Box::pin(live),
                Some(store) => Box::pin(
                    live.merge(rules_changed_stream(Arc::clone(store)).map(|event| vec![event])),
                ),
            };
        Box::pin(tokio_stream::iter(self.backlog(request)).chain(Flatten::new(merged)))
    }

    /// Was ein Abonnent vor dem Strom nachgeliefert bekommt.
    ///
    /// Zuerst die Befunde einer laufenden Störung der Aufzeichnung (HUM-250):
    /// Sie gingen einmal in den Strom, als die Störung begann, und ein Client,
    /// der sich danach verbindet, hätte sie sonst nie gesehen. Er zeigte dann
    /// „alles wird aufgezeichnet", während nichts ankommt.
    ///
    /// Danach die Flows: `since_flow_id` heißt „ich kenne alles bis hierher".
    /// Für jeden jüngeren Flow kommt ein `Received`; den Rest holt der Client
    /// über `ListFlows`. Genau so verhält sich der Fake, damit die Oberfläche
    /// keinen Unterschied sieht.
    fn backlog(&self, request: &v1::SubscribeRequest) -> Vec<Result<v1::FlowEvent, Status>> {
        let mut events = outage_events(
            self.recorder.as_ref(),
            &self.registry,
            self.domains.as_deref(),
        );
        events.extend(self.received_since(request));
        events
    }

    /// Ein `Received` für jeden Flow nach `since_flow_id`.
    fn received_since(&self, request: &v1::SubscribeRequest) -> Vec<Result<v1::FlowEvent, Status>> {
        if request.since_flow_id.is_empty() {
            return Vec::new();
        }
        let Ok(since) = FlowId::parse(&request.since_flow_id) else {
            return Vec::new();
        };
        self.summaries()
            .into_iter()
            .filter(|summary| request.include_passthrough || !summary.passthrough)
            .filter(|summary| FlowId::parse(&summary.flow_id).is_ok_and(|id| id > since))
            .map(|summary| {
                Ok(v1::FlowEvent {
                    at: summary.received_at,
                    event: Some(v1::flow_event::Event::Received(v1::flow_event::Received {
                        summary: Some(summary),
                        domain: None,
                    })),
                })
            })
            .collect()
    }

    /// Alle bekannten Flows als Zeilen, nach Ankunft aufsteigend.
    ///
    /// Die Registry sortiert für ihre eigene Sicht die wartenden Flows nach
    /// Frist nach vorn (HUM-016). Die Liste des Vertrags ist dagegen nach
    /// Ankunft geordnet, weil `cursor` und `since_flow_id` auf Flow-Ids
    /// zeigen; [`FlowId`] ist ein UUID der Fassung 7 und damit selbst die Zeitachse.
    fn summaries(&self) -> Vec<v1::FlowSummary> {
        let mut rows = self.registry.list(&FlowFilter::default());
        rows.sort_unstable_by_key(|row| row.id);
        rows.into_iter()
            .filter_map(|row| self.registry.get(row.id))
            .map(|record| convert::record_to_summary(&record, self.domains.as_deref()))
            .collect()
    }

    /// Eine Seite der Flow-Historie.
    fn page(&self, request: &v1::ListFlowsRequest, descending: bool) -> v1::FlowPage {
        let since = FlowId::parse(&request.since_flow_id).ok();
        let cursor = FlowId::parse(&request.cursor).ok();
        // Der Cursor zeigt auf das letzte gelieferte Element; die nächste Seite
        // liegt in Sortierrichtung dahinter, bei absteigender Reihenfolge also
        // davor.
        let mut flows: Vec<v1::FlowSummary> = self
            .summaries()
            .into_iter()
            .filter(|summary| request.include_passthrough || !summary.passthrough)
            .filter(|summary| convert::matches_filter(summary, &request.filter))
            .filter(|summary| convert::after(summary, since))
            .filter(|summary| {
                if descending {
                    convert::before(summary, cursor)
                } else {
                    convert::after(summary, cursor)
                }
            })
            .collect();
        if descending {
            flows.reverse();
        }

        let total = u64::try_from(flows.len()).unwrap_or(u64::MAX);
        let limit = match request.limit {
            0 => DEFAULT_PAGE_LIMIT,
            other => usize::try_from(other.min(MAX_PAGE_LIMIT)).unwrap_or(DEFAULT_PAGE_LIMIT),
        };
        let next_cursor = if flows.len() > limit {
            flows
                .get(limit - 1)
                .map(|summary| summary.flow_id.clone())
                .unwrap_or_default()
        } else {
            String::new()
        };
        flows.truncate(limit);
        v1::FlowPage {
            flows,
            next_cursor,
            total,
            // Die Registry zaehlt, was sie hat: die Zahl ist exakt.
            capped: false,
        }
    }

    /// Eine Seite der Flow-Historie aus der Aufzeichnung.
    ///
    /// Gefiltert, sortiert und geschnitten wird in `SQLite`; hier wird nur
    /// übersetzt. Vor dem Lesen steht ein [`Recorder::flush`]: Der Schreiber
    /// bündelt Schreibvorgänge, und eine Liste, die den gerade entschiedenen
    /// Flow noch nicht kennt, wäre für den Menschen ein verschwundener Flow.
    ///
    /// `since_flow_id` wird auf der Seite angewandt, nicht in der Abfrage: Die
    /// Aufzeichnung blättert nach Zeit, und der Anker ist eine Flow-Id. Weil
    /// eine [`FlowId`] ein UUID der Fassung 7 ist, ist ihre Ordnung die der
    /// Ankunft; die Zeilen davor fallen damit richtig weg. `total` bleibt die
    /// Zahl, die der Filter trifft.
    async fn recorded_page(
        &self,
        recorder: &Recorder,
        request: &v1::ListFlowsRequest,
    ) -> Result<v1::FlowPage, Diagnostic> {
        let query = flow_query(request)?;
        recorder.flush().await;
        let page = recorder
            .list_flows(&query)
            .await
            .map_err(humanitl_recorder::RecorderError::into_diagnostic)?;
        let since = FlowId::parse(&request.since_flow_id).ok();
        let flows = page
            .rows
            .iter()
            .filter(|row| since.is_none_or(|anchor| row.id > anchor))
            .map(|row| {
                let mut summary = convert::recorded_summary_to_proto(row);
                // Die harte Sperre kennt nur die laufende Sitzung, wie
                // `findings_truncated`: Die Tabelle `flows` führt sie nicht, und
                // ein Client, der nach der Ansage im Strom nachlädt, muss sie in
                // der Zeile finden (HUM-159).
                summary.send_refusal = self
                    .registry
                    .get(row.id)
                    .and_then(|record| record.send_refusal)
                    .as_ref()
                    .map(convert::diagnostic_to_proto);
                summary
            })
            .collect();
        Ok(v1::FlowPage {
            flows,
            next_cursor: page.next.as_ref().map(encode_cursor).unwrap_or_default(),
            total: page.total_estimate,
            // Der Recorder zaehlt nur bis zu seiner Obergrenze; ist sie
            // erreicht, ist `total` eine Untergrenze und die Oberflaeche muss
            // das sagen statt es zu raten (CONVENTIONS 4.13).
            capped: page.capped,
        })
    }

    /// Ein Flow mit allem, was zu ihm aufgezeichnet wurde.
    async fn recorded_detail(
        &self,
        recorder: &Recorder,
        id: FlowId,
    ) -> Result<Option<v1::FlowDetail>, Diagnostic> {
        recorder.flush().await;
        let Some(detail) = recorder
            .get_flow(id)
            .await
            .map_err(humanitl_recorder::RecorderError::into_diagnostic)?
        else {
            return Ok(None);
        };
        for message in &detail.messages {
            self.bodies.insert(message.body.sha256, (id, message.dir));
        }
        let preview = preview_of(recorder, &detail).await;
        Ok(Some(convert::recorded_detail_to_proto(
            &detail,
            self.domain_of(id, &detail.summary.host),
            // Nur die Registry der laufenden Sitzung weiß, ob der Scan die
            // ganze Anfrage gesehen hat; die Tabelle `flows` führt die Spalte
            // nicht (siehe Bericht zu HUM-026).
            self.registry.get(id).is_some_and(|r| r.findings_truncated),
            preview,
        )))
    }

    /// Die abgelegte Zusammenfassung eines Sandbox-Laufs (HUM-043).
    ///
    /// Sie kommt aus der Aufzeichnung und nicht aus dem laufenden Dienst: Ein
    /// Lauf, der längst beendet ist, hat keinen Zustand mehr, den man fragen
    /// könnte, und `humanitl sessions summary <id>` fragt gerade nach so einem.
    /// Der Text in der Zeile ist die serialisierte
    /// [`humanitl_sandbox::summary::SessionSummary`]; die Befunde
    /// (`SANDBOX_022` bis `SANDBOX_026`) rechnet die Übersetzung daraus, damit
    /// die Kommandozeile ein dünner Client bleibt (ADR-018).
    async fn stored_summary(
        recorder: &Recorder,
        sandbox: SandboxId,
    ) -> Result<v1::SessionSummary, Diagnostic> {
        // Die Zusammenfassung entsteht am Ende eines Laufs und geht über
        // denselben Schreib-Thread wie alles andere; ohne `flush` fände ein
        // Client, der sofort nach dem Ereignis fragt, seine eigene Zeile nicht.
        recorder.flush().await;
        let row = recorder
            .get_session_summary(sandbox)
            .await
            .map_err(humanitl_recorder::RecorderError::into_diagnostic)?
            .ok_or_else(|| unknown_summary(sandbox))?;
        let summary: humanitl_sandbox::summary::SessionSummary = serde_json::from_str(&row.json)
            .map_err(|error| {
                Diagnostic::builder(codes::RECORDER_003, Severity::Error)
                    .why(format!(
                        "the stored summary of sandbox {sandbox} cannot be read back ({error}); \
                         the recording holds a text that this version does not understand"
                    ))
                    .build()
            })?;
        Ok(convert::session_summary_to_proto(&summary, row.created))
    }

    /// Was der Katalog zu diesem Flow sagt.
    ///
    /// Für einen Flow dieser Sitzung die Antwort, die beim Eintreffen entstand;
    /// für einen älteren dieselbe Auskunft ohne die Zähler, denn die gehören
    /// dieser Sitzung. Ohne Katalog bleibt das Feld leer, statt einen Apex zu
    /// raten.
    fn domain_of(&self, id: FlowId, host: &str) -> Option<v1::DomainInfo> {
        let domains = self.domains.as_ref()?;
        let info = domains.get(id).or_else(|| {
            HostName::parse(host)
                .ok()
                .map(|host| domains.describe(&host))
        })?;
        Some(convert::domain_to_proto(&info))
    }

    /// Die Bytes hinter einem [`v1::BodyRef`].
    ///
    /// Zwei Wege, und der erste ist der genaue: Kennt der Dienst die Nachricht,
    /// zu der die Prüfsumme gehört ([`BodyIndex`]), liest er sie über deren
    /// Verweis und bekommt damit auch einen Body, der in der Datenbank steht.
    /// Sonst bleibt der Blob-Speicher, in dem jeder große Body unter seiner
    /// Prüfsumme liegt.
    async fn read_body(
        &self,
        recorder: &Recorder,
        wire: &v1::BodyRef,
    ) -> Result<Bytes, Diagnostic> {
        let sha256 = body_hash(wire)?;
        if let Some(entry) = self.bodies.get(&sha256) {
            let (flow, dir) = *entry.value();
            drop(entry);
            recorder.flush().await;
            let detail = recorder
                .get_flow(flow)
                .await
                .map_err(humanitl_recorder::RecorderError::into_diagnostic)?;
            if let Some(message) = detail
                .as_ref()
                .and_then(|detail| detail.messages.iter().find(|m| m.dir == dir))
                && message.body.sha256 == sha256
            {
                return recorder
                    .read_body(&message.body)
                    .await
                    .map_err(humanitl_recorder::RecorderError::into_diagnostic);
            }
        }
        let body = BodyRef {
            sha256,
            size: wire.size,
            inline: None,
            content_type: (!wire.content_type.is_empty()).then(|| wire.content_type.clone()),
            truncated: wire.truncated,
        };
        recorder
            .read_body(&body)
            .await
            .map_err(humanitl_recorder::RecorderError::into_diagnostic)
    }

    /// Entscheidet genau einen Flow.
    ///
    /// Der Befund kommt zusätzlich zum Ergebnis zurück, weil `Decide`
    /// ihn braucht, falls die Anfrage als Ganzes nichts bewirkt hat.
    ///
    /// `acknowledged` sind die bestätigten Funde aus
    /// `DecideRequest.acknowledged_findings`; [`validate::decide_plan`] hat
    /// schon geprüft, dass sie nur mit `allow` und einer Id kommen (HUM-160).
    fn decide_one(
        &self,
        text: &str,
        decision: &Decision,
        acknowledged: &[u32],
    ) -> (v1::DecideResult, Option<Diagnostic>) {
        let id = match validate::flow_id(text) {
            Ok(id) => id,
            Err(diagnostic) => return refused(text, diagnostic),
        };
        match self.queue.decide_acknowledging(
            id,
            decision.clone(),
            DecisionSource::User,
            acknowledged,
        ) {
            Ok(()) => (
                v1::DecideResult {
                    flow_id: text.to_owned(),
                    applied: true,
                    diagnostic: None,
                },
                None,
            ),
            Err(error) => refused(text, not_held(&error)),
        }
    }
}

/// Die Befunde einer laufenden Störung der Aufzeichnung als Ereignisse des
/// Stroms, ältester zuerst; leer ohne Aufzeichnung oder ohne Störung
/// (HUM-250).
fn outage_events(
    recorder: Option<&Recorder>,
    registry: &FlowRegistry,
    domains: Option<&DomainTable>,
) -> Vec<Result<v1::FlowEvent, Status>> {
    recorder
        .map(Recorder::outage)
        .unwrap_or_default()
        .into_iter()
        .map(|(at, diagnostic)| {
            Ok(convert::flow_event_to_proto(
                &FlowEvent::Diagnostic {
                    flow_id: None,
                    at,
                    diagnostic: Box::new(diagnostic),
                },
                registry,
                domains,
            ))
        })
        .collect()
}

/// Ein Strom aus Listen, Element für Element (HUM-250).
///
/// `tokio-stream` hat kein `flat_map`; der Ereignisstrom braucht es, weil auf
/// ein verlorenes Stück (`Lagged`) mehrere Ereignisse folgen können.
struct Flatten<T> {
    inner: Pin<Box<dyn tokio_stream::Stream<Item = Vec<T>> + Send>>,
    pending: std::collections::VecDeque<T>,
}

impl<T> Flatten<T> {
    fn new(inner: Pin<Box<dyn tokio_stream::Stream<Item = Vec<T>> + Send>>) -> Self {
        Self {
            inner,
            pending: std::collections::VecDeque::new(),
        }
    }
}

impl<T: Unpin> tokio_stream::Stream for Flatten<T> {
    type Item = T;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<T>> {
        loop {
            if let Some(item) = self.pending.pop_front() {
                return std::task::Poll::Ready(Some(item));
            }
            match self.inner.as_mut().poll_next(cx) {
                std::task::Poll::Ready(Some(items)) => self.pending.extend(items),
                std::task::Poll::Ready(None) => return std::task::Poll::Ready(None),
                std::task::Poll::Pending => return std::task::Poll::Pending,
            }
        }
    }
}

/// Eine Anfrage aus der Registry in ihrer Wire-Form, so maskiert, wie die
/// Aufzeichnung sie schriebe (HUM-140, HUM-250).
///
/// Kopfzeilen über [`Recorder::masked_headers`], Pfad und Ziel über die
/// Masken der Aufzeichnung, dazu `content_type` und `content_encoding` des
/// Body-Verweises: Ein Geheimnis, das der Agent in eine dieser Stellen
/// schreibt, verließe den Daemon sonst im Klartext, während dieselbe Anfrage
/// aus der Aufzeichnung maskiert käme.
fn masked_request(recorder: &Recorder, request: &humanitl_core::HttpRequest) -> v1::HttpRequest {
    let masks = recorder.masks();
    let mut wire = convert::request_to_proto(request);
    wire.headers = recorder
        .masked_headers(&request.headers)
        .into_iter()
        .map(|[name, value]| v1::Header {
            name,
            value: value.into_bytes(),
        })
        .collect();
    wire.path_and_query = masks.apply(&wire.path_and_query).into_owned();
    if let Some(authority) = wire.authority.as_mut() {
        mask_authority(&masks, authority);
    }
    if let Some(body) = wire.body.as_mut() {
        body.content_type = masks.apply(&body.content_type).into_owned();
        body.content_encoding = masks.apply(&body.content_encoding).into_owned();
    }
    wire
}

/// Zwei Listen von Befunden desselben Flusses als eine, in der Reihenfolge
/// ihres Eintreffens, jeder Befund einmal, höchstens
/// [`humanitl_core::MAX_FLOW_DIAGNOSTICS`] (HUM-256, Review).
///
/// Beide Listen sind Teilfolgen derselben Folge: `HoldQueue::publish` schreibt
/// Registry und Aufzeichnung unter einer Sperre in derselben Reihenfolge, nur
/// kann in jeder ein Befund fehlen. Gleich heißt gleich in der Wire-Form
/// (Code, Schwere, Überschrift, maskiertes `why`, Vorschlag, Adresse); eine
/// eigene laufende Nummer je Befund gibt es nicht. Stehen beide Köpfe gleich
/// da, ist es derselbe Befund; sonst kommt zuerst der Kopf der Aufzeichnung,
/// wenn er in der Registry nicht mehr folgt, und andernfalls der Kopf der
/// Registry, der in der Aufzeichnung fehlt.
fn merge_diagnostics(recorded: &[v1::Diagnostic], live: &[v1::Diagnostic]) -> Vec<v1::Diagnostic> {
    let mut merged = Vec::with_capacity(recorded.len().max(live.len()));
    let (mut i, mut j) = (0, 0);
    while i < recorded.len() && j < live.len() {
        if recorded[i] == live[j] {
            merged.push(recorded[i].clone());
            i += 1;
            j += 1;
        } else if !live[j..].contains(&recorded[i]) {
            merged.push(recorded[i].clone());
            i += 1;
        } else {
            merged.push(live[j].clone());
            j += 1;
        }
    }
    merged.extend_from_slice(&recorded[i..]);
    merged.extend_from_slice(&live[j..]);
    merged.truncate(humanitl_core::MAX_FLOW_DIAGNOSTICS);
    merged
}

/// Ein Befund aus der Registry in seiner Wire-Form, mit dem `why`, das die
/// Aufzeichnung schriebe (HUM-256): Ein Schlüssel im Host oder Pfad, den der
/// Satz nennt, verließe den Daemon sonst im Klartext, während derselbe Befund
/// aus der Aufzeichnung maskiert käme.
fn masked_diagnostic(recorder: &Recorder, diagnostic: &Diagnostic) -> v1::Diagnostic {
    let mut wire = convert::diagnostic_to_proto(diagnostic);
    wire.why = humanitl_core::sanitize_note(&recorder.masked_why(&diagnostic.why));
    wire
}

/// Ziel und Anzeigename durch die Masken, wie `flows.host` und
/// `flows.host_display` in der Aufzeichnung (HUM-140).
fn mask_authority(masks: &humanitl_core::SecretMasks, authority: &mut v1::Authority) {
    authority.host = masks.apply_host(&authority.host).into_owned();
    authority.display_host = masks.apply_host(&authority.display_host).into_owned();
}

/// Ein unbekannter Flow als `NOT_FOUND`, mit seinem Befund in den Details.
///
/// Die Ausnahme selbst steht in [`crate::server_stub::get_flow_status`], damit
/// der Fake sie nicht ein zweites Mal beschreiben muss.
fn unknown_flow(id: FlowId) -> Status {
    crate::server_stub::get_flow_status(&not_held(&NotHeld::Unknown { id }))
}

/// Die Protokollzeile zu einem Abonnenten, den der Rundfunk überholt hat:
/// `LIMIT_006`, nur `info` (HUM-057).
///
/// Der Abonnent selbst bekommt `Lagged` und lädt mit `ListFlows` nach; der
/// Proxy hat auf ihn nicht gewartet. Die Zeile sagt nur, dass es geschah und
/// wie viele Ereignisse fehlten, damit eine dauerhaft zu langsame Oberfläche
/// im Protokoll auffällt und nicht nur als flackernde Liste.
fn log_lagged(dropped: u64) {
    tracing::info!(
        code = codes::LIMIT_006.as_str(),
        dropped,
        "a subscriber fell behind the event stream (limits.event_buffer) and reloads with \
         ListFlows; no request was delayed"
    );
}

/// Ein Befund für einen Lauf, zu dem keine Zusammenfassung vorliegt.
///
/// Der Grund nennt beide Möglichkeiten, weil dieser Dienst sie nicht
/// unterscheiden kann: Es gab den Lauf nie auf diesem Rechner, oder es gab ihn
/// und er hinterließ keine Zusammenfassung (Projekt nur lesbar eingehängt,
/// Daemon ohne Aufzeichnung, Lauf älter als HUM-043).
pub(crate) fn unknown_summary(sandbox: SandboxId) -> Diagnostic {
    Diagnostic::builder(codes::SANDBOX_027, Severity::Error)
        .why(format!(
            "no summary is recorded for sandbox {sandbox}; either this machine never ran it, or \
             the run left none — a project mounted read-only cannot change, and a daemon without \
             a recording keeps nothing"
        ))
        .build()
}

/// Ein Befund für einen Flow, der nicht mehr wartet (`FailedPrecondition`).
///
/// Eine Bestätigung, die zu diesem Flow nicht passt, ist dagegen ein Fehler der
/// Anfrage: `IPC_004` (`InvalidArgument`), und der Flow wartet weiter (HUM-160).
///
/// Ein `Allow` auf einen Flow unter der harten Sperre ist keins von beiden: Der
/// Flow wartet, und die Anfrage war richtig gestellt, nur darf er so nicht
/// hinaus. Die Antwort ist dann der Befund `HOLD_004` vom Flow selbst, mit Art
/// und Ort des Geheimnisses und dem Vorschlag, den Schalter umzulegen
/// (HUM-159).
fn not_held(error: &NotHeld) -> Diagnostic {
    if let NotHeld::SendRefused { refusal, .. } = error {
        return refusal.as_ref().clone();
    }
    let code = if error.is_bad_request() {
        codes::IPC_004
    } else {
        codes::IPC_003
    };
    Diagnostic::builder(code, Severity::Error)
        .why(error.to_string())
        .build()
}

/// Ein abgelehntes Einzelergebnis einer Entscheidung, samt seinem Befund.
fn refused(flow_id: &str, diagnostic: Diagnostic) -> (v1::DecideResult, Option<Diagnostic>) {
    let result = v1::DecideResult {
        flow_id: flow_id.to_owned(),
        applied: false,
        diagnostic: Some(convert::diagnostic_to_proto(&diagnostic)),
    };
    (result, Some(diagnostic))
}

/// Der Strom der Regeländerungen als Ereignis des Vertrags.
///
/// Eine Regeländerung ist kein Flow-Ereignis; sie hat weder Flow noch Zustand.
/// Sie reist trotzdem im selben Strom, damit ein Client genau ein Abonnement
/// braucht (`backlog/sprint-2.md`, HUM-027). Der Inhalt ist nur die Revision:
/// Wer sie sieht, lädt die Liste über `Rules{list}` nach.
///
/// Fällt ein Zuhörer zurück, bekommt er statt der verpassten Zwischenstände
/// den aktuellen Stand — mehr braucht er nicht, weil er ohnehin nachlädt.
fn rules_changed_stream(store: Arc<RulesStore>) -> BoxStream<Result<v1::FlowEvent, Status>> {
    let stream = BroadcastStream::new(store.subscribe()).map(move |item| {
        let revision = match item {
            Ok(revision) => revision,
            Err(BroadcastStreamRecvError::Lagged(_)) => store.revision(),
        };
        Ok(v1::FlowEvent {
            at: Some(convert::timestamp(std::time::SystemTime::now())),
            event: Some(v1::flow_event::Event::RulesChanged(
                v1::flow_event::RulesChanged { revision },
            )),
        })
    });
    Box::pin(stream)
}

/// Die Anfrage an die Aufzeichnung, aus der Anfrage des Vertrags.
///
/// Der Filter wird durchgereicht, wie er ist: Seine Grammatik ist dieselbe für
/// Oberfläche, `ListFlows` und Kommandozeile (`backlog/sprint-2.md` HUM-026),
/// und ein unbekannter Schlüssel ist dort ein Befund mit `RECORDER_002`, nicht
/// eine stillschweigend leere Liste. Ohne `include_passthrough` kommt
/// `passthrough:false` dazu — derselbe Filter, den auch der Ereignisstrom legt.
///
/// # Errors
///
/// `IPC_005`, wenn `order_by` oder `cursor` nicht lesbar sind.
fn flow_query(request: &v1::ListFlowsRequest) -> Result<FlowQuery, Diagnostic> {
    let mut filter = request.filter.trim().to_owned();
    if !request.include_passthrough {
        if !filter.is_empty() {
            filter.push(' ');
        }
        filter.push_str("passthrough:false");
    }
    let (sort, desc) = order_of(&request.order_by)?;
    let cursor = if request.cursor.is_empty() {
        None
    } else {
        Some(decode_cursor(&request.cursor)?)
    };
    Ok(FlowQuery {
        filter,
        sort,
        desc,
        limit: request.limit,
        cursor,
    })
}

/// Sortierschlüssel und Richtung aus `ListFlowsRequest.order_by`.
///
/// Leer heißt „nach Ankunft, neueste zuerst". Ein Schlüssel, den die
/// Aufzeichnung nicht sortieren kann, wird abgelehnt und nicht durch einen
/// anderen ersetzt: Eine Liste in einer Reihenfolge, die niemand verlangt hat,
/// sähe aus wie die verlangte.
///
/// # Errors
///
/// `IPC_005` mit der Liste der gültigen Schlüssel.
fn order_of(order_by: &str) -> Result<(SortKey, bool), Diagnostic> {
    let (key, descending) = validate::order_by(order_by)?;
    let sort = match key {
        "host" => SortKey::Host,
        "duration" => SortKey::Duration,
        "size" => SortKey::Size,
        // `received_at` und alles, was [`crate::validate::order_by`] darauf
        // abbildet. Ein anderer Name kommt von dort nicht zurück.
        _ => SortKey::Ts,
    };
    Ok((sort, descending))
}

/// Der Cursor der nächsten Seite als Text des Vertrags.
///
/// Er trägt genau die drei Felder, die die Aufzeichnung braucht
/// (`backlog/CONVENTIONS.md` 4.14), und ist für den Klienten undurchsichtig:
/// Er reicht ihn zurück, wie er ihn bekommen hat.
fn encode_cursor(cursor: &Cursor) -> String {
    let sort = match &cursor.sort {
        None => String::new(),
        Some(CursorKey::Int(value)) => format!("i{value}"),
        Some(CursorKey::Text(value)) => format!("t{value}"),
    };
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(format!("{}\u{1f}{}\u{1f}{sort}", cursor.ts, cursor.id))
}

/// Liest einen Cursor zurück.
///
/// # Errors
///
/// `IPC_005`, wenn der Text nicht von [`encode_cursor`] stammt. Geraten wird
/// nichts: Ein halb gelesener Cursor lieferte eine Seite, die weder lückenlos
/// noch doppelfrei wäre.
fn decode_cursor(text: &str) -> Result<Cursor, Diagnostic> {
    let refuse = || {
        Diagnostic::builder(codes::IPC_005, Severity::Error)
            .why(format!(
                "{text:?} is not a cursor from this daemon; ask again without a cursor to \
                 start at the first page"
            ))
            .build()
    };
    let raw = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(text)
        .map_err(|_error| refuse())?;
    let decoded = String::from_utf8(raw).map_err(|_error| refuse())?;
    let mut parts = decoded.split('\u{1f}');
    let ts: i64 = parts
        .next()
        .ok_or_else(refuse)?
        .parse()
        .map_err(|_error| refuse())?;
    let id = parts.next().ok_or_else(refuse)?.to_owned();
    let sort = match parts.next().unwrap_or_default() {
        "" => None,
        rest => match rest.split_at(1) {
            ("i", value) => Some(CursorKey::Int(value.parse().map_err(|_error| refuse())?)),
            ("t", value) => Some(CursorKey::Text(value.to_owned())),
            _ => return Err(refuse()),
        },
    };
    if parts.next().is_some() {
        return Err(refuse());
    }
    Ok(Cursor { ts, id, sort })
}

/// Die Prüfsumme aus einem [`v1::BodyRef`].
///
/// # Errors
///
/// `IPC_005`, wenn sie keine 32 Bytes hat. Ein gekürzter Hash zeigte auf einen
/// anderen Inhalt oder auf keinen.
fn body_hash(wire: &v1::BodyRef) -> Result<[u8; 32], Diagnostic> {
    validate::body_hash(wire)
}

/// Der Anfang des Anfrage-Bodys für die Anzeige.
///
/// Steht der Body in der Datenbank, kommt er von dort; sonst aus dem
/// Blob-Speicher. Scheitert das Lesen, bleibt die Vorschau leer und der Grund
/// steht im Protokoll: Der vollständige Inhalt kommt ohnehin über `GetBody`,
/// und eine erfundene Vorschau wäre schlimmer als keine.
async fn preview_of(recorder: &Recorder, detail: &RecordedDetail) -> String {
    let Some(message) = detail
        .messages
        .iter()
        .find(|message| message.dir == Dir::Request)
    else {
        return String::new();
    };
    if let Some(inline) = message.body.inline.as_ref() {
        return convert::body_preview(inline);
    }
    if message.body.size == 0 {
        return String::new();
    }
    match recorder.read_body(&message.body).await {
        Ok(bytes) => convert::body_preview(&bytes),
        Err(error) => {
            tracing::warn!(
                flow = %detail.summary.id,
                why = %error,
                "the request body could not be read for the preview"
            );
            String::new()
        }
    }
}

/// Zerlegt einen Body in die Stücke, die `GetBody` streamt.
///
/// Die Stückelung selbst steht in [`crate::body::chunks`], damit der Fake
/// dieselbe benutzt; hier wird sie nur in den Strom gelegt, den tonic will.
fn body_stream(bytes: &Bytes, encoding_left: &str) -> BoxStream<Result<v1::BodyChunk, Status>> {
    let chunks: Vec<Result<v1::BodyChunk, Status>> = crate::body::chunks(bytes, encoding_left)
        .into_iter()
        .map(Ok)
        .collect();
    Box::pin(tokio_stream::iter(chunks))
}

#[tonic::async_trait]
impl v1::humanitl_server::Humanitl for IpcServer {
    type SubscribeStream = BoxStream<Result<v1::FlowEvent, Status>>;
    type GetBodyStream = BoxStream<Result<v1::BodyChunk, Status>>;
    type SandboxStream = BoxStream<Result<v1::SandboxEvent, Status>>;
    type TerminalStream = BoxStream<Result<v1::TerminalOutput, Status>>;
    type DiscoverLlmStream = BoxStream<Result<v1::DiscoverResult, Status>>;

    async fn get_info(&self, _request: Request<()>) -> Result<Response<v1::Info>, Status> {
        Ok(Response::new(self.info.clone()))
    }

    async fn subscribe(
        &self,
        request: Request<v1::SubscribeRequest>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        Ok(Response::new(self.event_stream(&request.into_inner())))
    }

    /// Eine Seite der Flow-Historie.
    ///
    /// Mit Aufzeichnung kommt sie aus der Datenbank und umfasst damit auch
    /// frühere Sitzungen; ohne sie aus der Registry dieser Sitzung.
    async fn list_flows(
        &self,
        request: Request<v1::ListFlowsRequest>,
    ) -> Result<Response<v1::FlowPage>, Status> {
        let request = request.into_inner();
        // Der Sortierschlüssel wird geprüft, bevor feststeht, woher die Seite
        // kommt: Ohne Aufzeichnung antwortete der Dienst sonst auf einen
        // Schlüssel, den er nicht sortieren kann, mit der Vorgabereihenfolge —
        // also anders als mit Aufzeichnung und anders als der Fake.
        let (_, descending) = validate::order_by(&request.order_by)
            .map_err(|diagnostic| diagnostic_to_status(&diagnostic))?;
        let Some(recorder) = self.recorder.as_ref() else {
            return Ok(Response::new(self.page(&request, descending)));
        };
        self.recorded_page(recorder, &request)
            .await
            .map(Response::new)
            .map_err(|diagnostic| diagnostic_to_status(&diagnostic))
    }

    /// Entscheidet einen oder mehrere wartende Flows.
    ///
    /// Ein Stapel entscheidet, soweit er kann: jedes Ergebnis trägt seinen
    /// eigenen Befund. Hat die Anfrage aber gar nichts bewirkt — der häufigste
    /// Fall ist die einzelne Id eines Flows, der nicht mehr wartet —, dann ist
    /// das ein Fehler der Anfrage und kein Erfolg mit leerem Inhalt: der
    /// Aufruf endet mit dem Befund des ersten Flows, bei `IPC_003` also mit
    /// `FailedPrecondition`.
    async fn decide(
        &self,
        request: Request<v1::DecideRequest>,
    ) -> Result<Response<v1::DecideResponse>, Status> {
        let request = request.into_inner();
        // Erst lesen, dann wirken: die Prüfungen und ihre Reihenfolge stehen in
        // [`crate::validate`] und gelten für den Fake genauso.
        let decision = match validate::decide_plan(&request, self.body_cap_bytes)
            .map_err(|diagnostic| diagnostic_to_status(&diagnostic))?
        {
            validate::DecidePlan::Decide(decision) => decision,
            validate::DecidePlan::RefuseEach(diagnostic) => {
                let results = request
                    .flow_ids
                    .iter()
                    .map(|text| refused(text, diagnostic.clone()).0)
                    .collect();
                return Ok(Response::new(v1::DecideResponse {
                    results,
                    created_rule_id: String::new(),
                    created_rule: None,
                }));
            }
        };

        // Erst die Regel, dann die Entscheidung: scheitert das Anlegen, wird
        // nichts entschieden (`backlog/sprint-2.md`, HUM-027). Der umgekehrte
        // Weg hinterließe einen freigegebenen Flow und einen Menschen, der
        // glaubt, die Freigabe gelte ab jetzt für alle.
        let created = match request.remember.as_ref() {
            None => None,
            Some(rule) => {
                let service = self
                    .rules_or_refuse()
                    .map_err(|diagnostic| diagnostic_to_status(&diagnostic))?;
                Some(
                    service
                        .remember(rule)
                        .map_err(|diagnostic| diagnostic_to_status(&diagnostic))?,
                )
            }
        };

        let (results, refusals): (Vec<v1::DecideResult>, Vec<Option<Diagnostic>>) = request
            .flow_ids
            .iter()
            .map(|text| self.decide_one(text, &decision, &request.acknowledged_findings))
            .unzip();

        if results.iter().all(|result| !result.applied) {
            // Nichts entschieden heißt: die Anfrage hat nichts bewirkt. Die
            // Regel, die nur zu dieser Entscheidung gehörte, wird deshalb
            // wieder zurückgenommen.
            if let (Some(rule), Some(service)) = (created.as_ref(), self.rules.as_ref())
                && let Ok(id) = humanitl_core::RuleId::parse(&rule.rule_id)
            {
                service.forget(id);
            }
            let first = refusals
                .into_iter()
                .flatten()
                .next()
                .unwrap_or_else(|| not_held(&NotHeld::Unknown { id: FlowId::nil() }));
            return Err(diagnostic_to_status(&first));
        }

        Ok(Response::new(v1::DecideResponse {
            results,
            created_rule_id: created
                .as_ref()
                .map(|rule| rule.rule_id.clone())
                .unwrap_or_default(),
            created_rule: created,
        }))
    }

    /// Alles, was zu einem Flow aufgezeichnet wurde.
    ///
    /// Ohne Aufzeichnung bleibt der Datensatz der laufenden Sitzung; er trägt
    /// die Anfrage, aber weder Antwort-Kopfzeilen noch Funde. Kennt weder die
    /// Aufzeichnung noch die Registry den Flow, ist das `NOT_FOUND` und kein
    /// leerer Erfolg: Ein leeres Detail sähe aus wie ein Flow ohne Inhalt.
    async fn get_flow(
        &self,
        request: Request<v1::FlowRef>,
    ) -> Result<Response<v1::FlowDetail>, Status> {
        let request = request.into_inner();
        let id = validate::flow_id(&request.flow_id)
            .map_err(|diagnostic| diagnostic_to_status(&diagnostic))?;
        if let Some(recorder) = self.recorder.as_ref()
            && let Some(detail) = self
                .recorded_detail(recorder, id)
                .await
                .map_err(|diagnostic| diagnostic_to_status(&diagnostic))?
        {
            let detail = self.with_live_request(recorder, id, detail);
            return Ok(Response::new(
                self.with_live_diagnostics(recorder, id, detail),
            ));
        }
        self.registry
            .get(id)
            .map(|record| {
                let detail = convert::record_to_detail(&record, self.domains.as_deref());
                Response::new(self.registry_detail(detail, &record.request, &record.diagnostics))
            })
            .ok_or_else(|| unknown_flow(id))
    }

    /// Der Inhalt eines Bodys, in Stücken.
    ///
    /// Ohne Aufzeichnung gibt es keinen Ort, an dem ein Body läge: Der Proxy
    /// hält ihn nur, solange die Anfrage läuft. Das ist dann kein
    /// `UNIMPLEMENTED` — die RPC gibt es —, sondern der Befund, dass dieser
    /// Daemon ohne Aufzeichnung läuft.
    ///
    /// Mit `BodyRef.decoded` nimmt der Daemon die Kodierung aus
    /// `BodyRef.content_encoding` ab, soweit er sie kann, und sagt in jedem
    /// Stück, was noch darauf liegt ([`crate::body::deliver`], HUM-119).
    /// Gespeichert bleibt, was auf der Leitung war; entpackt wird erst hier,
    /// beim Ausliefern.
    async fn get_body(
        &self,
        request: Request<v1::BodyRef>,
    ) -> Result<Response<Self::GetBodyStream>, Status> {
        let wire = request.into_inner();
        // Erst die Anfrage lesen, dann nachsehen, ob dieser Daemon sie
        // beantworten kann: Ein `sha256`, das keines ist, bleibt auch mit
        // Aufzeichnung unlesbar, und der Fake — der keine hat — antwortet
        // sonst auf dieselbe Anfrage etwas anderes.
        body_hash(&wire).map_err(|diagnostic| diagnostic_to_status(&diagnostic))?;
        let recorder = self.recorder.as_ref().ok_or_else(|| {
            diagnostic_to_status(
                &Diagnostic::builder(codes::RECORDER_001, Severity::Error)
                    .why(
                        "this daemon runs without a recording; bodies are only kept while the \
                         request is in flight"
                            .to_owned(),
                    )
                    .build(),
            )
        })?;
        let bytes = self
            .read_body(recorder, &wire)
            .await
            .map_err(|diagnostic| diagnostic_to_status(&diagnostic))?;
        let (bytes, encoding_left) = crate::body::deliver(
            &wire,
            bytes,
            self.decode_cap_bytes,
            self.max_decompress_ratio,
        );
        Ok(Response::new(body_stream(&bytes, &encoding_left)))
    }

    /// Liest oder ändert den Regelsatz.
    ///
    /// Jede Antwort trägt den vollständigen Regelsatz danach. Was die Anfrage
    /// nicht ausführen konnte, ist ein Fehler des Aufrufs, kein Erfolg mit
    /// leerem Inhalt; nur `reload` legt seine Befunde in die Antwort, weil
    /// dabei die alten Regeln in Kraft bleiben und der Client sie sehen soll.
    async fn rules(
        &self,
        request: Request<v1::RulesRequest>,
    ) -> Result<Response<v1::RulesResponse>, Status> {
        // Die Prüfung „ohne Operation ist keine Anfrage" steht genau einmal, in
        // [`crate::validate::rules_op`], und wird genau einmal gerufen, aus
        // [`RulesService::apply`]. Sie hier zusätzlich zu wiederholen, machte
        // ihre Entfernung dort unbemerkbar.
        let service = self
            .rules_or_refuse()
            .map_err(|diagnostic| diagnostic_to_status(&diagnostic))?;
        service
            .apply(request.into_inner())
            .await
            .map(Response::new)
            .map_err(|diagnostic| diagnostic_to_status(&diagnostic))
    }

    /// Was der Agent bekommt, und Start und Stopp (HUM-040).
    ///
    /// Der Strom endet, wenn die Operation beantwortet ist; er bleibt nicht
    /// offen. Ein Client, der zusehen will, fragt nach jeder Änderung erneut —
    /// die Ereignisse einer laufenden Sitzung (Terminal, Isolationsprüfungen)
    /// kommen mit HUM-041 und HUM-042 dazu.
    async fn sandbox(
        &self,
        request: Request<v1::SandboxRequest>,
    ) -> Result<Response<Self::SandboxStream>, Status> {
        let sandbox = self.sandbox.as_ref().ok_or_else(|| {
            diagnostic_to_status(
                &Diagnostic::builder(codes::IPC_006, Severity::Error)
                    .why(
                        "this daemon runs without a sandbox, so it cannot say what an agent \
                         would get or start one"
                            .to_owned(),
                    )
                    .build(),
            )
        })?;
        Ok(Response::new(Box::pin(
            sandbox.stream(request.into_inner()).map(Ok),
        )))
    }

    /// Das Terminal der laufenden Sitzung (HUM-042).
    ///
    /// Ein Schreiber, beliebig viele Leser. Der Strom endet mit `close`, mit
    /// dem Agenten oder mit einem Befund; er schließt nie das Pseudoterminal.
    /// Ohne Sandbox antwortet er wie die `Sandbox`-RPC mit `IPC_006` —
    /// hier allerdings als `Diagnostic` im Strom und nicht als Status, denn
    /// der Vertrag hat für diesen Fall ein Feld
    /// (`TerminalOutput.diagnostic`), und ein Client, der schon liest, soll
    /// den Grund an derselben Stelle finden wie jeden anderen.
    async fn terminal(
        &self,
        request: Request<tonic::Streaming<v1::TerminalInput>>,
    ) -> Result<Response<Self::TerminalStream>, Status> {
        let sandbox = self.sandbox.clone();
        let input = crate::server_stub::plain_stream(request.into_inner());
        let open = move |wanted: &v1::terminal_input::Open| match sandbox.as_ref() {
            Some(sandbox) => sandbox.terminal_for(wanted),
            None => Err(Diagnostic::builder(codes::IPC_006, Severity::Error)
                .why(
                    "this daemon runs without a sandbox, so there is no terminal to attach to"
                        .to_owned(),
                )
                .build()),
        };
        Ok(Response::new(Box::pin(
            crate::terminal::serve(open, input).map(Ok),
        )))
    }

    /// Prüfung, Ende, Seiten und Export der Audit-Kette (HUM-156,
    /// [`crate::audit`]).
    ///
    /// Eine gebrochene Kette ist eine Antwort (`ok: false` samt `AUDIT_001`),
    /// kein Fehler. Ein Fehler ist, was nicht geht: eine Anfrage, die so nicht
    /// gilt, ein Log, das sich nicht lesen lässt, ein Export, der sich nicht
    /// schreiben lässt.
    async fn audit(
        &self,
        request: Request<v1::AuditRequest>,
    ) -> Result<Response<v1::AuditResponse>, Status> {
        let Some(service) = self.audit_log.as_ref() else {
            return Err(diagnostic_to_status(
                &Diagnostic::builder(codes::IPC_006, Severity::Error)
                    .why(
                        "this daemon runs without an audit log, so there is no chain to check, \
                         list or write"
                            .to_owned(),
                    )
                    .build(),
            ));
        };
        service
            .answer(&request.into_inner())
            .await
            .map(Response::new)
            .map_err(|diagnostic| diagnostic_to_status(&diagnostic))
    }

    /// Die Konfiguration, wie der nächste Start sie lädt, frisch aus den
    /// Dateien (HUM-069, [`config_rpc::snapshot`]).
    ///
    /// Frisch und nicht aus der Auflösung des Daemon-Starts: Das Formular
    /// zeigt, was in `config.toml` steht, und ein `SetConfig` von eben gehört
    /// dazu, auch wenn der laufende Daemon den Wert erst nach seinem Neustart
    /// liest.
    async fn get_config(
        &self,
        request: Request<v1::GetConfigRequest>,
    ) -> Result<Response<v1::ConfigSnapshot>, Status> {
        let include_schema = request.into_inner().include_schema;
        let paths = self.doctor.paths.clone();
        let mut snapshot = tokio::task::spawn_blocking(move || config_rpc::snapshot(&paths))
            .await
            .map_err(|error| diagnostic_to_status(&unfinished("GetConfig", &error)))?;
        if include_schema {
            snapshot.json_schema = humanitl_config::json_schema().to_string();
        }
        Ok(Response::new(snapshot))
    }

    /// Setzt eine Einstellung des Formulars oder eine CA-Variable der Sandbox
    /// in `config.toml`, und nur diese (HUM-151, HUM-069, [`config_rpc`]).
    ///
    /// Die Datei wird in [`tokio::task::spawn_blocking`] gelesen und
    /// geschrieben, wie die Prüfungen des Doctors.
    async fn set_config(
        &self,
        request: Request<v1::SetConfigRequest>,
    ) -> Result<Response<v1::ConfigSnapshot>, Status> {
        let v1::SetConfigRequest {
            key,
            value,
            expected_server,
        } = request.into_inner();
        let paths = self.doctor.paths.clone();
        let snapshot = tokio::task::spawn_blocking(move || {
            config_rpc::set_expecting(&paths, &key, &value, &expected_server)
        })
        .await
        .map_err(|error| diagnostic_to_status(&unfinished("SetConfig", &error)))?
        .map_err(|diagnostic| diagnostic_to_status(&diagnostic))?;
        Ok(Response::new(snapshot))
    }

    /// Prüft die Maschine, auf der dieser Daemon läuft (HUM-075).
    ///
    /// Derselbe Bericht, den `humanitl doctor` ausgibt: Beide rufen
    /// [`humanitl_sandbox::doctor::run`] über denselben [`DoctorProbe`], und
    /// keiner von beiden hat eine eigene Prüfung (ADR-018).
    ///
    /// Zwei Dinge sind hier anders als in der Kommandozeile, und beide sind
    /// Tatsachen und keine Wahl:
    ///
    /// - Die Zeile `daemon` ist nicht gemessen. Wer diese Antwort liest, hat
    ///   den Daemon erreicht; ein Daemon kann aber nicht sagen, ob ein anderer
    ///   Client ihn erreicht. Der Beleg sagt genau das.
    /// - Die Zeile `llm` ist nicht gemessen. `Doctor` nimmt keinen Auftrag
    ///   entgegen (`rpc Doctor(Empty)`), also kann niemand darin um eine
    ///   Verbindung bitten — und eine Verbindung, um die niemand gebeten hat,
    ///   baut dieser Daemon nicht auf. Gemessen wird sie über `ProbeLlm`
    ///   (HUM-039).
    ///
    /// Die Prüfungen lesen Dateien und starten kurze Programme; das läuft in
    /// [`tokio::task::spawn_blocking`], damit eine hängende Probe die Laufzeit
    /// nicht anhält.
    async fn doctor(&self, _request: Request<()>) -> Result<Response<v1::DoctorReport>, Status> {
        let setup = self.doctor.clone();
        let socket = setup.paths.daemon_socket();
        let daemon = humanitl_sandbox::doctor::DaemonFacts::NotTried {
            socket,
            why: "this report comes from the daemon itself, which cannot tell whether a client \
                  reaches it"
                .to_owned(),
        };
        let report = tokio::task::spawn_blocking(move || {
            let setup = setup.refreshed();
            let llm = setup.llm_facts();
            let probe = DoctorProbe::new(setup.paths.env())
                .with_agent(setup.adapter.clone(), setup.agent_command.clone());
            humanitl_sandbox::doctor::run(&probe.collect(daemon, llm))
        })
        .await
        .map_err(|error| Status::internal(format!("the doctor did not finish: {error}")))?;
        Ok(Response::new(convert::doctor_report_to_proto(&report)))
    }

    /// Sucht LLM-Server im eigenen Netz (HUM-076).
    ///
    /// Nur auf ausdrücklichen Wunsch: Diese RPC ist der Klick, und ohne sie
    /// entsteht keine einzige Verbindung. Der Scan läuft im Daemon, im
    /// Host-Netz, nie in der Sandbox; er ist kein Fluss und erscheint in keiner
    /// Warteschlange.
    ///
    /// Das Netz kommt aus der Anfrage oder, wenn sie keines nennt, aus der
    /// Vorgaberoute. Weiter als ein `/24` geht die Suche nicht — die Weigerung
    /// steht als `LLM_008` in der Antwort, statt still ein größeres Netz
    /// abzuklopfen.
    async fn discover_llm(
        &self,
        request: Request<v1::DiscoverRequest>,
    ) -> Result<Response<Self::DiscoverLlmStream>, Status> {
        let request = request.into_inner();
        let probe = self.llm_probe.as_ref().ok_or_else(|| {
            let diagnostic = self.llm_probe_error.clone().unwrap_or_else(no_probe);
            diagnostic_to_status(&diagnostic)
        })?;
        // Ein Port jenseits von `u16` ist keine Portnummer. Er wird nicht
        // still übergangen, sonst fiele eine Liste aus lauter Unsinn auf die
        // Vorgabe zurück und der Aufrufer hielte sie für befolgt.
        let ports: Vec<u16> = request
            .ports
            .iter()
            .map(|port| {
                u16::try_from(*port).map_err(|_error| {
                    diagnostic_to_status(
                        &Diagnostic::builder(codes::LLM_008, Severity::Error)
                            .why(format!("{port} is not a port number"))
                            .build(),
                    )
                })
            })
            .collect::<Result<Vec<u16>, Status>>()?;

        // Das eigene Netz steht immer fest, auch wenn der Aufrufer eines
        // nennt: Ein fremdes `/24` ist genauso breit wie das eigene und
        // trotzdem das Netz von jemand anderem. Die Suche bleibt darin, und
        // die Schleife gehört dazu — dort läuft Ollama, wenn es auf demselben
        // Rechner läuft.
        let local = humanitl_proxy::local_net()
            .await
            .map_err(|diagnostic| diagnostic_to_status(&diagnostic))?;
        let loopback = Subnet::local_24(Ipv4Addr::LOCALHOST);
        let subnet = if request.subnet.is_empty() {
            local.subnet
        } else {
            let asked = Subnet::parse(&request.subnet)
                .map_err(|diagnostic| diagnostic_to_status(&diagnostic))?;
            if !local.subnet.contains(asked) && !loopback.contains(asked) {
                return Err(diagnostic_to_status(
                    &Diagnostic::builder(codes::LLM_008, Severity::Error)
                        .why(format!(
                            "{asked} is not inside the network of this machine ({}): the search \
                             stays in the network it belongs to, and scanning someone else's is \
                             not what this button promises",
                            local.subnet
                        ))
                        .build(),
                ));
            }
            asked
        };
        tracing::info!(
            %subnet,
            interface = %local.interface,
            "searching for LLM servers"
        );
        let scan = Scan::new(subnet)
            .with_ports(ports)
            .map_err(|diagnostic| diagnostic_to_status(&diagnostic))?
            .first(vec![Ipv4Addr::LOCALHOST, local.own]);

        // Ein eigener Egress für den Scan: Er trägt die kurze Frist der Suche
        // (200 ms), während die Probe die längere der Verbindung behält.
        let sweep: Arc<dyn Egress> = Arc::new(Direct::new(CONNECT_TIMEOUT));
        let probe = Arc::clone(probe);
        // Zwei Kanäle statt einem: Dazwischen zählt `relay_scan` die Treffer
        // und vermerkt die Suche im Audit-Log, sobald sie endet (HUM-076,
        // HUM-050). Geht der Aufrufer, lässt das Relais den inneren Empfänger
        // fallen, und der Scan bricht ab wie vorher.
        let (found_tx, found_rx) = tokio::sync::mpsc::channel(16);
        let (tx, rx) = tokio::sync::mpsc::channel(16);
        let searched = LlmDiscover {
            subnet: subnet.to_string(),
            ports: scan.ports.clone(),
            found: 0,
        };
        tokio::spawn(relay_scan(found_rx, tx, self.audit.clone(), searched));
        tokio::spawn(async move { humanitl_proxy::discover(probe, sweep, &scan, found_tx).await });
        Ok(Response::new(Box::pin(
            tokio_stream::wrappers::ReceiverStream::new(rx)
                .map(|found| Ok(convert::found_to_proto(&found))),
        )))
    }

    /// Prüft genau den Endpunkt, den der Aufrufer nennt (HUM-039).
    ///
    /// Nichts davon läuft in der Sandbox, nichts davon ändert etwas: zwei
    /// `GET` auf zwei feste Pfade, keine Weiterleitung, und ein
    /// Zugangsschlüssel nur, wenn der Auftrag ihn verlangt
    /// (`IpcServer::probe_key`, HUM-140). Was der Endpunkt nicht beantwortet
    /// hat, steht als Befund in der Antwort und nie als erfundene Modellliste.
    async fn probe_llm(
        &self,
        request: Request<v1::ProbeLlmRequest>,
    ) -> Result<Response<v1::ProbeLlmResponse>, Status> {
        let mut request = request.into_inner();
        // Der eingetippte Schlüssel verlässt die Nachricht sofort in eine
        // Hülle, die beim Fallenlassen überschrieben wird, auch auf jedem
        // frühen Rückweg unten (HUM-140, Review).
        let typed = Zeroizing::new(std::mem::take(&mut request.api_key));
        // Dieselbe Reihenfolge wie beim Fake und wie bei `GetBody` und `Rules`:
        // erst die Anfrage lesen, dann fragen, ob dieser Daemon sie beantworten
        // kann. Ein Endpunkt, der keine URL ist, bleibt auch mit Probe keine;
        // andersherum antwortete ein Daemon ohne Probe auf denselben Unsinn
        // `IPC_006`, der Fake dagegen `LLM_007`.
        let endpoint = validate::llm_endpoint(&request.endpoint)
            .map_err(|diagnostic| diagnostic_to_status(&diagnostic))?;
        let probe = self.llm_probe.as_ref().ok_or_else(|| {
            let diagnostic = self.llm_probe_error.clone().unwrap_or_else(no_probe);
            diagnostic_to_status(&diagnostic)
        })?;
        // Die Probe klemmt die Frist selbst auf `MAX_TIMEOUT_MS`; hier steht
        // nur die Umrechnung, und `0` heißt die Vorgabe.
        let timeout = match request.timeout_ms {
            0 => None,
            ms => Some(Duration::from_millis(u64::from(ms))),
        };
        let key = self
            .probe_key(&endpoint, &typed, request.use_stored_key, &request.server)
            .await
            .map_err(|diagnostic| diagnostic_to_status(&diagnostic))?;
        let result = probe
            .probe_with_key(&endpoint, timeout, key.as_ref())
            .await
            .map_err(|diagnostic| diagnostic_to_status(&diagnostic))?;
        Ok(Response::new(convert::probe_result_to_proto(&result)))
    }

    /// Der Zugangsschlüssel des Modells: Stand, setzen, entfernen (HUM-140,
    /// [`llm_key`]). Keine Antwort trägt den Schlüssel.
    async fn llm_key(
        &self,
        request: Request<v1::LlmKeyRequest>,
    ) -> Result<Response<v1::LlmKeyStatus>, Status> {
        let request = request.into_inner();
        let paths = self.doctor.paths.clone();
        let keys = self.key_store();
        let status = tokio::task::spawn_blocking(move || llm_key::answer(&paths, &keys, request))
            .await
            .map_err(|error| diagnostic_to_status(&unfinished("LlmKey", &error)))?
            .map_err(|diagnostic| diagnostic_to_status(&diagnostic))?;
        Ok(Response::new(status))
    }

    /// Die gespeicherten Modellserver (HUM-289, [`llm_servers`]). Ein
    /// Wechsel gilt ab der nächsten Sitzung; keine Antwort trägt einen
    /// Schlüssel.
    async fn llm_servers(
        &self,
        request: Request<v1::LlmServersRequest>,
    ) -> Result<Response<v1::LlmServerList>, Status> {
        let request = request.into_inner();
        let paths = self.doctor.paths.clone();
        let keys = self.key_store();
        let sandbox = self.sandbox.clone();
        let list = tokio::task::spawn_blocking(move || {
            // Den aktiven Server entfernt der Dienst nur, solange keine
            // Sitzung läuft oder startet (HUM-289, Review E).
            let idle = |work: &mut dyn FnMut()| match &sandbox {
                Some(sandbox) => sandbox.while_idle(work),
                None => llm_servers::always_idle(work),
            };
            let no_start = |work: &mut dyn FnMut()| match &sandbox {
                Some(sandbox) => sandbox.while_no_start(work),
                None => llm_servers::always_idle(work),
            };
            let gates = llm_servers::Gates {
                idle: &idle,
                no_start: &no_start,
            };
            llm_servers::answer(&paths, &keys, &gates, &request)
        })
        .await
        .map_err(|error| diagnostic_to_status(&unfinished("LlmServers", &error)))?
        .map_err(|diagnostic| diagnostic_to_status(&diagnostic))?;
        Ok(Response::new(list))
    }

    /// Was ein Sandbox-Lauf im Projektverzeichnis hinterlassen hat.
    ///
    /// Dieselbe Reihenfolge wie bei `GetBody` und `ProbeLlm`: erst die Anfrage
    /// lesen, dann fragen, ob dieser Daemon sie beantworten kann. Eine Kennung,
    /// die keine ist, bleibt auch mit Aufzeichnung keine; andersherum
    /// antwortete ein Daemon ohne Aufzeichnung auf denselben Unsinn
    /// `RECORDER_001` und der Fake `IPC_005`.
    /// Eine weitere Shell in der laufenden Sandbox (HUM-299).
    ///
    /// Ohne Sandbox wie `Terminal`: `IPC_006`. Jeder Befund geht als Status
    /// hinaus; die Antwort trägt die Kennung, unter der `Terminal` das
    /// Terminal der Shell öffnet.
    async fn exec(
        &self,
        request: Request<v1::ExecRequest>,
    ) -> Result<Response<v1::ExecResponse>, Status> {
        let Some(sandbox) = self.sandbox.as_ref() else {
            return Err(diagnostic_to_status(
                &Diagnostic::builder(codes::IPC_006, Severity::Error)
                    .why(
                        "this daemon runs without a sandbox, so there is nothing to open a shell \
                         in"
                        .to_owned(),
                    )
                    .build(),
            ));
        };
        sandbox
            .exec(request.into_inner())
            .await
            .map(Response::new)
            .map_err(|diagnostic| diagnostic_to_status(&diagnostic))
    }

    async fn get_session_summary(
        &self,
        request: Request<v1::SessionSummaryRef>,
    ) -> Result<Response<v1::SessionSummary>, Status> {
        let sandbox = validate::sandbox_id(&request.into_inner().sandbox_id)
            .map_err(|diagnostic| diagnostic_to_status(&diagnostic))?;
        let recorder = self.recorder.as_ref().ok_or_else(|| {
            diagnostic_to_status(
                &Diagnostic::builder(codes::RECORDER_001, Severity::Error)
                    .why(
                        "this daemon runs without a recording, so no summary of an earlier \
                         sandbox run was kept"
                            .to_owned(),
                    )
                    .build(),
            )
        })?;
        Self::stored_summary(recorder, sandbox)
            .await
            .map(Response::new)
            .map_err(|diagnostic| crate::server_stub::missing_status(&diagnostic))
    }
}

/// Der Befund, wenn der Arbeitsfaden einer RPC nicht zu Ende kam (Panik oder
/// Abbruch beim Beenden des Daemons). Geschrieben ist dann nichts, oder es ist
/// nicht bekannt; der Befund sagt das, statt einen nackten `INTERNAL` zu
/// schicken (HUM-069, Review).
fn unfinished(rpc: &str, error: &tokio::task::JoinError) -> Diagnostic {
    Diagnostic::builder(codes::IPC_006, Severity::Error)
        .why(format!(
            "{rpc} did not finish: its worker thread ended early ({error}); nothing it did can \
             be relied on, so read the state again or repeat the call"
        ))
        .fix(FixAction::CopyCommand("humanitl daemon status".to_owned()))
        .build()
}

/// Baut den Verbindungsstapel der Endpunkt-Probe aus der Konfiguration.
///
/// Ein **eigener** Stapel, nicht der des Proxys, und zwar mit Absicht: Der
/// Zähler des Proxy-Resolvers belegt, dass vor einer Freigabe kein Name
/// aufgelöst wird (ADR-006, Escape-Test 3). Eine Auflösung, die ein Mensch im
/// Setup selbst angestoßen hat, gehört nicht in diesen Beweis. Die
/// Einstellungen sind dieselben (`resolver.*`, `limits.*`), also verhält sich
/// die Probe gegenüber `ollama.lan` genauso wie später der Proxy.
///
/// Die Vertrauensanker sind die des Upstreams, nicht die Humanitl-CA: Die
/// Probe redet mit dem Endpunkt, nicht mit dem Agenten. Welche das sind,
/// entscheidet `tls.trust_system_store` wie im Proxy (HUM-257).
fn build_llm_probe(config: &Config) -> Result<LlmProbe, Diagnostic> {
    let resolver = Arc::new(ResolverPort::from_config(&config.resolver)?);
    let client_tls = ClientTls::configured(config.tls.trust_system_store, &[], false)?;
    let upstream = Upstream::new(
        Arc::new(Direct::new(Duration::from_secs(
            config.limits.connect_timeout_secs,
        ))),
        resolver as Arc<dyn Resolver>,
        client_tls,
        config.resolver.prefer,
        Duration::from_secs(config.limits.header_timeout_secs),
    );
    Ok(LlmProbe::new(upstream))
}

/// Der Befund für einen Daemon ohne Endpunkt-Probe.
///
/// `IPC_006` und nicht `IPC_005`: Der Regel-RPC hat damit nichts zu tun, und
/// ein Code, der „Rules-Anfrage ungültig" heißt, schickte den Leser an die
/// falsche Stelle.
fn no_probe() -> Diagnostic {
    Diagnostic::builder(codes::IPC_006, Severity::Error)
        .why(
            "this daemon runs without an endpoint probe; llm.endpoint cannot be tested here"
                .to_owned(),
        )
        .build()
}

/// Reicht die Treffer einer Netzsuche an den Aufrufer weiter und vermerkt die
/// Suche im Audit-Log, sobald sie endet (HUM-076, HUM-050).
///
/// Genau ein `llm.discover` je Suche, auch wenn der Aufrufer vorher geht: Die
/// Verbindungsversuche ins Netz haben dann trotzdem stattgefunden. `found`
/// zählt jeden Server, der geantwortet hat, bevor die Suche endete. Der Record
/// ist abgeschickt, bevor der Strom des Aufrufers endet; wer das Ende sieht,
/// findet ihn im Log.
async fn relay_scan<T: Send + 'static>(
    mut found: tokio::sync::mpsc::Receiver<T>,
    out: tokio::sync::mpsc::Sender<T>,
    audit: Option<AuditHandle>,
    mut searched: LlmDiscover,
) {
    while let Some(server) = found.recv().await {
        searched.found += 1;
        if out.send(server).await.is_err() {
            break;
        }
    }
    // Geht der Aufrufer, endet mit diesem Empfänger auch der Scan: Er fängt
    // nichts mehr an und bricht ab, was noch läuft.
    drop(found);
    if let Some(audit) = audit {
        audit.record(None, RecordKind::LlmDiscover(searched));
    }
    drop(out);
}

/// Bindet einen Unix-Socket mit `0600`.
///
/// Scheitert das Setzen der Rechte, bleibt kein offener Socket zurück: ein
/// Socket, den die halbe Maschine öffnen darf, wäre der bequemste Weg an jeder
/// Entscheidung vorbei.
///
/// # Errors
///
/// [`Diagnostic`] mit `DAEMON_004`, wenn Binden oder `chmod` scheitert.
pub fn bind_socket(path: &Path) -> Result<UnixListener, Diagnostic> {
    let listener = UnixListener::bind(path).map_err(|error| {
        Diagnostic::builder(codes::DAEMON_004, Severity::Blocking)
            .why(format!(
                "cannot bind the socket {}: {error}",
                path.display()
            ))
            .build()
    })?;
    if let Err(error) = std::fs::set_permissions(path, Permissions::from_mode(auth::TOKEN_MODE)) {
        drop(listener);
        let _ = std::fs::remove_file(path);
        return Err(Diagnostic::builder(codes::DAEMON_004, Severity::Blocking)
            .why(format!(
                "cannot set 0600 on the socket {}: {error}",
                path.display()
            ))
            .build());
    }
    Ok(listener)
}

/// Bedient den Vertrag auf `socket`, bis `shutdown` fertig ist.
///
/// Reihenfolge, und sie ist wichtig: erst das Token schreiben, dann den Socket
/// binden. Ein Client, der auf den Socket wartet, findet die Datei sonst noch
/// nicht, wenn der Dienst schon antwortet.
///
/// Aufgeräumt wird, was dieser Aufruf angelegt hat: Socket und Token
/// verschwinden, wenn der Dienst endet, auch wenn er mit einem Fehler endet.
/// Eine liegen gebliebene Token-Datei wäre ein Schlüssel zu einem Dienst, den
/// es nicht mehr gibt.
///
/// # Errors
///
/// [`Diagnostic`] mit `DAEMON_004`, wenn Token oder Socket nicht angelegt
/// werden können, und mit `DAEMON_001`, wenn tonic den Dienst abbricht.
pub async fn serve(
    socket: &Path,
    token_path: &Path,
    server: IpcServer,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<(), Diagnostic> {
    let token = auth::new_token()?;
    auth::write_token(token_path, &token)?;
    let listener = match bind_socket(socket) {
        Ok(listener) => listener,
        Err(diagnostic) => {
            let _ = std::fs::remove_file(token_path);
            return Err(diagnostic);
        }
    };
    tracing::info!(
        socket = %socket.display(),
        token = %token_path.display(),
        "listening"
    );

    let service =
        v1::humanitl_server::HumanitlServer::with_interceptor(server, auth::TokenAuth::new(token));
    // Das Signal wird abgezweigt: tonic hört auf, Verbindungen anzunehmen, und
    // hier beginnt zugleich die Frist aus `SHUTDOWN_GRACE`.
    let (fired, started) = oneshot::channel();
    let signal = async move {
        shutdown.await;
        let _ = fired.send(());
    };
    let serving = Server::builder()
        .add_service(service)
        .serve_with_incoming_shutdown(UnixListenerStream::new(listener), signal);
    tokio::pin!(serving);

    let outcome = tokio::select! {
        result = &mut serving => result,
        _ = started => drain(&mut serving).await,
    };

    let result = outcome.map_err(|error| {
        Diagnostic::builder(codes::DAEMON_001, Severity::Blocking)
            .title("gRPC-Server abgebrochen")
            .why(format!("serving {} failed: {error}", socket.display()))
            .build()
    });
    let _ = std::fs::remove_file(socket);
    let _ = std::fs::remove_file(token_path);
    tracing::info!("stopped, socket and token removed");
    result
}

/// Lässt laufenden Aufrufen die Frist aus [`SHUTDOWN_GRACE`] und endet dann.
async fn drain<F>(serving: &mut F) -> Result<(), tonic::transport::Error>
where
    F: Future<Output = Result<(), tonic::transport::Error>> + Unpin,
{
    if let Ok(result) = tokio::time::timeout(SHUTDOWN_GRACE, serving).await {
        return result;
    }
    tracing::warn!(
        grace_secs = SHUTDOWN_GRACE.as_secs(),
        "a client kept its connection open past the grace period; closing anyway"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::sync::Arc;
    use std::time::SystemTime;

    use humanitl_config::{Config, Limits};
    use humanitl_core::diagnostics::codes;
    use humanitl_core::{
        Authority, BodyRef, Diagnostic, Flow, FlowEvent, FlowId, HostName, HttpRequest, Method,
        Scheme, SessionId, Severity, TransitionInput,
    };
    use humanitl_proxy::registry::FlowRecord;
    use humanitl_proxy::{ConnMeta, FlowRegistry, HoldQueue};
    use tonic::Request;

    use super::{CAPABILITIES, IpcServer, unfinished};
    use crate::v1;
    use crate::v1::humanitl_server::Humanitl as _;

    /// Ein Audit-Log in einem Wegwerf-Verzeichnis und seine Zeilen der Art
    /// `llm.discover`.
    fn discover_log() -> (tempfile::TempDir, humanitl_audit_log::AuditWriter) {
        let dir = tempfile::tempdir().unwrap();
        let key = humanitl_audit::AuditKey::from_bytes([1; 32], humanitl_audit::KeyOrigin::File);
        let (writer, _) = humanitl_audit_log::AuditWriter::open(
            &dir.path().join("audit.jsonl"),
            &key,
            humanitl_audit_log::WriterOptions::default(),
            &[],
            None,
        )
        .unwrap();
        (dir, writer)
    }

    fn discover_lines(writer: humanitl_audit_log::AuditWriter) -> Vec<String> {
        writer.handle().sync().unwrap();
        let path = writer.path().to_path_buf();
        drop(writer);
        std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .filter(|line| line.contains("\"kind\":\"llm.discover\""))
            .map(str::to_owned)
            .collect()
    }

    /// Eine Netzsuche hinterlässt genau einen `llm.discover`, mit Netz, Ports
    /// und der Zahl der Treffer — auch dann, wenn der Aufrufer vorher geht
    /// (HUM-076: „Audit-Eintrag vorhanden").
    #[tokio::test]
    async fn a_discover_scan_leaves_exactly_one_audit_record() {
        let searched = humanitl_audit::kinds::LlmDiscover {
            subnet: "192.168.1.0/24".to_owned(),
            ports: vec![11434, 8080],
            found: 0,
        };

        // Der Aufrufer bleibt bis zum Ende: zwei Treffer, ein Record.
        let (_dir, writer) = discover_log();
        let (found_tx, found_rx) = tokio::sync::mpsc::channel(16);
        let (tx, mut rx) = tokio::sync::mpsc::channel(16);
        let relay = tokio::spawn(super::relay_scan(
            found_rx,
            tx,
            Some(writer.handle()),
            searched.clone(),
        ));
        found_tx.send("a").await.unwrap();
        found_tx.send("b").await.unwrap();
        drop(found_tx);
        assert_eq!(rx.recv().await, Some("a"));
        assert_eq!(rx.recv().await, Some("b"));
        assert_eq!(rx.recv().await, None);
        relay.await.unwrap();
        let lines = discover_lines(writer);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(
            lines[0].contains("\"subnet\":\"192.168.1.0/24\""),
            "{}",
            lines[0]
        );
        assert!(lines[0].contains("\"ports\":[11434,8080]"), "{}", lines[0]);
        assert!(lines[0].contains("\"found\":2"), "{}", lines[0]);

        // Der Aufrufer geht nach dem ersten Treffer: trotzdem genau einer.
        let (_dir, writer) = discover_log();
        let (found_tx, found_rx) = tokio::sync::mpsc::channel(16);
        let (tx, rx) = tokio::sync::mpsc::channel::<&str>(16);
        drop(rx);
        let relay = tokio::spawn(super::relay_scan(
            found_rx,
            tx,
            Some(writer.handle()),
            searched,
        ));
        found_tx.send("a").await.unwrap();
        relay.await.unwrap();
        assert!(
            found_tx.send("b").await.is_err(),
            "the scan is told to stop once the caller left"
        );
        let lines = discover_lines(writer);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].contains("\"found\":1"), "{}", lines[0]);
    }

    /// Derselbe Record über die RPC selbst, gegen die Schleife.
    ///
    /// Ohne Vorgaberoute hat der Rechner kein eigenes Netz; dann weigert sich
    /// der Dienst mit `LLM_008`, es findet keine Suche statt, und im Log steht
    /// auch keine. Beide Ausgänge werden geprüft, keiner ist ein stilles Grün.
    #[tokio::test]
    async fn the_discover_rpc_writes_its_audit_record() {
        use tokio_stream::StreamExt as _;

        let (_dir, writer) = discover_log();
        let queue = queue();
        let server = server(&queue).with_audit(writer.handle());
        let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = closed.local_addr().unwrap().port();
        drop(closed);
        let answer = server
            .discover_llm(Request::new(v1::DiscoverRequest {
                subnet: "127.0.0.1/32".to_owned(),
                ports: vec![u32::from(port)],
            }))
            .await;
        match answer {
            Err(status) => {
                assert!(status.message().contains("LLM_008"), "{}", status.message());
                assert!(discover_lines(writer).is_empty(), "no search, no record");
            }
            Ok(stream) => {
                let results: Vec<_> = stream.into_inner().collect().await;
                assert!(results.iter().all(Result::is_ok), "{results:?}");
                let lines = discover_lines(writer);
                assert_eq!(lines.len(), 1, "{lines:?}");
                assert!(
                    lines[0].contains("\"subnet\":\"127.0.0.1/32\""),
                    "{}",
                    lines[0]
                );
                assert!(
                    lines[0].contains(&format!("\"ports\":[{port}]")),
                    "{}",
                    lines[0]
                );
            }
        }
    }

    fn queue() -> Arc<HoldQueue> {
        let limits = Limits::default();
        Arc::new(HoldQueue::with_registry(
            &limits,
            Arc::new(FlowRegistry::new(&limits)),
        ))
    }

    fn server(queue: &Arc<HoldQueue>) -> IpcServer {
        IpcServer::new(
            Arc::clone(queue),
            &Config::default(),
            Some(SessionId::new()),
        )
    }

    fn analyzed(session: SessionId, host: &str) -> Flow {
        let request = HttpRequest::new(
            Method::GET,
            Scheme::Https,
            Authority::with_scheme(HostName::Dns(host.to_owned()), Scheme::Https),
            "/v1/x",
        )
        .with_body(BodyRef::detached([0; 32], 7));
        let mut flow = Flow::new(FlowId::new(), session, SystemTime::now(), request);
        flow.apply(
            TransitionInput::Analyze { findings: vec![] },
            SystemTime::now(),
        )
        .unwrap();
        flow
    }

    #[tokio::test]
    async fn get_info_names_the_contract_and_the_session() {
        let queue = queue();
        let server = server(&queue);
        let info = server
            .get_info(Request::new(()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(info.proto_major, crate::PROTO_MAJOR);
        assert_eq!(info.proto_minor, crate::PROTO_MINOR);
        assert_eq!(info.capabilities, CAPABILITIES);
        assert!(!info.session_id.is_empty());
    }

    /// `Doctor` antwortet seit HUM-075 und misst dabei nichts im Netz.
    ///
    /// Der ganze Bericht wird in `tests/ipc_server.rs` geprüft; hier steht die
    /// eine Zusage, die kein Rechner verwässern kann: Der Daemon baut von sich
    /// aus keine Verbindung zum Sprachmodell auf, und die Zeile `llm` sagt
    /// genau das, statt eine Messung zu behaupten.
    #[tokio::test]
    async fn doctor_answers_and_never_contacts_the_endpoint_by_itself() {
        let queue = queue();
        // Its own directories: since HUM-277 the doctor reads config.toml
        // afresh, and the configuration of the person running the tests is
        // none of this test's business.
        let dir = tempfile::tempdir().unwrap();
        let server = server(&queue).with_paths(humanitl_config::Paths::new(
            humanitl_config::Env::from_pairs([
                ("HOME", dir.path().display().to_string()),
                (
                    "XDG_CONFIG_HOME",
                    dir.path().join("config").display().to_string(),
                ),
            ]),
        ));
        let report = server
            .doctor(Request::new(()))
            .await
            .expect("Doctor answers")
            .into_inner();
        let llm = report
            .checks
            .iter()
            .find(|check| check.id == "llm")
            .expect("the llm line");
        assert_ne!(llm.status, v1::CheckStatus::Ok as i32, "{llm:?}");
        assert!(
            llm.evidence
                .starts_with(humanitl_sandbox::doctor::NOT_MEASURED)
                || llm.evidence.contains("not set"),
            "{}",
            llm.evidence
        );
    }

    /// Ein Arbeitsfaden, der nicht zu Ende kam, ist ein Befund mit Grund und
    /// nicht ein nackter `INTERNAL` (HUM-069, Review).
    #[tokio::test]
    async fn an_unfinished_worker_is_a_diagnostic_with_why() {
        let error = tokio::task::spawn_blocking(|| panic!("the worker panics"))
            .await
            .expect_err("the worker panicked");
        let diagnostic = unfinished("SetConfig", &error);
        assert_eq!(diagnostic.code, codes::IPC_006);
        assert!(
            diagnostic.why.contains("SetConfig did not finish"),
            "{}",
            diagnostic.why
        );
        assert!(diagnostic.fix.is_some());
    }

    /// `GetConfig` antwortet seit HUM-069 mit den Werten der Datei, als
    /// JSON, und mit der Datei selbst; das Schema nur auf Wunsch.
    #[tokio::test]
    async fn get_config_answers_with_the_values_and_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let paths = humanitl_config::Paths::new(humanitl_config::Env::from_pairs([
            ("HOME", dir.path().display().to_string()),
            (
                "XDG_CONFIG_HOME",
                dir.path().join("config").display().to_string(),
            ),
        ]));
        std::fs::create_dir_all(paths.config_dir()).unwrap();
        std::fs::write(paths.config_path(), "[ui]\ntheme = \"light\"\n").unwrap();
        let queue = queue();
        let server = server(&queue).with_paths(paths.clone());

        let snapshot = server
            .get_config(Request::new(v1::GetConfigRequest::default()))
            .await
            .unwrap()
            .into_inner();
        let values: serde_json::Value = serde_json::from_str(&snapshot.values_json).unwrap();
        assert_eq!(values["ui"]["theme"], "light");
        assert_eq!(
            snapshot.config_path,
            paths.config_path().display().to_string()
        );
        assert!(snapshot.json_schema.is_empty());

        let with_schema = server
            .get_config(Request::new(v1::GetConfigRequest {
                include_schema: true,
            }))
            .await
            .unwrap()
            .into_inner();
        assert!(with_schema.json_schema.contains("api_key_ref"));
    }

    /// `Doctor` reads the configuration afresh (HUM-277): after
    /// `SetConfig llm.endpoint` the `llm` line names the new endpoint instead
    /// of "llm.endpoint is not set" from the daemon's start.
    #[tokio::test]
    async fn doctor_names_an_endpoint_saved_after_the_start() {
        let dir = tempfile::tempdir().unwrap();
        let paths = humanitl_config::Paths::new(humanitl_config::Env::from_pairs([
            ("HOME", dir.path().display().to_string()),
            (
                "XDG_CONFIG_HOME",
                dir.path().join("config").display().to_string(),
            ),
            (
                "XDG_DATA_HOME",
                dir.path().join("data").display().to_string(),
            ),
            (
                "XDG_RUNTIME_DIR",
                dir.path().join("run").display().to_string(),
            ),
        ]));
        let queue = queue();
        // The daemon started without an endpoint (`Config::default()`).
        let server = server(&queue).with_paths(paths);
        let llm_line = |report: &v1::DoctorReport| {
            report
                .checks
                .iter()
                .find(|check| check.id == "llm")
                .expect("the llm line")
                .clone()
        };
        let before = server.doctor(Request::new(())).await.unwrap().into_inner();
        assert!(
            !llm_line(&before).evidence.contains("llm.example.net"),
            "{:?}",
            llm_line(&before)
        );

        server
            .set_config(Request::new(v1::SetConfigRequest {
                key: "llm.endpoint".to_owned(),
                value: "https://llm.example.net/api/v1/".to_owned(),
                expected_server: String::new(),
            }))
            .await
            .expect("SetConfig writes");
        let after = server.doctor(Request::new(())).await.unwrap().into_inner();
        let line = llm_line(&after);
        let said = format!(
            "{} {}",
            line.evidence,
            line.diagnostic
                .as_ref()
                .map(|diagnostic| diagnostic.why.as_str())
                .unwrap_or_default()
        );
        assert!(said.contains("llm.example.net"), "{line:?}");
        assert!(!said.contains("not set"), "{line:?}");
    }

    /// Ein Netz, das weiter ist als ein `/24`, wird abgelehnt, bevor irgendein
    /// Paket entsteht (HUM-076).
    ///
    /// Die Weigerung ist kein `UNIMPLEMENTED` und kein leerer Strom: Sie nennt
    /// den Grund, damit ein Mensch versteht, dass die Suche im eigenen Netz
    /// bleibt und nicht, dass sein Server nicht gefunden wurde.
    #[tokio::test]
    async fn a_network_wider_than_a_24_is_refused_before_anything_connects() {
        let queue = queue();
        let server = server(&queue);
        for subnet in [
            "10.0.0.0/8",
            "192.168.0.0/16",
            "nonsense",
            "192.168.2.0/33",
            // Genauso breit wie ein eigenes Netz und trotzdem fremdes: Der
            // Scan bleibt in dem Netz, in dem dieser Rechner steht.
            "8.8.8.0/24",
        ] {
            let status = server
                .discover_llm(Request::new(v1::DiscoverRequest {
                    subnet: subnet.to_owned(),
                    ports: Vec::new(),
                }))
                .await
                .err()
                .unwrap_or_else(|| panic!("{subnet} is not a local network"));
            assert_ne!(status.code(), tonic::Code::Unimplemented);
            assert!(status.message().contains("LLM_008"), "{}", status.message());
        }
    }

    /// Mehr Ports, als die Zusage nennt, werden abgelehnt.
    ///
    /// Die Dauer des Scans hängt an ihrer Zahl: Jeder Port multipliziert die
    /// Verbindungsversuche, und „unter fünf Sekunden" ist eine Aussage über
    /// vier davon. Eine Liste aus der Anfrage eines Clients darf sie nicht
    /// aushebeln (Review Codex, 2026-09-06).
    #[tokio::test]
    async fn more_ports_than_the_promise_are_refused() {
        let queue = queue();
        let server = server(&queue);
        for ports in [
            vec![1, 2, 3, 4, 5],
            vec![0],
            vec![70000],
            (1..=1000).collect::<Vec<u32>>(),
        ] {
            let status = server
                .discover_llm(Request::new(v1::DiscoverRequest {
                    subnet: String::new(),
                    ports,
                }))
                .await
                .err()
                .expect("more ports than the search asks");
            assert!(status.message().contains("LLM_008"), "{}", status.message());
        }
    }

    /// Ohne Sandbox sagt der Dienst, dass er keine hat.
    ///
    /// Nicht `UNIMPLEMENTED`: Die RPC gibt es, dieser Daemon hat nur nichts,
    /// wovon er berichten könnte. Eine leere Momentaufnahme wäre die falsche
    /// Antwort — sie sähe aus wie „nichts eingehängt".
    #[tokio::test]
    async fn without_a_sandbox_the_rpc_says_the_daemon_has_none() {
        let queue = queue();
        let server = server(&queue);
        let status = server
            .sandbox(Request::new(v1::SandboxRequest::default()))
            .await
            .err()
            .expect("a daemon without a sandbox must refuse");
        let diagnostic =
            crate::diagnostic_from_status(&status).expect("the refusal carries a diagnostic");
        assert_eq!(diagnostic.code, codes::IPC_006.to_string());
    }

    /// Ein Endpunkt, der keine URL ist, wird gelesen, bevor der Dienst nach
    /// seiner Probe sieht.
    ///
    /// Die Reihenfolge ist beobachtbar und deshalb Teil des Vertrags: Ein
    /// Daemon ohne Probe antwortete sonst `IPC_006` („diese Fähigkeit fehlt"),
    /// wo der Fake `LLM_007` sagt — auf dieselbe Anfrage zwei Antworten. Der
    /// Fake hat nie eine Probe, also prüft `tests/fake_parity.rs` nur die
    /// Fassung mit Probe; diese Zeile deckt die ohne ab.
    #[tokio::test]
    async fn an_unreadable_endpoint_is_refused_before_the_probe_is_looked_up() {
        let queue = queue();
        let mut server = server(&queue);
        server.llm_probe = None;
        server.llm_probe_error = None;

        let Err(status) = server
            .probe_llm(Request::new(v1::ProbeLlmRequest {
                endpoint: "not-a-url".to_owned(),
                timeout_ms: 0,
                ..v1::ProbeLlmRequest::default()
            }))
            .await
        else {
            panic!("an endpoint that is no url is refused, probe or not");
        };
        let diagnostic = crate::server_stub::diagnostic_from_status(&status)
            .expect("the status carries the diagnostic");
        assert_eq!(
            diagnostic.code,
            codes::LLM_007.as_str(),
            "a daemon without a probe must still say what is wrong with the request"
        );
    }

    /// `ProbeLlm` gibt es (HUM-039), und was es nicht lesen kann, lehnt es
    /// ab, statt zu raten.
    #[tokio::test]
    async fn probe_llm_refuses_an_endpoint_it_cannot_read() {
        let queue = queue();
        let server = server(&queue);
        for endpoint in ["", "not-a-url", "ftp://model.lan/"] {
            let Err(status) = server
                .probe_llm(Request::new(v1::ProbeLlmRequest {
                    endpoint: endpoint.to_owned(),
                    timeout_ms: 50,
                    ..v1::ProbeLlmRequest::default()
                }))
                .await
            else {
                panic!("an endpoint that is no http url is refused: {endpoint:?}");
            };
            assert_ne!(
                status.code(),
                tonic::Code::Unimplemented,
                "the rpc exists: {endpoint:?}"
            );
            let diagnostic = crate::server_stub::diagnostic_from_status(&status)
                .expect("the status carries the diagnostic");
            assert_eq!(
                diagnostic.code, "LLM_007",
                "nothing was measured, so no code may claim a measurement — {endpoint:?}: {}",
                diagnostic.why
            );
        }
    }

    /// Ein Ziel, an dem niemand lauscht, ist `LLM_001` samt einem `curl` —
    /// kein Fehler des Nutzers und keine erfundene Modellliste.
    #[tokio::test(flavor = "multi_thread")]
    async fn probe_llm_reports_an_endpoint_that_does_not_answer() {
        let queue = queue();
        let server = server(&queue);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("a free port");
        let port = listener.local_addr().expect("an address").port();
        drop(listener);

        let Err(status) = server
            .probe_llm(Request::new(v1::ProbeLlmRequest {
                endpoint: format!("http://127.0.0.1:{port}"),
                timeout_ms: 2_000,
                ..v1::ProbeLlmRequest::default()
            }))
            .await
        else {
            panic!("nothing is listening on {port}, so the probe must refuse");
        };
        let diagnostic = crate::server_stub::diagnostic_from_status(&status)
            .expect("the status carries the diagnostic");
        assert_eq!(diagnostic.code, "LLM_001", "{}", diagnostic.why);
        assert!(
            matches!(
                diagnostic.fix.as_ref().and_then(|fix| fix.action.as_ref()),
                Some(v1::fix_action::Action::CopyCommand(_))
            ),
            "the fix is a command the human can run: {:?}",
            diagnostic.fix
        );
    }

    #[tokio::test]
    async fn without_a_recording_a_flow_detail_is_not_found_and_a_body_says_why() {
        // `GetFlow` und `GetBody` sind kein `UNIMPLEMENTED` mehr (HUM-026). Ohne
        // Aufzeichnung antwortet der Dienst aus der Registry dieser Sitzung, und
        // was auch die nicht kennt, ist `NOT_FOUND` — nie ein leeres Detail, das
        // wie ein Flow ohne Inhalt aussähe.
        let queue = queue();
        let server = server(&queue);

        let status = server
            .get_flow(Request::new(v1::FlowRef {
                flow_id: FlowId::new().to_string(),
            }))
            .await
            .unwrap_err();
        assert_eq!(status.code(), tonic::Code::NotFound, "{status}");

        // Ein Body dagegen hat ohne Aufzeichnung überhaupt keinen Ort: Der Proxy
        // hält ihn nur, solange die Anfrage läuft. Das sagt der Befund. Die
        // Prüfsumme muss dafür lesbar sein — eine unlesbare wäre `IPC_005`,
        // bevor der Dienst überhaupt nach einer Aufzeichnung sieht.
        let refused = server
            .get_body(Request::new(v1::BodyRef {
                sha256: vec![0u8; 32],
                ..v1::BodyRef::default()
            }))
            .await;
        let Err(status) = refused else {
            panic!("a daemon without a recording has no body to hand out")
        };
        assert!(status.message().contains("RECORDER_001"), "{status}");
    }

    #[tokio::test]
    async fn a_flow_of_this_session_is_answered_from_the_registry() {
        let queue = queue();
        let server = server(&queue);
        let session = SessionId::new();
        let flow = analyzed(session, "api.github.com");
        let id = flow.id;
        queue
            .registry()
            .insert(FlowRecord::new(&flow, &ConnMeta::plain(session)));

        let detail = server
            .get_flow(Request::new(v1::FlowRef {
                flow_id: id.to_string(),
            }))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(detail.summary.expect("a summary").flow_id, id.to_string());
        assert!(!detail.findings_truncated, "nothing was cut short");
    }

    #[tokio::test]
    async fn rules_without_a_store_name_the_reason_instead_of_answering_empty() {
        // `Rules` ist kein `UNIMPLEMENTED` mehr (HUM-027). Ein Daemon ohne
        // Regelspeicher sagt das; eine leere Liste sähe aus wie „keine Regeln".
        let queue = queue();
        let server = server(&queue);
        let status = server
            .rules(Request::new(v1::RulesRequest {
                op: Some(v1::rules_request::Op::List(())),
            }))
            .await
            .unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert!(status.message().contains("IPC_005"), "{status}");
        assert!(status.message().contains("rule store"), "{status}");
    }

    #[tokio::test]
    async fn a_decision_without_a_decision_field_is_refused() {
        let queue = queue();
        let server = server(&queue);
        let status = server
            .decide(Request::new(v1::DecideRequest {
                flow_ids: vec![FlowId::new().to_string()],
                ..v1::DecideRequest::default()
            }))
            .await
            .unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert!(status.message().contains("never an allow"), "{status}");
        assert!(status.message().contains("IPC_004"), "{status}");
    }

    #[tokio::test]
    async fn a_decision_for_a_flow_that_is_not_held_is_failed_precondition() {
        let queue = queue();
        let server = server(&queue);
        let status = server
            .decide(Request::new(v1::DecideRequest {
                flow_ids: vec![FlowId::new().to_string()],
                decision: Some(v1::decide_request::Decision::Allow(())),
                ..v1::DecideRequest::default()
            }))
            .await
            .unwrap_err();
        assert_eq!(status.code(), tonic::Code::FailedPrecondition);
    }

    #[tokio::test]
    async fn an_edited_body_over_the_cap_is_invalid_argument() {
        let queue = queue();
        let config = Config {
            limits: Limits {
                hold_body_cap_bytes: 16,
                ..Limits::default()
            },
            ..Config::default()
        };
        let server = IpcServer::new(Arc::clone(&queue), &config, None);

        let status = server
            .decide(Request::new(v1::DecideRequest {
                flow_ids: vec![FlowId::new().to_string()],
                decision: Some(v1::decide_request::Decision::AllowEdited(
                    v1::EditedRequest {
                        method: v1::Method::Post as i32,
                        url: "https://example.com/x".to_owned(),
                        body: vec![b'x'; 17],
                        ..v1::EditedRequest::default()
                    },
                )),
                ..v1::DecideRequest::default()
            }))
            .await
            .unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert!(status.message().contains("hold_body_cap_bytes"), "{status}");
    }

    #[tokio::test]
    async fn a_slow_listener_gets_a_lagged_event_and_reloads_with_list_flows() {
        use tokio_stream::StreamExt as _;

        let limits = Limits {
            event_buffer: 8,
            ..Limits::default()
        };
        let queue = Arc::new(HoldQueue::with_registry(
            &limits,
            Arc::new(FlowRegistry::new(&limits)),
        ));
        let config = Config {
            limits,
            ..Config::default()
        };
        let server = IpcServer::new(Arc::clone(&queue), &config, None);
        // Der Strom wird angelegt und dann nicht gelesen: genau der Zuhörer,
        // den der Rundfunk überholt.
        let mut stream = server.event_stream(&v1::SubscribeRequest::default());

        let session = SessionId::new();
        for _ in 0..20 {
            let flow = analyzed(session, "example.com");
            queue
                .registry()
                .insert(FlowRecord::new(&flow, &ConnMeta::plain(session)));
            queue.publish(flow.received_event());
        }

        let event = stream.next().await.expect("the stream stays open").unwrap();
        let Some(v1::flow_event::Event::Lagged(lagged)) = event.event else {
            panic!("the first event after the overrun is Lagged, got {event:?}");
        };
        assert!(lagged.dropped > 0, "Lagged names how many events are gone");

        // Nachladen: die Registry kennt jeden Flow, auch die verpassten.
        assert_eq!(
            server.page(&v1::ListFlowsRequest::default(), true).total,
            20
        );
    }

    /// `event_buffer = 8`, der Abonnent liest nicht, 50 Flüsse entstehen: Das
    /// erste gelesene Ereignis ist `Lagged` mit mindestens 42 verlorenen, danach
    /// kommen die aktuellen (HUM-057).
    #[tokio::test]
    async fn subscriber_lag_gets_lagged_event() {
        use tokio_stream::StreamExt as _;

        let limits = Limits {
            event_buffer: 8,
            ..Limits::default()
        };
        let queue = Arc::new(HoldQueue::with_registry(
            &limits,
            Arc::new(FlowRegistry::new(&limits)),
        ));
        let config = Config {
            limits,
            ..Config::default()
        };
        let server = IpcServer::new(Arc::clone(&queue), &config, None);
        let mut stream = server.event_stream(&v1::SubscribeRequest::default());

        let session = SessionId::new();
        let mut ids = Vec::new();
        for _ in 0..50 {
            let flow = analyzed(session, "example.com");
            ids.push(flow.id.to_string());
            queue
                .registry()
                .insert(FlowRecord::new(&flow, &ConnMeta::plain(session)));
            queue.publish(flow.received_event());
        }

        let first = stream.next().await.expect("the stream stays open").unwrap();
        let Some(v1::flow_event::Event::Lagged(lagged)) = first.event else {
            panic!("the first event after the overrun is Lagged, got {first:?}");
        };
        assert!(lagged.dropped >= 42, "dropped {}", lagged.dropped);

        // Danach die aktuellen: die Flüsse, die nach dem Überlauf noch im
        // Puffer standen, bis zum letzten.
        let dropped = usize::try_from(lagged.dropped).unwrap();
        for expected in &ids[dropped..] {
            let event = stream.next().await.expect("the stream stays open").unwrap();
            let Some(v1::flow_event::Event::Received(received)) = event.event else {
                panic!("a current Received after Lagged, got {event:?}");
            };
            let id = received
                .summary
                .as_ref()
                .map(|summary| summary.flow_id.clone())
                .unwrap_or_default();
            assert_eq!(&id, expected);
        }
    }

    /// Ging der eine Befund einer Störung der Aufzeichnung mit einem
    /// Überlauf verloren, folgt er hinter `Lagged` erneut (HUM-250, Review):
    /// Sonst bliebe die Oberfläche grün, während nichts aufgezeichnet wird.
    #[tokio::test]
    async fn a_lag_during_a_recorder_outage_repeats_the_outage() {
        use tokio_stream::StreamExt as _;

        let limits = Limits {
            event_buffer: 8,
            ..Limits::default()
        };
        let queue = Arc::new(HoldQueue::with_registry(
            &limits,
            Arc::new(FlowRegistry::new(&limits)),
        ));
        let config = Config {
            limits,
            ..Config::default()
        };
        let dir = tempfile::tempdir().unwrap();
        let recorder = humanitl_recorder::Recorder::open(
            &dir.path().join("humanitl.db"),
            &dir.path().join("blobs"),
            humanitl_recorder::RecorderSettings::default(),
        )
        .unwrap();
        let server =
            IpcServer::new(Arc::clone(&queue), &config, None).with_recorder(recorder.clone());
        // Der Strom steht, bevor die Störung beginnt: Der Rückstand beim
        // Abonnieren ist leer, und nur die Wiederholung kann sie bringen.
        let mut stream = server.event_stream(&v1::SubscribeRequest::default());

        let session = SessionId::new();
        // Ohne Sitzung scheitert die Zeile: die Störung.
        recorder.apply(&analyzed(session, "lost.example").received_event());
        recorder.flush().await;
        assert_eq!(recorder.outage().len(), 1);
        for _ in 0..50 {
            let flow = analyzed(session, "example.com");
            queue
                .registry()
                .insert(FlowRecord::new(&flow, &ConnMeta::plain(session)));
            queue.publish(flow.received_event());
        }

        let first = stream.next().await.expect("the stream stays open").unwrap();
        assert!(
            matches!(first.event, Some(v1::flow_event::Event::Lagged(_))),
            "{first:?}"
        );
        let second = stream.next().await.expect("the stream stays open").unwrap();
        let Some(v1::flow_event::Event::Diagnostic(diagnostic)) = second.event else {
            panic!("the outage follows the lag, got {second:?}");
        };
        assert_eq!(diagnostic.code, "RECORDER_003");
    }

    /// Das Detail trägt die Befunde, die am Fluss stehen: ein am Cap
    /// geblockter Fluss `LIMIT_001` mit dem Regelvorschlag `stream: true`, ein
    /// anderer keinen (HUM-057, HUM-058 Zeile 9).
    #[tokio::test]
    async fn the_detail_carries_the_diagnostics_of_its_flow() {
        let queue = queue();
        let server = server(&queue);
        let session = SessionId::new();
        let at_cap = analyzed(session, "upload.example.com");
        let quiet = analyzed(session, "upload.example.com");
        for flow in [&at_cap, &quiet] {
            queue
                .registry()
                .insert(FlowRecord::new(flow, &ConnMeta::plain(session)));
        }
        let rule = humanitl_core::Rule::new(
            humanitl_core::RuleId::new(),
            humanitl_core::Action::Ask,
            humanitl_core::Matcher::host(humanitl_core::HostPattern::Exact(HostName::Dns(
                "upload.example.com".to_owned(),
            ))),
        )
        .with_stream(true);
        queue.publish(FlowEvent::Diagnostic {
            flow_id: Some(at_cap.id),
            at: SystemTime::now(),
            diagnostic: Box::new(
                Diagnostic::builder(codes::LIMIT_001, Severity::Warning)
                    .why("the request body is 41943040 bytes".to_owned())
                    .fix(humanitl_core::FixAction::AddRule(Box::new(rule)))
                    .build(),
            ),
        });

        let detail = |id: FlowId| {
            let server = &server;
            async move {
                server
                    .get_flow(Request::new(v1::FlowRef {
                        flow_id: id.to_string(),
                    }))
                    .await
                    .unwrap()
                    .into_inner()
            }
        };
        let blocked = detail(at_cap.id).await;
        assert_eq!(blocked.diagnostics.len(), 1, "{:?}", blocked.diagnostics);
        let limit = &blocked.diagnostics[0];
        assert_eq!(limit.code, "LIMIT_001");
        let Some(v1::fix_action::Action::AddRule(suggested)) =
            limit.fix.as_ref().and_then(|fix| fix.action.clone())
        else {
            panic!("the fix is a rule, got {:?}", limit.fix);
        };
        assert!(suggested.stream, "the suggestion streams");
        assert_eq!(suggested.action, v1::RuleAction::Ask as i32);

        assert!(detail(quiet.id).await.diagnostics.is_empty());
    }

    /// Eine Aufzeichnung in `dir` mit laufender Sitzung `session`.
    fn recording_in(dir: &std::path::Path, session: SessionId) -> humanitl_recorder::Recorder {
        let recorder = humanitl_recorder::Recorder::open(
            &dir.join("humanitl.db"),
            &dir.join("blobs"),
            humanitl_recorder::RecorderSettings::default(),
        )
        .unwrap();
        recorder.start_session(&humanitl_recorder::SessionMeta {
            id: session,
            started_at: SystemTime::now(),
            sandbox_profile: "default".to_owned(),
            llm_endpoint: None,
            work_dir: "/w".to_owned(),
            agent: "opencode".to_owned(),
        });
        recorder
    }

    /// `GetFlow` über eine Anfrage.
    async fn get_flow(server: &IpcServer, id: FlowId) -> v1::FlowDetail {
        server
            .get_flow(Request::new(v1::FlowRef {
                flow_id: id.to_string(),
            }))
            .await
            .unwrap()
            .into_inner()
    }

    /// Nach einem Neustart des Daemons kommen die Befunde eines Flusses aus
    /// der Aufzeichnung: dieselben, in derselben Reihenfolge, mit demselben
    /// `fix` in der Wire-Form wie aus der Registry vorher (HUM-256).
    #[tokio::test]
    async fn get_flow_after_a_restart_takes_the_diagnostics_from_the_recording() {
        let dir = tempfile::tempdir().unwrap();
        let session = SessionId::new();
        let flow = analyzed(session, "upload.example.com");
        let rule = humanitl_core::Rule::new(
            humanitl_core::RuleId::new(),
            humanitl_core::Action::Ask,
            humanitl_core::Matcher::host(humanitl_core::HostPattern::Exact(HostName::Dns(
                "upload.example.com".to_owned(),
            )))
            .with_path_prefixes(vec!["/v1".to_owned()]),
        )
        .with_stream(true)
        .with_expiry(humanitl_core::Expiry::Session(session))
        .created_from(flow.id);
        let offered = [
            Diagnostic::builder(codes::LIMIT_001, Severity::Warning)
                .why("the request body is 41943040 bytes".to_owned())
                .fix(humanitl_core::FixAction::AddRule(Box::new(rule)))
                .build(),
            Diagnostic::builder(codes::LIMIT_004, Severity::Warning)
                .why("the response went silent".to_owned())
                .fix(humanitl_core::FixAction::ChangeSetting {
                    key: "limits.body_timeout_secs".to_owned(),
                    value: "60".to_owned(),
                })
                .build(),
        ];
        let before = {
            let recorder = recording_in(dir.path(), session);
            let queue = Arc::new(
                HoldQueue::with_registry(
                    &Limits::default(),
                    Arc::new(FlowRegistry::new(&Limits::default())),
                )
                .recording(recorder.clone()),
            );
            queue
                .registry()
                .insert(FlowRecord::new(&flow, &ConnMeta::plain(session)));
            queue.publish(flow.received_event());
            for diagnostic in &offered {
                queue.publish(FlowEvent::Diagnostic {
                    flow_id: Some(flow.id),
                    at: SystemTime::now(),
                    diagnostic: Box::new(diagnostic.clone()),
                });
            }
            let server = server(&queue).with_recorder(recorder);
            get_flow(&server, flow.id).await.diagnostics
        };
        assert_eq!(before.len(), 2, "{before:?}");

        // Der neue Daemon: dieselbe Datei, eine leere Registry.
        let queue = queue();
        let server = server(&queue).with_recorder(recording_in(dir.path(), SessionId::new()));
        assert!(queue.registry().get(flow.id).is_none());
        let after = get_flow(&server, flow.id).await.diagnostics;
        let expected: Vec<v1::Diagnostic> = offered
            .iter()
            .map(crate::convert::diagnostic_to_proto)
            .collect();
        assert_eq!(after, expected, "the same diagnostics, order and fix");
        assert_eq!(before, after, "and what the running daemon showed");
    }

    /// Die Registry bleibt der Rückfall der laufenden Sitzung: Kennt sie mehr
    /// Befunde als die Aufzeichnung, gelten ihre, und ihr `why` verlässt den
    /// Daemon maskiert wie aus der Aufzeichnung (HUM-256).
    #[tokio::test]
    async fn the_registry_stays_the_fallback_and_is_masked_like_the_recording() {
        const SECRET: &str = "sk-live-0123456789abcdef";
        let dir = tempfile::tempdir().unwrap();
        let session = SessionId::new();
        let recorder = recording_in(dir.path(), session);
        recorder.mask_secret(SECRET, None);
        let queue = Arc::new(
            HoldQueue::with_registry(
                &Limits::default(),
                Arc::new(FlowRegistry::new(&Limits::default())),
            )
            .recording(recorder.clone()),
        );
        let flow = analyzed(session, "upload.example.com");
        queue
            .registry()
            .insert(FlowRecord::new(&flow, &ConnMeta::plain(session)));
        queue.publish(flow.received_event());
        // Der erste Befund geht in beide, der zweite nur in die Registry, wie
        // bei einer Zeile, deren Schreiben scheiterte.
        queue.publish(FlowEvent::Diagnostic {
            flow_id: Some(flow.id),
            at: SystemTime::now(),
            diagnostic: Box::new(
                Diagnostic::builder(codes::FINDINGS_002, Severity::Warning)
                    .why("the scan stopped early".to_owned())
                    .build(),
            ),
        });
        queue.registry().record(&FlowEvent::Diagnostic {
            flow_id: Some(flow.id),
            at: SystemTime::now(),
            diagnostic: Box::new(
                Diagnostic::builder(codes::LIMIT_001, Severity::Warning)
                    .why(format!("the request to /v1?key={SECRET} is too large"))
                    .build(),
            ),
        });
        let server = server(&queue).with_recorder(recorder);

        let shown = get_flow(&server, flow.id).await.diagnostics;
        let codes: Vec<&str> = shown.iter().map(|d| d.code.as_str()).collect();
        assert_eq!(
            codes,
            ["FINDINGS_002", "LIMIT_001"],
            "the registry knows more"
        );
        assert_eq!(shown[1].why, "the request to /v1?key=•••• is too large");

        // Ein Fluss ohne Zeile, den nur die Registry kennt: derselbe Schutz.
        let unrecorded = analyzed(session, "upload.example.com");
        queue
            .registry()
            .insert(FlowRecord::new(&unrecorded, &ConnMeta::plain(session)));
        queue.registry().record(&FlowEvent::Diagnostic {
            flow_id: Some(unrecorded.id),
            at: SystemTime::now(),
            diagnostic: Box::new(
                Diagnostic::builder(codes::LIMIT_001, Severity::Warning)
                    .why(format!("key {SECRET}"))
                    .build(),
            ),
        });
        let shown = get_flow(&server, unrecorded.id).await.diagnostics;
        assert_eq!(shown.len(), 1, "{shown:?}");
        assert_eq!(shown[0].why, "key ••••");
    }

    /// Gleich lange Listen mit verschiedenem Inhalt: Ein Befund steht nur in
    /// der Aufzeichnung, ein anderer nur in der Registry. `GetFlow` zeigt
    /// beide, in Reihenfolge, statt einen nach der Länge zu verwerfen (HUM-256,
    /// Review).
    #[tokio::test]
    async fn equal_length_lists_are_merged_by_identity() {
        let dir = tempfile::tempdir().unwrap();
        let session = SessionId::new();
        let recorder = recording_in(dir.path(), session);
        let queue = Arc::new(
            HoldQueue::with_registry(
                &Limits::default(),
                Arc::new(FlowRegistry::new(&Limits::default())),
            )
            .recording(recorder.clone()),
        );
        let flow = analyzed(session, "upload.example.com");
        queue
            .registry()
            .insert(FlowRecord::new(&flow, &ConnMeta::plain(session)));
        queue.publish(flow.received_event());
        let scan = Diagnostic::builder(codes::FINDINGS_002, Severity::Warning)
            .why("the scan stopped early".to_owned())
            .build();
        let blocking = Diagnostic::builder(codes::LIMIT_001, Severity::Warning)
            .why("the request body is 41943040 bytes".to_owned())
            .build();
        // Nur in der Aufzeichnung, wie ein Befund vor dem Datensatz.
        recorder.store_diagnostic(flow.id, &scan, |_| String::new());
        // Nur in der Registry, wie eine Zeile, deren Schreiben scheiterte.
        queue.registry().record(&FlowEvent::Diagnostic {
            flow_id: Some(flow.id),
            at: SystemTime::now(),
            diagnostic: Box::new(blocking),
        });
        let server = server(&queue).with_recorder(recorder);

        let shown = get_flow(&server, flow.id).await.diagnostics;
        let codes: Vec<&str> = shown.iter().map(|d| d.code.as_str()).collect();
        assert_eq!(codes, ["FINDINGS_002", "LIMIT_001"]);
    }

    /// Ein Schlüssel, der erst nach dem Schreiben eines Befunds zu den Masken
    /// kommt, steht auch im aufgezeichneten `why` maskiert, und der Befund
    /// erscheint einmal, nicht als zwei verschiedene Fassungen (HUM-256,
    /// Review).
    #[tokio::test]
    async fn a_mask_added_after_recording_applies_and_keeps_one_copy() {
        const SECRET: &str = "sk-live-0123456789abcdef";
        let dir = tempfile::tempdir().unwrap();
        let session = SessionId::new();
        let recorder = recording_in(dir.path(), session);
        let queue = Arc::new(
            HoldQueue::with_registry(
                &Limits::default(),
                Arc::new(FlowRegistry::new(&Limits::default())),
            )
            .recording(recorder.clone()),
        );
        let flow = analyzed(session, "upload.example.com");
        queue
            .registry()
            .insert(FlowRecord::new(&flow, &ConnMeta::plain(session)));
        queue.publish(flow.received_event());
        queue.publish(FlowEvent::Diagnostic {
            flow_id: Some(flow.id),
            at: SystemTime::now(),
            diagnostic: Box::new(
                Diagnostic::builder(codes::LIMIT_001, Severity::Warning)
                    .why(format!("key {SECRET}"))
                    .build(),
            ),
        });
        recorder.flush().await;
        recorder.mask_secret(SECRET, None);
        let server = server(&queue).with_recorder(recorder);

        let shown = get_flow(&server, flow.id).await.diagnostics;
        let whys: Vec<&str> = shown.iter().map(|d| d.why.as_str()).collect();
        assert_eq!(whys, ["key ••••"]);
    }

    /// Das Zusammenführen hält die Reihenfolge, nimmt jeden Befund einmal und
    /// hält die Grenze ein.
    #[test]
    fn merging_keeps_order_drops_duplicates_and_caps() {
        let d = |why: &str| v1::Diagnostic {
            code: "LIMIT_004".to_owned(),
            why: why.to_owned(),
            ..v1::Diagnostic::default()
        };
        let whys = |list: Vec<v1::Diagnostic>| -> Vec<String> {
            list.into_iter().map(|diagnostic| diagnostic.why).collect()
        };
        assert_eq!(
            whys(super::merge_diagnostics(
                &[d("a"), d("c"), d("d")],
                &[d("b"), d("c"), d("e")]
            )),
            ["a", "b", "c", "d", "e"]
        );
        assert_eq!(
            whys(super::merge_diagnostics(
                &[d("a"), d("b")],
                &[d("a"), d("b")]
            )),
            ["a", "b"]
        );
        let many: Vec<v1::Diagnostic> = (0..20).map(|n| d(&n.to_string())).collect();
        assert_eq!(
            super::merge_diagnostics(&many, &[]).len(),
            humanitl_core::MAX_FLOW_DIAGNOSTICS
        );
    }

    /// Ein durchgereichter Fluss ist eingeklappt, seine Warnung nicht.
    ///
    /// Der Filter für `include_passthrough: false` versteckt jedes Ereignis
    /// eines Durchreich-Flusses. Ein Befund darf davon nie betroffen sein:
    /// `LLM_005` warnt vor genau der Anfrage, die hier versteckt wird, und mit
    /// ihr zu verschwinden kehrte die Zusage aus `docs/SECURITY.md` 3.1 um
    /// („ein Treffer erzeugt eine Warnung"). Geprüft wird über
    /// [`IpcServer::event_stream`], nicht über den rohen Rundfunk: Nur der
    /// Strom des Dienstes trägt den Filter, und nur seine Zustellung zählt.
    #[tokio::test]
    async fn a_warning_survives_the_passthrough_filter_that_hides_its_flow() {
        let queue = queue();
        let session = SessionId::new();
        let server = IpcServer::new(Arc::clone(&queue), &Config::default(), Some(session));
        let mut stream = server.event_stream(&v1::SubscribeRequest::default());

        let mut flow = analyzed(session, "192.168.1.50");
        queue
            .registry()
            .insert(FlowRecord::new(&flow, &ConnMeta::plain(session)));
        // Erst die Entscheidung: Danach steht `passthrough = true` am
        // Datensatz, und genau in diesem Zustand liest die Filter-Closure
        // beim Pollen.
        let decided = flow
            .apply(
                TransitionInput::Decide {
                    decision: humanitl_core::Decision::Allow,
                    source: humanitl_core::DecisionSource::Passthrough,
                },
                SystemTime::now(),
            )
            .unwrap();
        queue.publish(decided);
        queue.publish(FlowEvent::Diagnostic {
            flow_id: Some(flow.id),
            at: SystemTime::now(),
            diagnostic: Box::new(
                Diagnostic::builder(codes::LLM_005, Severity::Warning)
                    .why("two potential secrets".to_owned())
                    .build(),
            ),
        });
        queue.publish(FlowEvent::Recorded {
            flow_id: flow.id,
            at: SystemTime::now(),
        });

        let mut seen = Vec::new();
        while let Ok(Some(event)) = tokio::time::timeout(
            std::time::Duration::from_millis(500),
            tokio_stream::StreamExt::next(&mut stream),
        )
        .await
        {
            let event = event.unwrap();
            let Some(inner) = event.event else { continue };
            seen.push(inner);
            if seen.len() == 1 {
                break;
            }
        }

        let Some(v1::flow_event::Event::FlowDiagnostic(warning)) = seen.first() else {
            panic!("the only event that reaches a default subscriber is the warning: {seen:?}");
        };
        assert_eq!(
            warning.flow_id,
            flow.id.to_string(),
            "and it names the flow it warns about, so the details are reachable"
        );
        assert_eq!(
            warning.diagnostic.as_ref().map(|d| d.code.as_str()),
            Some("LLM_005")
        );
    }

    /// Mit `include_passthrough` kommt alles durch, die Warnung eingeschlossen.
    #[tokio::test]
    async fn with_include_passthrough_the_whole_flow_arrives() {
        let queue = queue();
        let session = SessionId::new();
        let server = IpcServer::new(Arc::clone(&queue), &Config::default(), Some(session));
        let mut stream = server.event_stream(&v1::SubscribeRequest {
            include_passthrough: true,
            ..v1::SubscribeRequest::default()
        });

        let mut flow = analyzed(session, "192.168.1.50");
        queue
            .registry()
            .insert(FlowRecord::new(&flow, &ConnMeta::plain(session)));
        let decided = flow
            .apply(
                TransitionInput::Decide {
                    decision: humanitl_core::Decision::Allow,
                    source: humanitl_core::DecisionSource::Passthrough,
                },
                SystemTime::now(),
            )
            .unwrap();
        queue.publish(decided);

        let event = tokio::time::timeout(
            std::time::Duration::from_millis(500),
            tokio_stream::StreamExt::next(&mut stream),
        )
        .await
        .expect("an event")
        .expect("the stream stays open")
        .unwrap();
        assert!(
            matches!(event.event, Some(v1::flow_event::Event::Decided(_))),
            "{event:?}"
        );
    }

    #[tokio::test]
    /// Ohne `order_by` gilt „nach Ankunft, neueste zuerst".
    ///
    /// Die Registry sortierte hier bis HUM-022 aufsteigend, weil sie nur nach
    /// dem Wort `desc` sah; die Aufzeichnung, der Fake und die Beschreibung des
    /// Vertrags sagten absteigend. Der Schlüssel wird jetzt an einer Stelle
    /// gelesen ([`crate::validate::order_by`]), und alle drei sagen dasselbe.
    async fn list_flows_orders_by_arrival_and_honours_the_limit() {
        let queue = queue();
        let server = server(&queue);
        let session = SessionId::new();
        let mut ids = Vec::new();
        for host in ["a.example.com", "b.example.com", "c.example.com"] {
            let flow = analyzed(session, host);
            ids.push(flow.id);
            queue
                .registry()
                .insert(FlowRecord::new(&flow, &ConnMeta::plain(session)));
        }

        let page = server.page(&v1::ListFlowsRequest::default(), true);
        assert_eq!(page.total, 3);
        let order: Vec<String> = page.flows.iter().map(|row| row.flow_id.clone()).collect();
        let expected: Vec<String> = ids.iter().map(ToString::to_string).collect();
        let newest_first: Vec<String> = expected.iter().rev().cloned().collect();
        assert_eq!(order, newest_first);

        let filtered = server.page(
            &v1::ListFlowsRequest {
                filter: "host:b.example.com".to_owned(),
                ..v1::ListFlowsRequest::default()
            },
            true,
        );
        assert_eq!(filtered.flows.len(), 1);

        let first = server.page(
            &v1::ListFlowsRequest {
                limit: 2,
                ..v1::ListFlowsRequest::default()
            },
            true,
        );
        assert_eq!(first.flows.len(), 2);
        assert_eq!(first.next_cursor, newest_first[1]);
    }

    /// Ein hinterlegter Schlüssel geht nur an die Herkunft seines eigenen
    /// Servers (HUM-289): ohne Namen der des aktiven, mit Namen der dieses
    /// Eintrags, und keiner an die Herkunft eines anderen.
    #[tokio::test]
    async fn a_stored_key_goes_only_to_the_origin_of_its_own_server() {
        const HOME_KEY: &str = "sk-home-0123456789abcdef";
        const CLOUD_KEY: &str = "sk-cloud-0123456789abcdef";
        let dir = tempfile::tempdir().unwrap();
        let paths = humanitl_config::Paths::new(humanitl_config::Env::from_pairs([
            ("HOME", dir.path().display().to_string()),
            (
                "XDG_CONFIG_HOME",
                dir.path().join("config").display().to_string(),
            ),
            (
                "XDG_DATA_HOME",
                dir.path().join("data").display().to_string(),
            ),
        ]));
        let keys = humanitl_config::secret::KeyStore::for_paths(&paths).with_secret_tool(None);
        for (name, key) in [("llm", HOME_KEY), ("cloud", CLOUD_KEY)] {
            keys.store_named(name, &humanitl_config::secret::ApiKey::new(key).unwrap())
                .unwrap();
        }
        std::fs::create_dir_all(paths.config_dir()).unwrap();
        std::fs::write(
            paths.config_path(),
            r#"[llm]
active = "home"
endpoint = "http://home.lan:11434/"
api_key_ref = "file:llm"
[llm.servers.home]
endpoint = "http://home.lan:11434/"
api_key_ref = "file:llm"
[llm.servers.cloud]
endpoint = "https://api.example.com/v1"
api_key_ref = "file:cloud"
"#,
        )
        .unwrap();
        let queue = queue();
        let server = server(&queue).with_paths(paths).with_key_store(keys);
        let home = url::Url::parse("http://home.lan:11434/v1").unwrap();
        let cloud = url::Url::parse("https://api.example.com/v1").unwrap();
        let sent =
            |key: Option<humanitl_config::secret::ApiKey>| key.map(|key| key.expose().to_owned());

        let active = server.probe_key(&home, "", true, "").await.unwrap();
        assert_eq!(sent(active).as_deref(), Some(HOME_KEY));
        let elsewhere = server.probe_key(&cloud, "", true, "").await.unwrap();
        assert!(
            elsewhere.is_none(),
            "the key of the active server went to the origin of another server"
        );

        let own = server.probe_key(&cloud, "", true, "cloud").await.unwrap();
        assert_eq!(
            sent(own).as_deref(),
            Some(CLOUD_KEY),
            "a saved server is probed with its own key"
        );
        let crossed = server.probe_key(&home, "", true, "cloud").await.unwrap();
        assert!(
            crossed.is_none(),
            "the key of a saved server went to the origin of the active one"
        );

        let unknown = server.probe_key(&home, "", true, "nope").await.unwrap_err();
        assert_eq!(unknown.code, codes::CONFIG_021);
    }
}
