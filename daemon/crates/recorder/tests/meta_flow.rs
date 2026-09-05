//! Der Meta-Fluss in der Aufzeichnung (HUM-103).
//!
//! Eine Anfrage an den reservierten Namen `humanitl.internal` beantwortet der
//! Proxy selbst. Sie geht nirgendwo hin, sie hält nichts auf, und über sie
//! entscheidet niemand. Sie steht trotzdem in der Historie — als das, was sie
//! ist: eine Auskunft, die der Agent sich geholt hat.
//!
//! Geprüft wird beides, und beides gehört zusammen:
//!
//! 1. Der Datensatz trägt den Vermerk `meta` und **keine** Entscheidung.
//! 2. Der Filter teilt die Historie mit `meta:true` und `meta:false`
//!    vollständig und überschneidungsfrei, und keine Auswertung über
//!    Entscheidungen zählt einen Meta-Fluss mit.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::{Duration, Instant, SystemTime};

use humanitl_core::{
    Authority, BlockReason, Decision, DecisionSource, Flow, FlowEvent, FlowId, HostName,
    HttpRequest, META_HOST, Method, Scheme, SessionId,
};
use humanitl_recorder::{FlowQuery, FlowSummary, Recorder, RecorderSettings, SessionMeta};

/// Eine Aufzeichnung in einem eigenen Temp-Verzeichnis.
struct Harness {
    _dir: tempfile::TempDir,
    recorder: Recorder,
    session: SessionId,
}

impl Harness {
    fn open() -> Self {
        let dir = tempfile::tempdir().unwrap_or_else(|err| panic!("{err}"));
        let db = dir.path().join("data").join("humanitl.db");
        let blobs = dir.path().join("data").join("blobs");
        let recorder = Recorder::open(&db, &blobs, RecorderSettings::default())
            .unwrap_or_else(|err| panic!("{err}"));
        let session = SessionId::new();
        recorder.start_session(&SessionMeta {
            id: session,
            started_at: SystemTime::now(),
            sandbox_profile: "default".to_owned(),
            llm_endpoint: None,
            work_dir: "/home/x/projekt".to_owned(),
            agent: "opencode".to_owned(),
        });
        Self {
            _dir: dir,
            recorder,
            session,
        }
    }

    /// Die Zeilen, die dieser Filter trifft, jüngste zuerst.
    async fn rows(&self, filter: &str) -> Vec<FlowSummary> {
        self.recorder.flush().await;
        self.recorder
            .list_flows(&FlowQuery::new(filter))
            .await
            .unwrap_or_else(|err| panic!("{filter}: {err}"))
            .rows
    }
}

fn request(host: &str, path: &str) -> HttpRequest {
    let host = HostName::parse(host).unwrap_or_else(|err| panic!("{err}"));
    HttpRequest::new(
        Method::GET,
        Scheme::Https,
        Authority::with_scheme(host, Scheme::Https),
        path,
    )
}

/// Ein Meta-Fluss, genau auf dem Weg, den der Proxy nimmt.
///
/// Über `Flow::answer` und nicht am Automaten vorbei: Fiele der Übergang
/// `Received → Recorded` weg oder verlöre er seine Bindung an den Fluss,
/// schlüge schon `answer` fehl.
fn meta_flow(harness: &Harness, path: &str, status: u16) -> FlowId {
    let mut flow = Flow::new(
        FlowId::new(),
        harness.session,
        SystemTime::now(),
        request(META_HOST, path),
    );
    let event = flow
        .answer(SystemTime::now())
        .unwrap_or_else(|err| panic!("{path}: {err}"));
    harness.recorder.apply(&flow.received_event());
    harness.recorder.apply(&event);
    harness.recorder.set_meta_answer(flow.id, status);
    flow.id
}

/// Ein gewöhnlicher Fluss mit einer Entscheidung.
fn decided_flow(harness: &Harness, host: &str, decision: Decision, status: u16) -> FlowId {
    let flow = FlowId::new();
    let at = SystemTime::now();
    harness.recorder.apply(&FlowEvent::Received {
        flow_id: flow,
        at,
        request: Box::new(request(host, "/user")),
    });
    harness.recorder.apply(&FlowEvent::Analyzed {
        flow_id: flow,
        at,
        findings: Vec::new(),
    });
    harness.recorder.apply(&FlowEvent::Held {
        flow_id: flow,
        at,
        deadline: Instant::now() + Duration::from_secs(300),
        queue_bytes: 0,
        queue_count: 1,
    });
    harness.recorder.apply(&FlowEvent::Decided {
        flow_id: flow,
        at,
        decision,
        source: DecisionSource::User,
    });
    harness.recorder.apply(&FlowEvent::ResponseHeaders {
        flow_id: flow,
        at,
        status,
    });
    harness
        .recorder
        .apply(&FlowEvent::Recorded { flow_id: flow, at });
    flow
}

/// Legt drei Meta-Flüsse und zwei entschiedene an.
async fn populated() -> (Harness, Vec<FlowId>, Vec<FlowId>) {
    let harness = Harness::open();
    let meta = vec![
        meta_flow(&harness, "/", 200),
        meta_flow(&harness, "/why/0199c0ff-ee00-7000-8000-8000deadbeef", 404),
        meta_flow(&harness, "/ask", 202),
    ];
    let ordinary = vec![
        decided_flow(&harness, "api.github.com", Decision::Allow, 200),
        decided_flow(
            &harness,
            "pypi.org",
            Decision::Block {
                reason: BlockReason::User,
                note: None,
            },
            403,
        ),
    ];
    harness.recorder.flush().await;
    (harness, meta, ordinary)
}

