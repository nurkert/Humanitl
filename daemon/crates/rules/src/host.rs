//! Host-Muster: einlesen, prüfen, vergleichen.
//!
//! Verglichen werden immer ganze Labels, nie Zeichenketten. Das ist der Kern
//! der Sicherheitsaussage dieser Datei: `*.github.com` darf `evil-github.com`
//! nicht treffen und `github.com.evil.io` auch nicht. Beides wäre mit
//! `ends_with` oder `contains` sofort falsch, und beides ist der übliche Weg an
//! einer Host-Prüfung vorbei (BACKLOG.md 4.5 Test 4).
//!
//! Die zweite Aussage: eine IP-Adresse trifft nie ein Glob. Wer `169.254.169.254`
//! oder das Heimnetz erlauben will, schreibt `ip:` oder `cidr:` hin (ADR-007).

use std::net::IpAddr;

use humanitl_core::diagnostics::codes::{RULES_002, RULES_003};
use humanitl_core::rule::{HostPattern, glob_matches};
use humanitl_core::{Diagnostic, HostName, Severity};

/// Liest ein Host-Muster aus der Schreibweise von `rules.yaml`.
///
/// Ergänzt [`HostPattern::parse`] aus dem Kern um die Befunde, die ein Mensch
/// sehen soll:
///
/// - Ein Punycode-Literal (`xn--…`), das der Mensch selbst geschrieben hat, ist
///   eine Warnung (`RULES_002`). Es ist gültig, aber niemand liest ihm an,
///   welchen Namen es trägt; wer `münchen.de` meint, soll `münchen.de`
///   schreiben. Geprüft wird deshalb der eingegebene Text, nicht das
///   normalisierte Muster: `münchen.de` wird von IDNA selbst zu
///   `xn--mnchen-3ya.de` und ist genau die Schreibweise, die wir empfehlen.
/// - Eine nackte IP-Adresse an der Stelle des Hosts wird zu [`HostPattern::Ip`]
///   und ebenfalls gemeldet (`RULES_002`): so ist die Regel wirksam, statt als
///   toter Glob dazustehen, und die Schreibweise `ip:` bleibt die eindeutige.
/// - Ein Netz in IPv4-mapped Schreibweise (`cidr:::ffff:192.168.0.0/112`) wird
///   auf seine IPv4-Form gebracht, damit die Präfixlänge dieselbe Menge von
///   Adressen bezeichnet wie vorher.
///
/// # Errors
///
/// Ein [`Diagnostic`] mit `RULES_003`, wenn das Muster kein Host, kein Glob und
/// keine Adresse ist, oder wenn ein IPv4-mapped Netz eine Präfixlänge unter 96
/// trägt und damit über die eingebettete IPv4-Adresse hinausreicht.
pub fn parse_pattern(input: &str) -> Result<(HostPattern, Vec<Diagnostic>), Diagnostic> {
    let pattern = HostPattern::parse(input).map_err(|err| {
        Diagnostic::builder(RULES_003, Severity::Error)
            .why(format!("host pattern {input:?} is invalid: {}", err.reason))
            .build()
    })?;

    let mut diagnostics = Vec::new();
    let pattern = match pattern {
        HostPattern::Exact(HostName::Ip(addr)) => {
            diagnostics.push(
                Diagnostic::builder(RULES_002, Severity::Warning)
                    .why(format!(
                        "host pattern {input:?} is an IP address, not a name; \
                         it is read as `ip:{addr}`, and only that spelling matches an address"
                    ))
                    .build(),
            );
            HostPattern::Ip(addr)
        }
        HostPattern::Cidr { addr, prefix } => {
            let Some((addr, prefix)) = normalize_network(addr, prefix) else {
                return Err(Diagnostic::builder(RULES_003, Severity::Error)
                    .why(format!(
                        "host pattern {input:?} is an IPv4-mapped network with a prefix below 96; \
                         write the network in its IPv4 form, so its length means what it says"
                    ))
                    .build());
            };
            HostPattern::Cidr { addr, prefix }
        }
        other => other,
    };

    if let Some(label) = punycode_literal(input) {
        diagnostics.push(
            Diagnostic::builder(RULES_002, Severity::Warning)
                .why(format!(
                    "host pattern {input:?} contains the punycode label {label:?}; \
                     write the name itself, so what the rule allows can be read"
                ))
                .build(),
        );
    }

    Ok((pattern, diagnostics))
}

