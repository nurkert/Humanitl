//! Der Resolver-Port: Namen zu Adressen, aber erst nach der Entscheidung.
//!
//! Der Proxy löst einen Hostnamen ausschließlich hier auf, und ausschließlich
//! nachdem eine Anfrage erlaubt wurde (ADR-006, `backlog/CONVENTIONS.md` 4.10:
//! kein `GaiResolver`, kein DNS im Connector). So kann eine Anfrage, die noch
//! in der Warteschlange liegt oder geblockt wird, keine DNS-Abfrage auslösen —
//! das wäre bereits ein beobachtbares Signal nach außen.
//!
//! Der MVP-Adapter [`SystemResolver`] fragt den Namensdienst des Systems
//! (`tokio::net::lookup_host`, also getaddrinfo in einem Blocking-Pool). Ein
//! späterer `hickory`-Adapter (ADR, Post-MVP) ersetzt nur diese Datei. Die
//! aufgelöste Adresse heftet der Handler an und gibt sie an
//! [`Egress::connect`](crate::egress::Egress::connect); es wird nie ein zweites
//! Mal aufgelöst.

use std::net::IpAddr;

use async_trait::async_trait;
use humanitl_config::IpPreference;

/// Warum eine Auflösung fehlschlug.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResolveError {
    /// Der Name ergab keine Adresse.
    #[error("no address for {host}")]
    NotFound {
        /// Der gefragte Name.
        host: String,
    },
    /// Der Namensdienst antwortete mit einem Fehler.
    #[error("resolving {host} failed: {reason}")]
    Failed {
        /// Der gefragte Name.
        host: String,
        /// Der Grund, so wie ihn das System nennt.
        reason: String,
    },
}

/// Löst Hostnamen zu Adressen auf, nach der Entscheidung.
#[async_trait]
pub trait Resolver: Send + Sync {
    /// Alle Adressen für `host`, in der Reihenfolge des Namensdienstes.
    ///
    /// `host` ist ein normalisierter DNS-Name (A-Label). Die Auswahl einer
    /// Adresse (IPv4/IPv6-Präferenz) trifft [`pick`], nicht der Adapter.
    ///
    /// # Errors
    ///
    /// [`ResolveError`], wenn kein Eintrag existiert oder der Dienst scheitert.
    async fn resolve(&self, host: &str) -> Result<Vec<IpAddr>, ResolveError>;
}

/// Wählt aus den aufgelösten Adressen die eine, die angeheftet wird.
///
/// Bevorzugt die Familie aus `prefer`; ist keine davon dabei, gewinnt die
/// erste Adresse. `None`, wenn die Liste leer ist.
#[must_use]
pub fn pick(addrs: &[IpAddr], prefer: IpPreference) -> Option<IpAddr> {
    let wanted_v6 = matches!(prefer, IpPreference::Ipv6);
    addrs
        .iter()
        .find(|ip| ip.is_ipv6() == wanted_v6)
        .or_else(|| addrs.first())
        .copied()
}

/// Der Namensdienst des Systems (getaddrinfo über `tokio::net::lookup_host`).
#[derive(Debug, Clone, Default)]
pub struct SystemResolver;

#[async_trait]
impl Resolver for SystemResolver {
    async fn resolve(&self, host: &str) -> Result<Vec<IpAddr>, ResolveError> {
        // Port 0: wir wollen nur die Adressen, den Port kennt der Handler.
        let addrs: Vec<IpAddr> = tokio::net::lookup_host((host, 0))
            .await
            .map_err(|err| ResolveError::Failed {
                host: host.to_owned(),
                reason: err.to_string(),
            })?
            .map(|sock| sock.ip())
            .collect();
        if addrs.is_empty() {
            return Err(ResolveError::NotFound {
                host: host.to_owned(),
            });
        }
        Ok(addrs)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    use humanitl_config::IpPreference;

    use super::pick;

    #[test]
    fn pick_honours_the_preference_then_falls_back() {
        let v4 = IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34));
        let v6 = IpAddr::V6(Ipv6Addr::LOCALHOST);

        assert_eq!(pick(&[v4, v6], IpPreference::Ipv4), Some(v4));
        assert_eq!(pick(&[v4, v6], IpPreference::Ipv6), Some(v6));
        // Nur eine Familie da: die Präferenz kann nicht erfüllt werden, die
        // erste Adresse gewinnt.
        assert_eq!(pick(&[v4], IpPreference::Ipv6), Some(v4));
        assert_eq!(pick(&[], IpPreference::Ipv4), None);
    }
}
