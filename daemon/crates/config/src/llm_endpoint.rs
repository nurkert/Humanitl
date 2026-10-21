//! Die eine Lesart von `llm.endpoint` (HUM-261).
//!
//! Der eingetragene Endpunkt ist die Basis-Adresse der OpenAI-kompatiblen
//! API, so wie `OpenCode` eine `baseURL` liest: Endet sein Pfad auf `/v1` (mit
//! oder ohne abschließenden `/`), ist er die Basis selbst; sonst hängt ein
//! `/v1` daran. `http://host:11434`, `http://host:11434/v1` und
//! `http://host:11434/v1/` meinen also dieselbe Basis `/v1`, und
//! `https://llm.example/api/v1/` meint die Basis `/api/v1`.
//!
//! Was davor steht, ist das **Pfad-Präfix** des Servers, bei
//! `https://llm.example/api/v1/` also `/api`. Ein Server hinter einem solchen
//! Präfix sitzt hinter einem Reverse-Proxy, der nach Pfaden verteilt. Die
//! Wurzel desselben Hosts gehört dann womöglich einem ganz anderen Dienst, und
//! die native Ollama-API (`/api/tags`, `/api/chat`) gibt es nur an der Wurzel
//! eines Ollama-Servers, nie unter einem fremden Präfix. Deshalb gilt sie nur
//! für einen Endpunkt ohne Präfix ([`serves_native_ollama`]).
//!
//! Probe, Agent-Konfiguration, Durchreichregel und jeder Vorschlag, der eine
//! Adresse zum Abschreiben nennt, lesen den Endpunkt über diese Funktionen.
//! Zwei Lesarten liefen auseinander; genau so entstand der `curl` auf
//! `/api/api/tags`, den es nicht gibt.

use url::Url;

/// Das Stück Pfad, unter dem eine OpenAI-kompatible API ihre Oberfläche
/// anbietet.
pub const OPENAI_SURFACE: &str = "/v1";

/// Der Pfad der OpenAI-kompatiblen Basis, ohne abschließenden `/`.
///
/// Immer mit `/` am Anfang und immer auf [`OPENAI_SURFACE`] endend:
/// `http://h:1` ergibt `/v1`, `http://h:1/api/v1/` ergibt `/api/v1`,
/// `http://h:1/openai` ergibt `/openai/v1`.
#[must_use]
pub fn openai_base_path(endpoint: &Url) -> String {
    let path = endpoint.path().trim_end_matches('/');
    if path.ends_with(OPENAI_SURFACE) {
        path.to_owned()
    } else {
        format!("{path}{OPENAI_SURFACE}")
    }
}

/// Die Basis-Adresse der OpenAI-kompatiblen API als Text, ohne
/// abschließenden `/`, ohne Query und ohne Fragment.
///
/// Das ist der Wert, den `OpenCode` als `baseURL` bekommt; an ihn hängt der
/// Agent `/chat/completions` und `/models`.
#[must_use]
pub fn openai_base_url(endpoint: &Url) -> String {
    let mut base = endpoint.clone();
    base.set_query(None);
    base.set_fragment(None);
    base.set_path(&openai_base_path(endpoint));
    base.as_str().trim_end_matches('/').to_owned()
}

/// Der Pfad eines Endpunkts der OpenAI-kompatiblen API, etwa `/models` oder
/// `/chat/completions`, unter der Basis dieses Endpunkts.
///
/// `relative` beginnt mit `/`.
#[must_use]
pub fn openai_path(endpoint: &Url, relative: &str) -> String {
    format!("{}{relative}", openai_base_path(endpoint))
}

/// Das Pfad-Präfix vor der OpenAI-Oberfläche, ohne abschließenden `/`; leer,
/// wenn der Endpunkt an der Wurzel des Servers sitzt.
#[must_use]
pub fn path_prefix(endpoint: &Url) -> String {
    let base = openai_base_path(endpoint);
    base.strip_suffix(OPENAI_SURFACE)
        .unwrap_or(&base)
        .to_owned()
}

