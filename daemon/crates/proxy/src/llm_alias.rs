//! Der reservierte Name des Sprachmodells, `llm.humanitl.internal` (HUM-387).
//!
//! `OpenCode` 2.2.5 schickt eine Anfrage an eine Loopback-`baseURL`
//! (`http://127.0.0.1:8080/v1`) unmittelbar, an `HTTP_PROXY` vorbei, auch mit
//! leerem `NO_PROXY`. Unter Seatbelt verweigert das Profil diese Verbindung,
//! unter Linux steht auf `127.0.0.1` der Sandbox nichts. Ein Name dagegen geht
//! durch den Proxy. Der Adapter schreibt deshalb bei einem Endpunkt auf dem
//! Loopback [`LLM_ALIAS_HOST`] in die `baseURL`, mit demselben Schema, Port und
//! Pfad, und der Proxy setzt den Namen hier auf das eingestellte Ziel um.
//!
//! **Was der Name erreicht.** Genau das Ziel aus `llm.endpoint`, und nur, wenn
//! dessen Host Loopback ist ([`HostName::is_loopback`]); dazu muss die Anfrage
//! dasselbe Schema und denselben Port tragen. Jede andere Anfrage an den Namen
//! wird abgelehnt, bevor eine Regel sie sieht: ohne Halten, ohne Frage, ohne
//! Auflösung. Ein Mensch kann den Namen also nicht für ein anderes Ziel
//! freigeben, und ein Agent erreicht über ihn keinen zweiten Dienst des Hosts.
//!
//! **Unter welchem Ziel der Fluss steht.** Unter dem echten Ziel. Die
//! Umsetzung geschieht vor den Regeln, so dass die Durchreiche des Modells
//! (`passthrough_llm`, `allow_private` nur für dieses Ziel) ohne zweite Regel
//! gilt, der Zugangsschlüssel an seinem eigenen Ziel kein Fund ist und eine
//! Sperre des Nutzers für das Ziel wirkt wie unter der echten Adresse;
//! für die Endpunkte der Durchleitung geht die Durchleitung weiter vor. Die
//! Aufzeichnung nennt damit, wohin die Bytes gingen; welchen Namen der Agent
//! benutzt hat, steht weiter in seinem `Host`-Kopf, der unverändert
//! aufgezeichnet wird. Nach oben geht der `Host` des echten Ziels.
//!
//! Eine abgelehnte Anfrage an den Namen ging nirgendwohin; ihr Fluss steht
//! unter dem Namen selbst, mit `no_route` und dem Befund [`PROXY_003`].

use humanitl_config::LlmConfig;
use humanitl_core::diagnostics::codes::PROXY_003;
use humanitl_core::{Authority, Diagnostic, HostName, LLM_ALIAS_HOST, Scheme, Severity};

/// Das Ziel, für das der reservierte Name steht: Schema und Authority des
/// eingestellten Endpunkts auf dem Loopback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmAliasTarget {
    /// Das Schema des Endpunkts; eine Anfrage an den Namen muss es tragen.
    pub scheme: Scheme,
    /// Host und Port des Endpunkts, normalisiert wie jede Anfrage
    /// ([`LlmConfig::endpoint_authority`]).
    pub authority: Authority,
}

impl LlmAliasTarget {
    /// Das Ziel des Namens für diese Konfiguration.
    ///
    /// `None` ohne `llm.endpoint`, bei einem Schema, das der Proxy nicht
    /// kennt, und bei jedem Host, der nicht Loopback ist: Ein Endpunkt im
    /// eigenen Netz oder unter einem Namen erreicht der Agent ohnehin über den
    /// Proxy, und der Name bekommt dann kein Ziel.
    #[must_use]
    pub fn for_llm(llm: &LlmConfig) -> Option<Self> {
        let endpoint = llm.endpoint.as_ref()?;
        let scheme = Scheme::parse(endpoint.scheme())?;
        let authority = llm.endpoint_authority()?;
        authority
            .host
            .is_loopback()
            .then_some(Self { scheme, authority })
    }
}

/// Wahr, wenn eine Anfrage an den reservierten Namen geht.
#[must_use]
pub fn is_llm_alias(host: &HostName) -> bool {
    host.is_llm_alias()
}

