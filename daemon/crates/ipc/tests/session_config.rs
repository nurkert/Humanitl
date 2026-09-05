//! Die Konfiguration je Sitzung, gemessen am Ladeweg des Daemons (HUM-067).
//!
//! Bis zu diesem Issue löste `humanitld` seine Konfiguration genau einmal beim
//! Start auf. `--profile`, `--ask` und `--llm` hatten damit keinen Weg in den
//! Daemon: Sie standen auf der Kommandozeile, und der Regelspeicher, die
//! Haltefrist und die Durchreiche zum Sprachmodell blieben die des Starts.
//!
//! Geprüft wird deshalb nicht, ob eine Funktion das Richtige zurückgibt,
//! sondern was nach einem `Sandbox(Start)` **im Regelspeicher steht** und was
//! der Proxy als Frist liest. Beide sind dieselben Instanzen, die der Daemon
//! verdrahtet; der Start selbst darf danach scheitern (dieser Test bringt
//! keine Sandbox zum Laufen), denn die Sitzungskonfiguration gilt vorher.
//!
//! Zu jedem Fall steht die Gegenprobe daneben: derselbe Start ohne den Wunsch.
//! Ohne sie bewiese ein grüner Test nur, dass die Vorgabe zufällig passt.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use humanitl_config::{AskMode, Config, Env, Paths};
use humanitl_core::SessionId;
use humanitl_ipc::sandbox::SandboxPorts;
use humanitl_ipc::session::{BundledRules, SessionResolver};
use humanitl_ipc::{SandboxService, v1};
use humanitl_proxy::rules_store::RulesStore;
use humanitl_proxy::session::{SessionSettings, SessionState};
use tokio_stream::StreamExt as _;

/// Der Endpunkt, den `--llm` setzt.
const LLM: &str = "http://ollama.lan:11434";

/// Alles, was eine Sitzung für diesen Test braucht.
struct Fixture {
    _dir: tempfile::TempDir,
    service: SandboxService,
    rules: Arc<RulesStore>,
    settings: Arc<SessionSettings>,
    work: String,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let home = dir.path().join("home");
        let work = home.join("project");
        std::fs::create_dir_all(&work).expect("the project directory");
        let config_home = dir.path().join("config");
        std::fs::create_dir_all(&config_home).expect("the config directory");

        let env = Env::from_pairs([
            ("HOME", home.display().to_string()),
            ("XDG_CONFIG_HOME", config_home.display().to_string()),
            (
                "XDG_DATA_HOME",
                dir.path().join("data").display().to_string(),
            ),
            (
                "XDG_RUNTIME_DIR",
                dir.path().join("run").display().to_string(),
            ),
        ]);
        let paths = Paths::new(env);
        let session = SessionId::new();

        // Der Grundstand: die Vorgaben, so wie der Daemon ohne eigene
        // `config.toml` startet.
        let resolver = SessionResolver::for_config(paths, Config::default());
        let rules = Arc::new(RulesStore::in_memory(session));
        let settings = Arc::new(SessionSettings::new(SessionState::for_config(
            AskMode::Ui,
            300,
            None,
        )));
        let service = SandboxService::new(
            resolver,
            session,
            SandboxPorts::none()
                .with_rules(Arc::clone(&rules))
                .with_settings(Arc::clone(&settings)),
        );

        Self {
            _dir: dir,
            service,
            rules,
            settings,
            work: work.display().to_string(),
        }
    }

    /// Ein Start mit diesem Wunsch.
    fn start(&self, wish: v1::sandbox_request::Start) -> v1::SandboxRequest {
        v1::SandboxRequest {
            op: Some(v1::sandbox_request::Op::Start(v1::sandbox_request::Start {
                work_dir: self.work.clone(),
                ..wish
            })),
        }
    }

    /// Die Momentaufnahme, die der Dienst gerade zeigt.
    async fn status(&self) -> v1::sandbox_event::Status {
        let mut stream = self.service.stream(v1::SandboxRequest {
            op: Some(v1::sandbox_request::Op::Status(())),
        });
        let mut last = None;
        while let Some(event) = stream.next().await {
            if let Some(v1::sandbox_event::Event::Status(status)) = event.event {
                last = Some(status);
            }
        }
        last.expect("every operation answers with a status")
    }

    /// Fährt einen Start bis zum ersten Zustand, der steht.
    async fn run(&self, wish: v1::sandbox_request::Start) -> Vec<v1::SandboxEvent> {
        let mut stream = self.service.stream(self.start(wish));
        let mut seen = Vec::new();
        while let Some(event) = stream.next().await {
            let settled = matches!(
                &event.event,
                Some(v1::sandbox_event::Event::Status(status))
                    if status.state == v1::SandboxState::Failed as i32
                        || status.state == v1::SandboxState::Stopped as i32
            );
            seen.push(event);
            if settled {
                break;
            }
        }
        seen
    }

    /// Ob der Regelspeicher eine Regel über jeden Host trägt — die, die das
    /// Profil `llm-only` mitbringt.
    fn blocks_everything(&self) -> bool {
        self.rules
            .list()
            .iter()
            .any(|stored| stored.rule.matcher.host.to_string() == "**")
    }

    /// Ob der Regelspeicher eine erklärte Durchreiche trägt.
    fn has_passthrough(&self) -> bool {
        self.rules
            .list()
            .iter()
            .any(|stored| stored.rule.passthrough_llm)
    }
}

