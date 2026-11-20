//! Der Einstellungskatalog: Kategorien, Reihenfolge, Texte und Bedienart
//! jeder Einstellung (HUM-534, ADR-0024 Punkt 3).
//!
//! Der Katalog ist die eine Quelle, aus der die App (HUM-535) und die
//! Einstellungs-TUI (HUM-541) zeichnen; kein Client hält eine eigene Liste.
//! Er liegt in dieser Crate, weil sie die eine Stelle ist, die ein Rust-Client
//! und die App lesen können, ohne dass eine Seite aus der anderen erzeugt wird.
//!
//! Aufbau:
//!
//! - [`CATEGORIES`] und [`DEFS`] tragen die Struktur: Kennung, Kategorie,
//!   Reihenfolge, Bedienart, ob ein Wert erst in der nächsten Sitzung gilt, und
//!   ob nur die App die Zeile zeigt ([`Client::App`]).
//! - Titel, Hilfe, Erklärung, Stichwörter und die Namen der Auswahlwerte
//!   stehen je Sprache in `catalog/en.toml` und `catalog/de.toml` neben dieser
//!   Datei und werden beim Bau eingebettet. `en` ist die Quelle, `de` die
//!   Übersetzung; fehlt ein Text in `de`, gilt der englische.
//! - `catalog/hidden.toml` nennt jedes Blatt des Schemas, das keine Zeile
//!   hat, mit dem Grund ([`hidden_reason`]). Der Test `tests/catalog.rs` hält Schema, Katalog und Liste
//!   gegeneinander: Ein neues Feld ohne Eintrag und ohne Grund färbt ihn rot.
//!
//! Herkunft, Sperre und ob ein Schlüssel auf der Erlaubnisliste der
//! Projektwahl steht, hängen von der Anfrage ab und kommen erst im Daemon
//! dazu (`GetConfig.include_catalog`).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::model::Language;
use crate::schema;
use crate::tier::Tier;

/// Wie eine Zeile bedient wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    /// Ein Schalter, an oder aus.
    Switch,
    /// Eine Auswahl aus [`Entry::choices`].
    Choice,
    /// Freier Text; der Daemon prüft ihn.
    Text,
    /// Eine ganze Zahl; Bereich und Prüfung im Daemon.
    Number,
    /// Ein Verweis auf ein Geheimnis. Der Wert selbst geht nie durch
    /// `SetConfig`, sondern über die RPC der Zeile (`LlmKey`, `Egress`), und
    /// kein Client zeigt ihn je an.
    SecretRef,
    /// Ein Pfad auf dem Host.
    Path,
    /// Eine Zeile mit eigenem Bedienelement, das über seine eigene RPC
    /// spricht (Server, Backends, Neustart); der Client ordnet es über
    /// [`Entry::id`] zu.
    Special,
}

impl Control {
    /// Der Name auf der Leitung.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Switch => "switch",
            Self::Choice => "choice",
            Self::Text => "text",
            Self::Number => "number",
            Self::SecretRef => "secret_ref",
            Self::Path => "path",
            Self::Special => "special",
        }
    }
}

/// Welcher Client eine Zeile zeigt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Client {
    /// Jeder Client: App und TUI.
    All,
    /// Nur die App: eine Einstellung des App-Fensters selbst, etwa die
    /// Skalierung, die die App lokal hält (HUM-541 Punkt 3).
    App,
}

impl Client {
    /// Der Name auf der Leitung; leer für jeden Client.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::All => "",
            Self::App => "app",
        }
    }
}

/// Die Kategorien in der Reihenfolge der linken Spalte.
pub const CATEGORIES: [&str; 10] = [
    "models",
    "agent",
    "agents",
    "sandbox",
    "requests",
    "interface",
    "shortcuts",
    "network",
    "egress",
    "daemon",
];

/// Die Struktur eines Eintrags; die Texte kommen aus den Sprachdateien.
#[derive(Debug, Clone, Copy)]
pub struct Def {
    /// Der Schlüssel der Konfiguration oder die Kennung einer besonderen
    /// Zeile ohne eigenen Schlüssel (`daemon.restart`).
    pub id: &'static str,
    /// Die Kategorie, eine aus [`CATEGORIES`].
    pub category: &'static str,
    /// Die Bedienart.
    pub control: Control,
    /// Die Werte einer Auswahl, wo das Schema keine Aufzählung hat (die
    /// Adapter von `agent.adapter` kennt nur die Sandbox-Crate). Leer heißt:
    /// die Werte der Aufzählung des Schemas.
    pub choices: &'static [&'static str],
    /// Ob eine laufende Sitzung den alten Wert behält.
    pub next_session: bool,
    /// Wer die Zeile zeigt.
    pub client: Client,
    /// Die Stufe einer Zeile ohne Schlüssel; für einen Schlüssel gilt
    /// `x-tier` des Schemas.
    pub tier: Tier,
}