/// Setzt eine Anfrage an [`LLM_ALIAS_HOST`] auf das eingestellte Ziel um.
///
/// `authority` muss an den Namen gehen ([`is_llm_alias`]). Das Ergebnis ist
/// die Authority des Ziels, unter der die Anfrage durch Regeln, Halten und
/// Weiterleitung läuft.
///
/// # Errors
///
/// [`PROXY_003`], wenn es kein Ziel gibt (kein Endpunkt auf dem Loopback) oder
/// Schema oder Port der Anfrage nicht die des Ziels sind. Der Name wird dann
/// nie aufgelöst und nirgendwohin verbunden.
pub fn map_llm_alias(
    target: Option<&LlmAliasTarget>,
    scheme: Scheme,
    authority: &Authority,
) -> Result<Authority, Diagnostic> {
    let Some(target) = target else {
        return Err(Diagnostic::builder(PROXY_003, Severity::Error)
            .why(format!(
                "the request goes to the reserved name {LLM_ALIAS_HOST}, which stands only for a \
                 language model on this machine's loopback, and llm.endpoint names none for this \
                 session; the name is never resolved, so the request was refused. Requests to a \
                 model in the local network or under its own name go to that address directly."
            ))
            .build());
    };
    if authority.port != target.authority.port || scheme != target.scheme {
        return Err(Diagnostic::builder(PROXY_003, Severity::Error)
            .why(format!(
                "the request goes to {}://{authority}, and the reserved name {LLM_ALIAS_HOST} \
                 stands only for the configured model endpoint {}://{LLM_ALIAS_HOST}:{}; any \
                 other port or scheme under the name reaches nothing on this machine, so the \
                 request was refused without resolving the name.",
                scheme.as_str(),
                target.scheme.as_str(),
                target.authority.port,
            ))
            .build());
    }
    Ok(target.authority.clone())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_config::LlmConfig;
    use humanitl_core::diagnostics::codes::PROXY_003;
    use humanitl_core::{Authority, HostName, LLM_ALIAS_HOST, Scheme};
    use url::Url;

    use super::{LlmAliasTarget, is_llm_alias, map_llm_alias};

    fn llm(endpoint: &str) -> LlmConfig {
        LlmConfig {
            endpoint: Some(Url::parse(endpoint).unwrap()),
            ..LlmConfig::default()
        }
    }

    fn alias(port: u16) -> Authority {
        Authority::new(HostName::parse(LLM_ALIAS_HOST).unwrap(), port)
    }

    #[test]
    fn only_a_loopback_endpoint_gives_the_name_a_target() {
        for (endpoint, host, port) in [
            ("http://127.0.0.1:8080/v1", "127.0.0.1", 8080),
            ("http://[::1]:11434", "::1", 11434),
            ("http://localhost:8080", "localhost", 8080),
            ("https://localhost/v1", "localhost", 443),
        ] {
            let target = LlmAliasTarget::for_llm(&llm(endpoint)).unwrap();
            assert_eq!(
                target.authority,
                Authority::new(HostName::parse(host).unwrap(), port),
                "for {endpoint}"
            );
        }
        for endpoint in [
            "http://192.168.1.50:11434",
            "http://model.lan:8080/v1",
            "https://api.example.net/v1",
        ] {
            assert_eq!(LlmAliasTarget::for_llm(&llm(endpoint)), None, "{endpoint}");
        }
        assert_eq!(LlmAliasTarget::for_llm(&LlmConfig::default()), None);
    }

    #[test]
    fn the_name_maps_only_to_the_configured_target() {
        let target = LlmAliasTarget::for_llm(&llm("http://127.0.0.1:8080/v1")).unwrap();
        assert!(is_llm_alias(&alias(8080).host));
        assert_eq!(
            map_llm_alias(Some(&target), Scheme::Http, &alias(8080)).unwrap(),
            Authority::new(HostName::parse("127.0.0.1").unwrap(), 8080)
        );
    }

    #[test]
    fn another_port_or_scheme_is_refused() {
        let target = LlmAliasTarget::for_llm(&llm("http://127.0.0.1:8080/v1")).unwrap();
        for (scheme, port) in [
            (Scheme::Http, 8081),
            (Scheme::Http, 22),
            (Scheme::Https, 8080),
            (Scheme::Http, 80),
        ] {
            let refused = map_llm_alias(Some(&target), scheme, &alias(port)).unwrap_err();
            assert_eq!(refused.code, PROXY_003);
            assert!(refused.why.contains(LLM_ALIAS_HOST), "{}", refused.why);
            assert!(
                refused.why.contains(":8080"),
                "names the one port: {}",
                refused.why
            );
        }
    }

    #[test]
    fn without_a_loopback_endpoint_the_name_is_refused() {
        let refused = map_llm_alias(None, Scheme::Http, &alias(8080)).unwrap_err();
        assert_eq!(refused.code, PROXY_003);
        assert!(refused.why.contains("llm.endpoint"), "{}", refused.why);
        let lan = LlmAliasTarget::for_llm(&llm("http://192.168.1.50:8080"));
        assert!(
            map_llm_alias(lan.as_ref(), Scheme::Http, &alias(8080)).is_err(),
            "a LAN endpoint gives the name no target"
        );
    }
}
