//! Der Regelspeicher: dauerhafte Regeln aus `rules.yaml`, Sitzungsregeln im
//! Speicher, mitgelieferte Regeln aus `rules/default.yaml`.
//!
//! # Warum der Speicher hier liegt und nicht in `humanitl-rules`
//!
//! `humanitl-rules` ist reine Auswertung: kein IO, kein Zustand, keine Uhr.
//! Genau das macht die Regel-Engine tabellengetrieben prüfbar. Der Speicher
//! dagegen liest und schreibt eine Datei, hält den geltenden Stand und
//! benachrichtigt Zuhörer. Er gehört deshalb in die Schicht, die ohnehin IO
//! macht, und steht neben der Halte-Warteschlange, die ihn braucht.
//!
//! # Die drei Gruppen
//!
//! | Gruppe | Woher | Wird geschrieben | Löschbar |
//! |---|---|---|---|
//! | Session | `expires: session` über den RPC | nie | ja |
//! | Dauerhaft | `rules.yaml` des Nutzers | ja | ja |
//! | Mitgeliefert | `rules/default.yaml`, Agent-Adapter | nie | nein (`RULES_010`) |
//!
//! # Die Auswertungsreihenfolge
//!
//! Vier Gruppen, geprüft in dieser Reihenfolge (`backlog/CONVENTIONS.md` 4.5):
//!
//! 1. die mitgelieferte Durchreiche zum Sprachmodell (`passthrough_llm` und
//!    `bundled`),
//! 2. die Sitzungsregeln,
//! 3. die dauerhaften Regeln des Nutzers,
//! 4. die mitgelieferten.
//!
//! Die ersten beiden Ränge macht [`RuleSet::evaluate`] selbst; sie hängen an
//! der Regel, nicht an ihrem Platz. Den Vermerk `bundled`, an dem Rang 1
//! hängt, setzt allein [`RuleSet::add_bundled`]: Eine `rules.yaml` und die
//! Inline-Regeln eines Profils bekommen ihn nicht (`humanitl_rules`
//! verwirft ihn mit `RULES_010`), und über die Leitung kommt er auch nicht. Die letzten beiden macht die Reihenfolge,
//! in der `snapshot_of` den Satz zusammensetzt: die mitgelieferten kommen
//! über [`RuleSet::add_bundled`] ans Ende.
//!
//! Die Gründe: Die Durchreiche ist der eine erklärte Seitenkanal und muss als
//! solcher erkennbar bleiben (`DecisionSource::Passthrough`, `LLM_005`); was
//! der Mensch gerade entschieden hat, soll sofort gelten, auch wenn eine
//! ältere, breitere Regel darunter steht; und eine eigene Regel steht vor
//! jeder mitgelieferten, sonst ließe sich eine mitgelieferte Regel nicht
//! überstimmen, ohne sie zu löschen (HUM-027).
//!
//! # Zusagen, die kein Compiler prüft
//!
//! 1. **Eine Änderung ist ganz oder gar nicht.** Geschrieben wird in eine
//!    Nebendatei im selben Verzeichnis, die mit `fsync` auf die Platte geht und
//!    dann über die alte umbenannt wird. Scheitert irgendetwas davon, bleibt
//!    `rules.yaml` unverändert **und** der Speicher verwirft die Änderung auch
//!    im Arbeitsspeicher: ein Nutzer, dessen Regel nicht ankam, soll sie nicht
//!    bis zum nächsten Start für gespeichert halten.
//! 2. **Sitzungsregeln stehen nie in der Datei.** Sie werden nie serialisiert;
//!    ein Wechsel der Gültigkeit verschiebt eine Regel zwischen den Gruppen.
//! 3. **Eine Regel, die über die Leitung kommt, wird von der Engine geprüft,
//!    bevor sie gilt.** Der Speicher schreibt sie dafür als YAML und liest sie
//!    mit [`parse_rules_for_session`] zurück; abgelehnt wird sie mit dem Befund
//!    der Engine samt Zeilennummer. Damit ist zugleich bewiesen, dass die Regel
//!    die Datei überlebt.
//! 4. **Mitgelieferte Regeln gehören nicht dem Nutzer.** Weder Anlegen noch
//!    Ändern noch Löschen; `bundled` von außen wird immer auf `false` gesetzt.

use std::collections::BTreeSet;
use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::Utc;
use humanitl_core::diagnostics::codes;
use humanitl_core::rule::{Action, Expiry, Rule};
use humanitl_core::{Diagnostic, FixAction, RuleId, SessionId, Severity};
use humanitl_rules::{RuleSet, parse_rules_for_session, serialize_rules};
use tokio::sync::broadcast;

/// Rechte der Regel-Datei und ihrer Nebendatei: nur der Besitzer.
///
/// Eine Regel entscheidet über Netzverkehr; wer sie ändern darf, entscheidet
/// mit. `rules.yaml` ist deshalb `0600` wie Token und Socket.
pub const RULES_MODE: u32 = 0o600;

/// Rechte des Verzeichnisses, falls der Speicher es anlegen muss.
pub const RULES_DIR_MODE: u32 = 0o700;

/// So viele Revisionen warten höchstens auf einen langsamen Zuhörer.
///
/// Ein Zuhörer, der zurückfällt, verliert Zwischenstände und erfährt die
/// jüngste Revision; mehr braucht er nicht, denn er lädt danach die ganze
/// Liste (`Subscribe`-Ereignis `RulesChanged`).
const REVISION_BUFFER: usize = 16;

/// Woher eine Regel kommt.
///
/// Die Gruppe bestimmt, ob eine Änderung in die Datei geht und ob die Regel
/// überhaupt geändert werden darf.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Origin {
    /// Nur für diese Sitzung, nur im Speicher.
    Session,
    /// Dauerhaft, steht in `rules.yaml`.
    User,
    /// Mitgeliefert, unveränderlich.
    Bundled,
}

impl Origin {
    /// Kurzname in `snake_case`, wie ihn CLI und Oberfläche zeigen.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::User => "user",
            Self::Bundled => "bundled",
        }
    }
}

