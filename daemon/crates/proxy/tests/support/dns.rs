//! Ein aufzeichnender Nameserver für die Tests des Hickory-Adapters (HUM-115).
//!
//! Er lauscht auf UDP und TCP desselben Ports von `127.0.0.1`, schreibt jede
//! Frage mit (Name, Typ, Transport) und antwortet aus einer kleinen Zone: Ein
//! eingetragener Name bekommt seine Adressen der gefragten Familie, und hat er
//! keine solche, ein `NOERROR` ohne Antwort; jeder andere Name bekommt
//! `NXDOMAIN`. Die Einträge tragen eine Frist von 60 Sekunden, damit ein
//! Zwischenspeicher im Adapter auffiele.
//!
//! Die Nachrichten liest und baut hickory-proto: Der Test prüft die Wirkung des
//! Adapters, nicht einen eigenen DNS-Parser.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};

use hickory_resolver::proto::op::{Message, ResponseCode};
use hickory_resolver::proto::rr::rdata::{A, AAAA};
use hickory_resolver::proto::rr::{RData, Record, RecordType};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::task::JoinHandle;

/// Die Frist der Antworten in Sekunden.
const ANSWER_TTL: u32 = 60;

/// Auf welchem Weg eine Frage ankam.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    /// Ein UDP-Datagramm.
    Udp,
    /// Eine TCP-Verbindung mit Längenpräfix.
    Tcp,
}

/// Eine Frage, wie der Stub sie gesehen hat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    /// Der gefragte Name, klein und ohne Punkt am Ende.
    pub name: String,
    /// Der gefragte Typ.
    pub kind: RecordType,
    /// Der Weg der Frage.
    pub transport: Transport,
}

impl Asked {
    /// Kurzform für die Erwartung eines Tests.
    pub fn new(name: &str, kind: RecordType, transport: Transport) -> Self {
        Self {
            name: name.to_owned(),
            kind,
            transport,
        }
    }
}

type Zone = HashMap<String, Vec<IpAddr>>;

/// Baut einen [`DnsStub`].
#[derive(Default)]
pub struct DnsStubBuilder {
    zone: Zone,
    truncate_udp: bool,
}

impl DnsStubBuilder {
    /// `name` gibt es, mit diesen Adressen.
    pub fn answer(mut self, name: &str, addrs: Vec<IpAddr>) -> Self {
        self.zone.insert(name.to_ascii_lowercase(), addrs);
        self
    }

    /// Jede Antwort über UDP kommt gekürzt (TC-Bit, ohne Inhalt), damit der
    /// Client auf TCP ausweicht.
    pub fn truncate_udp(mut self) -> Self {
        self.truncate_udp = true;
        self
    }

    /// Startet den Stub auf einem freien Port von `127.0.0.1`, UDP und TCP.
    pub async fn start(self) -> DnsStub {
        let (udp, tcp) = bind_pair().await;
        let addr = udp.local_addr().unwrap();
        let asked = Arc::new(Mutex::new(Vec::new()));
        let zone = Arc::new(self.zone);
        let truncate_udp = self.truncate_udp;

        let udp_task = {
            let asked = Arc::clone(&asked);
            let zone = Arc::clone(&zone);
            tokio::spawn(async move {
                let mut buf = vec![0_u8; 4096];
                loop {
                    let Ok((len, peer)) = udp.recv_from(&mut buf).await else {
                        return;
                    };
                    let reply = respond(&buf[..len], &zone, Transport::Udp, truncate_udp, &asked);
                    if let Some(reply) = reply {
                        let _ = udp.send_to(&reply, peer).await;
                    }
                }
            })
        };
        let tcp_task = {
            let asked = Arc::clone(&asked);
            tokio::spawn(async move {
                loop {
                    let Ok((stream, _peer)) = tcp.accept().await else {
                        return;
                    };
                    tokio::spawn(serve_tcp(stream, Arc::clone(&zone), Arc::clone(&asked)));
                }
            })
        };

        DnsStub {
            addr,
            asked,
            tasks: vec![udp_task, tcp_task],
        }
    }
}

/// Ein Nameserver auf `127.0.0.1`, der jede Frage mitschreibt.
pub struct DnsStub {
    addr: SocketAddr,
    asked: Arc<Mutex<Vec<Asked>>>,
    tasks: Vec<JoinHandle<()>>,
}

impl DnsStub {
    /// Ein Stub mit leerer Zone: Er antwortet auf alles `NXDOMAIN`.
    pub fn builder() -> DnsStubBuilder {
        DnsStubBuilder::default()
    }

    /// Adresse und Port, auf denen er lauscht (UDP und TCP).
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Alle Fragen bisher, in der Reihenfolge ihres Eintreffens.
    pub fn asked(&self) -> Vec<Asked> {
        self.asked.lock().unwrap().clone()
    }
}

impl Drop for DnsStub {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

/// Ein UDP-Socket und ein TCP-Listener auf demselben freien Port.
///
/// Der Port kommt vom UDP-Socket; ist er für TCP schon vergeben, versucht es
/// der nächste freie.
async fn bind_pair() -> (UdpSocket, TcpListener) {
    for _ in 0..32 {
        let udp = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let port = udp.local_addr().unwrap().port();
        if let Ok(tcp) = TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await {
            return (udp, tcp);
        }
    }
    panic!("no port on 127.0.0.1 was free for UDP and TCP at once");
}

/// Beantwortet Fragen auf einer TCP-Verbindung, jede mit Längenpräfix.
async fn serve_tcp(mut stream: TcpStream, zone: Arc<Zone>, asked: Arc<Mutex<Vec<Asked>>>) {
    loop {
        let Ok(len) = stream.read_u16().await else {
            return;
        };
        let mut request = vec![0_u8; usize::from(len)];
        if stream.read_exact(&mut request).await.is_err() {
            return;
        }
        let Some(reply) = respond(&request, &zone, Transport::Tcp, false, &asked) else {
            return;
        };
        let Ok(reply_len) = u16::try_from(reply.len()) else {
            return;
        };
        if stream.write_u16(reply_len).await.is_err() || stream.write_all(&reply).await.is_err() {
            return;
        }
    }
}

/// Schreibt die Frage mit und baut die Antwort; `None` für Unlesbares.
fn respond(
    request: &[u8],
    zone: &Zone,
    transport: Transport,
    truncate: bool,
    asked: &Mutex<Vec<Asked>>,
) -> Option<Vec<u8>> {
    let request = Message::from_vec(request).ok()?;
    let query = request.queries.first()?.clone();
    let name = query
        .name()
        .to_ascii()
        .trim_end_matches('.')
        .to_ascii_lowercase();
    asked.lock().unwrap().push(Asked {
        name: name.clone(),
        kind: query.query_type(),
        transport,
    });

    let mut response = Message::response(request.metadata.id, request.metadata.op_code);
    response.metadata.recursion_desired = request.metadata.recursion_desired;
    response.metadata.recursion_available = true;
    response.add_query(query.clone());
    if truncate {
        response.metadata.truncation = true;
        return response.to_vec().ok();
    }
    match zone.get(&name) {
        None => response.metadata.response_code = ResponseCode::NXDomain,
        Some(addrs) => {
            for ip in addrs {
                let rdata = match (ip, query.query_type()) {
                    (IpAddr::V4(v4), RecordType::A) => RData::A(A(*v4)),
                    (IpAddr::V6(v6), RecordType::AAAA) => RData::AAAA(AAAA(*v6)),
                    _ => continue,
                };
                response.add_answer(Record::from_rdata(query.name().clone(), ANSWER_TTL, rdata));
            }
        }
    }
    response.to_vec().ok()
}
