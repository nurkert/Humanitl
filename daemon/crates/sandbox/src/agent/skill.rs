//! Der Skill `humanitl`, den beide Agenten in der Sandbox vorfinden (HUM-364).
//!
//! Das Briefing ([`crate::agent::briefing`]) steht in jedem Kontextfenster und
//! bleibt deshalb kurz. Was ein Agent über Humanitl braucht, wenn eine Anfrage
//! lange wartet, abgelehnt wird oder er Arbeit im Netz vor sich hat, ist mehr:
//! wie er wartet, wie er eine Antwort des Proxys liest, wie er den Menschen
//! fragt und wie er eine Ablehnung meldet. Das steht im Skill. Beide Agenten
//! zeigen dem Modell von einem Skill nur Name und Beschreibung und laden den
//! Text erst, wenn die Beschreibung zur Aufgabe passt.
//!
//! **Das kehrt die Haltung aus HUM-309 für genau diesen einen Skill um.**
//! Dort hieß es: keine Skills von Humanitl. Der Grund des Owners vom
//! 2026-10-31: Agenten brechen ab, wiederholen in Schleifen, wechseln das
//! Werkzeug oder schließen auf ein totes Netz, wenn eine Anfrage gehalten
//! wird. Weitere Skills, Erweiterungen oder MCP-Konfiguration liefert Humanitl
//! weiter nicht.
//!
//! # Wo die Agenten Skills suchen
//!
//! Abgelesen an den installierten Fassungen, nicht an der Dokumentation:
//!
//! | Agent | Ort | Beleg |
//! |---|---|---|
//! | `OpenCode` 1.18.31 | `<config>/opencode/skills/humanitl/SKILL.md` | Zeichenketten des Binärprogramms: `Skill.discovery` durchsucht jedes Verzeichnis aus `Config.directories` mit `{skill,skills}/**/SKILL.md`, und `ConfigPaths.directories` beginnt mit `Global.Path.config`, also `$XDG_CONFIG_HOME/opencode`; dasselbe Verzeichnis, aus dem `OpenCode` die globale `AGENTS.md` liest. Ein Skill ohne `description` erscheint nicht in der Liste für das Modell. |
//! | pi 1.4.0 | `<agent-dir>/skills/humanitl/SKILL.md` | `docs/skills.md` des Pakets („user skills directory") und `dist/bundle`: `userDirs.skills = join(agentDir, "skills")`, rekursiv nach `SKILL.md` durchsucht; Name und Beschreibung stehen im Systemprompt, sobald ein Lesewerkzeug (`read` oder `bash`) aktiv ist. Skills im Agentenverzeichnis brauchen keine Vertrauensfrage, nur die des Projekts. |
//!
//! Beide lesen den Kopf im Format von agentskills.io: `name` (Kleinbuchstaben,
//! Ziffern, Bindestriche, höchstens 64 Zeichen) und `description` (höchstens
//! 1024 Zeichen). pi lädt einen Skill ohne Beschreibung nicht. Der Name des
//! Verzeichnisses ist der des Skills, wie es die Spezifikation empfiehlt.
//!
//! # Was der Text sagt und woran es geprüft ist
//!
//! | Aussage | Beleg im Code |
//! |---|---|
//! | Jede HTTP(S)-Anfrage geht über den Proxy der Umgebung, sonst nichts | wie im Briefing: `--unshare-all` in `crate::bwrap_args`, `humanitl_proxy::ca::ENV_KIT` |
//! | Erlaubt: sofort hinaus; nicht erlaubt: sofort `403`; weder noch: der nächste Abschnitt | `humanitl_proxy::pipeline::RulesPipeline`: `Matched { Allow }` und `Matched { Block }` entscheiden ohne Hold, `Ask` und `Default` gehen in den Hold |
//! | Modell-Aufrufe erlaubt eine mitgelieferte Regel auf ihren eingestellten Pfaden, sie warten hier nie | `crate::agent::opencode::passthrough_rule` mit `passthrough_llm(true)` und den Pfaden aus `llm.passthrough_paths`, im Proxy `DecisionSource::Passthrough` ohne Hold; nennt die Konfiguration einen Pfad, der kein Modell-Aufruf ist (etwa `/api/pull`), deckt die Regel auch ihn, darum steht „sonst nichts auf diesem Host“ nicht im Text |
//! | `Blocked by Humanitl.` mit den Zeilen `reason:`, `flow:`, `host:`, `note:` | `humanitl_core::block::block_response` und `failed_response` |
//! | `403`: hier nicht erlaubt, ein Geheimnis oder eine private Adresse | `BlockReason::http_status`: `User`, `Rule`, `AuthorityMismatch`, `PrivateAddress`, `Secret` |
//! | `504` mit `reason: timeout` | `BlockReason::Timeout` aus `Decision::TimedOut` |
//! | `502` mit `upstream_dns`, `upstream_connect`, `upstream_tls`, `upstream_timeout`: durchgelassen, das Ziel scheiterte | `UpstreamError::reason` und `UpstreamError::http_status` |
//! | `502` mit `upstream_private_address:<Adresse>`: der Name löst auf eine private Adresse auf | `UpstreamError::PrivateAddress`, `Display` hängt die Adresse an (ADR-006) |
//! | `503` mit `hold_max_flows` oder `hold_memory`: zu viele Anfragen oder zu viele Daten warten gleichzeitig | `BlockReason::HoldMaxFlows` (`limits.hold_max_flows`), `BlockReason::HoldMemory` (`limits.hold_max_bytes`), `humanitl_proxy::hold` |
//! | Jeder andere Status oder Grund (etwa `408`, `413`, `502 no_route`): nicht unverändert wiederholen | `BlockReason::http_status`: `ClientTimeout`/`ClientAborted` `408`, `BodyCap` `413`, `NoRoute` `502` |
//! | `GET /` zeigt `timeout=`, `llm=` und, was erlaubt ist, als Regeln, erster Treffer gewinnt | `humanitl_proxy::meta::MetaEndpoint::respond`, Kopfzeile `humanitl session=… ask=… timeout=… llm=…` und `rules (first match wins):` |
//! | `GET /why/<flow>` zeigt `decision=`, `reason=`, `note=` | `humanitl_proxy::meta`, `why_reply` |
//! | `POST /ask` antwortet sofort `202`, legt keine Regel an, höchstens zehn je Minute, darüber `429` mit `retry-after` | `MetaEndpoint::ask`: `202 queued`, `ASK_PER_WINDOW = 10` in `ASK_WINDOW = 60 s`, nur ein `FlowEvent::AgentAsk` |
//! | Mit Frage: was noch keine Regel erlaubt oder ablehnt, wartet bis `{timeout}`s, dann `504`, weil keine Erlaubnis rechtzeitig kam; solange keine Antwort; erlaubt dauert so lange wie der Server, abgelehnt endet sofort mit `403` | `hold.timeout_secs`, `HoldQueue::hold`; `Matched { Allow }` und `Matched { Block }` gehen nicht in den Hold |
//! | Ein Werkzeug, das früher aufgibt, beendet die Anfrage, eine neue wartet von vorn | `ClientTimeout` in `HoldQueue::hold`, die Karte verlässt die Warteschlange (HUM-308) |
//! | `curl` ohne `--max-time` wartet von selbst | `curl` hat keine eigene Gesamtfrist (HUM-308) |
//! | Ohne Frage: nichts wartet, abgelehnt `403`, alles andere sofort `504` | `ask_mode = none` setzt die Frist auf null; `Matched { Block }` bleibt `403` |
//! | Nach einem `504` einmal Zugang mit `POST /ask` beantragen und später einmal wiederholen, in beiden Ask-Modi gleich | Ein neuer Flow wird nach den dann geltenden Regeln entschieden |
//!
//! Die Frist, die der Text für `curl --max-time` vorschlägt, ist
//! `hold.timeout_secs` plus [`BASH_TIMEOUT_MARGIN_SECS`], derselbe Aufschlag,
//! mit dem [`crate::agent::opencode::bash_default_timeout_ms`] die Frist von
//! `bash` in `OpenCode` hebt: Verbindungsaufbau vor dem Halten und Antwort
//! danach.
//!
//! **Kein Weg vorbei am Proxy wird genannt.** Der Fallstrick aus HUM-071 gilt
//! auch hier: Der Text sagt, dass es keinen anderen Weg hinaus gibt und dass
//! ein anderes Werkzeug, ein anderer Host oder ein Spiegel eine neue Anfrage
//! ist, für die dieselben Regeln gelten. Er stellt keinen Weg vorbei am Proxy
//! als Möglichkeit in den Raum, auch nicht verneint.
//!
//! **Kein Mensch im Text.** Entscheidung des Owners vom 2026-10-31, wie im
//! Briefing: Der Agent erfährt nicht, dass ein Mensch über seine Anfragen
//! entscheidet. Die Latenz ist eine Eigenschaft der Umgebung, `403` heißt
//! „hier nicht erlaubt", `504` nach dem Warten „keine Erlaubnis rechtzeitig",
//! `/ask` beantragt Zugang, und Zugang kann später folgen. Das ist wahr und
//! lässt nur weg, wer entscheidet. Die Kopfzeile von `GET /` trägt weiter
//! `ask=`; der Text nennt das Feld nicht. Ein Test hält fest, dass weder
//! Briefing noch Skill von einer Person, einer Freigabe oder einer Prüfung
//! sprechen.
//!
//! # Das Format der Vorlagen
//!
//! `agents/skill/SKILL.{en,de}.md` haben dieselbe Form wie die Vorlagen des
//! Briefings: HTML-Kommentare fallen weg, und ab der ersten Zeile, die mit
//! `<!-- ask_mode:` beginnt, stehen die Varianten. Anders als im Briefing
//! bleiben die Zeilen einer Variante Zeilen, denn jede trägt eine Überschrift
//! und eine Liste. Der Kopf zwischen den beiden `---` muss nach dem Entfernen
//! der Kommentare am Anfang stehen; [`render`] prüft ihn mit
//! [`front_matter`].

