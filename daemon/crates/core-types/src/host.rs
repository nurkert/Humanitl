//! Hostnamen: Normalisierung, Anzeige, Einordnung als private Adresse.
//!
//! Ein [`HostName`] ist entweder ein normalisierter DNS-Name (A-Label,
//! Kleinbuchstaben, ohne abschließenden Punkt) oder eine IP-Adresse. Nur diese
//! beiden Formen existieren; roher Text aus einer Anfrage wird über
//! [`HostName::parse`] geprüft, bevor er irgendwo verglichen oder gespeichert
//! wird. Damit kann kein Vergleich an Groß-/Kleinschreibung, an einem
//! abschließenden Punkt oder an einer Unicode-Schreibweise vorbeigehen.

use core::fmt;
use core::str::FromStr;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Der reservierte Name, den der Proxy selbst beantwortet (ADR-014, HUM-073).
///
/// Er wird nie aufgelöst und nie an einen Upstream weitergereicht. Er steht
/// hier und nicht im Proxy, weil zwei Stellen ihn brauchen, die einander nicht
/// kennen dürfen: die Weiche des Proxys und der Nachweis
/// [`MetaAnswer`](crate::flow::MetaAnswer), mit dem ein Flow ohne Entscheidung
/// aufgezeichnet wird. Zwei Kopien derselben Zeichenkette liefen auseinander,
/// und dann hinge an der einen ein Endpunkt und an der anderen ein Weg am
/// Menschen vorbei.
pub const META_HOST: &str = "humanitl.internal";

/// Ein Text ließ sich nicht als Host lesen.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid host {input:?}: {reason}")]
pub struct HostParseError {
    /// Der abgelehnte Text, unverändert.
    pub input: String,
    /// Warum der Text abgelehnt wurde.
    pub reason: &'static str,
}

impl HostParseError {
    fn new(input: &str, reason: &'static str) -> Self {
        Self {
            input: input.to_owned(),
            reason,
        }
    }
}

/// Ziel-Host einer Anfrage, normalisiert.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HostName {
    /// Ein DNS-Name in A-Label-Form, klein geschrieben, ohne Punkt am Ende.
    Dns(String),
    /// Eine IP-Adresse; entstanden aus einem IP-Literal in der Authority.
    Ip(IpAddr),
}

impl HostName {
    /// Normalisiert einen Host aus einer Anfrage.
    ///
    /// Regeln:
    /// - `[::1]` und `::1` werden zu [`HostName::Ip`], ebenso `1.2.3.4`.
    /// - Ein abschließender Punkt wird entfernt.
    /// - Der Rest läuft durch `idna::domain_to_ascii_strict`: Unicode wird zu
    ///   A-Labels, Großbuchstaben werden klein, `STD3`-Regeln und die
    ///   DNS-Längen werden geprüft.
    /// - Endet der Name auf einem Zahl-Label, muss er eine kanonische
    ///   IPv4-Adresse sein. Damit sind `0x7f.1` und `0177.0.0.1` Fehler statt
    ///   DNS-Namen. Das folgt der Host-Grammatik des URL-Standards und
    ///   verhindert, dass zwei Schreibweisen derselben Adresse an einer Regel
    ///   unterschiedlich vorbeikommen.
    ///
    /// # Errors
    ///
    /// [`HostParseError`] mit kurzem Grund, wenn der Text keine der beiden
    /// Formen ergibt.
    pub fn parse(input: &str) -> Result<Self, HostParseError> {
        if input.is_empty() {
            return Err(HostParseError::new(input, "empty host"));
        }

        if let Some(inner) = input.strip_prefix('[') {
            let inner = inner
                .strip_suffix(']')
                .ok_or_else(|| HostParseError::new(input, "unclosed ipv6 literal"))?;
            return Ipv6Addr::from_str(inner)
                .map(|ip| Self::Ip(IpAddr::V6(ip)))
                .map_err(|_| HostParseError::new(input, "not an ipv6 address"));
        }

        if input.contains(':') {
            return Ipv6Addr::from_str(input)
                .map(|ip| Self::Ip(IpAddr::V6(ip)))
                .map_err(|_| HostParseError::new(input, "not an ipv6 address"));
        }

        let trimmed = input.strip_suffix('.').unwrap_or(input);
        if trimmed.is_empty() {
            return Err(HostParseError::new(input, "empty host"));
        }

        if ends_in_number(trimmed) {
            return Ipv4Addr::from_str(trimmed)
                .map(|ip| Self::Ip(IpAddr::V4(ip)))
                .map_err(|_| HostParseError::new(input, "not a canonical ipv4 address"));
        }

        let ascii = idna::domain_to_ascii_strict(trimmed)
            .map_err(|_| HostParseError::new(input, "not a valid domain name"))?;
        if ascii.is_empty() {
            return Err(HostParseError::new(input, "empty host"));
        }
        Ok(Self::Dns(ascii))
    }

