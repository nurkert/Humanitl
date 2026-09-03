//! Der Regel-RPC: lesen, ändern, probelaufen, neu laden (HUM-027).
//!
//! Hier steht die Verdrahtung zwischen dem Vertrag (`RulesRequest`,
//! `RulesResponse`) und dem [`RulesStore`] des Proxys. Die Fachlichkeit liegt
//! nicht hier: was eine Regel ist, sagt `humanitl-core`, ob sie gültig ist,
//! sagt `humanitl-rules`, und wohin sie geschrieben wird, entscheidet der
//! Speicher. Dieses Modul liest die Anfrage, ruft genau eine Methode und baut
//! die Antwort.
//!
//! # Was jede Antwort trägt
//!
//! `rules` ist immer der vollständige Regelsatz **nach** der Operation, in
//! Auswertungsreihenfolge. Ein Client muss nach einer Änderung nichts
//! nachladen, und eine Oberfläche, die zwei Änderungen kurz hintereinander
//! schickt, sieht am Ende denselben Stand wie der Daemon.
//!
//! # Probelauf
//!
//! `dry_run` ändert nichts. Er baut aus jedem der letzten `limit`
//! aufgezeichneten Flows einen [`RequestKey`] und wertet **nur** die
//! übergebene Regel aus. Das Ergebnis ist deshalb „diese Regel hätte hier
//! gegriffen", nicht „so wäre entschieden worden": ob eine frühere Regel
//! zuerst getroffen hätte, hängt an ihrer Position, und die wählt der Mensch
//! erst beim Anlegen.

use std::sync::Arc;

use humanitl_core::diagnostics::codes;
use humanitl_core::rule::Rule;
use humanitl_core::{Diagnostic, HostName, Method, RuleId, Scheme, SessionId, Severity, Upgrade};
use humanitl_proxy::rules_store::RulesStore;
use humanitl_recorder::{FlowQuery, Recorder};
use humanitl_rules::{RequestKey, RuleSet, Verdict};

use crate::convert;
use crate::v1;

/// Vorgabe für `RulesRequest.DryRun.limit`, wie im Vertrag beschrieben.
pub const DEFAULT_DRY_RUN_SCAN: u32 = 500;

/// Alles, was der Regel-RPC braucht.
///
/// Der Recorder ist optional: ohne ihn gibt es keine aufgezeichneten Flows,
/// gegen die ein Probelauf laufen könnte. Er antwortet dann mit null geprüften
/// Flows statt mit einer erfundenen Liste.
#[derive(Clone)]
pub struct RulesService {
    store: Arc<RulesStore>,
    recorder: Option<Recorder>,
}

impl core::fmt::Debug for RulesService {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RulesService")
            .field("path", &self.store.path())
            .field("recorder", &self.recorder.is_some())
            .finish_non_exhaustive()
    }
}

impl RulesService {
    /// Der Dienst über einem Regelspeicher.
    #[must_use]
    pub const fn new(store: Arc<RulesStore>, recorder: Option<Recorder>) -> Self {
        Self { store, recorder }
    }

    /// Der Regelspeicher, den dieser Dienst bedient.
    #[must_use]
    pub fn store(&self) -> &Arc<RulesStore> {
        &self.store
    }

    /// Die Sitzung, der `expires: session` gehört.
    #[must_use]
    pub fn session(&self) -> SessionId {
        self.store.session()
    }