impl Def {
    const fn key(id: &'static str, category: &'static str, control: Control) -> Self {
        Self {
            id,
            category,
            control,
            choices: &[],
            next_session: false,
            client: Client::All,
            tier: Tier::Basic,
        }
    }

    const fn next(mut self) -> Self {
        self.next_session = true;
        self
    }

    const fn app(mut self) -> Self {
        self.client = Client::App;
        self
    }

    const fn advanced(mut self) -> Self {
        self.tier = Tier::Advanced;
        self
    }

    const fn with_choices(mut self, choices: &'static [&'static str]) -> Self {
        self.choices = choices;
        self
    }

    /// Ob die Zeile ein Schlüssel der Konfiguration ist.
    #[must_use]
    pub fn is_key(&self) -> bool {
        schema::field(self.id).is_some()
    }
}

/// Die Adapter, die `agent.adapter` anbietet. Der Test der IPC-Crate hält sie
/// gegen das Register der Sandbox-Crate.
pub const AGENT_ADAPTERS: &[&str] = &["opencode", "pi", "claude"];

/// Jeder Eintrag, je Kategorie in der Reihenfolge seiner Seite.
pub const DEFS: &[Def] = &[
    // Modelle (HUM-405): Karten und Detailansicht der Server zeichnet die
    // Zeile `llm.servers`; Endpunkt, Modell und Schlüssel sind Zeilen der
    // Detailansicht.
    Def::key("llm.servers", "models", Control::Special),
    Def::key("llm.endpoint", "models", Control::Text).next(),
    Def::key("llm.models", "models", Control::Text).next(),
    Def::key("llm.api_key_ref", "models", Control::SecretRef).next(),
    Def::key("llm.server", "models", Control::Special),
    Def::key("agent.adapter", "agent", Control::Choice)
        .next()
        .with_choices(AGENT_ADAPTERS),
    // Das Agent-Depot je Agent (HUM-521): Quelle, Version, Anmeldung und
    // Wartungsläufe über `Agents`; die Schlüssel `agents.*` zeichnet die
    // Zeile selbst, darum stehen sie in `catalog/hidden.toml`.
    Def::key("agents.depot", "agents", Control::Special),
    Def::key("sandbox.work_dir", "sandbox", Control::Path).next(),
    Def::key("sandbox.mode", "sandbox", Control::Choice).next(),
    // Welche Backends es gibt und welche wählbar sind, sagt
    // `SandboxBackends`.
    Def::key("sandbox.backend", "sandbox", Control::Special).next(),
    Def::key("hold.timeout_secs", "requests", Control::Number),
    Def::key("hold.ask_mode", "requests", Control::Choice),
    Def::key("ui.language", "interface", Control::Choice),
    Def::key("ui.theme", "interface", Control::Choice),
    Def::key("ui.scale", "interface", Control::Special).app(),
    Def::key("ui.notifications", "interface", Control::Switch),
    // Die Tastenkürzel der App, nur zum Lesen (HUM-420); die Liste der
    // Kürzel ist Sache der App, nicht eine Einstellung.
    Def::key("shortcuts", "shortcuts", Control::Special).app(),
    Def::key("tls.trust_system_store", "network", Control::Switch),
    Def::key("egress.proxy", "egress", Control::Text),
    Def::key("egress.proxy_ca", "egress", Control::Path),
    Def::key("egress.proxy_resolve", "egress", Control::Choice),
    // Die Zugangsdaten gehen nur über `Egress`, nie über `SetConfig`.
    Def::key("egress.proxy_auth_ref", "egress", Control::SecretRef),
    Def::key("egress.check", "egress", Control::Special),
    Def::key("daemon.version", "daemon", Control::Special),
    Def::key("daemon.restart", "daemon", Control::Special).advanced(),
    Def::key("update.check", "daemon", Control::Switch),
    Def::key("daemon.config_file", "daemon", Control::Special).advanced(),
];

/// Die Datei der Blätter ohne Zeile, wie sie eingebettet ist.
pub const HIDDEN_TOML: &str = include_str!("../catalog/hidden.toml");

/// Die Blätter des Schemas ohne Zeile, aus `catalog/hidden.toml`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hidden {
    /// Die Gründe, je Kennung ein Satz.
    #[serde(default)]
    pub reasons: BTreeMap<String, String>,
    /// Je Blatt die Kennung seines Grundes.
    #[serde(default)]
    pub hidden: BTreeMap<String, String>,
}

/// Liest `catalog/hidden.toml`.
///
/// # Errors
///
/// Die Meldung des TOML-Lesers; der Test `tests/catalog.rs` liest die Datei,
/// im Betrieb kann das nicht mehr scheitern.
pub fn parse_hidden() -> Result<Hidden, toml::de::Error> {
    toml::from_str(HIDDEN_TOML)
}

