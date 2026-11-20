//! Regeln der `rules.yaml` des Nutzers, die sich nicht lesen ließen (HUM-561).
//!
//! Bis HUM-561 lehnte ein einziger Fehler die ganze Datei ab, und der Daemon
//! startete ohne eine einzige Regel des Nutzers. Jetzt fällt nur die Regel
//! heraus, die sich nicht lesen lässt; die übrigen gelten weiter. Drei Dinge
//! hängen daran, und sie stehen hier an einer Stelle.
//!
//! # 1. Was an ihrem Platz gilt
//!
//! Eine Regel weglassen heißt nicht nur, dass ihre eigene Entscheidung fehlt.
//! Die erste passende Regel gewinnt, also deckt jede Regel die unter ihr ab.
//! Fällt eine `block`-Regel für `uploads.example.com` weg, entscheidet für
//! diesen Host die nächste passende Regel darunter, und wenn das ein
//! `allow` für `**.example.com` ist, geht hinaus, was der Nutzer gesperrt hat.
//! Dasselbe gilt für `ask` (die Nachfrage fiele weg) und `redact` (die
//! Ersetzung fiele weg), auch gegenüber den mitgelieferten Regeln, die hinter
//! allen Regeln des Nutzers stehen. Nur eine `allow`-Regel lässt sich
//! gefahrlos weglassen: was sie freigab, entscheidet dann eine Regel darunter
//! oder der Mensch, und keine von beiden gibt mehr frei, als dort steht.
//!
//! Deshalb steht an der Stelle jeder unlesbaren Regel, die nicht erkennbar
//! `allow` ist, ein Ersatz ([`UnreadableRule::fences`]), der nie mehr erlaubt
//! als die Regel selbst:
//!
//! - **Aktion und Bedingung lesbar** (der Fehler steckt etwa in `expires`,
//!   `id` oder `created_from`): Der Ersatz ist die Regel selbst mit genau
//!   ihrer Bedingung und ihrer Aktion, ohne Ende. Eine Sperre bleibt also eine
//!   Sperre (fail closed), und sie wirkt eher zu lange als zu kurz.
//! - **Bedingung oder Aktion unlesbar**: Der Ersatz ist `ask` für den Host der
//!   Regel, ohne die übrigen Bedingungen, oder, wenn auch der Host nicht
//!   lesbar ist, für jeden Host und jede Adresse. Er trifft damit alles, was
//!   die Regel getroffen haben kann, und noch mehr. Gesperrt wird dabei
//!   nichts, was der Nutzer nicht gesperrt hat: Eine zu breite Sperre hielte
//!   ohne Grund Verkehr auf, der nie gemeint war, und `ask` gibt die
//!   Entscheidung dem Menschen, der sie ohnehin hat, wenn keine Regel passt.
//!
//! Der `ask`-Ersatz trifft gewöhnliche Anfragen und WebSocket-Upgrades
//! gleichermaßen; die Upgrade-Dimension ist die eine, die eine Regel ohne
//! `upgrade` nie trifft. Steht `expires: session` lesbar da, gilt der Ersatz
//! als Sitzungsregel und behält deren Rang vor allen dauerhaften Regeln;
//! sonst gilt er ohne Ende bis zur Reparatur.
//!
//! Der Ersatz gilt nur im Speicher. Er steht nicht in der Liste der Regeln
//! (`RulesStore::list`), wird nie geschrieben und verschwindet, sobald die
//! Regel wieder lesbar ist. Weil er unsichtbar ist, darf keine Änderung eine
//! Regel über ihn heben ([`lifted_above`]): Der Speicher lehnt sie ab, bis
//! die Regel repariert ist.
//!
//! # 2. Was in der Datei bleibt
//!
//! Der Eintrag bleibt, wie er gelesen wurde ([`UnreadableRule::entry`]), und
//! wird bei jedem Schreiben hinter die Regel zurückgeschrieben, hinter der er
//! stand ([`UnreadableRule::after`], [`reanchor`]). Eine Sicherungskopie
//! daneben wäre die schwächere Antwort: Der Mensch fände seine Regel nicht
//! mehr dort, wo er sie geschrieben hat, und müsste wissen, dass es die Kopie
//! gibt.
//!
//! # 3. Was der Mensch erfährt
//!
//! Je unlesbarer Regel genau ein Befund ([`UnreadableRule::diagnostic`]): Er
//! nennt Regel, Feld, Zeile und Grund und sagt, was bis zur Reparatur an
//! ihrem Platz gilt. Er trägt die Schwere [`Severity::Warning`] und den Code
//! des ersten Fehlers der Regel: Die Datei ist gelesen und gilt, und ein
//! Fehler in einer Antwort hieße für Kommandozeile und Oberfläche, dass der
//! Aufruf nichts bewirkt hat. So steht er in jeder Liste der Regeln, ohne sie
//! zu verhindern.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use humanitl_core::diagnostics::codes::RULES_001;
use humanitl_core::rule::{Action, Expiry, HostPattern, Matcher, Rule};
use humanitl_core::{Diagnostic, FixAction, RuleId, SessionId, Severity, Upgrade};
use serde_yaml_ng::Value;

