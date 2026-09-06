//! Die Suche nach LLM-Servern im eigenen Netz (HUM-076).
//!
//! Prinzip 9 sagt, dass ein Mensch keine IP kennen muss. Ein Netzscan ist aber
//! ein Vorgang vom Host aus, den niemand hinter dem Rücken des Nutzers
//! auslösen darf: Er läuft nur auf ausdrücklichen Wunsch, er bleibt im eigenen
//! `/24`, und er fasst genau vier Ports an. Was er tut, steht vorher im Text
//! über dem Knopf, und was er gefunden hat, steht danach als Liste da — nie
//! als geratene Zahl.
//!
//! Die Suche besteht aus zwei Schritten, und der zweite ist der teure: Erst
//! ein Verbindungsversuch je Adresse und Port mit kurzer Frist
//! ([`CONNECT_TIMEOUT`]), dann für jeden offenen Port dieselbe Probe, die auch
//! der Testknopf benutzt ([`crate::LlmProbe`], HUM-039). Damit kann die Suche
//! nichts behaupten, was die Probe nicht misst: Sie erkennt Ollama vor der
//! OpenAI-kompatiblen Oberfläche, sie erfindet keine Modellliste, und ein
//! Server, der `401` sagt, verschwindet nicht, sondern erscheint als
//! „braucht Zugangsdaten".
//!
//! Kein mDNS: Ollama kündigt sich nicht an. Keine Suche außerhalb des eigenen
//! `/24`: Ein `/16` wären 65 534 Verbindungsversuche in ein Netz, das dem
//! Nutzer vielleicht gar nicht gehört, und genau so sieht ein Portscan für
//! jedes IDS aus.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use humanitl_core::diagnostics::codes;
use humanitl_core::{Authority, Diagnostic, HostName, Severity};
use tokio::sync::{Semaphore, mpsc};
use url::Url;

use crate::egress::Egress;
use crate::llm_probe::{LlmFlavor, LlmProbe};

/// Die Ports, die ohne eigene Angabe gefragt werden.
///
/// `11434` ist Ollama, `1234` LM Studio, `8000` vLLM und die meisten
/// Python-Server, `8080` llama.cpp und llamafile. Mehr sind es mit Absicht
/// nicht: Jeder weitere Port vervierfacht die Zahl der Verbindungsversuche
/// und macht den Scan für ein IDS auffälliger, ohne die Trefferquote
/// nennenswert zu heben.
pub const DEFAULT_PORTS: [u16; 4] = [11434, 1234, 8000, 8080];

/// Wie lange ein einzelner Verbindungsversuch dauern darf.
///
/// Im eigenen Netz antwortet ein offener Port in Millisekunden; ein
/// geschlossener antwortet sofort mit `RST`. Die Frist trifft nur Adressen, an
/// denen niemand ist und deren Pakete verschluckt werden, und sie ist die
/// Zahl, aus der sich die Gesamtdauer ergibt.
pub const CONNECT_TIMEOUT: Duration = Duration::from_millis(200);

/// Wie lange die Probe eines offenen Ports dauern darf.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// Wie viele Verbindungsversuche gleichzeitig laufen.
///
/// 254 Adressen mal vier Ports sind 1016 Versuche. Mit 64 gleichzeitig und
/// 200 ms Frist bleibt der schlimmste Fall — kein einziger antwortet — unter
/// vier Sekunden, und die Zahl offener Dateideskriptoren bleibt weit unter
/// jedem Limit.
pub const PARALLEL: usize = 64;

/// Wie viele Ports eine Anfrage höchstens nennen darf.
///
/// Vier, wie die Vorgabe. Die Zusage über dem Knopf nennt vier Ports, und die
/// Dauer des Scans hängt an ihrer Zahl: Mit acht Ports wären es 2032 Versuche
/// und die Zusage „unter fünf Sekunden" wäre keine mehr.
pub const MAX_PORTS: usize = DEFAULT_PORTS.len();

/// Die Routing-Tabelle des Kernels.
const ROUTE_TABLE: &str = "/proc/net/route";

/// Ein Netz, in dem gesucht wird.
///
/// Immer ein `/24` oder enger. Der Typ hält die Grenze fest, damit sie nicht
/// an jeder Aufrufstelle neu geprüft werden muss.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Subnet {
    network: Ipv4Addr,
    prefix: u8,
}

