//! Was der Weiterleiter vor einem Entscheidungs-Proxy ablehnt, bevor
//! irgendetwas aufgelöst oder gesendet wird (ADR-0022, HUM-511).
//!
//! Zwei Prüfungen, getrennt nach dem, wofür sie gelten:
//!
//! - [`local_name`] gilt vor **jedem** zweiten Proxy, gleich wer auflöst: Ein
//!   per Definition lokaler Name meint den Rechner, auf dem er aufgelöst
//!   oder verbunden wird. Auch eine Auflösung in Humanitl (etwa über
//!   `resolver.overrides`) macht ihn nicht zu einem öffentlichen Ziel.
//! - [`literal`] gilt, wenn Humanitl nicht auflöst
//!   (`egress.proxy_resolve = "proxy"`): ein IP-Literal wird auf private
//!   Netze geprüft, ein Name geht als Name weiter.

use std::net::IpAddr;

use humanitl_core::{HostName, HttpRequest, UpstreamError, ip_is_private};

use super::http_proxy::is_local_name;
use crate::upstream::ForwardError;

/// Lehnt einen lokalen Namen ab, außer eine Regel erlaubt private Ziele.
///
/// # Errors
///
/// [`ForwardError::local_name`]: [`UpstreamError::Dns`], und der Handler
/// stellt `PROXY_016` mit einer passenden Regel an den Fluss.
pub fn local_name(request: &HttpRequest, allow_private: bool) -> Result<(), ForwardError> {
    match &request.authority.host {
        HostName::Dns(name) if is_local_name(name) && !allow_private => {
            Err(ForwardError::local_name())
        }
        _ => Ok(()),
    }
}

/// Prüft ein Ziel, das Humanitl nicht auflöst: die Adresse eines Literals,
/// für einen Namen `None`.
///
/// # Errors
///
/// [`UpstreamError::PrivateAddress`] für ein privates Literal (der Handler
/// meldet `PROXY_008`), [`UpstreamError::Dns`] für den Namen des
/// Sprachmodells, der nie hinausgeht.
pub fn literal(request: &HttpRequest, allow_private: bool) -> Result<Option<IpAddr>, ForwardError> {
    match &request.authority.host {
        HostName::Ip(ip) => {
            if ip_is_private(*ip) && !allow_private {
                return Err(UpstreamError::PrivateAddress(*ip).into());
            }
            Ok(Some(*ip))
        }
        HostName::Dns(_) if request.authority.host.is_llm_alias() => Err(UpstreamError::Dns.into()),
        HostName::Dns(_) => Ok(None),
    }
}
