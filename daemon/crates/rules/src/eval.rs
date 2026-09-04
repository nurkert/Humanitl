//! Der Regelsatz und seine Auswertung.
//!
//! Ein [`RuleSet`] ist eine geordnete Liste. Ausgewertet wird in zwei
//! Durchgängen: zuerst die Regeln dieser Sitzung, dann die dauerhaften
//! (`backlog/CONVENTIONS.md` 4.5). Innerhalb jedes Durchgangs gewinnt die erste
//! passende Regel. Der Grund für die zwei Durchgänge steht in ADR-007: was der
//! Mensch gerade entschieden hat, soll sofort gelten, auch wenn eine ältere,
//! breitere Regel in der Datei darüber steht.
//!
//! Passt keine Regel, ist das Ergebnis [`Verdict::Default`], und das heißt
//! `ask`. Es gibt keinen Weg, auf dem eine Anfrage ohne Regel durchgeht.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use humanitl_core::rule::{Action, Expiry, Matcher, Rule};
use humanitl_core::{HostName, Method, RuleId, Scheme, SessionId, Upgrade};

use crate::host;
use crate::path::PathMatcher;

/// Die Methoden, die eine Regel überhaupt treffen kann.
///
/// Eine Anfrage mit einer anderen Methode wird nie automatisch entschieden:
/// Regeln sind über bekannten Methoden geschrieben, und ein `BREW` an einen
/// erlaubten Host ist genau der Fall, den ein Mensch sehen soll.
const KNOWN_METHODS: [Method; 9] = [
    Method::GET,
    Method::HEAD,
    Method::POST,
    Method::PUT,
    Method::PATCH,
    Method::DELETE,
    Method::OPTIONS,
    Method::CONNECT,
    Method::TRACE,
];

/// Wahr, wenn die Methode zur bekannten Menge gehört.
#[must_use]
pub fn is_known_method(method: &Method) -> bool {
    KNOWN_METHODS.iter().any(|known| known == method)
}

/// Die Merkmale einer Anfrage, gegen die eine Regel prüft.
///
/// Der Schlüssel trägt keine Header und keinen Body: eine Regel entscheidet
/// über das Ziel, nie über den Inhalt. Der Inhalt ist Sache der Detektoren
/// (`humanitl-findings`) und des Menschen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestKey<'a> {
    /// Der Ziel-Host, schon normalisiert.
    pub host: &'a HostName,
    /// Die Methode der Anfrage.
    pub method: &'a Method,
    /// Pfad und Query; verglichen wird nur der Pfad.
    pub path: &'a str,
    /// Das Schema der Anfrage.
    pub scheme: Scheme,
    /// Der Port, nie leer (der Standard des Schemas ist eingesetzt).
    pub port: u16,
    /// Ein angefragter Protokollwechsel, falls es einen gibt.
    pub upgrade: Option<Upgrade>,
}

impl<'a> RequestKey<'a> {
    /// Ein Schlüssel ohne Protokollwechsel.
    #[must_use]
    pub const fn new(
        host: &'a HostName,
        method: &'a Method,
        path: &'a str,
        scheme: Scheme,
        port: u16,
    ) -> Self {
        Self {
            host,
            method,
            path,
            scheme,
            port,
            upgrade: None,
        }
    }

    /// Derselbe Schlüssel mit einem angefragten Protokollwechsel.
    #[must_use]
    pub const fn with_upgrade(mut self, upgrade: Upgrade) -> Self {
        self.upgrade = Some(upgrade);
        self
    }
}

/// Das Ergebnis einer Auswertung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Eine Regel hat getroffen.
    Matched {
        /// Welche Regel.
        rule: RuleId,
        /// Was sie sagt.
        action: Action,
    },
    /// Keine Regel hat getroffen; es gilt `ask`.
    Default,
}

impl Verdict {
    /// Die Aktion, die gilt: die der Regel, sonst [`Action::Ask`].
    #[must_use]
    pub const fn action(&self) -> Action {
        match self {
            Self::Matched { action, .. } => *action,
            Self::Default => Action::Ask,
        }
    }

