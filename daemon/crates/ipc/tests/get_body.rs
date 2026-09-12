//! `GetBody` mit `decoded` und der `Content-Encoding` im Detail (HUM-119).
//!
//! Der Daemon entpackt, die Oberfläche nicht mehr. Das ist keine Bequemlichkeit,
//! sondern die Bedingung dafür, dass eine Fundstelle stimmt: Die Detektoren
//! suchen in den entpackten Bytes (`humanitl_findings::decode`), und ihre
//! Bereiche zeigen dorthin. Solange die Oberfläche die gepackten Bytes hielt,
//! behielt jeder Fund in einem brotli-Rumpf seinen Namen und verlor seine
//! Stelle.
//!
//! Geprüft wird der ganze Weg über die Aufzeichnung: eine Nachricht mit
//! `Content-Encoding` hinein, `GetFlow` für den Verweis, `GetBody` für die
//! Bytes. Ein Test, der nur `humanitl_ipc::body::deliver` aufriefe, ließe genau
//! die Stellen offen, an denen das Feld verloren gehen kann — den Converter und
//! die Aufzeichnung.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::Write as _;
use std::time::SystemTime;

use bytes::Bytes;
use humanitl_config::Config;
use humanitl_core::http::{HeaderMap, HeaderName, HeaderValue};
use humanitl_core::{
    Authority, FlowEvent, FlowId, HostName, HttpRequest, Method, Scheme, SessionId,
};
use humanitl_ipc::v1::humanitl_server::Humanitl as _;
use humanitl_ipc::{IpcServer, v1};
use humanitl_recorder::{Dir, Recorder, RecorderSettings, SessionMeta};
use tokio_stream::StreamExt as _;
use tonic::Request;

/// Ein Text, in dem ein Fund sitzt, lang genug zum Packen.
const PLAIN: &[u8] =
    b"{\"note\":\"Kontakt vorname.nachname@kunde.de\",\"padding\":\"aaaaaaaaaaaaaaaaaaaaaaaa\"}";

/// Die Mailadresse in [`PLAIN`]; auf sie zeigt der Bereich eines Fundes.
const ADDRESS: &[u8] = b"vorname.nachname@kunde.de";

/// Packt mit gzip.
fn gzip(plain: &[u8]) -> Bytes {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(plain).unwrap();
    Bytes::from(encoder.finish().unwrap())
}

/// Packt mit brotli.
fn brotli(plain: &[u8]) -> Bytes {
    let mut packed = Vec::new();
    {
        let mut encoder = brotli::CompressorWriter::new(&mut packed, 4096, 5, 22);
        encoder.write_all(plain).unwrap();
    }
    Bytes::from(packed)
}

/// Kopfzeilen mit einem `Content-Encoding`, leer für `identity`.
fn headers(encoding: &str) -> HeaderMap {
    let mut map = HeaderMap::new();
    map.insert(
        HeaderName::from_static("content-type"),
        HeaderValue::from_static("application/json"),
    );
    if !encoding.is_empty() {
        map.insert(
            HeaderName::from_static("content-encoding"),
            HeaderValue::from_str(encoding).expect("a header value"),
        );
    }
    map
}

/// Ein Dienst mit Aufzeichnung, in der genau ein Flow steht.
struct Fixture {
    _dir: tempfile::TempDir,
    server: IpcServer,
    recorder: Recorder,
    flow: FlowId,
}

impl Fixture {
    /// Öffnet die Aufzeichnung und legt den Flow an, noch ohne Nachricht.
    fn open() -> Self {
        Self::with_limits(&Config::default())
    }

    /// Wie [`Fixture::open`], mit anderen `limits`.
    fn with_limits(config: &Config) -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let recorder = Recorder::open(
            &dir.path().join("humanitl.db"),
            &dir.path().join("blobs"),
            RecorderSettings::default(),
        )
        .expect("the recording opens");
        let session = SessionId::new();
        recorder.start_session(&SessionMeta {
            id: session,
            started_at: SystemTime::now(),
            sandbox_profile: "default".to_owned(),
            llm_endpoint: None,
            work_dir: "/work".to_owned(),
            agent: "opencode".to_owned(),
        });