    /// Führt eine Operation aus und baut die Antwort.
    ///
    /// # Errors
    ///
    /// [`Diagnostic`], wenn die Anfrage als Ganzes nichts bewirkt: keine
    /// Operation (`IPC_005`), eine unlesbare oder abgelehnte Regel
    /// (`RULES_00x`), eine unbekannte Id (`IPC_005`), eine mitgelieferte Regel
    /// (`RULES_010`) oder eine Datei, die sich nicht schreiben ließ
    /// (`RULES_009`). Ein `reload`, dessen Datei die Engine ablehnt, ist kein
    /// Fehler des Aufrufs: die Befunde stehen in der Antwort, und es gelten
    /// weiter die Regeln von vorher.
    pub async fn apply(&self, request: v1::RulesRequest) -> Result<v1::RulesResponse, Diagnostic> {
        let mut diagnostics = Vec::new();
        let mut dry_run = None;

        match request.op {
            None => return Err(no_op()),
            Some(v1::rules_request::Op::List(())) => {}
            Some(v1::rules_request::Op::Add(rule)) => {
                let position = position_of(&rule);
                let rule = self.read_rule(&rule)?;
                self.store.add(&rule, position)?;
            }
            Some(v1::rules_request::Op::Update(rule)) => {
                let rule = self.read_rule(&rule)?;
                self.store.update(&rule)?;
            }
            Some(v1::rules_request::Op::Remove(id)) => {
                self.store.remove(rule_id(&id)?)?;
            }
            Some(v1::rules_request::Op::Reorder(order)) => {
                let mut ids = Vec::with_capacity(order.rule_ids_in_order.len());
                for id in &order.rule_ids_in_order {
                    ids.push(rule_id(id)?);
                }
                self.store.reorder_all(&ids)?;
            }
            Some(v1::rules_request::Op::MakePermanent(id)) => {
                self.store.make_permanent(rule_id(&id)?)?;
            }
            Some(v1::rules_request::Op::Reload(())) => {
                diagnostics = self.store.reload();
            }
            Some(v1::rules_request::Op::DryRun(request)) => {
                let rule = request.rule.as_ref().ok_or_else(|| {
                    Diagnostic::builder(codes::IPC_005, Severity::Error)
                        .why("dry_run came without a rule".to_owned())
                        .build()
                })?;
                let rule = self.read_rule(rule)?;
                dry_run = Some(self.dry_run(&rule, request.limit).await);
            }
        }

        Ok(self.response(&diagnostics, dry_run))
    }

    /// Legt die Regel aus `DecideRequest.remember` an.
    ///
    /// Wird vor dem Entscheiden aufgerufen: scheitert das Anlegen, wird nicht
    /// entschieden (`backlog/sprint-2.md`, HUM-027).
    ///
    /// # Errors
    ///
    /// Wie [`RulesService::apply`] für `add`.
    pub fn remember(&self, rule: &v1::Rule) -> Result<v1::Rule, Diagnostic> {
        let parsed = self.read_rule(rule)?;
        let added = self.store.add(&parsed, position_of(rule))?;
        let stored = self.store.get(added.id).map_or_else(
            || convert::rule_to_proto(&added),
            |stored| convert::stored_rule_to_proto(&stored),
        );
        Ok(stored)
    }

    /// Nimmt eine gerade angelegte Regel zurück.
    ///
    /// Gebraucht wird das an genau einer Stelle: `Decide` legt die Regel aus
    /// `remember` an, bevor es entscheidet, und wenn danach kein einziger Flow
    /// entschieden werden konnte, hat der Aufruf nichts bewirkt — dann soll
    /// auch die Regel nicht bleiben. Scheitert das Zurücknehmen, bleibt es
    /// beim Befund des eigentlichen Fehlers; er ist der, den der Mensch sehen
    /// muss.
    pub fn forget(&self, id: RuleId) {
        if let Err(diagnostic) = self.store.remove(id) {
            tracing::warn!(rule = %id, why = %diagnostic.why, "could not roll back a remembered rule");
        }
    }

    /// Die vollständige Antwort: alle Regeln, Befunde, Probelauf.
    fn response(
        &self,
        diagnostics: &[Diagnostic],
        dry_run: Option<(Vec<v1::FlowSummary>, u32)>,
    ) -> v1::RulesResponse {
        let (matches, scanned) = dry_run.unwrap_or_default();
        let wire: Vec<v1::Diagnostic> = diagnostics
            .iter()
            .map(convert::diagnostic_to_proto)
            .collect();
        v1::RulesResponse {
            rules: self
                .store
                .list()
                .iter()
                .map(convert::stored_rule_to_proto)
                .collect(),
            dry_run_matches: matches,
            diagnostic: wire.first().cloned(),
            diagnostics: wire,
            dry_run_scanned: scanned,
        }
    }

    /// Liest eine Regel von der Leitung.
    fn read_rule(&self, rule: &v1::Rule) -> Result<Rule, Diagnostic> {
        convert::rule_from_proto(rule, self.session()).map_err(|error| {
            let code = match error {
                convert::RuleError::Host(_) => codes::RULES_003,
                _ => codes::IPC_005,
            };
            Diagnostic::builder(code, Severity::Error)
                .why(format!("the rule is not readable: {error}"))
                .build()
        })
    }

