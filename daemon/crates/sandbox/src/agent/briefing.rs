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
//! | Internetzugang über HTTP(S) mit `curl`, `wget` und anderen Clients, durch den Proxy der Umgebung, sonst nichts | `--unshare-all` in `crate::bwrap_args` (kein anderer Weg hinaus), `humanitl_proxy::ca::ENV_KIT` mit `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`, leerem `NO_PROXY` und `NODE_USE_ENV_PROXY=1` für Nodes `fetch` (HUM-304); pi stellt seinen Dispatcher selbst auf dieselben Variablen ein |
//! | Ein Rumpf mit `Blocked by Humanitl.` kommt vom Proxy | `humanitl_core::block::BANNER` |
//! | `403` heißt: hier nicht erlaubt | `BlockReason::User`, `Rule`, `AuthorityMismatch`, `PrivateAddress`, `Secret` |
//! | Eine `note:`-Zeile kann dabeistehen | `humanitl_core::block::block_response` |
//! | `GET http://humanitl.internal/` listet, was erlaubt ist | `humanitl_proxy::meta::MetaEndpoint::respond`, `200` mit Kurzstatus und einer Zeile je geltender Regel |
//! | `POST /ask` mit einer Zeile beantragt Zugang, Antwort `202`, Zugang kann später folgen | `MetaEndpoint::ask`: `202 queued`, dazu `FlowEvent::AgentAsk` und eine Karte in der Oberfläche; legt der Mensch danach eine Regel an, gilt sie für die nächste Anfrage |
//! | `/ask` gewährt von sich aus nichts | `/ask` erzeugt ein Ereignis und nie eine Regel (ADR-0014, HUM-073 „Nicht-Ziel") |
//! | Eine Regel deckt die Modell-Aufrufe, andere Pfade nicht | `OpenCodeAdapter::llm_passthrough` baut **eine** Regel: nur dieser Host, dieses Schema, dieser Port, `GET`/`POST` und die Pfade aus `llm.passthrough_paths`; `POST /api/pull` und `POST /v1/files` fallen durch und gehen den Weg jeder anderen Anfrage |
//! | Modell-Aufrufe warten hier nie, eine Verzögerung kommt vom Modellserver (HUM-364) | `Matched { Allow }` mit `passthrough_llm` wird in `humanitl_proxy::pipeline::RulesPipeline` zu `Decision::Allow` mit Quelle `Passthrough`, ohne Hold; Funde warnen (`LLM_005`) statt anzuhalten; die Notiz der Regel sagt „Logged, never held." |
//! | Mit Frage: hohe, schwankende Latenz bis `{timeout}`s, dann `504`, weil keine Erlaubnis rechtzeitig kam (HUM-364) | Was keine Regel erlaubt oder blockt, hält `HoldQueue::hold` bis `hold.timeout_secs` (Vorgabe 300); solange kommt keine Antwort, danach `BlockReason::Timeout` aus `Decision::TimedOut` |
//! | Tool-Fristen über `{timeout}`s, keine Schleifen, keine parallelen Duplikate, kein Wechsel von Werkzeug oder Spiegel (HUM-364) | Jede neue Anfrage ist ein neuer Flow und wartet von vorn; viele gleichzeitig enden in `503` (`BlockReason::HoldMaxFlows`) |
//! | Gibt ein Werkzeug früher auf, ist die Anfrage weg; `curl` wartet (nur mit Frage, HUM-308) | Bricht der Client ab, endet der Flow in `HoldQueue::hold` als `ClientTimeout`, und die Karte verlässt die Warteschlange. `webfetch` von `OpenCode` 1.18.31 gibt nach 30, höchstens 120 Sekunden auf, fest im Code; `curl` hat keine eigene Gesamtfrist, `bash` von pi 1.4.0 keine Vorgabefrist, und die von `OpenCode` hebt `opencode::bash_default_timeout_ms` über `hold.timeout_secs` |
//! | Ohne Frage geht nur durch, was Regeln erlauben, nichts wartet, alles andere sofort `504` | `ask_mode = none` setzt die Frist auf null, der Flow endet sofort als `TimedOut` |
//! | Mehr im Skill `humanitl` (HUM-364) | Beide Adapter legen ihn neben dem Briefing an, solange `agent.briefing.enabled` gilt ([`crate::agent::skill`]) |
//!
//! **Kein Mensch im Text (HUM-364).** Entscheidung des Owners vom
//! 2026-10-31: Der Agent erfährt nicht, dass ein Mensch über seine Anfragen
//! entscheidet. Die Wartezeit steht als Eigenschaft der Umgebung da, `403` als
//! „hier nicht erlaubt", `504` als „keine Erlaubnis rechtzeitig", `/ask` als
//! Antrag auf Zugang. Jede dieser Aussagen bleibt wahr; sie lässt nur weg, wer
//! entscheidet. „Der Nutzer" im Text ist der, mit dem der Agent spricht, und
//! dem sagt er, was nicht erreichbar war.
//!
//! Der Satz über das Modell sagt zuerst *„eine Regel erlaubt"* und nicht nur
//! *„läuft ohne Warten durch"*. Eine `allow`-Regel wartet ebenfalls nicht;
//! „ohne Warten" allein wäre also kein Merkmal der Durchreiche. Seit HUM-364
//! folgt dennoch „wartet hier nie", und zwar mit seiner Folge für den Agenten:
//! eine langsame Antwort des Modells ist eine Sache des Modellservers, nicht
//! des Proxys. Der Satz gilt in beiden Ask-Modi.
//!
//! **Kein Weg vorbei am Proxy im Text.** Der Block für `ui` nennt den Wechsel
//! von Werkzeug oder Spiegel als etwas, das gegen die Wartezeit nicht hilft:
//! jede neue Anfrage geht durch denselben Proxy und wartet ebenso. Ein Weg
//! vorbei am Proxy wird dabei nicht genannt, weder bejaht noch verneint.
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
//! nennt deshalb nur die Tatsache, dass HTTP(S) über den Proxy der Umgebung
//! geht und sonst nichts. Genannt wird die Umgebung, nicht eine einzelne
//! Variable: `humanitl_proxy::ca::ENV_KIT` setzt `HTTP_PROXY`, `HTTPS_PROXY`,
//! `ALL_PROXY` und die kleingeschriebenen Formen, und ein Satz über nur eine
//! davon wäre halb wahr.
//!
//! **Warum der erste Satz mit dem Zugang beginnt und nicht mit der Grenze
//! (HUM-304).** Bis dahin hieß er „No network interface: HTTP(S) leaves only
//! through the proxy". Ein Modell las das als „kein Internet" und sagte dem
//! Nutzer, es könne nichts im Web nachsehen; pi hat nur `bash` als Werkzeug
//! dafür und versuchte es gar nicht erst. Die Grenze ist dieselbe geblieben
//! („sonst nichts"), der Satz nennt aber zuerst, was geht, und womit.
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
/// Nachgezählt am 2026-10-29 nach HUM-304 (erster Satz über den
/// Internetzugang, dafür ohne „beantwortet der Proxy" vor dem Meta-Endpunkt):
/// englisch 170 und 168, deutsch weiter 184 und 179.
///
/// Nachgezählt am 2026-10-29 nach HUM-308 (Satz über Werkzeuge mit kürzerer
/// Frist im Block für `ui`; dafür entfallen „or retry" und „in a body", im
/// Deutschen „nicht wiederholen" im Block, „im Rumpf", „die" vor „Regeln",
/// „läuft" und „auf diesem Host", aus „das meiste" wird „fast alles", und
/// „eine etwaige `note:`-Zeile" wird „etwaige `note:`-Zeilen"): englisch 181
/// (`ui`) und 165 (`none`), deutsch 185 und 169. „Nicht wiederholen" fällt
/// nicht nur aus Platzgründen: Hat ein Werkzeug aufgegeben, ist die Frage weg,
/// und nur ein neuer Abruf fragt erneut. Das `403` bleibt „nicht wiederholen".
///
/// **Nachgezählt am 2026-10-31 nach HUM-364, Grenze 270.** Der Owner will,
/// dass die Agenten eine lange Wartezeit nicht für einen Ausfall halten, und
/// zwar als Eigenschaft der Umgebung: der Agent erfährt nicht, dass ein
/// Mensch entscheidet. Der Block für `ui` sagt deshalb, dass Anfragen hier
/// hohe, schwankende Latenz haben, dass eine, die noch keine Regel erlaubt
/// oder ablehnt, bis zur Frist warten kann (eine erlaubte läuft so lange wie
/// der Server braucht, eine abgelehnte endet sofort mit `403`), dass ein
/// `504` danach heißt, dass keine Erlaubnis rechtzeitig kam, und was der
/// Agent dann nicht tut (Schleifen, parallele Duplikate, Wechsel von Werkzeug
/// oder Spiegel, Schluss auf ein totes Netz); er nennt Tool-Fristen über der
/// Frist. Der Block für `none` sagt, dass nichts wartet, Abgelehntes `403`
/// bekommt und alles andere `504`, „nicht erlaubt" und kein Ausfall. Der Satz
/// über das Modell sagt, dass die Regel nur auf ihren Pfaden gilt
/// (`llm.passthrough_paths` kann auch Pfade nennen, die kein Modell-Aufruf
/// sind, etwa `/api/pull`; „sonst nichts auf diesem Host" wäre dann falsch),
/// dass diese Aufrufe hier nie warten und eine Verzögerung vom Modellserver
/// kommt; nach dem Meta-Endpunkt
/// steht der Verweis auf den Skill `humanitl`. Gemessen: englisch 250 (`ui`)
/// und 205 (`none`), deutsch 266 und 215. Jede dieser Aussagen ist genau das
/// Verhalten, das das Briefing verhindern soll, und keine passt in den Skill
/// allein: der wird nur geladen, wenn das Modell ihn für passend hält
/// (`docs/skills.md` von pi: „A model might fail to load a relevant skill").
///
/// 270 ist die Grenze knapp über dem gemessenen Wert, also weiter ein
/// Stolperdraht und kein Freibrief. Wer den Text wachsen lässt, zählt nach und
/// streicht anderswo; wer die Grenze heben will, nennt die Aussage, die den
/// Platz wert ist.
///
/// Gezählt wird mit `tools/briefing-tokens.py`. In dieser Crate liegt kein
/// Tokenisierer; die Tests halten deshalb zusätzlich eine Zeichengrenze als
/// billigen Stolperdraht fest.
pub const TOKEN_BUDGET: usize = 270;

