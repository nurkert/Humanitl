//! `humanitl egress status|check|auth set|auth remove` (HUM-511, ADR-0022).
//!
//! Ein gRPC-Aufruf je Befehl und sonst nichts (ADR-018): Welche Einstellung
//! gilt, ob der Entscheidungs-Proxy erreichbar ist und wo die Zugangsdaten
//! liegen, weiß der Daemon. Die Zugangsdaten kommen nur über `stdin` herein,
//! nie über die Kommandozeile, und keine Antwort trägt sie.

use humanitl_ipc::v1;
use serde_json::json;

use crate::cli::{EgressAuthCmd, EgressCmd};
use crate::cmd::llm::{SecretPrompt, read_key_interruptible};
use crate::cmd::{Context, EXIT_CHECK, EXIT_OK, Failure, from_proto, status_diagnostic};

/// Die Eingabe von `egress auth set`.
const CREDENTIALS: SecretPrompt = SecretPrompt {
    what: "proxy credentials USER:PASSWORD",
    fix: "printf '%s' \"$USER_COLON_PASSWORD\" | humanitl egress auth set",
};

/// Exit-Code 130 nach `Ctrl+C` an der verdeckten Eingabe.
const EXIT_INTERRUPTED: u8 = 130;

/// Führt `humanitl egress <cmd>` aus.
///
/// # Errors
///
/// `DAEMON_001`, wenn kein Daemon antwortet; der Befund des Daemons, wenn er
/// ablehnt. Ein Check, der scheitert, endet mit [`EXIT_CHECK`] und dem
/// Befund.
pub async fn run(ctx: &Context, cmd: &EgressCmd) -> Result<u8, Failure> {
    let op = match cmd {
        EgressCmd::Status => v1::egress_request::Op::Status(v1::egress_request::Status {}),
        EgressCmd::Check => {
            v1::egress_request::Op::CheckConnection(v1::egress_request::CheckConnection {})
        }
        EgressCmd::Auth {
            cmd: EgressAuthCmd::Set,
        } => {
            let Some(credentials) = read_key_interruptible(&CREDENTIALS)
                .await
                .map_err(Failure::new)?
            else {
                return Ok(EXIT_INTERRUPTED);
            };
            v1::egress_request::Op::SetAuth(v1::egress_request::SetAuth { credentials })
        }
        EgressCmd::Auth {
            cmd: EgressAuthCmd::Remove,
        } => v1::egress_request::Op::RemoveAuth(v1::egress_request::RemoveAuth {}),
    };
    let mut client = ctx.connect().await?;
    let status = client
        .egress(v1::EgressRequest { op: Some(op) })
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "Egress")))?
        .into_inner();
    print_status(ctx, &status);
    for diagnostic in status.diagnostics.iter().filter_map(from_proto) {
        ctx.render.diagnostic(&diagnostic);
    }
    if matches!(cmd, EgressCmd::Check) && !status.reachable {
        return Ok(EXIT_CHECK);
    }
    Ok(EXIT_OK)
}

/// Ein Weg in Worten: direkt, oder über wen und wer auflöst.
fn route_text(route: Option<&v1::EgressRoute>) -> String {
    match route {
        Some(route) if !route.proxy.is_empty() => {
            let resolver = if route.resolve == "humanitl" {
                "Humanitl resolves"
            } else {
                "the proxy resolves"
            };
            format!("through {} ({resolver})", route.proxy)
        }
        _ => "direct".to_owned(),
    }
}

fn print_status(ctx: &Context, status: &v1::EgressStatus) {
    let configured = route_text(status.configured.as_ref());
    let active = route_text(status.active.as_ref());
    if ctx.render.is_json() {
        ctx.render.value(&json!({
            "configured": status.configured.as_ref().map(|r| json!({"proxy": r.proxy, "resolve": r.resolve})),
            "active": status.active.as_ref().map(|r| json!({"proxy": r.proxy, "resolve": r.resolve})),
            "proxy_ca": status.proxy_ca,
            "auth_set": status.auth_set,
            "auth_ref": status.auth_ref,
            "check_target": status.check_target,
            "reachable": status.reachable,
        }));
        return;
    }
    ctx.render.line(&format!("Egress: {configured}"));
    if !status.proxy_ca.is_empty() {
        ctx.render.line(&format!("CA: {}", status.proxy_ca));
    }
    ctx.render.line(&format!(
        "Credentials: {}",
        if status.auth_set {
            status.auth_ref.as_str()
        } else {
            "none"
        }
    ));
    if configured != active {
        ctx.render.note(&format!(
            "The running daemon still goes {active}; config.toml takes effect after `humanitl \
             daemon restart`."
        ));
    }
    if !status.check_target.is_empty() {
        ctx.render.line(&format!(
            "Check {}: {}",
            status.check_target,
            if status.reachable {
                "tunnel and TLS handshake through the proxy succeeded"
            } else {
                "failed"
            }
        ));
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_ipc::v1;

    use super::route_text;

    #[test]
    fn a_route_reads_as_direct_or_through_whom() {
        assert_eq!(route_text(None), "direct");
        assert_eq!(route_text(Some(&v1::EgressRoute::default())), "direct");
        let route = v1::EgressRoute {
            proxy: "127.0.0.1:8080".to_owned(),
            resolve: "proxy".to_owned(),
        };
        assert_eq!(
            route_text(Some(&route)),
            "through 127.0.0.1:8080 (the proxy resolves)"
        );
    }
}
