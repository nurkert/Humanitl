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

/// HUM-146: Sonderbereiche und IPv6-Adressen, die eine IPv4-Adresse in sich
/// tragen. Je Bereich eine Adresse innen und eine knapp außerhalb, damit eine
/// zu weite Maske genauso auffällt wie eine fehlende.
#[test]
fn is_private_covers_special_ranges_and_embedded_ipv4() {
    let cases = [
        ("192.0.0.1", true),
        ("192.0.1.1", false),
        ("198.18.0.1", true),
        ("198.19.255.254", true),
        ("198.17.255.255", false),
        ("198.20.0.1", false),
        ("240.0.0.1", true),
        ("239.255.255.255", false),
        ("[fec0::1]", true),
        ("[feff::1]", true),
        ("[ff00::1]", false),
        ("[fe7f::1]", false),
        // NAT64, 64:ff9b::/96: über das Gateway die Metadaten-Adresse.
        ("[64:ff9b::a9fe:a9fe]", true),
        ("[64:ff9b::a00:1]", true),
        ("[64:ff9b::808:808]", false),
        ("[64:ff9b:0:1::a9fe:a9fe]", false),
        // Lokales NAT64, 64:ff9b:1::/48, gilt ganz als privat.
        ("[64:ff9b:1::808:808]", true),
        ("[64:ff9b:2::1]", false),
        // IPv4-translated, ::ffff:0:0/96.
        ("[::ffff:0:a9fe:a9fe]", true),
        ("[::ffff:0:808:808]", false),
        // 6to4, 2002::/16: die IPv4-Adresse steht in den Bits 16 bis 48.
        ("[2002:a9fe:a9fe::1]", true),
        ("[2002:a00:1::1]", true),
        ("[2002:808:808::1]", false),
        ("[2003:a9fe:a9fe::1]", false),
        // ISATAP-Kennung unter beliebigem Präfix.
        ("[2001:db8::5efe:a9fe:a9fe]", true),
        ("[2001:db8::200:5efe:a00:1]", true),
        ("[2001:db8::5efe:808:808]", false),
        ("[2001:db8::5eff:a9fe:a9fe]", false),
        // Teredo, 2001::/32; 2001:1:: und 2001:4860:: gehören nicht dazu.
        ("[2001:0:4136:e378:8000:63bf:3fff:fdd2]", true),
        ("[2001:1::1]", false),
        ("[2001:4860:4860::8888]", false),
    ];
    for (input, expected) in cases {
        let host = HostName::parse(input).unwrap_or_else(|err| panic!("{input}: {err}"));
        assert_eq!(host.is_private(), expected, "input {input}");
    }
}