/// Wahr, wenn das Muster den Host trifft.
///
/// Der Algorithmus steht in `backlog/sprint-2.md` unter HUM-022 und in
/// `backlog/CONVENTIONS.md` 3.3:
///
/// 1. Eine IP-Adresse trifft nur [`HostPattern::Ip`] und [`HostPattern::Cidr`],
///    nie ein Glob und nie einen exakten Namen.
/// 2. Ein Glob vergleicht Label für Label: `*` steht für genau ein Label, `**`
///    für ein oder mehr.
/// 3. Beginnt das Muster mit `**` und hat es mehr als ein Label, trifft es
///    zusätzlich den Namen ohne diese Labels: `**.example.com` trifft auch
///    `example.com` selbst (Apex-Ausnahme).
#[must_use]
pub fn matches(pattern: &HostPattern, host: &HostName) -> bool {
    match (pattern, host) {
        // Eine Adresse trifft genau ihre Adresse. Ein exaktes Muster, in dem
        // eine Adresse steht, zählt hier mit: aus `rules.yaml` kommt es über
        // [`parse_pattern`] gar nicht erst in dieser Form, und wer es im
        // Programm baut, soll keine still wirkungslose Block-Regel bekommen.
        (
            HostPattern::Exact(HostName::Ip(expected)) | HostPattern::Ip(expected),
            HostName::Ip(actual),
        ) => canonical(*expected) == canonical(*actual),
        (HostPattern::Exact(expected), _) => expected == host,
        (HostPattern::Glob(glob), HostName::Dns(_)) => glob_matches(glob, host),
        (HostPattern::Cidr { addr, prefix }, HostName::Ip(actual)) => {
            // Ein Netz wird zusammen mit seiner Präfixlänge umgerechnet, nie
            // die Adresse allein: `::ffff:192.168.0.0/112` bezeichnet
            // `192.168.0.0/16`, und eine Länge ohne ihre Familie ist keine
            // Länge. Ein Netz, das sich nicht umrechnen lässt, trifft nichts
            // (aus `rules.yaml` kommt es über [`parse_pattern`] gar nicht so
            // weit).
            match normalize_network(*addr, *prefix) {
                Some((addr, prefix)) => in_network(addr, prefix, canonical(*actual)),
                None => false,
            }
        }
        // Ein Glob trifft nie eine Adresse, `ip:` und `cidr:` nie einen Namen.
        (HostPattern::Glob(_) | HostPattern::Ip(_) | HostPattern::Cidr { .. }, _) => false,
    }
}

/// Entpackt eine IPv4-Adresse aus ihrer IPv6-Schreibweise.
///
/// `::ffff:140.82.112.3` und `140.82.112.3` sind dieselbe Adresse; ohne diesen
/// Schritt käme die eine Schreibweise an einer Regel für die andere vorbei.
fn canonical(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V4(_) => ip,
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => IpAddr::V4(v4),
            None => ip,
        },
    }
}

/// Bringt ein Netz auf die Familie, in der es verglichen wird.
///
/// Ein IPv4-mapped Netz (`::ffff:a.b.c.d/n`) wird zu `a.b.c.d/(n - 96)`: die
/// ersten 96 Bit sind das Präfix `::ffff:0:0/96`, das jede IPv4-Adresse trägt.
/// Ein Netz mit `n < 96` reicht über dieses Präfix hinaus und bezeichnet damit
/// nicht mehr die eingebettete IPv4-Adresse; es hat in einer Regel nichts zu
/// suchen und wird abgelehnt (`None`). Jedes andere Netz bleibt, wie es ist.
fn normalize_network(addr: IpAddr, prefix: u8) -> Option<(IpAddr, u8)> {
    const MAPPED_PREFIX_BITS: u8 = 96;

    match addr {
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) if prefix >= MAPPED_PREFIX_BITS => {
                Some((IpAddr::V4(v4), prefix - MAPPED_PREFIX_BITS))
            }
            Some(_) => None,
            None => Some((addr, prefix)),
        },
        IpAddr::V4(_) => Some((addr, prefix)),
    }
}

/// Wahr, wenn `actual` im Netz `addr/prefix` liegt.
///
/// Adressfamilien werden nie gemischt: ein IPv4-Netz enthält keine
/// IPv6-Adresse, auch keine, die eine IPv4-Adresse einpackt — die ist vorher
/// über [`canonical`] gelaufen.
fn in_network(addr: IpAddr, prefix: u8, actual: IpAddr) -> bool {
    // Die Bitbreite steht als Zahl da, statt über `Ipv4Addr::BITS` zu kommen:
    // aus `std::net` benutzt diese Crate genau einen Typ, `IpAddr`
    // (Akzeptanzkriterium HUM-022), und 32 beziehungsweise 128 sind keine
    // Größen, die sich ändern.
    match (addr, actual) {
        (IpAddr::V4(net), IpAddr::V4(ip)) => same_prefix(&net.octets(), &ip.octets(), prefix, 32),
        (IpAddr::V6(net), IpAddr::V6(ip)) => same_prefix(&net.octets(), &ip.octets(), prefix, 128),
        _ => false,
    }
}

fn same_prefix(net: &[u8], ip: &[u8], prefix: u8, bits: u32) -> bool {
    let prefix = u32::from(prefix).min(bits);
    let whole = (prefix / 8) as usize;
    let rest = prefix % 8;
    if net[..whole] != ip[..whole] {
        return false;
    }
    if rest == 0 {
        return true;
    }
    let mask = 0xffu8 << (8 - rest);
    net[whole] & mask == ip[whole] & mask
}

/// Das erste Punycode-Literal im eingegebenen Text, falls es eines gibt.
///
/// Geprüft wird der Text, den der Mensch geschrieben hat, nicht das
/// normalisierte Muster: nach IDNA trägt jeder Unicode-Name ein `xn--`-Label,
/// und `münchen.de` zu melden, weil es zu `xn--mnchen-3ya.de` wird, hieße die
/// empfohlene Schreibweise zu beanstanden. Adressmuster (`ip:`, `cidr:`) haben
/// keine Labels und werden übergangen.
fn punycode_literal(input: &str) -> Option<String> {
    if input.starts_with("ip:") || input.starts_with("cidr:") {
        return None;
    }
    input
        .split('.')
        .find(|label| {
            label
                .get(..4)
                .is_some_and(|head| head.eq_ignore_ascii_case("xn--"))
        })
        .map(str::to_ascii_lowercase)
}
