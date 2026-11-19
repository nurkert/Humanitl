//! Speicher der Flow-Registry über eine lange Sitzung (HUM-556).
//!
//! Ein zählender Allokator misst, was auf dem Heap liegt, während eine
//! Sitzung viele kleine Anfragen bis `Recorded` durchschickt, so wie der
//! Handler sie über [`FlowRegistry::record`] mitschreibt. Zwei Aussagen:
//!
//! - Ein Datensatz einer kleinen Anfrage belegt samt seinem Platz in der
//!   Tabelle ungefähr so viel, wie der Modulkommentar der Registry nennt; die
//!   Zahl steht mit `--nocapture` im Log.
//! - Mit der Grenze wächst der Speicher nach dem Erreichen der Grenze nicht
//!   mehr, gleich wie viele Anfragen noch kommen. Ohne Verdrängung wüchse er
//!   um die Größe eines Datensatzes je Anfrage, und der Test fiele.
//!
//! Der Allokator gilt für das ganze Testprogramm; deshalb steht hier genau ein
//! Test, damit kein zweiter gleichzeitig mitzählt.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::SystemTime;

use http::HeaderValue;
use humanitl_config::Limits;
use humanitl_core::{
    Authority, BodyRef, Decision, DecisionSource, Flow, FlowId, HeaderMap, HostName, HttpRequest,
    Method, Scheme, SessionId, TransitionInput,
};
use humanitl_proxy::ConnectionContext;
use humanitl_proxy::registry::{FlowRecord, FlowRegistry};

/// Zählt die belegten Bytes des Heaps.
struct Counting;

static LIVE: AtomicIsize = AtomicIsize::new(0);

#[allow(clippy::cast_possible_wrap)]
fn bytes(layout: Layout) -> isize {
    layout.size() as isize
}

// SAFETY: Reicht jeden Aufruf unverändert an `System` weiter und zählt nur mit.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: Derselbe Vertrag wie der Aufruf selbst.
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            LIVE.fetch_add(bytes(layout), Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: Derselbe Vertrag wie der Aufruf selbst.
        unsafe { System.dealloc(ptr, layout) };
        LIVE.fetch_sub(bytes(layout), Ordering::Relaxed);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: Derselbe Vertrag wie der Aufruf selbst.
        let new = unsafe { System.realloc(ptr, layout, new_size) };
        if !new.is_null() {
            #[allow(clippy::cast_possible_wrap)]
            LIVE.fetch_add(new_size as isize - bytes(layout), Ordering::Relaxed);
        }
        new
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn live() -> isize {
    LIVE.load(Ordering::Relaxed)
}

/// Eine kleine Anfrage, wie ein Agent sie an eine API schickt: sechs
/// Kopfzeilen, ein Pfad mit Query, der Body nur als Verweis.
fn small_request(n: usize) -> HttpRequest {
    let mut headers = HeaderMap::new();
    for (name, value) in [
        ("host", "api.example.com".to_owned()),
        ("user-agent", "agent/1.2.3 (linux; x86_64)".to_owned()),
        ("accept", "application/json".to_owned()),
        ("content-type", "application/json".to_owned()),
        ("content-length", "128".to_owned()),
        ("authorization", format!("Bearer sk-test-{n:0>48}")),
    ] {
        headers.insert(name, HeaderValue::from_str(&value).unwrap());
    }
    HttpRequest::new(
        Method::POST,
        Scheme::Https,
        Authority::with_scheme(HostName::Dns("api.example.com".to_owned()), Scheme::Https),
        format!("/v1/items/{n}?expand=owner&page=1"),
    )
    .with_headers(headers)
    .with_body(BodyRef::detached([7; 32], 128))
}

/// Schickt eine Anfrage durch bis `Recorded`.
fn one_request(registry: &FlowRegistry, session: SessionId, n: usize) -> FlowId {
    let mut flow = Flow::new(FlowId::new(), session, SystemTime::now(), small_request(n));
    registry.insert(FlowRecord::new(&flow, &ConnectionContext::plain(session)));
    for input in [
        TransitionInput::Analyze { findings: vec![] },
        TransitionInput::Decide {
            decision: Decision::Allow,
            source: DecisionSource::Passthrough,
        },
        TransitionInput::Forward,
        TransitionInput::Respond { status: 200 },
        TransitionInput::Record,
    ] {
        let event = flow.apply(input, SystemTime::now()).unwrap();
        registry.record(&event);
    }
    flow.id
}

#[test]
fn a_long_session_keeps_the_registry_bounded() {
    const CAP: usize = 1_000;
    const REQUESTS: usize = 64 * CAP;

    // Was ein Datensatz kostet: eine Registry, deren Grenze über der Zahl der
    // Anfragen liegt, also nichts verdrängt.
    let session = SessionId::new();
    let before = live();
    let unbounded = FlowRegistry::new(&Limits {
        registry_max_finished_flows: u32::try_from(2 * CAP).unwrap(),
        ..Limits::default()
    });
    for n in 0..2 * CAP {
        one_request(&unbounded, session, n);
    }
    let per_record = (live() - before) / isize::try_from(2 * CAP).unwrap();
    println!(
        "HUM-556: {per_record} bytes of heap per finished flow ({} bytes inline)",
        std::mem::size_of::<FlowRecord>()
    );
    assert_eq!(unbounded.len(), 2 * CAP);
    drop(unbounded);

    // Mit Grenze: Der Speicher steigt, bis die Tabelle ihre Größe gefunden
    // hat, und bleibt dann stehen, gleich wie viele Anfragen noch kommen.
    let before = live();
    let registry = FlowRegistry::new(&Limits {
        registry_max_finished_flows: u32::try_from(CAP).unwrap(),
        ..Limits::default()
    });
    let mut done = 0;
    let mut checkpoint = 2 * CAP;
    while done < REQUESTS {
        for n in done..checkpoint {
            one_request(&registry, session, n);
        }
        done = checkpoint;
        println!(
            "HUM-556: cap {CAP}: {} bytes after {done} requests",
            live() - before
        );
        checkpoint *= 2;
    }
    let at_end = live() - before;
    assert_eq!(registry.len(), CAP);
    // Gemessen rund 1,8 Datensätze je Platz der Grenze, weil die Splitter der
    // Tabelle Luft behalten; ohne Verdrängung wären es 64.
    let bound = 3 * per_record * isize::try_from(CAP).unwrap();
    assert!(
        at_end <= bound,
        "{at_end} bytes after {REQUESTS} requests with a cap of {CAP}, more than {bound}"
    );
}