/// Drei Meta-Anfragen ergeben drei Einträge, unterscheidbar von Entscheidungen.
#[tokio::test]
async fn three_meta_requests_are_three_entries_without_a_decision() {
    let (harness, meta, _ordinary) = populated().await;

    let all = harness.rows("").await;
    assert_eq!(all.len(), 5, "the history shows what really happened");

    let rows = harness.rows("meta:true").await;
    assert_eq!(rows.len(), 3);
    let mut paths: Vec<&str> = rows.iter().map(|row| row.path.as_str()).collect();
    paths.sort_unstable();
    assert_eq!(
        paths,
        vec!["/", "/ask", "/why/0199c0ff-ee00-7000-8000-8000deadbeef"]
    );

    for row in &rows {
        assert!(row.meta, "{} carries the mark", row.path);
        assert_eq!(
            row.decision, None,
            "nobody decided about {}; a decision here would be a claim about a person who did \
             nothing",
            row.path
        );
        assert_eq!(row.block_reason, None, "{}", row.path);
        assert_eq!(row.rule_id, None, "{}", row.path);
        assert_eq!(row.state, "recorded", "{}", row.path);
        assert_eq!(row.host, META_HOST, "{}", row.path);
        assert!(row.duration_ms.is_some(), "{}", row.path);
        assert_eq!(row.held_ms, None, "a meta request is never held");
    }

    // Der Status ist der, den der Proxy selbst geschrieben hat.
    let mut statuses: Vec<u16> = rows.iter().filter_map(|row| row.status).collect();
    statuses.sort_unstable();
    assert_eq!(statuses, vec![200, 202, 404]);

    let ids: Vec<FlowId> = rows.iter().map(|row| row.id).collect();
    for id in &meta {
        assert!(ids.contains(id), "{id} is in the history");
    }
}

/// `meta:true` und `meta:false` teilen die Historie vollständig und
/// überschneidungsfrei.
#[tokio::test]
async fn meta_true_and_meta_false_split_the_history() {
    let (harness, meta, ordinary) = populated().await;

    let yes: Vec<FlowId> = harness
        .rows("meta:true")
        .await
        .iter()
        .map(|row| row.id)
        .collect();
    let no: Vec<FlowId> = harness
        .rows("meta:false")
        .await
        .iter()
        .map(|row| row.id)
        .collect();
    let all: Vec<FlowId> = harness.rows("").await.iter().map(|row| row.id).collect();

    assert_eq!(yes.len(), 3);
    assert_eq!(no.len(), 2);
    assert!(
        yes.iter().all(|id| !no.contains(id)),
        "no flow may be on both sides"
    );
    assert_eq!(yes.len() + no.len(), all.len(), "and none may fall through");
    for id in &meta {
        assert!(yes.contains(id));
        assert!(!no.contains(id));
    }
    for id in &ordinary {
        assert!(no.contains(id));
        assert!(!yes.contains(id));
    }
}

/// Keine Zählung über Entscheidungen ändert sich durch Meta-Flüsse.
#[tokio::test]
async fn no_count_over_decisions_sees_a_meta_flow() {
    let (harness, _meta, ordinary) = populated().await;

    for term in [
        "decision:allow",
        "decision:block",
        "decision:allow_edited",
        "decision:timed_out",
    ] {
        let rows = harness.rows(term).await;
        assert!(
            rows.iter().all(|row| !row.meta),
            "{term} must not see a meta flow"
        );
    }

    // Die beiden entschiedenen Flüsse zählen wie zuvor, je einer.
    let allow = harness.rows("decision:allow").await;
    assert_eq!(allow.len(), 1);
    assert_eq!(allow[0].id, ordinary[0]);
    let block = harness.rows("decision:block").await;
    assert_eq!(block.len(), 1);
    assert_eq!(block[0].id, ordinary[1]);
    assert_eq!(harness.rows("reason:user").await.len(), 1);

    // Und die Zählung selbst, nicht nur die Zeilen: `total_estimate` ist die
    // Zahl, die die Fußzeile der Historie zeigt.
    harness.recorder.flush().await;
    let page = harness
        .recorder
        .list_flows(&FlowQuery::new("decision:allow"))
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(page.total_estimate, 1);
    assert!(!page.capped);
}

/// Zwei Terme zusammen bleiben ein `AND`: `meta:true decision:allow` trifft
/// nichts, weil ein Meta-Fluss keine Entscheidung trägt.
#[tokio::test]
async fn meta_and_decision_never_meet() {
    let (harness, _meta, _ordinary) = populated().await;
    assert!(harness.rows("meta:true decision:allow").await.is_empty());
    assert!(harness.rows("meta:true decision:block").await.is_empty());
    assert_eq!(harness.rows("meta:false decision:allow").await.len(), 1);
}

/// Eine Datenbank aus der Zeit vor der Migration liest weiter, und ihre
/// Bestandszeilen sind keine Meta-Flüsse.
#[tokio::test]
async fn rows_from_before_the_migration_are_not_meta() {
    let harness = Harness::open();
    let flow = decided_flow(&harness, "api.github.com", Decision::Allow, 200);
    harness.recorder.flush().await;

    let rows = harness.rows("").await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, flow);
    assert!(
        !rows[0].meta,
        "the default of the new column is 0, not unknown"
    );
    assert_eq!(harness.rows("meta:false").await.len(), 1);
    assert!(harness.rows("meta:true").await.is_empty());
}
