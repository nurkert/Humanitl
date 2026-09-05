//! Strukturelle Zusagen des Vertrags, geprueft am eingecheckten
//! `proto/descriptor.binpb`.
//!
//! `buf lint` gibt es nur in CI. Diese Tabelle prueft lokal die Regeln, an
//! denen HUM-003 haengt: `_UNSPECIFIED = 0` ueberall, Enum-Werte mit
//! Typ-Praefix, `snake_case`-Felder, keine Bodies in Ereignissen, und dass
//! jede Nachricht und jeder RPC aus BACKLOG.md 3.3 samt der Erweiterungen aus
//! CONVENTIONS.md 4.3 wirklich existiert, und dass `FlowEvent.event` und
//! `DecideRequest.decision` genau die vereinbarten Varianten tragen.
//!
//! Der Descriptor ist zugleich der Drift-Wachhund: er entsteht ueber
//! `cargo xtask proto` (`scripts/gen-proto.sh`) und muss zu den
//! `.proto`-Dateien passen. Dass er es tut, prueft
//! `checked_in_descriptor_matches_the_proto_sources`: derselbe Codepfad wie
//! `xtask` uebersetzt die Quellen hier noch einmal und vergleicht Byte fuer
//! Byte. Ohne diesen Test wuerden alle anderen Tests einen veralteten
//! Descriptor pruefen und nicht den Vertrag.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use prost::Message as _;
use prost_types::{
    DescriptorProto, EnumDescriptorProto, FieldDescriptorProto, FileDescriptorProto,
    FileDescriptorSet, field_descriptor_proto::Type,
};

/// Der eingecheckte Descriptor-Set des gesamten Vertrags.
const DESCRIPTOR_SET: &[u8] = include_bytes!("../../../../proto/descriptor.binpb");

/// Das Paket, dessen Dateien geprueft werden. Importe bleiben aussen vor.
const PACKAGE: &str = "humanitl.v1";

/// Alle Dateien des Pakets `humanitl.v1`.
fn files() -> Vec<FileDescriptorProto> {
    let set = FileDescriptorSet::decode(DESCRIPTOR_SET)
        .expect("proto/descriptor.binpb is not a FileDescriptorSet; run scripts/gen-proto.sh");
    let files: Vec<_> = set
        .file
        .into_iter()
        .filter(|f| f.package() == PACKAGE)
        .collect();
    assert_eq!(
        files.len(),
        3,
        "expected common.proto, rules.proto and humanitl.proto"
    );
    files
}

/// Die gemeinsame Uebersetzung aus `proto_gen.rs`, wortgleich mit `build.rs`
/// und `cargo xtask proto`. Nur so vergleicht der Test genau das, was `xtask`
/// schreibt, und nicht eine zweite Uebersetzung mit anderen Einstellungen.
mod proto_gen {
    include!("../proto_gen.rs");
}

#[test]
fn checked_in_descriptor_matches_the_proto_sources() {
    // `proto/descriptor.binpb` ist eingecheckt und Eingabe aller anderen
    // Tests dieser Datei. Wer die `.proto` aendert und den Descriptor nicht,
    // liesse sie sonst gegen den alten Vertrag laufen. CI hat dafuer den
    // Drift-Schritt (`git diff --exit-code` nach `scripts/gen-proto.sh`);
    // dieser Test ist dasselbe Urteil in `cargo test`.
    use protox::prost::Message as _;

    let proto_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../proto");
    let fresh = proto_gen::compile_protos(&proto_dir, false)
        .expect("proto/humanitl/v1/*.proto compile")
        .encode_to_vec();
    assert!(
        fresh == DESCRIPTOR_SET,
        "proto/descriptor.binpb is stale: it does not match proto/humanitl/v1/*.proto \
         (checked in {} bytes, fresh {} bytes); run `cargo xtask proto` in daemon/ and \
         commit the result",
        DESCRIPTOR_SET.len(),
        fresh.len()
    );
}

/// Sammelt alle Nachrichten, auch geschachtelte, als `(voller Name, Message)`.
fn messages() -> Vec<(String, DescriptorProto)> {
    fn walk(prefix: &str, message: &DescriptorProto, out: &mut Vec<(String, DescriptorProto)>) {
        let name = format!("{prefix}.{}", message.name());
        for nested in &message.nested_type {
            walk(&name, nested, out);
        }
        out.push((name, message.clone()));
    }

    let mut out = Vec::new();
    for file in files() {
        for message in &file.message_type {
            walk(PACKAGE, message, &mut out);
        }
    }
    out
}

/// Sammelt alle Enums, auch geschachtelte, als `(voller Name, Enum)`.
fn enums() -> Vec<(String, EnumDescriptorProto)> {
    fn walk(prefix: &str, message: &DescriptorProto, out: &mut Vec<(String, EnumDescriptorProto)>) {
        let name = format!("{prefix}.{}", message.name());
        for value in &message.enum_type {
            out.push((format!("{name}.{}", value.name()), value.clone()));
        }
        for nested in &message.nested_type {
            walk(&name, nested, out);
        }
    }

    let mut out = Vec::new();
    for file in files() {
        for value in &file.enum_type {
            out.push((format!("{PACKAGE}.{}", value.name()), value.clone()));
        }
        for message in &file.message_type {
            walk(PACKAGE, message, &mut out);
        }
    }
    out
}

