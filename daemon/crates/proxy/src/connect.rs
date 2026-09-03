//! Der Verbindungskontext und die Konsistenzprüfung von CONNECT-Ziel, SNI und
//! Authority (HUM-023, ADR-007 Domain-Fronting).
//!
//! Ein Client kann `CONNECT github.com:443` senden und innerhalb der
//! TLS-Verbindung `Host: evil.io` setzen oder eine andere SNI schicken. Wird
//! die Regel nur auf das CONNECT-Ziel angewendet, ist ein Allow für
//! `github.com` ein Allow für alles. Deshalb gilt: Das Ziel, zu dem die
//! Verbindung wirklich führt (CONNECT-Ziel und SNI), und der Host, den die
//! Anfrage nennt (`Host` beziehungsweise `:authority`), müssen dasselbe sein.
//! Weicht eines ab, wird die Anfrage ohne Rückfrage geblockt
//! ([`BlockReason::AuthorityMismatch`]) und mit dem echten Ziel als Authority
//! des Flows verbucht.
//!
//! Es gibt genau eine Stelle, die das prüft: [`check_authority`]. Ihr Ergebnis
//! wird zur [`HttpRequest::authority`](humanitl_core::HttpRequest), und der
//! ganze weitere Weg — Regeln, Hold, Weiterleitung — verwendet ausschließlich
//! diese. Das CONNECT-Ziel allein wird nie ausgewertet.

use std::net::SocketAddr;

use http::header::HOST;
use http::{Request, Version};
use humanitl_core::diagnostics::codes::PROXY_002;
use humanitl_core::{
    Authority, BlockReason, Diagnostic, HostName, Scheme, SessionId, Severity, Upgrade,
};

/// Was der Handler über die Verbindung weiß, aus der ein Flow stammt.
///
/// Pro Client-Verbindung, nicht pro Flow: eine Keep-Alive-Verbindung trägt
/// mehrere Flows mit denselben Angaben. Der Kontext lebt vom `CONNECT` bis zum
/// Verbindungsende; nach dem `CONNECT` legt der Handler einen zweiten Kontext
/// für die entschlüsselte Verbindung an, der Tunnelziel und SNI trägt.
///
/// Der Name aus HUM-015 ([`ConnMeta`]) bleibt als Alias bestehen, damit die
/// Aufrufer außerhalb dieser Crate nicht mitwandern müssen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionContext {
    /// Die Sitzung, zu der die Verbindung gehört.
    pub session: SessionId,
    /// Das Ziel des `CONNECT`, falls die Verbindung ein Tunnel ist; `None` bei
    /// Klartext-HTTP ohne `CONNECT`.
    pub connect_target: Option<Authority>,
    /// Der Name aus dem `ClientHello`, sobald der TLS-Handschlag steht.
    ///
    /// `None` heißt: Der Client hat keine SNI geschickt. In einem Tunnel zu
    /// einem DNS-Namen ist das ein Mismatch (Clients ohne SNI sind im
    /// Ziel-Umfeld nicht legitim); zu einer IP-Adresse darf sie fehlen, weil
    /// TLS für IP-Ziele keine SNI vorsieht.
    pub sni: Option<HostName>,
    /// Die Gegenstelle, falls sie eine Adresse hat.
    ///
    /// Der Proxy lauscht auf einem Unix-Socket (Garantie 2: kein
    /// Loopback-Port auf dem Host), und eine Unix-Verbindung hat keine
    /// Peer-Adresse; in M1 ist das Feld deshalb immer `None`. Es steht hier,
    /// weil Befunde und Aufzeichnung die Adresse tragen, sobald es einen
    /// Zuhörer gibt, der eine hat.
    pub client_addr: Option<SocketAddr>,
    /// Erlaubt Ziele in privaten Netzen (RFC 1918, Loopback, Link-Local,
    /// CGNAT). In M1 ein Test-Hook; später setzt ihn die
    /// LLM-Passthrough-Regel (`backlog/CONVENTIONS.md` 4.10).
    pub allow_private: bool,
}

