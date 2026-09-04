//! Die Einweisung, die der Agent in der Sandbox vorfindet (HUM-071, ADR-0014).
//!
//! Ein Agent in dieser Sandbox erlebt eine Umgebung, für die er nicht
//! trainiert wurde: sein Abruf hängt minutenlang und endet mit `403`, sein
//! Update-Check scheitert, ein Werkzeug bekommt gar keine Verbindung. Ohne
//! Erklärung wiederholt er, variiert die Adresse und sucht einen Umweg. Das
//! Briefing ist der eine Text, mit dem Humanitl das verhindert, und der Agent
//! behandelt ihn als Wahrheit.
//!
//! Daraus folgen die beiden Regeln, nach denen der Text geschrieben ist:
//!
//! 1. **Jede Aussage ist am Verhalten geprüft, nicht an der Spezifikation.**
//!    Der Statuscode einer Ablehnung steht in `BlockReason::http_status`, der
//!    Wortlaut des Rumpfs in `humanitl_core::block`, die Frist in
//!    `hold.timeout_secs`. Eine Aussage, die dem Agenten ein falsches Modell
//!    gibt, erzeugt genau das Verhalten, das der Text verhindern soll.
//! 2. **Der Text bleibt kurz.** ADR-0014 nennt etwa 150 Token; die Grenze ist
//!    [`TOKEN_BUDGET`]. Ein langes Briefing verdrängt Kontext, den der Agent
//!    für seine Arbeit braucht, und wird überlesen.
//!
//! # Was der Text sagt und woran es geprüft ist
//!
//! | Aussage | Beleg im Code |
//! |---|---|
//! | Keine Netzwerkschnittstelle, alles über den Proxy | `--unshare-all` in `crate::bwrap_args`, `humanitl_proxy::ca::ENV_KIT` mit `HTTP_PROXY` und leerem `NO_PROXY` |
//! | Regeln entscheiden das meiste, der Rest wartet auf einen Menschen | `humanitl_proxy::pipeline::RulesPipeline` über `AskPipeline` |
//! | Ein Rumpf mit `Blocked by Humanitl.` kommt vom Proxy | `humanitl_core::block::BANNER` |
//! | `403` heißt: gegen die Anfrage entschieden | `BlockReason::User`, `Rule`, `AuthorityMismatch`, `PrivateAddress`, `Secret` |
//! | Eine `note:`-Zeile kann dabeistehen | `humanitl_core::block::block_response` |
//! | Mit Frage endet die abgelaufene Frist in `504` | `BlockReason::Timeout` aus `Decision::TimedOut` |
//! | Ohne Frage scheitert eine Anfrage ohne Regel sofort mit `504` | `ask_mode = none` setzt die Frist auf null, der Flow endet sofort als `TimedOut` |
//! | Eine Regel deckt die Modell-Aufrufe, andere Pfade nicht | `OpenCodeAdapter::llm_passthrough` baut **eine** Regel: nur dieser Host, dieses Schema, dieser Port, `GET`/`POST` und die Pfade aus `llm.passthrough_paths`; `POST /api/pull` und `POST /v1/files` fallen durch und gehen den Weg jeder anderen Anfrage |
//!
//! Der Satz über das Modell sagt ausdrücklich *„eine Regel erlaubt"* und nicht
//! *„läuft ohne Warten durch"*. Eine `allow`-Regel des Nutzers wartet ebenfalls
//! nicht; „ohne Warten" wäre also kein Merkmal der Durchreiche, sondern eine
//! Aussage, die für jede erlaubte Anfrage gilt — und im Modus `none` obendrein
//! irreführend, weil dort auch das Abgelehnte nicht wartet.
//! | `GET http://humanitl.internal/` listet die Regeln | `humanitl_proxy::meta::MetaEndpoint::respond`, `200` mit Kurzstatus und einer Zeile je geltender Regel |
//! | `POST /ask` mit einer Zeile fragt den Nutzer, Antwort `202` | `MetaEndpoint::ask`: `202 queued`, dazu `FlowEvent::AgentAsk` und eine Karte in der Oberfläche |
//! | Nur der Nutzer kann eine Regel anlegen | `/ask` erzeugt ein Ereignis und nie eine Regel (ADR-0014, HUM-073 „Nicht-Ziel") |
//!
//! **Warum `504` in den Ask-Modus-Blöcken steht und nicht im gemeinsamen
//! Text.** Der Code ist in beiden Modi derselbe, die Ursache nicht: mit Frage
//! ist eine Frist abgelaufen, ohne Frage wurde nie jemand gefragt. Ein
//! gemeinsamer Satz „niemand hat rechtzeitig entschieden" wäre im Modus `none`
//! irreführend und ließe den Agenten auf eine Entscheidung warten, die es dort
//! nicht gibt (Review vom 2026-09-04).
//!
//! **Was der Text über `/ask` nicht sagt.** Ein leerer Rumpf ist `400`, mehr
//! als 2 KiB `413`, mehr als zehn Bitten je Minute und Sitzung `429`. Der Text
//! nennt nur den Weg, der gilt, wenn der Agent sich an „eine Zeile" hält;
//! jeder Fehlercode mehr kostet Token, die keine Fehlbedienung verhindern.
//!
//! Ebenfalls nicht im Text: eine Aufforderung, den Proxy nicht zu umgehen.
//! Der Fallstrick des Issues ist ausdrücklich, dass keine Formulierung
//! „Umgehen" als Möglichkeit in den Raum stellt, auch nicht verneint. Der Text
//! nennt deshalb nur die Tatsache, dass ohne die Proxy-Umgebung keine
//! Verbindung zustande kommt. Genannt wird die Umgebung, nicht eine einzelne
//! Variable: `humanitl_proxy::ca::ENV_KIT` setzt `HTTP_PROXY`, `HTTPS_PROXY`,
//! `ALL_PROXY` und die kleingeschriebenen Formen, und ein Satz über nur eine
//! davon wäre halb wahr.
//!
//! # Das Format der Vorlagen
//!
//! `agents/opencode/briefing.{en,de}.md` sind Markdown mit drei Platzhaltern
//! ([`PLACEHOLDER_ASK_MODE`], [`PLACEHOLDER_TIMEOUT`],
//! [`PLACEHOLDER_LLM_HOST`]) und zwei Besonderheiten:
//!
//! - HTML-Kommentare fallen weg, auch mehrzeilige. So kann in der Datei
//!   stehen, was ein Übersetzer wissen muss, ohne dass der Agent es liest.
//! - Alles ab der ersten Zeile, die mit `<!-- ask_mode:` beginnt, sind
//!   Varianten und keine Ausgabe. Die zum Ask-Modus passende Variante tritt an
//!   die Stelle von [`PLACEHOLDER_ASK_MODE`]. Damit steht auch der Satz, der
//!   von `hold.ask_mode` abhängt, in der Datei und nicht im Quelltext — sonst
//!   wäre die Hälfte des Textes nicht übersetzbar (ADR-0014).