/// `FlowSummary` wird zu `FLOW_SUMMARY`; so lautet das Praefix der Enum-Werte.
fn screaming_snake(pascal: &str) -> String {
    let mut out = String::new();
    for (index, ch) in pascal.char_indices() {
        if ch.is_ascii_uppercase() && index != 0 {
            out.push('_');
        }
        out.push(ch.to_ascii_uppercase());
    }
    out
}

#[test]
fn every_enum_has_unspecified_zero() {
    for (name, value) in enums() {
        let first = value.value.first().expect("enum without values");
        assert_eq!(first.number(), 0, "{name}: first value is not 0");
        assert!(
            first.name().ends_with("_UNSPECIFIED"),
            "{name}: zero value {} does not end in _UNSPECIFIED",
            first.name()
        );
        let zeroes = value.value.iter().filter(|v| v.number() == 0).count();
        assert_eq!(zeroes, 1, "{name}: more than one zero value");
    }
}

#[test]
fn every_enum_value_carries_the_type_prefix() {
    // buf-Regel ENUM_VALUE_PREFIX, lokal nachgestellt.
    for (full_name, value) in enums() {
        let prefix = screaming_snake(value.name());
        for entry in &value.value {
            assert!(
                entry.name().starts_with(&format!("{prefix}_")),
                "{full_name}: value {} lacks the prefix {prefix}_",
                entry.name()
            );
        }
    }
}

#[test]
fn field_names_are_lower_snake_case() {
    for (name, message) in messages() {
        for field in &message.field {
            let field_name = field.name();
            assert!(
                field_name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
                "{name}.{field_name} is not lower_snake_case"
            );
        }
        for oneof in &message.oneof_decl {
            let oneof_name = oneof.name();
            assert!(
                oneof_name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
                "{name}: oneof {oneof_name} is not lower_snake_case"
            );
        }
    }
}

/// Alle Nachrichten des Pakets, die von `FlowEvent` aus ueber Felder
/// erreichbar sind, `FlowEvent` selbst eingeschlossen. Importierte Typen
/// (`google.protobuf.*`) bleiben aussen vor.
fn reachable_from_flow_event() -> std::collections::BTreeSet<String> {
    let all = messages();
    let by_name = |name: &str| all.iter().find(|(n, _)| n == name).map(|(_, m)| m);
    let package_prefix = format!("{PACKAGE}.");
    let mut queue = vec![format!("{PACKAGE}.FlowEvent")];
    let mut visited = std::collections::BTreeSet::new();
    while let Some(name) = queue.pop() {
        if !visited.insert(name.clone()) {
            continue;
        }
        let descriptor = by_name(&name).unwrap_or_else(|| panic!("unknown message {name}"));
        for field in &descriptor.field {
            if field.r#type() == Type::Message {
                let target = field.type_name().trim_start_matches('.');
                if target.starts_with(&package_prefix) {
                    queue.push(target.to_owned());
                }
            }
        }
    }
    // Der Lauf muss ueber die Ereignisse hinaus in Finding und Header
    // gelangen, sonst prueft er nichts.
    for reached in ["humanitl.v1.Finding", "humanitl.v1.Header"] {
        assert!(visited.contains(reached), "walk did not reach {reached}");
    }
    visited
}

#[test]
fn no_body_bytes_reachable_from_flow_events() {
    // Bodies reisen nur als BodyRef und ueber GetBody, nie in Ereignissen
    // (CONVENTIONS.md 3.6). Geprueft wird transitiv: jede Nachricht des
    // Pakets, die von `FlowEvent` aus ueber Felder erreichbar ist. Ein
    // `bytes`-Feld darf es dort nur geben, wenn es hier steht; wer ein neues
    // braucht, traegt es mit Begruendung ein.
    const ALLOWED_BYTES: &[&str] = &[
        // Hash des Bodys, nie der Body selbst. Erreichbar, sobald HUM-018
        // den `HttpRequest` in das Held-Ereignis legt.
        "humanitl.v1.BodyRef.sha256",
        // Hash des gefundenen Werts, nie der Wert selbst.
        "humanitl.v1.Finding.value_hash",
        // Header sind nicht garantiert UTF-8, siehe `header_values_are_bytes`.
        "humanitl.v1.Header.value",
    ];

    let all = messages();
    let by_name = |name: &str| all.iter().find(|(n, _)| n == name).map(|(_, m)| m);

    // Jeder Eintrag der Liste muss ein echtes bytes-Feld sein, sonst veraltet
    // die Liste unbemerkt.
    for allowed in ALLOWED_BYTES {
        let (message, field_name) = allowed
            .rsplit_once('.')
            .expect("allow list entries look like Message.field");
        let descriptor = by_name(message)
            .unwrap_or_else(|| panic!("allow list names unknown message {message}"));
        let field = descriptor
            .field
            .iter()
            .find(|f| f.name() == field_name)
            .unwrap_or_else(|| panic!("allow list names unknown field {allowed}"));
        assert_eq!(
            field.r#type(),
            Type::Bytes,
            "{allowed} is on the allow list but is not bytes"
        );
    }

    for name in reachable_from_flow_event() {
        let descriptor = by_name(&name).unwrap_or_else(|| panic!("unknown message {name}"));
        for field in &descriptor.field {
            let full = format!("{name}.{}", field.name());
            if field.r#type() == Type::Bytes {
                assert!(
                    ALLOWED_BYTES.contains(&full.as_str()),
                    "{full} carries raw bytes reachable from FlowEvent; events must reference bodies"
                );
            }
        }
    }

    // Der eine volle Body des Vertrags (`EditedRequest.body`) braucht keinen
    // Eintrag in der Liste: er reist in `DecideRequest`, und die ist von
    // `FlowEvent` aus nicht zu erreichen. Stuende `EditedRequest` je in einem
    // Ereignis, schluege der Lauf oben an, und dieser Satz hier zuerst.
    assert!(
        !ALLOWED_BYTES.contains(&"humanitl.v1.EditedRequest.body"),
        "EditedRequest.body must not need the allow list; it is outside the event stream"
    );
}

