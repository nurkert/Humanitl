//! Der Egress über einen Entscheidungs-Proxy des Nutzers (ADR-0022, HUM-511).
//!
//! Gemessen an einem Stellvertreter-Proxy im Test
//! ([`support::stand_in::StandIn`]): Er nimmt `CONNECT` und Anfragen in
//! absoluter Form an, schreibt mit, was ankommt (Anfragezeile, `Host`,
//! `Proxy-Authorization`, SNI), und beantwortet die Anfragen selbst, durch
//! den Tunnel mit einem Blatt aus seiner eigenen CA, so wie mitmproxy es
//! tut. Der Weiterleiter wird genau so gebaut wie im Daemon
//! ([`humanitl_proxy::egress::from_config`]); der Namensdienst ist ein
//! [`MockResolver`], der jede Frage zählt.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use http_body_util::BodyExt as _;
use humanitl_config::{Config, IpPreference, ProxyResolve};
use humanitl_core::diagnostics::codes::{
    PROXY_012, PROXY_013, PROXY_014, PROXY_015, PROXY_016, PROXY_017, PROXY_018, PROXY_019,
};
use humanitl_core::{Authority, HostName, HttpRequest, Method, Scheme, UpstreamError};
use humanitl_proxy::egress::{Resolution, decision_proxy, from_config};
use humanitl_proxy::{
    ClientTls, Direct, ForwardError, Resolver, TrustAnchors, Upstream, roots_from_pem,
};
use tokio::net::TcpListener;

use support::stand_in::{Behaviour, ConnectAnswer, HOST, PUBLIC, StandIn, ca_file, config};
use support::{FakeUpstream, ForeignCa, MockResolver};

/// Der Weiterleiter, wie `humanitld` ihn baut.
fn upstream(config: &Config, resolver: &Arc<MockResolver>) -> Upstream {
    let wiring = from_config(config, &[], None).unwrap();
    Upstream::new(
        wiring.egress,
        Arc::clone(resolver) as Arc<dyn Resolver>,
        wiring.client_tls,
        IpPreference::Ipv4,
        Duration::from_secs(config.limits.header_timeout_secs),
    )
}

fn request(scheme: Scheme, host: HostName, port: u16) -> HttpRequest {
    HttpRequest::new(
        Method::GET,
        scheme,
        Authority::new(host, port),
        "/echo".to_owned(),
    )
}

fn named(scheme: Scheme, port: u16) -> HttpRequest {
    request(scheme, HostName::Dns(HOST.to_owned()), port)
}

async fn body_of(response: hyper::Response<hyper::body::Incoming>) -> Bytes {
    response.into_body().collect().await.unwrap().to_bytes()
}

fn code_of(failure: &ForwardError) -> String {
    failure
        .diagnostic
        .as_ref()
        .map_or_else(|| "-".to_owned(), |d| d.code.as_str().to_owned())
}

