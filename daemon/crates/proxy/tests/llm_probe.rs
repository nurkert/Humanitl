//! HUM-039, host-seitige Hälfte: die Probe des LLM-Endpunkts.
//!
//! Sie läuft gegen echte Server (den Fake-Upstream auf axum), über denselben
//! `Egress`- und `Resolver`-Port wie der Proxy. Geprüft wird, was sie
//! behauptet und was nicht: Sie erkennt Ollama vor der OpenAI-kompatiblen
//! Oberfläche, sie erfindet keine Modellliste, sie ändert nichts, und ein
//! Endpunkt, der schweigt, ist ein Befund mit einem `curl` und nicht ein
//! Vorwurf.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use humanitl_config::ResolverConfig;
use humanitl_core::{Authority, Diagnostic, HostName};
use humanitl_proxy::{
    AsyncStream, ClientTls, Direct, Egress, LlmFlavor, LlmProbe, Resolver, ResolverPort,
    TrustAnchors, Upstream,
};
use support::{FakeUpstream, MockResolver};
use url::Url;

/// Ein Egress, der jede Verbindung auf dieselbe Adresse legt.
///
/// Damit lässt sich prüfen, was die Probe über eine **öffentliche** Adresse
/// sagt, ohne dass ein Test ins Internet greift: Der Resolver antwortet mit
/// einer Adresse aus dem öffentlichen Raum, die Verbindung geht trotzdem zum
/// Fake auf der Schleife. Das ist genau die Trennung, für die es den Port gibt
/// (ADR-017).
struct PinnedEgress {
    addr: SocketAddr,
    inner: Direct,
}

#[async_trait]
impl Egress for PinnedEgress {
    async fn connect(
        &self,
        _authority: &Authority,
        _resolved: Option<IpAddr>,
    ) -> Result<Box<dyn AsyncStream>, Diagnostic> {
        let real = Authority::new(HostName::Ip(self.addr.ip()), self.addr.port());
        self.inner.connect(&real, Some(self.addr.ip())).await
    }
}

/// Eine Probe über dem gewöhnlichen Egress, mit einem Mock-Resolver darunter.
fn probe_with(answers: Vec<(&str, Vec<IpAddr>)>, egress: Arc<dyn Egress>) -> LlmProbe {
    let mut mock = MockResolver::answering(IpAddr::V4(Ipv4Addr::LOCALHOST));
    for (host, addrs) in answers {
        mock = mock.with_answer(host, addrs);
    }
    let config = ResolverConfig {
        cache_ttl_secs: 0,
        ..ResolverConfig::default()
    };
    let resolver =
        Arc::new(ResolverPort::over(Arc::new(mock) as Arc<dyn Resolver>, &config).unwrap());
    let upstream = Upstream::new(
        egress,
        resolver as Arc<dyn Resolver>,
        ClientTls::new(&TrustAnchors::bundled(), &[], false).unwrap(),
        config.prefer,
        Duration::from_secs(5),
    );
    LlmProbe::new(upstream)
}

/// Die Vorgabe: direkter Egress, keine besonderen Antworten.
fn probe() -> LlmProbe {
    probe_with(Vec::new(), Arc::new(Direct::default()))
}

fn endpoint(port: u16) -> Url {
    Url::parse(&format!("http://127.0.0.1:{port}")).unwrap()
}

/// Ollama wird als Ollama erkannt, obwohl es auch `/v1/models` beantwortet.
#[tokio::test(flavor = "multi_thread")]
async fn probe_detects_ollama() {
    let upstream = FakeUpstream::ollama().await;
    let result = probe()
        .probe(&endpoint(upstream.port()), None)
        .await
        .unwrap();

    assert_eq!(
        result.flavor,
        LlmFlavor::Ollama,
        "asking /api/tags first is what keeps the flavour right"
    );
    assert_eq!(
        result.models,
        vec!["qwen2.5-coder:14b".to_owned(), "llama3.1:8b".to_owned()]
    );
    assert!(result.endpoint_is_private, "127.0.0.1 is loopback");
    assert!(
        result.diagnostics.is_empty(),
        "nothing to warn about: {:?}",
        result.diagnostics
    );
    assert_eq!(
        upstream.hits(),
        1,
        "one GET, and the second path was never needed"
    );
}