/// Wahr, wenn der Endpunkt an der Wurzel des Servers sitzt und die native
/// Ollama-API (`/api/…`) deshalb dort zu erwarten ist.
///
/// Ein Endpunkt mit Pfad-Präfix bekommt sie nie, weder mit dem Präfix davor
/// (das ergäbe Pfade wie `/api/api/tags`, die kein Server kennt) noch an der
/// Wurzel des Hosts (die gehört womöglich einem anderen Dienst, und der
/// Mensch hat sie nicht eingetragen).
#[must_use]
pub fn serves_native_ollama(endpoint: &Url) -> bool {
    path_prefix(endpoint).is_empty()
}

/// Der Endpunkt zum Anzeigen in einem Befund: Schema, Host, Port und Pfad,
/// **ohne** Benutzer, Passwort, Query und Fragment.
///
/// In jedem dieser drei Teile kann ein Geheimnis stehen
/// (`https://user:pw@host/v1?token=…`), und keiner davon geht mit einer
/// Anfrage der Probe hinaus. Ein Befund nennt deshalb nur, was die Probe
/// tatsächlich fragt (HUM-261).
#[must_use]
pub fn display(endpoint: &Url) -> String {
    without_secrets(endpoint).to_string()
}

/// Der Endpunkt ohne Benutzer, Passwort, Query und Fragment (siehe
/// [`display`]), als Adresse, an die sich weiter Pfade hängen lassen.
#[must_use]
pub fn without_secrets(endpoint: &Url) -> Url {
    let mut shown = endpoint.clone();
    shown.set_query(None);
    shown.set_fragment(None);
    // Beides scheitert nur bei einer Adresse ohne Host; dann gibt es auch
    // keinen Benutzer und kein Passwort, die stehen bleiben könnten.
    let _no_user = shown.set_username("");
    let _no_password = shown.set_password(None);
    shown
}

/// Der Pfad, unter dem Ollama seine Modelle auflistet, immer an der Wurzel
/// des Servers.
pub const OLLAMA_TAGS_PATH: &str = "/api/tags";

/// Der Endpunkt, unter dem eine OpenAI-kompatible API ihre Modelle auflistet,
/// relativ zu ihrer Basis.
pub const OPENAI_MODELS: &str = "/models";

/// Die Pfade, die die Probe des Endpunkts fragt, in ihrer Reihenfolge.
///
/// [`OLLAMA_TAGS_PATH`] nur an der Wurzel des Servers
/// ([`serves_native_ollama`]), danach immer die Modellliste unter der Basis.
#[must_use]
pub fn probe_paths(endpoint: &Url) -> Vec<String> {
    let mut paths = Vec::with_capacity(2);
    if serves_native_ollama(endpoint) {
        paths.push(OLLAMA_TAGS_PATH.to_owned());
    }
    paths.push(openai_path(endpoint, OPENAI_MODELS));
    paths
}

