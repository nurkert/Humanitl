//! Die Probe des LLM-Endpunkts: läuft auf dem Host, ändert nichts (HUM-039).
//!
//! Der Setup-Bildschirm hat ein Feld für `llm.endpoint` und daneben einen
//! Knopf. Was der Knopf auslöst, steht hier: zwei `GET`-Anfragen an die Adresse,
//! die der Mensch gerade eingetippt hat, und daraus die Antwort auf drei
//! Fragen — ist da etwas, was ist es, und welche Modelle bietet es an.
//!
//! # Was die Probe nicht tut
//!
//! - **Sie ändert nichts.** Nur `GET`, nur auf zwei feste Pfade
//!   ([`OLLAMA_TAGS_PATH`], [`OPENAI_MODELS_PATH`]). Beide sind Auskunft;
//!   keiner legt etwas an, lädt etwas nach oder löscht etwas.
//! - **Sie folgt keiner Weiterleitung.** Ein `3xx` ist eine Antwort, die nicht
//!   die gesuchte API ist, und nichts weiter. Wer der Umleitung folgte, fragte
//!   eine Adresse, die der Mensch nicht eingetippt hat.
//! - **Sie schickt keine Zugangsdaten und keine Cookies.** Die einzige
//!   Kopfzeile, die sie setzt, ist `accept: application/json` (und der
//!   `Host`-Kopf, den [`Upstream`] ohnehin aus der Authority bildet).
//! - **Sie läuft nie aus der Sandbox.** Sie hängt am gRPC-Dienst des Daemons,
//!   also am Host, und ihr Ergebnis geht an die Oberfläche, nicht an den
//!   Agenten.
//! - **Sie behauptet nichts, was sie nicht gemessen hat.** Ein Endpunkt, der
//!   schweigt, ist `LLM_001` samt dem `curl`, mit dem der Mensch dasselbe
//!   nachprüfen kann — kein Fehler, den er gemacht hätte. Ein Endpunkt, der
//!   antwortet, aber unbekannt bleibt, ist [`LlmFlavor::Unknown`] mit einer
//!   leeren Liste, nie eine erfundene.
//!
//! # Auflösung
//!
//! Die Probe geht über denselben [`Upstream`] wie der Proxy und damit über
//! denselben `Resolver`-Port und denselben `Egress`-Port. Ein Name wie
//! `ollama.lan` verhält sich in der Probe deshalb genauso wie später im
//! Proxy-Pfad, und `resolver.overrides` gilt für beide (HUM-039, Fallstricke).
//!
//! Der Daemon gibt der Probe dafür einen **eigenen** Resolver-Stapel mit
//! derselben Konfiguration, nicht den des Proxys. Der Grund ist ADR-006: Der
//! Zähler des Proxy-Resolvers ist der Zeuge dafür, dass vor einer Freigabe
//! nichts aufgelöst wird (Escape-Test 3). Eine Auflösung, die der Mensch selbst
//! angestoßen hat, gehört nicht in diesen Zähler — sie würde den Beweis
//! verwässern, den er führen soll.
//!
//! # Private Adressen
//!
//! Die Probe erlaubt sie: Ein lokales Modell läuft auf einer privaten Adresse,
//! und genau dafür ist der Endpunkt da. Sie merkt sich dabei, ob die Adresse
//! privat war, und legt `LLM_006` bei, wenn nicht (siehe
//! [`ProbeResult::diagnostics`]). Die Erlaubnis endet hier; sie sagt nichts
//! über den Proxy-Pfad, wo weiterhin allein die Regel entscheidet (ADR-006).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use bytes::Bytes;
use humanitl_core::diagnostics::codes::{LLM_001, LLM_002, LLM_003, LLM_006, LLM_007};
use humanitl_core::{
    Authority, BodyRef, Diagnostic, FixAction, HostName, HttpRequest, Method, Scheme, Severity,
    UpstreamError, ip_is_private,
};
use hyper::header::{HeaderName, HeaderValue};
use url::Url;