use humanitl_config::{AskMode, Language};
use humanitl_core::Diagnostic;

use crate::agent::opencode_models::broken;

/// Die englische Vorlage.
pub const TEMPLATE_EN: &str = include_str!("../../../../../agents/opencode/briefing.en.md");

/// Die deutsche Vorlage.
pub const TEMPLATE_DE: &str = include_str!("../../../../../agents/opencode/briefing.de.md");

/// Der Pfad der englischen Vorlage im Repository; steht in Fehlermeldungen.
pub const TEMPLATE_EN_FILE: &str = "agents/opencode/briefing.en.md";

/// Der Pfad der deutschen Vorlage im Repository; steht in Fehlermeldungen.
pub const TEMPLATE_DE_FILE: &str = "agents/opencode/briefing.de.md";

/// So viele Token darf der gerenderte Text höchstens haben.
///
/// **Warum 185 und nicht die 160 aus `backlog/sprint-3.md`.** Beide Zahlen —
/// „etwa 150" in ADR-0014 und 160 im Issue — sind Schätzungen aus der Zeit vor
/// dem Meta-Endpunkt und vor der ersten Zählung. Gemessen mit
/// `tools/briefing-tokens.py` im ungünstigsten Fall (`o200k_base`, langer
/// Endpunkt, vierstellige Frist) braucht der englische Text 162 Token und der
/// deutsche 184. Der Unterschied ist keine Weitschweifigkeit der Übersetzung,
/// sondern der Preis der Sprache: jeder BPE-Tokenisierer zerlegt deutsche
/// Wörter feiner, hier um 14 Prozent. Der deutsche Text unter 160 zu
/// halten hieße, eine Aussage wegzulassen, die der englische macht — und damit
/// die Zusage aufzugeben, dass die Übersetzung dieselben Aussagen in derselben
/// Reihenfolge trägt.
///
/// 185 ist die Grenze knapp über dem gemessenen Wert, also weiter ein
/// Stolperdraht und kein Freibrief. Wer den Text wachsen lässt, zählt nach und
/// streicht anderswo; wer die Grenze heben will, nennt die Aussage, die den
/// Platz wert ist.
///
/// Gezählt wird mit `tools/briefing-tokens.py`. In dieser Crate liegt kein
/// Tokenisierer; die Tests halten deshalb zusätzlich eine Zeichengrenze als
/// billigen Stolperdraht fest.
pub const TOKEN_BUDGET: usize = 185;

