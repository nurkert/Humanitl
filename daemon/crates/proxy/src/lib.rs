//! Moderierender MITM-Proxy: Hold-Queue, Egress-Port, Flow-Ablauf.
//!
//! Siehe `docs/ARCHITECTURE.md` für die Schichtung und `backlog/CONVENTIONS.md`
//! Abschnitt 3.1 für die erlaubten Abhängigkeiten dieser Crate.
//!
//! # Aufbau (HUM-015)
//!
//! - [`core`]: [`ProxyCore`] bindet je Sitzung einen Unix-Socket und nimmt
//!   darauf Verbindungen an. Es gibt keinen Loopback-TCP-Port auf dem Host:
//!   der Spike aus `backlog/CONVENTIONS.md` 4.10 hat gezeigt, dass hudsucker
//!   0.24.1 nur einen `TcpListener` annimmt, und dass sein Client und seine
//!   CA ohnehin durch den Egress-Port und die eigene CA ersetzt würden. Die
//!   Accept-Schleife liegt deshalb direkt auf hyper 1 und `tokio-rustls`.
//! - [`handler`]: [`FlowHandler`] terminiert HTTP/1.1 und, nach `CONNECT`,
//!   TLS mit einem Leaf aus der eigenen CA; puffert den Request-Body bis
//!   `limits.hold_body_cap_bytes`; treibt den Zustandsautomaten des Kerns.
//! - [`pipeline`]: [`FlowPipeline`] entscheidet über einen analysierten Flow;
//!   [`RulesPipeline`] wertet den Regelsatz aus, [`AskPipeline`] hält alles,
//!   was `ask` ergibt, über die [`HoldQueue`].
//! - [`upstream`]: nach `Allow` auflösen ([`Resolver`]-Port), Adresse
//!   anheften, über den [`Egress`]-Port verbinden, HTTP/1.1 sprechen.
//! - [`resolver`]: der eine Ort, an dem ein Name zu einer Adresse wird
//!   (HUM-024, ADR-006). [`ResolverPort`] ist der verdrahtete Stapel aus festen
//!   Zuordnungen, Zwischenspeicher und Namensdienst; [`ResolverPort::stats`]
//!   liefert die Zähler, die `daemon status --json` zeigt.
//! - [`connect`], [`tls`]: der [`ConnectionContext`] einer Verbindung und die
//!   Prüfung, dass CONNECT-Ziel, SNI und `Host` dasselbe Ziel meinen
//!   (HUM-023). Nur ihr Ergebnis wird ausgewertet, nie das CONNECT-Ziel
//!   allein.
//! - [`findings`]: der Port für die Detektoren (HUM-025).
//! - [`llm_probe`]: die host-seitige Probe des LLM-Endpunkts (HUM-039). Sie
//!   liegt hier, weil sie denselben [`Upstream`] benutzt wie der Proxy und
//!   damit dasselbe Auflösungs- und Verbindungsverhalten hat.
//! - [`ca`], [`hold`]: CA und Halte-Warteschlange (HUM-014, HUM-016). Die
//!   Warteschlange ist zugleich der eine Trichter, durch den jedes Ereignis
//!   geht: dort hängen die Aufzeichnung ([`HoldQueue::recording`], HUM-026)
//!   und der Domain-Katalog ([`DomainSink`], HUM-031).
//! - [`rules_store`]: [`RulesStore`] hält den geltenden Regelsatz aus
//!   `rules.yaml`, den Sitzungsregeln und den mitgelieferten Regeln und
//!   schreibt Änderungen atomar zurück (HUM-027).
//! - [`registry`]: [`FlowRegistry`] hält je Flow einen [`FlowRecord`] und
//!   beantwortet `ListFlows`; sie teilt sich den Ereignisstrom mit der
//!   [`HoldQueue`] (HUM-016).
//! - [`tls_observe`]: deutet einen gescheiterten Handschlag des Clients und
//!   macht ihn als Befund und als Flow sichtbar (HUM-045).
//! - [`meta`]: der reservierte Host `humanitl.internal`, den der Proxy selbst
//!   beantwortet — ohne Namensauflösung, ohne Upstream, ohne Regelauswertung
//!   (HUM-073, ADR-014). Die Weiche dorthin liegt im [`handler`], vor beidem.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod body;
pub mod ca;
pub mod connect;
pub mod core;
pub mod egress;
pub mod findings;
pub mod handler;
pub mod hold;
pub mod listener;
pub mod llm_probe;
pub mod meta;
pub mod pipeline;
pub mod registry;
pub mod resolver;
pub mod rules_store;
pub mod session;
pub mod tls;
pub mod tls_observe;
pub mod upstream;

pub use crate::connect::{
    AuthorityError, AuthorityRefusal, ConnMeta, ConnectionContext, RequestTarget, check_authority,
};
pub use crate::core::ProxyCore;
pub use crate::egress::{AsyncStream, Direct, Egress};
pub use crate::findings::{NoScan, Scanner, Tier1Scanner};
pub use crate::handler::{FlowHandler, HandlerPorts, ProxyLimits};
pub use crate::hold::{DomainSink, HoldQueue};
pub use crate::listener::SessionSocket;
pub use crate::llm_probe::{LlmFlavor, LlmProbe, ProbeResult, not_private_by_name};
pub use crate::meta::{
    META_HOST, MetaClock, MetaEndpoint, MetaOutcome, MetaReply, MetaRequest, MetaStatus,
    SuggestedTarget, SystemClock, is_meta_host, suggested_target,
};
pub use crate::pipeline::{AskPipeline, FlowPipeline, PassthroughPipeline, RulesPipeline};
pub use crate::registry::{FlowFilter, FlowRecord, FlowRegistry, FlowSummary};
pub use crate::resolver::{
    AddressRefusal, CachingResolver, OverrideResolver, ResolveError, Resolver, ResolverMetrics,
    ResolverPort, ResolverStats, SystemResolver,
};
pub use crate::rules_store::{Origin, ReloadReport, RulesStore, StoredRule};
pub use crate::session::{SessionSettings, SessionState};
pub use crate::tls_observe::{
    HandshakeWatch, TlsFailure, ToolHint, classify, diagnostic_for, tool_hint,
};
pub use crate::upstream::{ClientTls, Upstream};