/// Ein Server mit nur der OpenAI-kompatiblen Oberfläche wird als solcher erkannt.
#[tokio::test(flavor = "multi_thread")]
async fn probe_detects_openai() {
    let upstream = FakeUpstream::openai().await;
    let result = probe()
        .probe(&endpoint(upstream.port()), None)
        .await
        .unwrap();

    assert_eq!(result.flavor, LlmFlavor::OpenAiCompatible);
    assert_eq!(
        result.models,
        vec!["gpt-oss:20b".to_owned(), "phi-4".to_owned()]
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
}

/// Antwortet weder `/api/tags` noch `/v1/models`, ist das ein Ergebnis mit
/// `LLM_003` und einer leeren Liste — nie eine erfundene.
#[tokio::test(flavor = "multi_thread")]
async fn probe_unknown_paths_llm_003() {
    let upstream = FakeUpstream::plain().await;
    let result = probe()
        .probe(&endpoint(upstream.port()), None)
        .await
        .unwrap();

    assert_eq!(result.flavor, LlmFlavor::Unknown);
    assert!(result.models.is_empty());
    let codes: Vec<&str> = result
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect();
    assert_eq!(codes, vec!["LLM_003"], "{:?}", result.diagnostics);
    assert_eq!(
        result.diagnostics[0].severity,
        humanitl_core::Severity::Warning
    );
    // HUM-283: Neben `LLM_003` steht, was zurückkam, nicht „0 Modelle".
    let answer = result.answer.as_deref().expect("what came back");
    assert!(answer.starts_with("HTTP 404"), "{answer}");
    assert!(
        result.diagnostics[0].why.contains(answer),
        "{}",
        result.diagnostics[0].why
    );
}

/// Kommt eine Modellliste, gibt es nichts zu erklären: `answer` bleibt leer
/// (HUM-283).
#[tokio::test(flavor = "multi_thread")]
async fn a_model_list_leaves_no_answer_to_explain() {
    let upstream = FakeUpstream::ollama().await;
    let result = probe()
        .probe(&endpoint(upstream.port()), None)
        .await
        .unwrap();
    assert_eq!(result.answer, None);
}

/// Ein Server, der eine Anmeldung verlangt, ist `LLM_002` und keine leere
/// Modellliste.
#[tokio::test(flavor = "multi_thread")]
async fn probe_needs_auth_llm_002() {
    let upstream = FakeUpstream::needs_auth().await;
    let diagnostic = probe()
        .probe(&endpoint(upstream.port()), None)
        .await
        .unwrap_err();

    assert_eq!(diagnostic.code.as_str(), "LLM_002");
    assert_eq!(diagnostic.severity, humanitl_core::Severity::Error);
    assert!(diagnostic.why.contains("401"), "{}", diagnostic.why);
}

/// Ein Endpunkt, der annimmt und dann schweigt, läuft in die Frist und ist
/// `LLM_001` mit einem `curl`, den der Mensch selbst laufen lassen kann.
#[tokio::test(flavor = "multi_thread")]
async fn probe_timeout_llm_001() {
    // Ein Listener, der annimmt und nichts sagt. Die Verbindung kommt also
    // zustande; was fehlt, ist die Antwort.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let silent = tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((stream, _)) = listener.accept().await {
            held.push(stream);
        }
    });

    let diagnostic = probe()
        .probe(&endpoint(port), Some(Duration::from_millis(200)))
        .await
        .unwrap_err();
    silent.abort();

    assert_eq!(diagnostic.code.as_str(), "LLM_001");
    assert_eq!(diagnostic.severity, humanitl_core::Severity::Blocking);
    match diagnostic.fix {
        Some(humanitl_core::FixAction::CopyCommand(command)) => assert_eq!(
            command,
            format!("curl -sS http://127.0.0.1:{port}/api/tags"),
            "the fix asks by hand exactly the path that went unanswered (HUM-261)"
        ),
        other => panic!("expected a copyable curl, got {other:?}"),
    }
}

/// Ein Port, auf dem niemand lauscht, ist derselbe Befund — ohne zu warten.
#[tokio::test(flavor = "multi_thread")]
async fn probe_refused_connection_llm_001() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let diagnostic = probe()
        .probe(&endpoint(port), Some(Duration::from_secs(5)))
        .await
        .unwrap_err();
    assert_eq!(diagnostic.code.as_str(), "LLM_001");
}