/// Eine Regel mit ihrer Gruppe und ihrem Platz darin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredRule {
    /// Die Regel selbst.
    pub rule: Rule,
    /// Aus welcher Gruppe sie stammt.
    pub origin: Origin,
    /// Platz innerhalb der Gruppe, 1-basiert.
    pub position: u32,
}

/// Was ein [`RulesStore::reload`] verändert hat.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReloadReport {
    /// Regeln, die es vorher nicht gab.
    pub added: usize,
    /// Regeln, die es nachher nicht mehr gibt.
    pub removed: usize,
    /// Regeln mit derselben Id und anderem Inhalt.
    pub changed: usize,
    /// Regeln, die unverändert blieben.
    pub unchanged: usize,
}

impl ReloadReport {
    /// Wahr, wenn die Datei denselben Regelsatz enthielt wie vorher.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.added == 0 && self.removed == 0 && self.changed == 0
    }

    /// Der Satz, den der Nutzer liest.
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "{} added, {} removed, {} changed, {} unchanged",
            self.added, self.removed, self.changed, self.unchanged
        )
    }
}

/// Der veränderliche Teil des Speichers.
#[derive(Debug)]
struct State {
    /// Regeln dieser Sitzung, in ihrer Reihenfolge.
    session: Vec<Rule>,
    /// Dauerhafte Regeln des Nutzers, in ihrer Reihenfolge.
    persistent: Vec<Rule>,
    /// Mitgelieferte Regeln, in ihrer Reihenfolge.
    bundled: Vec<Rule>,
    /// Die Ids mitgelieferter Regeln, die der Nutzer abgeschaltet hat.
    ///
    /// Sie stehen getrennt von den Regeln, weil sie eine Regel benennen dürfen,
    /// die es in dieser Fassung (noch) nicht gibt: Ein Abschalten darf nicht
    /// stillschweigend zurückgehen, nur weil `rules/default.yaml` sich geändert
    /// hat (HUM-038).
    disabled_bundled: BTreeSet<RuleId>,
}

impl State {
    /// Die Gruppe, in der die Regel mit dieser Id steht.
    fn origin_of(&self, id: RuleId) -> Option<Origin> {
        if self.session.iter().any(|rule| rule.id == id) {
            return Some(Origin::Session);
        }
        if self.persistent.iter().any(|rule| rule.id == id) {
            return Some(Origin::User);
        }
        if self.bundled.iter().any(|rule| rule.id == id) {
            return Some(Origin::Bundled);
        }
        None
    }
}

/// Regeln lesen, ändern und dauerhaft machen.
///
/// Der Speicher ist ein Handle, kein Wert: Proxy und gRPC-Dienst halten
/// dasselbe [`Arc`], der eine liest bei jeder Anfrage, der andere ändert.
/// Deshalb nehmen auch die ändernden Methoden `&self` und der Zustand liegt
/// hinter einem Mutex. Das weicht von der Signatur in `backlog/sprint-2.md`
/// (HUM-027) ab, die `&mut self` vorsah; ein `&mut` wäre nur mit einem zweiten
/// Schloss um den ganzen Speicher zu haben, und die Auswertung im Proxy stünde
/// dann hinter demselben Schloss wie das Schreiben der Datei.
#[derive(Debug)]
pub struct RulesStore {
    path: PathBuf,
    session: SessionId,
    state: Mutex<State>,
    /// Der geltende Regelsatz, wie ihn die Auswertung im Proxy liest.
    ///
    /// Genau die Form, die [`crate::pipeline::RulesPipeline::new`] erwartet:
    /// Der Proxy liest hier, ohne den Speicher zu kennen.
    snapshot: Arc<RwLock<RuleSet>>,
    revision: AtomicU64,
    changed: broadcast::Sender<u64>,
}

impl RulesStore {
    /// Liest `rules.yaml` und baut den Speicher.
    ///
    /// Eine fehlende Datei ist kein Fehler: vor der ersten eigenen Regel gibt
    /// es sie nicht. Lässt die Engine die Datei nicht durchgehen, startet der
    /// Speicher **ohne** dauerhafte Regeln des Nutzers und meldet die Befunde;
    /// ein halb geladener Regelsatz wäre die schlechtere Antwort, weil niemand
    /// ihm ansieht, welche Hälfte fehlt. Ohne Regel wird jede Anfrage gehalten,
    /// der Fehler kostet also Rückfragen und nie eine stille Freigabe.
    ///
    /// `bundled` sind die mitgelieferten Regeln aus `rules/default.yaml` und
    /// die Durchreiche des Agent-Adapters; sie werden hinter die Nutzerregeln
    /// gehängt und als `bundled` markiert. Die Marken stehen schon hier und
    /// nicht erst im Schnappschuss, weil [`RulesStore::list`] sie zeigt;
    /// [`RuleSet::add_bundled`] setzt dieselben Marken noch einmal, damit ein
    /// Satz auch dann stimmt, wenn er anderswo gebaut wurde.
    #[must_use]
    pub fn load(path: &Path, bundled: &[Rule], session: SessionId) -> (Self, Vec<Diagnostic>) {
        let (persistent, disabled_bundled, diagnostics) = read_file(path, session);
        let bundled: Vec<Rule> = bundled
            .iter()
            .cloned()
            .map(|rule| {
                let off = rule.disabled || disabled_bundled.contains(&rule.id);
                rule.bundled(true).disabled(off)
            })
            .collect();
        let state = State {
            session: Vec::new(),
            persistent,
            bundled,
            disabled_bundled,
        };
        let snapshot = Arc::new(RwLock::new(snapshot_of(&state)));
        let (changed, _idle) = broadcast::channel(REVISION_BUFFER);
        let store = Self {
            path: path.to_path_buf(),
            session,
            state: Mutex::new(state),
            snapshot,
            revision: AtomicU64::new(1),
            changed,
        };
        (store, diagnostics)
    }

    /// Ein Speicher ohne Datei, für Tests und den Fake-Modus.
    ///
    /// Schreiben scheitert dann mit `RULES_009`, weil es kein Verzeichnis gibt;
    /// Sitzungsregeln funktionieren.
    #[must_use]
    pub fn in_memory(session: SessionId) -> Self {
        let (store, _) = Self::load(Path::new(""), &[], session);
        store
    }