/// Der Platzhalter für den Satz, der von `hold.ask_mode` abhängt.
pub const PLACEHOLDER_ASK_MODE: &str = "{ask_mode}";

/// Der Platzhalter für `hold.timeout_secs`, in Sekunden ohne Einheit.
pub const PLACEHOLDER_TIMEOUT: &str = "{timeout}";

/// Der Platzhalter für den LLM-Endpunkt als `host` oder `host:port`.
pub const PLACEHOLDER_LLM_HOST: &str = "{llm_host}";

/// Die Zeile, die eine Variante einleitet.
const VARIANT_MARKER: &str = "<!-- ask_mode:";

/// Der Name der Variante für `ask_mode = ui` und `ask_mode = terminal`.
///
/// Beide fragen einen Menschen; nur der Ort der Frage ist ein anderer, und den
/// braucht der Agent nicht zu wissen.
const VARIANT_ASK: &str = "ui";

/// Der Name der Variante für `ask_mode = none`.
const VARIANT_NONE: &str = "none";

/// Beginn eines HTML-Kommentars.
const COMMENT_OPEN: &str = "<!--";

/// Ende eines HTML-Kommentars.
const COMMENT_CLOSE: &str = "-->";

/// Rendert die Einweisung für eine Sitzung.
///
/// `timeout_secs` ist `hold.timeout_secs`, `ask_mode` ist `hold.ask_mode`, und
/// `llm_host` ist der Host der Durchreiche als `host:port`.
///
/// **`llm_host` kommt fertig herein und wird hier nicht mehr geprüft.** Die
/// eine richtige Quelle dafür ist
/// [`crate::agent::opencode::passthrough_authority`], also dieselbe Prüfung,
/// aus der auch die Durchreichregel entsteht. `None` heißt: es gibt keine
/// Durchreiche, und dann fällt die Zeile mit [`PLACEHOLDER_LLM_HOST`] ganz weg
/// — ein Satz über ein Modell, das nicht durchgereicht wird, wäre eine
/// Behauptung ohne Beleg (`backlog/CONVENTIONS.md` 4.13).
///
/// # Errors
///
/// Ein [`Diagnostic`] mit `AGENT_003`, wenn die einkompilierte Vorlage nicht
/// die erwartete Form hat: keine Variante zum Ask-Modus, oder ein Platzhalter,
/// den niemand ersetzt hat. Das ist ein Fehler im Build, keine Nutzereingabe.
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

    let wanted = match ask_mode {
        AskMode::Ui | AskMode::Terminal => VARIANT_ASK,
        AskMode::None => VARIANT_NONE,
    };
    let (body, variants) = split_variants(template);
    let variant = variants
        .iter()
        .find(|(name, _)| name == wanted)
        .map(|(_, text)| text.as_str())
        .ok_or_else(|| {
            broken(
                file,
                &format!("there is no block `{VARIANT_MARKER} {wanted} {COMMENT_CLOSE}`"),
            )
        })?;

    let body = strip_comments(body);
    let mut text = String::with_capacity(body.len());
    for line in body.lines() {
        // Ohne Endpunkt fällt die ganze Zeile weg, nicht nur der Platzhalter:
        // ein Satz über „das Modell unter" ohne Adresse wäre schlimmer als
        // keiner.
        if llm_host.is_none() && line.contains(PLACEHOLDER_LLM_HOST) {
            continue;
        }
        text.push_str(line.trim_end());
        text.push('\n');
    }

    // Erst die Variante, dann die Frist: `{timeout}` steht in der Variante und
    // nicht im Rumpf, wäre also noch gar nicht da.
    let mut text = text
        .replace(PLACEHOLDER_ASK_MODE, variant)
        .replace(PLACEHOLDER_TIMEOUT, &timeout_secs.to_string());
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
    Ok(text)
}

