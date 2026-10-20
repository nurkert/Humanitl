//! Lasttest: Proxy und Oberfläche blockieren sich nicht gegenseitig (HUM-057).
//!
//! Ein Agent schickt 1000 Anfragen in zehn Sekunden (100 je Sekunde) an ein
//! erlaubtes Ziel, und ein Zuhörer am Ereignisstrom ist absichtlich langsam:
//! Er braucht für jedes Ereignis 20 ms, schafft also 50 je Sekunde, während
//! der Proxy rund siebenmal so viele erzeugt. Gemessen wird im Daemon, nicht
//! beim Zuhörer:
//!
//! - kein Fluss geht verloren: alle 1000 stehen als `recorded` in der
//!   Aufzeichnung;
//! - der langsame Zuhörer bekommt `Lagged` und findet danach in der
//!   Aufzeichnung alle 1000, so wie die Oberfläche über `ListFlows` nachlädt;
//! - p95 der Zeit von `Received` bis `Decided` unter 50 ms, beides Zeitstempel,
//!   die der Proxy selbst setzt;
//! - der höchste residente Speicher des Prozesses unter 300 MiB.
//!
//! Der Test läuft lokal nicht mit (`#[ignore]`): Er dauert über zehn Sekunden
//! und misst Zeiten, die ein belasteter Arbeitsrechner verfälscht. Die CI
//! fährt ihn in einem eigenen Schritt mit `--ignored --nocapture`, damit die
//! Messwerte im Job-Log stehen (`.github/workflows/ci.yml`).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg(target_os = "linux")]

mod support;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime};

use humanitl_core::{FlowEvent, FlowId};
use humanitl_proxy::hold::next_event;
use humanitl_recorder::FlowQuery;
use hyper::StatusCode;
use support::{FakeUpstream, ProxyBuilder, get};
use tokio::sync::broadcast;
use tokio::task::{JoinHandle, JoinSet};

/// So viele Anfragen schickt der Agent.
const FLOWS: usize = 1000;

/// In diesem Abstand, also 100 je Sekunde.
const PACE: Duration = Duration::from_millis(10);

/// So lange braucht der langsame Zuhörer für ein Ereignis.
const SLOW: Duration = Duration::from_millis(20);

/// Die Schwelle für p95 von `Received` bis `Decided`.
const P95_LIMIT: Duration = Duration::from_millis(50);

/// Die Schwelle für den höchsten residenten Speicher.
const RSS_LIMIT_KIB: u64 = 300 * 1024;

/// Ein Feld aus `/proc/self/status` in KiB.
fn status_kib(field: &str) -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    status
        .lines()
        .find_map(|line| line.strip_prefix(field)?.strip_prefix(':'))
        .and_then(|rest| rest.trim().trim_end_matches("kB").trim().parse().ok())
        .unwrap_or_else(|| panic!("no {field} in /proc/self/status"))
}

/// Das `p`-Quantil einer sortierten Liste.
fn quantile(sorted: &[Duration], p: f64) -> Duration {
    assert!(!sorted.is_empty());
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    let index = ((sorted.len() as f64 * p).ceil() as usize).clamp(1, sorted.len()) - 1;
    sorted[index]
}

/// Der schnelle Zuhörer misst: Er tut nichts, als die Zeitstempel des Proxys
/// aufzuschreiben, und darf deshalb selbst nie überholt werden. Liefert die
/// Spannen `Received` bis `Decided` und die Zahl verlorener Ereignisse.
fn measure(mut rx: broadcast::Receiver<FlowEvent>) -> JoinHandle<(Vec<Duration>, u64)> {
    tokio::spawn(async move {
        let mut received: HashMap<FlowId, SystemTime> = HashMap::new();
        let mut latencies = Vec::with_capacity(FLOWS);
        let mut recorded = 0usize;
        let mut lagged = 0u64;
        while recorded < FLOWS {
            match next_event(&mut rx).await {
                Some(FlowEvent::Received { flow_id, at, .. }) => {
                    received.insert(flow_id, at);
                }
                Some(FlowEvent::Decided { flow_id, at, .. }) => {
                    if let Some(start) = received.get(&flow_id) {
                        latencies.push(at.duration_since(*start).unwrap_or_default());
                    }
                }
                Some(FlowEvent::Recorded { .. }) => recorded += 1,
                Some(FlowEvent::Lagged { n }) => lagged += n,
                Some(_) => {}
                None => break,
            }
        }
        (latencies, lagged)
    })
}

/// Der langsame Zuhörer: 20 ms je Ereignis, wie eine Oberfläche, die mit dem
/// Zeichnen nicht hinterherkommt. Liefert, wie oft er überholt wurde.
fn slow_subscriber(
    mut rx: broadcast::Receiver<FlowEvent>,
    stop: Arc<AtomicBool>,
) -> JoinHandle<u64> {
    tokio::spawn(async move {
        let mut lagged = 0u64;
        while !stop.load(Ordering::Relaxed) {
            let next = tokio::time::timeout(Duration::from_millis(200), next_event(&mut rx));
            match next.await {
                Ok(Some(FlowEvent::Lagged { .. })) => lagged += 1,
                Ok(Some(_)) => tokio::time::sleep(SLOW).await,
                Ok(None) => break,
                Err(_) => {}
            }
        }
        lagged
    })
}

