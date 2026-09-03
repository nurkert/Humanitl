//! Der erzeugte Vertrag existiert, laesst sich kodieren und wieder lesen.
//!
//! Diese Tests halten nicht die Serialisierung von prost fest, sondern die
//! Zusagen aus HUM-003: jedes Enum hat eine Null mit `_UNSPECIFIED`, Header
//! duerfen beliebige Bytes tragen, und Ereignisse ueberleben den Weg durch
//! die Leitung unveraendert.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use humanitl_ipc::v1;
use prost::Message;

/// Kodiert und dekodiert eine Nachricht und vergleicht sie mit dem Original.
fn roundtrip<M: Message + Default + PartialEq + std::fmt::Debug>(message: &M) -> M {
    let bytes = message.encode_to_vec();
    let decoded = M::decode(bytes.as_slice()).expect("decoding what we just encoded must work");
    assert_eq!(&decoded, message, "roundtrip changed the message");
    decoded
}

#[test]
fn flow_event_roundtrip() {
    let event = v1::FlowEvent {
        at: Some(prost_types::Timestamp {
            seconds: 1_767_225_600,
            nanos: 42,
        }),
        event: Some(v1::flow_event::Event::Held(v1::flow_event::Held {
            flow_id: "018f0000-0000-7000-8000-000000000001".to_owned(),
            deadline: Some(prost_types::Timestamp {
                seconds: 1_767_225_900,
                nanos: 0,
            }),
            queue_bytes: 4096,
            queue_count: 3,
        })),
    };

    let decoded = roundtrip(&event);
    let Some(v1::flow_event::Event::Held(held)) = decoded.event else {
        panic!("expected Held");
    };
    assert_eq!(held.queue_count, 3);
    assert_eq!(held.queue_bytes, 4096);
}

#[test]
fn flow_event_received_carries_domain_info() {
    let event = v1::FlowEvent {
        at: None,
        event: Some(v1::flow_event::Event::Received(v1::flow_event::Received {
            summary: Some(v1::FlowSummary {
                flow_id: "018f0000-0000-7000-8000-000000000002".to_owned(),
                method: v1::Method::Post as i32,
                scheme: v1::Scheme::Https as i32,
                authority: Some(v1::Authority {
                    host: "api.github.com".to_owned(),
                    port: 443,
                    is_ip_literal: false,
                    display_host: "api.github.com".to_owned(),
                }),
                state: v1::FlowState::Received as i32,
                ..Default::default()
            }),
            domain: Some(v1::DomainInfo {
                apex: "github.com".to_owned(),
                catalog_id: "github".to_owned(),
                tranco_rank: 64,
                first_seen: None,
                seen_count: 12,
            }),
        })),
    };

    let decoded = roundtrip(&event);
    let Some(v1::flow_event::Event::Received(received)) = decoded.event else {
        panic!("expected Received");
    };
    assert_eq!(
        received.domain.map(|d| d.apex).as_deref(),
        Some("github.com")
    );
}

#[test]
fn flow_event_failed_roundtrip() {
    // Ein Upstream-Fehler ist ein eigenes Ereignis, nie ein Responded{502}
    // (CONVENTIONS.md 3.2, 4.10). Die private Adresse reist als Text mit,
    // damit `UpstreamError::PrivateAddress(IpAddr)` die Abbildung uebersteht.
    let event = v1::FlowEvent {
        at: None,
        event: Some(v1::flow_event::Event::Failed(v1::flow_event::Failed {
            flow_id: "018f0000-0000-7000-8000-000000000005".to_owned(),
            error: v1::UpstreamError::PrivateAddress as i32,
            resolved_ip: "10.0.0.7".to_owned(),
        })),
    };

    let decoded = roundtrip(&event);
    let Some(v1::flow_event::Event::Failed(failed)) = decoded.event else {
        panic!("expected Failed");
    };
    assert_eq!(failed.error(), v1::UpstreamError::PrivateAddress);
    assert_eq!(failed.resolved_ip, "10.0.0.7");
}

#[test]
fn header_value_survives_non_utf8_bytes() {
    // HTTP-Header sind nicht garantiert UTF-8. Deshalb `bytes`, nicht `string`:
    // mit `string` wuerde das Dekodieren hier scheitern.
    let request = v1::HttpRequest {
        method: v1::Method::Other as i32,
        method_raw: "PROPFIND".to_owned(),
        scheme: v1::Scheme::Https as i32,
        authority: Some(v1::Authority {
            host: "xn--bcher-kva.example".to_owned(),
            port: 443,
            is_ip_literal: false,
            display_host: "bücher.example".to_owned(),
        }),
        path_and_query: "/dav?depth=1".to_owned(),
        headers: vec![v1::Header {
            name: "x-legacy".to_owned(),
            value: vec![0xff, 0xfe, 0x00, 0x41],
        }],
        body: None,
        version: "HTTP/1.1".to_owned(),
    };

    let decoded = roundtrip(&request);
    assert_eq!(decoded.headers[0].value, vec![0xff, 0xfe, 0x00, 0x41]);
    assert_eq!(decoded.method_raw, "PROPFIND");
}

#[test]
fn decide_request_carries_note_and_remembered_rule() {
    let request = v1::DecideRequest {
        flow_ids: vec!["018f0000-0000-7000-8000-000000000003".to_owned()],
        decision: Some(v1::decide_request::Decision::Block(
            v1::decide_request::Block {
                note: "Nutze PyPI statt GitHub".to_owned(),
            },
        )),
        remember: Some(v1::Rule {
            action: v1::RuleAction::Block as i32,
            matcher: Some(v1::RuleMatcher {
                host: "**.github.com".to_owned(),
                methods: vec![v1::Method::Get as i32],
                ..Default::default()
            }),
            expires: Some(v1::RuleExpiry {
                expiry: Some(v1::rule_expiry::Expiry::Session(())),
            }),
            ..Default::default()
        }),
        acknowledge_findings: true,
    };

    let decoded = roundtrip(&request);
    let Some(v1::decide_request::Decision::Block(block)) = decoded.decision else {
        panic!("expected Block");
    };
    assert_eq!(block.note, "Nutze PyPI statt GitHub");
    assert!(decoded.remember.is_some());
}