use humanitl_config::{AskMode, Language};
use humanitl_core::Diagnostic;

use crate::agent::briefing::{
    PLACEHOLDER_ASK_MODE, PLACEHOLDER_LLM_HOST, PLACEHOLDER_TIMEOUT, split_variants_with,
    strip_comments, tidy, variant_for,
};
use crate::agent::opencode::BASH_TIMEOUT_MARGIN_SECS;
use crate::agent::opencode_models::broken;

/// Der Name des Skills, im Kopf und als Name seines Verzeichnisses.
pub const SKILL_NAME: &str = "humanitl";

/// Der Name der Datei, die beide Agenten in einem Skill-Verzeichnis suchen.
pub const SKILL_FILE_NAME: &str = "SKILL.md";

/// Das Verzeichnis unter dem Konfigurations- beziehungsweise
/// Agentenverzeichnis, in dem beide Agenten Skills suchen.
pub const SKILLS_DIR_NAME: &str = "skills";

/// Die englische Vorlage.
pub const TEMPLATE_EN: &str = include_str!("../../../../../agents/skill/SKILL.en.md");

/// Die deutsche Vorlage.
pub const TEMPLATE_DE: &str = include_str!("../../../../../agents/skill/SKILL.de.md");

/// Der Pfad der englischen Vorlage im Repository; steht in Fehlermeldungen.
pub const TEMPLATE_EN_FILE: &str = "agents/skill/SKILL.en.md";

