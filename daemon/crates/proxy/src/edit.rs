//! Die bearbeitete Anfrage, bevor sie hinausgeht (HUM-047).
//!
//! Ein Mensch hat eine gehaltene Anfrage im Editor pseudonymisiert und
//! `AllowEdited` geschickt. Was dabei ankommt, ist eine vollständige
//! [`HttpRequest`] samt Body — geschrieben von einem Client, und ein Client ist
//! nicht die Oberfläche, sondern alles, was auf dem Socket spricht. Dieses
//! Modul ist die Stelle, an der aus dieser Behauptung eine Anfrage wird, die
//! der Proxy weiterleiten darf.
//!
//! # Warum die Prüfung hier steht und nicht im Eingabefeld
//!
//! Die Oberfläche sperrt Host, Port und Schema; das ist Bequemlichkeit. Die
//! Sicherheitsaussage trägt allein [`apply_edit`]: Über eine gehaltene Anfrage
//! wurde für **dieses** Ziel unter **diesem** Schema entschieden, und eine
//! Bearbeitung, die das Ziel verschöbe, wäre ein Egress, den niemand
//! freigegeben hat. Ein `https` nach `http` umgeschrieben ginge an denselben
//! Host, aber im Klartext — auch das hat niemand entschieden.
//!
//! # Die Reihenfolge der Prüfungen ist Teil des Vertrags
//!
//! 1. [`EDIT_001`] das Ziel (Host, Port, Schema),
//! 2. [`EDIT_002`] die Methode,
//! 3. [`EDIT_003`] Pfad und Query,
//! 4. die gesperrten Kopfzeilen — **kein Fehler**, sie werden still verworfen
//!    und als `edit.locked_header_dropped` vermerkt; der Daemon setzt sie
//!    selbst (deshalb ist die Nummer `EDIT_004` im Register frei),
//! 5. [`EDIT_005`] die Größe des Bodys.
//!
//! Wer die Reihenfolge dreht, meldet einer zu langen Anfrage an ein fremdes
//! Ziel die Länge und nicht das Ziel; der schwerere Befund gehört nach vorn.
//!
//! # Was der Daemon danach selbst setzt
//!
//! `content-length` ist die **Byte**-Länge des neuen Bodys, nie eine
//! Zeichenzahl. `transfer-encoding` fällt weg, weil beides zusammen ein
//! HTTP-Fehler ist (RFC 9112 Abschnitt 6.1) und der Body ohnehin vollständig
//! gepuffert vorliegt. `content-encoding` fällt weg, weil der Editor auf dem
//! **dekodierten** Text arbeitet: Was hier ankommt, ist unkomprimiert, und
//! eine stehengebliebene Kodierungs-Kopfzeile beschriebe etwas, das nicht mehr
//! da ist. `expect` fällt weg, weil auf nichts mehr zu warten ist. `host` wird
//! aus der Authority der gehaltenen Anfrage neu gesetzt.

use bytes::Bytes;
use humanitl_core::diagnostics::codes::{EDIT_001, EDIT_002, EDIT_003, EDIT_005};
use humanitl_core::http::HeaderValue;
use humanitl_core::{BodyRef, Diagnostic, Finding, HeaderMap, HeaderName, HttpRequest, Severity};

use crate::findings::Scanner;
use crate::upstream::{host_header, wire_content_length};

/// Die Kopfzeilen, die eine Bearbeitung nicht mitbringen darf.
///
/// Alle fünf beschreiben, wie der Body übertragen wird, oder wohin die Anfrage
/// geht; beides gehört dem Daemon. Sie werden verworfen, nicht abgelehnt: Ein
/// Client, der `content-length` mitschickt, ist nicht bösartig, er ist
/// überflüssig, und `host` schickt jeder HTTP/1.1-Client mit.
pub const DAEMON_OWNED_HEADERS: [&str; 5] = [
    "host",
    "content-length",
    "transfer-encoding",
    "content-encoding",
    "expect",
];

/// Die längste Methode, die noch ein Token ist.
const METHOD_MAX_LEN: usize = 16;

