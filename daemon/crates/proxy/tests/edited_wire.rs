//! Die bearbeitete Anfrage auf dem Draht (HUM-047).
//!
//! `edit::apply_edit` setzt `content-length` und streicht `transfer-encoding`,
//! `content-encoding` und `expect` — für die **Aufzeichnung**. Hinaus geht die
//! Anfrage über `upstream::build_outgoing`, das jeden mitgebrachten
//! `content-length` verwirft und ihn aus der Länge des gepufferten Rumpfs neu
//! setzt, nach derselben Regel (`upstream::wire_content_length`). Ob die
//! Kopfzeilen, die der Einheitstest in `edit.rs` am Datensatz misst, auch
//! wirklich beim Ziel ankommen, sagt nur ein Lauf durch den echten Proxy.
//!
//! Hier schickt ein Agent eine Anfrage, die `chunked` und mit
//! `content-encoding: gzip` ankommt — der Fall, vor dem die Fallstricke des
//! Issues warnen. Ein Mensch gibt sie bearbeitet frei, und die Bearbeitung
//! trägt selbst noch `transfer-encoding`, `content-encoding`, `expect` und
//! einen falschen `content-length` mit, so wie ein nachlässiger Client sie
//! mitschicken könnte. Der Fake-Upstream aus HUM-017 (Karte `Matrix`)
//! antwortet mit den Kopfzeilen, die **er** empfangen hat, samt Länge und
//! sha256 des Rumpfs, den er gelesen hat.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use bytes::Bytes;
use humanitl_core::http::HeaderValue;
use humanitl_core::{
    Authority, BodyRef, Decision, HeaderMap, HeaderName, HostName, HttpRequest, Method, Scheme,
};

use support::upstream::sha256_hex;
use support::{FakeUpstream, ProxyBuilder};

/// Was der Agent schickt: `chunked`, gzip angekündigt, ein Geheimnis darin.
///
/// Der Rumpf ist kein echtes gzip. Das ist Absicht: Der Proxy leitet nach der
/// Bearbeitung den Text des Menschen weiter, und stünde irgendwo noch der
/// Rumpf des Agenten oder seine Kodierung, fiele es an sha256 und
/// `content-encoding` beim Ziel auf.
const ORIGINAL: &[u8] = b"token=ghp_R8kQexampleexampleexampleexample00";

/// Schickt [`ORIGINAL`] als `chunked` durch den Proxy an `/echo` von
/// `upstream`, lässt ihn mit `edited` freigeben und liefert die Antwort des
/// Upstreams als Text: Statuszeile, Kopf und das JSON des Echos.
async fn send_edited(upstream: &FakeUpstream, edited: HttpRequest) -> String {
    let proxy = ProxyBuilder::new().start().await;
    let mut events = proxy.events();
    let _decider = proxy.decide_with(Decision::AllowEdited {
        request: Box::new(edited),
    });

    let port = upstream.port();
    let head = format!(
        "POST http://127.0.0.1:{port}/echo HTTP/1.1\r\n\
         host: 127.0.0.1:{port}\r\n\
         content-type: application/x-www-form-urlencoded\r\n\
         content-encoding: gzip\r\n\
         transfer-encoding: chunked\r\n\
         connection: close\r\n\
         \r\n"
    );
    let mut chunked = format!("{:x}\r\n", ORIGINAL.len()).into_bytes();
    chunked.extend_from_slice(ORIGINAL);
    chunked.extend_from_slice(b"\r\n0\r\n\r\n");

    let answer = proxy.raw_exchange(&head, &chunked).await;
    events.wait_for("recorded").await;
    assert_eq!(upstream.hits(), 1, "exactly the edited request arrives");
    assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
    answer
}

/// Die Fassung des Menschen: derselbe Host, derselbe Port, ein neuer Rumpf,
/// und die Kopfzeilen, die der Daemon selbst setzt, absichtlich falsch dabei.
fn edited(upstream: &FakeUpstream, body: &[u8]) -> HttpRequest {
    edited_as(upstream, Method::POST, body)
}

/// Wie [`edited`], mit der Methode `method`.
fn edited_as(upstream: &FakeUpstream, method: Method, body: &[u8]) -> HttpRequest {
    let authority = Authority::new(HostName::parse("127.0.0.1").unwrap(), upstream.port());
    let mut headers = HeaderMap::new();
    for (name, value) in [
        ("content-type", "text/plain; charset=utf-8"),
        ("x-edited-by", "humanitl-test"),
        ("transfer-encoding", "chunked"),
        ("content-encoding", "gzip"),
        ("content-length", "999"),
        ("expect", "100-continue"),
    ] {
        headers.append(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }
    HttpRequest::new(method, Scheme::Http, authority, "/echo")
        .with_headers(headers)
        .with_body(BodyRef::from_bytes(Bytes::copy_from_slice(body)))
}

/// Ein Feld aus dem JSON des Fake-Upstreams, auch eine empfangene Kopfzeile.
///
/// Die Kopfzeilen stehen als flaches Objekt im JSON, und kein Name darin fällt
/// mit einem Feld der obersten Ebene zusammen, das hier abgefragt wird.
fn json_field(answer: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\":");
    let start = answer.find(&needle)? + needle.len();
    let rest = answer[start..].trim_start();
    if let Some(text) = rest.strip_prefix('"') {
        let end = text.find('"')?;
        return Some(text[..end].to_owned());
    }
    let end = rest.find([',', '}'])?;
    Some(rest[..end].trim().to_owned())
}