/// Der Pfad der deutschen Vorlage im Repository; steht in Fehlermeldungen.
pub const TEMPLATE_DE_FILE: &str = "agents/skill/SKILL.de.md";

/// Der Platzhalter für die Frist, die der Text `curl --max-time` vorschlägt.
pub const PLACEHOLDER_MAX_TIME: &str = "{max_time}";

/// So lang darf die Beschreibung höchstens sein (agentskills.io, pi 1.4.0).
pub const DESCRIPTION_MAX_CHARS: usize = 1024;

/// So lang darf der Name höchstens sein (agentskills.io, pi 1.4.0).
pub const NAME_MAX_CHARS: usize = 64;

/// Der Trenner, der den Kopf umschließt.
const FRONT_MATTER_FENCE: &str = "---";

/// Name und Beschreibung aus dem Kopf eines Skills.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontMatter {
    /// `name:`.
    pub name: String,
    /// `description:`, ohne die Anführungszeichen.
    pub description: String,
}

/// Rendert den Skill für eine Sitzung.
///
/// Dieselben Eingaben wie [`crate::agent::briefing::render`], aus denselben
/// Quellen; `llm_host` ist `None`, wenn es keine Durchreiche gibt, und dann
/// fällt die Zeile über das Modell weg.
///
/// # Errors
///
/// Ein [`Diagnostic`] mit `AGENT_003`, wenn die einkompilierte Vorlage nicht
/// die erwartete Form hat: keine Variante zum Ask-Modus, ein Platzhalter, den
/// niemand ersetzt hat, oder ein Kopf, den die Agenten nicht lesen könnten.
pub fn render(
    language: Language,
    ask_mode: AskMode,
    timeout_secs: u64,
    llm_host: Option<&str>,
) -> Result<String, Diagnostic> {
    let (file, template) = match language {
        Language::En => (TEMPLATE_EN_FILE, TEMPLATE_EN),
        Language::De => (TEMPLATE_DE_FILE, TEMPLATE_DE),
    };
    let wanted = variant_for(ask_mode);
    let (body, variants) = split_variants_with(template, true);
    let variant = variants
        .iter()
        .find(|(name, _)| name == wanted)
        .map(|(_, text)| text.as_str())
        .ok_or_else(|| broken(file, &format!("there is no block for ask_mode {wanted}")))?;

    let body = strip_comments(body);
    let mut text = String::with_capacity(body.len() + variant.len());
    for line in body.lines() {
        if llm_host.is_none() && line.contains(PLACEHOLDER_LLM_HOST) {
            continue;
        }
        text.push_str(line.trim_end());
        text.push('\n');
    }

    let max_time = timeout_secs.saturating_add(BASH_TIMEOUT_MARGIN_SECS);
    let mut text = text
        .replace(PLACEHOLDER_ASK_MODE, variant)
        .replace(PLACEHOLDER_TIMEOUT, &timeout_secs.to_string())
        .replace(PLACEHOLDER_MAX_TIME, &max_time.to_string());
    if let Some(host) = llm_host {
        text = text.replace(PLACEHOLDER_LLM_HOST, host);
    }
    let text = tidy(&text);

    if let Some((_, rest)) = text.split_once('{') {
        return Err(broken(
            file,
            &format!(
                "a placeholder was left unreplaced near {:?}",
                rest.chars().take(24).collect::<String>()
            ),
        ));
    }
    let Some(head) = front_matter(&text) else {
        return Err(broken(
            file,
            "the skill does not start with a front matter of name and description",
        ));
    };
    if head.name != SKILL_NAME {
        return Err(broken(
            file,
            &format!("the skill is named {:?}, not {SKILL_NAME:?}", head.name),
        ));
    }
    Ok(text)
}