/// Ein Endpunkt außerhalb des eigenen Netzes bekommt `LLM_006` — als Hinweis
/// neben dem Ergebnis, nicht statt seiner.
#[tokio::test(flavor = "multi_thread")]
async fn probe_public_ip_llm_006() {
    let upstream = FakeUpstream::ollama().await;
    let egress = Arc::new(PinnedEgress {
        addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), upstream.port()),
        inner: Direct::default(),
    });
    let probe = probe_with(
        vec![(
            "model.example.com",
            vec![IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))],
        )],
        egress,
    );

    let result = probe
        .probe(&Url::parse("http://model.example.com:11434").unwrap(), None)
        .await
        .unwrap();

    assert_eq!(result.flavor, LlmFlavor::Ollama, "the probe still worked");
    assert!(!result.models.is_empty());
    assert!(!result.endpoint_is_private);
    let codes: Vec<&str> = result
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect();
    assert_eq!(codes, vec!["LLM_006"], "{:?}", result.diagnostics);
    assert_eq!(
        result.diagnostics[0].severity,
        humanitl_core::Severity::Info
    );
}

/// Ein Name im eigenen Namensraum zählt als privat, auch wenn die Adresse es
/// nicht ist. `.lan` ist der Fall, den Menschen tatsächlich tippen.
#[tokio::test(flavor = "multi_thread")]
async fn probe_keeps_a_lan_name_private() {
    let upstream = FakeUpstream::ollama().await;
    let egress = Arc::new(PinnedEgress {
        addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), upstream.port()),
        inner: Direct::default(),
    });
    let probe = probe_with(
        vec![(
            "ollama.lan",
            vec![IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))],
        )],
        egress,
    );

    let result = probe
        .probe(&Url::parse("http://ollama.lan:11434").unwrap(), None)
        .await
        .unwrap();
    assert!(result.endpoint_is_private);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
}

/// Die Probe ändert nichts: Sie fragt zwei Pfade mit `GET`, und `/api/pull`
/// ist keiner davon.
#[tokio::test(flavor = "multi_thread")]
async fn probe_never_touches_a_mutating_path() {
    let upstream = FakeUpstream::ollama().await;
    let result = probe()
        .probe(&endpoint(upstream.port()), None)
        .await
        .unwrap();
    assert_eq!(result.flavor, LlmFlavor::Ollama);
    // Ein einziger Treffer, und der war `/api/tags`. Hätte die Probe
    // irgendetwas anderes gefragt, stünde hier eine höhere Zahl.
    assert_eq!(upstream.hits(), 1);
}

/// Ein Endpunkt mit `/v1` im Pfad meint denselben Server: Die Probe fragt
/// dieselbe Wurzel wie ohne (HUM-039, Fallstricke).
#[tokio::test(flavor = "multi_thread")]
async fn probe_ignores_a_trailing_v1_in_the_endpoint() {
    let upstream = FakeUpstream::ollama().await;
    let with_v1 = Url::parse(&format!("http://127.0.0.1:{}/v1", upstream.port())).unwrap();
    let result = probe().probe(&with_v1, None).await.unwrap();
    assert_eq!(result.flavor, LlmFlavor::Ollama);
    assert_eq!(result.models.len(), 2);
}