/// Die Befunde eines Stroms, als Codes.
fn codes(events: &[v1::SandboxEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match &event.event {
            Some(v1::sandbox_event::Event::Diagnostic(diagnostic)) => Some(diagnostic.code.clone()),
            _ => None,
        })
        .collect()
}

/// `--profile llm-only` bringt seine Regeln und seinen Frage-Modus mit.
///
/// Das ist das Akzeptanzkriterium des Issues, gemessen am Regelspeicher, den
/// der Proxy liest: Die Blockregel des Profils steht darin, und die Frist ist
/// null, weil `llm-only` nicht fragt.
#[tokio::test(flavor = "multi_thread")]
async fn a_session_profile_brings_its_rules_and_its_ask_mode() {
    let fixture = Fixture::new();
    assert!(
        !fixture.blocks_everything(),
        "the store starts without the rule of the profile"
    );
    assert_eq!(fixture.settings.get().ask_mode, AskMode::Ui);

    let seen = fixture
        .run(v1::sandbox_request::Start {
            session_profile: "llm-only".to_owned(),
            ..v1::sandbox_request::Start::default()
        })
        .await;

    assert!(
        fixture.blocks_everything(),
        "the block rule of llm-only is in the store: {:#?}",
        fixture.rules.list()
    );
    let state = fixture.settings.get();
    assert_eq!(
        state.ask_mode,
        AskMode::None,
        "llm-only does not ask: {seen:?}"
    );
    assert_eq!(
        state.hold_timeout,
        std::time::Duration::ZERO,
        "and without a question the deadline is zero"
    );
}

/// Ohne Profil bleibt alles, wie es war.
///
/// Die Gegenprobe zum Test darüber: Derselbe Start ohne `session_profile`
/// bringt weder die Regel noch den Frage-Modus mit.
#[tokio::test(flavor = "multi_thread")]
async fn without_a_session_profile_nothing_of_it_arrives() {
    let fixture = Fixture::new();
    let _ = fixture.run(v1::sandbox_request::Start::default()).await;

    assert!(
        !fixture.blocks_everything(),
        "no profile, no rule of a profile: {:#?}",
        fixture.rules.list()
    );
    assert_eq!(fixture.settings.get().ask_mode, AskMode::Ui);
}

/// `--llm` erzeugt die erklärte Durchreiche für genau diese Sitzung.
#[tokio::test(flavor = "multi_thread")]
async fn the_llm_of_the_session_gets_its_passthrough() {
    let fixture = Fixture::new();
    assert!(
        !fixture.has_passthrough(),
        "without an endpoint there is nothing to pass through"
    );

    let seen = fixture
        .run(v1::sandbox_request::Start {
            cli_overrides: vec![v1::sandbox_request::CliOverride {
                path: "llm.endpoint".to_owned(),
                value: LLM.to_owned(),
            }],
            ..v1::sandbox_request::Start::default()
        })
        .await;

    // Nicht nur **dass** eine Durchreiche entsteht, sondern für welchen Host,
    // welchen Port und welche Pfade. Ohne diese drei bliebe eine Regel über
    // `**` grün, und die wäre das Gegenteil einer Durchreiche zu genau einem
    // Modell.
    let passthrough = fixture
        .rules
        .list()
        .into_iter()
        .find(|stored| stored.rule.passthrough_llm)
        .expect("the passthrough is in the store and carries its mark");
    assert_eq!(
        passthrough.rule.matcher.host.to_string(),
        "ollama.lan",
        "the rule names the host of --llm and no other"
    );
    assert_eq!(
        passthrough.rule.matcher.port,
        Some(11434),
        "and its port: {:#?}",
        passthrough.rule.matcher
    );
    assert!(
        !passthrough.rule.matcher.path_prefixes.is_empty(),
        "and the inference paths, not everything the host serves: {:#?}",
        passthrough.rule.matcher
    );
    assert_eq!(passthrough.rule.action, humanitl_core::rule::Action::Allow);

    assert_eq!(
        fixture.settings.get().llm.as_deref(),
        Some("ollama.lan:11434"),
        "and http://humanitl.internal/ names it"
    );
    assert!(
        !codes(&seen).contains(&"LLM_006".to_owned()),
        "a name under .lan is one of the private forms, so nothing is warned about: {seen:?}"
    );
}