/// Was unter macOS anders ist, in beiden Sprachen (HUM-377). Es hängt am
/// Backend und nicht am Ask-Modus, steht deshalb nicht in den Vorlagen und
/// zählt nicht zu [`TOKEN_BUDGET`], das die Linux-Einweisung begrenzt.
///
/// `tests/escape/macos/run.sh` prüft `/tmp`, die Maske auf `.git/config` und
/// das verschachtelte `sandbox-exec`, nicht `whoami` und `hostname`, die von
/// Hand gemessen sind. `/tmp` ist unter macOS die echte `/private/tmp`, das Profil verbietet sie, und das Abbilden
/// der Pfade erreicht nur Umgebung, Kommando und Adapter-Dateien, nicht einen
/// fest verdrahteten Pfad im Programm des Agenten.
///
/// Der Punkt direkt hinter `/tmp` ist Absicht: Diese Datei läuft durch
/// [`crate::seatbelt::PathMap::map_text`], und nur so bleibt dort der Name
/// stehen, den ein Agent aus dem Gedächtnis schreibt.
pub const SEATBELT_NOTE_EN: &str = "macOS sandbox (Seatbelt): temporary files go to `$TMPDIR`; a \
     write outside it, `$HOME` and a writable project ends in `EPERM`, and so does /tmp. A masked \
     file answers `EPERM`, not \"not found\"; `.git/config` is one, and `git` itself works. \
     A nested `sandbox-exec` fails. `whoami` and \
     `hostname` show the real machine.";