impl Subnet {
    /// Das Netz um `address` herum, auf `/24` geschnitten.
    #[must_use]
    pub fn local_24(address: Ipv4Addr) -> Self {
        let octets = address.octets();
        Self {
            network: Ipv4Addr::new(octets[0], octets[1], octets[2], 0),
            prefix: 24,
        }
    }

    /// Genau eine Adresse.
    #[must_use]
    pub const fn single(address: Ipv4Addr) -> Self {
        Self {
            network: address,
            prefix: 32,
        }
    }

    /// Liest ein Netz in CIDR-Schreibweise.
    ///
    /// # Errors
    ///
    /// [`Diagnostic`] mit `LLM_008`, wenn der Text kein CIDR ist oder das Netz
    /// weiter als ein `/24` ist.
    pub fn parse(cidr: &str) -> Result<Self, Diagnostic> {
        let (address, prefix) = cidr.split_once('/').ok_or_else(|| {
            refused(format!(
                "{cidr} is not a network in CIDR notation; it needs a prefix, as in 192.168.1.0/24"
            ))
        })?;
        let address: Ipv4Addr = address
            .parse()
            .map_err(|_error| refused(format!("{address} is not an IPv4 address")))?;
        let prefix: u8 = prefix
            .parse()
            .map_err(|_error| refused(format!("{prefix} is not a prefix length")))?;
        if prefix > 32 {
            return Err(refused(format!(
                "/{prefix} is not a prefix of an IPv4 network; the longest is /32"
            )));
        }
        if prefix < 24 {
            // `checked_shr` und nicht `>>`: Ein Schieben um 32 oder mehr Bits
            // ist in Rust ein Panik-Fall, und der Wert käme hier aus der
            // Anfrage eines Clients.
            let addresses = u32::MAX.checked_shr(u32::from(prefix)).unwrap_or(0);
            return Err(refused(format!(
                "/{prefix} is wider than the /24 this search stays in: it would be {addresses} \
                 connection attempts into a network that may not be yours"
            )));
        }
        Ok(Self {
            network: masked(address, prefix),
            prefix,
        })
    }

    /// Die Adressen, an denen ein Server stehen kann.
    ///
    /// Ohne Netz- und Broadcast-Adresse, denn dort steht keiner. Bei `/31` und
    /// `/32` gibt es diese Sonderrolle nicht, und beide Adressen zählen.
    pub fn hosts(&self) -> impl Iterator<Item = Ipv4Addr> + use<> {
        let first = u32::from(self.network);
        let count = 1_u32 << (32 - u32::from(self.prefix));
        // Erst abziehen, dann addieren: `first + count` ist für ein Netz am
        // oberen Ende des Adressraums (255.255.255.0/24) genau 2^32 und läuft
        // über, bevor die 1 oder die 2 wieder abgezogen wäre.
        let (from, to) = if self.prefix >= 31 {
            (first, first + (count - 1))
        } else {
            (first + 1, first + (count - 2))
        };
        (from..=to).map(Ipv4Addr::from)
    }

    /// Ob `other` ganz in diesem Netz liegt.
    ///
    /// Die Frage, die entscheidet, ob eine Suche im eigenen Netz bleibt: Ein
    /// `/24` irgendwo auf der Welt ist genauso breit wie das eigene und
    /// trotzdem fremdes Netz.
    #[must_use]
    pub fn contains(&self, other: Self) -> bool {
        other.prefix >= self.prefix && masked(other.network, self.prefix) == self.network
    }

    /// Wie viele Adressen [`Subnet::hosts`] liefert.
    #[must_use]
    pub fn len(&self) -> u32 {
        let count = 1_u32 << (32 - u32::from(self.prefix));
        if self.prefix >= 31 { count } else { count - 2 }
    }

    /// Ob das Netz keine Adresse enthält. Kann nicht vorkommen; `clippy` will
    /// die Frage neben [`Subnet::len`] sehen.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl std::fmt::Display for Subnet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.network, self.prefix)
    }
}