/// Was sich aus einem unlesbaren Eintrag noch lesen ließ.
#[derive(Debug, Clone, Default)]
pub(crate) struct Readable {
    /// Die Aktion, wenn sie eine der bekannten ist.
    pub(crate) action: Option<Action>,
    /// Die ganze Bedingung, wenn jedes ihrer Felder durchging.
    pub(crate) matcher: Option<Matcher>,
    /// Das Host-Muster allein, wenn es durchging.
    pub(crate) host: Option<HostPattern>,
    /// Die Sitzung, wenn `expires: session` lesbar dasteht.
    pub(crate) session: Option<SessionId>,
}

/// Was bis zur Reparatur an der Stelle der Regel gilt.
#[derive(Debug, Clone, PartialEq)]
enum Stand {
    /// Eine `allow`-Regel: Sie fehlt einfach.
    LeftOut,
    /// Aktion und Bedingung waren lesbar und gelten weiter.
    Kept(Action),
    /// `ask` für diesen Host, oder für jeden, wenn er nicht lesbar war.
    Held(Option<HostPattern>),
}

/// Eine Regel der `rules.yaml`, die sich nicht lesen ließ.
#[derive(Debug, Clone, PartialEq)]
pub struct UnreadableRule {
    index: usize,
    line: Option<usize>,
    after: Option<RuleId>,
    entry: Value,
    fences: Vec<Rule>,
    diagnostic: Diagnostic,
}

impl UnreadableRule {
    /// Baut den Eintrag samt Ersatz und Befund.
    ///
    /// `errors` sind die Befunde, die beim Lesen der Regel auffielen; sie
    /// gehen in den einen Befund dieser Regel ein.
    pub(crate) fn new(
        index: usize,
        line: Option<usize>,
        after: Option<RuleId>,
        entry: Value,
        errors: &[Diagnostic],
        readable: &Readable,
    ) -> Self {
        let stand = stand_of(readable);
        let fences = fences_for(&stand, readable, index, line);
        let diagnostic = diagnostic_for(&stand, errors, index, line);
        Self {
            index,
            line,
            after,
            entry,
            fences,
            diagnostic,
        }
    }

    /// Der Platz der Regel in der Liste `rules`, wie sie gelesen wurde.
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Die 1-basierte Zeile, in der die Regel beginnt, soweit bekannt.
    #[must_use]
    pub const fn line(&self) -> Option<usize> {
        self.line
    }

    /// Die lesbare Regel, hinter der der Eintrag steht; `None` heißt: vor allen.
    #[must_use]
    pub const fn after(&self) -> Option<RuleId> {
        self.after
    }

    /// Der Eintrag, wie er in der Datei stand.
    #[must_use]
    pub const fn entry(&self) -> &Value {
        &self.entry
    }

