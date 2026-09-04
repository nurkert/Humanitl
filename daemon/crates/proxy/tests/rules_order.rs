//! HUM-104: die Auswertungsreihenfolge, gemessen am echten Ladeweg.
//!
//! Diese Datei baut den Regelsatz **nicht** selbst. Sie schreibt eine
//! `rules.yaml` auf die Platte, reicht die mitgelieferten Regeln daneben und
//! lässt `RulesStore::load` den Satz zusammensetzen — denselben Weg, den
//! `load_rules` in `humanitld` geht. Der Unterschied ist der Punkt des Issues:
//! `RuleSet::prepend_bundled` hatte grüne Tests und im Daemon keinen Aufrufer,
//! die Reihenfolge im Produkt war deshalb eine andere als die gemessene.
//!
//! Geprüft werden die drei Zusagen aus `backlog/CONVENTIONS.md` 4.5:
//!
//! 1. Eine Regel des Nutzers, die alles blockt, blockt nicht das Sprachmodell.
//! 2. Eine Regel des Nutzers, die alles erlaubt, nimmt der Durchreiche nicht
//!    ihre Merkmale: `DecisionSource::Passthrough` und `LLM_005` bleiben.
//! 3. Eine Regel des Nutzers überstimmt weiterhin eine mitgelieferte
//!    (HUM-027); das darf hier nicht als Nebenwirkung verschwinden.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::sync::Arc;
use std::time::Duration;

use humanitl_core::{DecisionSource, FlowEvent, RuleId};
use humanitl_findings::FindingsSettings;
use humanitl_proxy::rules_store::Origin;
use humanitl_proxy::{Scanner, Tier1Scanner};
use hyper::StatusCode;
use support::{FakeUpstream, ProxyBuilder, body_string, get, post};

/// Ein GitHub-Token in der Form, die der Detektor kennt: `ghp_` und 36 Zeichen.
const TOKEN: &str = "ghp_A1b2C3d4E5f6G7h8I9j0K1l2M3n4O5p6Q7r8";

/// Die feste Id der Durchreiche, wie der Agent-Adapter sie vergibt.
const PASSTHROUGH_ID: &str = "01920000-0000-7000-8000-0000000000ff";

/// Die Id, unter der die Regel des Nutzers in diesen Tests steht.
const USER_RULE_ID: &str = "01920000-0000-7000-8000-0000000000a1";

/// Die Id der mitgelieferten Regel, die in Test 3 überstimmt wird.
const BUNDLED_RULE_ID: &str = "01920000-0000-7000-8000-0000000000b1";

/// Der Host des Sprachmodells. Der Mock-Resolver löst ihn auf `127.0.0.1` auf.
///
/// Ein Name und keine IP, weil ein IP-Literal nie eine Host-Regel trifft
/// (ADR-007) — eine Regel `host: "**"` des Nutzers ginge sonst an der
/// Durchreiche vorbei, ohne dass der Test etwas beweist.
const LLM_HOST: &str = "ollama.lan";

/// Die mitgelieferten Regeln einer Sitzung: nur die Durchreiche.
///
/// Wortgleich mit dem, was `OpenCodeAdapter::llm_passthrough` baut: dieser
/// Host, dieser Port, dieses Schema, `GET` und `POST`, einzelne
/// Inferenz-Endpunkte, `allow_private` und `passthrough_llm`.
fn bundled_passthrough(port: u16) -> String {
    format!(
        "version: 1\n\
         rules:\n\
         \x20 - id: {PASSTHROUGH_ID}\n\
         \x20   action: allow\n\
         \x20   match:\n\
         \x20     host: \"{LLM_HOST}\"\n\
         \x20     port: {port}\n\
         \x20     scheme: http\n\
         \x20     method: [POST, GET]\n\
         \x20     path_prefixes: [\"/v1/chat/completions\", \"/v1/models\", \"/api/chat\", \
         \"/api/tags\"]\n\
         \x20   allow_private: true\n\
         \x20   passthrough_llm: true\n\
         \x20   note: \"LLM passthrough. Logged, never held.\"\n"
    )
}

/// Die `rules.yaml` des Nutzers mit genau einer Regel über jeden Host.
fn user_rule_over_everything(action: &str) -> String {
    format!(
        "version: 1\n\
         rules:\n\
         \x20 - id: {USER_RULE_ID}\n\
         \x20   action: {action}\n\
         \x20   match:\n\
         \x20     host: \"**\"\n\
         \x20   note: \"the user decides about every host\"\n"
    )
}