#[test]
fn edited_request_and_body_preview_stay_out_of_the_event_stream() {
    // `EditedRequest` traegt den vollstaendigen Body als `bytes`, weil der
    // Daemon den bearbeiteten Inhalt sonst nirgends hat; `FlowDetail` traegt
    // eine Vorschau als `string`. Beides darf nur in Aufrufen reisen, nie in
    // Ereignissen (CONVENTIONS.md 3.6, 4.11).
    let reachable = reachable_from_flow_event();
    for outside in [
        "humanitl.v1.EditedRequest",
        "humanitl.v1.FlowDetail",
        "humanitl.v1.DecideRequest",
    ] {
        assert!(
            !reachable.contains(outside),
            "{outside} is reachable from FlowEvent; it carries a body or a preview"
        );
    }
    for name in &reachable {
        let descriptor = message(name.trim_start_matches("humanitl.v1."));
        assert!(
            descriptor.field.iter().all(|f| f.name() != "body_preview"),
            "{name}.body_preview is reachable from FlowEvent; previews belong to FlowDetail only"
        );
    }

    let edited = message("EditedRequest");
    assert_eq!(
        check_fields("EditedRequest", &edited, EDITED_REQUEST_FIELDS),
        Ok(())
    );

    let detail = message("FlowDetail");
    let preview = detail
        .field
        .iter()
        .find(|f| f.name() == "body_preview")
        .expect("FlowDetail.body_preview is missing");
    assert_eq!(
        preview.r#type(),
        Type::String,
        "the preview is text, never raw bytes"
    );
    assert_eq!(preview.number(), 9);
}

#[test]
fn decide_request_reserves_the_retired_allow_edited_number() {
    // Feld 3 trug `allow_edited` als `HttpRequest` ohne Body. Die Nummer
    // bleibt gesperrt, damit ein alter Client nie eine `EditedRequest` als
    // `HttpRequest` liest oder umgekehrt (docs/PROTOCOL.md 4).
    let decide = message("DecideRequest");
    let reserved = decide
        .reserved_range
        .iter()
        .any(|range| range.start() <= 3 && 3 < range.end());
    assert!(reserved, "DecideRequest does not reserve field number 3");
    assert!(
        decide.field.iter().all(|f| f.number() != 3),
        "DecideRequest reuses the retired field number 3"
    );
}