    /// Was an seiner Stelle gilt, bis er lesbar ist; leer für eine
    /// `allow`-Regel. Siehe das Modul.
    #[must_use]
    pub fn fences(&self) -> &[Rule] {
        &self.fences
    }

    /// Der eine Befund dieser Regel: Zeile, Grund und was an ihrem Platz gilt.
    #[must_use]
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }
}

/// Die Entscheidung aus dem Modulkopf, Abschnitt 1.
fn stand_of(readable: &Readable) -> Stand {
    match (readable.action, &readable.matcher) {
        (Some(Action::Allow), _) => Stand::LeftOut,
        (Some(action), Some(_)) => Stand::Kept(action),
        _ => Stand::Held(readable.host.clone()),
    }
}

fn fences_for(stand: &Stand, readable: &Readable, index: usize, line: Option<usize>) -> Vec<Rule> {
    let note = format!(
        "stands in for {} of rules.yaml, which could not be read",
        place(index, line)
    );
    // Eine Sitzungsregel behält ihren Rang vor den dauerhaften Regeln; jede
    // andere gilt bis zur Reparatur, auch wenn ihr Ende lesbar war.
    let expires = readable.session.map_or(Expiry::Never, Expiry::Session);
    let fence = |action: Action, matcher: Matcher| {
        Rule::new(RuleId::new(), action, matcher)
            .with_expiry(expires)
            .with_note(note.clone())
    };
    // Die Upgrade-Dimension ist die eine, die beidseitig gilt: Eine Regel
    // ohne `upgrade` trifft nie ein Upgrade (`eval.rs`). Methode, Schema und
    // Port fehlen im Ersatz und treffen damit jeden Wert. Ein Ersatz je Host
    // also für gewöhnliche Anfragen und je Art von Upgrade.
    let held = |host: HostPattern| {
        [None, Some(Upgrade::WebSocket)].map(|upgrade| {
            let mut matcher = Matcher::host(host.clone());
            matcher.upgrade = upgrade;
            fence(Action::Ask, matcher)
        })
    };
    match stand {
        Stand::LeftOut => Vec::new(),
        Stand::Kept(action) => readable
            .matcher
            .clone()
            .map(|matcher| vec![fence(*action, matcher)])
            .unwrap_or_default(),
        Stand::Held(Some(host)) => held(host.clone()).to_vec(),
        Stand::Held(None) => every_host().into_iter().flat_map(held).collect(),
    }
}

/// Muster, die zusammen jedes Ziel treffen: jeden Namen, jede IPv4- und jede
/// IPv6-Adresse. Ein Glob allein träfe keine Adresse (`docs/SECURITY.md` 7).
fn every_host() -> [HostPattern; 3] {
    [
        HostPattern::Glob("**".to_owned()),
        HostPattern::Cidr {
            addr: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            prefix: 0,
        },
        HostPattern::Cidr {
            addr: IpAddr::V6(Ipv6Addr::UNSPECIFIED),
            prefix: 0,
        },
    ]
}

fn diagnostic_for(
    stand: &Stand,
    errors: &[Diagnostic],
    index: usize,
    line: Option<usize>,
) -> Diagnostic {
    let code = errors.first().map_or(RULES_001, |error| error.code);
    let reasons = if errors.is_empty() {
        format!("{} could not be read", place(index, line))
    } else {
        errors
            .iter()
            .map(|error| error.why.as_str())
            .collect::<Vec<_>>()
            .join("; ")
    };
    let consequence = match stand {
        Stand::LeftOut => {
            "It is an `allow` rule, so what it let through is asked instead.".to_owned()
        }
        Stand::Kept(action) => format!(
            "Its action and its match could be read, so until it is fixed it stands as \
             `{action}` on exactly that match, without an end."
        ),
        Stand::Held(Some(host)) => format!(
            "Until it is fixed, every request to `{host}` is held for you at its place, so no \
             rule below it lets through what it may have stopped."
        ),
        Stand::Held(None) => "Until it is fixed, every request that reaches its place is held \
                              for you, whatever the host, so no rule below it lets through what \
                              it may have stopped."
            .to_owned(),
    };
    Diagnostic::builder(code, Severity::Warning)
        .why(format!(
            "{reasons}. {} is left out and the other rules apply. {consequence} The entry stays \
             in rules.yaml as it is, also when the daemon writes the file; fix or remove it and \
             reload the rules",
            place(index, line)
        ))
        .fix(FixAction::CopyCommand("humanitl rules reload".to_owned()))
        .build()
}