    /// Die Regel, die getroffen hat.
    #[must_use]
    pub const fn rule(&self) -> Option<RuleId> {
        match self {
            Self::Matched { rule, .. } => Some(*rule),
            Self::Default => None,
        }
    }
}

/// Eine Regel mit dieser Id gibt es im Regelsatz nicht.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("no rule with id {id}")]
pub struct UnknownRule {
    /// Die gesuchte Id.
    pub id: RuleId,
}

/// Das übersetzte Pfadmuster einer Regel.
#[derive(Debug, Clone)]
enum CompiledPath {
    /// Die Regel hat kein Pfadmuster; jeder Pfad passt.
    Any,
    /// Das übersetzte Muster.
    Matcher(PathMatcher),
    /// Das Muster ließ sich nicht übersetzen; die Regel trifft nichts.
    ///
    /// Aus `rules.yaml` kommt dieser Zustand nie (`parse_rules` lehnt die Datei
    /// mit `RULES_005` ab). Er entsteht nur, wenn eine Regel im Programm gebaut
    /// und über [`RuleSet::insert`] eingehängt wird. Eine Regel, die nichts
    /// trifft, führt zu [`Verdict::Default`], also zu `ask`: der Fehler kostet
    /// eine Rückfrage, nie eine stille Freigabe.
    Broken,
}

/// Die geprüften Pfadpräfixe einer Regel.
#[derive(Debug, Clone)]
enum CompiledPrefixes {
    /// Die Regel trägt keine Präfixe; jeder Pfad passt.
    Any,
    /// Die brauchbaren Präfixe der Regel; mindestens einer muss passen.
    Some(Vec<String>),
    /// Die Regel trägt Präfixe, aber keinen brauchbaren; sie trifft nichts.
    ///
    /// Fail closed: Eine Präfix-Liste ist eine Grenze. Wer sie schreibt und
    /// dabei nur Unbrauchbares hinschreibt, bekommt eine Regel, die nichts
    /// trifft — nie eine, die alles trifft (HUM-039, Fallstricke). Aus
    /// `rules.yaml` kommt dieser Zustand nicht, `parse_rules` lehnt die Datei
    /// vorher mit `RULES_005` ab; er entsteht nur für eine Regel, die im
    /// Programm gebaut wurde.
    Broken,
}

impl CompiledPrefixes {
    fn new(prefixes: &[String]) -> Self {
        if prefixes.is_empty() {
            return Self::Any;
        }
        let usable: Vec<String> = prefixes
            .iter()
            .filter(|prefix| humanitl_core::path_prefix_is_valid(prefix))
            .cloned()
            .collect();
        if usable.is_empty() {
            Self::Broken
        } else {
            Self::Some(usable)
        }
    }

    fn matches(&self, path_and_query: &str) -> bool {
        match self {
            Self::Any => true,
            Self::Some(prefixes) => crate::path::prefix_matches(prefixes, path_and_query),
            Self::Broken => false,
        }
    }
}

/// Eine Regel samt ihrem übersetzten Pfadmuster.
#[derive(Debug, Clone)]
struct CompiledRule {
    rule: Rule,
    path: CompiledPath,
    prefixes: CompiledPrefixes,
}

impl CompiledRule {
    fn new(rule: Rule) -> Self {
        let path = match rule.matcher.path.as_ref() {
            None => CompiledPath::Any,
            Some(pattern) => match PathMatcher::compile(pattern) {
                Ok(matcher) => CompiledPath::Matcher(matcher),
                Err(_) => CompiledPath::Broken,
            },
        };
        let prefixes = CompiledPrefixes::new(&rule.matcher.path_prefixes);
        Self {
            rule,
            path,
            prefixes,
        }
    }