/// Liest den Kopf eines Skills, streng.
///
/// Strenger als die Agenten selbst, damit nichts durchrutscht, was einer von
/// beiden anders läse: Die erste Zeile ist `---`, dann folgen genau die
/// Schlüssel `name` und `description`, je einmal, bis zur nächsten Zeile
/// `---`. Der Name ist eine schlichte Zeichenfolge nach agentskills.io
/// (Kleinbuchstaben, Ziffern, einzelne Bindestriche innen, höchstens
/// [`NAME_MAX_CHARS`]). Die Beschreibung steht in doppelten
/// Anführungszeichen, ohne `"` und `\` darin, damit kein YAML-Leser einen
/// Doppelpunkt oder ein Escape anders deutet, und ist nicht leer und höchstens
/// [`DESCRIPTION_MAX_CHARS`] lang.
///
/// `None`, wenn eine dieser Bedingungen nicht hält.
#[must_use]
pub fn front_matter(text: &str) -> Option<FrontMatter> {
    let mut lines = text.lines();
    if lines.next()? != FRONT_MATTER_FENCE {
        return None;
    }
    let mut name = None;
    let mut description = None;
    for line in lines.by_ref() {
        if line == FRONT_MATTER_FENCE {
            let name: String = name?;
            let description: String = description?;
            return Some(FrontMatter { name, description });
        }
        let (key, value) = line.split_once(": ")?;
        match key {
            "name" if name.is_none() && is_valid_name(value) => name = Some(value.to_owned()),
            "description" if description.is_none() => {
                let inner = value.strip_prefix('"')?.strip_suffix('"')?;
                if inner.is_empty()
                    || inner.chars().count() > DESCRIPTION_MAX_CHARS
                    || inner.contains(['"', '\\'])
                {
                    return None;
                }
                description = Some(inner.to_owned());
            }
            _ => return None,
        }
    }
    None
}