use crate::body::{self, BufferError};
use crate::upstream::Upstream;

/// Der Pfad, unter dem Ollama seine Modelle auflistet.
pub const OLLAMA_TAGS_PATH: &str = "/api/tags";

/// Der Pfad, unter dem eine OpenAI-kompatible API ihre Modelle auflistet.
pub const OPENAI_MODELS_PATH: &str = "/v1/models";

/// Die Frist der Probe, wenn der Aufrufer keine nennt (`3000` ms).
pub const DEFAULT_TIMEOUT_MS: u32 = 3_000;

/// Die längste Frist, die ein Aufrufer verlangen darf (`30000` ms).
///
/// Eine Probe hält einen Task und eine ausgehende Verbindung fest, solange sie
/// läuft. Ohne Obergrenze könnte ein Aufrufer beides für Tage binden; dreißig
/// Sekunden sind das Zehnfache der Vorgabe und mehr, als ein Mensch vor einem
/// Testknopf wartet.
pub const MAX_TIMEOUT_MS: u32 = 30_000;

/// Eine Adresse in der Form, die `llm.endpoint` erwartet.
///
/// Steht im Vorschlag der Befunde, die auf `llm.endpoint` zeigen. Ein leerer
/// Vorschlag wäre schlimmer als keiner: Eine Oberfläche, die den Fix anwendet,
/// löschte damit den Endpunkt.
pub const EXAMPLE_ENDPOINT: &str = "http://192.168.1.50:11434";

/// So viele Bytes einer Antwort liest die Probe höchstens.
///
/// Eine Modellliste ist ein paar Kilobyte groß. Ein Endpunkt, der stattdessen
/// megabyteweise schickt, ist nicht die gesuchte API, und die Probe soll an ihm
/// weder hängen bleiben noch Speicher verbrauchen.
const RESPONSE_CAP_BYTES: u64 = 1 << 20;

/// Die Namensräume, die für „im eigenen Netz" stehen, ohne dass etwas
/// aufgelöst werden müsste.
const PRIVATE_SUFFIXES: &[&str] = &[".local", ".lan", ".home.arpa", ".internal"];

/// Welche Art von Server hinter dem Endpunkt steht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmFlavor {
    /// Ollama: `GET /api/tags` hat mit `models[].name` geantwortet.
    Ollama,
    /// Eine OpenAI-kompatible API: `GET /v1/models` hat mit `data[].id`
    /// geantwortet.
    OpenAiCompatible,
    /// Die Verbindung steht, aber keiner der beiden Pfade hat geantwortet.
    Unknown,
}

impl LlmFlavor {
    /// Kurzname in `snake_case`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::OpenAiCompatible => "openai_compatible",
            Self::Unknown => "unknown",
        }
    }
}

/// Was die Probe gemessen hat.
#[derive(Debug, Clone, PartialEq)]
pub struct ProbeResult {
    /// Welche API geantwortet hat.
    pub flavor: LlmFlavor,
    /// Die Modelle, die der Server nennt, in seiner Reihenfolge. Leer, wenn
    /// keine API geantwortet hat.
    pub models: Vec<String>,
    /// Wie lange die ganze Probe gedauert hat.
    ///
    /// Gemessen vom Beginn der Auflösung bis zu der Antwort, aus der das
    /// Ergebnis stammt — die Auflösung zählt also mit, und bei einem
    /// OpenAI-kompatiblen Server auch der erste Umlauf nach
    /// [`OLLAMA_TAGS_PATH`], der ins Leere ging. Das ist die Zahl, die ein
    /// Mensch am Testknopf erlebt, nicht die Antwortzeit des Modells.
    pub latency_ms: u32,
    /// Wahr, wenn der Endpunkt im eigenen Netz liegt.
    pub endpoint_is_private: bool,
    /// Befunde, die die Probe überlebt hat: `LLM_003`, wenn keine API
    /// geantwortet hat, `LLM_006`, wenn der Endpunkt nicht privat ist. Beide
    /// stehen neben einem Ergebnis, nicht an seiner Stelle.
    pub diagnostics: Vec<Diagnostic>,
}