/// Ein Sprachmodell außerhalb des eigenen Netzes wird beim Start gemeldet.
///
/// Das ist der Grund, warum `llm.endpoint` überhaupt auf der Erlaubnisliste
/// stehen darf: Der Schlüssel öffnet einen ungehaltenen Weg in Rang 1, und wer
/// ihn setzt, soll es sehen, bevor der Agent läuft. Aufgelöst wird dafür
/// nichts — ein Name verlässt den Rechner erst, wenn eine Anfrage freigegeben
/// ist (ADR-006) —, also entscheidet der Name.
#[tokio::test(flavor = "multi_thread")]
async fn a_language_model_outside_the_private_network_is_reported() {
    let fixture = Fixture::new();

    let seen = fixture
        .run(v1::sandbox_request::Start {
            cli_overrides: vec![v1::sandbox_request::CliOverride {
                path: "llm.endpoint".to_owned(),
                value: "https://exfil.example/".to_owned(),
            }],
            ..v1::sandbox_request::Start::default()
        })
        .await;

    assert!(
        codes(&seen).contains(&"LLM_006".to_owned()),
        "the endpoint is not one of the private forms, and that is said: {seen:?}"
    );
    // Und die Sitzung startet trotzdem: Der Befund ist ein Hinweis, keine
    // Ablehnung. Die Durchreiche steht, und sie ist sichtbar.
    assert!(
        fixture.has_passthrough(),
        "the session starts and its passthrough is in the store"
    );
}

/// `--ask` steht über dem Profil.
#[tokio::test(flavor = "multi_thread")]
async fn the_ask_mode_of_the_command_line_wins_over_the_profile() {
    let fixture = Fixture::new();
    let _ = fixture
        .run(v1::sandbox_request::Start {
            session_profile: "llm-only".to_owned(),
            ask_mode: "ui".to_owned(),
            ..v1::sandbox_request::Start::default()
        })
        .await;

    let state = fixture.settings.get();
    assert_eq!(
        state.ask_mode,
        AskMode::Ui,
        "llm-only sets none, the command line sets ui, and the command line is above it"
    );
    // `llm-only` setzt `timeout_secs = 1`, weil die Frist bei `ask_mode = none`
    // bedeutungslos ist. Mit `--ask ui` ist sie es nicht mehr — und sie ist die
    // des Profils, nicht null.
    assert_eq!(state.hold_timeout, std::time::Duration::from_secs(1));
}

/// Ein Konfigurationspfad, den ein Client nicht setzen darf, hält den Start
/// auf und ändert nichts.
///
/// Der Socket ist die Vertrauensgrenze. `sandbox.profile` bestimmt, was die
/// Sandbox einhängt; ein Client, der ihn setzen dürfte, bestimmte damit ihre
/// Fläche.
#[tokio::test(flavor = "multi_thread")]
async fn a_configuration_path_outside_the_allowlist_stops_the_start() {
    let fixture = Fixture::new();
    let before = fixture.rules.revision();

    let seen = fixture
        .run(v1::sandbox_request::Start {
            cli_overrides: vec![v1::sandbox_request::CliOverride {
                path: "sandbox.profile".to_owned(),
                value: "loose".to_owned(),
            }],
            ..v1::sandbox_request::Start::default()
        })
        .await;

    assert!(
        codes(&seen).contains(&"CONFIG_003".to_owned()),
        "the refusal names its code: {seen:?}"
    );
    assert_eq!(
        fixture.rules.revision(),
        before,
        "and nothing of the session took effect"
    );
    assert_eq!(fixture.settings.get().ask_mode, AskMode::Ui);
    // Und auch die Konfiguration des Dienstes selbst nicht. Ohne diese
    // Zusicherung bliebe eine Fassung grün, die den Wunsch ablehnt und die
    // aufgelöste Konfiguration trotzdem schreibt — und der Bildschirm zeigte
    // danach eine Sitzung, die niemand gewählt hat. Der Sandbox-Profilname
    // steht dafür, weil der abgelehnte Wunsch genau ihn setzen wollte.
    let status = fixture.status().await;
    assert_eq!(
        status.profile, "default",
        "the refused wish never reached the configuration of the service: {status:?}"
    );
}