    /// Der Pfad der Regel-Datei.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Die Sitzung, für die `expires: session` gilt.
    #[must_use]
    pub const fn session(&self) -> SessionId {
        self.session
    }

    /// Der geltende Regelsatz, wie ihn der Proxy liest.
    ///
    /// Dasselbe Handle bleibt über jede Änderung hinweg gültig; der Inhalt
    /// wird ersetzt, nie das Schloss. Das ist die Naht zum Proxy:
    /// [`crate::pipeline::RulesPipeline::new`] nimmt genau diesen Wert.
    #[must_use]
    pub fn snapshot(&self) -> Arc<RwLock<RuleSet>> {
        Arc::clone(&self.snapshot)
    }

    /// Der geltende Regelsatz als Kopie.
    ///
    /// Für Tests und Aufrufer, die einmal auswerten wollen, ohne das Handle zu
    /// halten. Der Proxy nimmt [`RulesStore::snapshot`].
    #[must_use]
    pub fn effective(&self) -> RuleSet {
        self.snapshot
            .read()
            .map_or_else(|poisoned| poisoned.into_inner().clone(), |set| set.clone())
    }

    /// Die laufende Revision. Steigt mit jeder Änderung.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision.load(Ordering::Relaxed)
    }

    /// Ein Zuhörer auf Änderungen; jede Nachricht ist die neue Revision.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<u64> {
        self.changed.subscribe()
    }

    /// Alle Regeln mit ihrer Gruppe und ihrem Platz darin: Sitzung, Nutzer,
    /// mitgeliefert.
    ///
    /// Das ist die Reihenfolge der **Gruppen**, nicht die der Auswertung: Die
    /// mitgelieferte Durchreiche zum Sprachmodell steht hier am Ende, in ihrer
    /// Gruppe, und wird trotzdem vor allem anderen geprüft
    /// (`backlog/CONVENTIONS.md` 4.5). Sortiert wird deshalb nicht: Die Liste
    /// ist das, was Oberfläche und `humanitl rules list` zeigen, und dort ist
    /// die Gruppe die Ordnung, die ein Mensch sucht — die Durchreiche trägt
    /// ihren Rang sichtbar an sich (`passthrough_llm`, in der Liste als eigene
    /// Kennzeichnung).
    #[must_use]
    pub fn list(&self) -> Vec<StoredRule> {
        let state = self.lock();
        let mut out = Vec::new();
        for (origin, rules) in [
            (Origin::Session, &state.session),
            (Origin::User, &state.persistent),
            (Origin::Bundled, &state.bundled),
        ] {
            for (index, rule) in rules.iter().enumerate() {
                out.push(StoredRule {
                    rule: rule.clone(),
                    origin,
                    position: u32::try_from(index + 1).unwrap_or(u32::MAX),
                });
            }
        }
        out
    }

    /// Die Regel mit dieser Id, samt ihrer Gruppe.
    #[must_use]
    pub fn get(&self, id: RuleId) -> Option<StoredRule> {
        self.list().into_iter().find(|stored| stored.rule.id == id)
    }

    /// Legt eine Regel an.
    ///
    /// `pos` ist der 0-basierte Platz **innerhalb der Gruppe**, in die die
    /// Regel gehört, nicht in der effektiven Liste (`backlog/sprint-2.md`,
    /// HUM-027, Fallstricke). `None` heißt: ans Ende. Eine Regel mit
    /// `expires: session` bleibt im Speicher, jede andere geht in die Datei.
    ///
    /// # Errors
    ///
    /// Der Befund der Engine, wenn die Regel nicht durchgeht (`RULES_00x` mit
    /// Zeilennummer), `RULES_010`, wenn sie sich als mitgeliefert ausgibt, und
    /// `RULES_009`, wenn die Datei nicht geschrieben werden konnte.
    pub fn add(&self, rule: &Rule, pos: Option<usize>) -> Result<Rule, Diagnostic> {
        let rule = self.validated(rule)?;
        let mut state = self.lock();
        if state.origin_of(rule.id).is_some() {
            return Err(duplicate(rule.id));
        }
        let session_scoped = matches!(rule.expires, Expiry::Session(_));
        if session_scoped {
            let at = clamp(pos, state.session.len());
            state.session.insert(at, rule.clone());
        } else {
            let mut next = state.persistent.clone();
            next.insert(clamp(pos, next.len()), rule.clone());
            self.write(&next, &state.disabled_bundled)?;
            state.persistent = next;
        }
        self.publish(&state);
        Ok(rule)
    }

    /// Ersetzt eine Regel an ihrem Platz.
    ///
    /// Wechselt dabei die Gültigkeit zwischen `session` und dauerhaft, wandert
    /// die Regel in die andere Gruppe und wird ans Ende gehängt: ihr alter
    /// Platz gehört zu einer Liste, in der sie nicht mehr steht.
    ///
    /// # Errors
    ///
    /// `IPC_005`, wenn es die Regel nicht gibt, `RULES_010` bei einer
    /// mitgelieferten Regel, sonst wie [`RulesStore::add`].
    pub fn update(&self, rule: &Rule) -> Result<Rule, Diagnostic> {
        let rule = self.validated(rule)?;
        let mut state = self.lock();
        let origin = state.origin_of(rule.id).ok_or_else(|| unknown(rule.id))?;
        if origin == Origin::Bundled {
            return Err(immutable_bundled(&rule, "changed"));
        }
        let wants_session = matches!(rule.expires, Expiry::Session(_));
        match (origin, wants_session) {
            (Origin::Session, true) => {
                replace_in(&mut state.session, &rule);
            }
            (Origin::User, false) => {
                let mut next = state.persistent.clone();
                replace_in(&mut next, &rule);
                self.write(&next, &state.disabled_bundled)?;
                state.persistent = next;
            }
            (Origin::Session, false) => {
                let mut next = state.persistent.clone();
                next.push(rule.clone());
                self.write(&next, &state.disabled_bundled)?;
                state.session.retain(|old| old.id != rule.id);
                state.persistent = next;
            }
            (Origin::User, true) => {
                let mut next = state.persistent.clone();
                next.retain(|old| old.id != rule.id);
                self.write(&next, &state.disabled_bundled)?;
                state.persistent = next;
                state.session.push(rule.clone());
            }
            (Origin::Bundled, _) => unreachable!("bundled rules are refused above"),
        }
        self.publish(&state);
        Ok(rule)
    }

    /// Nimmt eine Regel heraus.
    ///
    /// # Errors
    ///
    /// `IPC_005`, wenn es die Regel nicht gibt, `RULES_010` bei einer
    /// mitgelieferten Regel, `RULES_009`, wenn die Datei nicht geschrieben
    /// werden konnte.
    pub fn remove(&self, id: RuleId) -> Result<Rule, Diagnostic> {
        let mut state = self.lock();
        let origin = state.origin_of(id).ok_or_else(|| unknown(id))?;
        match origin {
            Origin::Bundled => {
                let rule = state
                    .bundled
                    .iter()
                    .find(|rule| rule.id == id)
                    .cloned()
                    .ok_or_else(|| unknown(id))?;
                Err(immutable_bundled(&rule, "removed"))
            }
            Origin::Session => {
                let at = position_of(&state.session, id).ok_or_else(|| unknown(id))?;
                let rule = state.session.remove(at);
                self.publish(&state);
                Ok(rule)
            }
            Origin::User => {
                let at = position_of(&state.persistent, id).ok_or_else(|| unknown(id))?;
                let mut next = state.persistent.clone();
                let rule = next.remove(at);
                self.write(&next, &state.disabled_bundled)?;
                state.persistent = next;
                self.publish(&state);
                Ok(rule)
            }
        }
    }

    /// Verschiebt eine Regel innerhalb ihrer Gruppe.
    ///
    /// `pos` ist 0-basiert und wird auf das Ende geklemmt.
    ///
    /// # Errors
    ///
    /// `IPC_005`, wenn es die Regel nicht gibt, `RULES_010` bei einer
    /// mitgelieferten Regel, `RULES_009` beim Schreiben.
    pub fn reorder(&self, id: RuleId, pos: usize) -> Result<(), Diagnostic> {
        let mut state = self.lock();
        match state.origin_of(id).ok_or_else(|| unknown(id))? {
            Origin::Bundled => {
                let rule = state
                    .bundled
                    .iter()
                    .find(|rule| rule.id == id)
                    .cloned()
                    .ok_or_else(|| unknown(id))?;
                Err(immutable_bundled(&rule, "reordered"))
            }
            Origin::Session => {
                move_within(&mut state.session, id, pos);
                self.publish(&state);
                Ok(())
            }
            Origin::User => {
                let mut next = state.persistent.clone();
                move_within(&mut next, id, pos);
                self.write(&next, &state.disabled_bundled)?;
                state.persistent = next;
                self.publish(&state);
                Ok(())
            }
        }
    }

    /// Ordnet eine Gruppe vollständig nach einer Liste von Ids um.
    ///
    /// Ids, die nicht vorkommen, werden übergangen; Regeln, die in der Liste
    /// fehlen, behalten ihre Reihenfolge und stehen hinten. Die Liste darf
    /// Regeln beider Gruppen nennen; jede Gruppe wird für sich sortiert, weil
    /// eine Position nur innerhalb einer Gruppe eine Bedeutung hat.
    ///
    /// # Errors
    ///
    /// `RULES_009`, wenn die Datei nicht geschrieben werden konnte.
    pub fn reorder_all(&self, order: &[RuleId]) -> Result<(), Diagnostic> {
        let mut state = self.lock();
        let sorted_session = sort_by_order(&state.session, order);
        let sorted_persistent = sort_by_order(&state.persistent, order);
        if sorted_persistent != state.persistent {
            self.write(&sorted_persistent, &state.disabled_bundled)?;
            state.persistent = sorted_persistent;
        }
        state.session = sorted_session;
        self.publish(&state);
        Ok(())
    }

    /// Macht eine Sitzungsregel dauerhaft.
    ///
    /// # Errors
    ///
    /// `IPC_005`, wenn es die Regel nicht gibt oder sie schon dauerhaft ist,
    /// `RULES_010` bei einer mitgelieferten Regel, `RULES_009` beim Schreiben.
    pub fn make_permanent(&self, id: RuleId) -> Result<Rule, Diagnostic> {
        let stored = self.get(id).ok_or_else(|| unknown(id))?;
        match stored.origin {
            Origin::Bundled => Err(immutable_bundled(&stored.rule, "changed")),
            Origin::User => Err(Diagnostic::builder(codes::IPC_005, Severity::Error)
                .why(format!("the rule {id} is already permanent"))
                .build()),
            Origin::Session => self.update(&stored.rule.with_expiry(Expiry::Never)),
        }
    }

    /// Liest `rules.yaml` neu ein.
    ///
    /// Lehnt die Engine die Datei ab, bleiben die geltenden Regeln in Kraft und
    /// die Befunde beschreiben, warum. Sitzungsregeln bleiben unberührt, sie
    /// stehen nie in der Datei.
    ///
    /// Der erste Befund ist bei Erfolg ein `RULES_011` mit dem, was sich
    /// geändert hat.
    pub fn reload(&self) -> Vec<Diagnostic> {
        let (loaded, disabled_bundled, mut diagnostics) = read_file(&self.path, self.session);
        if diagnostics
            .iter()
            .any(|diagnostic| matches!(diagnostic.severity, Severity::Error | Severity::Blocking))
        {
            return diagnostics;
        }
        let mut state = self.lock();
        let report = compare(&state.persistent, &loaded);
        state.persistent = loaded;
        // Auch das Abschalten kommt aus der Datei: Wer sie von Hand ändert,
        // erwartet, dass ein `reload` genau das übernimmt.
        for rule in &mut state.bundled {
            rule.disabled = disabled_bundled.contains(&rule.id);
        }
        state.disabled_bundled = disabled_bundled;
        self.publish(&state);
        diagnostics.insert(
            0,
            Diagnostic::builder(codes::RULES_011, Severity::Info)
                .why(format!(
                    "{} reloaded: {}",
                    self.path.display(),
                    report.summary()
                ))
                .build(),
        );
        diagnostics
    }

    /// Schaltet eine mitgelieferte Regel ab oder wieder an.
    ///
    /// Mitgelieferte Regeln gehören nicht dem Nutzer: löschen oder ändern
    /// lehnt der Speicher mit `RULES_010` ab. Abschalten ist der Weg, sie
    /// aufzuheben, ohne die Begründung zu verlieren, die im Rules-Screen
    /// steht. Der Zustand liegt in der `rules.yaml` des Nutzers als Liste
    /// `disabled_bundled` und überlebt eine neue Fassung von
    /// `rules/default.yaml` (HUM-038).
    ///
    /// # Errors
    ///
    /// `RULES_010`, wenn die Id keine mitgelieferte Regel benennt — eine
    /// eigene Regel löscht man, statt sie abzuschalten. `RULES_009`, wenn die
    /// Datei nicht geschrieben werden konnte; der Speicher bleibt dann
    /// unverändert.
    pub fn set_bundled_disabled(&self, id: RuleId, disabled: bool) -> Result<Rule, Diagnostic> {
        let mut state = self.lock();
        let Some(position) = state.bundled.iter().position(|rule| rule.id == id) else {
            return Err(Diagnostic::builder(codes::RULES_010, Severity::Error)
                .why(format!(
                    "there is no bundled rule with the id {id}; only bundled rules are \
                     disabled instead of removed"
                ))
                .build());
        };

        let mut next = state.disabled_bundled.clone();
        if disabled {
            next.insert(id);
        } else {
            next.remove(&id);
        }
        let persistent = state.persistent.clone();
        self.write(&persistent, &next)?;

        state.disabled_bundled = next;
        let Some(rule) = state.bundled.get_mut(position) else {
            return Err(Diagnostic::builder(codes::RULES_010, Severity::Error)
                .why(format!(
                    "the bundled rule {id} disappeared while it was changed"
                ))
                .build());
        };
        rule.disabled = disabled;
        let changed = rule.clone();
        self.publish(&state);
        Ok(changed)
    }

    /// Entfernt abgelaufene Regeln aus beiden Gruppen.
    ///
    /// Dauerhafte Regeln mit einem Zeitpunkt in der Vergangenheit verschwinden
    /// dabei auch aus der Datei.
    ///
    /// # Errors
    ///
    /// `RULES_009`, wenn die Datei nicht geschrieben werden konnte. Die
    /// Sitzungsregeln sind dann trotzdem aufgeräumt.
    pub fn prune(&self) -> Result<usize, Diagnostic> {
        let now = Utc::now();
        let mut state = self.lock();
        let before = state.session.len() + state.persistent.len();
        state
            .session
            .retain(|rule| !rule.is_expired(now, self.session));
        let kept: Vec<Rule> = state
            .persistent
            .iter()
            .filter(|rule| !rule.is_expired(now, self.session))
            .cloned()
            .collect();
        let removed = before - (state.session.len() + kept.len());
        if kept.len() != state.persistent.len() {
            self.write(&kept, &state.disabled_bundled)?;
            state.persistent = kept;
        }
        if removed > 0 {
            self.publish(&state);
        }
        Ok(removed)
    }

    /// Prüft eine Regel mit der Engine, bevor sie gilt.
    ///
    /// Der Weg über YAML ist Absicht: Er benutzt genau den Parser, der die
    /// Datei liest, und beweist damit zugleich, dass die Regel die Datei
    /// überlebt. Der Befund trägt deshalb dieselbe Form wie ein Fehler in
    /// `rules.yaml`, samt Feldpfad und Zeile.
    fn validated(&self, rule: &Rule) -> Result<Rule, Diagnostic> {
        if rule.bundled {
            return Err(Diagnostic::builder(codes::RULES_010, Severity::Error)
                .why("a rule that arrives over the wire is never bundled".to_owned())
                .build());
        }
        let yaml = serialize_rules(&RuleSet::from_rules([rule.clone()]));
        match parse_rules_for_session(&yaml, self.session) {
            Ok((set, _warnings)) => set
                .iter()
                .next()
                .cloned()
                .ok_or_else(|| {
                    Diagnostic::builder(codes::RULES_001, Severity::Error)
                        .why(
                            "the rule did not survive the round trip through rules.yaml".to_owned(),
                        )
                        .build()
                })
                .map(|parsed| Rule {
                    // Die Sitzungs-Id geht beim Serialisieren verloren
                    // (`expires: session` steht ohne Id in der Datei) und kommt
                    // aus dieser Sitzung zurück; alles andere stammt aus dem
                    // Parser.
                    expires: rule.expires,
                    ..parsed
                }),
            Err(diagnostics) => Err(diagnostics.into_iter().next().unwrap_or_else(|| {
                Diagnostic::builder(codes::RULES_001, Severity::Error)
                    .why("the rule was refused without a reason".to_owned())
                    .build()
            })),
        }
    }

    /// Schreibt die dauerhaften Regeln, atomar.
    fn write(&self, rules: &[Rule], disabled_bundled: &BTreeSet<RuleId>) -> Result<(), Diagnostic> {
        let mut set = RuleSet::from_rules(rules.iter().cloned());
        // Ohne diese Zeile verschwände die Liste bei der nächsten Änderung aus
        // der Datei, und ein abgeschaltetes Bündel käme still zurück.
        set.set_disabled_bundled(disabled_bundled.iter().copied());
        write_atomically(&self.path, &serialize_rules(&set))
    }

    /// Übernimmt den neuen Stand in den Schnappschuss und meldet die Revision.
    fn publish(&self, state: &State) {
        let next = snapshot_of(state);
        match self.snapshot.write() {
            Ok(mut slot) => *slot = next,
            Err(poisoned) => *poisoned.into_inner() = next,
        }
        let revision = self.revision.fetch_add(1, Ordering::Relaxed) + 1;
        // Ohne Zuhörer ist der Fehler der Normalfall (niemand abonniert).
        let _ = self.changed.send(revision);
    }

    /// Der Zustand, auch wenn ein Thread mit dem Schloss abgestürzt ist.
    ///
    /// Ein vergiftetes Schloss heißt hier nicht, dass die Daten kaputt sind:
    /// Jede Änderung ist erst nach dem erfolgreichen Schreiben sichtbar. Den
    /// Speicher deshalb stillzulegen, hieße, den Proxy ohne Regeln laufen zu
    /// lassen.
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Der Regelsatz, den der Proxy sieht: die Regeln plus die Liste der
/// abgeschalteten mitgelieferten.
///
/// Hier entsteht die Reihenfolge der Ränge 3 und 4 aus dem Kopf dieser Datei:
/// erst die Sitzungsregeln und die dauerhaften des Nutzers, dann über
/// [`RuleSet::add_bundled`] die mitgelieferten ans Ende. Ein Vertauschen wäre
/// nicht bloß Kosmetik — es nähme dem Nutzer die Möglichkeit, eine
/// mitgelieferte Regel zu überstimmen, und machte damit den Vorschlag aus
/// [`immutable_bundled`] unerfüllbar. Der Test
/// `a_user_rule_overrides_a_bundled_rule`
/// (`daemon/crates/proxy/tests/rules_order.rs`) hält das fest.
///
/// Die Durchreiche zum Sprachmodell kommt als mitgelieferte Regel ebenfalls
/// ans Ende und wird trotzdem zuerst geprüft: Ihren Vorrang trägt sie an sich
/// selbst, nicht an ihrem Platz in der Liste.
fn snapshot_of(state: &State) -> RuleSet {
    let mut set = RuleSet::from_rules(state.session.iter().chain(&state.persistent).cloned());
    // Vor `add_bundled`: die Liste entscheidet dort mit, welche mitgelieferte
    // Regel abgeschaltet in den Satz geht.
    set.set_disabled_bundled(state.disabled_bundled.iter().copied());
    set.add_bundled(state.bundled.iter().cloned());
    set
}

