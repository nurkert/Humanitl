//! Die Prüfung hinter dem Fuzz-Target `rules_yaml` (HUM-056).
//!
//! Eine Quelle für beide Seiten: `main.rs` bindet die Datei als Modul ein, das
//! Target unter `daemon/fuzz/fuzz_targets/rules_yaml.rs` über `#[path]`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::net::{IpAddr, Ipv4Addr};

use chrono::{DateTime, Utc};
use humanitl_core::{HostName, Method, Scheme, SessionId, Upgrade};
use humanitl_rules::{RequestKey, parse_rules_for_session, serialize_rules};

/// Liest `data` als `rules.yaml`, schreibt den Regelsatz zurück und wertet ihn
/// aus.
///
/// Invarianten:
///
/// - Kein Panic, auch nicht bei ungültigem UTF-8 (dann gibt es nichts zu
///   lesen) und nicht bei einem regulären Ausdruck mit Backtracking-Bombe
///   (`regex` arbeitet linear; ein Muster, das sich nicht übersetzen lässt,
///   ist ein Befund, kein Absturz).
/// - Was sich lesen lässt, lässt sich schreiben und wieder lesen, und es ist
///   danach derselbe Regelsatz.
/// - Die Auswertung endet für drei feste Anfragen: `api.github.com`, die
///   Adresse `192.168.1.50` und ein WebSocket-Upgrade.
pub fn rules_yaml(data: &[u8]) {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let session = SessionId::nil();
    let Ok((parsed, _warnings)) = parse_rules_for_session(text, session) else {
        return;
    };

    let written = serialize_rules(&parsed);
    let (again, _) = parse_rules_for_session(&written, session).unwrap_or_else(|err| {
        panic!("a rule set that was read does not read back:\n{written}\n{err:?}")
    });
    assert!(
        again == parsed,
        "the rule set changed on the way through serialize_rules:\n{written}"
    );

    let now = fixed_now();
    let github = HostName::Dns("api.github.com".to_owned());
    let lan = HostName::Ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 50)));
    let keys = [
        RequestKey::new(&github, &Method::GET, "/repos/a/b?x=1", Scheme::Https, 443),
        RequestKey::new(&lan, &Method::POST, "/api/chat", Scheme::Http, 11434),
        RequestKey::new(&github, &Method::GET, "/socket", Scheme::Https, 443)
            .with_upgrade(Upgrade::WebSocket),
    ];
    for key in &keys {
        let _ = parsed.evaluate(key, now, session);
    }
}

/// Der feste Zeitpunkt jeder Auswertung; ein Target liest keine Uhr.
fn fixed_now() -> DateTime<Utc> {
    DateTime::from_timestamp(1_790_000_000, 0).unwrap_or_default()
}