/// Die echten Detektoren mit den Vorgabe-Einstellungen.
fn tier1() -> Arc<dyn Scanner> {
    Arc::new(Tier1Scanner::new(&FindingsSettings::default()).unwrap())
}

fn rule_id(text: &str) -> RuleId {
    text.parse().expect("a rule id")
}

/// Das Profil `llm-only` im Kleinen: Der Nutzer blockt jeden Host, und das
/// Sprachmodell ist trotzdem erreichbar.
///
/// Genau dieser Fall war der Fehler: Die Durchreiche stand als letzte Regel des
/// Satzes, die Blockregel traf zuerst, und `humanitl run --profile llm-only`
/// hätte sein eigenes Modell nicht erreicht.
#[tokio::test(flavor = "multi_thread")]
async fn a_user_block_over_everything_does_not_reach_the_passthrough() {
    let upstream = FakeUpstream::ollama().await;
    let port = upstream.port();
    let proxy = ProxyBuilder::new()
        .rules_store(
            &user_rule_over_everything("block"),
            &bundled_passthrough(port),
        )
        // Die Verbindung selbst darf keine privaten Ziele: Was durchkommt,
        // kommt allein wegen `allow_private` an der Durchreichregel durch.
        .allow_private(false)
        .ask(Duration::from_secs(30))
        .start()
        .await;
    let mut events = proxy.events();

    // Der Ladeweg hat die Durchreiche hinter die Regel des Nutzers gehängt.
    // Sie trifft trotzdem zuerst; genau das ist die Zusage.
    let listed = proxy
        .rules_store
        .as_ref()
        .expect("the store is wired")
        .list();
    let origins: Vec<Origin> = listed.iter().map(|stored| stored.origin).collect();
    assert_eq!(
        origins,
        vec![Origin::User, Origin::Bundled],
        "the passthrough is a bundled rule and stands behind the rule of the user"
    );

    let mut client = proxy.client().await;
    let response = client
        .send(post(
            &format!("http://{LLM_HOST}:{port}/v1/chat/completions?count=1&interval_ms=1"),
            r#"{"model":"qwen"}"#,
        ))
        .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "the declared passthrough is not shadowed by a rule of the user"
    );
    let _ = body_string(response.into_body()).await;

    let decided = events.wait_for("decided").await;
    let FlowEvent::Decided { source, .. } = decided else {
        panic!("decided is decided");
    };
    assert_eq!(
        source,
        DecisionSource::Passthrough,
        "and it is the passthrough rule that decided, not the rule of the user"
    );
    assert_eq!(events.count("held"), 0, "a passthrough is never held");

    // Die Regel des Nutzers gilt weiterhin für alles daneben — auch für einen
    // Pfad an demselben Host, den die Durchreiche nicht nennt.
    let mut client = proxy.client().await;
    let response = client
        .send(post(
            &format!("http://{LLM_HOST}:{port}/api/pull"),
            r#"{"name":"qwen"}"#,
        ))
        .await;
    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "`block **` still covers everything the passthrough does not name"
    );
}