/// Trennt den Rumpf der Vorlage von den Varianten.
///
/// Alles vor der ersten Markerzeile ist Rumpf; danach folgt je Marker ein
/// Block bis zum nächsten Marker oder zum Dateiende. Geprüft wird der Anfang
/// der Zeile, nicht ein Vorkommen irgendwo: `<!-- ask_mode: … -->` steht auch
/// im erklärenden Kommentar am Kopf der Datei, und der ist Rumpf.
///
/// Ohne Marker ist die ganze Vorlage Rumpf und die Liste leer; [`render`]
/// meldet das als `AGENT_003`.
fn split_variants(template: &str) -> (&str, Vec<(String, String)>) {
    let mut offset = 0;
    let mut start = None;
    for line in template.split_inclusive('\n') {
        if line.trim_start().starts_with(VARIANT_MARKER) {
            start = Some(offset);
            break;
        }
        offset += line.len();
    }
    let Some(start) = start else {
        return (template, Vec::new());
    };

    let (body, tail) = template.split_at(start);
    let mut variants: Vec<(String, String)> = Vec::new();
    for line in tail.lines() {
        let trimmed = line.trim();
        if let Some(name) = variant_name(trimmed) {
            variants.push((name, String::new()));
            continue;
        }
        if let Some((_, text)) = variants.last_mut() {
            if !text.is_empty() && !trimmed.is_empty() {
                text.push(' ');
            }
            text.push_str(trimmed);
        }
    }
    for (_, text) in &mut variants {
        *text = text.trim().to_owned();
    }
    (body, variants)
}

/// Der Name einer Variante aus ihrer Markerzeile.
fn variant_name(line: &str) -> Option<String> {
    let rest = line.strip_prefix(VARIANT_MARKER)?;
    let name = rest.strip_suffix(COMMENT_CLOSE)?;
    Some(name.trim().to_owned())
}

/// Entfernt jeden HTML-Kommentar, auch einen mehrzeiligen.
///
/// Ein Kommentar ohne Ende schluckt den Rest der Datei. Das ist die
/// vorsichtige Seite: eher fehlt ein Satz im Briefing, als dass ein Hinweis
/// für Übersetzer beim Agenten landet.
fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find(COMMENT_OPEN) {
        out.push_str(&rest[..open]);
        let after = &rest[open + COMMENT_OPEN.len()..];
        let Some(close) = after.find(COMMENT_CLOSE) else {
            return out;
        };
        rest = &after[close + COMMENT_CLOSE.len()..];
    }
    out.push_str(rest);
    out
}