    /// Der Probelauf gegen die letzten aufgezeichneten Flows.
    ///
    /// Liefert die Treffer und die Zahl der geprüften Flows. Ohne Aufzeichnung
    /// sind beide leer beziehungsweise null: was der Daemon nicht weiß, wird
    /// nicht geschätzt (`backlog/CONVENTIONS.md` 4.13).
    async fn dry_run(&self, rule: &Rule, limit: u32) -> (Vec<v1::FlowSummary>, u32) {
        let Some(recorder) = self.recorder.as_ref() else {
            return (Vec::new(), 0);
        };
        let query = FlowQuery {
            limit: match limit {
                0 => DEFAULT_DRY_RUN_SCAN,
                other => other,
            },
            ..FlowQuery::default()
        };
        let page = match recorder.list_flows(&query).await {
            Ok(page) => page,
            Err(error) => {
                tracing::warn!(why = %error, "dry run could not read the recorded flows");
                return (Vec::new(), 0);
            }
        };
        let scanned = u32::try_from(page.rows.len()).unwrap_or(u32::MAX);
        let set = RuleSet::from_rules([rule.clone()]);
        let now = chrono::Utc::now();
        let session = self.session();
        let hits = page
            .rows
            .iter()
            .filter(|row| would_hit(&set, row, now, session))
            .map(convert::recorded_summary_to_proto)
            .collect();
        (hits, scanned)
    }
}

/// Wahr, wenn die Regel diesen aufgezeichneten Flow getroffen hätte.
///
/// Ein Flow, dessen Host, Methode oder Schema sich nicht mehr lesen lässt,
/// zählt als „nicht getroffen": ein Probelauf, der raten müsste, verspräche
/// mehr, als er weiß.
fn would_hit(
    set: &RuleSet,
    row: &humanitl_recorder::FlowSummary,
    now: chrono::DateTime<chrono::Utc>,
    session: SessionId,
) -> bool {
    let Ok(host) = HostName::parse(&row.host) else {
        return false;
    };
    let Ok(method) = Method::from_bytes(row.method.as_bytes()) else {
        return false;
    };
    let Some(scheme) = Scheme::parse(&row.scheme) else {
        return false;
    };
    let mut key = RequestKey::new(&host, &method, &row.path, scheme, row.port);
    if row.upgrade.is_some() {
        key = key.with_upgrade(Upgrade::WebSocket);
    }
    matches!(set.evaluate(&key, now, session), Verdict::Matched { .. })
}

/// Die gewünschte Position aus einer Regel der Leitung.
///
/// Der Vertrag zählt 1-basiert und kennt `0` als „ans Ende"
/// (`proto/humanitl/v1/rules.proto`); der Speicher zählt 0-basiert.
fn position_of(rule: &v1::Rule) -> Option<usize> {
    match rule.position {
        0 => None,
        other => Some(
            usize::try_from(other)
                .unwrap_or(usize::MAX)
                .saturating_sub(1),
        ),
    }
}

/// Liest eine Regel-Id von der Leitung.
fn rule_id(text: &str) -> Result<RuleId, Diagnostic> {
    RuleId::parse(text).map_err(|error| {
        Diagnostic::builder(codes::IPC_005, Severity::Error)
            .why(format!("{text:?} is not a rule id: {error}"))
            .build()
    })
}

/// Der Befund für eine Anfrage ohne Operation.
fn no_op() -> Diagnostic {
    Diagnostic::builder(codes::IPC_005, Severity::Error)
        .why("rules came without an operation; there is no default".to_owned())
        .build()
}

/// Der Befund für einen Daemon, der ohne Regelspeicher läuft.
///
/// Kein `UNIMPLEMENTED`: der RPC gibt es, dieser Daemon hat nur keinen Ort für
/// Regeln. Der Unterschied ist für den Client wichtig, weil das eine sich mit
/// einem Update ändert und das andere mit dem Start.
#[must_use]
pub fn no_store() -> Diagnostic {
    Diagnostic::builder(codes::IPC_005, Severity::Error)
        .why("this daemon runs without a rule store; rules cannot be read or changed".to_owned())
        .build()
}