/// Liest die Regel-Datei; eine fehlende Datei ist ein leerer Regelsatz.
fn read_file(path: &Path, session: SessionId) -> (Vec<Rule>, BTreeSet<RuleId>, Vec<Diagnostic>) {
    if path.as_os_str().is_empty() {
        return (Vec::new(), BTreeSet::new(), Vec::new());
    }
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return (Vec::new(), BTreeSet::new(), Vec::new());
        }
        Err(error) => {
            return (
                Vec::new(),
                BTreeSet::new(),
                vec![
                    Diagnostic::builder(codes::RULES_001, Severity::Error)
                        .why(format!("cannot read {}: {error}", path.display()))
                        .build(),
                ],
            );
        }
    };
    match parse_rules_for_session(&text, session) {
        Ok((set, warnings)) => (
            set.iter().cloned().collect(),
            set.disabled_bundled().collect(),
            warnings,
        ),
        Err(diagnostics) => (Vec::new(), BTreeSet::new(), diagnostics),
    }
}

/// Schreibt eine Datei ganz oder gar nicht.
///
/// Reihenfolge, und sie ist der ganze Punkt dieser Funktion:
///
/// 1. Nebendatei im **selben** Verzeichnis anlegen, mit `0600` und
///    `create_new`, damit weder ein anderes Dateisystem noch eine
///    vorgefundene Datei im Weg ist.
/// 2. Inhalt schreiben und mit `sync_all` auf die Platte zwingen.
/// 3. Über die alte Datei umbenennen; das ist die atomare Stelle.
/// 4. Das Verzeichnis synchronisieren, damit auch der Namenseintrag einen
///    Stromausfall übersteht.
///
/// Scheitert einer der Schritte, verschwindet die Nebendatei und die alte
/// Datei bleibt, wie sie war.
///
/// # Errors
///
/// `RULES_009` mit dem Schritt, der scheiterte.
fn write_atomically(path: &Path, content: &str) -> Result<(), Diagnostic> {
    let parent = path.parent().filter(|dir| !dir.as_os_str().is_empty());
    let Some(dir) = parent else {
        return Err(io_failed(path, "the path has no directory"));
    };
    if !dir.exists() {
        std::fs::create_dir_all(dir).map_err(|error| io_failed(path, &error.to_string()))?;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(RULES_DIR_MODE));
    }

    let temp = dir.join(temp_name(path));
    let write = || -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(RULES_MODE)
            .open(&temp)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temp, path)?;
        // Der Umbenennung fehlt sonst der Eintrag im Verzeichnis, wenn der
        // Strom zwischen `rename` und dem nächsten Schreiben ausfällt.
        File::open(dir)?.sync_all()
    };
    match write() {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = std::fs::remove_file(&temp);
            Err(io_failed(path, &error.to_string()))
        }
    }
}

