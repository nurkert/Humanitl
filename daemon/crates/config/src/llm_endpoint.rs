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

use humanitl_core::diagnostics::codes::CONFIG_003;
use humanitl_core::{Diagnostic, FixAction, Severity};
use url::Url;

use crate::pasted::strip_paste_debris;

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
    // Ein Text, den [`check_text`] ablehnt, wird nie gefragt; die
    // Ankündigung nennt dann auch keinen Pfad wie `/api/v1/%22/v1/models`
    // (HUM-283).
    let Some(url) = check_text(endpoint)
        .ok()
        .and_then(|()| Url::parse(endpoint).ok())
    else {
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

/// Der Schlüssel der Einstellung, um die es hier geht.
pub const SETTING: &str = "llm.endpoint";

/// Ob ein Zeichen in einer eingetragenen Adresse nichts verloren hat.
///
/// Anführungszeichen, spitze Klammern, ein Backslash, Leerraum und
/// Steuerzeichen. Der Parser der Adresse nähme sie hin und machte daraus
/// etwas anderes: `"` und `<` würden zu `%22` und `%3C` im Pfad, ein
/// Backslash zu `/`, Leerraum am Rand fiele still weg. Die Probe fragte dann
/// `/api/v1/%22/v1/models` (HUM-283), und der Agent bekäme dieselbe falsche
/// Basis.
#[must_use]
pub fn is_refused_char(c: char) -> bool {
    matches!(c, '"' | '\'' | '<' | '>' | '\\') || c.is_whitespace() || c.is_control()
}

/// Prüft den Text einer Adresse, bevor ein Parser ihn sieht (HUM-283).
///
/// Dieselbe Prüfung gilt für `config.toml`, `humanitl config set`, `SetConfig`
/// und die Probe (`ProbeLlm`), damit keiner dieser Wege eine Adresse annimmt,
/// die ein anderer ablehnt.
///
/// # Errors
///
/// `CONFIG_003`, wenn der Text ein Zeichen aus [`is_refused_char`] trägt.
/// Das `why` nennt das erste solche Zeichen und seine Stelle, nicht die ganze
/// Adresse: In Benutzer, Passwort oder Query kann ein Geheimnis stehen. Der
/// Vorschlag ist die bereinigte Adresse ([`cleaned`]).
pub fn check_text(text: &str) -> Result<(), Diagnostic> {
    let Some((index, refused)) = text.chars().enumerate().find(|(_, c)| is_refused_char(*c)) else {
        return Ok(());
    };
    let position = index + 1;
    let length = text.chars().count();
    // Der Vorschlag trägt kein Geheimnis (Review zu HUM-283): Benutzer,
    // Passwort, Query und Fragment fallen weg ([`without_secrets`]). Lässt
    // sich die bereinigte Adresse nicht lesen, bleibt nur der Befehl ohne
    // Wert; ein Text, den kein Parser zerlegt hat, könnte noch eins tragen.
    let clean = cleaned(text);
    let fix = match Url::parse(&clean) {
        Ok(url) => FixAction::ChangeSetting {
            key: SETTING.to_owned(),
            value: if carries_secrets(&url) {
                without_secrets(&url).to_string()
            } else {
                clean
            },
        },
        Err(_unreadable) => FixAction::CopyCommand(format!("humanitl config set {SETTING} URL")),
    };
    Err(Diagnostic::builder(CONFIG_003, Severity::Error)
        .why(format!(
            "{REFUSAL_MARK}{} at position {position} of {length}; a URL carries no quotes, \
             angle brackets, backslashes or white space. Such a character is usually left over \
             from copying the address out of a configuration file",
            char_name(refused)
        ))
        .fix(fix)
        .build())
}

/// Ob die Adresse einen Teil trägt, in dem ein Geheimnis stehen kann.
fn carries_secrets(url: &Url) -> bool {
    !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
}

/// Der Satzanfang von [`check_text`], an dem [`set_aside`] den Befund erkennt.
const REFUSAL_MARK: &str = "llm.endpoint contains ";

/// Der Befund einer Ladung, die einen eingetragenen `llm.endpoint`
/// beiseitegelegt hat (HUM-283, Review).
///
/// Eine Adresse, die eine ältere Fassung angenommen hat, bricht den Start
/// nicht ab: Sie gilt als nicht gesetzt, und dieser Befund (`CONFIG_003`,
/// Warnung, mit der sauberen Adresse als Vorschlag) sagt warum. Doctor und
/// Setup zeigen ihn an der Zeile `llm`.
#[must_use]
pub fn set_aside(diagnostics: &[Diagnostic]) -> Option<&Diagnostic> {
    diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == CONFIG_003 && diagnostic.why.contains(REFUSAL_MARK))
}

/// Die Adresse ohne die Reste einer Zwischenablage und ohne jedes Zeichen aus
/// [`is_refused_char`].
///
/// Erst fällt weg, was außen steht ([`strip_paste_debris`]), dann jedes
/// verbliebene abgelehnte Zeichen. Das Ergebnis ist ein Vorschlag, den der
/// Mensch sieht, bevor er ihn übernimmt.
#[must_use]
pub fn cleaned(text: &str) -> String {
    strip_paste_debris(text)
        .chars()
        .filter(|c| !is_refused_char(*c))
        .collect()
}