/// Die Probe über einem [`Upstream`].
///
/// Zustandslos bis auf die Ports, die sie leiht. Der Daemon baut sie einmal und
/// hält sie am gRPC-Dienst; jede Probe ist eine eigene Verbindung.
pub struct LlmProbe {
    upstream: Upstream,
}

impl std::fmt::Debug for LlmProbe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmProbe").finish_non_exhaustive()
    }
}

impl LlmProbe {
    /// Eine Probe über diesem Weg nach draußen.
    #[must_use]
    pub const fn new(upstream: Upstream) -> Self {
        Self { upstream }
    }

    /// Fragt den Endpunkt, was er ist und welche Modelle er kennt.
    ///
    /// Die Reihenfolge ist Ollama zuerst: Ollama beantwortet auch
    /// [`OPENAI_MODELS_PATH`], und wer danach zuerst fragte, bekäme für einen
    /// Ollama-Server die Antwort „OpenAI-kompatibel" (HUM-039, Fallstricke).
    ///
    /// `timeout` gilt für die Probe als Ganzes, nicht je Anfrage. `None` heißt
    /// [`DEFAULT_TIMEOUT_MS`].
    ///
    /// # Errors
    ///
    /// [`Diagnostic`] mit `LLM_001`, wenn nichts antwortet, der Name nicht
    /// auflöst oder die Frist abläuft; mit `LLM_002`, wenn der Server `401`
    /// oder `403` sagt; mit `LLM_007`, wenn die Adresse gar keine HTTP-Adresse
    /// ist. Eine Antwort, die nur unbekannt ist, ist kein Fehler: sie kommt als
    /// [`LlmFlavor::Unknown`] samt `LLM_003` in [`ProbeResult::diagnostics`]
    /// zurück.
    pub async fn probe(
        &self,
        endpoint: &Url,
        timeout: Option<Duration>,
    ) -> Result<ProbeResult, Diagnostic> {
        let timeout = timeout
            .unwrap_or(Duration::from_millis(u64::from(DEFAULT_TIMEOUT_MS)))
            .min(Duration::from_millis(u64::from(MAX_TIMEOUT_MS)));
        let target = Target::parse(endpoint)?;
        let started = Instant::now();

        // Wer schon geantwortet hat, ist nicht unerreichbar. Ohne diese Marke
        // meldete eine Frist, die beim zweiten Pfad ablief, „konnte nicht
        // erreicht werden" — über einen Server, der nachweislich geantwortet
        // hat (`backlog/CONVENTIONS.md` 4.13: nie mehr behaupten als bewiesen).
        let answered = Arc::new(AtomicBool::new(false));
        let outcome = tokio::time::timeout(timeout, self.run(&target, &answered))
            .await
            .unwrap_or_else(|_elapsed| {
                let millis = timeout.as_millis();
                Err(if answered.load(Ordering::SeqCst) {
                    Failure::Incomplete(format!(
                        "the server answered, but the probe was not finished within {millis} ms"
                    ))
                } else {
                    Failure::Unreachable(format!("no answer within {millis} ms"))
                })
            })
            .map_err(|failure| failure.into_diagnostic(&target))?;

        let latency_ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
        let mut diagnostics = Vec::new();
        if !outcome.endpoint_is_private {
            diagnostics.push(not_private(endpoint));
        }
        if outcome.flavor == LlmFlavor::Unknown {
            diagnostics.push(unknown_api(endpoint));
        }
        Ok(ProbeResult {
            flavor: outcome.flavor,
            models: outcome.models,
            latency_ms,
            endpoint_is_private: outcome.endpoint_is_private,
            diagnostics,
        })
    }

