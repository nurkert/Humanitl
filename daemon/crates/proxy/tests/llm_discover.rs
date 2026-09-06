//! HUM-076: die Suche nach LLM-Servern im eigenen Netz.
//!
//! Was hier geprüft wird, ist die Zusage des Blattes im Setup: Ein Server, der
//! antwortet, steht mit seinen Modellen in der Liste; ein Port, an dem niemand
//! ist, kostet nichts; und keine einzige Verbindung entsteht, ohne dass jemand
//! die Suche gestartet hat.
//!
//! Die Läufe bleiben auf der Schleife. Ein Test, der ein echtes `/24` abfragt,
//! schickte Pakete in das Netz, in dem er zufällig läuft — auf einem
//! CI-Läufer wäre das der Portscan, den die Spezifikation gerade ausschließt.
//! Die Gesamtdauer eines Scans ohne Treffer prüft deshalb der Rechentest über
//! den Konstanten (`the_time_budget_of_a_full_scan_holds`), nicht ein Lauf ins
//! Blaue.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use humanitl_config::ResolverConfig;
use humanitl_core::{Authority, Diagnostic};
use humanitl_proxy::{
    AsyncStream, CONNECT_TIMEOUT, ClientTls, DEFAULT_PORTS, Direct, Egress, Found, LlmFlavor,
    LlmProbe, PARALLEL, Resolver, ResolverPort, Scan, Subnet, Upstream, discover,
};
use support::{FakeUpstream, MockResolver};
use tokio::sync::mpsc;

/// Wie lange ein Scan über ein `/24` höchstens dauern darf (HUM-076).
const BUDGET: Duration = Duration::from_secs(5);

/// Ein Egress, der jeden Verbindungsversuch zählt.
///
/// Die Zahl ist die Zusicherung „ohne Klick kein einziger Verbindungsversuch":
/// Sie steht auf null, solange niemand [`discover`] aufruft.
struct CountingEgress {
    inner: Direct,
    attempts: Arc<AtomicUsize>,
}

#[async_trait]
impl Egress for CountingEgress {
    async fn connect(
        &self,
        authority: &Authority,
        resolved: Option<IpAddr>,
    ) -> Result<Box<dyn AsyncStream>, Diagnostic> {
        self.attempts.fetch_add(1, Ordering::SeqCst);
        self.inner.connect(authority, resolved).await
    }
}

/// Eine Probe über dem gewöhnlichen Egress; der Resolver antwortet nur für
/// Namen, und die Suche fragt ausschließlich Adressen.
fn probe() -> LlmProbe {
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
    LlmProbe::new(Upstream::new(
        Arc::new(Direct::default()),
        resolver as Arc<dyn Resolver>,
        ClientTls::new(&[], false).unwrap(),
        config.prefer,
        Duration::from_secs(5),
    ))
}

/// Sammelt, was ein Scan findet.
async fn run(scan: Scan, egress: Arc<dyn Egress>) -> Vec<Found> {
    let (tx, mut rx) = mpsc::channel(16);
    let scanning = tokio::spawn({
        let probe = Arc::new(probe());
        async move { discover(probe, egress, &scan, tx).await }
    });
    let mut found = Vec::new();
    while let Some(result) = rx.recv().await {
        found.push(result);
    }
    scanning.await.unwrap();
    found
}

/// Der Mock antwortet, und was in der Liste steht, hat er selbst gesagt.
#[tokio::test(flavor = "multi_thread")]
async fn detects_ollama_mock() {
    let upstream = FakeUpstream::ollama().await;
    let scan = Scan::new(Subnet::single(Ipv4Addr::LOCALHOST))
        .with_ports(vec![upstream.port()])
        .expect("the ports of this scan");

    let found = run(scan, Arc::new(Direct::default())).await;

    assert_eq!(found.len(), 1, "one server answered: {found:?}");
    let server = &found[0];
    assert_eq!(server.host, Ipv4Addr::LOCALHOST);
    assert_eq!(server.port, upstream.port());
    assert_eq!(server.flavor, LlmFlavor::Ollama);
    assert_eq!(
        server.models,
        vec!["qwen2.5-coder:14b".to_owned(), "llama3.1:8b".to_owned()],
        "the models come from the server, not from the search"
    );
    assert!(!server.auth_required);
}