/// Probe und Proxy-Pfad lösen denselben Namen auf dieselbe Weise auf.
///
/// Der Fallstrick von HUM-039 nennt dafür einen `/etc/hosts`-Eintrag im CI.
/// Der ist hier durch `resolver.overrides` ersetzt: derselbe Schlüssel, den ein
/// Nutzer setzen würde, und beide Seiten bekommen dieselbe
/// `ResolverConfig`-Instanz. Der Namensdienst darunter **scheitert** für
/// `ollama.lan`; kommt trotzdem eine Verbindung zustande, hat die feste
/// Zuordnung geantwortet — und zwar auf beiden Wegen. Ein `/etc/hosts` täte
/// dasselbe, verlangte aber Schreibrechte am System und wäre kein Test mehr,
/// sondern eine Umgebung.
#[tokio::test(flavor = "multi_thread")]
async fn probe_and_proxy_resolve_a_lan_name_the_same_way() {
    use std::collections::BTreeMap;

    use humanitl_core::Decision;
    use support::ProxyBuilder;

    const NAME: &str = "ollama.lan";

    let upstream = FakeUpstream::ollama().await;
    let mut overrides = BTreeMap::new();
    overrides.insert(NAME.to_owned(), "127.0.0.1".to_owned());
    let config = ResolverConfig {
        overrides,
        cache_ttl_secs: 0,
        ..ResolverConfig::default()
    };

    // Seite 1: die Probe. Der Mock scheitert für den Namen, die feste
    // Zuordnung nicht.
    let mock = MockResolver::answering(IpAddr::V4(Ipv4Addr::LOCALHOST)).failing_for(NAME);
    let resolver =
        Arc::new(ResolverPort::over(Arc::new(mock) as Arc<dyn Resolver>, &config).unwrap());
    let probe = LlmProbe::new(Upstream::new(
        Arc::new(PinnedEgress {
            addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), upstream.port()),
            inner: Direct::default(),
        }),
        resolver as Arc<dyn Resolver>,
        ClientTls::new(&TrustAnchors::bundled(), &[], false).unwrap(),
        config.prefer,
        Duration::from_secs(5),
    ));
    let result = probe
        .probe(
            &Url::parse(&format!("http://{NAME}:{}", upstream.port())).unwrap(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(result.flavor, LlmFlavor::Ollama, "the override answered");
    assert!(result.endpoint_is_private, "a .lan name is the own network");

    // Seite 2: der Proxy mit derselben Konfiguration und derselben
    // Durchreichregel. Was gehalten wird, wird geblockt; ein `200` heißt also,
    // dass die Regel griff und der Name auflöste.
    let port = upstream.port();
    let yaml = format!(
        "version: 1\n\
         rules:\n\
         \x20 - id: 01920000-0000-7000-8000-0000000000ff\n\
         \x20   action: allow\n\
         \x20   match:\n\
         \x20     host: \"{NAME}\"\n\
         \x20     port: {port}\n\
         \x20     scheme: http\n\
         \x20     method: [POST, GET]\n\
         \x20     path_prefixes: [\"/api/tags\"]\n\
         \x20   allow_private: true\n\
         \x20   passthrough_llm: true\n"
    );
    let proxy = ProxyBuilder::new()
        .rules(&yaml)
        .allow_private(false)
        .resolver_config(config)
        .resolve_fails(NAME)
        .ask(Duration::from_secs(30))
        .start()
        .await;
    let _decider = proxy.decide_with(Decision::Block {
        reason: humanitl_core::BlockReason::User,
        note: None,
    });

    let mut client = proxy.client().await;
    let response = client
        .send(support::get(&format!("http://{NAME}:{port}/api/tags")))
        .await;
    assert_eq!(
        response.status(),
        hyper::StatusCode::OK,
        "the same override carries the same name through the proxy path"
    );
    assert!(
        !proxy.resolver.hosts().iter().any(|host| host == NAME),
        "and the name server below was never asked: {:?}",
        proxy.resolver.hosts()
    );
}

/// Ein Server, der nur mit dem richtigen Schlüssel antwortet, und der jede
/// Kopfzeile `Authorization` mitschreibt, die bei ihm ankommt.
async fn keyed_server(key: &'static str) -> (u16, Arc<std::sync::Mutex<Vec<String>>>) {
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let log = Arc::clone(&seen);
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let log = Arc::clone(&log);
            tokio::spawn(async move {
                let mut head = Vec::new();
                let mut byte = [0u8; 1];
                while !head.ends_with(b"\r\n\r\n") {
                    if stream.read(&mut byte).await.unwrap_or(0) == 0 {
                        return;
                    }
                    head.push(byte[0]);
                }
                let head = String::from_utf8_lossy(&head).into_owned();
                let auth = head
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .starts_with("authorization:")
                            .then(|| line["authorization:".len()..].trim().to_owned())
                    })
                    .unwrap_or_default();
                log.lock().unwrap().push(auth.clone());
                let body = r#"{"models":[{"name":"keyed-model"}]}"#;
                let reply = if auth == format!("Bearer {key}") {
                    format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: \
                         {}\r\nconnection: close\r\n\r\n{body}",
                        body.len()
                    )
                } else {
                    "HTTP/1.1 401 Unauthorized\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
                        .to_owned()
                };
                let _ignored = stream.write_all(reply.as_bytes()).await;
            });
        }
    });
    (port, seen)
}

