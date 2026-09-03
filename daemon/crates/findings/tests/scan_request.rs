//! Der Scan über eine ganze Anfrage: Orte, Bereiche, Deduplikation, Grenzen.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use core::time::Duration;
use std::io::Write as _;
use std::time::Instant;

use flate2::Compression;
use flate2::write::GzEncoder;

use humanitl_core::http::{HeaderMap, HeaderName, HeaderValue};
use humanitl_core::{
    Authority, Finding, FindingKind, FindingLocation, HostName, HttpRequest, Method, Scheme, Tier,
};
use humanitl_findings::{DetectorRegistry, FindingsSettings};

fn request(path_and_query: &str, headers: &[(&'static str, &str)]) -> HttpRequest {
    let mut map = HeaderMap::new();
    for (name, value) in headers {
        map.insert(
            HeaderName::from_static(name),
            HeaderValue::from_str(value).unwrap(),
        );
    }
    HttpRequest::new(
        Method::POST,
        Scheme::Https,
        Authority::with_scheme(HostName::Dns("api.example.com".to_owned()), Scheme::Https),
        path_and_query,
    )
    .with_headers(map)
}

fn registry(settings: &FindingsSettings) -> DetectorRegistry {
    DetectorRegistry::tier1(settings).unwrap()
}

fn scan(settings: &FindingsSettings, request: &HttpRequest, body: &[u8]) -> Vec<Finding> {
    registry(settings).scan_request(request, body)
}

#[test]
fn the_registry_holds_the_seven_tier1_detectors() {
    let ids = registry(&FindingsSettings::default()).detector_ids();
    assert_eq!(ids, humanitl_findings::TIER1_DETECTOR_IDS.to_vec());
}

#[test]
fn disabled_findings_mean_no_detector_at_all() {
    let settings = FindingsSettings {
        enabled: false,
        ..FindingsSettings::default()
    };
    assert!(registry(&settings).detector_ids().is_empty());
    assert!(scan(&settings, &request("/v1", &[]), b"AKIAIOSFODNN7EXAMPLE").is_empty());
}

#[test]
fn bearer_header_only() {
    let settings = FindingsSettings::default();
    let value = "Bearer abcdefghijklmnopqrstuvwxyz0123";
    let with_header = request("/v1", &[("authorization", value)]);
    let findings = scan(&settings, &with_header, b"");
    assert_eq!(findings.len(), 1);
    assert_eq!(
        findings[0].location,
        FindingLocation::Header(HeaderName::from_static("authorization"))
    );
    assert_eq!(findings[0].kind, FindingKind::ApiKey("bearer".to_owned()));
    // Der Bereich ist nur der Token, nicht das Wort "Bearer".
    assert_eq!(findings[0].span, 7..value.len());
    assert_eq!(findings[0].display_prefix, "abcdef…");

    // Derselbe Text im Body ist kein bearer-Fund.
    let plain = request("/v1", &[]);
    assert!(scan(&settings, &plain, value.as_bytes()).is_empty());
}

#[test]
fn email_allow_domain() {
    let request = request("/v1", &[]);
    let allowed = FindingsSettings::default().with_email_allow_domains(["example.com".to_owned()]);
    assert!(scan(&allowed, &request, b"x@example.com").is_empty());
    assert_eq!(
        scan(&FindingsSettings::default(), &request, b"x@example.com").len(),
        1
    );
}

#[test]
fn query_decoded_span() {
    let raw = "q=user%40example.com";
    let request = request(&format!("/v1?{raw}"), &[]);
    let findings = scan(&FindingsSettings::default(), &request, b"");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].location, FindingLocation::Query);
    assert_eq!(findings[0].kind, FindingKind::Email);
    // Der Bereich zeigt auf den Rohtext, nicht auf die dekodierte Kopie.
    assert_eq!(findings[0].span, 2..raw.len());
    assert_eq!(&raw[findings[0].span.clone()], "user%40example.com");
    assert_eq!(findings[0].display_prefix, "u***@example.com");
}

