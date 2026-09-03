//! Tabellengetriebene Prüfung der Tier-1-Detektoren.
//!
//! Jede Zeile ist ein Body und die Funde, die er ergeben muss, als
//! `art@anzeige`. Eine leere Erwartung ist ein Fehlalarm, der nicht passieren
//! darf; genau die stehen in der Spezifikation von HUM-025 neben den echten
//! Treffern.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use humanitl_core::{Authority, HostName, HttpRequest, Method, Scheme};
use humanitl_findings::{DetectorRegistry, FindingsSettings};

/// Der Body einer Anfrage an einen textartigen Endpunkt.
fn scan(body: &str, terms: &[&str]) -> Vec<String> {
    let settings =
        FindingsSettings::default().with_user_terms(terms.iter().map(|term| (*term).to_owned()));
    let registry = DetectorRegistry::tier1(&settings).unwrap();
    let request = HttpRequest::new(
        Method::POST,
        Scheme::Https,
        Authority::with_scheme(HostName::Dns("api.example.com".to_owned()), Scheme::Https),
        "/v1/chat",
    );
    registry
        .scan_request(&request, body.as_bytes())
        .into_iter()
        .map(|found| format!("{}@{}", found.kind, found.display_prefix))
        .collect()
}

struct Case {
    name: &'static str,
    /// Der Body in Teilen. Zusammengesetzt wird erst zur Laufzeit, weil ein
    /// Geheimnis-Detektor nur an echt geformten Werten zu prüfen ist und
    /// Der Push-Schutz von GitHub genau diese Form im Quelltext blockiert
    /// (`backlog/CONVENTIONS.md` 4.13). Ein Wert ohne diese Form braucht nur
    /// einen Teil.
    body: &'static [&'static str],
    expected: &'static [&'static str],
}

impl Case {
    fn body(&self) -> String {
        self.body.concat()
    }
}