/// Der stille Fall: Eine weite `allow`-Regel darf der Durchreiche nicht ihre
/// Merkmale nehmen.
///
/// Ohne die eigene Reihenfolge entschiede die Regel des Nutzers, der Fluss
/// trüge `DecisionSource::Rule` statt `DecisionSource::Passthrough`, und die
/// Warnung `LLM_005` bliebe aus. Der Verkehr ginge genauso hinaus — er sähe
/// nur aus wie jede andere Freigabe, und der erklärte Seitenkanal wäre in der
/// Aufzeichnung nicht mehr zu finden.
#[tokio::test(flavor = "multi_thread")]
async fn a_user_allow_over_everything_keeps_the_passthrough_visible() {
    let upstream = FakeUpstream::ollama().await;
    let port = upstream.port();
    let proxy = ProxyBuilder::new()
        .rules_store(
            &user_rule_over_everything("allow"),
            &bundled_passthrough(port),
        )
        // Die `allow`-Regel des Nutzers trägt kein `allow_private`. Käme die
        // Antwort trotzdem, hätte die Durchreiche entschieden — und die
        // Behauptungen darunter prüfen, dass sie es sichtbar tat.
        .allow_private(false)
        .recording(true)
        .scanner(tier1())
        .ask(Duration::from_secs(30))
        .start()
        .await;
    let mut events = proxy.events();

    let mut client = proxy.client().await;
    let response = client
        .send(post(
            &format!("http://{LLM_HOST}:{port}/v1/chat/completions?count=1&interval_ms=1"),
            format!(r#"{{"prompt":"here is my token {TOKEN}"}}"#),
        ))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let _ = body_string(response.into_body()).await;

    let decided = events.wait_for("decided").await;
    let FlowEvent::Decided {
        flow_id, source, ..
    } = decided
    else {
        panic!("decided is decided");
    };
    assert_eq!(
        source,
        DecisionSource::Passthrough,
        "a broad allow of the user must not turn the declared channel into an ordinary one"
    );

    events.wait_for("recorded").await;
    let warnings: Vec<_> = events
        .seen
        .iter()
        .filter(|event| match event {
            FlowEvent::Diagnostic { diagnostic, .. } => diagnostic.code.as_str() == "LLM_005",
            _ => false,
        })
        .collect();
    assert_eq!(
        warnings.len(),
        1,
        "the warning about findings on the way to the model is still raised"
    );

    // Und die Aufzeichnung zeigt den Fluss als Durchreiche, nicht als Allow
    // wie jedes andere.
    let recorder = proxy.recorder.as_ref().expect("recording is on");
    recorder.flush().await;
    let detail = recorder
        .get_flow(flow_id)
        .await
        .unwrap()
        .expect("the passthrough is in the history");
    assert!(
        detail.summary.passthrough,
        "the history has to be able to show it in amber"
    );
}

/// HUM-027 bleibt: Eine eigene Regel überstimmt eine mitgelieferte.
///
/// Mitgelieferte Regeln lassen sich nicht löschen (`RULES_010`); der Fix, den
/// dieser Befund vorschlägt, ist eine eigene Regel mit demselben Muster.
/// Stünden die mitgelieferten vorn, verspräche er etwas Unmögliches.
#[tokio::test(flavor = "multi_thread")]
async fn a_user_rule_overrides_a_bundled_rule() {
    let upstream = FakeUpstream::ollama().await;
    let port = upstream.port();
    let user = format!(
        "version: 1\n\
         rules:\n\
         \x20 - id: {USER_RULE_ID}\n\
         \x20   action: allow\n\
         \x20   match:\n\
         \x20     host: \"models.dev\"\n\
         \x20   note: \"I trust this one\"\n"
    );
    let bundled = format!(
        "version: 1\n\
         rules:\n\
         \x20 - id: {BUNDLED_RULE_ID}\n\
         \x20   action: block\n\
         \x20   match:\n\
         \x20     host: \"models.dev\"\n\
         \x20   note: \"bundled: no model catalogue\"\n"
    );
    let proxy = ProxyBuilder::new()
        .rules_store(&user, &bundled)
        .ask(Duration::from_secs(30))
        .start()
        .await;
    let mut events = proxy.events();

    let mut client = proxy.client().await;
    let response = client
        .send(get(&format!("http://models.dev:{port}/api/tags")))
        .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "the rule of the user decides, the bundled one below it does not"
    );
    let _ = body_string(response.into_body()).await;

    let decided = events.wait_for("decided").await;
    let FlowEvent::Decided { source, .. } = decided else {
        panic!("decided is decided");
    };
    assert_eq!(
        source,
        DecisionSource::Rule(rule_id(USER_RULE_ID)),
        "and it is the rule of the user that decided"
    );

    // Und der Speicher zeigt beide, in der Reihenfolge, in der er sie auswertet.
    let listed = proxy
        .rules_store
        .as_ref()
        .expect("the store is wired")
        .list();
    let origins: Vec<Origin> = listed.iter().map(|stored| stored.origin).collect();
    assert_eq!(origins, vec![Origin::User, Origin::Bundled]);
    assert!(
        listed[1].rule.bundled,
        "what comes in as bundled stays bundled, whatever the file says"
    );
}