    /// Die zwei Anfragen, ohne Frist und ohne Zeitmessung.
    async fn run(&self, target: &Target, answered: &AtomicBool) -> Result<Outcome, Failure> {
        // Genau eine Auflösung für die ganze Probe, und die Adresse wird für
        // beide Anfragen angeheftet: Der Endpunkt, über den die Oberfläche
        // gleich etwas behauptet, ist derselbe, der geantwortet hat.
        let ip = self
            .upstream
            .resolve(&target.authority, true)
            .await
            .map_err(|error| match error {
                UpstreamError::Dns => Failure::Unreachable(format!(
                    "{} does not resolve on this machine",
                    target.authority.host
                )),
                other => Failure::Unreachable(other.to_string()),
            })?;
        let endpoint_is_private = target.is_private(ip);

        let tags = self.get(target, OLLAMA_TAGS_PATH, ip, answered).await?;
        if let Some(models) = tags.as_ref().and_then(|body| ollama_models(body)) {
            return Ok(Outcome {
                flavor: LlmFlavor::Ollama,
                models,
                endpoint_is_private,
            });
        }

        let models_body = self.get(target, OPENAI_MODELS_PATH, ip, answered).await?;
        if let Some(models) = models_body.as_ref().and_then(|body| openai_models(body)) {
            return Ok(Outcome {
                flavor: LlmFlavor::OpenAiCompatible,
                models,
                endpoint_is_private,
            });
        }

        // Verbindung steht, Antwort da, aber keine der beiden Formen. Das ist
        // ein Ergebnis, kein Fehler: Der Mensch erfährt, dass etwas da ist und
        // dass es nicht das Gesuchte ist.
        Ok(Outcome {
            flavor: LlmFlavor::Unknown,
            models: Vec::new(),
            endpoint_is_private,
        })
    }

    /// Ein `GET` auf einen der beiden Pfade.
    ///
    /// `Ok(Some(body))` heißt `200` mit einem gelesenen Körper, `Ok(None)`
    /// jede andere Antwort, die der Server geschickt hat (`404`, `3xx`, `5xx`).
    /// `Err` gibt es nur, wenn es keine Antwort gab oder der Server eine
    /// Anmeldung verlangt.
    async fn get(
        &self,
        target: &Target,
        suffix: &str,
        ip: std::net::IpAddr,
        answered: &AtomicBool,
    ) -> Result<Option<Bytes>, Failure> {
        let request = target.request(suffix);
        let response = self
            .upstream
            .forward_to(&request, Bytes::new(), ip)
            .await
            .map_err(|error| {
                Failure::Unreachable(format!(
                    "{}{suffix} did not answer: {error}",
                    target.authority
                ))
            })?;

        answered.store(true, Ordering::SeqCst);
        let status = response.status().as_u16();
        if status == 401 || status == 403 {
            return Err(Failure::NeedsAuth(status));
        }
        if status != 200 {
            return Ok(None);
        }
        // Die Uhr der Probe ist die äußere ([`LlmProbe::probe`] legt sie um
        // beide Anfragen); die Frist je Stück steht deshalb auf
        // [`MAX_TIMEOUT_MS`] und kann nie vor ihr greifen. Zwei Uhren über
        // derselben Spanne wären der Fehler aus HUM-101.
        let idle = Duration::from_millis(u64::from(MAX_TIMEOUT_MS));
        match body::buffer(response.into_body(), RESPONSE_CAP_BYTES, idle).await {
            Ok(bytes) => Ok(Some(bytes)),
            // Ein Körper über dem Cap, ein abgerissener oder ein
            // stehengebliebener Strom ist keine Modellliste. Die Probe zählt
            // ihn wie eine fremde Antwort.
            Err(BufferError::Cap | BufferError::Read | BufferError::Idle) => Ok(None),
        }
    }
}

/// Das Ergebnis der zwei Anfragen, noch ohne Zeit und Befunde.
struct Outcome {
    flavor: LlmFlavor,
    models: Vec<String>,
    endpoint_is_private: bool,
}