/// Das eigene Netz, wie es die Routing-Tabelle beschreibt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalNet {
    /// Das `/24` um die eigene Adresse.
    pub subnet: Subnet,
    /// Die eigene Adresse in diesem Netz.
    pub own: Ipv4Addr,
    /// Die Schnittstelle, über die die Vorgaberoute läuft.
    pub interface: String,
}

/// Das eigene `/24`, abgeleitet aus der Vorgaberoute.
///
/// Zwei Schritte, beide ohne ein einziges Paket: Die Vorgaberoute nennt die
/// Schnittstelle und das Gateway, und ein verbundener UDP-Socket auf dieses
/// Gateway nennt die eigene Adresse, die der Kernel dafür wählen würde.
/// `connect` auf einem UDP-Socket schickt nichts; es setzt nur das Ziel.
///
/// # Errors
///
/// [`Diagnostic`] mit `LLM_008`, wenn es keine Vorgaberoute gibt (kein Netz)
/// oder die eigene Adresse nicht zu ermitteln ist.
pub async fn local_net() -> Result<LocalNet, Diagnostic> {
    let table = tokio::fs::read_to_string(ROUTE_TABLE)
        .await
        .map_err(|error| {
            refused(format!(
                "the routing table {ROUTE_TABLE} cannot be read ({error}); without it there is no \
             network to search"
            ))
        })?;
    let (interface, gateway) = default_route(&table).ok_or_else(|| {
        refused(
            "there is no default route on this host, so there is no local network to search"
                .to_owned(),
        )
    })?;
    let own = address_towards(gateway).await?;
    Ok(LocalNet {
        subnet: Subnet::local_24(own),
        own,
        interface,
    })
}

/// Die Schnittstelle und das Gateway der Vorgaberoute aus dem Text von
/// `/proc/net/route`.
///
/// Das Format ist eine Kopfzeile und danach je Route eine Zeile mit
/// Tabulatoren; Ziel, Gateway und Maske stehen als kleinendige Hexzahlen.
/// Vorgaberoute heißt: Ziel `00000000` und Maske `00000000`. Gibt es mehrere,
/// gewinnt die mit der kleinsten Metrik — dieselbe Wahl, die der Kernel
/// trifft.
#[must_use]
pub fn default_route(table: &str) -> Option<(String, Ipv4Addr)> {
    let mut best: Option<(u32, String, Ipv4Addr)> = None;
    for line in table.lines().skip(1) {
        let mut fields = line.split_whitespace();
        let (Some(interface), Some(destination), Some(gateway), Some(flags)) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let flags = u32::from_str_radix(flags, 16).unwrap_or(0);
        let metric = fields.nth(2).and_then(|text| text.parse::<u32>().ok());
        let mask = fields.next().and_then(hex_address);
        // `RTF_UP` (0x1) und `RTF_GATEWAY` (0x2): Eine Route, die nicht steht,
        // zählt nicht, und eine ohne Gateway ist eine Punkt-zu-Punkt-Strecke
        // (WireGuard, OpenVPN). Deren Gateway steht als `00000000` in der
        // Tabelle, und ein UDP-Socket auf `0.0.0.0` landet auf der Schleife —
        // die Suche liefe dann über `127.0.0.0/24` statt über das eigene Netz
        // und fände nichts, ohne es zu sagen.
        if flags & 0x0003 != 0x0003
            || destination != "00000000"
            || mask != Some(Ipv4Addr::UNSPECIFIED)
        {
            continue;
        }
        let Some(gateway) = hex_address(gateway).filter(|address| !address.is_unspecified()) else {
            continue;
        };
        let metric = metric.unwrap_or(u32::MAX);
        if best.as_ref().is_none_or(|(seen, _, _)| metric < *seen) {
            best = Some((metric, interface.to_owned(), gateway));
        }
    }
    best.map(|(_, interface, gateway)| (interface, gateway))
}

