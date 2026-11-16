//! Die Befunde des Wegs über einen Entscheidungs-Proxy (HUM-511, ADR-0022).
//!
//! Jeder nennt den Proxy, das Ziel und die Einstellung, die hilft; der
//! Weiterleiter reicht sie über [`crate::upstream::ForwardError`] an den
//! Fluss, statt sie auf einen nackten [`humanitl_core::UpstreamError`] zu
//! kürzen.

use std::time::Duration;

use humanitl_core::diagnostics::codes::{
    PROXY_003, PROXY_012, PROXY_013, PROXY_014, PROXY_015, PROXY_017, PROXY_018, PROXY_019,
};
use humanitl_core::{Authority, Diagnostic, FixAction, Severity};

use super::http_proxy::ProxyEndpoint;

/// `PROXY_012`: der Proxy ist nicht erreichbar.
pub(crate) fn unreachable(endpoint: &ProxyEndpoint, detail: &str) -> Diagnostic {
    Diagnostic::builder(PROXY_012, Severity::Error)
        .why(format!(
            "the decision proxy at {endpoint} (egress.proxy) is not reachable: {detail}. The \
             request did not leave directly instead."
        ))
        .fix(FixAction::CopyCommand("humanitl egress check".to_owned()))
        .build()
}

/// `PROXY_013`: der Proxy hat den Tunnel nicht aufgebaut.
pub(crate) fn refused(endpoint: &ProxyEndpoint, target: &str, detail: &str) -> Diagnostic {
    Diagnostic::builder(PROXY_013, Severity::Error)
        .why(format!(
            "the decision proxy at {endpoint} did not open a tunnel to {target}: {detail}"
        ))
        .fix(FixAction::CopyCommand("humanitl egress check".to_owned()))
        .build()
}

/// `PROXY_014`: der Proxy verlangt Zugangsdaten (`407`).
#[must_use]
pub fn wants_credentials(endpoint: &impl std::fmt::Display, what: &str) -> Diagnostic {
    Diagnostic::builder(PROXY_014, Severity::Error)
        .why(format!(
            "the decision proxy at {endpoint} answered {what} with 407 Proxy Authentication \
             Required; egress.proxy_auth_ref is empty or names other credentials than the proxy \
             expects"
        ))
        .fix(FixAction::CopyCommand(
            "humanitl egress auth set".to_owned(),
        ))
        .build()
}

/// `PROXY_017`: eine Einstellung des Proxys taugt nicht.
pub(crate) fn unusable(key: &str, detail: &str) -> Diagnostic {
    Diagnostic::builder(PROXY_017, Severity::Error)
        .why(format!(
            "{key} cannot be used: {detail}. Every request of the agent fails with this finding \
             until it is fixed and the daemon restarted; nothing leaves directly."
        ))
        .fix(FixAction::ChangeSetting {
            key: key.to_owned(),
            value: String::new(),
        })
        .build()
}

/// `PROXY_015`: das Zertifikat durch den Tunnel stammt von keiner
/// vertrauten Stelle; meist fehlt die CA des Proxys.
#[must_use]
pub fn ca_mismatch(proxy: &str, authority: &Authority) -> Diagnostic {
    Diagnostic::builder(PROXY_015, Severity::Error)
        .why(format!(
            "the certificate for {authority} that came through the decision proxy at {proxy} is \
             signed by none of the trusted CAs; egress.proxy_ca is empty or names another CA than \
             the one this proxy signs with. That CA is trusted only on this way, never directly."
        ))
        .fix(FixAction::ChangeSetting {
            key: "egress.proxy_ca".to_owned(),
            value: "/home/USER/.mitmproxy/mitmproxy-ca-cert.pem".to_owned(),
        })
        .build()
}

/// `PROXY_018`: der TLS-Handschlag durch den Tunnel scheiterte aus einem
/// anderen Grund als einer unbekannten Stelle, etwa ein Zertifikat für
/// einen anderen Namen, oder er kam nicht in der Frist zustande.
#[must_use]
pub fn tls_failed(proxy: &str, authority: &Authority, detail: &str) -> Diagnostic {
    Diagnostic::builder(PROXY_018, Severity::Error)
        .why(format!(
            "the TLS handshake with {authority} through the decision proxy at {proxy} failed: \
             {detail}. The proxy presents a certificate of its own for every target; one that \
             does not name {host} usually means the proxy does not intercept this host as \
             expected.",
            host = authority.host.display()
        ))
        .fix(FixAction::CopyCommand("humanitl egress check".to_owned()))
        .build()
}

/// `PROXY_019`: über den Proxy kam der Kopf der Antwort nicht in
/// `egress.proxy_response_timeout_secs`; dort entscheidet vielleicht noch
/// ein Mensch.
#[must_use]
pub fn response_late(proxy: &str, authority: &Authority, waited: Duration) -> Diagnostic {
    Diagnostic::builder(PROXY_019, Severity::Error)
        .why(format!(
            "the answer to the request to {authority} did not arrive through the decision proxy \
             at {proxy} within {}s (egress.proxy_response_timeout_secs); a person deciding there \
             may need longer. The agent got 502.",
            waited.as_secs()
        ))
        .fix(FixAction::ChangeSetting {
            key: "egress.proxy_response_timeout_secs".to_owned(),
            value: "600".to_owned(),
        })
        .build()
}

/// `PROXY_003` für eine Prüfung des Entscheidungs-Proxys, die ohne eigenen
/// Befund scheiterte.
#[must_use]
pub fn check_failed(detail: &str) -> Diagnostic {
    Diagnostic::builder(PROXY_003, Severity::Error)
        .why(format!("the check of the decision proxy failed: {detail}"))
        .fix(FixAction::CopyCommand("humanitl doctor".to_owned()))
        .build()
}

/// `PROXY_017`: ohne `egress.proxy` gibt es nichts zu prüfen.
#[must_use]
pub fn nothing_to_check() -> Diagnostic {
    Diagnostic::builder(PROXY_017, Severity::Error)
        .why(
            "egress.proxy is empty, so there is no decision proxy to check; requests leave \
             directly"
                .to_owned(),
        )
        .fix(FixAction::ChangeSetting {
            key: "egress.proxy".to_owned(),
            value: "http://127.0.0.1:8080".to_owned(),
        })
        .build()
}