/// Eine geprüfte Bearbeitung: die Anfrage, wie sie hinausgeht, plus ihr Body.
///
/// Der Body steht neben der Anfrage und nicht nur in
/// [`BodyRef::inline`](humanitl_core::BodyRef), weil der Weiterleitungspfad
/// ihn als [`Bytes`] braucht und ein zweiter Weg, an dieselben Bytes zu
/// kommen, ein zweiter Weg wäre, sie auseinanderlaufen zu lassen.
#[derive(Debug, Clone, PartialEq)]
pub struct Edited {
    /// Die Anfrage mit den Kopfzeilen, die der Daemon selbst gesetzt hat.
    pub request: HttpRequest,
    /// Der Body, genau die Bytes, die `content-length` zählt.
    pub body: Bytes,
}

/// Prüft eine bearbeitete Anfrage und setzt die Kopfzeilen, die dem Daemon
/// gehören.
///
/// `original` ist die gehaltene Anfrage des Agenten, `edited` die Fassung des
/// Menschen, `cap_bytes` die Grenze, gegen die der Hold gepuffert hat
/// (`limits.hold_body_cap_bytes`).
///
/// # Errors
///
/// [`EDIT_001`] bei abweichendem Ziel, [`EDIT_002`] bei ungültiger Methode,
/// [`EDIT_003`] bei ungültigem Pfad, [`EDIT_005`] bei zu großem Body. Jeder
/// Befund trägt `why`; ein `fix` hat keiner, weil an keiner dieser Stellen
/// eine Einstellung oder eine Regel hilft — die Anfrage selbst ist falsch.
pub fn apply_edit(
    original: &HttpRequest,
    edited: HttpRequest,
    cap_bytes: u64,
) -> Result<Edited, Diagnostic> {
    if edited.authority != original.authority || edited.scheme != original.scheme {
        return Err(target_changed(original, &edited));
    }
    check_method(edited.method.as_str())?;
    check_path(&edited.path_and_query)?;

    let body = edited
        .body
        .inline
        .clone()
        .unwrap_or_else(|| Bytes::from_static(b""));
    let size = u64::try_from(body.len()).unwrap_or(u64::MAX);
    if size > cap_bytes {
        return Err(Diagnostic::builder(EDIT_005, Severity::Error)
            .why(format!(
                "the edited body is {size} bytes, over the limit the hold buffered against \
                 ({cap_bytes})"
            ))
            .build());
    }

    let mut request = edited;
    // Der `Content-Type` ist der der Bearbeitung, und nur der. Nennt sie
    // keinen, hat der Mensch die Zeile gelöscht — der Editor beginnt immer
    // mit der Zeile der gehaltenen Anfrage, und die Spezifikation erlaubt das
    // Löschen ausdrücklich. Den Typ des Originals dann wieder einzusetzen,
    // machte eine Löschung rückgängig, die der Mensch getroffen hat: Ein
    // geleerter Rumpf ginge weiter als `application/json` hinaus. Bis zur
    // Review-Runde von HUM-047 stand hier ein solcher Rückfall; er ist mit
    // Absicht entfernt.
    let content_type = request.body.content_type.clone();
    request.headers = daemon_headers(&request, &host_header(original), body.len());
    request.body = BodyRef::from_bytes(body.clone());
    request.body.content_type = content_type;
    Ok(Edited { request, body })
}

/// Welche Funde nach der Bearbeitung noch offen sind.
///
/// Der Scan läuft ein zweites Mal, über die bearbeitete Anfrage: Was der
/// Mensch ersetzt hat, ist weg, was er stehen ließ, steht noch da, und nur die
/// zweite Liste darf für die Freigabe zählen. Die Funde der ersten Runde
/// zeigen in den **alten** Body; sie nach einer Ersetzung weiterzuführen hieße,
/// auf Stellen zu zeigen, die es nicht mehr gibt. Die Liste und nicht nur ihre
/// Länge, weil die harte Sperre von HUM-049 nach Art und Stufe fragt
/// ([`check_allow`](crate::findings::check_allow)).
#[must_use]
pub fn remaining_findings(
    scanner: &dyn Scanner,
    request: &HttpRequest,
    body: &[u8],
) -> Vec<Finding> {
    scanner.scan(request, body).findings
}