/// Was die Probe abbrechen lässt.
enum Failure {
    /// Nichts geantwortet: kein Name, keine Verbindung, keine Zeit mehr.
    Unreachable(String),
    /// Es kam eine Antwort, aber die Probe wurde nicht fertig.
    Incomplete(String),
    /// Der Server verlangt eine Anmeldung.
    NeedsAuth(u16),
}

impl Failure {
    /// Der Befund, den der Mensch zu sehen bekommt.
    ///
    /// Der Vorschlag zu `LLM_001` ist der `curl`, mit dem sich dieselbe Frage
    /// von Hand stellen lässt: Ein Endpunkt, der schweigt, ist kein Fehler des
    /// Nutzers, und die nächste sinnvolle Handlung ist nachzusehen, ob er von
    /// diesem Rechner aus überhaupt erreichbar ist.
    fn into_diagnostic(self, target: &Target) -> Diagnostic {
        match self {
            Self::Unreachable(detail) => Diagnostic::builder(LLM_001, Severity::Blocking)
                .why(format!(
                    "Humanitl could not reach {} from this machine ({detail}). The agent will \
                     not be able to talk to the model.",
                    target.authority
                ))
                .fix(FixAction::CopyCommand(format!(
                    "curl -sS {}",
                    target.url(OLLAMA_TAGS_PATH)
                )))
                .build(),
            // Erreichbar war der Server; nur fertig wurde die Probe nicht.
            // Derselbe Code, weil der Agent so oder so nicht arbeiten kann,
            // aber ein `why`, das die Beobachtung nicht überzeichnet.
            Self::Incomplete(detail) => Diagnostic::builder(LLM_001, Severity::Blocking)
                .why(format!(
                    "{} answered, but not in time ({detail}). Raise the timeout or check what \
                     the server is doing.",
                    target.authority
                ))
                .fix(FixAction::CopyCommand(format!(
                    "curl -sS {}",
                    target.url(OLLAMA_TAGS_PATH)
                )))
                .build(),
            Self::NeedsAuth(status) => Diagnostic::builder(LLM_002, Severity::Error)
                .why(format!(
                    "the LLM server at {} answered {status}. It requires authentication that \
                     Humanitl does not send in the MVP.",
                    target.authority
                ))
                .build(),
        }
    }
}

/// Alles, was die Probe über den Endpunkt weiß, bevor sie ihn fragt.
#[derive(Debug)]
struct Target {
    authority: Authority,
    scheme: Scheme,
    /// Der Pfad der API-Wurzel, ohne abschließenden `/`; leer für die Wurzel.
    root: String,
}

impl Target {
    /// Liest den Endpunkt.
    ///
    /// Ein abschließendes `/v1` gehört zur OpenAI-kompatiblen Oberfläche und
    /// nicht zur Wurzel der API: `http://host:1/v1` und `http://host:1` meinen
    /// denselben Server, und die Probe fragt beide Male `…/api/tags` und
    /// `…/v1/models`. So passt sie zu `OpenCodeAdapter::base_url`, das an
    /// dieselbe Wurzel wieder ein `/v1` hängt.
    fn parse(endpoint: &Url) -> Result<Self, Diagnostic> {
        let scheme = match endpoint.scheme() {
            "http" => Scheme::Http,
            "https" => Scheme::Https,
            other => {
                return Err(unreadable_endpoint(format!(
                    "{endpoint} uses the scheme {other:?}; the probe speaks http and https only"
                )));
            }
        };
        let host = endpoint
            .host_str()
            .ok_or(())
            .and_then(|text| HostName::parse(text).map_err(|_err| ()))
            .map_err(|()| {
                unreadable_endpoint(format!("{endpoint} has no host that Humanitl can read"))
            })?;
        let port = endpoint.port_or_known_default().unwrap_or(match scheme {
            Scheme::Https => 443,
            _ => 80,
        });
        let root = endpoint.path().trim_end_matches('/');
        let root = root.strip_suffix("/v1").unwrap_or(root).to_owned();
        Ok(Self {
            authority: Authority::new(host, port),
            scheme,
            root,
        })
    }