/// Ein Name für die Nebendatei, der nicht mit einem zweiten Schreiber kollidiert.
fn temp_name(path: &Path) -> String {
    let stem = path.file_name().map_or_else(
        || "rules.yaml".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.subsec_nanos());
    format!(".{stem}.tmp.{}.{nanos}", std::process::id())
}

/// Klemmt eine gewünschte Position auf die Länge der Liste.
fn clamp(pos: Option<usize>, len: usize) -> usize {
    pos.unwrap_or(len).min(len)
}

/// Der Platz der Regel mit dieser Id.
fn position_of(rules: &[Rule], id: RuleId) -> Option<usize> {
    rules.iter().position(|rule| rule.id == id)
}

/// Ersetzt die Regel mit derselben Id an ihrem Platz.
fn replace_in(rules: &mut [Rule], rule: &Rule) {
    if let Some(slot) = rules.iter_mut().find(|old| old.id == rule.id) {
        *slot = rule.clone();
    }
}

/// Verschiebt eine Regel innerhalb ihrer Liste.
fn move_within(rules: &mut Vec<Rule>, id: RuleId, pos: usize) {
    let Some(at) = position_of(rules, id) else {
        return;
    };
    let rule = rules.remove(at);
    let target = pos.min(rules.len());
    rules.insert(target, rule);
}