/// Eine `rules.yaml`, die sich selbst eine Durchreiche ausstellt, bekommt den
/// ersten Rang nicht.
///
/// `passthrough_llm` steht auch in der Datei des Nutzers, und der erste Rang
/// laesst eine Anfrage ungehalten hinaus. Haenge der Rang allein an diesem
/// Feld, ueberholte eine Datei damit ihre eigenen Block-Regeln — unbemerkt,
/// denn eine Durchreiche wird nicht gehalten. Der Rang haengt deshalb am
/// Vermerk `bundled`, und den setzt nur der Lader (HUM-104).
#[tokio::test(flavor = "multi_thread")]
async fn a_passthrough_written_into_rules_yaml_does_not_outrank_the_user() {
    let upstream = FakeUpstream::ollama().await;
    let port = upstream.port();
    // Die Datei blockt alles und stellt sich danach eine Durchreiche aus,
    // samt `bundled: true`. Beides zusammen waere der erste Rang.
    let user = format!(
        "version: 1\n\
         rules:\n\
         \x20 - id: {USER_RULE_ID}\n\
         \x20   action: block\n\
         \x20   match:\n\
         \x20     host: \"**\"\n\
         \x20 - id: {PASSTHROUGH_ID}\n\
         \x20   action: allow\n\
         \x20   match:\n\
         \x20     host: \"{LLM_HOST}\"\n\
         \x20     port: {port}\n\
         \x20     scheme: http\n\
         \x20     method: [POST, GET]\n\
         \x20     path_prefixes: [\"/v1/chat/completions\"]\n\
         \x20   allow_private: true\n\
         \x20   bundled: true\n\
         \x20   passthrough_llm: true\n"
    );
    let proxy = ProxyBuilder::new()
        .rules_store(&user, "version: 1\nrules: []\n")
        .ask(Duration::from_secs(30))
        .start()
        .await;
    let mut events = proxy.events();

    let listed = proxy
        .rules_store
        .as_ref()
        .expect("the store is wired")
        .list();
    assert!(
        listed.iter().all(|stored| stored.origin == Origin::User),
        "both rules belong to the user; the file does not create a bundled group"
    );
    assert!(
        listed.iter().all(|stored| !stored.rule.bundled),
        "the mark is set on loading, not in the file"
    );

    let mut client = proxy.client().await;
    let response = client
        .send(post(
            &format!("http://{LLM_HOST}:{port}/v1/chat/completions?count=1&interval_ms=1"),
            r#"{"model":"qwen"}"#,
        ))
        .await;
    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "the block rule of the same file stands first and decides"
    );

    let decided = events.wait_for("decided").await;
    let FlowEvent::Decided { source, .. } = decided else {
        panic!("decided is decided");
    };
    assert_eq!(
        source,
        DecisionSource::Rule(rule_id(USER_RULE_ID)),
        "and it is an ordinary rule decision, not the declared side channel"
    );
    assert_ne!(source, DecisionSource::Passthrough);
}

/// Eine Sitzungsregel überstimmt eine mitgelieferte, aber nicht die
/// Durchreiche.
///
/// Der erste Teil ist die Zusage aus HUM-027 („für diese Sitzung erlauben"
/// schlägt einen mitgelieferten Block"), der zweite die Grenze davon: Auch die
/// jüngste Absicht des Menschen darf dem einen erklärten Seitenkanal nicht
/// seine Merkmale nehmen.
#[tokio::test(flavor = "multi_thread")]
async fn a_session_rule_does_not_shadow_the_passthrough() {
    let upstream = FakeUpstream::ollama().await;
    let port = upstream.port();
    let proxy = ProxyBuilder::new()
        .rules_store("version: 1\nrules: []\n", &bundled_passthrough(port))
        .allow_private(false)
        .ask(Duration::from_secs(30))
        .start()
        .await;
    let store = proxy.rules_store.as_ref().expect("the store is wired");

    // Dieselbe Regel, die die Oberfläche über den `Rules`-RPC anlegt: alles
    // erlauben, nur für diese Sitzung.
    let session_rule = humanitl_core::rule::Rule::new(
        RuleId::new(),
        humanitl_core::rule::Action::Allow,
        humanitl_core::rule::Matcher::host(
            humanitl_core::rule::HostPattern::parse("**").expect("pattern"),
        ),
    )
    .with_expiry(humanitl_core::rule::Expiry::Session(proxy.session));
    store.add(&session_rule, None).expect("the session rule");

    let mut events = proxy.events();
    let mut client = proxy.client().await;
    let response = client
        .send(post(
            &format!("http://{LLM_HOST}:{port}/v1/chat/completions?count=1&interval_ms=1"),
            r#"{"model":"qwen"}"#,
        ))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let _ = body_string(response.into_body()).await;

    let decided = events.wait_for("decided").await;
    let FlowEvent::Decided { source, .. } = decided else {
        panic!("decided is decided");
    };
    assert_eq!(
        source,
        DecisionSource::Passthrough,
        "the passthrough is checked before the rules of this session, too"
    );
    assert_eq!(
        store.list().first().map(|stored| stored.origin),
        Some(Origin::Session),
        "the session rule is the first of the list and still does not decide here"
    );
}