    /// Wahr, wenn der Endpunkt im eigenen Netz liegt.
    ///
    /// Zwei Wege dorthin, und beide zählen: die aufgelöste Adresse (RFC 1918,
    /// Loopback, Link-Local, CGNAT) und der Name (`.local`, `.lan`,
    /// `.home.arpa`, `.internal`). Der Name allein genügt nicht — ein
    /// öffentlicher Name kann auf eine private Adresse zeigen —, aber er ist
    /// die Antwort für den Fall, dass der Nutzer sein Netz so benennt.
    fn is_private(&self, resolved: std::net::IpAddr) -> bool {
        if ip_is_private(resolved) {
            return true;
        }
        match &self.authority.host {
            HostName::Ip(ip) => ip_is_private(*ip),
            HostName::Dns(name) => {
                name == "localhost" || PRIVATE_SUFFIXES.iter().any(|suffix| name.ends_with(suffix))
            }
        }
    }

    /// Die Adresse, die die Probe fragt, als Text — für den `curl` im Vorschlag.
    fn url(&self, suffix: &str) -> String {
        format!(
            "{}://{}{}{suffix}",
            self.scheme.as_str(),
            self.authority,
            self.root
        )
    }

    /// Die Anfrage an einen der beiden Pfade.
    fn request(&self, suffix: &str) -> HttpRequest {
        let mut request = HttpRequest::new(
            Method::GET,
            self.scheme,
            self.authority.clone(),
            format!("{}{suffix}", self.root),
        );
        request.headers.insert(
            HeaderName::from_static("accept"),
            HeaderValue::from_static("application/json"),
        );
        request.body = BodyRef::empty();
        request
    }
}

/// Der Befund für einen Endpunkt außerhalb des eigenen Netzes, ohne dass etwas
/// aufgelöst würde.
///
/// [`LlmProbe`] entscheidet die Frage mit der aufgelösten Adresse **und** dem
/// Namen; hier steht nur der Name zur Verfügung. Das genügt für den Fall, auf
/// den es beim Start einer Sitzung ankommt: `--llm` baut aus dem Endpunkt eine
/// Regel in Rang 1, die nicht gehalten wird und die eigenen Block-Regeln des
/// Nutzers überholt (`AgentAdapter::llm_passthrough`). Wer dort eine fremde
/// Adresse einträgt, soll es erfahren, bevor der Agent läuft — und nicht erst,
/// wenn jemand die Probe von Hand fährt.
///
/// Aufgelöst wird dabei nichts: Ein Name verlässt den Rechner erst, wenn eine
/// Anfrage freigegeben ist (ADR-006). Deshalb sagt der Befund im Namensfall
/// ausdrücklich, worauf er sich stützt.
///
/// `None` heißt: Der Endpunkt ist nach dem Namen privat, oder er ist keine
/// lesbare Adresse — Letzteres meldet die Konfiguration schon selbst.
#[must_use]
pub fn not_private_by_name(endpoint: &Url) -> Option<Diagnostic> {
    let host = endpoint.host_str()?;
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return (!ip_is_private(ip)).then(|| not_private(endpoint));
    }
    let name = host.to_ascii_lowercase();
    if name == "localhost" || PRIVATE_SUFFIXES.iter().any(|suffix| name.ends_with(suffix)) {
        return None;
    }
    Some(
        Diagnostic::builder(LLM_006, Severity::Info)
            .why(format!(
                "{endpoint} is not one of the private forms — neither an address in a private \
                 network nor localhost or a name under {}. Judged from the name alone; nothing \
                 was resolved. Traffic to this address bypasses the queue, so only put a machine \
                 you control here.",
                PRIVATE_SUFFIXES.join(", ")
            ))
            .fix(FixAction::ChangeSetting {
                key: "llm.endpoint".to_owned(),
                value: EXAMPLE_ENDPOINT.to_owned(),
            })
            .build(),
    )
}