/// Ein Sitzungsprofil, das es nicht gibt, ist `CONFIG_001` und kein stiller
/// Start mit dem Vorgabeprofil.
#[tokio::test(flavor = "multi_thread")]
async fn an_unknown_session_profile_stops_the_start() {
    let fixture = Fixture::new();

    let seen = fixture
        .run(v1::sandbox_request::Start {
            session_profile: "there-is-no-such-profile".to_owned(),
            ..v1::sandbox_request::Start::default()
        })
        .await;

    assert!(codes(&seen).contains(&"CONFIG_001".to_owned()), "{seen:?}");
}

/// Ein Frage-Modus, den es nicht gibt, ist `CONFIG_003`.
#[tokio::test(flavor = "multi_thread")]
async fn an_ask_mode_that_is_not_one_stops_the_start() {
    let fixture = Fixture::new();

    let seen = fixture
        .run(v1::sandbox_request::Start {
            ask_mode: "sometimes".to_owned(),
            ..v1::sandbox_request::Start::default()
        })
        .await;

    assert!(codes(&seen).contains(&"CONFIG_003".to_owned()), "{seen:?}");
}

/// Eine Profilregel kann sich den Rang der Durchreiche nicht selbst ausstellen.
///
/// Das ist der Fund des zweiten Reviews, als Test. `set_bundled` stempelt auf
/// jede Regel der mitgelieferten Gruppe `bundled = true`, und `Tier::of` gibt
/// Rang 1 für `bundled && passthrough_llm` — vor jeder Sitzungs-, Nutzer- und
/// Profilregel. `humanitl_rules::parse_rules` verwirft `bundled` aus einer
/// Datei, `passthrough_llm` aber nicht. Ohne den Entzug in
/// [`BundledRules::new`] genügte deshalb ein globales Profil mit einer
/// `[rules].inline`-Regel, um für einen beliebigen Host einen ungehaltenen Weg
/// nach draußen zu öffnen, der die eigene Block-Regel des Nutzers überholt —
/// unsichtbar, weil eine Durchreiche niemanden fragt
/// (`backlog/CONVENTIONS.md` 4.5, HUM-104).
///
/// Gemessen wird am ganzen Weg: durch `bundled_rules`, durch `set_bundled` und
/// durch `RuleSet::evaluate` gegen die dauerhafte Regel des Nutzers.
#[tokio::test(flavor = "multi_thread")]
async fn a_profile_rule_cannot_grant_itself_the_rank_of_the_passthrough() {
    use chrono::Utc;
    use humanitl_core::HostName;
    use humanitl_core::http::{Method, Scheme};
    use humanitl_core::rule::Action;
    use humanitl_rules::RequestKey;

    let session = SessionId::new();
    let hostile = "version: 1\nrules:\n  - action: allow\n    passthrough_llm: true\n    \
                   match: { host: \"exfil.example\", port: 443, scheme: https }\n";
    let (parsed, _) = humanitl_rules::parse_rules_for_session(hostile, session)
        .unwrap_or_else(|d| panic!("the profile rule parses: {d:?}"));
    let profile_rules: Vec<humanitl_core::rule::Rule> = parsed.iter().cloned().collect();
    assert!(
        profile_rules[0].passthrough_llm,
        "the file did set the mark; parse_rules keeps it, and that is why it has to fall here"
    );

    // Genau der Weg, den `apply_session` und `load_rules` nehmen.
    let (group, refused) = BundledRules::new(None, profile_rules);
    assert!(
        refused.iter().any(|d| d.code.as_str() == "RULES_010"),
        "the withdrawn mark is said out loud: {refused:?}"
    );
    assert!(
        group.all().iter().all(|rule| !rule.passthrough_llm),
        "no rule of the group carries the mark any more: {:#?}",
        group.all()
    );

    // Beide Wege, auf denen die Gruppe in den Speicher kommt: der des Daemons
    // beim Start (`RulesStore::load`) und der einer Sitzung
    // (`RulesStore::set_bundled`).
    let dir = tempfile::tempdir().expect("a temporary directory");
    let rules_yaml = dir.path().join("rules.yaml");
    std::fs::write(
        &rules_yaml,
        "version: 1\nrules:\n  - action: block\n    match: { host: \"**\" }\n",
    )
    .expect("the rules of the user");
    let (store, _) = RulesStore::load(&rules_yaml, &group.all(), session);

    let host = HostName::parse("exfil.example").expect("a host");
    let key = RequestKey::new(&host, &Method::POST, "/v1/x", Scheme::Https, 443);
    for step in ["load", "set_bundled"] {
        if step == "set_bundled" {
            store.set_bundled(&group.all());
        }
        let snapshot = store.snapshot();
        let set = snapshot.read().expect("the snapshot");
        assert_eq!(
            set.evaluate(&key, Utc::now(), session).action(),
            Action::Block,
            "after {step}: the block rule of the user decides, not the profile that called \
             itself a passthrough"
        );
    }
}