/// Ein Zeichen so benannt, dass man es in einer Zeile wiedererkennt.
fn char_name(c: char) -> String {
    match c {
        '"' => "a double quote (\")".to_owned(),
        '\'' => "a single quote (')".to_owned(),
        '<' | '>' => format!("an angle bracket ({c})"),
        '\\' => "a backslash (\\)".to_owned(),
        ' ' => "a space".to_owned(),
        '\t' => "a tab".to_owned(),
        '\n' | '\r' => "a line break".to_owned(),
        other => format!("the character U+{:04X}", u32::from(other)),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use humanitl_core::FixAction;
    use humanitl_core::diagnostics::codes::CONFIG_003;
    use url::Url;

    use super::{
        check_text, cleaned, display, openai_base_path, openai_base_url, openai_path, path_prefix,
        probe_paths, probe_plan, serves_native_ollama,
    };

    /// Die Adresse des Besitzers vom 2026-10-23 mit dem Anführungszeichen am
    /// Ende: abgelehnt mit Zeichen und Stelle, der Vorschlag ist die saubere
    /// Adresse, und keine Probe fragt je `%22` (HUM-283).
    #[test]
    fn a_stray_quote_is_refused_with_its_position_and_the_clean_url() {
        let pasted = "https://llm.hzo.example.net/api/v1/\"";
        let refused = check_text(pasted).unwrap_err();
        assert_eq!(refused.code, CONFIG_003);
        assert!(
            refused.why.contains("a double quote (\")"),
            "{}",
            refused.why
        );
        assert!(refused.why.contains("position 36 of 36"), "{}", refused.why);
        assert_eq!(
            refused.fix,
            Some(FixAction::ChangeSetting {
                key: "llm.endpoint".to_owned(),
                value: "https://llm.hzo.example.net/api/v1/".to_owned(),
            })
        );
        assert!(check_text(&cleaned(pasted)).is_ok());
    }

    /// Jedes der Zeichen wird abgelehnt, auch im Inneren, und eine saubere
    /// Adresse geht durch.
    #[test]
    fn quotes_brackets_backslashes_and_white_space_are_refused_anywhere() {
        for (text, named) in [
            ("'http://h:1/v1'", "a single quote"),
            ("http://h:1/<v1>", "an angle bracket (<)"),
            ("http://h:1\\v1", "a backslash"),
            (" http://h:1/v1", "a space at position 1"),
            ("http://h:1/v1\n", "a line break"),
            ("http://h:1/a b", "a space at position 13"),
            ("http://h:1/\tv1", "a tab"),
        ] {
            let refused = check_text(text).unwrap_err();
            assert_eq!(refused.code, CONFIG_003, "{text:?}");
            assert!(refused.why.contains(named), "{text:?}: {}", refused.why);
        }
        for clean in [
            "http://192.168.1.50:11434",
            "https://llm.example.net/api/v1/",
            "https://user:pw@llm.example.net/v1?x=1#f",
        ] {
            assert!(check_text(clean).is_ok(), "{clean}");
        }
    }

    /// Das `why` wiederholt die Adresse nicht: In Benutzer, Passwort oder
    /// Query kann ein Geheimnis stehen.
    #[test]
    fn the_refusal_does_not_repeat_the_address() {
        let refused = check_text("https://user:geheim@h/v1?token=abc\"").unwrap_err();
        assert!(!refused.why.contains("geheim"), "{}", refused.why);
        assert!(!refused.why.contains("token=abc"), "{}", refused.why);
    }

    /// Auch der Vorschlag trägt kein Geheimnis: Benutzer, Passwort, Query und
    /// Fragment fallen weg, und was kein Parser liest, bekommt nur den Befehl
    /// ohne Wert (HUM-283, Review).
    #[test]
    fn the_fix_carries_no_secret() {
        let refused = check_text("https://user:geheim@h/v1?token=x\"").unwrap_err();
        let fix = format!("{:?}", refused.fix);
        for secret in ["geheim", "token=x"] {
            assert!(!refused.why.contains(secret), "{}", refused.why);
            assert!(!fix.contains(secret), "{fix}");
        }
        assert_eq!(
            refused.fix,
            Some(FixAction::ChangeSetting {
                key: "llm.endpoint".to_owned(),
                value: "https://h/v1".to_owned(),
            })
        );
        assert_eq!(
            check_text("\"not a url\"").unwrap_err().fix,
            Some(FixAction::CopyCommand(
                "humanitl config set llm.endpoint URL".to_owned()
            ))
        );
    }

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
        // HUM-283: eine Adresse, die abgelehnt wird, kündigt keinen Pfad an.
        let refused = probe_plan("https://llm.example.net/api/v1/\"");
        assert!(!refused.contains("%22"), "{refused}");
        assert!(refused.contains("model list"), "{refused}");
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