/// Der Befund für einen Endpunkt außerhalb des eigenen Netzes.
fn not_private(endpoint: &Url) -> Diagnostic {
    Diagnostic::builder(LLM_006, Severity::Info)
        .why(format!(
            "{endpoint} is not on a private network. Traffic to this address bypasses the queue, \
             so only put a machine you control here."
        ))
        .fix(FixAction::ChangeSetting {
            key: "llm.endpoint".to_owned(),
            value: EXAMPLE_ENDPOINT.to_owned(),
        })
        .build()
}

/// Der Befund für eine Adresse, die sich gar nicht als HTTP-Adresse lesen lässt.
///
/// Eigener Code, nicht `LLM_001` und nicht `LLM_003`: Beide behaupten eine
/// Beobachtung am Endpunkt, und hier hat nichts stattgefunden — es wurde weder
/// aufgelöst noch verbunden (HUM-039).
fn unreadable_endpoint(why: String) -> Diagnostic {
    Diagnostic::builder(LLM_007, Severity::Error)
        .why(why)
        .fix(FixAction::ChangeSetting {
            key: "llm.endpoint".to_owned(),
            value: EXAMPLE_ENDPOINT.to_owned(),
        })
        .build()
}

/// Der Befund für einen Endpunkt, der antwortet, aber keine bekannte API ist.
fn unknown_api(endpoint: &Url) -> Diagnostic {
    Diagnostic::builder(LLM_003, Severity::Warning)
        .why(format!(
            "connected to {endpoint}, but neither {OLLAMA_TAGS_PATH} nor {OPENAI_MODELS_PATH} \
             answered. Check that the URL points at the API root, not at a chat UI."
        ))
        .fix(FixAction::ChangeSetting {
            key: "llm.endpoint".to_owned(),
            value: EXAMPLE_ENDPOINT.to_owned(),
        })
        .build()
}

/// Die Modellnamen aus einer Antwort von `GET /api/tags`.
///
/// `None`, wenn die Antwort nicht diese Form hat. Eine leere Liste ist dagegen
/// ein Ergebnis: Ollama läuft, kennt aber kein Modell.
fn ollama_models(body: &[u8]) -> Option<Vec<String>> {
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    let models = value.get("models")?.as_array()?;
    Some(
        models
            .iter()
            .filter_map(|model| model.get("name")?.as_str().map(str::to_owned))
            .collect(),
    )
}