/// Der bisherige Name des Verbindungskontexts (HUM-015).
pub type ConnMeta = ConnectionContext;

impl ConnectionContext {
    /// Eine Verbindung ohne Tunnel, ohne TLS, ohne private Ziele.
    #[must_use]
    pub const fn plain(session: SessionId) -> Self {
        Self {
            session,
            connect_target: None,
            sni: None,
            client_addr: None,
            allow_private: false,
        }
    }

    /// Der Kontext der entschlüsselten Verbindung hinter einem `CONNECT`.
    #[must_use]
    pub fn tunnel(&self, target: Authority, sni: Option<HostName>) -> Self {
        Self {
            connect_target: Some(target),
            sni,
            ..self.clone()
        }
    }

    /// Wahr, wenn diese Verbindung hinter einem `CONNECT` liegt und der Proxy
    /// auf ihr TLS terminiert hat.
    #[must_use]
    pub const fn is_tunnel(&self) -> bool {
        self.connect_target.is_some()
    }
}

/// Schema und Ziel einer Anfrage, nachdem sie geprüft wurden.
///
/// Die Spezifikation nennt nur die [`Authority`]; das Schema gehört dazu, weil
/// es in derselben Prüfung entsteht (Absolut-Form, sonst der TLS-Zustand der
/// Verbindung) und der Aufrufer beides zugleich braucht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestTarget {
    /// Das Schema, unter dem die Anfrage hinausginge.
    pub scheme: Scheme,
    /// Das Ziel, das für den ganzen weiteren Weg gilt.
    pub authority: Authority,
}

/// Warum eine Anfrage kein auswertbares Ziel hat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorityError {
    /// Die Angaben widersprechen sich: geblockt, ohne zu fragen.
    Mismatch(AuthorityRefusal),
    /// Es gibt überhaupt kein Ziel (kein `Host`, kein Tunnel, keine
    /// Absolut-Form). Daraus lässt sich kein Flow bauen, weil niemand sagen
    /// könnte, wohin er ginge; der Client bekommt `400`, bevor irgendetwas
    /// beginnt.
    ///
    /// `backlog/sprint-2.md` HUM-023 nennt für den fehlenden `Host` einer
    /// HTTP/1.1-Anfrage durchweg `AuthorityMismatch`. Für diesen einen Fall —
    /// Origin-Form, kein Tunnel, kein `Host` — bleibt es bewusst bei `400`:
    /// Ein Flow braucht eine [`Authority`], und die gäbe es hier nur erfunden.
    /// Ein Datensatz mit einem Ziel, das niemand genannt hat, und eine
    /// `403`-Antwort mit `host: unknown` wären beide unwahr; RFC 9110 §7.2
    /// verlangt an dieser Stelle ohnehin `400`. Sobald ein Ziel bekannt ist —
    /// im Tunnel das Tunnelziel, ohne Tunnel die Absolut-Form —, gilt wieder
    /// [`AuthorityError::Mismatch`] mit `403`, wie die Spezifikation es will.
    /// Weitergeleitet wird in keinem der beiden Fälle etwas.
    NoTarget(&'static str),
}

/// Eine abgelehnte Anfrage samt dem Ziel, unter dem sie verbucht wird.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorityRefusal {
    /// Das echte Ziel der Verbindung; unter ihm wird der Flow verbucht und in
    /// der `403`-Antwort genannt.
    pub target: RequestTarget,
    /// Der Satz für den Befund; nennt beide Seiten des Widerspruchs.
    pub why: String,
}

impl AuthorityRefusal {
    /// Der Grund, mit dem der Flow endet.
    #[must_use]
    pub const fn reason(&self) -> BlockReason {
        BlockReason::AuthorityMismatch
    }