/// Der zweite Scan als Zahl für das `Decided`-Ereignis (HUM-160).
///
/// Die Warteschlange veröffentlicht `Decided`, bevor der Handler die
/// bearbeitete Fassung prüft und weiterleitet; die Zahl der Funde, mit denen
/// sie hinausgeht, gehört aber in genau dieses Ereignis. Deshalb läuft
/// derselbe Weg zweimal: hier für die Zahl, im Handler für die harte Sperre.
/// Beide gehen durch [`apply_edit`] und [`remaining_findings`] mit demselben
/// Scanner und derselben Grenze, also sehen beide dieselbe Anfrage. Der
/// doppelte Scan trifft nur bearbeitete Freigaben, und die kommen im Takt
/// eines Menschen.
pub struct SecondScan {
    scanner: std::sync::Arc<dyn Scanner>,
    cap_bytes: u64,
}

impl SecondScan {
    /// Zählt mit `scanner` über Bearbeitungen bis `cap_bytes`
    /// (`limits.hold_body_cap_bytes`, dieselbe Grenze wie im Handler).
    #[must_use]
    pub fn new(scanner: std::sync::Arc<dyn Scanner>, cap_bytes: u64) -> Self {
        Self { scanner, cap_bytes }
    }
}

impl core::fmt::Debug for SecondScan {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SecondScan")
            .field("cap_bytes", &self.cap_bytes)
            .finish_non_exhaustive()
    }
}

impl crate::hold::EditCount for SecondScan {
    fn unresolved(&self, held: &HttpRequest, edited: &HttpRequest) -> Option<u32> {
        let Edited { request, body } = apply_edit(held, edited.clone(), self.cap_bytes).ok()?;
        let remaining = remaining_findings(self.scanner.as_ref(), &request, &body);
        Some(u32::try_from(remaining.len()).unwrap_or(u32::MAX))
    }
}

/// Der Befund für ein verschobenes Ziel.
fn target_changed(original: &HttpRequest, edited: &HttpRequest) -> Diagnostic {
    Diagnostic::builder(EDIT_001, Severity::Error)
        .why(format!(
            "the edited request goes to {}://{}, the held one to {}://{}; the target of a held \
             request may not change, or the rule check that let it through would be worthless",
            edited.scheme, edited.authority, original.scheme, original.authority,
        ))
        .build()
}

/// `^[A-Z]{1,16}$`, von Hand statt über eine Regex-Maschine.
fn check_method(method: &str) -> Result<(), Diagnostic> {
    let ok = (1..=METHOD_MAX_LEN).contains(&method.len())
        && method.bytes().all(|byte| byte.is_ascii_uppercase());
    if ok {
        return Ok(());
    }
    Err(Diagnostic::builder(EDIT_002, Severity::Error)
        .why(format!(
            "{method:?} is not a method; a method is one to {METHOD_MAX_LEN} uppercase ASCII \
             letters"
        ))
        .build())
}

/// Origin-Form: führender Schrägstrich, kein Leerzeichen, kein Steuerzeichen.
fn check_path(path: &str) -> Result<(), Diagnostic> {
    let why = if path.starts_with('/') {
        path.bytes()
            .find(|byte| *byte == b' ' || byte.is_ascii_control() || *byte > 0x7E)
            .map(|byte| format!("it carries the byte {byte:#04x}, which has to be percent-encoded"))
    } else {
        Some("it does not start with `/`".to_owned())
    };
    match why {
        None => Ok(()),
        Some(why) => Err(Diagnostic::builder(EDIT_003, Severity::Error)
            .why(format!(
                "the edited path {path:?} is not an origin-form path: {why}"
            ))
            .build()),
    }
}

