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

use humanitl_audit::kinds::{RuleChange, RuleOrigin};
use humanitl_audit::{AuditHandle, RecordKind};
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
    /// Wohin jede Änderung als `rule.added`, `rule.updated` oder
    /// `rule.removed` geht (HUM-050). `None` im Fake und in Tests.
    audit: Option<AuditHandle>,
}

impl core::fmt::Debug for RulesService {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RulesService")
            .field("path", &self.store.path())
            .field("recorder", &self.recorder.is_some())
            .field("audit", &self.audit.is_some())
            .finish_non_exhaustive()
    }
}

impl RulesService {
    /// Der Dienst über einem Regelspeicher.
    #[must_use]
    pub const fn new(store: Arc<RulesStore>, recorder: Option<Recorder>) -> Self {
        Self {
            store,
            recorder,
            audit: None,
        }
    }

    /// Derselbe Dienst, der jede Änderung ins Audit-Log schreibt (HUM-050).
    ///
    /// Vermerkt wird, was sich an einer Regel geändert hat: Anlegen, Ändern,
    /// Dauerhaft-Machen, Ab- und Anschalten einer mitgelieferten Regel,
    /// Löschen. Nicht vermerkt werden `reorder` (die Regeln selbst bleiben,
    /// wie sie sind) und `reload` (die Datei hat jemand von Hand geändert; was
    /// sich darin geändert hat, sagt der Speicher nicht einzeln).
    #[must_use]
    pub fn with_audit(mut self, audit: AuditHandle) -> Self {
        self.audit = Some(audit);
        self
    }

    /// Schreibt eine Änderung ins Audit-Log, sofern eines verdrahtet ist.
    fn audit(&self, kind: fn(RuleChange) -> RecordKind, rule: &Rule, origin: RuleOrigin) {
        if let Some(audit) = &self.audit {
            audit.record(
                Some(self.store.session()),
                kind(RuleChange::new(rule, origin)),
            );
        }
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
        let mut test = None;

        // Die Prüfung „ohne Operation ist keine Anfrage" steht in
        // [`crate::validate`] und gilt für den Fake genauso.
        crate::validate::rules_op(&request)?;
        match request.op {
            None | Some(v1::rules_request::Op::List(())) => {}
            // Der Ursprung ist `rpc`: Oberfläche und Kommandozeile schicken
            // dieselbe Nachricht, und keine weist sich aus.
            Some(v1::rules_request::Op::Add(rule)) => {
                let position = position_of(&rule);
                let rule = self.read_rule(&rule)?;
                let added = self.store.add(&rule, position)?;
                self.audit(RecordKind::RuleAdded, &added, RuleOrigin::Rpc);
            }
            Some(v1::rules_request::Op::Update(rule)) => {
                let rule = self.read_rule(&rule)?;
                let updated = self.store.update(&rule)?;
                self.audit(RecordKind::RuleUpdated, &updated, RuleOrigin::Rpc);
            }
            Some(v1::rules_request::Op::Remove(id)) => {
                let removed = self.store.remove(rule_id(&id)?)?;
                self.audit(RecordKind::RuleRemoved, &removed, RuleOrigin::Rpc);
            }
            Some(v1::rules_request::Op::Reorder(order)) => {
                let mut ids = Vec::with_capacity(order.rule_ids_in_order.len());
                for id in &order.rule_ids_in_order {
                    ids.push(rule_id(id)?);
                }
                self.store.reorder_all(&ids)?;
            }
            Some(v1::rules_request::Op::MakePermanent(id)) => {
                let permanent = self.store.make_permanent(rule_id(&id)?)?;
                self.audit(RecordKind::RuleUpdated, &permanent, RuleOrigin::Rpc);
            }
            Some(v1::rules_request::Op::Reload(())) => {
                diagnostics = self.store.reload();
            }
            Some(v1::rules_request::Op::SetDisabled(request)) => {
                let switched = self
                    .store
                    .set_bundled_disabled(rule_id(&request.rule_id)?, request.disabled)?;
                self.audit(RecordKind::RuleUpdated, &switched, RuleOrigin::Rpc);
            }
            Some(v1::rules_request::Op::Test(probe)) => {
                test = Some(self.test(&probe)?);
            }
            Some(v1::rules_request::Op::DryRun(request)) => {
                let rule = self.read_rule(crate::validate::dry_run_rule(&request)?)?;
                dry_run = Some(self.dry_run(&rule, request.limit, &mut diagnostics).await);
            }
        }

        Ok(self.response(&diagnostics, dry_run, test))
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
        self.audit(RecordKind::RuleAdded, &added, RuleOrigin::Remember);
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
        match self.store.remove(id) {
            Ok(removed) => self.audit(RecordKind::RuleRemoved, &removed, RuleOrigin::Remember),
            Err(diagnostic) => {
                tracing::warn!(rule = %id, why = %diagnostic.why, "could not roll back a remembered rule");
            }
        }
    }