/// Räumt den gerenderten Text auf: keine doppelten Leerzeilen, kein Leerraum
/// am Rand, genau ein Zeilenende am Schluss.
fn tidy(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_was_blank = true;
    for line in text.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            if last_was_blank {
                continue;
            }
            last_was_blank = true;
        } else {
            last_was_blank = false;
        }
        out.push_str(line);
        out.push('\n');
    }
    while out.ends_with("\n\n") {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_config::{AskMode, Language};

    use super::{
        PLACEHOLDER_ASK_MODE, PLACEHOLDER_LLM_HOST, TEMPLATE_DE, TEMPLATE_EN, render,
        split_variants, strip_comments, tidy,
    };

    /// Ein Endpunkt, wie ihn `passthrough_authority` liefert.
    const HOST: &str = "ollama.lan:11434";

    #[test]
    fn both_templates_carry_both_variants() {
        for (language, template) in [(Language::En, TEMPLATE_EN), (Language::De, TEMPLATE_DE)] {
            let (body, variants) = split_variants(template);
            let names: Vec<&str> = variants.iter().map(|(name, _)| name.as_str()).collect();
            assert_eq!(names, vec!["ui", "none"], "{language:?}");
            assert!(
                body.contains(PLACEHOLDER_ASK_MODE),
                "{language:?}: the body carries the placeholder the variants replace"
            );
            for (name, text) in &variants {
                assert!(!text.is_empty(), "{language:?}: the block {name} is empty");
            }
        }
    }

    #[test]
    fn the_timeout_of_the_configuration_ends_up_in_the_text() {
        let text = render(Language::En, AskMode::Ui, 42, None).unwrap();
        assert!(text.contains("up to 42s"), "{text}");
    }

    /// Ohne Durchreiche fehlt die Zeile über das Modell ganz, statt einen Rest
    /// zu hinterlassen.
    #[test]
    fn without_a_passthrough_the_model_line_is_gone() {
        for language in [Language::En, Language::De] {
            let text = render(language, AskMode::Ui, 300, None).unwrap();
            assert!(!text.contains(PLACEHOLDER_LLM_HOST), "{text}");
            assert!(!text.contains("11434"), "{text}");
            let with = render(language, AskMode::Ui, 300, Some(HOST)).unwrap();
            assert!(with.contains(HOST), "{with}");
        }
    }

    /// `ask_mode = none` fragt niemanden; der Text darf dann weder vom Warten
    /// sprechen noch `504` als abgelaufene Frist erklären, sonst wartet der
    /// Agent auf eine Entscheidung, die nie kommt.
    #[test]
    fn ask_mode_none_replaces_the_sentence_about_waiting() {
        for (language, waiting) in [
            (Language::En, "Waiting is normal"),
            (Language::De, "Warten ist normal"),
        ] {
            let asked = render(language, AskMode::Ui, 300, None).unwrap();
            assert!(asked.contains(waiting), "{asked}");
            assert!(asked.contains("504"), "the deadline ends in 504: {asked}");

            let unasked = render(language, AskMode::None, 300, None).unwrap();
            assert!(!unasked.contains(waiting), "{unasked}");
            assert!(
                !unasked.contains("300"),
                "no deadline where nobody is asked: {unasked}"
            );
            assert!(
                unasked.contains("504"),
                "the immediate failure is 504 as well: {unasked}"
            );
        }
        // `terminal` fragt einen Menschen wie `ui` und bekommt denselben Satz.
        assert_eq!(
            render(Language::En, AskMode::Terminal, 300, None).unwrap(),
            render(Language::En, AskMode::Ui, 300, None).unwrap()
        );
    }

    /// Der Weg zum Nutzer steht im Text, und die Grenze dieses Weges auch.
    ///
    /// `/ask` erzeugt eine Karte und nie eine Regel (ADR-0014). Ein Briefing,
    /// das das offenließe, legte dem Agenten nahe, er könne sich selbst
    /// Zugang verschaffen.
    #[test]
    fn the_meta_endpoint_is_named_together_with_its_limit() {
        for (language, only_the_user) in [
            (Language::En, "Only the user can add a rule."),
            (Language::De, "Nur der Nutzer kann eine Regel anlegen."),
        ] {
            for ask_mode in [AskMode::Ui, AskMode::None] {
                let text = render(language, ask_mode, 300, Some(HOST)).unwrap();
                assert!(text.contains("http://humanitl.internal/"), "{text}");
                assert!(text.contains("/ask"), "{text}");
                assert!(text.contains("202"), "{text}");
                assert!(text.contains(only_the_user), "{text}");
            }
        }
    }

    #[test]
    fn comments_are_stripped_across_lines() {
        assert_eq!(strip_comments("a<!-- x\ny -->b"), "ab");
        assert_eq!(strip_comments("a<!-- x\ny"), "a");
        assert_eq!(strip_comments("plain"), "plain");
    }

    #[test]
    fn tidy_collapses_blank_lines_and_ends_with_exactly_one_newline() {
        assert_eq!(tidy("\n\na\n\n\n\nb  \n\n\n"), "a\n\nb\n");
    }
}