    /// Der Befund für den Ereignisstrom.
    ///
    /// Code [`PROXY_002`] („Authority-Mismatch"), nicht das in `sprint-2.md`
    /// genannte `PROXY_003`: Das Register in `backlog/CONVENTIONS.md` 4.11 hat
    /// `PROXY_003` seit HUM-015 der gescheiterten Upstream-Verbindung
    /// zugewiesen, und `PROXY_002` ist genau dieser Fall.
    #[must_use]
    pub fn diagnostic(&self) -> Diagnostic {
        Diagnostic::builder(PROXY_002, Severity::Warning)
            .why(self.why.clone())
            .build()
    }
}

/// Prüft CONNECT-Ziel, SNI und `Host`/`:authority` auf Konsistenz.
///
/// Die Reihenfolge ist die aus `backlog/sprint-2.md` HUM-023:
///
/// 1. Das Schema muss zur Verbindung passen: In einem Tunnel ist es `https`,
///    ohne Tunnel `http`. `http` im Tunnel wäre eine Herabstufung auf
///    Klartext, `https` ohne Tunnel überspränge die SNI-Prüfung; beides ist
///    ein Widerspruch.
/// 2. `A` ist die Authority der Anfrage: bei HTTP/2 `:authority`, bei
///    HTTP/1.1 der `Host`-Kopf, der dort Pflicht ist. Trägt die Anfragezeile
///    eine Absolut-Form, muss deren Host dem `Host`-Kopf gleichen. Mehr als
///    ein `Host` ist ein Widerspruch mit sich selbst.
/// 3. Fehlt `A` der Port, gilt der Standardport des Schemas: 443 im Tunnel,
///    sonst 80.
/// 4. Im Tunnel zu `C` muss `A == C` sein, und die SNI muss `C.host` sein.
///    Fehlt die SNI und ist `C.host` ein DNS-Name, ist das ein Mismatch.
/// 5. Ohne Tunnel ist `A` maßgeblich, das Schema `http`.
///
/// # Errors
///
/// [`AuthorityError::Mismatch`], wenn sich die Angaben widersprechen — der
/// Aufrufer blockt ohne Rückfrage. [`AuthorityError::NoTarget`], wenn es gar
/// kein Ziel gibt — der Aufrufer antwortet `400`, ohne einen Flow zu bauen.
pub fn check_authority<B>(
    ctx: &ConnectionContext,
    req: &Request<B>,
) -> Result<RequestTarget, AuthorityError> {
    let scheme = check_scheme(ctx, req.uri())?;
    let authority = named_authority(ctx, req, scheme)?;
    check_tunnel(ctx, scheme, &authority)?;
    Ok(RequestTarget { scheme, authority })
}

/// Schritt 1: Das Schema der Anfrage muss das der Verbindung sein.
///
/// Der Weg nach draußen hat genau zwei Formen. `http://` geht im Klartext an
/// den Proxy, `https://` geht durch ein `CONNECT`, das der Proxy mit TLS
/// terminiert. Jede andere Paarung ist ein Widerspruch, und beide Richtungen
/// sind gefährlich:
///
/// - `http://` **im Tunnel** wäre eine Herabstufung. Der Client hat einen
///   TLS-Tunnel aufgebaut, die Anfrage darin verlangt aber Klartext; der
///   Weiterleiter richtet sich nach dem Schema der Anfrage
///   ([`Scheme::is_secure`]) und schickte sie unverschlüsselt hinaus. Aus
///   einem `CONNECT` würde so ein Klartext-Egress, den niemand freigegeben
///   hat.
/// - `https://` **ohne Tunnel** würde die SNI-Prüfung überspringen. Es gäbe
///   keinen Handschlag zum Vergleichen, und der Proxy müsste TLS selbst
///   aufbauen, ohne dass ein `CONNECT` das Ziel genannt hätte. Der einzige Weg
///   zu `https` ist deshalb das `CONNECT` (`backlog/sprint-2.md` HUM-023,
///   Vergleichsregel 4).
fn check_scheme(ctx: &ConnectionContext, uri: &http::Uri) -> Result<Scheme, AuthorityError> {
    let connection = if ctx.is_tunnel() {
        Scheme::Https
    } else {
        Scheme::Http
    };
    let Some(text) = uri.scheme_str() else {
        return Ok(connection);
    };
    let named = Scheme::parse(text).ok_or(AuthorityError::NoTarget("unsupported scheme"))?;
    if named == connection {
        return Ok(named);
    }
    let why = if ctx.is_tunnel() {
        format!(
            "the request line names scheme {named} inside a tunnel that carries {connection}; \
             a request that asks for cleartext inside a TLS tunnel would leave the proxy \
             unencrypted",
            named = named.as_str(),
            connection = connection.as_str(),
        )
    } else {
        format!(
            "the request line names scheme {named} on a cleartext connection that carries \
             {connection}; {named} reaches the proxy only through CONNECT, and only there is \
             there a handshake to compare the target against",
            named = named.as_str(),
            connection = connection.as_str(),
        )
    };
    Err(refuse(ctx, connection, why, None))
}