/// Ein Name nach agentskills.io.
fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= NAME_MAX_CHARS
        && name
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_config::{AskMode, Language};

    use super::{
        DESCRIPTION_MAX_CHARS, FrontMatter, SKILL_NAME, TEMPLATE_DE, TEMPLATE_EN, front_matter,
        render,
    };
    use crate::agent::briefing::split_variants_with;

    const HOST: &str = "ollama.lan:11434";

    /// So viele Zeichen darf der gerenderte Skill höchstens haben.
    ///
    /// Ein Stolperdraht wie die Zeichengrenze des Briefings: Gemessen am
    /// 2026-10-31 hat der längste Fall (Deutsch, `ui`, langer Endpunkt)
    /// rund 4300 Zeichen. Der Skill steht nur im Kontext, wenn das Modell ihn
    /// lädt, darf also länger sein als das Briefing, aber nicht unbegrenzt.
    const SKILL_MAX_CHARS: usize = 5000;

    /// Weder Briefing noch Skill erzählen dem Agenten von einem Menschen, der
    /// entscheidet (Entscheidung des Owners vom 2026-10-31, HUM-364). „Der
    /// Nutzer" darf vorkommen: das ist der, mit dem der Agent spricht und dem
    /// er sagt, was nicht erreichbar war, nie der, der entscheidet.
    ///
    /// Verglichen wird Wort für Wort, nicht nach Teilzeichenfolgen: sonst
    /// träfe `human` den Namen `humanitl` und `frage` die `Anfrage`. Ein
    /// Eintrag mit `*` am Ende ist ein Wortanfang (`mensch*` trifft
    /// `Menschen`), einer mit Leerzeichen eine Folge ganzer Wörter.
    #[test]
    fn no_text_tells_the_agent_about_a_person_deciding() {
        let forbidden = [
            "person*",
            "human",
            "humans",
            "mensch*",
            "approv*",
            "freigabe*",
            "review*",
            "prüf*",
            "decide*",
            "entschied*",
            "entscheid*",
            "question*",
            "frage",
            "fragt",
            "held",
            "angehalten",
            "asks the user",
            "only the user",
            "nur der nutzer",
        ];
        for language in [Language::En, Language::De] {
            for ask_mode in [AskMode::Ui, AskMode::Terminal, AskMode::None] {
                let texts = [
                    render(language, ask_mode, 300, Some(HOST)).unwrap(),
                    crate::agent::briefing::render(language, ask_mode, 300, Some(HOST)).unwrap(),
                ];
                for text in &texts {
                    let lower = text.to_lowercase();
                    let words: Vec<&str> = lower
                        .split(|ch: char| !ch.is_alphanumeric())
                        .filter(|word| !word.is_empty())
                        .collect();
                    for entry in forbidden {
                        assert!(
                            !contains_words(&words, entry),
                            "{language:?}/{ask_mode:?}: {entry:?} in: {text}"
                        );
                    }
                }
            }
        }
    }

    /// Die Antworten des Proxys, die der Skill erklärt, und derselbe Rat nach
    /// einem `504` in beiden Ask-Modi (Review HUM-364).
    #[test]
    fn the_answers_and_the_advice_after_a_504_are_complete() {
        for (language, retry, data, catch_all) in [
            (
                Language::En,
                "request access once with `POST /ask` and send the request once more later.",
                "too many requests or too much data",
                "Any other status or reason",
            ),
            (
                Language::De,
                "beantrage einmal Zugang mit `POST /ask` und sende die Anfrage später noch einmal.",
                "zu viele Anfragen oder zu viele Daten",
                "Jeder andere Status oder Grund",
            ),
        ] {
            for ask_mode in [AskMode::Ui, AskMode::None] {
                let text = render(language, ask_mode, 300, Some(HOST)).unwrap();
                for needle in [
                    retry,
                    data,
                    catch_all,
                    "upstream_private_address",
                    "`429`",
                    "retry-after",
                    "`403`",
                ] {
                    assert!(
                        text.contains(needle),
                        "{language:?}/{ask_mode:?}: {needle:?}: {text}"
                    );
                }
                assert_eq!(
                    text.matches(retry).count(),
                    1,
                    "{language:?}/{ask_mode:?}: the advice stands once"
                );
            }
        }
    }

    /// Wahr, wenn `entry` als ganze Wörter in `words` steht; ein `*` am Ende
    /// des letzten Worts erlaubt jede Fortsetzung dieses Worts.
    fn contains_words(words: &[&str], entry: &str) -> bool {
        let wanted: Vec<&str> = entry.split(' ').collect();
        words.windows(wanted.len()).any(|window| {
            window.iter().zip(&wanted).all(|(word, want)| {
                want.strip_suffix('*')
                    .map_or(word == want, |prefix| word.starts_with(prefix))
            })
        })
    }

    #[test]
    fn the_word_matcher_respects_word_boundaries() {
        let words = ["humanitl", "anfrage", "menschen", "only", "the", "user"];
        assert!(!contains_words(&words, "human"));
        assert!(!contains_words(&words, "frage"));
        assert!(contains_words(&words, "mensch*"));
        assert!(contains_words(&words, "only the user"));
        assert!(!contains_words(&words, "the only"));
    }

    #[test]
    fn both_templates_carry_both_variants() {
        for (language, template) in [(Language::En, TEMPLATE_EN), (Language::De, TEMPLATE_DE)] {
            let (_, variants) = split_variants_with(template, true);
            let names: Vec<&str> = variants.iter().map(|(name, _)| name.as_str()).collect();
            assert_eq!(names, vec!["ui", "none"], "{language:?}");
            for (name, text) in &variants {
                assert!(
                    text.starts_with("## "),
                    "{language:?}: the block {name} starts with its heading: {text}"
                );
                assert!(
                    text.lines().count() > 1,
                    "{language:?}: the block {name} keeps its lines"
                );
            }
        }
    }

    /// Der Kopf steht als Erstes, trägt den Namen `humanitl` und eine
    /// Beschreibung, nach der das Modell den Skill bei langsamen, gehaltenen
    /// oder abgelehnten Anfragen und vor Arbeit im Netz lädt.
    #[test]
    fn the_front_matter_parses_and_routes_on_slow_or_refused_requests() {
        for (language, triggers) in [
            (
                Language::En,
                &[
                    "slow",
                    "fails",
                    "blocked",
                    "refused",
                    "504",
                    "before network-heavy work",
                ][..],
            ),
            (
                Language::De,
                &[
                    "langsam",
                    "scheitert",
                    "blockiert",
                    "abgelehnt",
                    "504",
                    "vor netzlastiger Arbeit",
                ][..],
            ),
        ] {
            for ask_mode in [AskMode::Ui, AskMode::Terminal, AskMode::None] {
                for host in [None, Some(HOST)] {
                    let text = render(language, ask_mode, 300, host).unwrap();
                    assert!(text.starts_with("---\nname: humanitl\n"), "{text}");
                    let FrontMatter { name, description } =
                        front_matter(&text).expect("the front matter parses");
                    assert_eq!(name, SKILL_NAME);
                    assert!(description.chars().count() <= DESCRIPTION_MAX_CHARS);
                    for trigger in triggers {
                        assert!(
                            description.contains(trigger),
                            "{language:?}: the description names {trigger:?}: {description}"
                        );
                    }
                    assert!(!text.contains("<!--"), "a comment leaked: {text}");
                    assert!(!text.contains("-->"), "a comment leaked: {text}");
                    assert!(text.ends_with('\n'));
                }
            }
        }
    }

    /// Die Frist der Konfiguration steht im Text, mit der Frist für `curl`
    /// darüber; ohne Frage steht keine Frist im Text.
    #[test]
    fn the_timeout_is_rendered_and_curl_waits_longer() {
        for (language, up_to) in [(Language::En, "up to 42s"), (Language::De, "bis zu 42s")] {
            let asked = render(language, AskMode::Ui, 42, Some(HOST)).unwrap();
            assert!(asked.contains(up_to), "{asked}");
            assert!(asked.contains("curl --max-time 102"), "{asked}");
            assert!(!asked.contains('{'), "{asked}");

            let unasked = render(language, AskMode::None, 42, Some(HOST)).unwrap();
            assert!(
                !unasked.contains("42s"),
                "no deadline where nobody is asked"
            );
            assert!(!unasked.contains("--max-time"), "{unasked}");
            assert!(unasked.contains("504"), "{unasked}");
        }
    }

    /// Ohne Durchreiche fällt der Satz über das Modell weg, sonst nennt er den
    /// Host.
    #[test]
    fn the_model_line_follows_the_passthrough() {
        for language in [Language::En, Language::De] {
            let without = render(language, AskMode::Ui, 300, None).unwrap();
            assert!(!without.contains("11434"), "{without}");
            let with = render(language, AskMode::Ui, 300, Some(HOST)).unwrap();
            assert!(with.contains(HOST), "{with}");
        }
    }

    /// Die Endpunkte des Meta-Hosts, die der Text nennt, und die Grenze von
    /// `/ask`.
    #[test]
    fn the_meta_endpoints_are_named_with_their_limit() {
        for (language, only_the_user) in [
            (Language::En, "It grants nothing by itself."),
            (Language::De, "Er gewährt von sich aus nichts."),
        ] {
            let text = render(language, AskMode::Ui, 300, Some(HOST)).unwrap();
            for needle in [
                "curl -s http://humanitl.internal/",
                "http://humanitl.internal/why/",
                "http://humanitl.internal/ask",
                "`202`",
                only_the_user,
            ] {
                assert!(text.contains(needle), "{language:?}: {needle:?}: {text}");
            }
        }
    }

    #[test]
    fn the_skill_stays_within_its_size() {
        for language in [Language::En, Language::De] {
            for ask_mode in [AskMode::Ui, AskMode::None] {
                let text = render(
                    language,
                    ask_mode,
                    3600,
                    Some("ollama.services.example.internal:11434"),
                )
                .unwrap();
                assert!(
                    text.chars().count() <= SKILL_MAX_CHARS,
                    "{language:?}/{ask_mode:?} is {} characters",
                    text.chars().count()
                );
            }
        }
    }

    #[test]
    fn the_front_matter_parser_is_strict() {
        let ok = "---\nname: humanitl\ndescription: \"Load: when slow.\"\n---\n# x\n";
        assert_eq!(
            front_matter(ok),
            Some(FrontMatter {
                name: "humanitl".to_owned(),
                description: "Load: when slow.".to_owned(),
            })
        );
        for bad in [
            "# x\n---\nname: humanitl\ndescription: \"d\"\n---\n",
            "---\nname: humanitl\n---\n",
            "---\ndescription: \"d\"\n---\n",
            "---\nname: Humanitl\ndescription: \"d\"\n---\n",
            "---\nname: humanitl\ndescription: plain: colon\n---\n",
            "---\nname: humanitl\ndescription: \"\"\n---\n",
            "---\nname: humanitl\ndescription: \"a \\\" b\"\n---\n",
            "---\nname: humanitl\nname: humanitl\ndescription: \"d\"\n---\n",
            "---\nname: humanitl\ndescription: \"d\"\nlicense: MIT\n---\n",
            "---\nname: humanitl\ndescription: \"d\"\n",
        ] {
            assert_eq!(front_matter(bad), None, "{bad:?}");
        }
        let long = format!(
            "---\nname: humanitl\ndescription: \"{}\"\n---\n",
            "x".repeat(DESCRIPTION_MAX_CHARS + 1)
        );
        assert_eq!(front_matter(&long), None);
    }
}