/// Was für jede bearbeitete Anfrage gilt, gleich welcher Rumpf.
fn assert_daemon_owned_headers_are_clean(answer: &str) {
    assert_eq!(
        json_field(answer, "transfer-encoding"),
        None,
        "no transfer-encoding reaches the upstream: {answer}"
    );
    assert_eq!(
        json_field(answer, "content-encoding"),
        None,
        "the edited body goes out uncompressed, and says so: {answer}"
    );
    assert_eq!(
        json_field(answer, "expect"),
        None,
        "nothing is left to wait for: {answer}"
    );
    assert_eq!(
        json_field(answer, "x-edited-by").as_deref(),
        Some("humanitl-test"),
        "a free header of the edit travels: {answer}"
    );
    assert_eq!(
        json_field(answer, "content-type").as_deref(),
        Some("text/plain; charset=utf-8"),
        "the content type is the one of the edit: {answer}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn an_edited_body_arrives_with_its_byte_length() {
    let upstream = FakeUpstream::matrix().await;
    let body = b"token=<SECRET_1>";
    let answer = send_edited(&upstream, edited(&upstream, body)).await;

    assert_eq!(
        json_field(&answer, "content-length").as_deref(),
        Some("16"),
        "{answer}"
    );
    assert_eq!(json_field(&answer, "body_len").as_deref(), Some("16"));
    assert_eq!(
        json_field(&answer, "body_sha256"),
        Some(sha256_hex(body)),
        "the upstream reads the edited bytes, not the agent's"
    );
    assert_daemon_owned_headers_are_clean(&answer);
}

/// Ein geleerter POST sagt `content-length: 0` (HUM-047, RFC 9110 8.6).
///
/// Von selbst schriebe hyper für einen leeren `Full`-Rumpf gar keinen; bis
/// zur Review-Runde der Restarbeit von HUM-047 stand die Kopfzeile deshalb
/// nur in der Aufzeichnung (`post_with_empty_body_says_zero` in `edit.rs`)
/// und nicht auf dem Draht. `upstream::build_outgoing` setzt sie jetzt selbst.
#[tokio::test(flavor = "multi_thread")]
async fn an_emptied_post_arrives_with_content_length_zero() {
    let upstream = FakeUpstream::matrix().await;
    let answer = send_edited(&upstream, edited(&upstream, b"")).await;

    assert_eq!(
        json_field(&answer, "content-length").as_deref(),
        Some("0"),
        "a POST without a body says so: {answer}"
    );
    assert_eq!(json_field(&answer, "body_len").as_deref(), Some("0"));
    assert_eq!(json_field(&answer, "body_sha256"), Some(sha256_hex(b"")));
    assert_daemon_owned_headers_are_clean(&answer);
}

/// Die eine Ausnahme: `GET` ohne Rumpf trägt keinen `content-length`, weil
/// manche Server `content-length: 0` an einem `GET` als Rumpf-Ankündigung
/// lesen. Die Bearbeitung macht hier aus dem POST des Agenten ein GET.
#[tokio::test(flavor = "multi_thread")]
async fn an_emptied_get_arrives_without_content_length() {
    let upstream = FakeUpstream::matrix().await;
    let answer = send_edited(&upstream, edited_as(&upstream, Method::GET, b"")).await;

    assert_eq!(json_field(&answer, "method").as_deref(), Some("GET"));
    assert_eq!(
        json_field(&answer, "content-length"),
        None,
        "a GET without a body announces none: {answer}"
    );
    assert_eq!(json_field(&answer, "body_len").as_deref(), Some("0"));
    assert_daemon_owned_headers_are_clean(&answer);
}

/// Die Länge in Bytes, nicht in Zeichen: 19 Zeichen, 26 Bytes UTF-8.
#[tokio::test(flavor = "multi_thread")]
async fn a_multi_byte_body_is_counted_in_bytes() {
    let upstream = FakeUpstream::matrix().await;
    let text = "Grüße an 🌍 Berlin ✓";
    let body = text.as_bytes();
    assert_eq!(text.chars().count(), 19);
    assert_eq!(body.len(), 26);
    let answer = send_edited(&upstream, edited(&upstream, body)).await;

    assert_eq!(
        json_field(&answer, "content-length").as_deref(),
        Some("26"),
        "content-length counts bytes, not characters: {answer}"
    );
    assert_eq!(json_field(&answer, "body_len").as_deref(), Some("26"));
    assert_eq!(json_field(&answer, "body_sha256"), Some(sha256_hex(body)));
    assert_daemon_owned_headers_are_clean(&answer);
}