/// Schritt 1 und 2: die Authority, die die Anfrage selbst nennt.
fn named_authority<B>(
    ctx: &ConnectionContext,
    req: &Request<B>,
    scheme: Scheme,
) -> Result<Authority, AuthorityError> {
    let default_port = scheme.default_port();

    // Zwei `Host`-Köpfe sind ein Widerspruch mit sich selbst; welcher gälte,
    // entschiede sonst die Reihenfolge im Puffer.
    let mut hosts = req.headers().get_all(HOST).iter();
    let host_header = hosts.next();
    if hosts.next().is_some() {
        return Err(refuse(
            ctx,
            scheme,
            "the request carries more than one Host header; \
             which one counts would be a matter of parsing order"
                .to_owned(),
            None,
        ));
    }

    let from_line = match req.uri().authority() {
        Some(authority) => Some(parse_authority(
            authority.host(),
            authority.port_u16(),
            default_port,
        )?),
        None => None,
    };
    let from_header = match host_header {
        Some(value) => {
            let text = value
                .to_str()
                .map_err(|_err| AuthorityError::NoTarget("invalid host"))?;
            let (host, port) = split_host_port(text);
            Some(parse_authority(host, port, default_port)?)
        }
        None => None,
    };

    match (from_line, from_header) {
        (Some(line), Some(header)) if line != header => Err(refuse(
            ctx,
            scheme,
            format!(
                "the request line names {line} and the Host header names {header}; \
                 an authority that contradicts itself is not a request to either"
            ),
            Some(line),
        )),
        // HTTP/2 nennt das Ziel in `:authority`, HTTP/1.1 in `Host`. Sind
        // beide da, sind sie hier schon als gleich erwiesen.
        (Some(line), _) if req.version() == Version::HTTP_2 => Ok(line),
        (_, Some(header)) => Ok(header),
        // Absolut-Form ohne `Host`: HTTP/1.1 verlangt den Kopf (RFC 9110
        // §7.2). Wer ihn wegläßt, trennt das, worüber der Mensch entscheidet
        // (die Anfragezeile), von dem, was der Ursprung später sieht (den
        // Kopf, den ein Zwischenglied ergänzt). Das ist keine Nachlässigkeit,
        // das ist die Naht, an der Entscheidung und Wirkung auseinandergehen.
        (Some(line), None) => Err(refuse(
            ctx,
            scheme,
            format!(
                "the request line names {line} but the request carries no Host header; \
                 HTTP/1.1 requires it, and without it the decision and the origin can be \
                 told two different targets"
            ),
            Some(line),
        )),
        (None, None) => match &ctx.connect_target {
            Some(target) => Err(refuse(
                ctx,
                scheme,
                format!(
                    "the request inside the tunnel to {target} carries no Host header; \
                     without it nothing says which host it is meant for"
                ),
                None,
            )),
            None => Err(AuthorityError::NoTarget("missing host")),
        },
    }
}