/// Sortiert eine Liste nach der Reihenfolge der Ids; Unbekanntes bleibt hinten.
fn sort_by_order(rules: &[Rule], order: &[RuleId]) -> Vec<Rule> {
    let mut rest: Vec<Rule> = rules.to_vec();
    let mut sorted = Vec::with_capacity(rules.len());
    for id in order {
        if let Some(at) = position_of(&rest, *id) {
            sorted.push(rest.remove(at));
        }
    }
    sorted.append(&mut rest);
    sorted
}

/// Vergleicht zwei Regellisten anhand ihrer Ids.
fn compare(before: &[Rule], after: &[Rule]) -> ReloadReport {
    let mut report = ReloadReport::default();
    for rule in after {
        match before.iter().find(|old| old.id == rule.id) {
            None => report.added += 1,
            Some(old) if old == rule => report.unchanged += 1,
            Some(_) => report.changed += 1,
        }
    }
    report.removed = before
        .iter()
        .filter(|old| !after.iter().any(|rule| rule.id == old.id))
        .count();
    report
}

/// Der Befund für eine Datei, die nicht geschrieben werden konnte.
fn io_failed(path: &Path, why: &str) -> Diagnostic {
    Diagnostic::builder(codes::RULES_009, Severity::Error)
        .why(format!(
            "cannot write {}: {why}; the rule set on disk is unchanged",
            path.display()
        ))
        .build()
}