#[test]
fn header_values_are_bytes() {
    // HTTP-Header sind nicht garantiert UTF-8; als `string` wuerde der
    // Decoder auf echten Antworten scheitern.
    let (_, header) = messages()
        .into_iter()
        .find(|(name, _)| name == "humanitl.v1.Header")
        .expect("message Header is missing");
    let value = header
        .field
        .iter()
        .find(|f| f.name() == "value")
        .expect("Header.value is missing");
    assert_eq!(value.r#type(), Type::Bytes);
}

#[test]
fn all_contract_messages_exist() {
    // BACKLOG.md 3.3 plus die Erweiterungen aus CONVENTIONS.md 4.3.
    const REQUIRED: &[&str] = &[
        "Info",
        "Header",
        "BodyRef",
        "Authority",
        "HttpRequest",
        "HttpResponseHead",
        "Finding",
        "FixAction",
        "Diagnostic",
        "DomainInfo",
        "FlowRef",
        "FlowSummary",
        "FlowDetail",
        "SubscribeRequest",
        "FlowEvent",
        "FlowEvent.Received",
        "FlowEvent.Analyzed",
        "FlowEvent.Held",
        "FlowEvent.Decided",
        "FlowEvent.ResponseHeaders",
        "FlowEvent.ResponseChunk",
        "FlowEvent.Lagged",
        "FlowEvent.RulesChanged",
        "FlowEvent.AgentAsk",
        "FlowEvent.Failed",
        "ListFlowsRequest",
        "FlowPage",
        "BodyChunk",
        "EditedRequest",
        "DecideRequest",
        "DecideRequest.Block",
        "DecideResponse",
        "DecideResult",
        "RuleMatcher",
        "RuleExpiry",
        "Rule",
        "RulesRequest",
        "RulesRequest.Reorder",
        "RulesRequest.DryRun",
        "RulesResponse",
        "CheckResult",
        "SandboxRequest",
        "SandboxRequest.Start",
        "SandboxRequest.Plan",
        "SandboxEvent",
        "SandboxEvent.Status",
        "SandboxEvent.LogLine",
        "SessionSummaryRef",
        "SessionSummary",
        "FileChange",
        "SymlinkEscape",
        "SummaryFinding",
        "Mount",
        "EnvVar",
        "TerminalInput",
        "TerminalInput.Open",
        "TerminalOutput",
        "TerminalOutput.Exit",
        "AuditRequest",
        "AuditRequest.Export",
        "AuditResponse",
        "GetConfigRequest",
        "ConfigSnapshot",
        "FieldOrigin",
        "SetConfigRequest",
        "DoctorCheck",
        "DoctorReport",
        "DiscoverRequest",
        "DiscoverResult",
    ];

    let present: Vec<String> = messages().into_iter().map(|(name, _)| name).collect();
    for required in REQUIRED {
        let full = format!("{PACKAGE}.{required}");
        assert!(present.contains(&full), "missing message {full}");
    }
}

#[test]
fn all_contract_enums_exist() {
    const REQUIRED: &[&str] = &[
        "Method",
        "Scheme",
        "Upgrade",
        "FlowState",
        "DecisionKind",
        "BlockReason",
        "DecisionSource",
        "UpstreamError",
        "FindingTier",
        "FindingLocation",
        "Severity",
        "RuleAction",
        "SandboxState",
        "IsolationCheck",
        "MountMode",
        "ValueOrigin",
        "CheckStatus",
        "LlmProduct",
        "FileChangeKind",
        "ScanSkip",
    ];

    let present: Vec<String> = enums().into_iter().map(|(name, _)| name).collect();
    for required in REQUIRED {
        let full = format!("{PACKAGE}.{required}");
        assert!(present.contains(&full), "missing enum {full}");
    }
}

#[test]
fn extension_fields_from_conventions_43_exist() {
    let all = messages();
    let field_of = |message: &str, field: &str| -> prost_types::FieldDescriptorProto {
        let (_, descriptor) = all
            .iter()
            .find(|(name, _)| name == &format!("{PACKAGE}.{message}"))
            .unwrap_or_else(|| panic!("missing message {message}"));
        descriptor
            .field
            .iter()
            .find(|f| f.name() == field)
            .unwrap_or_else(|| panic!("missing field {message}.{field}"))
            .clone()
    };

    assert_eq!(
        field_of("DecideRequest", "remember").type_name(),
        ".humanitl.v1.Rule"
    );
    assert_eq!(
        field_of("DecideResponse", "created_rule").type_name(),
        ".humanitl.v1.Rule"
    );
    assert_eq!(
        field_of("FlowEvent.Received", "domain").type_name(),
        ".humanitl.v1.DomainInfo"
    );
    assert_eq!(
        field_of("DecideRequest.Block", "note").r#type(),
        Type::String
    );
    assert_eq!(
        field_of("RulesRequest", "make_permanent").r#type(),
        Type::String
    );
    // HUM-027: `reload` liest `rules.yaml` neu ein, `diagnostics` traegt die
    // Befunde dazu, `dry_run_scanned` sagt, wie viele Flows der Probelauf
    // wirklich geprueft hat.
    assert_eq!(
        field_of("RulesRequest", "reload").type_name(),
        ".google.protobuf.Empty"
    );
    assert_eq!(
        field_of("RulesResponse", "diagnostics").type_name(),
        ".humanitl.v1.Diagnostic"
    );
    assert_eq!(
        field_of("RulesResponse", "dry_run_scanned").r#type(),
        Type::Uint32
    );
    assert_eq!(
        field_of("SandboxRequest", "argv").type_name(),
        ".google.protobuf.Empty"
    );
}

/// `FlowSummary.meta` steht auf Nummer 25 und ist ein `bool` (HUM-103).
///
/// Der Frische-Test belegt nur, dass der eingecheckte Deskriptor zur `.proto`
/// passt. Änderte jemand die Nummer und erzeugte neu, bliebe er grün — und ein
/// älterer Client läse für jeden Meta-Fluss still `false`, also „kein
/// Meta-Fluss". Eine Feldnummer ist Teil des Vertrags und wird nie recycelt
/// (`docs/PROTOCOL.md`); deshalb steht sie hier als Zusicherung.
#[test]
fn the_meta_mark_keeps_its_field_number() {
    let summary = message("FlowSummary");
    let meta = summary
        .field
        .iter()
        .find(|f| f.name() == "meta")
        .expect("FlowSummary.meta is missing");
    assert_eq!(
        meta.number(),
        25,
        "the field number is part of the contract"
    );
    assert_eq!(meta.r#type(), Type::Bool);

    // Und die Nummer gehört ihm allein.
    let holders: Vec<&str> = summary
        .field
        .iter()
        .filter(|f| f.number() == 25)
        .map(prost_types::FieldDescriptorProto::name)
        .collect();
    assert_eq!(holders, vec!["meta"]);
}

#[test]
fn failed_event_mirrors_the_core_state_machine() {
    // CONVENTIONS.md 3.2 und 4.10: `Failed` ist ein eigener Zustand mit
    // `UpstreamError`, nie ein `Responded { 502 }`. Ohne dieses Ereignis
    // koennte ein Zuhoerer den Fehlschlag nie erfahren, und die Abbildung
    // `core::FlowEvent -> v1::FlowEvent` (HUM-018) waere nicht total.
    let all = messages();
    let (_, failed) = all
        .iter()
        .find(|(name, _)| name == "humanitl.v1.FlowEvent.Failed")
        .expect("FlowEvent.Failed is missing");
    let field = |name: &str| {
        failed
            .field
            .iter()
            .find(|f| f.name() == name)
            .unwrap_or_else(|| panic!("FlowEvent.Failed.{name} is missing"))
    };
    assert_eq!(field("flow_id").r#type(), Type::String);
    assert_eq!(field("error").type_name(), ".humanitl.v1.UpstreamError");
    assert_eq!(field("resolved_ip").r#type(), Type::String);

    let (_, event) = all
        .iter()
        .find(|(name, _)| name == "humanitl.v1.FlowEvent")
        .expect("FlowEvent is missing");
    assert!(
        event
            .field
            .iter()
            .any(|f| f.name() == "failed" && f.type_name() == ".humanitl.v1.FlowEvent.Failed"),
        "FlowEvent.event lacks the `failed` variant"
    );
}

/// Die Nachricht `humanitl.v1.<short>`, auch geschachtelt (`DecideRequest.Block`).
fn message(short: &str) -> DescriptorProto {
    let full = format!("{PACKAGE}.{short}");
    let (_, descriptor) = messages()
        .into_iter()
        .find(|(name, _)| name == &full)
        .unwrap_or_else(|| panic!("missing message {full}"));
    descriptor
}

/// Der Typ eines Feldes, wie er in der `.proto` steht: bei Nachrichten und
/// Enums der volle Name (`.humanitl.v1.Rule`), bei Skalaren das
/// Schluesselwort (`string`, `bool`), mit `repeated ` davor bei Listen.
fn type_label(field: &FieldDescriptorProto) -> String {
    let base = if field.type_name().is_empty() {
        format!("{:?}", field.r#type()).to_ascii_lowercase()
    } else {
        field.type_name().to_owned()
    };
    if field.label() == prost_types::field_descriptor_proto::Label::Repeated {
        format!("repeated {base}")
    } else {
        base
    }
}

/// Eine Feldtabelle: Name, Nummer, Typ (siehe `type_label`) und der `oneof`,
/// in dem das Feld liegt.
type FieldTable = [(&'static str, i32, &'static str, Option<&'static str>)];

/// Vergleicht die Felder einer Nachricht exakt mit einer Tabelle. Fehlt ein
/// Feld, kommt eines dazu, traegt eines eine andere Nummer, einen anderen
/// Typ oder liegt es im falschen `oneof`, kommt `Err` mit dem Grund. Die
/// Tabelle ist damit die vollstaendige Beschreibung der Nachricht, nicht
/// nur eine Untergrenze.
fn check_fields(
    name: &str,
    message: &DescriptorProto,
    expected: &FieldTable,
) -> Result<(), String> {
    for (field_name, number, ty, oneof) in expected {
        let field = message
            .field
            .iter()
            .find(|f| f.name() == *field_name)
            .ok_or_else(|| format!("{name}: missing field {field_name}"))?;
        if field.number() != *number {
            return Err(format!(
                "{name}.{field_name}: number {} != {number}",
                field.number()
            ));
        }
        let actual_type = type_label(field);
        if actual_type != *ty {
            return Err(format!("{name}.{field_name}: type {actual_type} != {ty}"));
        }
        let actual_oneof = field.oneof_index.map(|index| {
            let index = usize::try_from(index).expect("negative oneof index");
            message.oneof_decl[index].name()
        });
        if actual_oneof != *oneof {
            return Err(format!(
                "{name}.{field_name}: oneof {actual_oneof:?} != {oneof:?}"
            ));
        }
    }
    let extra: Vec<&str> = message
        .field
        .iter()
        .map(FieldDescriptorProto::name)
        .filter(|field_name| !expected.iter().any(|(e, ..)| e == field_name))
        .collect();
    if !extra.is_empty() {
        return Err(format!("{name}: fields not in the table: {extra:?}"));
    }
    Ok(())
}

/// `FlowEvent` vollstaendig: `at` und jede Variante von `oneof event`.
///
/// `all_contract_messages_exist` sieht nur die geschachtelten Nachrichten.
/// Wer `diagnostic = 12`, `rules_changed = 13` oder `agent_ask = 14` aus dem
/// `oneof` streicht, laesst `FlowEvent.RulesChanged` und `FlowEvent.AgentAsk`
/// als Nachrichten bestehen (und `Diagnostic` ist ohnehin eine eigene), also
/// bliebe jener Test gruen. Diese Tabelle nicht: sie ist exakt, samt Nummer
/// und Typ, denn beides ist Drahtformat (docs/PROTOCOL.md 4).
const FLOW_EVENT_FIELDS: &FieldTable = &[
    ("at", 1, ".google.protobuf.Timestamp", None),
    (
        "received",
        2,
        ".humanitl.v1.FlowEvent.Received",
        Some("event"),
    ),
    (
        "analyzed",
        3,
        ".humanitl.v1.FlowEvent.Analyzed",
        Some("event"),
    ),
    ("held", 4, ".humanitl.v1.FlowEvent.Held", Some("event")),
    (
        "decided",
        5,
        ".humanitl.v1.FlowEvent.Decided",
        Some("event"),
    ),
    ("forwarded", 6, ".humanitl.v1.FlowRef", Some("event")),
    (
        "response_headers",
        7,
        ".humanitl.v1.FlowEvent.ResponseHeaders",
        Some("event"),
    ),
    (
        "response_chunk",
        8,
        ".humanitl.v1.FlowEvent.ResponseChunk",
        Some("event"),
    ),
    ("recorded", 9, ".humanitl.v1.FlowRef", Some("event")),
    ("timed_out", 10, ".humanitl.v1.FlowRef", Some("event")),
    ("lagged", 11, ".humanitl.v1.FlowEvent.Lagged", Some("event")),
    ("diagnostic", 12, ".humanitl.v1.Diagnostic", Some("event")),
    (
        "rules_changed",
        13,
        ".humanitl.v1.FlowEvent.RulesChanged",
        Some("event"),
    ),
    (
        "agent_ask",
        14,
        ".humanitl.v1.FlowEvent.AgentAsk",
        Some("event"),
    ),
    ("failed", 15, ".humanitl.v1.FlowEvent.Failed", Some("event")),
    // HUM-039: ein Befund, der zu genau einem Flow gehoert. `diagnostic` (12)
    // bleibt fuer sessionweite Befunde, die noch keinem Flow gehoeren.
    (
        "flow_diagnostic",
        16,
        ".humanitl.v1.FlowEvent.FlowDiagnostic",
        Some("event"),
    ),
];

/// `DecideRequest` vollstaendig, vor allem die drei Entscheidungen in
/// `oneof decision`: `allow`, `allow_edited`, `block`. Nummer 3 ist
/// `reserved`, siehe `decide_request_reserves_the_retired_allow_edited_number`.
const DECIDE_REQUEST_FIELDS: &FieldTable = &[
    ("flow_ids", 1, "repeated string", None),
    ("allow", 2, ".google.protobuf.Empty", Some("decision")),
    (
        "allow_edited",
        7,
        ".humanitl.v1.EditedRequest",
        Some("decision"),
    ),
    (
        "block",
        4,
        ".humanitl.v1.DecideRequest.Block",
        Some("decision"),
    ),
    ("remember", 5, ".humanitl.v1.Rule", None),
    ("acknowledge_findings", 6, "bool", None),
];

/// `EditedRequest` vollstaendig: die bearbeitete Anfrage samt Body als
/// `bytes`. Das Feld `body` ist absichtlich das einzige `bytes`-Feld des
/// Vertrags, das einen ganzen Body traegt.
const EDITED_REQUEST_FIELDS: &FieldTable = &[
    ("method", 1, ".humanitl.v1.Method", None),
    ("method_raw", 2, "string", None),
    ("url", 3, "string", None),
    ("headers", 4, "repeated .humanitl.v1.Header", None),
    ("body", 5, "bytes", None),
];

/// `SandboxEvent` vollstaendig: jede Variante von `oneof event`.
///
/// Die Nummern sind Drahtformat. Ein Arm, der seine Nummer wechselt, macht aus
/// der Zusammenfassung eines Laufs beim naechsten Client eine Ausgabe des
/// Agenten. `all_contract_messages_exist` faende das nicht: `SessionSummary`
/// ist eine eigene Nachricht und bliebe stehen.
const SANDBOX_EVENT_FIELDS: &FieldTable = &[
    (
        "status",
        1,
        ".humanitl.v1.SandboxEvent.Status",
        Some("event"),
    ),
    ("check", 2, ".humanitl.v1.CheckResult", Some("event")),
    ("argv_line", 3, "string", Some("event")),
    ("diagnostic", 4, ".humanitl.v1.Diagnostic", Some("event")),
    ("log", 5, ".humanitl.v1.SandboxEvent.LogLine", Some("event")),
    (
        "output",
        6,
        ".humanitl.v1.SandboxEvent.OutputChunk",
        Some("event"),
    ),
    ("exit", 7, ".humanitl.v1.SandboxEvent.Exit", Some("event")),
    // HUM-043: der achte Arm, nicht der sechste — `output` und `exit` kamen
    // mit HUM-067 dazu.
    ("summary", 8, ".humanitl.v1.SessionSummary", Some("event")),
];

/// `SessionSummary` vollstaendig (HUM-043).
///
/// Die Nachricht reist zweimal denselben Weg: als achter Arm von
/// `SandboxEvent` und als Antwort von `GetSessionSummary`. Beide Male ist sie
/// dieselbe, und diese Tabelle ist ihre vollstaendige Beschreibung.
const SESSION_SUMMARY_FIELDS: &FieldTable = &[
    ("session_id", 1, "string", None),
    ("sandbox_id", 2, "string", None),
    ("created", 3, ".google.protobuf.Timestamp", None),
    ("work_dir", 4, "string", None),
    ("changes", 5, "repeated .humanitl.v1.FileChange", None),
    ("findings", 6, "repeated .humanitl.v1.SummaryFinding", None),
    ("symlinks", 7, "repeated .humanitl.v1.SymlinkEscape", None),
    ("unprotected", 8, "repeated string", None),
    ("scanned_bytes", 9, "uint64", None),
    ("truncated", 10, "bool", None),
    ("diagnostics", 11, "repeated .humanitl.v1.Diagnostic", None),
];

#[test]
fn sandbox_event_has_every_variant_of_the_oneof() {
    let event = message("SandboxEvent");
    assert_eq!(
        check_fields("SandboxEvent", &event, SANDBOX_EVENT_FIELDS),
        Ok(())
    );
}

/// `FileChange` vollstaendig (HUM-043).
///
/// Feld 8 ist der Vermerk, dass der Fundscan in diese Datei **nicht** gesehen
/// hat. Faellt es weg oder wandert es, liest ein Client „kein Fund" als
/// „sauber", und genau das ist der Unterschied, um den es geht.
const FILE_CHANGE_FIELDS: &FieldTable = &[
    ("path", 1, "string", None),
    ("path_hash", 2, "string", None),
    ("mangled", 3, "bool", None),
    ("kind", 4, ".humanitl.v1.FileChangeKind", None),
    ("size", 5, "uint64", None),
    ("git_metadata", 6, "bool", None),
    ("unprotected_by", 7, "string", None),
    ("unscanned", 8, ".humanitl.v1.ScanSkip", None),
];

#[test]
fn a_changed_file_says_whether_it_was_searched() {
    let change = message("FileChange");
    assert_eq!(
        check_fields("FileChange", &change, FILE_CHANGE_FIELDS),
        Ok(())
    );
}

#[test]
fn the_session_summary_carries_what_the_run_left_behind() {
    let summary = message("SessionSummary");
    assert_eq!(
        check_fields("SessionSummary", &summary, SESSION_SUMMARY_FIELDS),
        Ok(())
    );
    // Der Verweis traegt genau die eine Kennung, die `humanitl sessions
    // summary <id>` zur Hand hat.
    let reference = message("SessionSummaryRef");
    assert_eq!(
        check_fields(
            "SessionSummaryRef",
            &reference,
            &[("sandbox_id", 1, "string", None)]
        ),
        Ok(())
    );
}

#[test]
fn flow_event_has_every_variant_of_the_oneof() {
    let event = message("FlowEvent");
    assert_eq!(check_fields("FlowEvent", &event, FLOW_EVENT_FIELDS), Ok(()));
}

#[test]
fn decide_request_has_every_decision_of_the_oneof() {
    let decide = message("DecideRequest");
    assert_eq!(
        check_fields("DecideRequest", &decide, DECIDE_REQUEST_FIELDS),
        Ok(())
    );
}

#[test]
fn field_tables_catch_a_removed_or_moved_variant() {
    // Beweis, dass die Tabellen greifen, ohne die .proto anzufassen: der
    // Descriptor wird nur im Speicher beschnitten. Jede dieser Aenderungen
    // liesse `all_contract_messages_exist` gruen.
    let mut event = message("FlowEvent");
    event.field.retain(|f| f.name() != "diagnostic");
    let err = check_fields("FlowEvent", &event, FLOW_EVENT_FIELDS)
        .expect_err("dropping `diagnostic = 12` went unnoticed");
    assert!(err.contains("missing field diagnostic"), "{err}");

    let mut event = message("FlowEvent");
    let rules_changed = event
        .field
        .iter_mut()
        .find(|f| f.name() == "rules_changed")
        .expect("rules_changed is present");
    rules_changed.number = Some(16);
    let err = check_fields("FlowEvent", &event, FLOW_EVENT_FIELDS)
        .expect_err("renumbering `rules_changed` went unnoticed");
    assert!(err.contains("rules_changed: number 16 != 13"), "{err}");

    let mut event = message("FlowEvent");
    let agent_ask = event
        .field
        .iter_mut()
        .find(|f| f.name() == "agent_ask")
        .expect("agent_ask is present");
    agent_ask.oneof_index = None;
    let err = check_fields("FlowEvent", &event, FLOW_EVENT_FIELDS)
        .expect_err("moving `agent_ask` out of the oneof went unnoticed");
    assert!(err.contains("agent_ask: oneof None"), "{err}");

    let mut decide = message("DecideRequest");
    decide.field.retain(|f| f.name() != "block");
    let err = check_fields("DecideRequest", &decide, DECIDE_REQUEST_FIELDS)
        .expect_err("dropping `block = 4` went unnoticed");
    assert!(err.contains("missing field block"), "{err}");
}

/// Die RPCs aus Backlog 33: Name, Eingabe, Ausgabe, Client-Stream, Server-Stream.
const EXPECTED_RPCS: &[(&str, &str, &str, bool, bool)] = &[
    (
        "GetInfo",
        ".google.protobuf.Empty",
        ".humanitl.v1.Info",
        false,
        false,
    ),
    (
        "Subscribe",
        ".humanitl.v1.SubscribeRequest",
        ".humanitl.v1.FlowEvent",
        false,
        true,
    ),
    (
        "ListFlows",
        ".humanitl.v1.ListFlowsRequest",
        ".humanitl.v1.FlowPage",
        false,
        false,
    ),
    (
        "GetFlow",
        ".humanitl.v1.FlowRef",
        ".humanitl.v1.FlowDetail",
        false,
        false,
    ),
    (
        "GetBody",
        ".humanitl.v1.BodyRef",
        ".humanitl.v1.BodyChunk",
        false,
        true,
    ),
    (
        "Decide",
        ".humanitl.v1.DecideRequest",
        ".humanitl.v1.DecideResponse",
        false,
        false,
    ),
    (
        "Rules",
        ".humanitl.v1.RulesRequest",
        ".humanitl.v1.RulesResponse",
        false,
        false,
    ),
    (
        "Sandbox",
        ".humanitl.v1.SandboxRequest",
        ".humanitl.v1.SandboxEvent",
        false,
        true,
    ),
    (
        "Terminal",
        ".humanitl.v1.TerminalInput",
        ".humanitl.v1.TerminalOutput",
        true,
        true,
    ),
    (
        "Audit",
        ".humanitl.v1.AuditRequest",
        ".humanitl.v1.AuditResponse",
        false,
        false,
    ),
    (
        "GetConfig",
        ".humanitl.v1.GetConfigRequest",
        ".humanitl.v1.ConfigSnapshot",
        false,
        false,
    ),
    (
        "SetConfig",
        ".humanitl.v1.SetConfigRequest",
        ".humanitl.v1.ConfigSnapshot",
        false,
        false,
    ),
    (
        "Doctor",
        ".google.protobuf.Empty",
        ".humanitl.v1.DoctorReport",
        false,
        false,
    ),
    (
        "DiscoverLlm",
        ".humanitl.v1.DiscoverRequest",
        ".humanitl.v1.DiscoverResult",
        false,
        true,
    ),
    // HUM-039: die Probe eines einzelnen Endpunkts, host-seitig und nur
    // lesend. Nicht in BACKLOG.md 3.3, aber Teil des Vertrags, seit der
    // Setup-Bildschirm sie braucht.
    (
        "ProbeLlm",
        ".humanitl.v1.ProbeLlmRequest",
        ".humanitl.v1.ProbeLlmResponse",
        false,
        false,
    ),
    // HUM-043: die gespeicherte Zusammenfassung eines Sandbox-Laufs. Nicht in
    // BACKLOG.md 3.3; `Sandbox` ist ein Strom ueber die laufende Sandbox und
    // kann einen Lauf, der beendet ist, nicht mehr beantworten.
    (
        "GetSessionSummary",
        ".humanitl.v1.SessionSummaryRef",
        ".humanitl.v1.SessionSummary",
        false,
        false,
    ),
];

#[test]
fn service_has_every_rpc_of_backlog_33() {
    let service = files()
        .into_iter()
        .flat_map(|f| f.service)
        .find(|s| s.name() == "Humanitl")
        .expect("service Humanitl is missing");

    assert_eq!(
        service.method.len(),
        EXPECTED_RPCS.len(),
        "the service has {} RPCs, the table expects {}",
        service.method.len(),
        EXPECTED_RPCS.len()
    );

    for (name, input, output, client_stream, server_stream) in EXPECTED_RPCS {
        let method = service
            .method
            .iter()
            .find(|m| m.name() == *name)
            .unwrap_or_else(|| panic!("missing rpc {name}"));
        assert_eq!(method.input_type(), *input, "{name}: wrong input");
        assert_eq!(method.output_type(), *output, "{name}: wrong output");
        assert_eq!(
            method.client_streaming(),
            *client_stream,
            "{name}: wrong client streaming"
        );
        assert_eq!(
            method.server_streaming(),
            *server_stream,
            "{name}: wrong server streaming"
        );
    }
}

#[test]
fn rules_live_in_their_own_file_imported_by_humanitl() {
    let files = files();
    let rules = files
        .iter()
        .find(|f| f.name().ends_with("rules.proto"))
        .expect("rules.proto is missing");
    assert!(
        rules.message_type.iter().any(|m| m.name() == "Rule"),
        "message Rule does not live in rules.proto"
    );

    let main = files
        .iter()
        .find(|f| f.name().ends_with("humanitl.proto"))
        .expect("humanitl.proto is missing");
    assert!(
        main.dependency.iter().any(|d| d.ends_with("rules.proto")),
        "humanitl.proto does not import rules.proto"
    );
}