/// Ein Port, an dem niemand horcht, erscheint nicht — und er erscheint auch
/// nicht als „unbekannt". Ohne diese Zusicherung stünde in der Liste des
/// Setups eine Zeile je geschlossenem Port.
#[tokio::test(flavor = "multi_thread")]
async fn a_closed_port_is_not_a_server() {
    let upstream = FakeUpstream::ollama().await;
    let ollama = upstream.port();
    let silent = if ollama == u16::MAX { 1024 } else { ollama + 1 };
    let scan = Scan::new(Subnet::single(Ipv4Addr::LOCALHOST))
        .with_ports(vec![silent])
        .expect("the ports of this scan");

    let found = run(scan, Arc::new(Direct::default())).await;

    assert!(found.is_empty(), "nothing listens there: {found:?}");
}

/// Ein Server, der auf einem der vier Ports horcht, aber keine der beiden APIs
/// beantwortet, steht als `unknown` in der Liste — ohne Modelle.
///
/// So steht es in der Spezifikation (`product: ollama|openai_compatible|
/// unknown`), und es ist die ehrlichere Hälfte: Der Mensch sieht, dass dort
/// etwas horcht, und sieht zugleich, dass es sich nicht als LLM ausgewiesen
/// hat. Eine leere Liste über einem laufenden Server wäre die schlechtere
/// Auskunft; eine erfundene Modellzeile die schlechteste.
#[tokio::test(flavor = "multi_thread")]
async fn a_server_without_a_known_api_is_listed_as_unknown() {
    let upstream = FakeUpstream::plain().await;
    let scan = Scan::new(Subnet::single(Ipv4Addr::LOCALHOST))
        .with_ports(vec![upstream.port()])
        .expect("the ports of this scan");

    let found = run(scan, Arc::new(Direct::default())).await;

    assert_eq!(found.len(), 1, "something listens there: {found:?}");
    assert_eq!(found[0].flavor, LlmFlavor::Unknown);
    assert!(
        found[0].models.is_empty(),
        "nothing answered with models, so none are claimed"
    );
    assert!(!found[0].auth_required);
}

/// Ein Server, der `401` sagt, bleibt in der Liste, mit Vermerk und ohne
/// erfundene Modelle (Fallstrick der Spezifikation: vLLM hinter einer
/// Anmeldung).
#[tokio::test(flavor = "multi_thread")]
async fn a_server_that_wants_credentials_stays_in_the_list() {
    let upstream = FakeUpstream::needs_auth().await;
    let scan = Scan::new(Subnet::single(Ipv4Addr::LOCALHOST))
        .with_ports(vec![upstream.port()])
        .expect("the ports of this scan");

    let found = run(scan, Arc::new(Direct::default())).await;

    assert_eq!(found.len(), 1, "the server answered, with a 401: {found:?}");
    assert!(found[0].auth_required);
    assert!(
        found[0].models.is_empty(),
        "nobody asked it for models, so none are claimed"
    );
}

/// Ohne Aufruf kein Paket. Die Zusicherung ist die Hälfte des Versprechens
/// über dem Knopf; die andere Hälfte ist der Text selbst.
#[tokio::test(flavor = "multi_thread")]
async fn nothing_connects_before_the_search_is_started() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let egress = Arc::new(CountingEgress {
        inner: Direct::default(),
        attempts: Arc::clone(&attempts),
    });
    let upstream = FakeUpstream::ollama().await;
    let scan = Scan::new(Subnet::single(Ipv4Addr::LOCALHOST))
        .with_ports(vec![upstream.port()])
        .expect("the ports of this scan");

    // Alles steht bereit: Probe, Egress, Auftrag. Solange niemand `discover`
    // ruft, bleibt der Zähler auf null.
    assert_eq!(attempts.load(Ordering::SeqCst), 0);

    let found = run(scan, egress).await;

    assert_eq!(found.len(), 1);
    assert_eq!(
        attempts.load(Ordering::SeqCst),
        1,
        "exactly one attempt for the one address and port of this scan"
    );
}

