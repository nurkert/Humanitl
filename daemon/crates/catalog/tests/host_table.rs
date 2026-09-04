//! Die Matching-Tabelle aus `backlog/sprint-2.md` (HUM-022), soweit sie für
//! Namensmuster gilt, durch die Schnittstelle des Katalogs gefahren.
//!
//! Die Datei prüft zwei Dinge, und sie tut es an denselben nummerierten Zeilen
//! wie `daemon/crates/rules/tests/host_table.rs`:
//!
//! 1. Was der Katalog **selbst** entscheidet: welche der vier Musterformen er
//!    annimmt (`ip:`, `cidr:` und eine nackte Adresse nimmt er nicht) und dass
//!    eine Adresse nie einen Katalogeintrag trifft.
//! 2. Dass [`humanitl_catalog::pattern::matches`] die Zeilen der Tabelle
//!    tatsächlich so beantwortet — end-to-end, von `parse` bis zum Ergebnis.
//!
//! Der Gang über die Labels selbst liegt seit dem Umzug nur noch einmal im
//! Baum, als [`humanitl_core::rule::glob_matches`], und seine eigene Tabelle
//! steht in `daemon/crates/core-types/tests/host_glob.rs`. Die Zeilen hier sind
//! deshalb keine zweite Fassung dieser Tabelle mehr, sondern die Probe, dass
//! der Katalog den Kern auch wirklich fragt: Wer `matches` durch `ends_with`
//! ersetzte, käme an `host_glob.rs` vorbei, aber nicht hier vorbei.
//!
//! Die Zeilen 27 bis 40 der Tabelle betreffen `ip:` und `cidr:`. Der Katalog
//! kennt diese Muster nicht; `catalog_rejects_address_patterns` hält das fest.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use humanitl_catalog::pattern::{matches, parse};
use humanitl_core::HostName;

fn hits(pat: &str, raw_host: &str) -> bool {
    let pattern = parse(pat).unwrap_or_else(|err| panic!("pattern {pat:?} must parse: {err}"));
    let host = HostName::parse(raw_host)
        .unwrap_or_else(|err| panic!("host {raw_host:?} must parse: {err}"));
    matches(&pattern, &host)
}

#[test]
fn host_01() {
    assert!(hits("*.github.com", "api.github.com"), "one label");
}

#[test]
fn host_02() {
    assert!(
        !hits("*.github.com", "github.com"),
        "`*` needs exactly one label"
    );
}

#[test]
fn host_03() {
    assert!(!hits("*.github.com", "a.b.github.com"), "two labels");
}

#[test]
fn host_04() {
    assert!(
        !hits("*.github.com", "evil-github.com"),
        "a label is not a substring"
    );
}

#[test]
fn host_05() {
    assert!(!hits("*.github.com", "github.com.evil.io"), "suffix trick");
}

#[test]
fn host_06() {
    assert!(hits("*.github.com", "API.GITHUB.COM"), "case");
}

#[test]
fn host_07() {
    assert!(hits("*.github.com", "api.github.com."), "trailing dot");
}

#[test]
fn host_08() {
    assert!(hits("**.github.com", "github.com"), "apex exception");
}

#[test]
fn host_09() {
    assert!(hits("**.github.com", "api.github.com"));
}

#[test]
fn host_10() {
    assert!(hits("**.github.com", "a.b.c.github.com"), "several labels");
}

#[test]
fn host_11() {
    assert!(!hits("**.github.com", "github.com.evil.io"));
}

#[test]
fn host_12() {
    assert!(!hits("**.github.com", "notgithub.com"));
}

#[test]
fn host_13() {
    assert!(hits("github.com", "github.com"), "exact");
}

#[test]
fn host_14() {
    assert!(!hits("github.com", "www.github.com"), "exact means exact");
}

#[test]
fn host_15() {
    assert!(hits("github.com", "GitHub.Com"), "case");
}

#[test]
fn host_16() {
    assert!(hits("*.*.example.com", "a.b.example.com"));
}

#[test]
fn host_17() {
    assert!(!hits("*.*.example.com", "a.example.com"));
}

#[test]
fn host_18() {
    assert!(hits("api.*.com", "api.github.com"), "star in the middle");
}

#[test]
fn host_19() {
    assert!(!hits("api.*.com", "api.co.uk"), "the last label decides");
}

#[test]
fn host_20() {
    assert!(hits("**", "anything.example"), "every DNS host");
}

#[test]
fn host_21() {
    assert!(
        !hits("**", "140.82.112.3"),
        "a glob never matches an address"
    );
}

#[test]
fn host_22() {
    assert!(hits("*", "localhost"), "one label");
}

#[test]
fn host_23() {
    assert!(!hits("*", "a.b"));
}

#[test]
fn host_24() {
    assert!(
        hits("*.münchen.de", "shop.münchen.de"),
        "unicode is normalised to A-labels on both sides"
    );
    assert!(hits("*.münchen.de", "shop.xn--mnchen-3ya.de"));
}

#[test]
fn host_26() {
    assert!(!hits("*.github.com", "140.82.112.3"), "an address");
}

#[test]
fn catalog_rejects_address_patterns() {
    // Zeilen 27 bis 40 der Tabelle: der Katalog beschreibt Namen, nicht
    // Adressen. Ein `ip:`- oder `cidr:`-Muster wird beim Laden abgelehnt,
    // statt still nie zu treffen.
    for text in ["ip:140.82.112.3", "cidr:192.168.0.0/16", "140.82.112.3"] {
        assert!(parse(text).is_err(), "{text} must be rejected");
    }
}
