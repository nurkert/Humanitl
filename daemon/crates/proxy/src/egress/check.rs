//! `humanitl egress check` (HUM-511): Tunnel und TLS-Handschlag zu einem
//! festen Ziel über den Entscheidungs-Proxy, ohne eine Anfrage.

use std::sync::Arc;
use std::time::Duration;

use humanitl_config::Config;
use humanitl_config::secret::KeyStore;
use humanitl_core::{Authority, Diagnostic, HostName, HttpRequest, Method, Scheme};

use super::decision_proxy;
use super::diagnostics::{check_failed, nothing_to_check};
use crate::resolver::ResolverPort;
use crate::trust::TrustAnchors;
use crate::upstream::{ClientTls, Upstream};

/// Das feste Ziel der Prüfung.
pub const CHECK_TARGET: (&str, u16) = ("example.com", 443);

/// Prüft den Entscheidungs-Proxy aus `config`.
///
/// Ein eigener Stapel mit eigenem Resolver-Port, wie die Endpunkt-Probe:
/// Der Zähler des Proxy-Resolvers belegt, dass vor einer Freigabe kein Name
/// aufgelöst wird (ADR-006), und eine Prüfung, die ein Mensch anstößt,
/// gehört nicht in diesen Beweis. Mit `egress.proxy_resolve = "humanitl"`
/// löst sie das feste Ziel auf.
///
/// # Errors
///
/// `PROXY_017` ohne `egress.proxy` oder mit Einstellungen, die nicht taugen;
/// sonst der Befund des Wegs (`PROXY_012` bis `PROXY_015`, `PROXY_018`), oder
/// `PROXY_003` mit dem Grund.
pub async fn run(config: &Config, keys: Option<KeyStore>) -> Result<(), Diagnostic> {
    let egress = config.egress.clone();
    let trust = config.tls.trust_system_store;
    let connect = Duration::from_secs(config.limits.connect_timeout_secs);
    // Der Schlüsselbund blockiert; das Lesen läuft deshalb abseits.
    let proxy = tokio::task::spawn_blocking(move || {
        decision_proxy(
            &egress,
            &TrustAnchors::configured(trust),
            &[],
            false,
            connect,
            keys.as_ref(),
        )
    })
    .await
    .map_err(|err| check_failed(&format!("the check did not finish ({err})")))??
    .ok_or_else(nothing_to_check)?;
    if let Some(problem) = proxy.problem() {
        return Err(problem.clone());
    }
    let resolver = Arc::new(ResolverPort::from_config(&config.resolver)?);
    let upstream = Upstream::new(
        Arc::new(proxy),
        resolver,
        ClientTls::configured(trust, &[], false)?,
        config.resolver.prefer,
        Duration::from_secs(config.limits.header_timeout_secs),
    );
    let (host, port) = CHECK_TARGET;
    let request = HttpRequest::new(
        Method::HEAD,
        Scheme::Https,
        Authority::new(HostName::Dns(host.to_owned()), port),
        "/".to_owned(),
    );
    upstream.handshake(&request).await.map_err(|failure| {
        failure.diagnostic.map_or_else(
            || {
                check_failed(&format!(
                    "{host}:{port} could not be reached: {}",
                    failure.error
                ))
            },
            |diagnostic| *diagnostic,
        )
    })
}