    /// Die Labels eines DNS-Namens, von links nach rechts. `None` bei einer IP.
    #[must_use]
    pub fn labels(&self) -> Option<Vec<&str>> {
        match self {
            Self::Dns(name) => Some(name.split('.').collect()),
            Self::Ip(_) => None,
        }
    }

    /// Form für die Oberfläche: U-Label (`münchen.de` statt `xn--mnchen-3ya.de`),
    /// bei einer IP die Adresse selbst.
    ///
    /// Nur für die Anzeige. Verglichen und gespeichert wird immer die Form aus
    /// [`fmt::Display`].
    #[must_use]
    pub fn display(&self) -> String {
        match self {
            Self::Dns(name) => idna::domain_to_unicode(name).0,
            Self::Ip(ip) => ip.to_string(),
        }
    }

    /// Wahr, wenn dieser Host der reservierte Meta-Host ist ([`META_HOST`]).
    ///
    /// Verglichen wird der normalisierte Name, nie der Text der Anfrage:
    /// [`HostName::parse`] hat aus `HUMANITL.INTERNAL` und
    /// `humanitl.internal.` vorher denselben Namen gemacht, und beide gehören
    /// hierher. Ein Name, der nur so *aussieht*, gehört nicht hierher:
    /// `evil-humanitl.internal`, `sub.humanitl.internal` und
    /// `humanitl.internal.evil.io` sind eigene Namen. Eine IP ist es nie.
    #[must_use]
    pub fn is_meta(&self) -> bool {
        matches!(self, Self::Dns(name) if name == META_HOST)
    }

    /// Wahr, wenn der Host eine Adresse aus einem privaten oder lokalen Bereich ist.
    ///
    /// Deckt RFC 1918, Loopback, Link-Local (also auch `169.254.169.254`),
    /// CGNAT (`100.64.0.0/10`), `0.0.0.0/8`, `IPv6` Unique-Local und
    /// IPv6-Link-Local ab; IPv4-mapped (`::ffff:a.b.c.d`) und IPv4-compatible
    /// (`::a.b.c.d`) IPv6-Adressen werden vorher entpackt.
    ///
    /// Für einen DNS-Namen ist das Ergebnis immer `false`: ob er auf eine
    /// private Adresse zeigt, weiß erst die Auflösung. Der Proxy prüft die
    /// aufgelöste Adresse mit [`ip_is_private`].
    #[must_use]
    pub fn is_private(&self) -> bool {
        match self {
            Self::Dns(_) => false,
            Self::Ip(ip) => ip_is_private(*ip),
        }
    }

    /// Der DNS-Name, falls es einer ist.
    #[must_use]
    pub fn as_dns(&self) -> Option<&str> {
        match self {
            Self::Dns(name) => Some(name.as_str()),
            Self::Ip(_) => None,
        }
    }

    /// Die IP-Adresse, falls es eine ist.
    #[must_use]
    pub fn as_ip(&self) -> Option<IpAddr> {
        match self {
            Self::Dns(_) => None,
            Self::Ip(ip) => Some(*ip),
        }
    }
}

/// Wahr, wenn die Adresse in einem privaten oder lokalen Bereich liegt.
///
/// Der Proxy ruft das nach der Namensauflösung auf: eine Regel ohne
/// `allow_private` darf nicht in das Heimnetz oder auf den Metadaten-Dienst
/// einer Cloud zeigen.
#[must_use]
pub fn ip_is_private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => ipv4_is_private(v4),
        // `to_ipv4` entpackt IPv4-mapped (`::ffff:a.b.c.d`) und die veraltete
        // IPv4-compatible Form (`::a.b.c.d`); `to_ipv4_mapped` ließe die zweite
        // durch. `::` und `::1` landen dabei in `0.0.0.0/8` und bleiben privat.
        IpAddr::V6(v6) => match v6.to_ipv4() {
            Some(v4) => ipv4_is_private(v4),
            None => ipv6_is_private(v6),
        },
    }
}