#[test]
fn strings_mode_binary() {
    // Pseudozufall mit einem eingebetteten Schlüssel. Der Generator ist fest
    // geimpft, damit der Fall reproduzierbar bleibt.
    //
    // Die Spezifikation nennt 1 MB. Der Test nimmt 256 KiB, weil der
    // regex-Crate ohne Optimierung um Größenordnungen langsamer läuft als mit:
    // 1 MiB kostet im Debug-Build 5 Sekunden und wäre damit der langsamste
    // Test des Arbeitsbaums. Für den Beweis, dass der strings-Modus die
    // Zeichenkette im Binärrauschen findet, reicht die kleinere Menge; die
    // Zahl für den Durchsatz liefert `eight_mebibyte_json_stays_fast` im
    // Release-Build.
    let mut body = pseudo_random(256 * 1024, 0x5EED_1234_ABCD_0001);
    let secret = b"AKIAIOSFODNN7EXAMPLE";
    let offset = 200_000;
    // Wie in einer echten Binärdatei steht die Zeichenkette zwischen Nullbytes;
    // ohne diesen Rahmen fängt ein zufälliger Buchstabe daneben die
    // Wortgrenze ab, die das Muster verlangt.
    body[offset - 1] = 0;
    body[offset..offset + secret.len()].copy_from_slice(secret);
    body[offset + secret.len()] = 0;
    let request = request("/v1", &[("content-type", "application/octet-stream")]);

    let findings = scan(&FindingsSettings::default(), &request, &body);
    let found = findings
        .iter()
        .find(|finding| finding.kind == FindingKind::ApiKey("aws".to_owned()))
        .expect("der eingebettete Schlüssel fehlt");
    assert_eq!(found.span, offset..offset + secret.len());
    assert_eq!(found.location, FindingLocation::Body);
}

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

#[test]
fn a_gzip_body_is_decoded_and_scanned() {
    let plain = br#"{"note":"aws_access_key_id = AKIAIOSFODNN7EXAMPLE"}"#;
    let request = request(
        "/v1",
        &[
            ("content-type", "application/json"),
            ("content-encoding", "gzip"),
        ],
    );
    let report = registry(&FindingsSettings::default()).scan(&request, &gzip(plain));
    assert!(!report.truncated);
    assert!(report.diagnostics.is_empty());
    assert_eq!(report.findings.len(), 1);

    let found = &report.findings[0];
    assert_eq!(found.kind, FindingKind::ApiKey("aws".to_owned()));
    assert_eq!(found.location, FindingLocation::Body);
    // Der Bereich zeigt in den entpackten Body, nicht in den gepackten.
    assert_eq!(&plain[found.span.clone()], b"AKIAIOSFODNN7EXAMPLE");
}

#[test]
fn a_gzip_bomb_is_scanned_only_as_far_as_the_budget_reaches() {
    let bomb = gzip(&vec![b'x'; 1024 * 1024]);
    let settings = FindingsSettings::default().with_limits(8 * 1024 * 1024, 100);
    let request = request(
        "/v1",
        &[("content-type", "text/plain"), ("content-encoding", "gzip")],
    );
    let report = registry(&settings).scan(&request, &bomb);
    assert!(report.truncated);
    assert_eq!(report.diagnostics.len(), 1);
    assert_eq!(report.diagnostics[0].code.as_str(), "FINDINGS_002");
}

#[test]
fn a_broken_compressed_body_is_reported_instead_of_scanned_blindly() {
    // Ein kaputter Strom wird nicht durchsucht; der Scan sagt das, statt in
    // gepackten Bytes Muster zu suchen (siehe FINDINGS_002).
    let request = request(
        "/v1",
        &[
            ("content-type", "application/json"),
            ("content-encoding", "gzip"),
        ],
    );
    let report = registry(&FindingsSettings::default()).scan(&request, b"\x1f\x8b\x08 gepackt");
    assert!(report.findings.is_empty());
    assert!(report.truncated);
    assert_eq!(report.diagnostics.len(), 1);
    assert_eq!(report.diagnostics[0].code.as_str(), "FINDINGS_002");
}

#[test]
fn a_body_over_the_cap_is_cut_and_the_report_says_so() {
    let settings = FindingsSettings::default().with_limits(64, 100);
    let mut body = vec![b'.'; 64];
    body.extend_from_slice(b"AKIAIOSFODNN7EXAMPLE");
    let request = request("/v1", &[("content-type", "text/plain")]);
    let report = registry(&settings).scan(&request, &body);
    assert!(
        report.findings.is_empty(),
        "hinter dem Cap wird nicht gesucht"
    );
    assert!(report.truncated);
    assert_eq!(report.diagnostics[0].code.as_str(), "FINDINGS_002");
}

