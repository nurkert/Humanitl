//! gRPC-Server und Abbildung zwischen Protobuf und Kern-Typen. Einzige Crate mit Protobuf.
//!
//! Siehe `docs/ARCHITECTURE.md` für die Schichtung und `backlog/CONVENTIONS.md`
//! Abschnitt 3.1 für die erlaubten Abhängigkeiten dieser Crate.
//!
//! Der Vertrag selbst steht in `proto/humanitl/v1/`. Der Rust-Code dazu wird
//! bei jedem Build von `build.rs` erzeugt und liegt in `OUT_DIR`, nie im
//! Quellbaum; er ist niemals von Hand zu ändern. Wie man den Vertrag ändert,
//! steht in `docs/PROTOCOL.md`.
//!
//! Von Hand geschrieben ist der Rest:
//!
//! - [`convert`] mit der Abbildung zwischen Kern-Typen und Wire-Form; jede
//!   Übersetzung steht dort genau einmal,
//! - [`auth`] mit dem Sitzungs-Token: erzeugen, ablegen, prüfen,
//! - [`domains`] mit dem Domain-Katalog am Ereignisstrom (HUM-031),
//! - [`server`] mit [`IpcServer`], dem Dienst des echten Daemons über
//!   Registry und Halte-Warteschlange (HUM-018),
//! - [`rules`] mit dem Regel-RPC über dem Regelspeicher des Proxys
//!   (HUM-027),
//! - [`sandbox`] mit der `Sandbox`-RPC: Einhängungen, Umgebung,
//!   Kommandozeile, Start und Stopp (HUM-040),
//! - [`client`] mit dem Gegenstück für CLI, Oberfläche und Tests,
//! - [`server_stub`] mit dem Port [`DaemonApi`] und dem tonic-Dienst darüber,
//! - [`fake`] mit einem Daemon, der statt eines Proxys eine aufgezeichnete
//!   Sitzung spielt (`humanitld --fake`, HUM-005),
//! - [`validate`] mit den Prüfungen, die für beide Dienste dieselben sind.
//!
//! [`server`] und [`fake`] erfüllen denselben Vertrag (ADR-0003, ADR-0018);
//! ein Client soll den Unterschied nicht sehen können. Alles, was dafür an
//! einer Anfrage zu prüfen ist, bevor sie wirkt, steht in [`validate`] und
//! nirgends sonst; `tests/fake_parity.rs` fährt beide Dienste durch dieselbe
//! Tabelle von Anfragen und vergleicht gRPC-Code und Befund.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// Major-Version des Vertrags. Ein Client mit kleinerer Major verweigert die
/// Verbindung (`Info.proto_major`).
pub const PROTO_MAJOR: u32 = 1;

/// Minor-Version des Vertrags. Steigt bei jeder additiven Änderung
/// (`Info.proto_minor`).
///
/// `4` seit der Sitzungskonfiguration am Start (`Start.session_profile`,
/// `Start.ask_mode`, `Start.cli_overrides`) und den beiden Ereignissen, die
/// den Agenten begleiten (`SandboxEvent.output`, `SandboxEvent.exit`,
/// HUM-067);
/// `3` war die Sandbox-Momentaufnahme mit `Mount`, `EnvVar`, `argv_preview`
/// und `agent_running` samt der Operation `SandboxRequest.Plan` (HUM-040);
/// `2` war `ProbeLlm` samt `Rule.passthrough_llm` und
/// `RuleMatcher.path_prefixes` (HUM-039); `1` war `FlowDetail.findings_truncated`
/// (HUM-026, Feld 10). Die
/// Spiegelung in `app/lib/core/ipc/proto_version.dart` darf nachziehen: eine
/// abweichende Minor ist verabredetermaßen kein Grund, die Verbindung
/// abzulehnen (`docs/PROTOCOL.md`).
pub const PROTO_MINOR: u32 = 4;

/// Metadata-Schlüssel für das Session-Token aus
/// `$XDG_RUNTIME_DIR/humanitl/token` (CONVENTIONS.md 3.6).
pub const TOKEN_METADATA_KEY: &str = "x-humanitl-token";

/// Der erzeugte Vertrag `humanitl.v1`: Nachrichten, Client und Server.
///
/// Der Inhalt ist generiert, deshalb sind die Lints hier abgeschaltet. Alles,
/// was von Hand geschrieben wird, gehört in ein Nachbarmodul, nicht hierher.
#[allow(
    missing_docs,
    clippy::all,
    clippy::pedantic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::doc_markdown
)]
pub mod v1 {
    include!(concat!(env!("OUT_DIR"), "/humanitl.v1.rs"));
}

pub mod auth;
pub mod client;
pub mod convert;
pub mod domains;
pub mod fake;
pub mod rules;
pub mod sandbox;
pub mod server;
pub mod server_stub;
pub mod session;
pub mod validate;

pub use crate::client::connect;
pub use crate::convert::diagnostic_to_proto;
pub use crate::domains::DomainTable;
pub use crate::rules::RulesService;
pub use crate::sandbox::SandboxService;
pub use crate::server::{IpcServer, bind_socket, serve};
pub use crate::server_stub::{
    BoxStream, DaemonApi, DaemonService, diagnostic_from_status, diagnostic_to_status,
};