fn ipv4_is_private(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_broadcast()
        // „Dieses Netz", 0.0.0.0/8, schließt 0.0.0.0 ein.
        || a == 0
        // CGNAT, 100.64.0.0/10.
        || (a == 100 && (64..128).contains(&b))
        // IETF-Protokollzuweisungen, 192.0.0.0/24 (RFC 6890).
        || (a == 192 && b == 0 && c == 0)
        // Benchmark-Netze, 198.18.0.0/15 (RFC 2544).
        || (a == 198 && (b & 0xfe) == 18)
        // Reserviert, 240.0.0.0/4 (RFC 1112).
        || a >= 240
}

fn ipv6_is_private(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    let first = segments[0];
    ip.is_loopback()
        || ip.is_unspecified()
        // Unique Local, fc00::/7.
        || (first & 0xfe00) == 0xfc00
        // Link-Local Unicast, fe80::/10.
        || (first & 0xffc0) == 0xfe80
        // Site-Local, fec0::/10: veraltet (RFC 3879), aber nie öffentlich.
        || (first & 0xffc0) == 0xfec0
        // Teredo, 2001::/32 (RFC 4380), und lokales NAT64, 64:ff9b:1::/48
        // (RFC 8215): Die eingebettete Adresse lässt sich nicht eindeutig
        // lesen, also gilt die Adresse als privat.
        || (first == 0x2001 && segments[1] == 0)
        || (first == 0x0064 && segments[1] == 0xff9b && segments[2] == 0x0001)
        || embedded_ipv4(ip).is_some_and(ipv4_is_private)
}

/// Die IPv4-Adresse, die eine IPv6-Adresse mit festem Muster in sich trägt:
/// NAT64 (`64:ff9b::/96`, RFC 6052), IPv4-translated (`::ffff:0:0/96`,
/// RFC 2765), 6to4 (`2002::/16`, RFC 3056) oder eine ISATAP-Kennung
/// (`…:0:5efe:a.b.c.d` oder `…:200:5efe:a.b.c.d` unter beliebigem Präfix,
/// RFC 5214). IPv4-mapped und IPv4-compatible entpackt schon `to_ipv4`.
///
/// In einem Netz mit DNS64 und NAT64 erreicht `64:ff9b::a9fe:a9fe` über das
/// Gateway die Cloud-Metadaten-Adresse `169.254.169.254`; ohne diese Prüfung
/// wäre der Satz „private Bereiche sind gesperrt" dort falsch. 6rd (RFC 5969)
/// hat kein festes Präfix und lässt sich ohne Netzkonfiguration nicht erkennen.
fn embedded_ipv4(ip: Ipv6Addr) -> Option<Ipv4Addr> {
    let segments = ip.segments();
    let join = |high: u16, low: u16| {
        let [a, b] = high.to_be_bytes();
        let [c, d] = low.to_be_bytes();
        Ipv4Addr::new(a, b, c, d)
    };
    let nat64 = segments[..6] == [0x0064, 0xff9b, 0, 0, 0, 0];
    let translated = segments[..6] == [0, 0, 0, 0, 0xffff, 0];
    let isatap = matches!(segments[4], 0 | 0x0200) && segments[5] == 0x5efe;
    if nat64 || translated || isatap {
        Some(join(segments[6], segments[7]))
    } else if segments[0] == 0x2002 {
        Some(join(segments[1], segments[2]))
    } else {
        None
    }
}

/// Wahr, wenn das letzte Label eine Zahl im Sinne der URL-Host-Grammatik ist.
fn ends_in_number(host: &str) -> bool {
    let last = host.rsplit('.').next().unwrap_or(host);
    if last.is_empty() {
        return false;
    }
    if let Some(hex) = last.strip_prefix("0x").or_else(|| last.strip_prefix("0X")) {
        return hex.is_empty() || hex.chars().all(|c| c.is_ascii_hexdigit());
    }
    last.chars().all(|c| c.is_ascii_digit())
}

impl fmt::Display for HostName {
    /// Die kanonische Form: A-Label beziehungsweise die IP ohne Klammern.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dns(name) => f.write_str(name),
            Self::Ip(ip) => write!(f, "{ip}"),
        }
    }
}

impl FromStr for HostName {
    type Err = HostParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl Serialize for HostName {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for HostName {
    /// Geht durch [`HostName::parse`], damit auch eingelesene Werte normalisiert sind.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(D::Error::custom)
    }
}
