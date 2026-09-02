//! Tabellentest der Host-Normalisierung.
//!
//! Zwei Schreibweisen desselben Ziels dürfen nie unterschiedlich an einer Regel
//! vorbeikommen. Die Fälle mit Oktal- und Hex-Schreibweise stehen hier, weil
//! genau sie der übliche Weg an einer Host-Prüfung vorbei sind.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::net::IpAddr;

use humanitl_core::HostName;

#[test]
fn parse_table() {
    let dns = [
        ("GitHub.COM.", "github.com"),
        ("github.com", "github.com"),
        ("münchen.de", "xn--mnchen-3ya.de"),
        ("MÜNCHEN.DE.", "xn--mnchen-3ya.de"),
        ("xn--mnchen-3ya.de", "xn--mnchen-3ya.de"),
        ("api.github.com", "api.github.com"),
    ];
    for (input, expected) in dns {
        let parsed = HostName::parse(input).unwrap_or_else(|err| panic!("{input}: {err}"));
        assert_eq!(parsed, HostName::Dns(expected.to_owned()), "input {input}");
        assert_eq!(parsed.to_string(), expected);
    }

    let ips = ["192.168.1.50", "[::1]", "::1", "8.8.8.8", "[2606:4700::1]"];
    for input in ips {
        let parsed = HostName::parse(input).unwrap_or_else(|err| panic!("{input}: {err}"));
        assert!(
            matches!(parsed, HostName::Ip(_)),
            "{input} should be an address"
        );
        assert!(parsed.labels().is_none());
    }

    let broken = [
        "0x7f.1",
        "0177.0.0.1",
        "",
        "a..b",
        "exa mple.com",
        ".",
        "999.1.1.1",
        "1.2.3.4.5",
        "[::1",
        "[not-an-ip]",
        "exam\u{0000}ple.com",
        "-leading.example.com",
    ];
    for input in broken {
        assert!(
            HostName::parse(input).is_err(),
            "{input} should not be a host"
        );
    }
}

#[test]
fn parse_error_keeps_the_input() {
    let err = HostName::parse("0177.0.0.1").expect_err("must fail");
    assert_eq!(err.input, "0177.0.0.1");
    assert!(err.to_string().contains("0177.0.0.1"));
}

#[test]
fn display_returns_ulabel() {
    let host = HostName::parse("xn--mnchen-3ya.de").unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(host.display(), "münchen.de");
    assert_eq!(host.to_string(), "xn--mnchen-3ya.de");

    let ip = HostName::parse("[::1]").unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(ip.display(), "::1");
}

#[test]
fn labels_split_a_dns_name() {
    let host = HostName::parse("api.github.com").unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(host.labels(), Some(vec!["api", "github", "com"]));
    assert_eq!(host.as_dns(), Some("api.github.com"));
    assert_eq!(host.as_ip(), None);
}

#[test]
fn is_private_table() {
    let cases = [
        ("10.0.0.1", true),
        ("172.16.0.1", true),
        ("192.168.1.50", true),
        ("127.0.0.1", true),
        ("169.254.169.254", true),
        ("100.64.0.1", true),
        ("0.0.0.0", true),
        ("8.8.8.8", false),
        ("140.82.121.4", false),
        ("[fc00::1]", true),
        ("[fe80::1]", true),
        ("[::1]", true),
        ("[::ffff:169.254.169.254]", true),
        ("[::169.254.169.254]", true),
        ("[::127.0.0.1]", true),
        ("[::ffff:8.8.8.8]", false),
        ("[2606:4700::1111]", false),
    ];
    for (input, expected) in cases {
        let host = HostName::parse(input).unwrap_or_else(|err| panic!("{input}: {err}"));
        assert_eq!(host.is_private(), expected, "input {input}");
    }

    let dns = HostName::parse("localhost").unwrap_or_else(|err| panic!("{err}"));
    assert!(
        !dns.is_private(),
        "a name says nothing before it is resolved"
    );

    let address: IpAddr = "10.1.2.3".parse().unwrap_or_else(|err| panic!("{err}"));
    assert!(humanitl_core::ip_is_private(address));
}