fn hidden() -> &'static Hidden {
    static HIDDEN: OnceLock<Hidden> = OnceLock::new();
    HIDDEN.get_or_init(|| parse_hidden().unwrap_or_default())
}

/// Der Grund, aus dem ein Blatt des Schemas keine Zeile hat, oder `None`
/// für ein Blatt, das eine haben muss. Jedes Blatt steht einzeln in
/// `catalog/hidden.toml`; die Liste ist Daten, damit die Felder der
/// Sandbox-Backends dort genannt werden, ohne Rust-Code an ein Backend zu
/// koppeln (`tools/check_coupling.py`).
#[must_use]
pub fn hidden_reason(path: &str) -> Option<&'static str> {
    let hidden = hidden();
    hidden
        .hidden
        .get(path)
        .and_then(|reason| hidden.reasons.get(reason))
        .map(String::as_str)
}

/// Die Pfade aller Blätter ohne Zeile.
#[must_use]
pub fn hidden_paths() -> Vec<&'static str> {
    hidden().hidden.keys().map(String::as_str).collect()
}

/// Eine Kategorie, wie ein Client sie zeigt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Category {
    /// Die Kennung, eine aus [`CATEGORIES`].
    pub id: &'static str,
    /// Die Stelle in der linken Spalte, ab 0.
    pub order: u32,
    /// Der Name.
    pub title: String,
}

/// Ein Wert einer Auswahl mit seinem Namen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    /// Der Wert, wie `SetConfig` ihn nimmt.
    pub value: String,
    /// Der Name.
    pub title: String,
}

/// Ein Eintrag, wie ein Client ihn zeigt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Schlüssel oder Kennung der Zeile.
    pub id: &'static str,
    /// Die Kategorie.
    pub category: &'static str,
    /// Die Stelle in der Kategorie, ab 0.
    pub order: u32,
    /// Der Name.
    pub title: String,
    /// Ein kurzer Satz unter dem Namen; leer, wenn es keinen gibt.
    pub help: String,
    /// Die längere Erklärung hinter dem Info-Knopf; leer, wenn es keine gibt.
    pub details: String,
    /// Wörter, die den Eintrag in der Suche auch finden; nie gezeigt.
    pub keywords: Vec<String>,
    /// Die Bedienart.
    pub control: Control,
    /// Die Werte einer Auswahl mit ihren Namen.
    pub choices: Vec<Choice>,
    /// Ob eine laufende Sitzung den alten Wert behält.
    pub next_session: bool,
    /// Die Stufe.
    pub tier: Tier,
    /// Wer die Zeile zeigt.
    pub client: Client,
}

/// Der Katalog in einer Sprache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog {
    /// Die Sprache der Texte.
    pub language: Language,
    /// Die Kategorien in der Reihenfolge der linken Spalte.
    pub categories: Vec<Category>,
    /// Die Einträge, je Kategorie in der Reihenfolge ihrer Seite.
    pub entries: Vec<Entry>,
}

/// Die Texte eines Eintrags in einer Sprachdatei.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntryTexts {
    /// Der Name.
    #[serde(default)]
    pub title: String,
    /// Der kurze Satz.
    #[serde(default)]
    pub help: String,
    /// Die längere Erklärung.
    #[serde(default)]
    pub details: String,
    /// Der kurze Satz auf macOS, wo er dort anders lautet (der Agent arbeitet
    /// unter dem echten Pfad statt unter `/work`); leer heißt: wie `help`.
    #[serde(default)]
    pub help_macos: String,
    /// Die Stichwörter.
    #[serde(default)]
    pub keywords: Vec<String>,
}

/// Eine Sprachdatei des Katalogs.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Texts {
    /// Die Namen der Kategorien.
    #[serde(default)]
    pub categories: BTreeMap<String, String>,
    /// Die Texte je Eintrag.
    #[serde(default)]
    pub entries: BTreeMap<String, EntryTexts>,
    /// Die Namen der Auswahlwerte je Eintrag.
    #[serde(default)]
    pub choices: BTreeMap<String, BTreeMap<String, String>>,
}

/// Die englische Sprachdatei, wie sie eingebettet ist.
pub const EN_TOML: &str = include_str!("../catalog/en.toml");
/// Die deutsche Sprachdatei, wie sie eingebettet ist.
pub const DE_TOML: &str = include_str!("../catalog/de.toml");

/// Die Texte einer Sprache.
///
/// # Errors
///
/// Die Meldung des TOML-Lesers, wenn die eingebettete Datei kein gültiger
/// Katalog ist. Der Test `tests/catalog.rs` liest beide Dateien; im Betrieb
/// kann das nicht mehr scheitern.
pub fn parse_texts(language: Language) -> Result<Texts, toml::de::Error> {
    toml::from_str(match language {
        Language::En => EN_TOML,
        Language::De => DE_TOML,
    })
}