/// Die eigene Adresse, die der Kernel für ein Ziel wählen würde.
async fn address_towards(gateway: Ipv4Addr) -> Result<Ipv4Addr, Diagnostic> {
    let socket = tokio::net::UdpSocket::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)))
        .await
        .map_err(|error| refused(format!("no local socket for the route lookup ({error})")))?;
    // `connect` auf UDP schickt nichts; es merkt sich das Ziel und lässt den
    // Kernel die Quelladresse wählen. Genau die ist gesucht.
    socket
        .connect(SocketAddr::from((gateway, 9)))
        .await
        .map_err(|error| {
            refused(format!(
                "the route towards the gateway {gateway} cannot be looked up ({error})"
            ))
        })?;
    match socket.local_addr() {
        Ok(SocketAddr::V4(address)) => Ok(*address.ip()),
        Ok(SocketAddr::V6(address)) => Err(refused(format!(
            "the route towards the gateway {gateway} ends at the IPv6 address {}; this search is \
             IPv4 only",
            address.ip()
        ))),
        Err(error) => Err(refused(format!(
            "the local address towards {gateway} cannot be read ({error})"
        ))),
    }
}

/// Was gesucht werden soll.
#[derive(Debug, Clone)]
pub struct Scan {
    /// Das Netz.
    pub subnet: Subnet,
    /// Die Ports, in der Reihenfolge, in der sie gefragt werden.
    pub ports: Vec<u16>,
    /// Adressen, die zuerst gefragt werden, auch wenn sie nicht im Netz
    /// liegen: die Schleife und die eigene Adresse. Ein Mensch, der Ollama auf
    /// demselben Rechner laufen lässt, soll es als Erstes sehen.
    pub first: Vec<Ipv4Addr>,
}

impl Scan {
    /// Ein Scan über `subnet` mit den Vorgabe-Ports.
    #[must_use]
    pub fn new(subnet: Subnet) -> Self {
        Self {
            subnet,
            ports: DEFAULT_PORTS.to_vec(),
            first: Vec::new(),
        }
    }

    /// Derselbe Scan, aber mit diesen Ports.
    ///
    /// Eine leere Liste ist keine Angabe und lässt die Vorgabe stehen.
    ///
    /// # Errors
    ///
    /// [`Diagnostic`] mit `LLM_008`, wenn mehr als [`MAX_PORTS`] verschiedene
    /// Ports genannt sind oder einer davon `0` ist. Die Zahl der Versuche
    /// hängt an dieser Liste, und sie kommt aus der Anfrage eines Clients.
    pub fn with_ports(mut self, ports: Vec<u16>) -> Result<Self, Diagnostic> {
        if ports.is_empty() {
            return Ok(self);
        }
        if ports.contains(&0) {
            return Err(refused("0 is not a port a server listens on".to_owned()));
        }
        let mut unique: Vec<u16> = Vec::with_capacity(ports.len());
        for port in ports {
            if !unique.contains(&port) {
                unique.push(port);
            }
        }
        if unique.len() > MAX_PORTS {
            return Err(refused(format!(
                "{} ports are more than the {MAX_PORTS} this search asks: every port multiplies \
                 the connection attempts, and the promise of a scan under five seconds hangs on \
                 their number",
                unique.len()
            )));
        }
        self.ports = unique;
        Ok(self)
    }

    /// Derselbe Scan, aber mit diesen Adressen zuerst.
    #[must_use]
    pub fn first(mut self, addresses: Vec<Ipv4Addr>) -> Self {
        self.first = addresses;
        self
    }

    /// Die Adressen in der Reihenfolge, in der sie gefragt werden.
    fn addresses(&self) -> Vec<Ipv4Addr> {
        let mut seen: Vec<Ipv4Addr> =
            Vec::with_capacity(self.first.len() + self.subnet.len() as usize);
        for address in self.first.iter().copied().chain(self.subnet.hosts()) {
            if !seen.contains(&address) {
                seen.push(address);
            }
        }
        seen
    }
}

/// Ein Server, der geantwortet hat.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    /// Die Adresse.
    pub host: Ipv4Addr,
    /// Der Port.
    pub port: u16,
    /// Was der Server ist, gemessen von der Probe.
    pub flavor: LlmFlavor,
    /// Die Modelle, die er nennt. Leer, wenn er Zugangsdaten verlangt.
    pub models: Vec<String>,
    /// Wie lange die Probe gedauert hat.
    pub latency_ms: u32,
    /// Der Endpunkt hat mit `401` oder `403` geantwortet.
    pub auth_required: bool,
}