/// Mit Schlüssel geht er als `Bearer` an genau diesen Endpunkt, und die
/// Antwort sagt, dass einer ging; ohne Schlüssel geht keiner, und `LLM_002`
/// schlägt vor, einen zu hinterlegen (HUM-140, als Einstellung seit HUM-275).
#[tokio::test(flavor = "multi_thread")]
async fn probe_sends_the_key_as_bearer_and_only_when_given() {
    const KEY: &str = "sk-probe-0123456789";
    let (port, seen) = keyed_server(KEY).await;
    let key = humanitl_config::secret::ApiKey::new(KEY).unwrap();

    let result = probe()
        .probe_with_key(&endpoint(port), None, Some(&key))
        .await
        .unwrap();
    assert_eq!(result.models, vec!["keyed-model".to_owned()]);
    assert!(result.key_sent);
    assert_eq!(seen.lock().unwrap().as_slice(), [format!("Bearer {KEY}")]);

    let refused = probe().probe(&endpoint(port), None).await.unwrap_err();
    assert_eq!(refused.code.as_str(), "LLM_002");
    assert!(refused.why.contains("none was sent"), "{}", refused.why);
    assert_eq!(refused.fix, Some(key_setting_fix()));
    assert_eq!(
        seen.lock().unwrap().last().map(String::as_str),
        Some(""),
        "the probe without a key sends no Authorization"
    );

    let wrong = humanitl_config::secret::ApiKey::new("sk-wrong-0000000000").unwrap();
    let rejected = probe()
        .probe_with_key(&endpoint(port), None, Some(&wrong))
        .await
        .unwrap_err();
    assert!(
        rejected.why.contains("does not accept this key"),
        "{}",
        rejected.why
    );
    assert!(!rejected.why.contains("sk-wrong"), "{}", rejected.why);
}

/// Der Vorschlag jedes `LLM_002` seit HUM-275: die Einstellung
/// `llm.api_key_ref` mit leerem Wert. Die Oberfläche macht daraus den Knopf,
/// der die Einstellungen am Feld des Zugangsschlüssels öffnet.
fn key_setting_fix() -> humanitl_core::FixAction {
    humanitl_core::FixAction::ChangeSetting {
        key: "llm.api_key_ref".to_owned(),
        value: String::new(),
    }
}

/// HUM-275: `LLM_002` trägt in beiden Fällen, ohne Schlüssel und mit einem
/// abgelehnten, den Vorschlag `ChangeSetting` auf `llm.api_key_ref`. Ohne ihn
/// bliebe die Oberfläche ohne Weg zum Feld, und der Mensch fände die
/// Einstellungen nur über `Ctrl+,`.
#[tokio::test(flavor = "multi_thread")]
async fn llm_002_carries_the_change_setting_fix() {
    let upstream = FakeUpstream::needs_auth().await;
    let missing = probe()
        .probe(&endpoint(upstream.port()), None)
        .await
        .unwrap_err();
    assert_eq!(missing.code.as_str(), "LLM_002");
    assert_eq!(missing.fix, Some(key_setting_fix()));

    let (port, _seen) = keyed_server("sk-right-0123456789").await;
    let wrong = humanitl_config::secret::ApiKey::new("sk-wrong-0000000000").unwrap();
    let rejected = probe()
        .probe_with_key(&endpoint(port), None, Some(&wrong))
        .await
        .unwrap_err();
    assert_eq!(rejected.code.as_str(), "LLM_002");
    assert_eq!(rejected.fix, Some(key_setting_fix()));
    assert_eq!(
        humanitl_proxy::llm_probe::api_key_fix(),
        key_setting_fix(),
        "the helper and the test name the same setting"
    );
}

/// HUM-257: Ein Endpunkt, dessen Zertifikat von einer Stelle stammt, der
/// dieser Rechner nicht vertraut, ist `LLM_001` mit genau dieser Ursache im
/// `why` und dem Weg, die CA ins System zu bringen — nicht „nicht
/// erreichbar" mit einem `curl`, der dasselbe Scheitern noch einmal zeigt.
#[tokio::test(flavor = "multi_thread")]
async fn an_endpoint_signed_by_an_unknown_authority_says_so() {
    let ca = support::UpstreamCa::new();
    let endpoint_server = FakeUpstream::tls(ca.store()).await;
    // Die Ablage enthält eine CA, nur nicht die des Endpunkts.
    let dir = tempfile::tempdir().unwrap();
    let other = dir.path().join("other.pem");
    std::fs::write(&other, support::ForeignCa::new().pem()).unwrap();
    let anchors = TrustAnchors::from_paths(Some(&other), None);
    let config = ResolverConfig {
        cache_ttl_secs: 0,
        ..ResolverConfig::default()
    };
    let resolver = Arc::new(
        ResolverPort::over(
            Arc::new(MockResolver::answering(IpAddr::V4(Ipv4Addr::LOCALHOST))) as Arc<dyn Resolver>,
            &config,
        )
        .unwrap(),
    );
    let probe = LlmProbe::new(Upstream::new(
        Arc::new(Direct::default()),
        resolver as Arc<dyn Resolver>,
        ClientTls::new(&anchors, &[], false).unwrap(),
        config.prefer,
        Duration::from_secs(5),
    ));
    let url = Url::parse(&format!("https://localhost:{}", endpoint_server.port())).unwrap();

    let diagnostic = probe.probe(&url, None).await.unwrap_err();

    assert_eq!(diagnostic.code.as_str(), "LLM_001");
    assert!(
        diagnostic
            .why
            .contains("signed by an authority this machine does not trust"),
        "{}",
        diagnostic.why
    );
    assert!(diagnostic.why.contains("localhost"), "{}", diagnostic.why);
    match diagnostic.fix {
        Some(humanitl_core::FixAction::CopyCommand(command)) => {
            assert_eq!(command, "sudo update-ca-certificates");
        }
        other => panic!("expected the command that installs the CA, got {other:?}"),
    }
    assert_eq!(endpoint_server.hits(), 0, "the handshake failed first");
}