/// Die Kopfzeilen der Bearbeitung, ohne die des Daemons, plus die des Daemons.
///
/// `host` kommt aus der Authority der **gehaltenen** Anfrage, nicht aus dem
/// Edit: Über die hat der Mensch entschieden, und [`apply_edit`] hat vorher
/// geprüft, dass beide dieselbe nennen. Geschrieben wird genau der Wert, den
/// [`host_header`] auch auf den Draht setzt — also ohne den Standard-Port des
/// Schemas. Stünde hier `api.github.com:443` und draußen `api.github.com`,
/// zeigte die Aufzeichnung eine Anfrage, die so nie hinausging.
fn daemon_headers(edited: &HttpRequest, host: &str, body_len: usize) -> HeaderMap {
    // `HeaderMap::new()` und nicht `with_capacity`: Die Kapazitaet kommt aus
    // einer Zahl, die ein Client auf dem Socket bestimmt, und `with_capacity`
    // panikt oberhalb von rund 24 577 Eintraegen. Eine Anfrage darf den Daemon
    // nicht anhalten koennen.
    let mut headers = HeaderMap::new();
    for (name, value) in &edited.headers {
        if DAEMON_OWNED_HEADERS.contains(&name.as_str()) {
            tracing::debug!(
                header = %name,
                "edit.locked_header_dropped: the daemon sets this header itself"
            );
            if name.as_str() == "content-encoding" {
                tracing::warn!(
                    "edit.content_encoding_dropped: the editor works on the decoded body, so \
                     the edited request goes out uncompressed"
                );
            }
            continue;
        }
        headers.append(name.clone(), value.clone());
    }
    if let Ok(value) = HeaderValue::from_str(host) {
        headers.insert(HeaderName::from_static("host"), value);
    }
    // Dieselbe Regel wie auf dem Draht (`upstream::build_outgoing`): kein
    // `content-length` nur für `GET` und `HEAD` ohne Rumpf.
    if let Some(length) = wire_content_length(edited.method.as_str(), body_len) {
        headers.insert(
            HeaderName::from_static("content-length"),
            HeaderValue::from(length),
        );
    }
    headers
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_core::{Authority, HostName, Method, Scheme};
    use humanitl_findings::FindingsSettings;

    use super::{DAEMON_OWNED_HEADERS, Edited, apply_edit, remaining_findings};
    use crate::findings::{Scanner as _, Tier1Scanner};
    use bytes::Bytes;
    use humanitl_core::http::HeaderValue;
    use humanitl_core::{BodyRef, HeaderName, HttpRequest};

    const CAP: u64 = 32 * 1024 * 1024;

    fn request(host: &str, port: u16, method: Method, path: &str, body: &[u8]) -> HttpRequest {
        let authority = Authority::new(HostName::Dns(host.to_owned()), port);
        let mut request = HttpRequest::new(method, Scheme::Https, authority, path);
        request.body = BodyRef::from_bytes(Bytes::copy_from_slice(body));
        request
    }

    fn held() -> HttpRequest {
        request("api.github.com", 443, Method::POST, "/repos", b"hello")
    }

    fn with_header(mut request: HttpRequest, name: &'static str, value: &str) -> HttpRequest {
        request.headers.insert(
            HeaderName::from_static(name),
            HeaderValue::from_str(value).unwrap(),
        );
        request
    }

    fn header(edited: &Edited, name: &str) -> Option<String> {
        edited
            .request
            .headers
            .get(name)
            .map(|value| value.to_str().unwrap().to_owned())
    }

    fn content_length(edited: &Edited) -> Option<String> {
        header(edited, "content-length")
    }

    #[test]
    fn authority_change_rejected() {
        let edited = request("evil.io", 443, Method::POST, "/repos", b"hello");
        let error = apply_edit(&held(), edited, CAP).expect_err("a moved target is EDIT_001");
        assert_eq!(error.code.as_str(), "EDIT_001");
        assert!(error.why.contains("evil.io"), "{}", error.why);
    }

    #[test]
    fn port_change_rejected() {
        let edited = request("api.github.com", 8443, Method::POST, "/repos", b"hello");
        let error = apply_edit(&held(), edited, CAP).expect_err("a moved port is EDIT_001");
        assert_eq!(error.code.as_str(), "EDIT_001");
    }

    #[test]
    fn scheme_downgrade_rejected() {
        let mut edited = request("api.github.com", 443, Method::POST, "/repos", b"hello");
        edited.scheme = Scheme::Http;
        let error = apply_edit(&held(), edited, CAP).expect_err("a downgrade is EDIT_001");
        assert_eq!(error.code.as_str(), "EDIT_001");
    }

    #[test]
    fn authority_case_insensitive_ok() {
        // `HostName::parse` normalisiert `API.GITHUB.COM.` auf die Form, die
        // die gehaltene Anfrage traegt; danach sind beide gleich.
        let host = HostName::parse("API.GITHUB.COM.").expect("a normalisable host");
        let authority = Authority::new(host, 443);
        let mut edited = HttpRequest::new(Method::POST, Scheme::Https, authority, "/repos");
        edited.body = BodyRef::from_bytes(Bytes::from_static(b"hello"));
        let out = apply_edit(&held(), edited, CAP).expect("the same host in another spelling");
        assert_eq!(out.request.authority, held().authority);
    }

    #[test]
    fn method_must_be_uppercase_letters() {
        // Kleinbuchstaben und sonst nichts: Der Buchstabe allein reicht nicht,
        // sonst ginge `post` durch und der Upstream saehe eine andere Methode
        // als die, ueber die entschieden wurde.
        let mut edited = held();
        edited.method = Method::from_bytes(b"post").expect("a token http accepts");
        let error = apply_edit(&held(), edited, CAP).expect_err("a lowercase method is EDIT_002");
        assert_eq!(error.code.as_str(), "EDIT_002");
        assert!(error.why.contains("\"post\""), "{}", error.why);
    }

    #[test]
    fn method_with_a_digit_is_refused() {
        let mut edited = held();
        edited.method = Method::from_bytes(b"P0ST").expect("a token http accepts");
        let error = apply_edit(&held(), edited, CAP).expect_err("a digit is EDIT_002");
        assert_eq!(error.code.as_str(), "EDIT_002");
    }

    #[test]
    fn empty_method_is_refused() {
        // `Method` selbst laesst sich nicht leer bauen; geprueft wird die
        // Regel, die `apply_edit` anwendet.
        assert!(super::check_method("").is_err());
        assert!(super::check_method(&"A".repeat(17)).is_err());
        assert!(super::check_method(&"A".repeat(16)).is_ok());
    }

    #[test]
    fn path_without_leading_slash_rejected() {
        let edited = request("api.github.com", 443, Method::POST, "repos", b"hello");
        let error = apply_edit(&held(), edited, CAP).expect_err("a relative path is EDIT_003");
        assert_eq!(error.code.as_str(), "EDIT_003");
        assert!(error.why.contains("`/`"), "{}", error.why);
    }

    #[test]
    fn path_with_a_space_rejected() {
        let edited = request("api.github.com", 443, Method::POST, "/a b", b"hello");
        let error = apply_edit(&held(), edited, CAP).expect_err("a space is EDIT_003");
        assert_eq!(error.code.as_str(), "EDIT_003");
    }

    #[test]
    fn content_length_recomputed() {
        // 17 Bytes UTF-8, 16 Zeichen: das `ü` kostet zwei Bytes.
        let body = "Grüsse an Berlin".as_bytes();
        assert_eq!(body.len(), 17);
        assert_eq!("Grüsse an Berlin".chars().count(), 16);
        let edited = request("api.github.com", 443, Method::POST, "/repos", body);
        let out = apply_edit(&held(), edited, CAP).expect("a valid edit");
        assert_eq!(content_length(&out).as_deref(), Some("17"));
        assert_eq!(out.body.len(), 17);
    }

    #[test]
    fn transfer_encoding_removed() {
        let edited = with_header(
            request("api.github.com", 443, Method::POST, "/repos", b"hello"),
            "transfer-encoding",
            "chunked",
        );
        let out = apply_edit(&held(), edited, CAP).expect("a valid edit");
        assert!(out.request.headers.get("transfer-encoding").is_none());
        // Beides zusammen waere ein HTTP-Fehler (RFC 9112 6.1); der Body war
        // gechunkt, also muss jetzt eine Laenge dastehen.
        assert_eq!(content_length(&out).as_deref(), Some("5"));
    }

    #[test]
    fn content_encoding_removed() {
        let edited = with_header(
            request("api.github.com", 443, Method::POST, "/repos", b"hello"),
            "content-encoding",
            "gzip",
        );
        let out = apply_edit(&held(), edited, CAP).expect("a valid edit");
        assert!(out.request.headers.get("content-encoding").is_none());
    }

    #[test]
    fn expect_removed() {
        let edited = with_header(
            request("api.github.com", 443, Method::POST, "/repos", b"hello"),
            "expect",
            "100-continue",
        );
        let out = apply_edit(&held(), edited, CAP).expect("a valid edit");
        assert!(out.request.headers.get("expect").is_none());
    }

    #[test]
    fn host_comes_from_the_held_request() {
        // Ohne den Standard-Port, genau wie `upstream::host_header` ihn auf den
        // Draht setzt: Stuende hier `:443` und draussen nicht, zeigte die
        // Aufzeichnung eine Anfrage, die so nie hinausging.
        let edited = with_header(
            request("api.github.com", 443, Method::POST, "/repos", b"hello"),
            "host",
            "evil.io",
        );
        let out = apply_edit(&held(), edited, CAP).expect("a valid edit");
        assert_eq!(header(&out, "host").as_deref(), Some("api.github.com"));
    }

    #[test]
    fn a_host_on_another_port_keeps_it() {
        let held = request("api.github.com", 8443, Method::POST, "/repos", b"hello");
        let edited = request("api.github.com", 8443, Method::POST, "/repos", b"hello");
        let out = apply_edit(&held, edited, CAP).expect("a valid edit");
        assert_eq!(header(&out, "host").as_deref(), Some("api.github.com:8443"));
    }

    #[test]
    fn other_headers_survive() {
        let edited = with_header(
            request("api.github.com", 443, Method::POST, "/repos", b"hello"),
            "content-type",
            "application/json",
        );
        let out = apply_edit(&held(), edited, CAP).expect("a valid edit");
        assert_eq!(
            out.request
                .headers
                .get("content-type")
                .unwrap()
                .to_str()
                .unwrap(),
            "application/json"
        );
        assert_eq!(DAEMON_OWNED_HEADERS.len(), 5);
    }

    #[test]
    fn content_type_of_the_edit_wins() {
        // Geprueft wird die **Kopfzeile**, nicht `BodyRef.content_type`: Was
        // zum Ziel geht, baut `upstream::build_outgoing` allein aus
        // `request.headers`, und ein Wert, der nur im Verweis steht, erreicht
        // den Draht nie.
        let mut held = held();
        held.headers.insert(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("application/x-www-form-urlencoded"),
        );
        held.body.content_type = Some("application/x-www-form-urlencoded".to_owned());
        let mut edited = with_header(
            request("api.github.com", 443, Method::POST, "/repos", b"{}"),
            "content-type",
            "application/json",
        );
        edited.body.content_type = Some("application/json".to_owned());
        let out = apply_edit(&held, edited, CAP).expect("a valid edit");
        assert_eq!(
            header(&out, "content-type").as_deref(),
            Some("application/json")
        );
        assert_eq!(
            out.request.body.content_type.as_deref(),
            Some("application/json")
        );
    }

    #[test]
    fn a_deleted_content_type_stays_deleted() {
        // Der Mensch hat die Zeile `Content-Type` gelöscht und den Rumpf
        // geleert. Der Typ der gehaltenen Anfrage darf nicht zurückkommen:
        // Das machte eine Entscheidung rückgängig, die er getroffen hat.
        let mut held = held();
        held.headers.insert(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("application/json"),
        );
        held.body.content_type = Some("application/json".to_owned());
        let edited = request("api.github.com", 443, Method::POST, "/repos", b"");
        assert!(edited.headers.get("content-type").is_none());
        let out = apply_edit(&held, edited, CAP).expect("a valid edit");
        assert_eq!(header(&out, "content-type"), None);
        assert_eq!(out.request.body.content_type, None);
    }

    #[test]
    fn no_content_type_anywhere_invents_none() {
        let edited = request("api.github.com", 443, Method::POST, "/repos", b"{}");
        let out = apply_edit(&held(), edited, CAP).expect("a valid edit");
        assert_eq!(header(&out, "content-type"), None);
    }

    #[test]
    fn an_emptied_body_goes_out_empty() {
        // Wer den ganzen Rumpf loescht, schickt einen leeren, nie wieder den
        // urspruenglichen: eine Ersetzung, die auf den alten Rumpf zurueckfaellt,
        // schickte genau das hinaus, was der Mensch entfernt hat.
        let edited = request("api.github.com", 443, Method::POST, "/repos", b"");
        let out = apply_edit(&held(), edited, CAP).expect("a valid edit");
        assert!(out.body.is_empty());
        assert_eq!(content_length(&out).as_deref(), Some("0"));
    }

    #[test]
    fn a_detached_body_goes_out_empty() {
        // Eine Bearbeitung ohne Inhalt ist eine Bearbeitung mit leerem Rumpf,
        // nie eine mit dem alten: Auf den urspruenglichen zurueckzufallen
        // schickte genau das hinaus, was der Mensch entfernt hat.
        let mut edited = request("api.github.com", 443, Method::POST, "/repos", b"");
        edited.body = BodyRef::detached([0; 32], 0);
        let out = apply_edit(&held(), edited, CAP).expect("a valid edit");
        assert!(out.body.is_empty());
        assert_eq!(content_length(&out).as_deref(), Some("0"));
    }

    #[test]
    fn thirty_thousand_headers_do_not_stop_the_daemon() {
        // `HeaderMap::with_capacity` panikt oberhalb von rund 24 577
        // Eintraegen, und die Zahl bestimmt, wer auf dem Socket spricht. Eine
        // Anfrage darf den Daemon nicht anhalten; sie geht durch oder wird mit
        // einem Befund abgelehnt, aber sie bringt ihn nicht zum Stehen.
        let mut edited = request("api.github.com", 443, Method::POST, "/repos", b"x");
        for i in 0..30_000_u32 {
            edited
                .headers
                .append(HeaderName::from_static("x-many"), HeaderValue::from(i));
        }
        let out = apply_edit(&held(), edited, CAP).expect("a valid edit");
        assert_eq!(out.request.headers.get_all("x-many").iter().count(), 30_000);
    }

    #[test]
    fn get_with_empty_body_no_content_length() {
        let edited = request("api.github.com", 443, Method::GET, "/repos", b"");
        let out = apply_edit(&held(), edited, CAP).expect("a valid edit");
        assert_eq!(content_length(&out), None);
    }

    #[test]
    fn post_with_empty_body_says_zero() {
        let edited = request("api.github.com", 443, Method::POST, "/repos", b"");
        let out = apply_edit(&held(), edited, CAP).expect("a valid edit");
        assert_eq!(content_length(&out).as_deref(), Some("0"));
    }

    #[test]
    fn body_over_cap_rejected() {
        let edited = request("api.github.com", 443, Method::POST, "/repos", &[b'x'; 64]);
        let error = apply_edit(&held(), edited, 32).expect_err("a body over the cap is EDIT_005");
        assert_eq!(error.code.as_str(), "EDIT_005");
        assert!(error.why.contains("64 bytes"), "{}", error.why);
    }

    #[test]
    fn findings_rescanned_after_edit() {
        let scanner = Tier1Scanner::new(&FindingsSettings::default()).expect("tier 1");
        let before = "mail a@x.de iban GB82 WEST 1234 5698 7654 32";
        let after = "mail <EMAIL_1> iban GB82 WEST 1234 5698 7654 32";
        let held = request(
            "api.github.com",
            443,
            Method::POST,
            "/repos",
            before.as_bytes(),
        );
        assert_eq!(scanner.scan(&held, before.as_bytes()).findings.len(), 2);
        let edited = request(
            "api.github.com",
            443,
            Method::POST,
            "/repos",
            after.as_bytes(),
        );
        let out = apply_edit(&held, edited, CAP).expect("a valid edit");
        assert_eq!(
            remaining_findings(&scanner, &out.request, &out.body).len(),
            1
        );
    }
}
