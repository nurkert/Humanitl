//! HTTP-Werttypen des Kerns.
//!
//! `Method`, `HeaderMap` und `Version` kommen unverändert aus der Crate `http`
//! (1.x, dieselbe Version, die hyper 1 und hudsucker benutzen) und werden hier
//! nur weitergereicht. Eigene Typen gibt es dort, wo der Kern eine Invariante
//! hält: [`Authority`] trägt einen normalisierten [`HostName`], [`BodyRef`]
//! trennt Inhalt von Verweis.

use core::fmt;
use std::net::IpAddr;

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::hex;
use crate::host::HostName;

pub use ::http::{HeaderMap, HeaderName, HeaderValue, Method, Version};

/// Schema der Anfrage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scheme {
    /// `http://`
    Http,
    /// `https://`
    Https,
    /// `ws://`
    Ws,
    /// `wss://`
    Wss,
}

impl Scheme {
    /// Kleinbuchstaben-Form ohne `://`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https => "https",
            Self::Ws => "ws",
            Self::Wss => "wss",
        }
    }

    /// Der Port, der ohne ausdrückliche Angabe gilt.
    #[must_use]
    pub const fn default_port(self) -> u16 {
        match self {
            Self::Http | Self::Ws => 80,
            Self::Https | Self::Wss => 443,
        }
    }

    /// Wahr für `https` und `wss`.
    #[must_use]
    pub const fn is_secure(self) -> bool {
        matches!(self, Self::Https | Self::Wss)
    }

    /// Liest ein Schema aus Text, ohne Rücksicht auf Groß-/Kleinschreibung.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text.to_ascii_lowercase().as_str() {
            "http" => Some(Self::Http),
            "https" => Some(Self::Https),
            "ws" => Some(Self::Ws),
            "wss" => Some(Self::Wss),
            _ => None,
        }
    }
}

impl fmt::Display for Scheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Protokollwechsel, den eine Anfrage anfragt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Upgrade {
    /// `Upgrade: websocket`
    WebSocket,
}

impl Upgrade {
    /// Kleinbuchstaben-Form, wie sie in `rules.yaml` steht.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WebSocket => "websocket",
        }
    }
}

impl fmt::Display for Upgrade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Ziel einer Anfrage: normalisierter Host plus Port.
///
/// Der Port ist immer gesetzt. Fehlt er in der Anfrage, wird er aus dem Schema
/// ergänzt, damit ein Vergleich mit einer Regel nie zwei Schreibweisen
/// desselben Ziels unterscheidet.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Authority {
    /// Der Host, normalisiert.
    pub host: HostName,
    /// Der Port, nie leer.
    pub port: u16,
}

impl Authority {
    /// Baut eine Authority aus Host und ausdrücklichem Port.
    #[must_use]
    pub const fn new(host: HostName, port: u16) -> Self {
        Self { host, port }
    }

    /// Baut eine Authority mit dem Standard-Port des Schemas.
    #[must_use]
    pub const fn with_scheme(host: HostName, scheme: Scheme) -> Self {
        Self {
            host,
            port: scheme.default_port(),
        }
    }

    /// Der Port, der ohne ausdrückliche Angabe gilt, siehe [`Scheme::default_port`].
    #[must_use]
    pub const fn default_port(scheme: Scheme) -> u16 {
        scheme.default_port()
    }

    /// Wahr, wenn der Port der Standard-Port des Schemas ist.
    #[must_use]
    pub const fn is_default_port(&self, scheme: Scheme) -> bool {
        self.port == scheme.default_port()
    }
}

impl fmt::Display for Authority {
    /// `host:port`, `IPv6` in eckigen Klammern.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", UrlHost(&self.host), self.port)
    }
}

/// Ein Host, so wie er in einer URL steht: `IPv6` in eckigen Klammern.
///
/// Die eine Stelle für die Klammer-Regel; [`Authority`] und
/// [`HttpRequest::url`] schreiben beide hierüber.
struct UrlHost<'a>(&'a HostName);