    /// Die vollständige Antwort: alle Regeln, Befunde, Probelauf.
    fn response(
        &self,
        diagnostics: &[Diagnostic],
        dry_run: Option<(Vec<v1::FlowSummary>, u32)>,
        test: Option<v1::RuleTest>,
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
            test,
        }
    }

    /// Fragt den geltenden Regelsatz, was er zu einer Anfrage sagt.
    ///
    /// Dieselbe Auswertung wie im Proxy-Pfad, mit demselben `RuleSet` und
    /// derselben Uhr; geändert wird nichts. Sie liegt hier und nicht in der
    /// Kommandozeile, weil es genau eine Auswertung geben darf: Eine zweite
    /// könnte anders antworten als die, die tatsächlich entscheidet (ADR-018).
    ///
    /// # Errors
    ///
    /// `IPC_005`, wenn Methode oder URL nicht lesbar sind. Geraten wird nichts:
    /// Eine Probe, die auf eine andere Anfrage antwortet als die gemeinte, wäre
    /// schlimmer als keine.
    fn test(&self, probe: &v1::rules_request::Test) -> Result<v1::RuleTest, Diagnostic> {
        let (method, scheme, authority, path) = crate::validate::rule_probe(probe)?;
        let mut key = RequestKey::new(&authority.host, &method, &path, scheme, authority.port);
        if v1::Upgrade::try_from(probe.upgrade) == Ok(v1::Upgrade::Websocket) {
            key = key.with_upgrade(Upgrade::WebSocket);
        }
        let verdict = self
            .store
            .effective()
            .evaluate(&key, chrono::Utc::now(), self.session());
        let rule = verdict.rule().and_then(|id| self.store.get(id));
        Ok(v1::RuleTest {
            action: convert::action_to_proto(verdict.action()) as i32,
            matched: matches!(verdict, Verdict::Matched { .. }),
            rule_id: verdict.rule().map(|id| id.to_string()).unwrap_or_default(),
            position: rule.map_or(0, |stored| stored.position),
        })
    }

    /// Liest eine Regel von der Leitung.
    fn read_rule(&self, rule: &v1::Rule) -> Result<Rule, Diagnostic> {
        crate::validate::rule(rule, self.session())
    }

    /// Der Probelauf gegen die letzten aufgezeichneten Flows.
    ///
    /// Liefert die Treffer und die Zahl der geprüften Flows. Ohne Aufzeichnung
    /// sind beide leer beziehungsweise null: was der Daemon nicht weiß, wird
    /// nicht geschätzt (`backlog/CONVENTIONS.md` 4.13).
    async fn dry_run(
        &self,
        rule: &Rule,
        limit: u32,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> (Vec<v1::FlowSummary>, u32) {
        let Some(recorder) = self.recorder.as_ref() else {
            diagnostics.push(
                Diagnostic::builder(codes::RULES_012, Severity::Info)
                    .why(
                        "This session records nothing, so there is no traffic to try the rule against."
                            .to_owned(),
                    )
                    .build(),
            );
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
                diagnostics.push(
                    Diagnostic::builder(codes::RULES_012, Severity::Warning)
                        .why(format!(
                            "The recorded requests could not be read, so the rule was tried against nothing: {error}"
                        ))
                        .build(),
                );
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::sync::Arc;

    use humanitl_audit::{AuditKey, AuditRecord, AuditWriter, KeyOrigin, WriterOptions};
    use humanitl_core::{RuleId, SessionId};

    use super::{RulesService, RulesStore};
    use crate::v1;

    /// Eine Regel für diese Sitzung; sie bleibt im Speicher und braucht keine
    /// Datei.
    fn session_rule(host: &str) -> v1::Rule {
        v1::Rule {
            action: v1::RuleAction::Allow as i32,
            matcher: Some(v1::RuleMatcher {
                host: host.to_owned(),
                path: "/geheim/**".to_owned(),
                ..v1::RuleMatcher::default()
            }),
            expires: Some(v1::RuleExpiry {
                expiry: Some(v1::rule_expiry::Expiry::Session(())),
            }),
            ..v1::Rule::default()
        }
    }

    /// Jede Änderung über den Regel-RPC und über `remember` steht im
    /// Audit-Log, mit ihrem Ursprung und ihrer Sitzung; das Pfadmuster steht
    /// nicht darin (HUM-050).
    #[tokio::test]
    async fn every_rule_change_is_an_audit_record() {
        let dir = tempfile::tempdir().unwrap();
        let key = AuditKey::from_bytes([2; 32], KeyOrigin::File);
        let (writer, _) = AuditWriter::open(
            &dir.path().join("audit.jsonl"),
            &key,
            WriterOptions::default(),
            &[],
            None,
        )
        .unwrap();
        let session = SessionId::new();
        let service = RulesService::new(Arc::new(RulesStore::in_memory(session)), None)
            .with_audit(writer.handle());

        let response = service
            .apply(v1::RulesRequest {
                op: Some(v1::rules_request::Op::Add(session_rule("audit.example"))),
            })
            .await
            .unwrap();
        let id = response
            .rules
            .iter()
            .find(|rule| {
                rule.matcher
                    .as_ref()
                    .is_some_and(|matcher| matcher.host == "audit.example")
            })
            .expect("the added rule is listed")
            .rule_id
            .clone();
        service
            .apply(v1::RulesRequest {
                op: Some(v1::rules_request::Op::Remove(id.clone())),
            })
            .await
            .unwrap();
        let remembered = service.remember(&session_rule("remember.example")).unwrap();
        service.forget(RuleId::parse(&remembered.rule_id).unwrap());

        writer.handle().sync().unwrap();
        let text = std::fs::read_to_string(writer.path()).unwrap();
        drop(writer);
        assert!(
            !text.contains("/geheim"),
            "the path pattern stays out: {text}"
        );
        let records: Vec<AuditRecord> = text
            .lines()
            .map(|line| AuditRecord::from_line(line.as_bytes()).unwrap())
            .collect();
        let seen: Vec<(&str, &str, &str)> = records
            .iter()
            .map(|record| {
                (
                    record.body.kind.as_str(),
                    record.body.data["origin"].as_str().unwrap(),
                    record.body.data["match_host"].as_str().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            seen,
            [
                ("rule.added", "rpc", "audit.example"),
                ("rule.removed", "rpc", "audit.example"),
                ("rule.added", "remember", "remember.example"),
                ("rule.removed", "remember", "remember.example"),
            ]
        );
        assert_eq!(records[0].body.data["rule"], id.as_str());
        assert_eq!(records[0].body.data["expires"], "session");
        let owner = session.to_string();
        assert!(records.iter().all(|record| record.body.session == owner));
    }
}