#[test]
fn decide_request_allow_edited_carries_the_full_body() {
    // Die bearbeitete Anfrage ist die eine Stelle, an der ein Body als Inhalt
    // zum Daemon reist (CONVENTIONS.md 4.11); er darf beliebige Bytes tragen.
    let request = v1::DecideRequest {
        flow_ids: vec!["018f0000-0000-7000-8000-000000000006".to_owned()],
        decision: Some(v1::decide_request::Decision::AllowEdited(
            v1::EditedRequest {
                method: v1::Method::Post as i32,
                method_raw: String::new(),
                url: "https://api.github.com/repos".to_owned(),
                headers: vec![v1::Header {
                    name: "content-type".to_owned(),
                    value: b"application/octet-stream".to_vec(),
                }],
                body: vec![0x00, 0xff, 0x7b, 0x22],
            },
        )),
        ..Default::default()
    };

    let decoded = roundtrip(&request);
    let Some(v1::decide_request::Decision::AllowEdited(edited)) = decoded.decision else {
        panic!("expected AllowEdited");
    };
    assert_eq!(edited.body, vec![0x00, 0xff, 0x7b, 0x22]);
    assert_eq!(edited.url, "https://api.github.com/repos");
    assert_eq!(edited.method(), v1::Method::Post);
}

#[test]
fn flow_detail_carries_a_body_preview_as_text() {
    // Die Vorschau ist Text, nie Bytes; sie gehoert zum Detail, nicht zu
    // einem Ereignis (der strukturelle Beweis steht in `proto_contract.rs`).
    let detail = v1::FlowDetail {
        body_preview: "{\"a\":1}".to_owned(),
        ..Default::default()
    };
    assert_eq!(roundtrip(&detail).body_preview, "{\"a\":1}");
}

#[test]
fn response_chunk_carries_progress_only() {
    // Bodies reisen nie in Ereignissen. `ResponseChunk` hat genau zwei Felder;
    // der strukturelle Beweis steht in `proto_contract.rs`.
    let chunk = v1::flow_event::ResponseChunk {
        flow_id: "018f0000-0000-7000-8000-000000000004".to_owned(),
        bytes_so_far: 1 << 20,
    };
    assert_eq!(roundtrip(&chunk).bytes_so_far, 1 << 20);
}

/// Jedes Enum des Vertrags: Null ist `_UNSPECIFIED`, und `try_from(0)` liefert
/// genau diese Variante zurueck.
#[test]
fn enums_have_unspecified_zero() {
    macro_rules! check_enums {
        ($($enum:ty => $name:literal),+ $(,)?) => {
            $({
                let zero = <$enum>::try_from(0);
                assert_eq!(
                    zero,
                    Ok(<$enum>::Unspecified),
                    "{}: value 0 is not Unspecified",
                    $name
                );
                assert_eq!(
                    <$enum>::Unspecified.as_str_name(),
                    concat!($name, "_UNSPECIFIED"),
                    "{}: zero value has the wrong proto name",
                    $name
                );
                assert_eq!(<$enum>::default(), <$enum>::Unspecified, "{}", $name);
            })+
        };
    }

    check_enums! {
        v1::Method => "METHOD",
        v1::Scheme => "SCHEME",
        v1::Upgrade => "UPGRADE",
        v1::FlowState => "FLOW_STATE",
        v1::DecisionKind => "DECISION_KIND",
        v1::BlockReason => "BLOCK_REASON",
        v1::DecisionSource => "DECISION_SOURCE",
        v1::UpstreamError => "UPSTREAM_ERROR",
        v1::FindingTier => "FINDING_TIER",
        v1::FindingLocation => "FINDING_LOCATION",
        v1::Severity => "SEVERITY",
        v1::RuleAction => "RULE_ACTION",
        v1::SandboxState => "SANDBOX_STATE",
        v1::IsolationCheck => "ISOLATION_CHECK",
        v1::CheckStatus => "CHECK_STATUS",
        v1::LlmProduct => "LLM_PRODUCT",
    }
}

#[test]
fn unknown_enum_value_is_rejected_not_guessed() {
    // Ein neuerer Daemon darf Werte schicken, die dieser Client nicht kennt.
    // prost liefert dann einen Fehler statt stillschweigend etwas anderes.
    assert!(v1::BlockReason::try_from(9_999).is_err());
    assert_eq!(
        v1::BlockReason::try_from(10),
        Ok(v1::BlockReason::PrivateAddress)
    );
}

#[test]
fn client_and_server_stubs_exist() {
    // Beide Richtungen werden erzeugt: der Daemon implementiert das Trait,
    // UI und CLI benutzen den Client.
    fn assert_send<T: Send>() {}
    assert_send::<v1::humanitl_client::HumanitlClient<tonic::transport::Channel>>();
    assert_eq!(v1::humanitl_server::SERVICE_NAME, "humanitl.v1.Humanitl");
}

#[test]
fn contract_version_constants_are_v1() {
    assert_eq!(humanitl_ipc::PROTO_MAJOR, 1);
    assert_eq!(humanitl_ipc::TOKEN_METADATA_KEY, "x-humanitl-token");
}