fn texts(language: Language) -> &'static Texts {
    static EN: OnceLock<Texts> = OnceLock::new();
    static DE: OnceLock<Texts> = OnceLock::new();
    let cell = match language {
        Language::En => &EN,
        Language::De => &DE,
    };
    // Ein Fehler hier hat der Test längst gemeldet; ohne Texte bleibt die
    // Kennung als Name stehen, statt dass der Daemon fällt.
    cell.get_or_init(|| parse_texts(language).unwrap_or_default())
}

/// Die Werte einer Auswahl: die eigenen des Eintrags oder die Aufzählung des
/// Schemas.
#[must_use]
pub fn choice_values(def: &Def) -> Vec<String> {
    if !def.choices.is_empty() {
        return def
            .choices
            .iter()
            .map(|value| (*value).to_owned())
            .collect();
    }
    if def.control != Control::Choice {
        return Vec::new();
    }
    schema::field(def.id)
        .and_then(|field| field.allowed.clone())
        .unwrap_or_default()
}

/// Die Stufe eines Eintrags: `x-tier` des Schemas oder die der Zeile.
#[must_use]
pub fn tier_of(def: &Def) -> Tier {
    schema::field(def.id).map_or(def.tier, |field| field.tier)
}

/// Der Katalog in `language` für das System, auf dem der Daemon läuft.
#[must_use]
pub fn catalog(language: Language) -> Catalog {
    catalog_for(language, cfg!(target_os = "macos"))
}

/// Der Katalog in `language`, mit den Texten für macOS, wenn `macos` gesetzt
/// ist. Die Vorlage des Fakes der App entsteht immer ohne, damit sie auf
/// jedem System gleich ist.
#[must_use]
pub fn catalog_for(language: Language, macos: bool) -> Catalog {
    let own = texts(language);
    let source = texts(Language::En);
    let categories = CATEGORIES
        .iter()
        .zip(0_u32..)
        .map(|(id, order)| Category {
            id,
            order,
            title: own
                .categories
                .get(*id)
                .or_else(|| source.categories.get(*id))
                .cloned()
                .unwrap_or_else(|| (*id).to_owned()),
        })
        .collect();
    let mut next_order: BTreeMap<&str, u32> = BTreeMap::new();
    let entries = DEFS
        .iter()
        .map(|def| {
            let order = next_order.entry(def.category).or_insert(0);
            let this = *order;
            *order = order.saturating_add(1);
            let mine = own.entries.get(def.id);
            let english = source.entries.get(def.id);
            let pick = |of: fn(&EntryTexts) -> &String| {
                mine.map(of)
                    .filter(|text| !text.is_empty())
                    .or_else(|| english.map(of))
                    .cloned()
                    .unwrap_or_default()
            };
            let keywords = mine
                .map(|texts| texts.keywords.clone())
                .filter(|words| !words.is_empty())
                .or_else(|| english.map(|texts| texts.keywords.clone()))
                .unwrap_or_default();
            let title = pick(|texts| &texts.title);
            Entry {
                id: def.id,
                category: def.category,
                order: this,
                title: if title.is_empty() {
                    def.id.to_owned()
                } else {
                    title
                },
                help: if macos && !pick(|texts| &texts.help_macos).is_empty() {
                    pick(|texts| &texts.help_macos)
                } else {
                    pick(|texts| &texts.help)
                },
                details: pick(|texts| &texts.details),
                keywords,
                control: def.control,
                choices: choice_values(def)
                    .into_iter()
                    .map(|value| Choice {
                        title: choice_title(own, source, def.id, &value),
                        value,
                    })
                    .collect(),
                next_session: def.next_session,
                tier: tier_of(def),
                client: def.client,
            }
        })
        .collect();
    Catalog {
        language,
        categories,
        entries,
    }
}

fn choice_title(own: &Texts, source: &Texts, id: &str, value: &str) -> String {
    [own, source]
        .iter()
        .find_map(|texts| texts.choices.get(id).and_then(|names| names.get(value)))
        .cloned()
        .unwrap_or_else(|| value.to_owned())
}

/// Die Sprache aus einem Text der Anfrage oder aus `ui.language`: `de` und
/// `de-…` sind Deutsch, alles andere Englisch.
#[must_use]
pub fn language_of(asked: &str, configured: Language) -> Language {
    let asked = asked.trim().to_ascii_lowercase();
    if asked.is_empty() {
        return configured;
    }
    if asked == "de" || asked.starts_with("de-") || asked.starts_with("de_") {
        Language::De
    } else {
        Language::En
    }
}