const CASES: &[Case] = &[
    Case {
        name: "iban_valid",
        body: &["Bitte an DE89 3704 0044 0532 0130 00 überweisen"],
        expected: &["iban@DE89 …"],
    },
    Case {
        name: "iban_valid_compact",
        body: &["GB82 WEST 1234 5698 7654 32"],
        expected: &["iban@GB82 …"],
    },
    Case {
        name: "iban_invalid_checksum",
        body: &["Bitte an DE89 3704 0044 0532 0130 01 überweisen"],
        // Kein Fund: die Prüfziffer stimmt nicht, und die Ziffern gehören
        // trotzdem der IBAN. Ohne diese Sperre wären es zwei Fehlalarme, denn
        // die veränderte letzte Stelle macht die Folge Luhn-gültig und lässt
        // sie mit 37 wie eine Amex-Karte beginnen.
        expected: &[],
    },
    Case {
        name: "card_luhn_valid",
        body: &["Karte 4111 1111 1111 1111"],
        expected: &["credit_card@**** 1111"],
    },
    Case {
        name: "card_luhn_invalid",
        body: &["Karte 4111 1111 1111 1112"],
        expected: &[],
    },
    Case {
        name: "jwt_detected",
        body: &[
            "token eyJ0eXAiOiJKV1QiLA0KICJhbGciOiJIUzI1NiJ9.eyJpc3MiOiJqb2UiLA0KICJleHAiOjEzMDA4MTkzODAsDQogImh0dHA6Ly9leGFtcGxlLmNvbS9pc19yb290Ijp0cnVlfQ.dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
        ],
        expected: &["jwt@eyJ0eX…"],
    },
    Case {
        name: "github_pat_36",
        body: &["GITHUB_TOKEN=ghp_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"],
        expected: &["api_key:github@ghp_aa…"],
    },
    Case {
        name: "github_pat_35",
        body: &["GITHUB_TOKEN=ghp_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"],
        expected: &[],
    },
    Case {
        name: "aws_access_token",
        body: &["aws_access_key_id = AKIAIOSFODNN7EXAMPLE"],
        expected: &["api_key:aws@AKIAIO…"],
    },
    Case {
        name: "openai_scoped_key",
        body: &["OPENAI_API_KEY=sk-proj-abcdefghijklmnopqrstuvwx"],
        expected: &["api_key:openai@sk-pro…"],
    },
    Case {
        name: "anthropic_key",
        body: &[
            "sk-ant",
            "-api03-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaAA",
        ],
        expected: &["api_key:anthropic@sk-ant…"],
    },
    Case {
        name: "slack_token",
        body: &["xoxb", "-1234567890-abcdefghij"],
        expected: &["api_key:slack@xoxb-1…"],
    },
    Case {
        name: "stripe_key",
        body: &["sk_live", "_abcdefghijklmnopqrstuvwx"],
        expected: &["api_key:stripe@sk_liv…"],
    },
    Case {
        name: "google_api_key",
        body: &["AIza", "SyA1234567890abcdefghijklmnopqrstuv"],
        expected: &["api_key:google@AIzaSy…"],
    },
    Case {
        name: "private_key_header",
        body: &["-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaA=="],
        expected: &["api_key:private_key@-----B…"],
    },
    Case {
        name: "email_found",
        body: &["Kontakt: vorname.nachname@kunde.de"],
        expected: &["email@v***@kunde.de"],
    },
    Case {
        name: "email_is_no_finding_without_a_tld",
        body: &["Kontakt: vorname@localhost"],
        expected: &[],
    },
    Case {
        name: "phone_international",
        body: &["Tel +49 30 1234567"],
        expected: &["phone@+49 …"],
    },
    Case {
        name: "phone_national_is_no_finding",
        body: &["Tel 030 1234567"],
        expected: &[],
    },
    Case {
        name: "ipv4_found",
        body: &["llm auf 192.168.1.20:11434"],
        expected: &["ipv4@192.168.1.20"],
    },
    Case {
        name: "ipv4_loopback_is_no_finding",
        body: &["llm auf 127.0.0.1:11434"],
        expected: &[],
    },
    Case {
        name: "ipv4_octet_over_255_is_no_finding",
        body: &["version 300.1.2.3"],
        expected: &[],
    },
    Case {
        name: "a_real_card_next_to_an_iban_stays_a_card",
        body: &["IBAN DE89 3704 0044 0532 0130 00, Karte 4111 1111 1111 1111"],
        expected: &["iban@DE89 …", "credit_card@**** 1111"],
    },
    Case {
        name: "plain_prose_is_no_finding",
        body: &["Bitte fasse die Datei README.md zusammen und antworte auf Deutsch."],
        expected: &[],
    },
];

#[test]
fn the_table_holds() {
    for case in CASES {
        let found = scan(&case.body(), &[]);
        assert_eq!(found, case.expected, "Fall {}", case.name);
    }
}

#[test]
fn user_term_word_boundary() {
    assert_eq!(
        scan("Acme Corp", &["Acme"]),
        vec!["user_term:Acme@Acme".to_owned()]
    );
    assert!(scan("Acmeified", &["Acme"]).is_empty());
    assert_eq!(
        scan("ACME", &["Acme"]),
        vec!["user_term:Acme@ACME".to_owned()]
    );
    assert_eq!(
        scan("Kunde Müller, ja", &["Müller"]),
        vec!["user_term:Müller@Müller".to_owned()]
    );
    assert!(scan("Müllerstraße", &["Müller"]).is_empty());
}

#[test]
fn no_finding_ever_carries_its_value() {
    // Zur Laufzeit zusammengesetzt, siehe Doc-Kommentar an `Case::body`.
    let secret = format!("{}{}", "ghp", "_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let settings = FindingsSettings::default();
    let registry = DetectorRegistry::tier1(&settings).unwrap();
    let request = HttpRequest::new(
        Method::POST,
        Scheme::Https,
        Authority::with_scheme(HostName::Dns("api.example.com".to_owned()), Scheme::Https),
        "/v1/chat",
    );
    let findings = registry.scan_request(&request, secret.as_bytes());
    assert_eq!(findings.len(), 1);
    let rendered = format!("{findings:?}");
    assert!(!rendered.contains(&secret), "{rendered}");
    assert!(!rendered.contains("aaaaaaaaaaaa"), "{rendered}");
}