        let flow = FlowId::new();
        let host = HostName::parse("api.github.com").expect("a host");
        recorder.apply(&FlowEvent::Received {
            flow_id: flow,
            at: SystemTime::now(),
            request: Box::new(HttpRequest::new(
                Method::POST,
                Scheme::Https,
                Authority::with_scheme(host, Scheme::Https),
                "/gists",
            )),
        });

        // Keine Warteschlange: Diese Tests holen alles aus der Aufzeichnung,
        // und `over_the_recording` hält die Kopplung an `humanitl-proxy` aus
        // dieser Datei heraus (`tools/coupling-baseline.toml`).
        let server =
            IpcServer::over_the_recording(config, Some(session)).with_recorder(recorder.clone());
        Self {
            _dir: dir,
            server,
            recorder,
            flow,
        }
    }

    /// Legt den Anfrage-Rumpf ab und liefert seinen Verweis aus `GetFlow`.
    ///
    /// Der Verweis kommt bewusst aus der RPC und nicht aus `store_message`:
    /// Genau diesen Verweis hat ein Klient in der Hand, und nur wenn er das
    /// `content_encoding` trägt, kann er es zurückreichen.
    async fn request_body(&self, encoding: &str, bytes: Bytes) -> v1::BodyRef {
        self.recorder
            .store_message(self.flow, Dir::Request, &headers(encoding), bytes)
            .await
            .expect("the message is stored");
        self.recorder.flush().await;
        self.detail()
            .await
            .request
            .expect("the detail carries the request")
            .body
            .expect("the request carries a body reference")
    }

    /// Das Detail des Flows, so wie ein Klient es bekommt.
    async fn detail(&self) -> v1::FlowDetail {
        self.server
            .get_flow(Request::new(v1::FlowRef {
                flow_id: self.flow.to_string(),
            }))
            .await
            .expect("the flow is recorded")
            .into_inner()
    }

    /// `GetBody` als ganzer Body plus dem `encoding_left` der Stücke.
    async fn body(&self, reference: v1::BodyRef) -> (Vec<u8>, String) {
        let mut stream = self
            .server
            .get_body(Request::new(reference))
            .await
            .expect("the body is readable")
            .into_inner();
        let mut bytes = Vec::new();
        let mut left: Option<String> = None;
        let mut last_seen = false;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.expect("a chunk arrives");
            assert!(!last_seen, "a chunk followed the one marked last");
            if let Some(before) = left.as_ref() {
                assert_eq!(
                    before, &chunk.encoding_left,
                    "every chunk must say the same about the encoding"
                );
            }
            assert_eq!(
                usize::try_from(chunk.offset).expect("an offset fits a usize"),
                bytes.len(),
                "the offsets must add up"
            );
            left = Some(chunk.encoding_left.clone());
            last_seen = chunk.last;
            bytes.extend_from_slice(&chunk.data);
        }
        assert!(last_seen, "the stream ended without a chunk marked last");
        (bytes, left.unwrap_or_default())
    }
}

#[tokio::test]
async fn detail_carries_the_content_encoding() {
    let fixture = Fixture::open();
    fixture.request_body("gzip", gzip(PLAIN)).await;
    fixture
        .recorder
        .store_message(fixture.flow, Dir::Response, &headers("br"), brotli(PLAIN))
        .await
        .expect("the response is stored");
    fixture.recorder.flush().await;

    let detail = fixture.detail().await;
    assert_eq!(
        detail.request.unwrap().body.unwrap().content_encoding,
        "gzip",
        "the request body must name what lies on the recorded bytes"
    );
    assert_eq!(
        detail.response_body.unwrap().content_encoding,
        "br",
        "the response body has its own encoding, not the one of the request"
    );
}