/// Der höchste residente Speicher, jede Sekunde gelesen, bis `stop`.
fn rss_sampler(stop: Arc<AtomicBool>) -> JoinHandle<u64> {
    tokio::spawn(async move {
        let mut max = status_kib("VmRSS");
        while !stop.load(Ordering::Relaxed) {
            tokio::time::sleep(Duration::from_secs(1)).await;
            max = max.max(status_kib("VmRSS"));
        }
        max
    })
}

/// Der Agent: 100 Anfragen je Sekunde, jede auf eigener Verbindung. Liefert
/// die Statuscodes mit ihrer Zahl.
async fn agent(socket: &Path, port: u16) -> HashMap<StatusCode, usize> {
    let mut agent = JoinSet::new();
    let mut tick = tokio::time::interval(PACE);
    for index in 0..FLOWS {
        tick.tick().await;
        let socket = socket.to_owned();
        agent.spawn(async move {
            let stream = tokio::net::UnixStream::connect(&socket).await.unwrap();
            let (mut sender, conn) =
                hyper::client::conn::http1::handshake(hyper_util::rt::TokioIo::new(stream))
                    .await
                    .unwrap();
            tokio::spawn(conn);
            let url = format!("http://127.0.0.1:{port}/status/200?n={index}");
            sender.send_request(get(&url)).await.unwrap().status()
        });
    }
    let mut statuses = HashMap::new();
    while let Some(status) = agent.join_next().await {
        *statuses.entry(status.unwrap()).or_insert(0usize) += 1;
    }
    statuses
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "load test, over ten seconds; CI runs it with --ignored --nocapture"]
async fn backpressure_1000_flows() {
    let upstream = FakeUpstream::plain().await;
    let port = upstream.port();
    let rules = format!(
        "version: 1\nrules:\n  - action: allow\n    match:\n      host: \"ip:127.0.0.1\"\n      \
         port: {port}\n"
    );
    let proxy = ProxyBuilder::new()
        .rules(&rules)
        .recording(true)
        .start()
        .await;

    let stop = Arc::new(AtomicBool::new(false));
    let measuring = measure(proxy.queue.subscribe());
    let slow = slow_subscriber(proxy.queue.subscribe(), Arc::clone(&stop));
    let rss = rss_sampler(Arc::clone(&stop));

    let started = Instant::now();
    let statuses = agent(&proxy.socket, port).await;
    let sent_in = started.elapsed();

    let (mut latencies, fast_lagged) = tokio::time::timeout(Duration::from_secs(30), measuring)
        .await
        .expect("every flow is recorded in time")
        .unwrap();
    stop.store(true, Ordering::Relaxed);
    let slow_lagged = slow.await.unwrap();
    let max_rss = rss.await.unwrap().max(status_kib("VmHWM"));

    // Wie die Oberfläche nach `Lagged`: nachladen, hier aus der Aufzeichnung.
    let recorder = proxy.recorder.as_ref().expect("recording is on");
    recorder.flush().await;
    let page = recorder
        .list_flows(&FlowQuery::new("state:recorded"))
        .await
        .unwrap();

    latencies.sort();
    let p50 = quantile(&latencies, 0.50);
    let p95 = quantile(&latencies, 0.95);
    let p99 = quantile(&latencies, 0.99);
    println!(
        "backpressure_1000_flows: sent {FLOWS} in {sent_in:?}, statuses {statuses:?}, \
         received->decided p50 {p50:?} p95 {p95:?} p99 {p99:?} (n = {}), max RSS {} MiB, \
         slow subscriber Lagged {slow_lagged} times, fast subscriber lost {fast_lagged}, \
         recorded in the history {} (capped: {})",
        latencies.len(),
        max_rss / 1024,
        page.total_estimate,
        page.capped,
    );

    assert_eq!(statuses.get(&StatusCode::OK), Some(&FLOWS), "{statuses:?}");
    assert_eq!(
        fast_lagged, 0,
        "the measuring subscriber must see every event"
    );
    assert_eq!(latencies.len(), FLOWS, "a decision for every flow");
    assert_eq!(page.total_estimate, FLOWS as u64, "no flow is lost");
    assert!(!page.capped);
    assert!(slow_lagged >= 1, "the slow subscriber was overtaken");
    assert!(p95 < P95_LIMIT, "p95 {p95:?} over {P95_LIMIT:?}");
    assert!(
        max_rss < RSS_LIMIT_KIB,
        "max RSS {} MiB over {} MiB",
        max_rss / 1024,
        RSS_LIMIT_KIB / 1024
    );
}