/// Ein OpenAI-kompatibler Server hinter dem Pfad-Präfix `/api`, wie ihn ein
/// Reverse-Proxy vor `Open WebUI` oder `LiteLLM` anbietet (HUM-261).
///
/// `GET /api/v1/models` beantwortet er mit dem richtigen Schlüssel mit einer
/// Modellliste, ohne ihn mit `401`. Jeder andere Pfad ist `404` mit dem Körper,
/// den der Server des Befunds vom 2026-10-20 schickte. Jede Anfrage schreibt
/// er als Pfad mit.
async fn prefixed_server(key: &'static str) -> (u16, Arc<std::sync::Mutex<Vec<String>>>) {
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let log = Arc::clone(&seen);
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let log = Arc::clone(&log);
            tokio::spawn(async move {
                let mut head = Vec::new();
                let mut byte = [0u8; 1];
                while !head.ends_with(b"\r\n\r\n") {
                    if stream.read(&mut byte).await.unwrap_or(0) == 0 {
                        return;
                    }
                    head.push(byte[0]);
                }
                let head = String::from_utf8_lossy(&head).into_owned();
                let path = head
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .unwrap_or_default()
                    .to_owned();
                let auth = head
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .starts_with("authorization:")
                            .then(|| line["authorization:".len()..].trim().to_owned())
                    })
                    .unwrap_or_default();
                log.lock().unwrap().push(path.clone());
                let reply = |status: &str, body: &str| {
                    format!(
                        "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: \
                         {}\r\nconnection: close\r\n\r\n{body}",
                        body.len()
                    )
                };
                let reply = if path == "/api/v1/models" {
                    if auth == format!("Bearer {key}") {
                        reply(
                            "200 OK",
                            r#"{"object":"list","data":[{"id":"prefixed-model"}]}"#,
                        )
                    } else {
                        reply("401 Unauthorized", r#"{"detail":"Not authenticated"}"#)
                    }
                } else {
                    reply("404 Not Found", r#"{"detail":"Not Found"}"#)
                };
                let _ignored = stream.write_all(reply.as_bytes()).await;
            });
        }
    });
    (port, seen)
}

/// HUM-261: Ein Endpunkt mit Pfad-Präfix ist die Basis der OpenAI-kompatiblen
/// API. Mit Schlüssel fragt die Probe genau `<Basis>/models` und erkennt den
/// Server; `/api/tags` fragt sie weder unter dem Präfix (`/api/api/tags`) noch
/// an der Wurzel des Hosts.
#[tokio::test(flavor = "multi_thread")]
async fn a_prefixed_endpoint_is_asked_under_its_base() {
    const KEY: &str = "sk-prefix-0123456789";
    let (port, seen) = prefixed_server(KEY).await;
    let key = humanitl_config::secret::ApiKey::new(KEY).unwrap();

    for written in ["/api/v1/", "/api/v1"] {
        seen.lock().unwrap().clear();
        let url = Url::parse(&format!("http://127.0.0.1:{port}{written}")).unwrap();
        let result = probe()
            .probe_with_key(&url, None, Some(&key))
            .await
            .unwrap();
        assert_eq!(result.flavor, LlmFlavor::OpenAiCompatible, "for {written}");
        assert_eq!(result.models, vec!["prefixed-model".to_owned()]);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(
            seen.lock().unwrap().as_slice(),
            ["/api/v1/models"],
            "for {written}: one GET, under the base, and no Ollama path"
        );
    }
}