#[tokio::test]
async fn detail_leaves_the_content_encoding_empty_for_identity() {
    let fixture = Fixture::open();
    let reference = fixture.request_body("", Bytes::from_static(PLAIN)).await;
    assert_eq!(reference.content_encoding, "");
    assert!(
        !reference.decoded,
        "the daemon never asks itself to unpack; only a client sets that flag"
    );
}

#[tokio::test]
async fn get_body_decoded_inflates_gzip() {
    let fixture = Fixture::open();
    let packed = gzip(PLAIN);
    let mut reference = fixture.request_body("gzip", packed.clone()).await;
    assert_eq!(
        usize::try_from(reference.size).expect("a size fits a usize"),
        packed.len()
    );

    reference.decoded = true;
    let (bytes, left) = fixture.body(reference).await;
    assert_eq!(bytes, PLAIN, "the client gets the plain text");
    assert_eq!(left, "", "nothing is left on bytes that were unpacked");
}

#[tokio::test]
async fn get_body_decoded_inflates_brotli() {
    let fixture = Fixture::open();
    let packed = brotli(PLAIN);
    assert_ne!(packed, Bytes::from_static(PLAIN), "the fixture is packed");
    let mut reference = fixture.request_body("br", packed).await;

    reference.decoded = true;
    let (bytes, left) = fixture.body(reference).await;
    assert_eq!(bytes, PLAIN);
    assert_eq!(left, "");
    // Die Stelle, an der es hängt: Ein Bereich der Detektoren zeigt in die
    // entpackten Bytes. Er trifft nur, wenn der Klient genau diese Bytes hält.
    let start = PLAIN
        .windows(ADDRESS.len())
        .position(|window| window == ADDRESS)
        .expect("the plain text carries the address");
    assert_eq!(
        &bytes[start..start + ADDRESS.len()],
        ADDRESS,
        "a span into the unpacked bytes must land on the value the scan found"
    );
}

#[tokio::test]
async fn get_body_decoded_leaves_zstd_alone() {
    let fixture = Fixture::open();
    // Kein echter zstd-Strom: Der Daemon kennt die Kodierung nicht und darf
    // deshalb gar nicht erst hineinsehen. Genau das prüft der Test.
    let stored = Bytes::from_static(b"\x28\xb5\x2f\xfd not really zstd");
    let mut reference = fixture.request_body("zstd", stored.clone()).await;
    assert_eq!(reference.content_encoding, "zstd");

    reference.decoded = true;
    let (bytes, left) = fixture.body(reference).await;
    assert_eq!(bytes, stored, "an encoding we cannot take off stays on");
    assert_eq!(left, "zstd", "and the client is told its name");
}

#[tokio::test]
async fn get_body_decoded_leaves_a_chain_alone() {
    let fixture = Fixture::open();
    // `gzip, br`: zwei Schichten. `decode.rs` kennt genau eine, also bleibt
    // alles liegen und die ganze Kette steht im Namen.
    let stored = gzip(&brotli(PLAIN));
    let mut reference = fixture.request_body("gzip, br", stored.clone()).await;
    assert_eq!(reference.content_encoding, "gzip, br");

    reference.decoded = true;
    let (bytes, left) = fixture.body(reference).await;
    assert_eq!(bytes, stored);
    assert_eq!(left, "gzip, br");
}

#[tokio::test]
async fn get_body_raw_is_unchanged() {
    let fixture = Fixture::open();
    let packed = gzip(PLAIN);
    let reference = fixture.request_body("gzip", packed.clone()).await;

    assert!(!reference.decoded, "a reference from GetFlow asks nothing");
    let (bytes, left) = fixture.body(reference).await;
    assert_eq!(bytes, packed, "without the flag the wire bytes come back");
    // Leer ist hier richtig und bedeutet etwas anderes als sonst: `deliver`
    // gibt ohne `decoded` leer zurück, weil der Klient nicht um das Auspacken
    // gebeten hat und deshalb auch nichts übrig bleiben kann, was ihn
    // überrascht. Die Kodierung liegt trotzdem noch auf den Bytes; was auf
    // ihnen liegt, steht im Verweis. Dass das Feld überhaupt gefüllt wird,
    // hält `get_body_decoded_leaves_zstd_alone` fest, nicht diese Zeile.
    assert_eq!(
        left, "",
        "a raw fetch leaves nothing for the client to undo"
    );
    assert_eq!(
        reference_encoding(&fixture).await,
        "gzip",
        "the encoding is still on these bytes; the reference says so"
    );
}