/// Sucht und schickt jeden Treffer, sobald er feststeht.
///
/// Kehrt zurück, wenn alle Adressen gefragt sind oder der Empfänger nicht mehr
/// zuhört — wer das Blatt schließt, beendet damit den Scan.
pub async fn discover(
    probe: Arc<LlmProbe>,
    egress: Arc<dyn Egress>,
    scan: &Scan,
    found: mpsc::Sender<Found>,
) {
    let permits = Arc::new(Semaphore::new(PARALLEL));
    let mut running = tokio::task::JoinSet::new();
    'sweep: for address in scan.addresses() {
        for port in scan.ports.iter().copied() {
            // Wer das Blatt schließt, lässt den Empfänger fallen. Dann wird
            // nichts mehr angefangen **und** nichts mehr zu Ende geführt: Die
            // laufenden Versuche werden abgebrochen, statt noch bis zu zwei
            // Sekunden weiterzufragen. Ohne `abort_all` wäre die Zusage „das
            // beendet den Scan" nur ungefähr wahr.
            if found.is_closed() {
                running.abort_all();
                break 'sweep;
            }
            let Ok(permit) = Arc::clone(&permits).acquire_owned().await else {
                return;
            };
            let probe = Arc::clone(&probe);
            let egress = Arc::clone(&egress);
            let found = found.clone();
            running.spawn(async move {
                let _permit = permit;
                if let Some(result) = ask(&probe, egress.as_ref(), address, port).await {
                    // Ein geschlossener Kanal heißt: Das Blatt ist zu. Der
                    // Fehler wird verworfen, die Schleife oben sieht ihn beim
                    // nächsten Durchgang.
                    let _ = found.send(result).await;
                }
            });
        }
    }
    while running.join_next().await.is_some() {}
}

/// Ein Verbindungsversuch und, wenn er trägt, die Probe.
async fn ask(probe: &LlmProbe, egress: &dyn Egress, address: Ipv4Addr, port: u16) -> Option<Found> {
    let authority = Authority::new(HostName::Ip(IpAddr::V4(address)), port);
    let started = Instant::now();
    let stream = egress
        .connect(&authority, Some(IpAddr::V4(address)))
        .await
        .ok()?;
    // Der Versuch hat nur gefragt, ob jemand da ist. Die Probe baut ihre eigene
    // Verbindung auf; diese hier wird sofort wieder zugemacht, damit ein
    // Server nicht 1016 offene Verbindungen zählt.
    drop(stream);

    let endpoint = Url::parse(&format!("http://{address}:{port}")).ok()?;
    match probe.probe(&endpoint, Some(PROBE_TIMEOUT)).await {
        Ok(result) => Some(Found {
            host: address,
            port,
            flavor: result.flavor,
            models: result.models,
            latency_ms: result.latency_ms,
            auth_required: false,
        }),
        // Ein Server, der Zugangsdaten verlangt, ist ein Server. Er
        // verschwindet nicht aus der Liste, er trägt einen Vermerk — und
        // gerade **keine** Geschmacksrichtung: Die Probe fragt zuerst
        // `/api/tags`, und ein `401` von dort sagt nichts darüber, ob dahinter
        // Ollama, eine OpenAI-kompatible API oder ein beliebiger
        // passwortgeschützter Webserver steht. Die Spezifikation schlug
        // `openai_compatible (auth required)` vor; das wäre eine Behauptung
        // über etwas, das niemand gemessen hat (Review Codex, 2026-09-06).
        Err(diagnostic) if diagnostic.code.as_str() == codes::LLM_002.as_str() => Some(Found {
            host: address,
            port,
            flavor: LlmFlavor::Unknown,
            models: Vec::new(),
            latency_ms: millis(started.elapsed()),
            auth_required: true,
        }),
        Err(_silent) => None,
    }
}

/// Millisekunden als Zahl, gedeckelt.
fn millis(elapsed: Duration) -> u32 {
    u32::try_from(elapsed.as_millis()).unwrap_or(u32::MAX)
}

/// Die Adresse mit den Bits unterhalb der Präfixlänge auf null.
fn masked(address: Ipv4Addr, prefix: u8) -> Ipv4Addr {
    if prefix == 0 {
        return Ipv4Addr::UNSPECIFIED;
    }
    let mask = u32::MAX << (32 - u32::from(prefix));
    Ipv4Addr::from(u32::from(address) & mask)
}