impl fmt::Display for UrlHost<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            HostName::Ip(IpAddr::V6(ip)) => write!(f, "[{ip}]"),
            host => write!(f, "{host}"),
        }
    }
}

/// Verweis auf einen Body.
///
/// Ereignisse tragen nie den Inhalt, sondern diesen Verweis: Hash und Größe
/// reichen für Anzeige, Vergleich und Nachschlagen im Blob-Speicher. `inline`
/// ist nur bei kleinen Bodies gesetzt (`recorder.inline_max_bytes`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyRef {
    /// SHA-256 über den vollständigen Body.
    pub sha256: [u8; 32],
    /// Länge des vollständigen Bodys in Bytes.
    pub size: u64,
    /// Der Inhalt selbst, wenn er klein genug ist.
    pub inline: Option<Bytes>,
    /// Der Wert aus `Content-Type`, falls die Anfrage einen hatte.
    pub content_type: Option<String>,
    /// Wahr, wenn `inline` nur ein Anfang des Bodys ist.
    pub truncated: bool,
}

impl BodyRef {
    /// Der leere Body: Hash über null Bytes, Größe 0.
    #[must_use]
    pub fn empty() -> Self {
        Self::from_bytes(Bytes::new())
    }

    /// Baut einen Verweis aus dem vollständigen Inhalt und hält ihn inline.
    #[must_use]
    pub fn from_bytes(body: Bytes) -> Self {
        let size = u64::try_from(body.len()).unwrap_or(u64::MAX);
        Self {
            sha256: sha256(&body),
            size,
            inline: Some(body),
            content_type: None,
            truncated: false,
        }
    }

    /// Baut einen Verweis ohne Inhalt, etwa für einen Body im Blob-Speicher.
    #[must_use]
    pub const fn detached(sha256: [u8; 32], size: u64) -> Self {
        Self {
            sha256,
            size,
            inline: None,
            content_type: None,
            truncated: false,
        }
    }

    /// Setzt den `Content-Type`.
    #[must_use]
    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    /// Der Hash als Kleinbuchstaben-Hex, 64 Zeichen.
    #[must_use]
    pub fn sha256_hex(&self) -> String {
        hex::encode(&self.sha256)
    }

    /// Wahr, wenn der Body null Bytes lang ist.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.size == 0
    }
}