/// HUM-261: Ohne Schlüssel sagt der Server `401` unter der Basis. Das ist
/// `LLM_002` mit der Adresse, die so geantwortet hat, und dem Weg zum
/// Schlüssel — nie „nicht erreichbar" und nie ein Pfad, den es nicht gibt.
#[tokio::test(flavor = "multi_thread")]
async fn a_prefixed_endpoint_without_a_key_asks_for_the_key() {
    let (port, seen) = prefixed_server("sk-prefix-0123456789").await;
    let url = Url::parse(&format!("http://127.0.0.1:{port}/api/v1/")).unwrap();

    let diagnostic = probe().probe(&url, None).await.unwrap_err();

    assert_eq!(diagnostic.code.as_str(), "LLM_002");
    assert!(
        diagnostic
            .why
            .contains(&format!("http://127.0.0.1:{port}/api/v1/models")),
        "{}",
        diagnostic.why
    );
    assert!(
        diagnostic.why.contains("none was sent"),
        "{}",
        diagnostic.why
    );
    assert!(!diagnostic.why.contains("/api/api/"), "{}", diagnostic.why);
    assert!(
        !diagnostic.why.contains("could not reach"),
        "{}",
        diagnostic.why
    );
    assert_eq!(diagnostic.fix, Some(key_setting_fix()));
    assert_eq!(seen.lock().unwrap().as_slice(), ["/api/v1/models"]);
}

/// HUM-261: Ein Endpunkt mit Pfad-Präfix, der schweigt, ist `LLM_001` mit
/// einem `curl` auf die Modellliste unter seiner Basis — nie auf
/// `/api/api/tags`, die Adresse aus dem Befund vom 2026-10-20.
#[tokio::test(flavor = "multi_thread")]
async fn a_silent_prefixed_endpoint_gets_a_curl_on_its_model_list() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let silent = tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((stream, _)) = listener.accept().await {
            held.push(stream);
        }
    });
    let url = Url::parse(&format!("http://127.0.0.1:{port}/api/v1/")).unwrap();

    let diagnostic = probe()
        .probe(&url, Some(Duration::from_millis(200)))
        .await
        .unwrap_err();
    silent.abort();

    assert_eq!(diagnostic.code.as_str(), "LLM_001");
    match diagnostic.fix {
        Some(humanitl_core::FixAction::CopyCommand(command)) => {
            assert_eq!(
                command,
                format!("curl -sS http://127.0.0.1:{port}/api/v1/models")
            );
            assert!(!command.contains("/api/api/"), "{command}");
        }
        other => panic!("expected a copyable curl, got {other:?}"),
    }
}

/// HUM-261: Antwortet unter einem Pfad-Präfix auch die Modellliste nicht,
/// nennt `LLM_003` genau den Pfad, der gefragt wurde, und keinen, der nicht
/// gefragt wurde.
#[tokio::test(flavor = "multi_thread")]
async fn an_unknown_prefixed_endpoint_names_only_the_path_it_asked() {
    let (port, seen) = prefixed_server("sk-prefix-0123456789").await;
    let url = Url::parse(&format!("http://127.0.0.1:{port}/other/")).unwrap();

    let result = probe().probe(&url, None).await.unwrap();

    assert_eq!(result.flavor, LlmFlavor::Unknown);
    let llm_003 = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_str() == "LLM_003")
        .expect("LLM_003");
    assert!(llm_003.why.contains("/other/v1/models"), "{}", llm_003.why);
    assert!(!llm_003.why.contains("/api/tags"), "{}", llm_003.why);
    assert_eq!(seen.lock().unwrap().as_slice(), ["/other/v1/models"]);
}

/// HUM-261: `…/v1` ohne Schrägstrich ist kein Präfix. Ein Server mit nur der
/// OpenAI-kompatiblen Oberfläche wird über `/v1/models` erkannt, nachdem
/// `/api/tags` an der Wurzel ins Leere ging.
#[tokio::test(flavor = "multi_thread")]
async fn a_trailing_v1_without_a_slash_is_no_prefix() {
    let upstream = FakeUpstream::openai().await;
    let url = Url::parse(&format!("http://127.0.0.1:{}/v1", upstream.port())).unwrap();
    let result = probe().probe(&url, None).await.unwrap();
    assert_eq!(result.flavor, LlmFlavor::OpenAiCompatible);
    assert_eq!(
        result.models,
        vec!["gpt-oss:20b".to_owned(), "phi-4".to_owned()]
    );
}

/// Ein Server, der `/api/tags` mit `404` beantwortet und auf jede andere
/// Anfrage schweigt.
async fn tags_404_then_silent() -> u16 {
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let mut parked = Vec::new();
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let mut head = Vec::new();
            let mut byte = [0u8; 1];
            while !head.ends_with(b"\r\n\r\n") {
                if stream.read(&mut byte).await.unwrap_or(0) == 0 {
                    break;
                }
                head.push(byte[0]);
            }
            if head.starts_with(b"GET /api/tags ") {
                let _ignored = stream
                    .write_all(
                        b"HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
                    )
                    .await;
            } else {
                parked.push(stream);
            }
        }
    });
    port
}