/// Die Modell-Ids aus einer Antwort von `GET /v1/models`.
fn openai_models(body: &[u8]) -> Option<Vec<String>> {
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    let data = value.get("data")?.as_array()?;
    Some(
        data.iter()
            .filter_map(|model| model.get("id")?.as_str().map(str::to_owned))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::net::{IpAddr, Ipv4Addr};

    use url::Url;

    use super::{LlmFlavor, Target, ollama_models, openai_models};

    #[test]
    fn the_api_root_drops_a_trailing_v1() {
        for (endpoint, expected) in [
            ("http://192.168.1.50:11434", ""),
            ("http://192.168.1.50:11434/", ""),
            ("http://host:1/v1", ""),
            ("http://host:1/v1/", ""),
            ("http://host:1/openai", "/openai"),
            ("http://host:1/openai/v1", "/openai"),
        ] {
            let target = Target::parse(&Url::parse(endpoint).unwrap()).unwrap();
            assert_eq!(target.root, expected, "for {endpoint}");
        }
    }

    #[test]
    fn the_probe_only_ever_gets_two_fixed_paths() {
        let target = Target::parse(&Url::parse("http://host:1").unwrap()).unwrap();
        for suffix in [super::OLLAMA_TAGS_PATH, super::OPENAI_MODELS_PATH] {
            let request = target.request(suffix);
            assert_eq!(request.method, humanitl_core::Method::GET);
            assert_eq!(request.path_and_query, suffix);
            assert_eq!(request.body.size, 0);
            assert!(
                request.headers.get("cookie").is_none()
                    && request.headers.get("authorization").is_none(),
                "the probe sends no credentials"
            );
        }
    }

    /// Eine Adresse, die keine HTTP-Adresse ist, wird abgewiesen, bevor
    /// irgendetwas den Rechner verlässt — und mit einem eigenen Code.
    ///
    /// `LLM_001` und `LLM_003` behaupten beide eine Beobachtung am Endpunkt;
    /// hier hat nichts stattgefunden.
    #[test]
    fn an_address_that_is_no_http_url_is_refused_with_its_own_code() {
        for text in ["file:///etc/passwd", "ftp://model.lan/", "ws://model.lan:1"] {
            let Ok(url) = Url::parse(text) else { continue };
            let diagnostic = Target::parse(&url).unwrap_err();
            assert_eq!(diagnostic.code.as_str(), "LLM_007", "for {text}");
            match diagnostic.fix {
                Some(humanitl_core::FixAction::ChangeSetting { key, value }) => {
                    assert_eq!(key, "llm.endpoint");
                    assert_eq!(
                        value,
                        super::EXAMPLE_ENDPOINT,
                        "an empty value would delete the endpoint the user typed"
                    );
                }
                other => panic!("expected a concrete suggestion, got {other:?}"),
            }
        }
    }

    /// Kein Befund, der auf `llm.endpoint` zeigt, schlägt einen leeren Wert
    /// vor. Eine Oberfläche, die den Fix anwendet, löschte damit den Endpunkt.
    #[test]
    fn no_fix_proposes_an_empty_endpoint() {
        let url = Url::parse("http://model.example:11434").unwrap();
        for diagnostic in [super::not_private(&url), super::unknown_api(&url)] {
            match diagnostic.fix {
                Some(humanitl_core::FixAction::ChangeSetting { key, value }) => {
                    assert_eq!(key, "llm.endpoint");
                    assert!(!value.is_empty(), "{} proposes nothing", diagnostic.code);
                }
                other => panic!("expected a setting change, got {other:?}"),
            }
        }
    }

    #[test]
    fn a_private_endpoint_is_recognised_by_address_and_by_name() {
        let public = IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34));
        let private = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 50));

        let by_address = Target::parse(&Url::parse("http://model.example:11434").unwrap()).unwrap();
        assert!(by_address.is_private(private));
        assert!(!by_address.is_private(public));

        let by_name = Target::parse(&Url::parse("http://ollama.lan:11434").unwrap()).unwrap();
        assert!(
            by_name.is_private(public),
            "a name in the home namespace counts even when the address does not"
        );

        let literal = Target::parse(&Url::parse("http://192.168.1.50:11434").unwrap()).unwrap();
        assert!(literal.is_private(private));
    }

    #[test]
    fn model_lists_are_read_from_the_two_known_shapes() {
        let tags = br#"{"models":[{"name":"llama3:8b"},{"name":"qwen:7b"}]}"#;
        assert_eq!(
            ollama_models(tags),
            Some(vec!["llama3:8b".to_owned(), "qwen:7b".to_owned()])
        );
        let openai = br#"{"object":"list","data":[{"id":"gpt-oss"},{"id":"phi-4"}]}"#;
        assert_eq!(
            openai_models(openai),
            Some(vec!["gpt-oss".to_owned(), "phi-4".to_owned()])
        );

        assert_eq!(ollama_models(openai), None, "the shapes are not confused");
        assert_eq!(openai_models(tags), None);
        assert_eq!(ollama_models(b"<html>a chat ui</html>"), None);
        assert_eq!(
            ollama_models(br#"{"models":[]}"#),
            Some(Vec::new()),
            "a server without models is still a server"
        );
    }

    #[test]
    fn flavor_names_are_stable() {
        assert_eq!(LlmFlavor::Ollama.as_str(), "ollama");
        assert_eq!(LlmFlavor::OpenAiCompatible.as_str(), "openai_compatible");
        assert_eq!(LlmFlavor::Unknown.as_str(), "unknown");
    }
}