/// Was die Probe an diesem Endpunkt tun wird, als englischer Halbsatz für
/// die Ankündigung vor der ersten Verbindung (`humanitl llm test`,
/// `humanitl doctor --probe-llm`).
///
/// Aus denselben Pfaden wie die Probe ([`probe_paths`]), damit die
/// Ankündigung nie einen Pfad nennt, den die Probe nicht fragt. Lässt sich
/// der Text nicht als Adresse lesen, bleibt der Satz neutral; die Probe
/// selbst weist ihn dann ab.
#[must_use]
pub fn probe_plan(endpoint: &str) -> String {
    let Ok(url) = Url::parse(endpoint) else {
        return format!(
            "GET requests for the model list of the endpoint, and {OLLAMA_TAGS_PATH} on a \
             plain Ollama server"
        );
    };
    match probe_paths(&url).as_slice() {
        [only] => format!("one GET request, {only}"),
        [first, second] => format!("two GET requests, {first} then {second}"),
        paths => format!("GET requests, {}", paths.join(" then ")),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use url::Url;

    use super::{
        display, openai_base_path, openai_base_url, openai_path, path_prefix, probe_paths,
        probe_plan, serves_native_ollama,
    };

    /// Benutzer, Passwort, Query und Fragment erscheinen nie in der Anzeige.
    #[test]
    fn the_display_drops_every_part_that_can_carry_a_secret() {
        let shown = display(&url(
            "https://user:pw@llm.example.net/api/v1/?token=geheim#frag",
        ));
        assert_eq!(shown, "https://llm.example.net/api/v1/");
        assert_eq!(display(&url("http://h:1")), "http://h:1/");
    }

    /// Die Ankündigung nennt genau die Pfade der Probe: zwei an der Wurzel,
    /// einer hinter einem Präfix, nie `/api/api/` (HUM-261).
    #[test]
    fn the_announcement_names_exactly_the_paths_of_the_probe() {
        assert_eq!(
            probe_plan("http://192.168.1.50:11434"),
            "two GET requests, /api/tags then /v1/models"
        );
        assert_eq!(
            probe_plan("http://h:1/v1"),
            "two GET requests, /api/tags then /v1/models"
        );
        assert_eq!(
            probe_plan("https://llm.example.net/api/v1/"),
            "one GET request, /api/v1/models"
        );
        assert_eq!(
            probe_paths(&url("https://llm.example.net/api/v1/")),
            ["/api/v1/models"]
        );
        assert!(probe_plan("not a url").contains("model list"));
    }

    fn url(text: &str) -> Url {
        Url::parse(text).unwrap()
    }

    #[test]
    fn v1_is_the_base_and_is_appended_at_most_once() {
        for (endpoint, base_path, base_url) in [
            (
                "http://192.168.1.50:11434",
                "/v1",
                "http://192.168.1.50:11434/v1",
            ),
            (
                "http://192.168.1.50:11434/",
                "/v1",
                "http://192.168.1.50:11434/v1",
            ),
            ("http://x:1/v1", "/v1", "http://x:1/v1"),
            ("http://x:1/v1/", "/v1", "http://x:1/v1"),
            ("http://x:1/openai", "/openai/v1", "http://x:1/openai/v1"),
            (
                "https://llm.example.net/api/v1/",
                "/api/v1",
                "https://llm.example.net/api/v1",
            ),
            (
                "https://llm.example.net/api/v1",
                "/api/v1",
                "https://llm.example.net/api/v1",
            ),
        ] {
            let endpoint = url(endpoint);
            assert_eq!(openai_base_path(&endpoint), base_path, "for {endpoint}");
            assert_eq!(openai_base_url(&endpoint), base_url, "for {endpoint}");
        }
    }

    #[test]
    fn the_models_path_lies_under_the_base() {
        assert_eq!(
            openai_path(&url("https://llm.example.net/api/v1/"), "/models"),
            "/api/v1/models"
        );
        assert_eq!(openai_path(&url("http://h:1"), "/models"), "/v1/models");
        assert_eq!(openai_path(&url("http://h:1/v1"), "/models"), "/v1/models");
    }

    #[test]
    fn query_and_fragment_are_not_part_of_the_base() {
        assert_eq!(
            openai_base_url(&url("http://h:1/api/v1/?x=1#y")),
            "http://h:1/api/v1"
        );
    }

    #[test]
    fn only_an_endpoint_at_the_server_root_serves_native_ollama() {
        for (endpoint, prefix) in [
            ("http://h:11434", ""),
            ("http://h:11434/", ""),
            ("http://h:11434/v1", ""),
            ("http://h:11434/v1/", ""),
            ("https://llm.example.net/api/v1/", "/api"),
            ("http://h:1/openai", "/openai"),
            ("http://h:1/openai/v1", "/openai"),
        ] {
            let endpoint = url(endpoint);
            assert_eq!(path_prefix(&endpoint), prefix, "for {endpoint}");
            assert_eq!(
                serves_native_ollama(&endpoint),
                prefix.is_empty(),
                "for {endpoint}"
            );
        }
    }
}