    fn matches(&self, key: &RequestKey<'_>) -> bool {
        let matcher: &Matcher = &self.rule.matcher;

        // Die Upgrade-Dimension ist beidseitig: eine Regel ohne `upgrade`
        // trifft nie ein Upgrade, eine Regel mit `upgrade` nie eine gewöhnliche
        // Anfrage. Ein WebSocket ist eine andere Sache als ein GET auf
        // denselben Host, und ADR-007 verlangt dafür eine eigene Entscheidung.
        if key.upgrade.is_some() != matcher.upgrade.is_some() {
            return false;
        }
        if let (Some(wanted), Some(actual)) = (matcher.upgrade, key.upgrade)
            && wanted != actual
        {
            return false;
        }
        if !host::matches(&matcher.host, key.host) {
            return false;
        }
        if let Some(methods) = matcher.methods.as_ref()
            && !methods.iter().any(|method| method == key.method)
        {
            return false;
        }
        if let Some(scheme) = matcher.scheme
            && scheme != key.scheme
        {
            return false;
        }
        if let Some(port) = matcher.port
            && port != key.port
        {
            return false;
        }
        // Pfadmuster und Präfixe stehen nebeneinander und schränken beide
        // ein: Wer beides schreibt, meint beides.
        if !self.prefixes.matches(key.path) {
            return false;
        }
        match &self.path {
            CompiledPath::Any => true,
            CompiledPath::Matcher(matcher) => matcher.matches(key.path),
            CompiledPath::Broken => false,
        }
    }
}

/// Der geordnete Regelsatz.
///
/// Erzeugt wird er aus `rules.yaml` über [`crate::parse_rules`] oder leer über
/// [`RuleSet::new`]. Die Reihenfolge ist Teil der Bedeutung: die erste passende
/// Regel gewinnt.
///
/// Daneben führt der Satz die Ids der mitgelieferten Regeln, die der Nutzer
/// abgeschaltet hat ([`RuleSet::disabled_bundled`]). Sie stehen getrennt, weil
/// sie eine Regel benennen können, die erst später dazukommt: Die `rules.yaml`
/// des Nutzers wird gelesen, bevor [`RuleSet::prepend_bundled`] die
/// mitgelieferten Regeln davorstellt.
#[derive(Debug, Clone, Default)]
pub struct RuleSet {
    rules: Vec<CompiledRule>,
    disabled_bundled: BTreeSet<RuleId>,
}

impl PartialEq for RuleSet {
    /// Zwei Regelsätze sind gleich, wenn sie dieselben Regeln in derselben
    /// Reihenfolge und dieselben abgeschalteten Ids tragen. Die übersetzten
    /// Muster sind daraus abgeleitet.
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self.disabled_bundled == other.disabled_bundled
            && self.iter().zip(other.iter()).all(|(a, b)| a == b)
    }
}

impl Eq for RuleSet {}