/// [`SEATBELT_NOTE_EN`] auf Deutsch.
pub const SEATBELT_NOTE_DE: &str = "macOS-Sandbox (Seatbelt): Temporäre Dateien gehören nach \
     `$TMPDIR`; ein Schreibzugriff außerhalb davon, von `$HOME` und einem beschreibbaren Projekt \
     endet mit `EPERM`, auch in /tmp. Eine maskierte Datei antwortet mit `EPERM`, nicht mit \
     „nicht gefunden\"; `.git/config` ist eine, `git` selbst funktioniert. Ein verschachteltes `sandbox-exec` scheitert. `whoami` und `hostname` \
     zeigen die echte Maschine.";

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

    let wanted = variant_for(ask_mode);
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

/// Der Name der Variante, die zu einem Ask-Modus gehört.
///
/// Auch der Skill `humanitl` ([`crate::agent::skill`]) wählt seine Variante
/// hiermit, damit Briefing und Skill nie verschiedene Modi beschreiben.
pub(crate) const fn variant_for(ask_mode: AskMode) -> &'static str {
    match ask_mode {
        AskMode::Ui | AskMode::Terminal => VARIANT_ASK,
        AskMode::None => VARIANT_NONE,
    }
}

/// [`render`] für eine Sitzung auf diesem System: unter macOS mit
/// [`SEATBELT_NOTE_EN`] am Ende.
///
/// # Errors
///
/// Wie [`render`].
pub fn render_for_session(
    language: Language,
    ask_mode: AskMode,
    timeout_secs: u64,
    llm_host: Option<&str>,
) -> Result<String, Diagnostic> {
    let text = render(language, ask_mode, timeout_secs, llm_host)?;
    Ok(if cfg!(target_os = "macos") {
        with_seatbelt_note(&text, language)
    } else {
        text
    })
}