/// HUM-261: Der `curl` von `LLM_001` fragt den Pfad, an dem die Probe
/// scheiterte. Ging `/api/tags` mit `404` durch und schweigt danach die
/// Modellliste, ist es die Modellliste.
#[tokio::test(flavor = "multi_thread")]
async fn the_curl_names_the_path_that_went_unanswered() {
    let port = tags_404_then_silent().await;
    let diagnostic = probe()
        .probe(&endpoint(port), Some(Duration::from_millis(300)))
        .await
        .unwrap_err();
    assert_eq!(diagnostic.code.as_str(), "LLM_001");
    assert_eq!(
        diagnostic.fix,
        Some(humanitl_core::FixAction::CopyCommand(format!(
            "curl -sS http://127.0.0.1:{port}/v1/models"
        )))
    );
}

/// HUM-261: Scheitert die Probe vor jeder Anfrage, an der Auflösung, nennt
/// der Befund keinen Pfad, der angeblich gefragt wurde.
#[tokio::test(flavor = "multi_thread")]
async fn a_name_that_does_not_resolve_names_no_path() {
    const NAME: &str = "nowhere.lan";
    let mock = MockResolver::answering(IpAddr::V4(Ipv4Addr::LOCALHOST)).failing_for(NAME);
    let config = ResolverConfig {
        cache_ttl_secs: 0,
        ..ResolverConfig::default()
    };
    let resolver =
        Arc::new(ResolverPort::over(Arc::new(mock) as Arc<dyn Resolver>, &config).unwrap());
    let probe = LlmProbe::new(Upstream::new(
        Arc::new(Direct::default()),
        resolver as Arc<dyn Resolver>,
        ClientTls::new(&TrustAnchors::bundled(), &[], false).unwrap(),
        config.prefer,
        Duration::from_secs(5),
    ));
    let url = Url::parse(&format!("http://{NAME}:11434/api/v1/")).unwrap();

    let diagnostic = probe.probe(&url, None).await.unwrap_err();

    assert_eq!(diagnostic.code.as_str(), "LLM_001");
    assert!(
        diagnostic.why.contains("does not resolve"),
        "{}",
        diagnostic.why
    );
    assert_eq!(
        diagnostic.fix,
        Some(humanitl_core::FixAction::CopyCommand(format!(
            "getent hosts {NAME}"
        )))
    );
}

/// HUM-261: Query und Fragment des eingetragenen Endpunkts gehen mit keiner
/// Anfrage hinaus und stehen in keinem Befund, weder in `LLM_003` noch im
/// `curl` von `LLM_001`.
#[tokio::test(flavor = "multi_thread")]
async fn a_token_in_the_endpoint_appears_in_no_diagnostic() {
    let (port, seen) = prefixed_server("sk-prefix-0123456789").await;
    let url = Url::parse(&format!("http://127.0.0.1:{port}/other/?token=geheim#frag")).unwrap();
    let result = probe().probe(&url, None).await.unwrap();
    assert_eq!(result.flavor, LlmFlavor::Unknown);
    assert!(!result.diagnostics.is_empty());
    for diagnostic in &result.diagnostics {
        assert!(!diagnostic.why.contains("geheim"), "{}", diagnostic.why);
        assert!(!format!("{:?}", diagnostic.fix).contains("geheim"));
    }
    assert!(
        seen.lock()
            .unwrap()
            .iter()
            .all(|path| !path.contains("geheim")),
        "{:?}",
        seen.lock().unwrap()
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let silent_port = listener.local_addr().unwrap().port();
    let silent = tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((stream, _)) = listener.accept().await {
            held.push(stream);
        }
    });
    let url = Url::parse(&format!(
        "http://127.0.0.1:{silent_port}/api/v1?token=geheim"
    ))
    .unwrap();
    let diagnostic = probe()
        .probe(&url, Some(Duration::from_millis(200)))
        .await
        .unwrap_err();
    silent.abort();
    assert_eq!(diagnostic.code.as_str(), "LLM_001");
    assert!(!diagnostic.why.contains("geheim"), "{}", diagnostic.why);
    assert_eq!(
        diagnostic.fix,
        Some(humanitl_core::FixAction::CopyCommand(format!(
            "curl -sS http://127.0.0.1:{silent_port}/api/v1/models"
        )))
    );
}
