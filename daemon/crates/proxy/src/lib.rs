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
//! - [`connect`], [`tls`]: der [`ConnectionContext`] einer Verbindung und die
//!   Prüfung, dass CONNECT-Ziel, SNI und `Host` dasselbe Ziel meinen
//!   (HUM-023). Nur ihr Ergebnis wird ausgewertet, nie das CONNECT-Ziel
//!   allein.
//! - [`findings`]: der Port für die Detektoren (HUM-025).
//! - [`ca`], [`hold`]: CA und Halte-Warteschlange (HUM-014, HUM-016).
//! - [`rules_store`]: [`RulesStore`] hält den geltenden Regelsatz aus
//!   `rules.yaml`, den Sitzungsregeln und den mitgelieferten Regeln und
//!   schreibt Änderungen atomar zurück (HUM-027).
//! - [`registry`]: [`FlowRegistry`] hält je Flow einen [`FlowRecord`] und
//!   beantwortet `ListFlows`; sie teilt sich den Ereignisstrom mit der
//!   [`HoldQueue`] (HUM-016).
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
pub mod pipeline;
pub mod registry;
pub mod resolver;
pub mod rules_store;
pub mod tls;
pub mod upstream;

pub use crate::connect::{
    AuthorityError, AuthorityRefusal, ConnMeta, ConnectionContext, RequestTarget, check_authority,
};
pub use crate::core::ProxyCore;
pub use crate::egress::{AsyncStream, Direct, Egress};
pub use crate::findings::{NoScan, Scanner, Tier1Scanner};
pub use crate::handler::{FlowHandler, ProxyLimits};
pub use crate::hold::HoldQueue;
pub use crate::listener::SessionSocket;
pub use crate::pipeline::{AskPipeline, FlowPipeline, PassthroughPipeline, RulesPipeline};
pub use crate::registry::{FlowFilter, FlowRecord, FlowRegistry, FlowSummary};
pub use crate::resolver::{ResolveError, Resolver, SystemResolver};
pub use crate::rules_store::{Origin, ReloadReport, RulesStore, StoredRule};
pub use crate::upstream::{ClientTls, Upstream};