/// Der Text mit dem Absatz für macOS.
fn with_seatbelt_note(text: &str, language: Language) -> String {
    let note = match language {
        Language::En => SEATBELT_NOTE_EN,
        Language::De => SEATBELT_NOTE_DE,
    };
    format!("{text}\n{note}\n")
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
    split_variants_with(template, false)
}

/// Wie [`split_variants`], mit der Wahl, ob die Zeilen einer Variante zu
/// einem Absatz zusammenfließen (`keep_lines = false`, das Briefing) oder als
/// Zeilen stehen bleiben (`true`, der Skill mit Überschrift und Liste je
/// Variante).
pub(crate) fn split_variants_with(
    template: &str,
    keep_lines: bool,
) -> (&str, Vec<(String, String)>) {
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
            if keep_lines {
                text.push_str(line.trim_end());
                text.push('\n');
                continue;
            }
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
pub(crate) fn strip_comments(text: &str) -> String {
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
pub(crate) fn tidy(text: &str) -> String {
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
        PLACEHOLDER_ASK_MODE, PLACEHOLDER_LLM_HOST, SEATBELT_NOTE_DE, SEATBELT_NOTE_EN,
        TEMPLATE_DE, TEMPLATE_EN, render, render_for_session, split_variants, strip_comments, tidy,
        with_seatbelt_note,
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
            (Language::En, "Requests here have high, variable latency"),
            (Language::De, "Anfragen haben hier hohe, schwankende Latenz"),
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
            (Language::En, "It grants nothing by itself."),
            (Language::De, "Er gewährt von sich aus nichts."),
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

    /// Der erste Satz sagt dem Agenten, dass er ins Internet kommt, und womit
    /// (HUM-304). „No network interface" am Anfang las ein Modell als „kein
    /// Internet" und versuchte keinen einzigen Abruf.
    #[test]
    fn the_briefing_opens_with_internet_access_over_the_proxy() {
        for (language, access, tools, proxy, old_opening) in [
            (
                Language::En,
                "You have internet access: HTTP(S)",
                "`curl`, `wget`",
                "through the proxy in the environment, nothing else.",
                "No network interface",
            ),
            (
                Language::De,
                "Du hast Internet: HTTP(S)",
                "`curl`, `wget`",
                "über den Proxy der Umgebung, sonst nichts.",
                "Keine Netzwerkschnittstelle",
            ),
        ] {
            for ask_mode in [AskMode::Ui, AskMode::None] {
                let text = render(language, ask_mode, 300, Some(HOST)).unwrap();
                let opening = text
                    .lines()
                    .find(|line| !line.is_empty() && !line.starts_with('#'))
                    .unwrap();
                assert!(opening.starts_with(access), "{opening}");
                assert!(opening.contains(tools), "{opening}");
                assert!(opening.contains(proxy), "{opening}");
                assert!(!text.contains(old_opening), "{text}");
            }
        }
    }

    /// Ein Werkzeug mit kürzerer Frist als der Halt zieht die Frage zurück,
    /// wenn es aufgibt (HUM-308): `webfetch` von `OpenCode` nach höchstens
    /// 120 Sekunden, der Halt dauert 300. Der Agent erfährt das und welches
    /// Werkzeug wartet. Ohne Frage gibt es nichts zurückzuziehen.
    #[test]
    fn a_tool_that_gives_up_sooner_is_named_together_with_curl() {
        for (language, gives_up, waits) in [
            (
                Language::En,
                "A tool that gives up sooner withdraws the request",
                "`curl` waits.",
            ),
            (
                Language::De,
                "Gibt ein Tool früher auf, ist die Anfrage weg",
                "`curl` wartet.",
            ),
        ] {
            let asked = render(language, AskMode::Ui, 300, Some(HOST)).unwrap();
            assert!(asked.contains(gives_up), "{asked}");
            assert!(asked.contains(waits), "{asked}");
            let unasked = render(language, AskMode::None, 300, Some(HOST)).unwrap();
            assert!(!unasked.contains(gives_up), "{unasked}");
        }
    }

    /// Lange Wartezeit ist kein Ausfall, in beiden Ask-Modi mit dem, was dort
    /// wahr ist (HUM-364): mit Frage Sekunden bis Minuten Warten, ohne
    /// Wiederholung in einer Schleife und ohne Wechsel von Werkzeug oder
    /// Host; ohne Frage wird nichts angehalten, und ein `504` ist eine
    /// fehlende Regel. Das Modell wird nie angehalten, in beiden Modi, und
    /// der Verweis auf den Skill steht immer da.
    #[test]
    fn latency_is_explained_in_both_ask_modes() {
        for (language, asked_lines, unasked_lines, model, skill) in [
            (
                Language::En,
                &[
                    "Requests here have high, variable latency",
                    "no retry loops or parallel duplicates, no switching tools or mirrors",
                ][..],
                &[
                    "nothing waits",
                    "refused requests get `403`",
                    "`504`: not permitted, not an outage",
                ][..],
                "they never wait here, so delays come from the model server.",
                "Details: skill `humanitl`.",
            ),
            (
                Language::De,
                &[
                    "Anfragen haben hier hohe, schwankende Latenz",
                    "keine Schleifen oder parallelen Duplikate, kein Tool- oder Spiegelwechsel",
                ][..],
                &[
                    "nichts wartet",
                    "Abgelehntes bekommt `403`",
                    "`504`: nicht erlaubt, kein Ausfall",
                ][..],
                "sie warten hier nie, Verzögerung kommt vom Modellserver.",
                "Mehr: Skill `humanitl`.",
            ),
        ] {
            let asked = render(language, AskMode::Ui, 300, Some(HOST)).unwrap();
            let unasked = render(language, AskMode::None, 300, Some(HOST)).unwrap();
            for line in asked_lines {
                assert!(asked.contains(line), "{language:?}: {line:?}: {asked}");
                assert!(!unasked.contains(line), "{language:?}: {line:?}: {unasked}");
            }
            for line in unasked_lines {
                assert!(unasked.contains(line), "{language:?}: {line:?}: {unasked}");
            }
            for text in [&asked, &unasked] {
                assert!(text.contains(model), "{language:?}: {text}");
                assert!(text.contains(skill), "{language:?}: {text}");
            }
        }
    }

    #[test]
    fn the_seatbelt_note_follows_the_briefing_and_names_what_differs() {
        for (language, note) in [
            (Language::En, SEATBELT_NOTE_EN),
            (Language::De, SEATBELT_NOTE_DE),
        ] {
            let plain = render(language, AskMode::Ui, 30, None).unwrap();
            let text = with_seatbelt_note(&plain, language);
            assert!(text.starts_with(&plain));
            assert!(text.ends_with(&format!("{note}\n")));
            for fact in ["`$TMPDIR`", "/tmp", "`EPERM`", "`sandbox-exec`", "`whoami`"] {
                assert!(note.contains(fact), "{fact} is missing: {note}");
            }
        }
    }

    #[test]
    fn the_session_briefing_carries_the_note_only_on_macos() {
        let text = render_for_session(Language::En, AskMode::Ui, 30, None).unwrap();
        assert_eq!(text.contains("Seatbelt"), cfg!(target_os = "macos"));
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