/// Schritt 3: Im Tunnel müssen Ziel, SNI und Authority dasselbe meinen.
fn check_tunnel(
    ctx: &ConnectionContext,
    scheme: Scheme,
    named: &Authority,
) -> Result<(), AuthorityError> {
    let Some(target) = &ctx.connect_target else {
        return Ok(());
    };
    if named != target {
        return Err(refuse(
            ctx,
            scheme,
            format!(
                "the client sent Host {named} inside a tunnel to {target}; \
                 a decision for one host is not a decision for another"
            ),
            None,
        ));
    }
    match &ctx.sni {
        Some(sni) if *sni != target.host => Err(refuse(
            ctx,
            scheme,
            format!(
                "the TLS handshake asked for {} inside a tunnel to {target}; \
                 the name in the ClientHello and the tunnel target must be the same",
                sni.display()
            ),
            None,
        )),
        // Ohne SNI lässt sich nicht belegen, dass der Handschlag demselben
        // Namen galt wie der Tunnel. Für ein IP-Ziel sieht TLS keine SNI vor,
        // dort ist das Fehlen richtig.
        None if matches!(target.host, HostName::Dns(_)) => Err(refuse(
            ctx,
            scheme,
            format!(
                "the client sent no SNI inside a tunnel to {target}; \
                 a TLS client that names no host cannot be held to one"
            ),
            None,
        )),
        _ => Ok(()),
    }
}

/// Baut die Ablehnung samt dem Ziel, unter dem der Flow verbucht wird: im
/// Tunnel das Tunnelziel, sonst das der Anfragezeile. Nie der Host, der gerade
/// bestritten wird.
fn refuse(
    ctx: &ConnectionContext,
    scheme: Scheme,
    why: String,
    fallback: Option<Authority>,
) -> AuthorityError {
    let authority = ctx.connect_target.clone().or(fallback).unwrap_or_else(|| {
        Authority::new(HostName::Dns("unknown".to_owned()), scheme.default_port())
    });
    AuthorityError::Mismatch(AuthorityRefusal {
        target: RequestTarget { scheme, authority },
        why,
    })
}

/// Der angefragte Protokollwechsel, falls die Anfrage einen nennt.
///
/// Nur `websocket`; alles andere ist in M1 kein Wechsel, den eine Regel
/// beschreiben könnte.
#[must_use]
pub fn requested_upgrade(headers: &humanitl_core::HeaderMap) -> Option<Upgrade> {
    headers
        .get(http::header::UPGRADE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case(Upgrade::WebSocket.as_str()))
        .then_some(Upgrade::WebSocket)
}

/// Baut eine [`Authority`] aus Host-Text und optionalem Port.
fn parse_authority(
    host: &str,
    port: Option<u16>,
    default_port: u16,
) -> Result<Authority, AuthorityError> {
    let host = HostName::parse(host).map_err(|_err| AuthorityError::NoTarget("invalid host"))?;
    Ok(Authority::new(host, port.unwrap_or(default_port)))
}