/// Eine Adresse aus dem kleinendigen Hex-Format von `/proc/net/route`.
fn hex_address(text: &str) -> Option<Ipv4Addr> {
    let raw = u32::from_str_radix(text, 16).ok()?;
    Some(Ipv4Addr::from(raw.to_be()))
}

/// Der Befund für eine Suche, die nicht stattfinden kann.
fn refused(why: String) -> Diagnostic {
    Diagnostic::builder(codes::LLM_008, Severity::Error)
        .why(why)
        .build()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::{Subnet, default_route, hex_address, masked};
    use std::net::Ipv4Addr;

    /// Der Text stammt von dieser Maschine, gekürzt auf drei Zeilen.
    const ROUTE: &str = "Iface\tDestination\tGateway \tFlags\tRefCnt\tUse\tMetric\tMask\t\tMTU\tWindow\tIRTT\n\
        wlan0\t00000000\t0102A8C0\t0003\t0\t0\t600\t00000000\t0\t0\t0\n\
        wlan0\t0002A8C0\t00000000\t0001\t0\t0\t600\t00FFFFFF\t0\t0\t0\n";

    #[test]
    fn the_default_route_names_the_interface_and_the_gateway() {
        let (interface, gateway) = default_route(ROUTE).expect("a default route");
        assert_eq!(interface, "wlan0");
        assert_eq!(gateway, Ipv4Addr::new(192, 168, 2, 1));
    }

    #[test]
    fn a_route_table_without_a_default_route_has_none() {
        let only_link = "Iface\tDestination\tGateway \tFlags\tRefCnt\tUse\tMetric\tMask\n\
            wlan0\t0002A8C0\t00000000\t0001\t0\t0\t600\t00FFFFFF\n";
        assert!(default_route(only_link).is_none());
        assert!(default_route("").is_none());
        assert!(default_route("Iface\tDestination\n").is_none());
    }

    /// Zwei Vorgaberouten, und die mit der kleineren Metrik gewinnt — dieselbe
    /// Wahl wie die des Kernels. Ohne diese Zusicherung führe der Scan bei
    /// einem zweiten, teureren Weg (etwa einem VPN im Ruhezustand) in dessen
    /// Netz statt in das eigene.
    #[test]
    fn the_cheaper_default_route_wins() {
        let two = "Iface\tDestination\tGateway \tFlags\tRefCnt\tUse\tMetric\tMask\n\
            tun0\t00000000\t0101010A\t0003\t0\t0\t1000\t00000000\n\
            wlan0\t00000000\t0102A8C0\t0003\t0\t0\t600\t00000000\n";
        let (interface, gateway) = default_route(two).expect("a default route");
        assert_eq!(interface, "wlan0");
        assert_eq!(gateway, Ipv4Addr::new(192, 168, 2, 1));
    }

    #[test]
    fn an_address_from_the_route_table_is_little_endian() {
        assert_eq!(hex_address("0102A8C0"), Some(Ipv4Addr::new(192, 168, 2, 1)));
        assert_eq!(hex_address("00000000"), Some(Ipv4Addr::UNSPECIFIED));
        assert_eq!(hex_address("nonsense"), None);
    }

    #[test]
    fn the_local_network_is_the_24_around_the_own_address() {
        let subnet = Subnet::local_24(Ipv4Addr::new(192, 168, 2, 37));
        assert_eq!(subnet.to_string(), "192.168.2.0/24");
        assert_eq!(subnet.len(), 254);
        let hosts: Vec<Ipv4Addr> = subnet.hosts().collect();
        assert_eq!(hosts.len(), 254);
        assert_eq!(hosts[0], Ipv4Addr::new(192, 168, 2, 1));
        assert_eq!(hosts[253], Ipv4Addr::new(192, 168, 2, 254));
        // Netz- und Broadcast-Adresse stehen nicht darin: Dort ist kein Server,
        // und der Broadcast wäre 254 Antworten auf eine Frage.
        assert!(!hosts.contains(&Ipv4Addr::new(192, 168, 2, 0)));
        assert!(!hosts.contains(&Ipv4Addr::new(192, 168, 2, 255)));
    }

    #[test]
    fn a_single_address_is_a_network_of_one() {
        let subnet = Subnet::single(Ipv4Addr::LOCALHOST);
        assert_eq!(subnet.len(), 1);
        assert_eq!(
            subnet.hosts().collect::<Vec<_>>(),
            vec![Ipv4Addr::LOCALHOST]
        );
    }

    /// Die Grenze ist der Punkt des ganzen Typs: Ein `/16` wäre ein Portscan
    /// über 65 534 Adressen, und der Text über dem Knopf verspricht das
    /// Gegenteil.
    #[test]
    fn a_network_wider_than_a_24_is_refused() {
        for cidr in ["192.168.0.0/16", "10.0.0.0/8", "0.0.0.0/0"] {
            let refusal = Subnet::parse(cidr).expect_err("wider than a /24");
            assert_eq!(refusal.code.as_str(), "LLM_008");
            assert!(
                refusal.why.contains("wider than the /24"),
                "the refusal must say why: {}",
                refusal.why
            );
        }
        assert!(Subnet::parse("192.168.2.0/24").is_ok());
        assert!(Subnet::parse("192.168.2.128/25").is_ok());
        assert!(Subnet::parse("192.168.2.7/32").is_ok());
    }

    /// Ein Präfix jenseits von 32 ist kein Netz — und es darf nicht in einer
    /// Panik enden. Die Zahl kommt aus der Anfrage eines Clients, und
    /// `u32::MAX >> 33` ist in Rust ein Absturz, kein großer Wert. Im Review
    /// von Antigravity ist genau das aufgefallen.
    #[test]
    fn a_prefix_beyond_32_is_refused_without_a_panic() {
        for cidr in ["192.168.2.0/33", "192.168.2.0/64", "192.168.2.0/255"] {
            let refusal = Subnet::parse(cidr).expect_err("not a prefix");
            assert_eq!(refusal.code.as_str(), "LLM_008");
            assert!(
                refusal.why.contains("the longest is /32"),
                "the refusal must say what a prefix can be: {}",
                refusal.why
            );
        }
    }

    /// Das Netz am oberen Ende des Adressraums. `first + count` ist dort genau
    /// 2^32; wer zuerst addiert und dann abzieht, läuft über.
    #[test]
    fn the_last_network_of_the_address_space_has_its_hosts() {
        let subnet = Subnet::parse("255.255.255.0/24").expect("a /24");
        let hosts: Vec<Ipv4Addr> = subnet.hosts().collect();
        assert_eq!(hosts.len(), 254);
        assert_eq!(hosts[0], Ipv4Addr::new(255, 255, 255, 1));
        assert_eq!(hosts[253], Ipv4Addr::new(255, 255, 255, 254));

        let single = Subnet::single(Ipv4Addr::BROADCAST);
        assert_eq!(
            single.hosts().collect::<Vec<_>>(),
            vec![Ipv4Addr::BROADCAST]
        );
    }

    /// Eine Vorgaberoute ohne Gateway ist eine Punkt-zu-Punkt-Strecke: ein
    /// VPN. Sie wird übergangen, denn ein UDP-Socket auf `0.0.0.0` landet auf
    /// der Schleife, und die Suche liefe still über `127.0.0.0/24`.
    #[test]
    fn a_default_route_without_a_gateway_is_no_default_route() {
        let vpn = "Iface\tDestination\tGateway \tFlags\tRefCnt\tUse\tMetric\tMask\n\
            wg0\t00000000\t00000000\t0001\t0\t0\t0\t00000000\n";
        assert!(default_route(vpn).is_none());

        // Dieselbe Tabelle, aber daneben ein echter Weg: Der gewinnt, obwohl
        // seine Metrik höher ist — die andere Zeile ist gar keine Wahl.
        let both = "Iface\tDestination\tGateway \tFlags\tRefCnt\tUse\tMetric\tMask\n\
            wg0\t00000000\t00000000\t0001\t0\t0\t0\t00000000\n\
            wlan0\t00000000\t0102A8C0\t0003\t0\t0\t600\t00000000\n";
        let (interface, gateway) = default_route(both).expect("the real default route");
        assert_eq!(interface, "wlan0");
        assert_eq!(gateway, Ipv4Addr::new(192, 168, 2, 1));
    }

    /// Eine Route, die nicht steht (`RTF_UP` fehlt), zählt nicht.
    #[test]
    fn a_route_that_is_down_is_not_taken() {
        let down = "Iface\tDestination\tGateway \tFlags\tRefCnt\tUse\tMetric\tMask\n\
            eth0\t00000000\t0102A8C0\t0002\t0\t0\t100\t00000000\n";
        assert!(default_route(down).is_none());
    }

    #[test]
    fn a_network_that_is_not_a_network_is_refused() {
        for cidr in ["192.168.2.0", "nonsense/24", "192.168.2.0/nonsense", ""] {
            assert!(Subnet::parse(cidr).is_err(), "{cidr} is not a network");
        }
    }

    #[test]
    fn a_network_keeps_only_the_bits_above_the_prefix() {
        assert_eq!(
            masked(Ipv4Addr::new(192, 168, 2, 37), 24),
            Ipv4Addr::new(192, 168, 2, 0)
        );
        assert_eq!(
            Subnet::parse("192.168.2.37/24").unwrap().to_string(),
            "192.168.2.0/24"
        );
    }

    /// Ein `/24` ist so breit wie das eigene und trotzdem fremdes Netz. Die
    /// Frage, die das entscheidet, gehört zum Typ und nicht an die
    /// Aufrufstelle.
    #[test]
    fn a_network_contains_only_what_lies_inside_it() {
        let own = Subnet::parse("192.168.2.0/24").unwrap();
        assert!(own.contains(own));
        assert!(own.contains(Subnet::parse("192.168.2.128/25").unwrap()));
        assert!(own.contains(Subnet::single(Ipv4Addr::new(192, 168, 2, 37))));
        assert!(!own.contains(Subnet::parse("8.8.8.0/24").unwrap()));
        assert!(!own.contains(Subnet::parse("192.168.3.0/24").unwrap()));
        assert!(!own.contains(Subnet::single(Ipv4Addr::LOCALHOST)));
    }

    /// Die Reihenfolge ist eine Zusage an den Menschen: Was auf demselben
    /// Rechner läuft, steht oben, und keine Adresse wird zweimal gefragt.
    #[test]
    fn the_own_addresses_come_first_and_only_once() {
        let scan = super::Scan::new(Subnet::parse("192.168.2.0/24").unwrap())
            .first(vec![Ipv4Addr::LOCALHOST, Ipv4Addr::new(192, 168, 2, 37)]);
        let addresses = scan.addresses();
        assert_eq!(addresses[0], Ipv4Addr::LOCALHOST);
        assert_eq!(addresses[1], Ipv4Addr::new(192, 168, 2, 37));
        assert_eq!(addresses.len(), 255, "254 hosts plus the loopback");
        assert_eq!(
            addresses
                .iter()
                .filter(|address| **address == Ipv4Addr::new(192, 168, 2, 37))
                .count(),
            1,
            "the own address is asked once, not twice"
        );
    }

    #[test]
    fn the_default_ports_are_the_four_of_the_specification() {
        assert_eq!(super::DEFAULT_PORTS, [11434, 1234, 8000, 8080]);
        let scan = super::Scan::new(Subnet::single(Ipv4Addr::LOCALHOST));
        assert_eq!(scan.ports, vec![11434, 1234, 8000, 8080]);
        // Eine leere Liste ist keine Angabe, keine Anweisung, nichts zu fragen.
        assert_eq!(
            scan.clone()
                .with_ports(Vec::new())
                .expect("no ports named")
                .ports
                .len(),
            4
        );
        assert_eq!(
            scan.clone().with_ports(vec![1234]).expect("one port").ports,
            vec![1234]
        );
        // Doppelte zählen einmal; das ist der Unterschied zwischen „vier
        // Ports" und „vier Einträgen".
        assert_eq!(
            scan.clone()
                .with_ports(vec![1234, 1234, 1234, 1234, 1234])
                .expect("one port, five times")
                .ports,
            vec![1234]
        );
        let too_many = scan
            .clone()
            .with_ports(vec![1, 2, 3, 4, 5])
            .expect_err("five ports are more than four");
        assert_eq!(too_many.code.as_str(), "LLM_008");
        let zero = scan.with_ports(vec![0]).expect_err("0 is not a port");
        assert_eq!(zero.code.as_str(), "LLM_008");
    }
}