/// `rules[2] (line 9)`, soweit die Zeile bekannt ist.
fn place(index: usize, line: Option<usize>) -> String {
    match line {
        Some(line) => format!("rules[{index}] (line {line})"),
        None => format!("rules[{index}]"),
    }
}

/// Mischt die unlesbaren Einträge hinter die Regel, hinter der sie stehen.
///
/// Ein Eintrag ohne Vorgänger steht vorn, einer, dessen Vorgänger es nicht
/// mehr gibt, hinten; [`reanchor`] sorgt dafür, dass es diesen Fall nach einer
/// Änderung nicht gibt. Mehrere Einträge hinter derselben Regel behalten ihre
/// Reihenfolge.
pub fn interleave<T>(
    rules: &[Rule],
    unreadable: &[UnreadableRule],
    rule: impl Fn(&Rule) -> T,
    kept: impl Fn(&UnreadableRule) -> T,
) -> Vec<T> {
    let mut out = Vec::with_capacity(rules.len() + unreadable.len());
    out.extend(
        unreadable
            .iter()
            .filter(|entry| entry.after.is_none())
            .map(&kept),
    );
    for each in rules {
        out.push(rule(each));
        out.extend(
            unreadable
                .iter()
                .filter(|entry| entry.after == Some(each.id))
                .map(&kept),
        );
    }
    out.extend(
        unreadable
            .iter()
            .filter(|entry| {
                entry
                    .after
                    .is_some_and(|id| !rules.iter().any(|rule| rule.id == id))
            })
            .map(&kept),
    );
    out
}

/// Die lesbaren Regeln mit dem Ersatz der unlesbaren an deren Platz.
///
/// So wertet der Proxy aus: Ein Ersatz steht genau dort, wo die Regel stand,
/// weil sein ganzer Zweck ist, die Regeln darunter abzudecken.
#[must_use]
pub fn with_fences(rules: &[Rule], unreadable: &[UnreadableRule]) -> Vec<Rule> {
    interleave(
        rules,
        unreadable,
        |rule| vec![rule.clone()],
        |entry| entry.fences.clone(),
    )
    .into_iter()
    .flatten()
    .collect()
}

/// Hängt Einträge um, deren Vorgänger beim Wechsel von `before` zu `after`
/// verschwunden ist.
///
/// Ein Eintrag rückt dann hinter die nächste Regel davor, die geblieben ist,
/// oder ganz nach vorn. Er bleibt so an seinem Platz relativ zu allem, was
/// über ihm stand, und deckt weiter die Regeln darunter ab.
pub fn reanchor(unreadable: &mut [UnreadableRule], before: &[Rule], after: &[Rule]) {
    let kept = |id: RuleId| after.iter().any(|rule| rule.id == id);
    for entry in unreadable {
        let Some(anchor) = entry.after else {
            continue;
        };
        if kept(anchor) {
            continue;
        }
        let Some(at) = before.iter().position(|rule| rule.id == anchor) else {
            continue;
        };
        entry.after = before[..at]
            .iter()
            .rev()
            .map(|rule| rule.id)
            .find(|id| kept(*id));
    }
}