/// Zerlegt einen `Host`-Kopf in Host und optionalen Port; `IPv6` in eckigen
/// Klammern wird korrekt behandelt.
#[must_use]
pub fn split_host_port(value: &str) -> (&str, Option<u16>) {
    if let Some(rest) = value.strip_prefix('[') {
        if let Some((inner, tail)) = rest.split_once(']') {
            let port = tail.strip_prefix(':').and_then(|p| p.parse().ok());
            // Host mit Klammern zurückgeben, damit `HostName::parse` das
            // IPv6-Literal erkennt.
            let end = 1 + inner.len() + 1;
            return (&value[..end], port);
        }
        return (value, None);
    }
    match value.rsplit_once(':') {
        // Nur trennen, wenn der Rest kein weiteres `:` trägt (also keine
        // bracketlose IPv6-Adresse ist) und der Port eine Zahl ist.
        Some((host, port)) if !host.contains(':') && !port.is_empty() => {
            match port.parse::<u16>() {
                Ok(port) => (host, Some(port)),
                Err(_) => (value, None),
            }
        }
        _ => (value, None),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::{AuthorityError, ConnectionContext, check_authority, split_host_port};
    use http::{Request, Version};
    use humanitl_core::{Authority, HostName, Scheme, SessionId};

    fn dns(name: &str) -> HostName {
        HostName::parse(name).unwrap()
    }

    fn authority(name: &str, port: u16) -> Authority {
        Authority::new(dns(name), port)
    }

    fn tunnel(target: &str, port: u16, sni: Option<&str>) -> ConnectionContext {
        ConnectionContext::plain(SessionId::new()).tunnel(authority(target, port), sni.map(dns))
    }

    fn request(uri: &str, headers: &[(&str, &str)]) -> Request<()> {
        let mut builder = Request::builder().uri(uri);
        for (name, value) in headers {
            builder = builder.header(*name, *value);
        }
        builder.body(()).unwrap()
    }

    fn refusal(err: &AuthorityError) -> &str {
        match err {
            AuthorityError::Mismatch(refusal) => &refusal.why,
            AuthorityError::NoTarget(why) => why,
        }
    }

    #[test]
    fn a_matching_triple_passes() {
        let ctx = tunnel("github.com", 443, Some("github.com"));
        let req = request("/repos", &[("host", "github.com")]);
        let target = check_authority(&ctx, &req).unwrap();
        assert_eq!(target.scheme, Scheme::Https);
        assert_eq!(target.authority, authority("github.com", 443));
    }

    #[test]
    fn a_host_that_contradicts_the_tunnel_is_a_mismatch() {
        let ctx = tunnel("github.com", 443, Some("github.com"));
        let req = request("/repos", &[("host", "evil.io")]);
        let err = check_authority(&ctx, &req).unwrap_err();
        let AuthorityError::Mismatch(refusal) = &err else {
            panic!("{err:?}");
        };
        assert_eq!(refusal.target.authority, authority("github.com", 443));
        assert!(refusal.why.contains("evil.io"), "{}", refusal.why);
    }

    #[test]
    fn an_sni_that_contradicts_the_tunnel_is_a_mismatch() {
        let ctx = tunnel("github.com", 443, Some("evil.io"));
        let req = request("/repos", &[("host", "github.com")]);
        assert!(matches!(
            check_authority(&ctx, &req),
            Err(AuthorityError::Mismatch(_))
        ));
    }

    #[test]
    fn a_missing_sni_is_a_mismatch_for_a_name_and_fine_for_an_address() {
        let named = tunnel("github.com", 443, None);
        let req = request("/repos", &[("host", "github.com")]);
        assert!(matches!(
            check_authority(&named, &req),
            Err(AuthorityError::Mismatch(_))
        ));

        let address = ConnectionContext::plain(SessionId::new())
            .tunnel(Authority::new(dns("192.168.1.50"), 11434), None);
        let req = request("/api/tags", &[("host", "192.168.1.50:11434")]);
        let target = check_authority(&address, &req).unwrap();
        assert_eq!(target.authority.port, 11434);
    }

    #[test]
    fn a_port_that_contradicts_the_tunnel_is_a_mismatch() {
        let ctx = tunnel("github.com", 8443, Some("github.com"));
        // Ohne Port im `Host` gilt 443, und 443 ist nicht 8443.
        let req = request("/repos", &[("host", "github.com")]);
        assert!(matches!(
            check_authority(&ctx, &req),
            Err(AuthorityError::Mismatch(_))
        ));
    }

    #[test]
    fn a_missing_host_inside_a_tunnel_is_a_mismatch() {
        let ctx = tunnel("github.com", 443, Some("github.com"));
        let req = request("/repos", &[]);
        let err = check_authority(&ctx, &req).unwrap_err();
        assert!(matches!(err, AuthorityError::Mismatch(_)), "{err:?}");
    }

    #[test]
    fn a_missing_host_without_a_tunnel_has_no_target_at_all() {
        let ctx = ConnectionContext::plain(SessionId::new());
        let req = request("/repos", &[]);
        let err = check_authority(&ctx, &req).unwrap_err();
        assert_eq!(refusal(&err), "missing host");
    }

    #[test]
    fn two_host_headers_are_a_mismatch() {
        let ctx = tunnel("github.com", 443, Some("github.com"));
        let req = request("/repos", &[("host", "github.com"), ("host", "evil.io")]);
        assert!(matches!(
            check_authority(&ctx, &req),
            Err(AuthorityError::Mismatch(_))
        ));
    }

    #[test]
    fn the_request_line_and_the_host_header_must_agree() {
        let ctx = ConnectionContext::plain(SessionId::new());
        let req = request("http://example.com/x", &[("host", "evil.io")]);
        assert!(matches!(
            check_authority(&ctx, &req),
            Err(AuthorityError::Mismatch(_))
        ));

        let req = request("http://example.com/x", &[("host", "example.com")]);
        let target = check_authority(&ctx, &req).unwrap();
        assert_eq!(target.authority, authority("example.com", 80));
        assert_eq!(target.scheme, Scheme::Http);
    }

    #[test]
    fn an_absolute_form_without_a_host_header_is_refused() {
        let ctx = ConnectionContext::plain(SessionId::new());
        let req = request("http://example.com:8080/x", &[]);
        let err = check_authority(&ctx, &req).unwrap_err();
        let AuthorityError::Mismatch(refusal) = &err else {
            panic!("{err:?}");
        };
        assert_eq!(refusal.target.authority, authority("example.com", 8080));
        assert!(refusal.why.contains("no Host header"), "{}", refusal.why);
    }

    #[test]
    fn a_cleartext_scheme_inside_a_tunnel_is_refused() {
        let ctx = tunnel("github.com", 443, Some("github.com"));
        let req = request("http://github.com/repos", &[("host", "github.com")]);
        let err = check_authority(&ctx, &req).unwrap_err();
        let AuthorityError::Mismatch(refusal) = &err else {
            panic!("{err:?}");
        };
        assert_eq!(refusal.target.scheme, Scheme::Https);
        assert_eq!(refusal.target.authority, authority("github.com", 443));
        assert!(refusal.why.contains("http"), "{}", refusal.why);
        assert!(refusal.why.contains("https"), "{}", refusal.why);
    }

    #[test]
    fn a_tls_scheme_without_a_tunnel_is_refused() {
        let ctx = ConnectionContext::plain(SessionId::new());
        let req = request("https://example.com/x", &[("host", "example.com")]);
        let err = check_authority(&ctx, &req).unwrap_err();
        let AuthorityError::Mismatch(refusal) = &err else {
            panic!("{err:?}");
        };
        assert_eq!(refusal.target.scheme, Scheme::Http);
        assert!(refusal.why.contains("CONNECT"), "{}", refusal.why);
    }

    #[test]
    fn h2_takes_the_authority_from_the_request_line() {
        let ctx = tunnel("github.com", 443, Some("github.com"));
        let mut req = request("https://github.com/repos", &[]);
        *req.version_mut() = Version::HTTP_2;
        let target = check_authority(&ctx, &req).unwrap();
        assert_eq!(target.authority, authority("github.com", 443));
    }

    #[test]
    fn a_bracketed_ipv6_host_keeps_its_port() {
        assert_eq!(split_host_port("[::1]:8080"), ("[::1]", Some(8080)));
        assert_eq!(split_host_port("[::1]"), ("[::1]", None));
        assert_eq!(split_host_port("::1"), ("::1", None));
        assert_eq!(split_host_port("host:80"), ("host", Some(80)));
        assert_eq!(split_host_port("host:notaport"), ("host:notaport", None));
    }
}