/// Der `content_encoding` des Anfrage-Rumpfs, wie `GetFlow` ihn liefert.
async fn reference_encoding(fixture: &Fixture) -> String {
    fixture
        .detail()
        .await
        .request
        .expect("the detail carries the request")
        .body
        .expect("the request carries a body reference")
        .content_encoding
}

#[tokio::test]
async fn get_body_decoded_stops_at_the_budget() {
    // Eine Bombe: 4 MiB Nullen, gepackt ein paar Kilobyte. Mit
    // `limits.max_decompress_ratio = 4` darf daraus nur das Vierfache der
    // gepackten Länge entstehen, und der Strom endet trotzdem sauber.
    let config = Config {
        limits: humanitl_config::Limits {
            max_decompress_ratio: 4,
            ..Config::default().limits
        },
        ..Config::default()
    };
    let fixture = Fixture::with_limits(&config);
    let bomb = gzip(&vec![0u8; 4 * 1024 * 1024]);
    let packed_len = bomb.len();
    let mut reference = fixture.request_body("gzip", bomb).await;

    reference.decoded = true;
    let (bytes, left) = fixture.body(reference).await;
    assert_eq!(
        bytes.len(),
        packed_len * 4,
        "the ratio is the budget, and it holds"
    );
    assert!(bytes.iter().all(|byte| *byte == 0), "what came is unpacked");
    assert_eq!(left, "", "these bytes are unpacked, there are just fewer");
}

#[tokio::test]
async fn get_body_decoded_keeps_the_prefix_of_a_truncated_body() {
    // Der Recorder hat nur einen Präfix: `limits.recorder_max_body_bytes` in
    // der Mitte des gepackten Stroms. Entpacken endet dann mitten im Strom,
    // und was bis dahin lesbar wurde, ist mehr wert als eine Wand aus Hex.
    let fixture = Fixture::open();
    let plain = b"x".repeat(64 * 1024);
    let packed = gzip(&plain);
    let half = packed.slice(..packed.len() / 2);
    let mut reference = fixture.request_body("gzip", half).await;
    reference.truncated = true;
    reference.decoded = true;

    let (bytes, left) = fixture.body(reference).await;
    assert!(!bytes.is_empty(), "the prefix that unpacked is handed out");
    assert!(bytes.len() < plain.len(), "and it is only a prefix");
    assert!(bytes.iter().all(|byte| *byte == b'x'));
    assert_eq!(left, "", "what arrived is unpacked, it is just not all");
}

#[tokio::test]
async fn get_body_decoded_on_a_broken_stream_hands_out_the_raw_bytes() {
    // Kein Präfix, sondern ein Strom, der vollständig sein müsste und es nicht
    // ist. Dann ist etwas anderes falsch als eine Kürzung, und die rohen Bytes
    // sind die ehrlichere Antwort als ein halber Text.
    let fixture = Fixture::open();
    let packed = gzip(PLAIN);
    let mut broken = packed.to_vec();
    let last = broken.len() - 5;
    broken[last] ^= 0xFF;
    let broken = Bytes::from(broken);
    let mut reference = fixture.request_body("gzip", broken.clone()).await;

    assert!(!reference.truncated, "the recording kept the whole body");
    reference.decoded = true;
    let (bytes, left) = fixture.body(reference).await;
    assert_eq!(bytes, broken);
    assert_eq!(left, "gzip");
}