/// Der Befund für eine Regel, die es nicht gibt.
fn unknown(id: RuleId) -> Diagnostic {
    Diagnostic::builder(codes::IPC_005, Severity::Error)
        .why(format!("there is no rule with the id {id}"))
        .build()
}

/// Der Befund für eine Id, die es schon gibt.
fn duplicate(id: RuleId) -> Diagnostic {
    Diagnostic::builder(codes::RULES_007, Severity::Error)
        .why(format!(
            "the id {id} is already taken; every rule needs its own"
        ))
        .build()
}

/// Der Befund für den Versuch, eine mitgelieferte Regel anzufassen.
///
/// Der Vorschlag ist die einzige Antwort, die dem Nutzer wirklich hilft: eine
/// eigene Regel mit demselben Muster, die davor steht. Sie hebt die
/// mitgelieferte auf, ohne sie zu löschen — und bleibt sichtbar, wenn jemand
/// später fragt, warum der Host durchgeht.
///
/// Öffentlich, weil der Fake-Daemon denselben Befund liefern muss: Eine
/// mitgelieferte Regel ist auch dort unlöschbar, und `RULES_010` zweimal zu
/// bauen hieße, dieselbe Aussage zweimal zu pflegen (ADR-0018).
#[must_use]
pub fn immutable_bundled(rule: &Rule, verb: &str) -> Diagnostic {
    let own = Rule::new(RuleId::new(), Action::Ask, rule.matcher.clone())
        .with_note("overrides the bundled rule above it");
    Diagnostic::builder(codes::RULES_010, Severity::Error)
        .why(format!(
            "the rule {} is bundled and cannot be {verb}",
            rule.id
        ))
        .fix(FixAction::AddRule(Box::new(own)))
        .build()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::os::unix::fs::PermissionsExt as _;

    use chrono::{Duration, Utc};
    use humanitl_core::rule::{Action, Expiry, HostPattern, Matcher, Rule};
    use humanitl_core::{HostName, Method, RuleId, Scheme, SessionId};
    use humanitl_rules::{RequestKey, Verdict};

    use super::{Origin, RULES_MODE, RulesStore};

    fn rule(host: &str, action: Action) -> Rule {
        let pattern = HostPattern::parse(host).expect("pattern");
        Rule::new(RuleId::new(), action, Matcher::host(pattern))
    }

    fn store(dir: &tempfile::TempDir) -> (RulesStore, SessionId) {
        let session = SessionId::new();
        let (store, diagnostics) = RulesStore::load(
            &dir.path().join("humanitl").join("rules.yaml"),
            &[],
            session,
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        (store, session)
    }

    #[test]
    fn a_session_rule_never_reaches_the_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (store, session) = store(&dir);
        let rule = rule("api.github.com", Action::Allow).with_expiry(Expiry::Session(session));

        let added = store.add(&rule, None).expect("add");
        assert_eq!(store.list().len(), 1);
        assert_eq!(store.get(added.id).expect("stored").origin, Origin::Session);
        assert!(
            !store.path().exists(),
            "a session rule must not create rules.yaml"
        );
    }

    #[test]
    fn a_permanent_rule_lands_in_the_file_with_0600() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (store, _) = store(&dir);
        store
            .add(&rule("api.github.com", Action::Allow), None)
            .expect("add");

        let text = std::fs::read_to_string(store.path()).expect("read");
        assert!(text.contains("api.github.com"), "{text}");
        let mode = std::fs::metadata(store.path())
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, RULES_MODE);

        // Was geschrieben wurde, liest der Speicher auch wieder ein.
        let (again, diagnostics) = RulesStore::load(store.path(), &[], SessionId::new());
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(again.list().len(), 1);
    }

    #[test]
    fn a_bundled_rule_cannot_be_removed_and_the_fix_names_a_replacement() {
        let dir = tempfile::tempdir().expect("tempdir");
        let session = SessionId::new();
        let bundled = rule("models.dev", Action::Block);
        let (store, _) = RulesStore::load(
            &dir.path().join("rules.yaml"),
            std::slice::from_ref(&bundled),
            session,
        );

        let error = store.remove(bundled.id).expect_err("bundled stays");
        assert_eq!(error.code.as_str(), "RULES_010");
        assert!(error.fix.is_some(), "the refusal names a way out");
        assert_eq!(store.list().len(), 1);
    }

    #[test]
    fn session_rules_are_evaluated_before_bundled_ones() {
        let dir = tempfile::tempdir().expect("tempdir");
        let session = SessionId::new();
        let bundled = rule("api.github.com", Action::Block);
        let (store, _) = RulesStore::load(&dir.path().join("rules.yaml"), &[bundled], session);
        store
            .add(
                &rule("api.github.com", Action::Allow).with_expiry(Expiry::Session(session)),
                None,
            )
            .expect("add");

        let host = HostName::parse("api.github.com").expect("host");
        let key = RequestKey::new(&host, &Method::GET, "/repos", Scheme::Https, 443);
        let verdict = store.effective().evaluate(&key, Utc::now(), session);
        assert_eq!(verdict.action(), Action::Allow, "{verdict:?}");
    }

    #[test]
    fn a_broken_file_keeps_the_rules_that_are_in_force() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (store, _) = store(&dir);
        store
            .add(&rule("api.github.com", Action::Allow), None)
            .expect("add");
        let before = store.effective();

        std::fs::write(store.path(), "version: 1\nrules:\n  - action: nonsense\n").expect("write");
        let diagnostics = store.reload();

        assert!(
            diagnostics
                .iter()
                .any(|d| d.severity >= humanitl_core::Severity::Error),
            "{diagnostics:?}"
        );
        assert_eq!(store.effective(), before);
        assert_eq!(store.list().len(), 1);
    }

    #[test]
    fn reload_reports_what_changed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (store, _) = store(&dir);
        let kept = store
            .add(&rule("api.github.com", Action::Allow), None)
            .expect("add");
        store
            .add(&rule("registry.npmjs.org", Action::Allow), None)
            .expect("add");

        // Die Datei von Hand auf eine Regel kürzen.
        let text = format!(
            "version: 1\nrules:\n  - id: {}\n    action: block\n    match:\n      host: api.github.com\n",
            kept.id
        );
        std::fs::write(store.path(), text).expect("write");

        let diagnostics = store.reload();
        let first = diagnostics.first().expect("a report");
        assert_eq!(first.code.as_str(), "RULES_011");
        assert!(first.why.contains("1 removed"), "{}", first.why);
        assert!(first.why.contains("1 changed"), "{}", first.why);
        assert_eq!(store.list().len(), 1);
    }

    #[test]
    fn an_unwritable_directory_leaves_neither_file_nor_memory_changed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let closed = dir.path().join("closed");
        std::fs::create_dir(&closed).expect("mkdir");
        std::fs::set_permissions(&closed, std::fs::Permissions::from_mode(0o500)).expect("chmod");
        let (store, _) = RulesStore::load(&closed.join("rules.yaml"), &[], SessionId::new());

        let error = store
            .add(&rule("api.github.com", Action::Allow), None)
            .expect_err("the write must fail");
        assert_eq!(error.code.as_str(), "RULES_009");
        assert!(store.list().is_empty(), "a failed write changes nothing");
        assert_eq!(store.revision(), 1, "a failed write is no revision");

        std::fs::set_permissions(&closed, std::fs::Permissions::from_mode(0o700)).expect("chmod");
    }

    #[test]
    fn making_a_session_rule_permanent_moves_it_into_the_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (store, session) = store(&dir);
        let added = store
            .add(
                &rule("api.github.com", Action::Allow).with_expiry(Expiry::Session(session)),
                None,
            )
            .expect("add");

        store.make_permanent(added.id).expect("make permanent");

        assert_eq!(store.get(added.id).expect("stored").origin, Origin::User);
        let text = std::fs::read_to_string(store.path()).expect("read");
        assert!(text.contains("expires: never"), "{text}");
        assert!(!text.contains("session"), "{text}");
    }

    #[test]
    fn positions_count_inside_the_group() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (store, session) = store(&dir);
        let first = store
            .add(&rule("a.example.com", Action::Allow), None)
            .expect("add");
        let second = store
            .add(&rule("b.example.com", Action::Allow), Some(0))
            .expect("add");
        let scoped = store
            .add(
                &rule("c.example.com", Action::Allow).with_expiry(Expiry::Session(session)),
                None,
            )
            .expect("add");

        let list = store.list();
        let position = |id| {
            list.iter()
                .find(|stored| stored.rule.id == id)
                .map(|stored| (stored.origin, stored.position))
        };
        assert_eq!(position(scoped.id), Some((Origin::Session, 1)));
        assert_eq!(position(second.id), Some((Origin::User, 1)));
        assert_eq!(position(first.id), Some((Origin::User, 2)));
    }

    #[test]
    fn an_expired_rule_disappears_from_file_and_memory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (store, _) = store(&dir);
        let gone = store
            .add(
                &rule("a.example.com", Action::Allow)
                    .with_expiry(Expiry::At(Utc::now() - Duration::seconds(1))),
                None,
            )
            .expect("add");
        store
            .add(&rule("b.example.com", Action::Allow), None)
            .expect("add");

        assert_eq!(store.prune().expect("prune"), 1);
        assert!(store.get(gone.id).is_none());
        let text = std::fs::read_to_string(store.path()).expect("read");
        assert!(!text.contains("a.example.com"), "{text}");
    }

    #[test]
    fn a_rule_the_engine_refuses_never_reaches_the_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (store, _) = store(&dir);
        let mut broken = rule("api.github.com", Action::Allow);
        broken.matcher.path = Some(humanitl_core::rule::PathPattern::Regex("([".to_owned()));

        let error = store
            .add(&broken, None)
            .expect_err("a broken regex is refused");
        assert_eq!(error.code.as_str(), "RULES_005");
        assert!(error.why.contains("line"), "{}", error.why);
        assert!(store.list().is_empty());
        assert!(!store.path().exists());
    }

    #[test]
    fn a_verdict_of_the_snapshot_changes_with_every_write() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (store, session) = store(&dir);
        let snapshot = store.snapshot();
        let host = HostName::parse("api.github.com").expect("host");
        let key = RequestKey::new(&host, &Method::GET, "/repos", Scheme::Https, 443);

        let read = |snapshot: &std::sync::Arc<std::sync::RwLock<humanitl_rules::RuleSet>>| {
            snapshot.read().map_or(Verdict::Default, |set| {
                set.evaluate(&key, Utc::now(), session)
            })
        };
        assert_eq!(read(&snapshot), Verdict::Default);

        store
            .add(&rule("api.github.com", Action::Block), None)
            .expect("add");
        assert_eq!(read(&snapshot).action(), Action::Block);
    }
}