#[test]
fn ignored_hash_skipped() {
    let request = request("/v1", &[]);
    let body = b"aws_access_key_id = AKIAIOSFODNN7EXAMPLE";
    let findings = scan(&FindingsSettings::default(), &request, body);
    assert_eq!(findings.len(), 1);
    let hash = findings[0].value_hash_hex();

    let quiet = FindingsSettings::default()
        .with_ignored_hashes_hex([hash.clone()])
        .unwrap();
    assert!(scan(&quiet, &request, body).is_empty());

    // Derselbe Wert an einer anderen Stelle hat denselben Hash und bleibt
    // ebenfalls still; genau das macht "immer ignorieren" brauchbar.
    let elsewhere = request_with_query(&format!("key={}", "AKIAIOSFODNN7EXAMPLE"));
    assert!(scan(&quiet, &elsewhere, b"").is_empty());
    assert_eq!(scan(&FindingsSettings::default(), &elsewhere, b"").len(), 1);
    assert_eq!(hash.len(), 64);
}

fn request_with_query(query: &str) -> HttpRequest {
    request(&format!("/v1?{query}"), &[])
}

#[test]
fn dedupe_keeps_highest_tier() {
    let settings = FindingsSettings::default().with_user_terms(["kunde@example.org".to_owned()]);
    let request = request("/v1", &[]);
    let findings = scan(&settings, &request, b"mail an kunde@example.org bitte");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].tier, Tier::UserTerm);
    assert_eq!(
        findings[0].kind,
        FindingKind::UserTerm("kunde@example.org".to_owned())
    );
}

#[test]
fn spans_stay_inside_the_scanned_bytes() {
    // Eigenschaftstest ohne Fremd-Crate: 200 zufällige Eingaben aus Text- und
    // Binärbytes, jeweils gegen alle Detektoren.
    let settings = FindingsSettings::default().with_user_terms(["Acme".to_owned(), "ü".to_owned()]);
    let registry = registry(&settings);
    let alphabet: &[u8] =
        b"0123456789 .-+@/ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz%=:\xc3\xbc\x00\xff";
    for seed in 0..200u64 {
        let noise = pseudo_random(512, seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let body: Vec<u8> = noise
            .iter()
            .map(|byte| alphabet[*byte as usize % alphabet.len()])
            .collect();
        let query = String::from_utf8_lossy(&body[..64]).replace(['?', '#'], "_");
        let header = String::from_utf8_lossy(&body[64..96]).replace(['\r', '\n', '\0'], "_");
        let Ok(value) = HeaderValue::from_str(&header) else {
            continue;
        };
        let mut headers = HeaderMap::new();
        headers.insert(HeaderName::from_static("x-test"), value.clone());
        let request = HttpRequest::new(
            Method::POST,
            Scheme::Https,
            Authority::with_scheme(HostName::Dns("api.example.com".to_owned()), Scheme::Https),
            format!("/v1?{query}"),
        )
        .with_headers(headers);

        for finding in registry.scan_request(&request, &body) {
            let limit = match &finding.location {
                FindingLocation::Header(_) => value.as_bytes().len(),
                FindingLocation::Query => query.len(),
                FindingLocation::Body => body.len(),
            };
            assert!(
                finding.span.start <= finding.span.end && finding.span.end <= limit,
                "Bereich {:?} liegt außerhalb von {limit} ({})",
                finding.span,
                finding.location
            );
        }
    }
}

#[test]
#[ignore = "Messung, kein Gate: läuft mit `cargo test -p humanitl-findings -- --ignored`"]
fn eight_mebibyte_json_stays_fast() {
    let unit = br#"{"id":123,"note":"Bitte fasse das Protokoll zusammen.","tags":["a","b"]},"#;
    let mut body = Vec::with_capacity(8 * 1024 * 1024 + unit.len());
    while body.len() < 8 * 1024 * 1024 {
        body.extend_from_slice(unit);
    }
    let request = request("/v1", &[("content-type", "application/json")]);
    let registry = registry(&FindingsSettings::default().with_user_terms(["Acme".to_owned()]));

    let started = Instant::now();
    let findings = registry.scan_request(&request, &body);
    let elapsed = started.elapsed();
    println!(
        "8 MiB JSON: {} ms, {} Funde",
        elapsed.as_millis(),
        findings.len()
    );
    assert!(
        elapsed < Duration::from_secs(2),
        "8 MiB brauchten {elapsed:?}"
    );
}

/// Ein fest geimpfter Generator (xorshift64*), damit die Tests reproduzierbar sind.
fn pseudo_random(len: usize, seed: u64) -> Vec<u8> {
    let mut state = seed | 1;
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        out.extend_from_slice(&state.to_le_bytes());
    }
    out.truncate(len);
    out
}