/// SHA-256 über beliebige Bytes.
#[must_use]
pub fn sha256(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

/// Eine Anfrage, so wie der Proxy sie sieht und wie die Oberfläche sie zeigt.
#[derive(Debug, Clone, PartialEq)]
pub struct HttpRequest {
    /// Die Methode, unverändert aus der Anfrage.
    pub method: Method,
    /// Das Schema, aus dem der Proxy die Verbindung aufgebaut hat.
    pub scheme: Scheme,
    /// Ziel-Host und Port.
    pub authority: Authority,
    /// Pfad samt Query, beginnend mit `/`.
    pub path_and_query: String,
    /// Die Header der Anfrage.
    pub headers: HeaderMap,
    /// Verweis auf den Body.
    pub body: BodyRef,
    /// Die HTTP-Version.
    pub version: Version,
}

impl HttpRequest {
    /// Baut eine Anfrage ohne Header und ohne Body.
    ///
    /// Header und Body werden danach gesetzt; das hält die Signatur kurz und
    /// vermeidet einen Konstruktor mit sieben Argumenten.
    #[must_use]
    pub fn new(
        method: Method,
        scheme: Scheme,
        authority: Authority,
        path_and_query: impl Into<String>,
    ) -> Self {
        Self {
            method,
            scheme,
            authority,
            path_and_query: path_and_query.into(),
            headers: HeaderMap::new(),
            body: BodyRef::empty(),
            version: Version::HTTP_11,
        }
    }

    /// Setzt die Header.
    #[must_use]
    pub fn with_headers(mut self, headers: HeaderMap) -> Self {
        self.headers = headers;
        self
    }

    /// Setzt den Body-Verweis.
    #[must_use]
    pub fn with_body(mut self, body: BodyRef) -> Self {
        self.body = body;
        self
    }

    /// Der Ziel-Host.
    #[must_use]
    pub const fn host(&self) -> &HostName {
        &self.authority.host
    }

    /// Die vollständige URL, wie sie in Oberfläche und Audit erscheint.
    ///
    /// Der Port wird weggelassen, wenn er der Standard-Port des Schemas ist;
    /// eine `IPv6`-Adresse steht in beiden Fällen in eckigen Klammern.
    #[must_use]
    pub fn url(&self) -> String {
        if self.authority.is_default_port(self.scheme) {
            format!(
                "{}://{}{}",
                self.scheme,
                UrlHost(&self.authority.host),
                self.path_and_query
            )
        } else {
            format!(
                "{}://{}{}",
                self.scheme, self.authority, self.path_and_query
            )
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::{Authority, BodyRef, HttpRequest, Method, Scheme, Upgrade, sha256};
    use crate::host::HostName;

    fn host(text: &str) -> HostName {
        HostName::parse(text).unwrap_or_else(|_| HostName::Dns("invalid.example".to_owned()))
    }

    #[test]
    fn default_ports() {
        assert_eq!(Authority::default_port(Scheme::Http), 80);
        assert_eq!(Authority::default_port(Scheme::Https), 443);
        assert_eq!(Authority::default_port(Scheme::Ws), 80);
        assert_eq!(Authority::default_port(Scheme::Wss), 443);
    }

    #[test]
    fn authority_display_brackets_ipv6() {
        let authority = Authority::new(host("[::1]"), 8443);
        assert_eq!(authority.to_string(), "[::1]:8443");
        assert_eq!(
            Authority::new(host("github.com"), 443).to_string(),
            "github.com:443"
        );
    }

    #[test]
    fn url_hides_the_default_port() {
        let request = HttpRequest::new(
            Method::GET,
            Scheme::Https,
            Authority::with_scheme(host("GitHub.com."), Scheme::Https),
            "/repos",
        );
        assert_eq!(request.url(), "https://github.com/repos");

        let odd = HttpRequest::new(
            Method::GET,
            Scheme::Http,
            Authority::new(host("example.com"), 8080),
            "/",
        );
        assert_eq!(odd.url(), "http://example.com:8080/");
    }

    #[test]
    fn url_brackets_ipv6_with_and_without_the_default_port() {
        let default = HttpRequest::new(
            Method::GET,
            Scheme::Https,
            Authority::with_scheme(host("[::1]"), Scheme::Https),
            "/x",
        );
        assert_eq!(default.url(), "https://[::1]/x");

        let explicit = HttpRequest::new(
            Method::GET,
            Scheme::Https,
            Authority::new(host("[::1]"), 8443),
            "/x",
        );
        assert_eq!(explicit.url(), "https://[::1]:8443/x");

        let v4 = HttpRequest::new(
            Method::GET,
            Scheme::Http,
            Authority::with_scheme(host("127.0.0.1"), Scheme::Http),
            "/",
        );
        assert_eq!(v4.url(), "http://127.0.0.1/");
    }

    #[test]
    fn empty_body_hashes_the_empty_string() {
        let body = BodyRef::empty();
        assert!(body.is_empty());
        assert_eq!(
            body.sha256_hex(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(sha256(b"").len(), 32);
    }

    #[test]
    fn scheme_and_upgrade_text_forms() {
        assert_eq!(Scheme::parse("HTTPS"), Some(Scheme::Https));
        assert_eq!(Scheme::parse("gopher"), None);
        assert_eq!(Scheme::Wss.to_string(), "wss");
        assert!(Scheme::Wss.is_secure());
        assert_eq!(Upgrade::WebSocket.to_string(), "websocket");
    }
}