impl RuleSet {
    /// Ein leerer Regelsatz. Ohne Regeln wird jede Anfrage gehalten.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            rules: Vec::new(),
            disabled_bundled: BTreeSet::new(),
        }
    }

    /// Baut einen Regelsatz aus Regeln in der gegebenen Reihenfolge.
    #[must_use]
    pub fn from_rules(rules: impl IntoIterator<Item = Rule>) -> Self {
        Self {
            rules: rules.into_iter().map(CompiledRule::new).collect(),
            disabled_bundled: BTreeSet::new(),
        }
    }

    /// Die Ids der mitgelieferten Regeln, die der Nutzer abgeschaltet hat.
    ///
    /// Eine Id darin muss keine Regel dieses Satzes benennen: Sie überlebt
    /// eine Fassung, in der es die Regel noch nicht oder nicht mehr gibt,
    /// damit ein Abschalten nicht stillschweigend zurückgeht.
    pub fn disabled_bundled(&self) -> impl Iterator<Item = RuleId> + '_ {
        self.disabled_bundled.iter().copied()
    }

    /// Übernimmt die Liste der abgeschalteten mitgelieferten Regeln.
    ///
    /// Regeln, die schon im Satz stehen und in der Liste vorkommen, werden
    /// sofort abgeschaltet; alles Weitere wirkt in [`RuleSet::prepend_bundled`].
    pub fn set_disabled_bundled(&mut self, ids: impl IntoIterator<Item = RuleId>) {
        self.disabled_bundled = ids.into_iter().collect();
        for compiled in &mut self.rules {
            if self.disabled_bundled.contains(&compiled.rule.id) {
                compiled.rule.disabled = true;
            }
        }
    }

    /// Wertet eine Anfrage aus.
    ///
    /// Reihenfolge, genau so umgesetzt:
    ///
    /// 1. Eine unbekannte Methode führt sofort zu [`Verdict::Default`].
    /// 2. Erster Durchgang über die Regeln dieser Sitzung, zweiter über alle
    ///    übrigen; abgelaufene und abgeschaltete Regeln werden übersprungen.
    /// 3. Innerhalb eines Durchgangs gewinnt die erste passende Regel.
    /// 4. Trifft nichts, gilt [`Verdict::Default`], also `ask`.
    ///
    /// Eine abgeschaltete Regel (`disabled`) zählt wie eine, die es nicht gibt.
    /// Sie bleibt im Satz stehen, damit der Rules-Screen sie samt Begründung
    /// zeigen kann; entscheiden darf sie nichts (HUM-038).
    #[must_use]
    pub fn evaluate(
        &self,
        key: &RequestKey<'_>,
        now: DateTime<Utc>,
        session: SessionId,
    ) -> Verdict {
        if !is_known_method(key.method) {
            return Verdict::Default;
        }

        for session_scoped in [true, false] {
            for compiled in &self.rules {
                if matches!(compiled.rule.expires, Expiry::Session(_)) != session_scoped {
                    continue;
                }
                if compiled.rule.disabled || compiled.rule.is_expired(now, session) {
                    continue;
                }
                if compiled.matches(key) {
                    return Verdict::Matched {
                        rule: compiled.rule.id,
                        action: compiled.rule.action,
                    };
                }
            }
        }

        Verdict::Default
    }

    /// Stellt mitgelieferte Regeln vor alle vorhandenen.
    ///
    /// `bundled` wird dabei erzwungen: Was auf diesem Weg hereinkommt, stammt
    /// aus dem Adapter oder aus `rules/default.yaml` und gehört nicht dem
    /// Nutzer, auch wenn die Datei es anders behauptet. Die Reihenfolge der
    /// übergebenen Regeln bleibt erhalten, und die Regeln des Nutzers rücken
    /// dahinter (`backlog/CONVENTIONS.md` 4.5, HUM-038).
    ///
    /// `disabled` kommt aus der Liste `disabled_bundled` dieses Satzes, also aus
    /// der `rules.yaml` des Nutzers. So wirkt ein Abschalten auch dann, wenn
    /// die Datei gelesen wurde, bevor es die Regel im Satz gab.
    pub fn prepend_bundled(&mut self, rules: impl IntoIterator<Item = Rule>) {
        let disabled = &self.disabled_bundled;
        let bundled: Vec<CompiledRule> = rules
            .into_iter()
            .map(|rule| {
                let off = rule.disabled || disabled.contains(&rule.id);
                CompiledRule::new(rule.bundled(true).disabled(off))
            })
            .collect();
        self.rules.splice(0..0, bundled);
    }

    /// Schaltet die Regel mit dieser Id ab oder wieder an.
    ///
    /// # Errors
    ///
    /// [`UnknownRule`], wenn es keine Regel mit dieser Id gibt.
    pub fn set_disabled(&mut self, id: RuleId, disabled: bool) -> Result<(), UnknownRule> {
        let at = self.position(id).ok_or(UnknownRule { id })?;
        let Some(compiled) = self.rules.get_mut(at) else {
            return Err(UnknownRule { id });
        };
        compiled.rule.disabled = disabled;
        Ok(())
    }

    /// Hängt eine Regel ein. `None` heißt: ans Ende.
    ///
    /// Eine Position hinter dem Ende wird auf das Ende geklemmt.
    pub fn insert(&mut self, pos: Option<usize>, rule: Rule) -> RuleId {
        let id = rule.id;
        let at = pos.unwrap_or(self.rules.len()).min(self.rules.len());
        self.rules.insert(at, CompiledRule::new(rule));
        id
    }

    /// Nimmt die Regel mit dieser Id heraus.
    pub fn remove(&mut self, id: RuleId) -> Option<Rule> {
        let at = self.position(id)?;
        Some(self.rules.remove(at).rule)
    }

    /// Ersetzt eine Regel an ihrem Platz.
    ///
    /// # Errors
    ///
    /// [`UnknownRule`], wenn keine Regel diese Id trägt.
    pub fn update(&mut self, rule: Rule) -> Result<(), UnknownRule> {
        let at = self.position(rule.id).ok_or(UnknownRule { id: rule.id })?;
        self.rules[at] = CompiledRule::new(rule);
        Ok(())
    }

    /// Verschiebt eine Regel an eine neue Position.
    ///
    /// Eine Position hinter dem Ende wird auf das Ende geklemmt.
    ///
    /// # Errors
    ///
    /// [`UnknownRule`], wenn keine Regel diese Id trägt.
    pub fn reorder(&mut self, id: RuleId, new_pos: usize) -> Result<(), UnknownRule> {
        let at = self.position(id).ok_or(UnknownRule { id })?;
        let rule = self.rules.remove(at);
        let target = new_pos.min(self.rules.len());
        self.rules.insert(target, rule);
        Ok(())
    }

    /// Entfernt alle abgelaufenen Regeln und meldet ihre Ids.
    ///
    /// Abgelaufen ist eine Regel mit einem Zeitpunkt in der Vergangenheit und
    /// jede Sitzungsregel einer anderen Sitzung.
    pub fn prune(&mut self, now: DateTime<Utc>, session: SessionId) -> Vec<RuleId> {
        let mut removed = Vec::new();
        self.rules.retain(|compiled| {
            if compiled.rule.is_expired(now, session) {
                removed.push(compiled.rule.id);
                false
            } else {
                true
            }
        });
        removed
    }

    /// Die Regeln in ihrer Reihenfolge.
    pub fn iter(&self) -> impl Iterator<Item = &Rule> {
        self.rules.iter().map(|compiled| &compiled.rule)
    }

    /// Die Regel mit dieser Id.
    #[must_use]
    pub fn get(&self, id: RuleId) -> Option<&Rule> {
        self.rules
            .iter()
            .find(|compiled| compiled.rule.id == id)
            .map(|compiled| &compiled.rule)
    }

    /// Wahr, wenn die Regel mit dieser Id die Durchreiche zum Sprachmodell ist.
    ///
    /// Eine Id, die es im Satz nicht gibt, ist `false`: Was der Regelsatz nicht
    /// kennt, reicht er auch nicht durch. Die Frage steht hier und nicht am
    /// [`Verdict`], weil zwischen Auswertung und Abfrage ein Neuladen liegen
    /// kann; die Antwort kommt dann aus dem Satz, der gerade gilt, und im
    /// Zweifel zur sicheren Seite.
    #[must_use]
    pub fn is_passthrough_llm(&self, id: RuleId) -> bool {
        self.get(id).is_some_and(|rule| rule.passthrough_llm)
    }

    /// Die Anzahl der Regeln.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// Wahr, wenn der Regelsatz keine Regel enthält.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    fn position(&self, id: RuleId) -> Option<usize> {
        self.rules
            .iter()
            .position(|compiled| compiled.rule.id == id)
    }
}

impl<'a> IntoIterator for &'a RuleSet {
    type Item = &'a Rule;
    type IntoIter = std::vec::IntoIter<&'a Rule>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter().collect::<Vec<_>>().into_iter()
    }
}