/// Der Scan hört auf, wenn niemand mehr zuhört.
///
/// Das ist der Abbruch-Knopf des Blattes: Wer es schließt, lässt den Empfänger
/// fallen, und der Scan darf danach nicht weiter 1016 Adressen abklopfen.
#[tokio::test(flavor = "multi_thread")]
async fn closing_the_receiver_ends_the_scan() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let egress = Arc::new(CountingEgress {
        inner: Direct::default(),
        attempts: Arc::clone(&attempts),
    });
    let scan = Scan::new(Subnet::parse("127.0.0.0/24").unwrap());

    let (tx, rx) = mpsc::channel(1);
    drop(rx);
    let started = Instant::now();
    discover(Arc::new(probe()), egress, &scan, tx).await;

    assert!(
        attempts.load(Ordering::SeqCst) < 254 * DEFAULT_PORTS.len(),
        "the scan kept going after the sheet was closed: {} attempts",
        attempts.load(Ordering::SeqCst)
    );
    assert!(
        started.elapsed() < BUDGET,
        "the scan took {:?} after the receiver was gone",
        started.elapsed()
    );
}

/// Die Zusage „ein `/24` ohne Treffer endet in unter fünf Sekunden" ist eine
/// Rechnung über zwei Konstanten, und dieser Test hält sie fest.
///
/// 254 Adressen mal vier Ports, [`PARALLEL`] gleichzeitig, jeder Versuch
/// höchstens [`CONNECT_TIMEOUT`]: Der schlimmste Fall ist, dass alle Adressen
/// schweigen und jede Frist voll abläuft. Wer eine der beiden Zahlen ändert,
/// merkt hier, ob die Zusage noch gilt.
#[test]
fn the_time_budget_of_a_full_scan_holds() {
    let attempts = 254 * DEFAULT_PORTS.len();
    let rounds = attempts.div_ceil(PARALLEL);
    let worst_case = CONNECT_TIMEOUT * u32::try_from(rounds).unwrap();

    assert_eq!(attempts, 1016);
    assert_eq!(rounds, 16);
    assert!(
        worst_case < BUDGET,
        "a /24 without a single answer would take {worst_case:?}, more than the {BUDGET:?} the \
         setup promises"
    );
}

/// Und die gemessene Hälfte derselben Zusage: ein ganzes `/24` auf der
/// Schleife, an dem nur ein einziger Port antwortet.
#[tokio::test(flavor = "multi_thread")]
async fn a_scan_over_a_full_24_stays_in_the_budget() {
    let upstream = FakeUpstream::ollama().await;
    let scan = Scan::new(Subnet::parse("127.0.0.0/24").unwrap())
        .with_ports(vec![upstream.port()])
        .expect("the ports of this scan")
        .first(vec![Ipv4Addr::LOCALHOST]);

    let started = Instant::now();
    let found = run(scan, Arc::new(Direct::default())).await;
    let took = started.elapsed();

    assert_eq!(found.len(), 1, "one server on the loopback: {found:?}");
    assert_eq!(found[0].host, Ipv4Addr::LOCALHOST);
    assert!(
        took < BUDGET,
        "the scan over a /24 took {took:?}, more than the {BUDGET:?} the setup promises"
    );
}

/// Das eigene Netz, so wie diese Maschine es beschreibt.
///
/// Der Test läuft überall: Auf einem Rechner mit Vorgaberoute muss das
/// Ergebnis in sich stimmen — das `/24` gehört zur eigenen Adresse, und die
/// Schnittstelle hat einen Namen. Auf einem Läufer ohne Vorgaberoute (ein
/// Container ohne Netz) ist die Weigerung `LLM_008` das richtige Ergebnis und
/// keine Panne. Was der Test ausschließt, ist das Dazwischen: ein Netz, das
/// nicht zur Adresse passt, oder eine Weigerung ohne Begründung.
#[tokio::test(flavor = "multi_thread")]
async fn the_local_network_belongs_to_the_own_address() {
    match humanitl_proxy::local_net().await {
        Ok(local) => {
            assert_eq!(
                local.subnet,
                Subnet::local_24(local.own),
                "the /24 must be the one around the own address"
            );
            assert!(!local.interface.is_empty(), "the interface has a name");
            assert!(
                local.subnet.hosts().any(|host| host == local.own),
                "the own address is one of the addresses that get asked"
            );
        }
        Err(refusal) => {
            assert_eq!(refusal.code.as_str(), "LLM_008");
            assert!(!refusal.why.is_empty(), "a refusal says why");
        }
    }
}