/// Die erste unlesbare Regel mit Ersatz, über die beim Wechsel von `before`
/// zu `after` eine Regel rückt, die vorher nicht über ihr stand.
///
/// Ein Ersatz deckt die Regeln unter sich ab, das ist sein ganzer Zweck.
/// Rückte eine Regel über ihn (eine neue an eine Stelle davor, eine
/// verschobene von darunter, oder der Ersatz selbst mit seinem Anker nach
/// unten), entschiede sie wieder über das, was die unlesbare Regel vielleicht
/// gestoppt hat. Der Speicher lehnt eine solche Änderung ab, bis die Regel
/// repariert ist. Eine Regel, die verschwindet oder von oben unter den Ersatz
/// rückt, lässt ihn so wirksam wie vorher.
#[must_use]
pub fn lifted_above<'a>(
    before: &[Rule],
    after: &[Rule],
    unreadable_before: &[UnreadableRule],
    unreadable_after: &'a [UnreadableRule],
) -> Option<&'a UnreadableRule> {
    unreadable_after
        .iter()
        .filter(|entry| !entry.fences.is_empty())
        .find(|entry| {
            let was = above(before, unreadable_before, entry.index);
            above(after, unreadable_after, entry.index)
                .iter()
                .any(|id| !was.contains(id))
        })
}

/// Die Ids der lesbaren Regeln über dem Eintrag mit diesem Platz in der Datei.
fn above(rules: &[Rule], unreadable: &[UnreadableRule], index: usize) -> Vec<RuleId> {
    let order = interleave(
        rules,
        unreadable,
        |rule| Err(rule.id),
        |entry| Ok(entry.index),
    );
    order
        .iter()
        .take_while(|item| **item != Ok(index))
        .filter_map(|item| item.err())
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn rule(host: &str) -> Rule {
        Rule::new(
            RuleId::new(),
            Action::Allow,
            Matcher::host(HostPattern::parse(host).unwrap()),
        )
    }

    fn entry(after: Option<RuleId>, action: Option<Action>) -> UnreadableRule {
        UnreadableRule::new(
            0,
            Some(3),
            after,
            Value::Null,
            &[],
            &Readable {
                action,
                matcher: None,
                host: None,
                session: None,
            },
        )
    }

    #[test]
    fn a_removed_anchor_moves_the_entry_behind_the_rule_before_it() {
        let (a, b, c) = (rule("a.example"), rule("b.example"), rule("c.example"));
        let before = vec![a.clone(), b.clone(), c.clone()];
        let mut entries = vec![entry(Some(b.id), Some(Action::Block))];

        reanchor(&mut entries, &before, &[a.clone(), c.clone()]);
        assert_eq!(entries[0].after(), Some(a.id));

        reanchor(&mut entries, &[a, c.clone()], std::slice::from_ref(&c));
        assert_eq!(entries[0].after(), None, "nothing above it is left");
    }

    #[test]
    fn interleave_keeps_the_place_of_every_entry() {
        let (a, b) = (rule("a.example"), rule("b.example"));
        let entries = vec![
            entry(None, Some(Action::Block)),
            entry(Some(a.id), Some(Action::Block)),
        ];
        let order = interleave(
            &[a.clone(), b],
            &entries,
            |rule| rule.matcher.host.to_string(),
            |_| "kept".to_owned(),
        );
        assert_eq!(order, vec!["kept", "a.example", "kept", "b.example"]);
    }

    #[test]
    fn an_unreadable_host_is_fenced_for_every_name_and_address() {
        let fenced = entry(None, Some(Action::Block));
        let hosts: Vec<String> = fenced
            .fences()
            .iter()
            .map(|rule| rule.matcher.host.to_string())
            .collect();
        assert_eq!(
            hosts,
            vec![
                "**",
                "**",
                "cidr:0.0.0.0/0",
                "cidr:0.0.0.0/0",
                "cidr:::/0",
                "cidr:::/0"
            ]
        );
        let upgrades = fenced
            .fences()
            .iter()
            .filter(|rule| rule.matcher.upgrade.is_some());
        assert_eq!(upgrades.count(), 3, "one fence per host for upgrades too");
        assert!(
            fenced
                .fences()
                .iter()
                .all(|rule| rule.action == Action::Ask)
        );
    }

    #[test]
    fn an_unreadable_allow_rule_leaves_no_fence() {
        assert!(entry(None, Some(Action::Allow)).fences().is_empty());
    }
}