#[tokio::test(flavor = "multi_thread")]
async fn with_proxy_resolve_the_name_goes_into_connect_and_nothing_is_resolved() {
    let ca = ForeignCa::new();
    let file = ca_file(&ca);
    let proxy = StandIn::start(&ca, Behaviour::default()).await;
    let resolver = Arc::new(MockResolver::answering(PUBLIC));
    let upstream = upstream(
        &config(proxy.addr, &file.path, ProxyResolve::Proxy),
        &resolver,
    );

    let response = upstream
        .forward(&named(Scheme::Https, 443), Bytes::new(), false)
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(body_of(response).await, "ok");

    let seen = proxy.seen();
    assert_eq!(seen.lines, ["CONNECT upstream.test:443 HTTP/1.1"]);
    assert_eq!(seen.sni, [HOST], "TLS through the tunnel names the host");
    assert_eq!(
        seen.inner_lines,
        ["GET /echo HTTP/1.1"],
        "origin form inside"
    );
    assert_eq!(seen.inner_hosts, [HOST]);
    assert!(
        resolver.hosts().is_empty(),
        "Humanitl must not resolve: {:?}",
        resolver.hosts()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn with_humanitl_resolve_the_address_goes_into_connect_and_sni_names_the_host() {
    let ca = ForeignCa::new();
    let file = ca_file(&ca);
    let proxy = StandIn::start(&ca, Behaviour::default()).await;
    let resolver = Arc::new(MockResolver::answering(PUBLIC));
    let upstream = upstream(
        &config(proxy.addr, &file.path, ProxyResolve::Humanitl),
        &resolver,
    );

    let response = upstream
        .forward(&named(Scheme::Https, 443), Bytes::new(), false)
        .await
        .unwrap();
    assert_eq!(body_of(response).await, "ok");

    let seen = proxy.seen();
    assert_eq!(seen.lines, ["CONNECT 93.184.216.34:443 HTTP/1.1"]);
    assert_eq!(seen.sni, [HOST], "SNI carries the name, not the address");
    assert_eq!(seen.inner_hosts, [HOST]);
    assert_eq!(
        resolver.hosts(),
        [HOST],
        "resolved exactly once, in Humanitl"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn private_literals_and_local_names_are_refused_before_the_proxy_sees_anything() {
    let ca = ForeignCa::new();
    let file = ca_file(&ca);
    let proxy = StandIn::start(&ca, Behaviour::default()).await;

    // `"proxy"`: nichts wird aufgelöst, Literale und lokale Namen fallen.
    let resolver = Arc::new(MockResolver::answering(PUBLIC));
    let through = upstream(
        &config(proxy.addr, &file.path, ProxyResolve::Proxy),
        &resolver,
    );
    for (host, scheme) in [
        (HostName::Ip(IpAddr::V4(Ipv4Addr::LOCALHOST)), Scheme::Https),
        (
            HostName::Ip(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))),
            Scheme::Http,
        ),
        (
            HostName::Ip(IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254))),
            Scheme::Http,
        ),
    ] {
        let failure = through
            .forward(&request(scheme, host.clone(), 443), Bytes::new(), false)
            .await
            .unwrap_err();
        assert!(
            matches!(failure.error, UpstreamError::PrivateAddress(_)),
            "{host}: {failure:?}"
        );
    }
    for name in [
        "localhost",
        "api.localhost",
        "metadata.google.internal",
        "nas.local",
        "metadata",
    ] {
        let failure = through
            .forward(
                &request(Scheme::Https, HostName::Dns(name.to_owned()), 443),
                Bytes::new(),
                false,
            )
            .await
            .unwrap_err();
        assert_eq!(failure.error, UpstreamError::Dns, "{name}");
        assert!(failure.local_name, "{name}: the handler reports PROXY_016");
    }
    assert!(resolver.hosts().is_empty(), "{:?}", resolver.hosts());

    // `"humanitl"`: Ein lokaler Name fällt vor jeder Auflösung, auch wenn
    // `resolver.overrides` ihn auf eine öffentliche Adresse legt; ein
    // privates Literal und ein Name mit privater Antwort fallen bei der
    // Prüfung der Adresse.
    let resolver = Arc::new(MockResolver::answering(PUBLIC).with_answer(
        "intranet.example.com",
        vec![IpAddr::V4(Ipv4Addr::new(10, 1, 2, 3))],
    ));
    let mut overridden = config(proxy.addr, &file.path, ProxyResolve::Humanitl);
    overridden
        .resolver
        .overrides
        .insert("localhost".to_owned(), PUBLIC.to_string());
    let resolving = upstream(&overridden, &resolver);
    for name in ["localhost", "metadata", "nas.local"] {
        let failure = resolving
            .forward(
                &request(Scheme::Https, HostName::Dns(name.to_owned()), 443),
                Bytes::new(),
                false,
            )
            .await
            .unwrap_err();
        assert_eq!(failure.error, UpstreamError::Dns, "{name}");
        assert!(failure.local_name, "{name}");
    }
    assert!(
        resolver.hosts().is_empty(),
        "a local name is refused before any resolution: {:?}",
        resolver.hosts()
    );
    for host in [
        HostName::Ip(IpAddr::V4(Ipv4Addr::LOCALHOST)),
        HostName::Dns("intranet.example.com".to_owned()),
    ] {
        let failure = resolving
            .forward(
                &request(Scheme::Https, host.clone(), 443),
                Bytes::new(),
                false,
            )
            .await
            .unwrap_err();
        assert!(
            matches!(failure.error, UpstreamError::PrivateAddress(_)),
            "{host}: {failure:?}"
        );
    }

    assert!(
        proxy.seen().lines.is_empty(),
        "the proxy saw {:?}",
        proxy.seen().lines
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn plain_http_goes_in_absolute_form_with_the_name_in_host() {
    let ca = ForeignCa::new();
    let file = ca_file(&ca);
    let proxy = StandIn::start(&ca, Behaviour::default()).await;

    let resolver = Arc::new(MockResolver::answering(PUBLIC));
    let response = upstream(
        &config(proxy.addr, &file.path, ProxyResolve::Proxy),
        &resolver,
    )
    .forward(&named(Scheme::Http, 8080), Bytes::new(), false)
    .await
    .unwrap();
    assert_eq!(body_of(response).await, "ok");
    let response = upstream(
        &config(proxy.addr, &file.path, ProxyResolve::Humanitl),
        &resolver,
    )
    .forward(&named(Scheme::Http, 8080), Bytes::new(), false)
    .await
    .unwrap();
    assert_eq!(body_of(response).await, "ok");

    let seen = proxy.seen();
    assert_eq!(
        seen.lines,
        [
            "GET http://upstream.test:8080/echo HTTP/1.1",
            "GET http://93.184.216.34:8080/echo HTTP/1.1",
        ]
    );
    assert_eq!(seen.hosts, ["upstream.test:8080", "upstream.test:8080"]);
    assert_eq!(
        resolver.hosts(),
        [HOST],
        "only the \"humanitl\" run resolved"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_proxy_ca_is_trusted_only_through_the_proxy() {
    let ca = ForeignCa::new();
    let file = ca_file(&ca);
    let proxy = StandIn::start(&ca, Behaviour::default()).await;
    let config = config(proxy.addr, &file.path, ProxyResolve::Proxy);
    let target = FakeUpstream::tls_foreign(&ca, HOST).await;
    let resolver = Arc::new(MockResolver::answering(IpAddr::V4(Ipv4Addr::LOCALHOST)));

    // Die TLS-Konfiguration der direkten Verbindungen aus derselben
    // Verdrahtung kennt die CA des Proxys nicht.
    let wiring = from_config(&config, &[], None).unwrap();
    let direct = Upstream::new(
        Arc::new(Direct::default()),
        Arc::clone(&resolver) as Arc<dyn Resolver>,
        wiring.client_tls,
        IpPreference::Ipv4,
        Duration::from_secs(5),
    );
    let failure = direct
        .forward_to(
            &named(Scheme::Https, target.port()),
            Bytes::new(),
            IpAddr::V4(Ipv4Addr::LOCALHOST),
        )
        .await
        .unwrap_err();
    assert_eq!(failure.error, UpstreamError::Tls);
    assert!(failure.untrusted_issuer.is_some(), "{failure:?}");
    assert_eq!(target.hits(), 0);

    // Gegenprobe: mit der CA als Anker antwortet dasselbe Ziel.
    let trusting = Upstream::new(
        Arc::new(Direct::default()),
        Arc::clone(&resolver) as Arc<dyn Resolver>,
        ClientTls::new(
            &TrustAnchors::bundled(),
            &roots_from_pem(ca.pem().as_bytes()),
            false,
        )
        .unwrap(),
        IpPreference::Ipv4,
        Duration::from_secs(5),
    );
    trusting
        .forward_to(
            &named(Scheme::Https, target.port()),
            Bytes::new(),
            IpAddr::V4(Ipv4Addr::LOCALHOST),
        )
        .await
        .unwrap();

    // Durch den Proxy gilt sie.
    upstream(&config, &resolver)
        .forward(&named(Scheme::Https, 443), Bytes::new(), false)
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn another_ca_behind_the_proxy_is_a_finding_about_egress_proxy_ca() {
    let ca = ForeignCa::new();
    let other = ForeignCa::new();
    let file = ca_file(&other);
    let proxy = StandIn::start(&ca, Behaviour::default()).await;
    let resolver = Arc::new(MockResolver::answering(PUBLIC));

    let failure = upstream(
        &config(proxy.addr, &file.path, ProxyResolve::Proxy),
        &resolver,
    )
    .forward(&named(Scheme::Https, 443), Bytes::new(), false)
    .await
    .unwrap_err();
    assert_eq!(failure.error, UpstreamError::Tls);
    assert_eq!(code_of(&failure), PROXY_015.as_str());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_late_response_head_reaches_the_client_when_the_response_timeout_allows() {
    // Der Kopf kommt nach 3 s; `limits.header_timeout_secs` und
    // `limits.connect_timeout_secs` sind 1 s. Steht dort ein Mensch, der
    // eine Minute braucht, gilt dasselbe mit `egress.proxy_response_timeout_secs`
    // über 60.
    let ca = ForeignCa::new();
    let file = ca_file(&ca);
    let proxy = StandIn::start(
        &ca,
        Behaviour {
            head_delay: Duration::from_secs(3),
            ..Behaviour::default()
        },
    )
    .await;
    let resolver = Arc::new(MockResolver::answering(PUBLIC));
    let mut config = config(proxy.addr, &file.path, ProxyResolve::Proxy);
    config.limits.connect_timeout_secs = 1;
    config.limits.header_timeout_secs = 1;
    config.egress.proxy_response_timeout_secs = 6;

    for scheme in [Scheme::Https, Scheme::Http] {
        let response = upstream(&config, &resolver)
            .forward(&named(scheme, 443), Bytes::new(), false)
            .await
            .unwrap_or_else(|failure| panic!("{scheme:?}: {failure:?}"));
        assert_eq!(body_of(response).await, "ok");
    }

    // Kürzer als die Wartezeit des Proxys: `Timeout`.
    config.egress.proxy_response_timeout_secs = 1;
    config.limits.header_timeout_secs = 6;
    for scheme in [Scheme::Https, Scheme::Http] {
        let failure = upstream(&config, &resolver)
            .forward(&named(scheme, 443), Bytes::new(), false)
            .await
            .unwrap_err();
        assert_eq!(failure.error, UpstreamError::Timeout, "{scheme:?}");
        assert_eq!(code_of(&failure), PROXY_019.as_str(), "{scheme:?}");
        assert!(
            failure
                .diagnostic
                .as_ref()
                .unwrap()
                .why
                .contains(&proxy.addr.to_string()),
            "the finding names the proxy"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_silent_connect_is_cut_by_the_connect_timeout() {
    let ca = ForeignCa::new();
    let file = ca_file(&ca);
    let proxy = StandIn::start(
        &ca,
        Behaviour {
            connect: ConnectAnswer::Silent,
            ..Behaviour::default()
        },
    )
    .await;
    let resolver = Arc::new(MockResolver::answering(PUBLIC));
    let mut config = config(proxy.addr, &file.path, ProxyResolve::Proxy);
    config.limits.connect_timeout_secs = 1;
    config.egress.proxy_response_timeout_secs = 60;

    let started = Instant::now();
    let failure = upstream(&config, &resolver)
        .forward(&named(Scheme::Https, 443), Bytes::new(), false)
        .await
        .unwrap_err();
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "CONNECT waited {:?}, the response timeout instead of the connect timeout",
        started.elapsed()
    );
    assert_eq!(failure.error, UpstreamError::Connect);
    assert_eq!(code_of(&failure), PROXY_013.as_str());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_407_on_connect_and_on_plain_http_is_a_finding_and_no_answer() {
    let ca = ForeignCa::new();
    let file = ca_file(&ca);
    let proxy = StandIn::start(
        &ca,
        Behaviour {
            connect: ConnectAnswer::Status(407),
            plain_status: 407,
            ..Behaviour::default()
        },
    )
    .await;
    let resolver = Arc::new(MockResolver::answering(PUBLIC));
    let upstream = upstream(
        &config(proxy.addr, &file.path, ProxyResolve::Proxy),
        &resolver,
    );
    for scheme in [Scheme::Https, Scheme::Http] {
        let failure = upstream
            .forward(&named(scheme, 443), Bytes::new(), false)
            .await
            .unwrap_err();
        assert_eq!(failure.error, UpstreamError::Connect, "{scheme:?}");
        assert_eq!(code_of(&failure), PROXY_014.as_str(), "{scheme:?}");
    }

    let refusing = StandIn::start(
        &ca,
        Behaviour {
            connect: ConnectAnswer::Status(502),
            ..Behaviour::default()
        },
    )
    .await;
    let failure = self::upstream(
        &config(refusing.addr, &file.path, ProxyResolve::Proxy),
        &resolver,
    )
    .forward(&named(Scheme::Https, 443), Bytes::new(), false)
    .await
    .unwrap_err();
    assert_eq!(code_of(&failure), PROXY_013.as_str());
    assert!(
        failure.diagnostic.unwrap().why.contains("502"),
        "the status is named"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn credentials_reach_the_proxy_and_never_the_target() {
    let ca = ForeignCa::new();
    let file = ca_file(&ca);
    let proxy = StandIn::start(&ca, Behaviour::default()).await;
    let config = config(proxy.addr, &file.path, ProxyResolve::Proxy);
    let egress = decision_proxy(
        &config.egress,
        &TrustAnchors::bundled(),
        &[],
        false,
        Duration::from_secs(2),
        None,
    )
    .unwrap()
    .unwrap()
    .with_credentials("alice:pw");
    assert_eq!(
        humanitl_proxy::Egress::resolution(&egress),
        Resolution::Proxy
    );
    let upstream = Upstream::new(
        Arc::new(egress),
        Arc::new(MockResolver::answering(PUBLIC)) as Arc<dyn Resolver>,
        ClientTls::new(&TrustAnchors::bundled(), &[], false).unwrap(),
        IpPreference::Ipv4,
        Duration::from_secs(2),
    );
    let mut with_agent_header = named(Scheme::Https, 443);
    with_agent_header.headers.insert(
        "proxy-authorization",
        "Basic YWdlbnQ6c2VjcmV0".parse().unwrap(),
    );
    upstream
        .forward(&with_agent_header, Bytes::new(), false)
        .await
        .unwrap();
    let mut plain = named(Scheme::Http, 80);
    plain.headers.insert(
        "proxy-authorization",
        "Basic YWdlbnQ6c2VjcmV0".parse().unwrap(),
    );
    upstream.forward(&plain, Bytes::new(), false).await.unwrap();

    let seen = proxy.seen();
    // `alice:pw`, nie die Zeile des Agenten.
    assert_eq!(seen.auth, ["Basic YWxpY2U6cHc=", "Basic YWxpY2U6cHc="]);
    assert_eq!(
        seen.inner_auth,
        [""],
        "nothing for the target inside the tunnel"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unreachable_or_unusable_proxy_fails_closed_with_a_finding() {
    let ca = ForeignCa::new();
    let file = ca_file(&ca);
    let resolver = Arc::new(MockResolver::answering(PUBLIC));
    // Ein Port, auf dem niemand lauscht.
    let closed = {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        listener.local_addr().unwrap()
    };
    let failure = upstream(&config(closed, &file.path, ProxyResolve::Proxy), &resolver)
        .forward(&named(Scheme::Https, 443), Bytes::new(), false)
        .await
        .unwrap_err();
    assert_eq!(failure.error, UpstreamError::Connect);
    assert_eq!(code_of(&failure), PROXY_012.as_str());

    let proxy = StandIn::start(&ca, Behaviour::default()).await;
    let missing = file.path.with_file_name("missing.pem");
    let failure = upstream(
        &config(proxy.addr, &missing, ProxyResolve::Proxy),
        &resolver,
    )
    .forward(&named(Scheme::Http, 80), Bytes::new(), false)
    .await
    .unwrap_err();
    assert_eq!(code_of(&failure), PROXY_017.as_str());
    assert!(proxy.seen().lines.is_empty(), "nothing left without the CA");
}

#[tokio::test(flavor = "multi_thread")]
async fn without_egress_proxy_the_egress_is_direct_and_humanitl_resolves() {
    let mut config = Config::default();
    config.tls.trust_system_store = false;
    let wiring = from_config(&config, &[], None).unwrap();
    assert_eq!(wiring.egress.resolution(), Resolution::Humanitl);

    let target = FakeUpstream::plain().await;
    let resolver = Arc::new(MockResolver::answering(IpAddr::V4(Ipv4Addr::LOCALHOST)));
    let upstream = Upstream::new(
        wiring.egress,
        Arc::clone(&resolver) as Arc<dyn Resolver>,
        wiring.client_tls,
        IpPreference::Ipv4,
        Duration::from_secs(5),
    );
    upstream
        .forward(&named(Scheme::Http, target.port()), Bytes::new(), true)
        .await
        .unwrap();
    assert_eq!(resolver.hosts(), [HOST]);
    assert_eq!(target.hits(), 1);
}

/// Der Befund des Wegs erreicht den Ereignisstrom, statt auf
/// `UpstreamError::Connect` verkürzt zu werden; der Agent bekommt `502`
/// (HUM-511, Punkt 3).
#[tokio::test(flavor = "multi_thread")]
async fn the_finding_of_the_decision_proxy_reaches_the_event_stream() {
    let ca = ForeignCa::new();
    let file = ca_file(&ca);
    let refusing = StandIn::start(
        &ca,
        Behaviour {
            plain_status: 407,
            ..Behaviour::default()
        },
    )
    .await;
    let config = config(refusing.addr, &file.path, ProxyResolve::Proxy);
    let egress = from_config(&config, &[], None).unwrap().egress;
    let proxy = support::ProxyBuilder::new()
        .passthrough()
        .allow_private(false)
        .egress(egress)
        .start()
        .await;
    let mut events = proxy.events();
    let mut client = proxy.client().await;
    let response = client.send(support::get("http://upstream.test/echo")).await;
    assert_eq!(response.status(), hyper::StatusCode::BAD_GATEWAY);
    let diagnostic = match events.wait_for("diagnostic").await {
        humanitl_core::FlowEvent::Diagnostic { diagnostic, .. } => *diagnostic,
        other => panic!("expected a Diagnostic event, got {other:?}"),
    };
    assert_eq!(diagnostic.code, PROXY_014);
    assert_eq!(
        refusing.seen().lines,
        ["GET http://upstream.test:80/echo HTTP/1.1"]
    );
    assert_eq!(proxy.resolver.calls(), 0, "nothing resolved in Humanitl");
}

/// `humanitl egress check`: Tunnel und TLS-Handschlag zum festen Ziel, ohne
/// eine Anfrage (HUM-511, Punkt 7).
#[tokio::test(flavor = "multi_thread")]
async fn the_check_opens_the_tunnel_and_the_handshake_and_sends_no_request() {
    let ca = ForeignCa::new();
    let file = ca_file(&ca);
    let (host, port) = Upstream::CHECK_TARGET;
    let proxy = StandIn::start_for(&ca, host, Behaviour::default()).await;
    let config = config(proxy.addr, &file.path, ProxyResolve::Proxy);

    Upstream::check_decision_proxy(&config, None).await.unwrap();
    let seen = proxy.seen();
    assert_eq!(seen.lines, [format!("CONNECT {host}:{port} HTTP/1.1")]);
    assert_eq!(seen.sni, [host]);
    assert!(
        seen.inner_lines.is_empty(),
        "no request: {:?}",
        seen.inner_lines
    );

    // Eine andere CA in `egress.proxy_ca`: `PROXY_015`.
    let other = ca_file(&ForeignCa::new());
    let mismatch = Upstream::check_decision_proxy(
        &self::config(proxy.addr, &other.path, ProxyResolve::Proxy),
        None,
    )
    .await
    .unwrap_err();
    assert_eq!(mismatch.code, PROXY_015);

    // Ohne `egress.proxy` gibt es nichts zu prüfen.
    let mut direct = Config::default();
    direct.tls.trust_system_store = false;
    let nothing = Upstream::check_decision_proxy(&direct, None)
        .await
        .unwrap_err();
    assert_eq!(nothing.code, PROXY_017);
}

/// Ein Zertifikat, das die CA des Proxys signiert hat, aber für einen
/// anderen Namen: `PROXY_018`, nicht nur `UpstreamError::Tls` (HUM-511).
#[tokio::test(flavor = "multi_thread")]
async fn a_certificate_for_another_name_behind_the_proxy_is_a_finding() {
    let ca = ForeignCa::new();
    let file = ca_file(&ca);
    let proxy = StandIn::start_for(&ca, "other.test", Behaviour::default()).await;
    let resolver = Arc::new(MockResolver::answering(PUBLIC));
    let failure = upstream(
        &config(proxy.addr, &file.path, ProxyResolve::Proxy),
        &resolver,
    )
    .forward(&named(Scheme::Https, 443), Bytes::new(), false)
    .await
    .unwrap_err();
    assert_eq!(failure.error, UpstreamError::Tls);
    assert_eq!(code_of(&failure), PROXY_018.as_str());
    let why = &failure.diagnostic.as_ref().unwrap().why;
    assert!(
        why.contains(HOST) && why.contains(&proxy.addr.to_string()),
        "{why}"
    );
}

/// Ein lokaler Name, für den eine Regel private Ziele erlaubt, geht zum
/// Proxy weiter, in beiden Arten der Auflösung (ADR-0022).
#[tokio::test(flavor = "multi_thread")]
async fn allow_private_lets_a_local_name_through_to_the_proxy() {
    let ca = ForeignCa::new();
    let file = ca_file(&ca);
    let proxy = StandIn::start_for(&ca, "localhost", Behaviour::default()).await;
    let resolver = Arc::new(MockResolver::answering(IpAddr::V4(Ipv4Addr::LOCALHOST)));
    for resolve in [ProxyResolve::Proxy, ProxyResolve::Humanitl] {
        upstream(&config(proxy.addr, &file.path, resolve), &resolver)
            .forward(
                &request(Scheme::Https, HostName::Dns("localhost".to_owned()), 443),
                Bytes::new(),
                true,
            )
            .await
            .unwrap_or_else(|failure| panic!("{resolve:?}: {failure:?}"));
    }
    assert_eq!(
        proxy.seen().lines,
        [
            "CONNECT localhost:443 HTTP/1.1",
            "CONNECT 127.0.0.1:443 HTTP/1.1"
        ]
    );
}

/// Ein gestreamter Upload über den Proxy: Der Kopf der Antwort kommt nach
/// `limits.header_timeout_secs`, aber vor `egress.proxy_response_timeout_secs`
/// und erreicht den Client (HUM-511, Punkt 5).
#[tokio::test(flavor = "multi_thread")]
async fn a_streamed_upload_waits_for_the_head_as_long_as_the_proxy_allows() {
    const BIG: usize = 512 * 1024;
    let ca = ForeignCa::new();
    let file = ca_file(&ca);
    let stand_in = StandIn::start(
        &ca,
        Behaviour {
            head_delay: Duration::from_secs(3),
            ..Behaviour::default()
        },
    )
    .await;
    let mut config = config(stand_in.addr, &file.path, ProxyResolve::Proxy);
    config.egress.proxy_response_timeout_secs = 8;
    let egress = from_config(&config, &[], None).unwrap().egress;
    let rules = "version: 1\n\
         rules:\n\
         \x20 - action: allow\n\
         \x20   match:\n\
         \x20     host: upstream.test\n\
         \x20     scheme: http\n\
         \x20     method: [POST]\n\
         \x20     path_prefixes: [\"/sink\"]\n\
         \x20   stream: true\n";
    let proxy = support::ProxyBuilder::new()
        .body_cap(64 * 1024)
        .header_timeout(Duration::from_secs(1))
        .rules(rules)
        .allow_private(false)
        .egress(egress)
        .start()
        .await;
    let mut client = proxy.client().await;
    let response = client
        .send(support::post("http://upstream.test/sink", vec![b's'; BIG]))
        .await;
    assert_eq!(response.status(), hyper::StatusCode::OK);
    assert_eq!(support::body_string(response.into_body()).await, "ok");
    assert_eq!(
        stand_in.seen().body_bytes,
        [BIG],
        "the whole upload arrived"
    );
}

/// Ein lokaler Name vor dem Entscheidungs-Proxy: `502`, und am Fluss steht
/// `PROXY_016` mit einer Regel, die genau dieses Ziel öffnet (ADR-0022).
#[tokio::test(flavor = "multi_thread")]
async fn a_local_name_is_reported_at_the_flow_with_a_rule() {
    let ca = ForeignCa::new();
    let file = ca_file(&ca);
    let stand_in = StandIn::start(&ca, Behaviour::default()).await;
    let config = config(stand_in.addr, &file.path, ProxyResolve::Humanitl);
    let egress = from_config(&config, &[], None).unwrap().egress;
    let proxy = support::ProxyBuilder::new()
        .passthrough()
        .allow_private(false)
        .egress(egress)
        .start()
        .await;
    let mut events = proxy.events();
    let mut client = proxy.client().await;
    let response = client
        .send(support::get("http://metadata/computeMetadata"))
        .await;
    assert_eq!(response.status(), hyper::StatusCode::BAD_GATEWAY);
    let diagnostic = match events.wait_for("diagnostic").await {
        humanitl_core::FlowEvent::Diagnostic { diagnostic, .. } => *diagnostic,
        other => panic!("expected a Diagnostic event, got {other:?}"),
    };
    assert_eq!(diagnostic.code, PROXY_016);
    assert!(
        matches!(diagnostic.fix, Some(humanitl_core::FixAction::AddRule(_))),
        "{diagnostic:?}"
    );
    assert!(stand_in.seen().lines.is_empty(), "the proxy saw nothing");
    assert_eq!(proxy.resolver.calls(), 0, "nothing was resolved");
}

/// Derselbe gestreamte Upload, aber der Kopf kommt erst nach
/// `egress.proxy_response_timeout_secs`: `502` und genau ein Befund,
/// `PROXY_019`. `LIMIT_004` mit seinem Rat zu `limits.header_timeout_secs`
/// wäre über den Proxy der falsche Weg (HUM-511).
#[tokio::test(flavor = "multi_thread")]
async fn a_streamed_upload_past_the_proxy_response_timeout_is_only_proxy_019() {
    const BIG: usize = 512 * 1024;
    let ca = ForeignCa::new();
    let file = ca_file(&ca);
    let stand_in = StandIn::start(
        &ca,
        Behaviour {
            head_delay: Duration::from_secs(5),
            ..Behaviour::default()
        },
    )
    .await;
    let mut config = config(stand_in.addr, &file.path, ProxyResolve::Proxy);
    config.egress.proxy_response_timeout_secs = 2;
    let egress = from_config(&config, &[], None).unwrap().egress;
    let rules = "version: 1\n\
         rules:\n\
         \x20 - action: allow\n\
         \x20   match:\n\
         \x20     host: upstream.test\n\
         \x20     scheme: http\n\
         \x20     method: [POST]\n\
         \x20     path_prefixes: [\"/sink\"]\n\
         \x20   stream: true\n";
    let proxy = support::ProxyBuilder::new()
        .body_cap(64 * 1024)
        .header_timeout(Duration::from_secs(1))
        .rules(rules)
        .allow_private(false)
        .egress(egress)
        .start()
        .await;
    let mut events = proxy.events();
    let mut client = proxy.client().await;
    let response = client
        .send(support::post("http://upstream.test/sink", vec![b's'; BIG]))
        .await;
    assert_eq!(response.status(), hyper::StatusCode::BAD_GATEWAY);
    events.wait_for("recorded").await;
    let codes: Vec<String> = events
        .seen
        .iter()
        .filter_map(|event| match event {
            humanitl_core::FlowEvent::Diagnostic { diagnostic, .. } => {
                Some(diagnostic.code.as_str().to_owned())
            }
            _ => None,
        })
        .filter(|code| code != "LIMIT_001")
        .collect();
    assert_eq!(codes, [PROXY_019.as_str()]);
}

/// Eine Gegenstelle, die den Kopf jeder Anfrage liest und dann `reply`
/// schreibt (leer: nichts) und schließt. Steht als Ziel oder als
/// Entscheidungs-Proxy für `http://` (HUM-555).
async fn answering_with(reply: &'static [u8]) -> std::net::SocketAddr {
    use tokio::io::AsyncWriteExt as _;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut tcp, _)) = listener.accept().await {
            tokio::spawn(async move {
                if support::stand_in::read_head(&mut tcp).await.is_some() {
                    let _ = tcp.write_all(reply).await;
                }
                let _ = tcp.shutdown().await;
            });
        }
    });
    addr
}

/// Schließt der Entscheidungs-Proxy nach der Anfrage ohne Antwort, heißt das
/// `ProxyClosed`, nicht `Connect` (HUM-555).
#[tokio::test(flavor = "multi_thread")]
async fn a_proxy_that_closes_without_an_answer_is_proxy_closed() {
    let file = ca_file(&ForeignCa::new());
    let proxy = answering_with(b"").await;
    let resolver = Arc::new(MockResolver::answering(PUBLIC));
    let failure = upstream(&config(proxy, &file.path, ProxyResolve::Proxy), &resolver)
        .forward(&named(Scheme::Http, 80), Bytes::new(), false)
        .await
        .unwrap_err();
    assert_eq!(failure.error, UpstreamError::ProxyClosed);
}

/// Schickt der Proxy etwas Unlesbares, hat er nicht geschlossen: Das bleibt
/// `Connect` wie vor HUM-555. Fängt `Failed::sending` jeden Fehler außer
/// `is_user` als Schließen, wird der Test rot.
#[tokio::test(flavor = "multi_thread")]
async fn a_proxy_that_answers_garbage_did_not_close() {
    let file = ca_file(&ForeignCa::new());
    let proxy = answering_with(b"not http at all\r\n\r\n").await;
    let resolver = Arc::new(MockResolver::answering(PUBLIC));
    let failure = upstream(&config(proxy, &file.path, ProxyResolve::Proxy), &resolver)
        .forward(&named(Scheme::Http, 80), Bytes::new(), false)
        .await
        .unwrap_err();
    assert_eq!(failure.error, UpstreamError::Connect);
}

/// Direkt, ohne Entscheidungs-Proxy, bleibt ein Ziel, das ohne Antwort
/// schließt, `Connect` (HUM-555): `ProxyClosed` gibt es nur über den Proxy.
/// Wird `(Some(_), Failed::Closed)` in `send_via` zu `(_, Failed::Closed)`,
/// wird der Test rot.
#[tokio::test(flavor = "multi_thread")]
async fn a_direct_target_that_closes_without_an_answer_stays_connect() {
    let target = answering_with(b"").await;
    let resolver = Arc::new(MockResolver::answering(IpAddr::V4(Ipv4Addr::LOCALHOST)));
    let mut config = Config::default();
    config.tls.trust_system_store = false;
    let failure = upstream(&config, &resolver)
        .forward(&named(Scheme::Http, target.port()), Bytes::new(), true)
        .await
        .unwrap_err();
    assert_eq!(failure.error, UpstreamError::Connect);
}
