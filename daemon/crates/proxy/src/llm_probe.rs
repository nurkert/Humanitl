//! Die Probe des LLM-Endpunkts: läuft auf dem Host, ändert nichts (HUM-039).
//!
//! Der Setup-Bildschirm hat ein Feld für `llm.endpoint` und daneben einen
//! Knopf. Was der Knopf auslöst, steht hier: höchstens zwei `GET`-Anfragen an
//! die Adresse, die der Mensch gerade eingetippt hat, und daraus die Antwort
//! auf drei Fragen — ist da etwas, was ist es, und welche Modelle bietet es an.
//!
//! # Welche Adressen sie fragt
//!
//! Der Endpunkt ist die Basis der OpenAI-kompatiblen API, gelesen wie
//! `OpenCode` eine `baseURL` liest ([`humanitl_config::llm_endpoint`],
//! HUM-261). Die Probe fragt `<Basis>`[`OPENAI_MODELS`] — bei
//! `http://host:11434` also `/v1/models`, bei `https://host/api/v1/` also
//! `/api/v1/models`. Davor fragt sie [`OLLAMA_TAGS_PATH`], aber nur, wenn der
//! Endpunkt an der Wurzel des Servers sitzt: Unter einem Pfad-Präfix gibt es
//! die native Ollama-API nicht, und die Wurzel des Hosts hat der Mensch nicht
//! eingetragen.
//!
//! # Was die Probe nicht tut
//!
//! - **Sie ändert nichts.** Nur `GET`, nur auf diese beiden Pfade. Beide sind
//!   Auskunft; keiner legt etwas an, lädt etwas nach oder löscht etwas.
//! - **Sie folgt keiner Weiterleitung.** Ein `3xx` ist eine Antwort, die nicht
//!   die gesuchte API ist, und nichts weiter. Wer der Umleitung folgte, fragte
//!   eine Adresse, die der Mensch nicht eingetippt hat.
//! - **Sie schickt keine Cookies und Zugangsdaten nur auf Verlangen.** Sie
//!   setzt `accept: application/json` (und den `Host`-Kopf, den [`Upstream`]
//!   ohnehin aus der Authority bildet). Einen Zugangsschlüssel schickt sie nur,
//!   wenn der Aufrufer ihr einen gibt ([`LlmProbe::probe_with_key`], HUM-140),
//!   und dann als `Authorization: Bearer` an genau diesen Endpunkt, mit der
//!   Kopfzeile als empfindlich markiert, damit `hyper` sie nicht in Logs
//!   zeigt. Wem der Daemon den hinterlegten Schlüssel gibt, entscheidet der
//!   gRPC-Dienst, nicht die Probe.
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
use humanitl_config::llm_endpoint;
use humanitl_config::secret::ApiKey;
use humanitl_core::diagnostics::codes::{LLM_001, LLM_002, LLM_003, LLM_006, LLM_007};
use humanitl_core::shell::shell_word;
use humanitl_core::{
    Authority, BodyRef, Diagnostic, FixAction, HostName, HttpRequest, Method, Scheme, Severity,
    UpstreamError, ip_is_private,
};
use hyper::header::{HeaderName, HeaderValue};
use url::Url;

use crate::body::{self, BufferError};
use crate::trust::{self, TrustSource};
use crate::upstream::Upstream;

/// Der Pfad, unter dem Ollama seine Modelle auflistet, immer an der Wurzel
/// des Servers ([`llm_endpoint::OLLAMA_TAGS_PATH`]).
pub const OLLAMA_TAGS_PATH: &str = llm_endpoint::OLLAMA_TAGS_PATH;

/// Der Endpunkt, unter dem eine OpenAI-kompatible API ihre Modelle auflistet,
/// relativ zu ihrer Basis ([`llm_endpoint::OPENAI_MODELS`]).
pub const OPENAI_MODELS: &str = llm_endpoint::OPENAI_MODELS;

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
    /// Eine OpenAI-kompatible API: `GET <Basis>/models` hat mit `data[].id`
    /// geantwortet.
    OpenAiCompatible,
    /// Die Verbindung steht, aber kein gefragter Pfad hat geantwortet.
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
    /// Wahr, wenn die Probe einen Zugangsschlüssel mitgeschickt hat.
    pub key_sent: bool,
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
    /// `/v1/models`, und wer danach zuerst fragte, bekäme für einen
    /// Ollama-Server die Antwort „OpenAI-kompatibel" (HUM-039, Fallstricke).
    /// Ein Endpunkt mit Pfad-Präfix bekommt nur die zweite Frage (HUM-261).
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
        self.probe_with_key(endpoint, timeout, None).await
    }

    /// Wie [`LlmProbe::probe`], mit einem Zugangsschlüssel (HUM-140).
    ///
    /// Beide Anfragen tragen dann `Authorization: Bearer <key>`. Antwortet der
    /// Endpunkt trotzdem mit `401` oder `403`, sagt `LLM_002`, dass er den
    /// Schlüssel abgelehnt hat, statt dass einer fehlte.
    ///
    /// # Errors
    ///
    /// Wie [`LlmProbe::probe`].
    pub async fn probe_with_key(
        &self,
        endpoint: &Url,
        timeout: Option<Duration>,
        key: Option<&ApiKey>,
    ) -> Result<ProbeResult, Diagnostic> {
        let timeout = timeout
            .unwrap_or(Duration::from_millis(u64::from(DEFAULT_TIMEOUT_MS)))
            .min(Duration::from_millis(u64::from(MAX_TIMEOUT_MS)));
        let mut target = Target::parse(endpoint)?;
        target.authorization = key
            .map(|key| HeaderValue::from_str(&format!("Bearer {}", key.expose())))
            .transpose()
            .map_err(|_invalid| {
                // `ApiKey::new` lässt nur sichtbare Zeichen ohne Leerraum zu;
                // was hier scheitert, wäre kein Wert einer Kopfzeile.
                Diagnostic::builder(LLM_002, Severity::Error)
                    .why(
                        "the stored access key is not a valid header value, so it was not sent"
                            .to_owned(),
                    )
                    .fix(FixAction::CopyCommand("humanitl llm key set".to_owned()))
                    .build()
            })?
            .map(|mut value| {
                value.set_sensitive(true);
                value
            });
        let started = Instant::now();

        // Wer schon geantwortet hat, ist nicht unerreichbar. Ohne diese Marke
        // meldete eine Frist, die beim zweiten Pfad ablief, „konnte nicht
        // erreicht werden" — über einen Server, der nachweislich geantwortet
        // hat (`backlog/CONVENTIONS.md` 4.13: nie mehr behaupten als bewiesen).
        // Dazu der Pfad, der gerade gefragt wird: Läuft die Frist ab, nennt
        // der Befund genau ihn und keinen, der nie gefragt wurde (HUM-261).
        let progress = Arc::new(Progress::default());
        let outcome = tokio::time::timeout(timeout, self.run(&target, &progress))
            .await
            .unwrap_or_else(|_elapsed| {
                let millis = timeout.as_millis();
                let path = progress.attempted();
                Err(if progress.answered.load(Ordering::SeqCst) {
                    Failure::Incomplete {
                        detail: format!(
                            "the server answered, but the probe was not finished within {millis} \
                             ms"
                        ),
                        path,
                    }
                } else {
                    Failure::Unreachable {
                        detail: format!("no answer within {millis} ms"),
                        path,
                    }
                })
            })
            .map_err(|failure| failure.into_diagnostic(&target))?;

        let latency_ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
        let mut diagnostics = Vec::new();
        if !outcome.endpoint_is_private {
            diagnostics.push(not_private(endpoint));
        }
        if outcome.flavor == LlmFlavor::Unknown {
            diagnostics.push(unknown_api(&target));
        }
        Ok(ProbeResult {
            flavor: outcome.flavor,
            models: outcome.models,
            latency_ms,
            endpoint_is_private: outcome.endpoint_is_private,
            diagnostics,
            key_sent: target.authorization.is_some(),
        })
    }

    /// Die zwei Anfragen, ohne Frist und ohne Zeitmessung.
    async fn run(&self, target: &Target, progress: &Progress) -> Result<Outcome, Failure> {
        // Genau eine Auflösung für die ganze Probe, und die Adresse wird für
        // beide Anfragen angeheftet: Der Endpunkt, über den die Oberfläche
        // gleich etwas behauptet, ist derselbe, der geantwortet hat.
        let ip = self
            .upstream
            .resolve(&target.authority, true)
            .await
            .map_err(|error| Failure::Unreachable {
                detail: match error {
                    UpstreamError::Dns => {
                        format!("{} does not resolve on this machine", target.authority.host)
                    }
                    other => other.to_string(),
                },
                // Vor jeder Anfrage gescheitert: Kein Pfad wurde gefragt.
                path: None,
            })?;
        let endpoint_is_private = target.is_private(ip);

        // Ein `404` von hier ist kein Abbruch: `get` gibt dann `None`, und die
        // Probe fragt weiter nach der OpenAI-kompatiblen Liste.
        if target.native_ollama {
            let tags = self.get(target, OLLAMA_TAGS_PATH, ip, progress).await?;
            if let Some(models) = tags.as_ref().and_then(|body| ollama_models(body)) {
                return Ok(Outcome {
                    flavor: LlmFlavor::Ollama,
                    models,
                    endpoint_is_private,
                });
            }
        }

        let models_body = self.get(target, &target.models_path, ip, progress).await?;
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

    /// Ein `GET` auf einen der beiden Pfade, `path` absolut ab der Wurzel des
    /// Servers.
    ///
    /// `Ok(Some(body))` heißt `200` mit einem gelesenen Körper, `Ok(None)`
    /// jede andere Antwort, die der Server geschickt hat (`404`, `3xx`, `5xx`).
    /// `Err` gibt es nur, wenn es keine Antwort gab oder der Server eine
    /// Anmeldung verlangt.
    async fn get(
        &self,
        target: &Target,
        path: &str,
        ip: std::net::IpAddr,
        progress: &Progress,
    ) -> Result<Option<Bytes>, Failure> {
        progress.attempt(path);
        let request = target.request(path);
        let response = self
            .upstream
            .forward_to(&request, Bytes::new(), ip)
            .await
            .map_err(|failure| match failure.untrusted_issuer {
                // Der Server war da, nur seinem Zertifikat traut dieser
                // Rechner nicht. Das `why` sagt das, statt „nicht erreichbar"
                // zu behaupten (HUM-257).
                Some(source) => Failure::Untrusted(source),
                None => Failure::Unreachable {
                    detail: format!("{} did not answer: {failure}", target.url(path)),
                    path: Some(path.to_owned()),
                },
            })?;

        progress.answered.store(true, Ordering::SeqCst);
        let status = response.status().as_u16();
        if status == 401 || status == 403 {
            return Err(Failure::NeedsAuth {
                status,
                url: target.url(path),
            });
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
            Err(BufferError::Cap | BufferError::Read | BufferError::Idle | BufferError::Budget) => {
                Ok(None)
            }
        }
    }
}

/// Wie weit die Probe gekommen ist, geteilt mit der Frist um sie herum.
#[derive(Default)]
struct Progress {
    /// Wahr, sobald der Server auf irgendeine Anfrage geantwortet hat.
    answered: AtomicBool,
    /// Der Pfad der Anfrage, die zuletzt begonnen hat; `None` vor der ersten.
    attempted: std::sync::Mutex<Option<String>>,
}

impl Progress {
    /// Merkt sich, dass jetzt dieser Pfad gefragt wird.
    fn attempt(&self, path: &str) {
        if let Ok(mut attempted) = self.attempted.lock() {
            *attempted = Some(path.to_owned());
        }
    }

    /// Der zuletzt gefragte Pfad.
    fn attempted(&self) -> Option<String> {
        self.attempted
            .lock()
            .ok()
            .and_then(|attempted| attempted.clone())
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
    Unreachable {
        /// Was genau fehlte.
        detail: String,
        /// Der Pfad, der dabei gefragt wurde; `None`, wenn es gar nicht erst
        /// zu einer Anfrage kam (Auflösung).
        path: Option<String>,
    },
    /// Es kam eine Antwort, aber die Probe wurde nicht fertig.
    Incomplete {
        /// Was genau fehlte.
        detail: String,
        /// Der Pfad, der beim Ablauf der Frist gefragt wurde.
        path: Option<String>,
    },
    /// Der Server verlangt eine Anmeldung.
    NeedsAuth {
        /// `401` oder `403`.
        status: u16,
        /// Die Adresse, die so geantwortet hat, wie sie ein `curl` fragen
        /// würde.
        url: String,
    },
    /// Das Zertifikat des Servers stammt von einer Stelle, der dieser Rechner
    /// nicht vertraut (HUM-257).
    Untrusted(TrustSource),
}

impl Failure {
    /// Der Befund, den der Mensch zu sehen bekommt.
    ///
    /// Der Vorschlag zu `LLM_001` ist der `curl`, mit dem sich dieselbe Frage
    /// von Hand stellen lässt: Ein Endpunkt, der schweigt, ist kein Fehler des
    /// Nutzers, und die nächste sinnvolle Handlung ist nachzusehen, ob er von
    /// diesem Rechner aus überhaupt erreichbar ist. Der `curl` fragt genau den
    /// Pfad, an dem die Probe scheiterte, nie einen, den sie nicht gefragt hat
    /// (HUM-261). Scheiterte sie vor jeder Anfrage, an der Auflösung, ist der
    /// Vorschlag die Auflösung von Hand.
    fn into_diagnostic(self, target: &Target) -> Diagnostic {
        match self {
            Self::Unreachable { detail, path } => Diagnostic::builder(LLM_001, Severity::Blocking)
                .why(format!(
                    "Humanitl could not reach {} from this machine ({detail}). The agent will \
                     not be able to talk to the model.",
                    target.authority
                ))
                .fix(target.retry(path.as_deref()))
                .build(),
            // Erreichbar war der Server; nur fertig wurde die Probe nicht.
            // Derselbe Code, weil der Agent so oder so nicht arbeiten kann,
            // aber ein `why`, das die Beobachtung nicht überzeichnet.
            Self::Incomplete { detail, path } => Diagnostic::builder(LLM_001, Severity::Blocking)
                .why(format!(
                    "{} answered, but not in time ({detail}). Raise the timeout or check what \
                     the server is doing.",
                    target.authority
                ))
                .fix(target.retry(path.as_deref()))
                .build(),
            // Derselbe Code wie „nicht erreichbar", weil der Agent so oder so
            // nicht mit dem Modell reden kann; das `why` nennt die Ursache, und
            // der Vorschlag ist der Weg, die CA ins System zu bringen, nicht
            // ein `curl`, der dasselbe Scheitern noch einmal zeigt.
            Self::Untrusted(source) => {
                let (why, fix) = trust::untrusted_issuer(&target.authority.to_string(), source);
                Diagnostic::builder(LLM_001, Severity::Blocking)
                    .why(format!(
                        "{why}. The agent will not be able to talk to the model."
                    ))
                    .fix(fix)
                    .build()
            }
            Self::NeedsAuth { status, url } if target.authorization.is_some() => {
                Diagnostic::builder(LLM_002, Severity::Error)
                    .why(format!(
                        "the LLM server answered {status} at {url} to the access key that was \
                         sent; it does not accept this key."
                    ))
                    .fix(FixAction::CopyCommand("humanitl llm key set".to_owned()))
                    .build()
            }
            Self::NeedsAuth { status, url } => Diagnostic::builder(LLM_002, Severity::Error)
                .why(format!(
                    "the LLM server answered {status} at {url}. It requires an access key, and \
                     none was sent."
                ))
                .fix(FixAction::CopyCommand("humanitl llm key set".to_owned()))
                .build(),
        }
    }
}

/// Alles, was die Probe über den Endpunkt weiß, bevor sie ihn fragt.
#[derive(Debug)]
struct Target {
    authority: Authority,
    scheme: Scheme,
    /// Der Pfad der Modellliste unter der OpenAI-kompatiblen Basis, etwa
    /// `/v1/models` oder `/api/v1/models` ([`llm_endpoint::openai_path`]).
    models_path: String,
    /// Wahr, wenn der Endpunkt an der Wurzel des Servers sitzt und die Probe
    /// deshalb zuerst [`OLLAMA_TAGS_PATH`] fragt
    /// ([`llm_endpoint::serves_native_ollama`]).
    native_ollama: bool,
    /// `Authorization: Bearer …`, wenn ein Schlüssel mitgeht, als empfindlich
    /// markiert (HUM-140).
    authorization: Option<HeaderValue>,
}

impl Target {
    /// Liest den Endpunkt.
    ///
    /// Über dieselben Funktionen, aus denen `OpenCode` seine `baseURL`
    /// bekommt ([`llm_endpoint`]): `http://host:1/v1` und `http://host:1`
    /// meinen denselben Server, die Probe fragt beide Male `/api/tags` und
    /// `/v1/models`. `https://host/api/v1/` ist ein Server hinter dem Präfix
    /// `/api`; dort fragt sie nur `/api/v1/models` (HUM-261).
    fn parse(endpoint: &Url) -> Result<Self, Diagnostic> {
        let scheme = match endpoint.scheme() {
            "http" => Scheme::Http,
            "https" => Scheme::Https,
            other => {
                return Err(unreadable_endpoint(format!(
                    "{} uses the scheme {other:?}; the probe speaks http and https only",
                    llm_endpoint::display(endpoint)
                )));
            }
        };
        let host = endpoint
            .host_str()
            .ok_or(())
            .and_then(|text| HostName::parse(text).map_err(|_err| ()))
            .map_err(|()| {
                unreadable_endpoint(format!(
                    "{} has no host that Humanitl can read",
                    llm_endpoint::display(endpoint)
                ))
            })?;
        let port = endpoint.port_or_known_default().unwrap_or(match scheme {
            Scheme::Https => 443,
            _ => 80,
        });
        Ok(Self {
            authority: Authority::new(host, port),
            scheme,
            models_path: llm_endpoint::openai_path(endpoint, OPENAI_MODELS),
            native_ollama: llm_endpoint::serves_native_ollama(endpoint),
            authorization: None,
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

    /// Die Adresse, die die Probe fragt, als Text — für den `curl` im
    /// Vorschlag und für das `why`. `path` ist absolut.
    fn url(&self, path: &str) -> String {
        format!("{}://{}{path}", self.scheme.as_str(), self.authority)
    }

    /// Der Vorschlag, dieselbe Frage von Hand zu stellen.
    ///
    /// Mit Pfad der `curl` auf genau diese Adresse; ohne, weil die Probe vor
    /// jeder Anfrage scheiterte, die Auflösung des Namens.
    ///
    /// Die Adresse geht als ein Wort der Shell hinein
    /// ([`humanitl_core::shell::shell_word`]): Ein Pfad wie `/api;id;/v1`
    /// führte sonst beim Einfügen `id` als eigenen Befehl aus.
    fn retry(&self, path: Option<&str>) -> FixAction {
        FixAction::CopyCommand(match path {
            Some(path) => format!("curl -sS {}", shell_word(&self.url(path))),
            None => format!(
                "getent hosts {}",
                shell_word(&self.authority.host.to_string())
            ),
        })
    }

    /// Die Pfade, die die Probe fragt, in ihrer Reihenfolge.
    fn asked_paths(&self) -> Vec<&str> {
        let mut paths = Vec::with_capacity(2);
        if self.native_ollama {
            paths.push(OLLAMA_TAGS_PATH);
        }
        paths.push(self.models_path.as_str());
        paths
    }

    /// Die Anfrage an einen der beiden Pfade, `path` absolut.
    fn request(&self, path: &str) -> HttpRequest {
        let mut request = HttpRequest::new(
            Method::GET,
            self.scheme,
            self.authority.clone(),
            path.to_owned(),
        );
        request.headers.insert(
            HeaderName::from_static("accept"),
            HeaderValue::from_static("application/json"),
        );
        if let Some(value) = &self.authorization {
            request
                .headers
                .insert(HeaderName::from_static("authorization"), value.clone());
        }
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
                "{} is not one of the private forms — neither an address in a private \
                 network nor localhost or a name under {}. Judged from the name alone; nothing \
                 was resolved. Traffic to this address bypasses the queue, so only put a machine \
                 you control here.",
                llm_endpoint::display(endpoint),
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
            "{} is not on a private network. Traffic to this address bypasses the queue, \
             so only put a machine you control here.",
            llm_endpoint::display(endpoint)
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
///
/// Das `why` nennt genau die Pfade, die gefragt wurden, nicht eine feste
/// Liste: Bei einem Endpunkt mit Pfad-Präfix war `/api/tags` nicht dabei.
///
/// Gebaut aus dem Ziel der Anfragen und den gefragten Pfaden, nie aus dem
/// eingetragenen Text: Dessen Query oder Fragment ging nicht hinaus und kann
/// ein Geheimnis tragen (HUM-261).
fn unknown_api(target: &Target) -> Diagnostic {
    let urls: Vec<String> = target
        .asked_paths()
        .into_iter()
        .map(|path| target.url(path))
        .collect();
    let asked = match urls.as_slice() {
        [only] => format!("{only} did not answer"),
        many => format!("neither {} answered", many.join(" nor ")),
    };
    Diagnostic::builder(LLM_003, Severity::Warning)
        .why(format!(
            "connected to {}, but {asked} with a model list. Check that the URL is the base URL \
             of the OpenAI-compatible API (as in OpenCode), not a chat UI.",
            target.authority
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

/// Die Modell-Ids aus einer Antwort von `GET <Basis>/models`.
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

    /// Der Endpunkt ist die Basis der OpenAI-kompatiblen API (HUM-261): Ein
    /// abschließendes `/v1` wird nicht verdoppelt, ein Pfad-Präfix bleibt vor
    /// `/v1/models`, und `/api/tags` gilt nur an der Wurzel des Servers.
    #[test]
    fn the_models_path_lies_under_the_base_and_tags_only_at_the_root() {
        for (endpoint, models, native_ollama) in [
            ("http://192.168.1.50:11434", "/v1/models", true),
            ("http://192.168.1.50:11434/", "/v1/models", true),
            ("http://host:1/v1", "/v1/models", true),
            ("http://host:1/v1/", "/v1/models", true),
            ("http://host:1/openai", "/openai/v1/models", false),
            ("http://host:1/openai/v1", "/openai/v1/models", false),
            ("https://llm.example.net/api/v1/", "/api/v1/models", false),
            ("https://llm.example.net/api/v1", "/api/v1/models", false),
        ] {
            let target = Target::parse(&Url::parse(endpoint).unwrap()).unwrap();
            assert_eq!(target.models_path, models, "for {endpoint}");
            assert_eq!(target.native_ollama, native_ollama, "for {endpoint}");
            assert!(
                target
                    .asked_paths()
                    .iter()
                    .all(|path| !path.contains("/api/api/")),
                "for {endpoint}: {:?}",
                target.asked_paths()
            );
            // Die Ankündigung der Kommandozeile liest dieselben Pfade aus
            // `llm_endpoint::probe_paths`; die Probe fragt genau diese.
            assert_eq!(
                target.asked_paths(),
                humanitl_config::llm_endpoint::probe_paths(&Url::parse(endpoint).unwrap()),
                "for {endpoint}"
            );
        }
    }

    #[test]
    fn the_probe_only_ever_gets_two_fixed_paths() {
        let target = Target::parse(&Url::parse("http://host:1").unwrap()).unwrap();
        assert_eq!(
            target.asked_paths(),
            [super::OLLAMA_TAGS_PATH, "/v1/models"]
        );
        for path in target.asked_paths() {
            let request = target.request(path);
            assert_eq!(request.method, humanitl_core::Method::GET);
            assert_eq!(request.path_and_query, path);
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
        for diagnostic in [
            super::not_private(&url),
            super::unknown_api(&Target::parse(&url).unwrap()),
        ] {
            match diagnostic.fix {
                Some(humanitl_core::FixAction::ChangeSetting { key, value }) => {
                    assert_eq!(key, "llm.endpoint");
                    assert!(!value.is_empty(), "{} proposes nothing", diagnostic.code);
                }
                other => panic!("expected a setting change, got {other:?}"),
            }
        }
    }

    /// Der Kopier-Befehl nennt die Adresse als ein Wort der Shell (HUM-261):
    /// `;`, `$(…)` und ein Leerzeichen im Pfad führen beim Einfügen keinen
    /// zweiten Befehl aus.
    #[test]
    fn the_copied_command_quotes_the_address() {
        let target =
            Target::parse(&Url::parse("http://127.0.0.1:1/a b;id;$(id)/v1").unwrap()).unwrap();
        assert_eq!(
            target.retry(Some(&target.models_path)),
            humanitl_core::FixAction::CopyCommand(
                "curl -sS 'http://127.0.0.1:1/a%20b;id;$(id)/v1/models'".to_owned()
            )
        );
        let plain = Target::parse(&Url::parse("http://127.0.0.1:1").unwrap()).unwrap();
        assert_eq!(
            plain.retry(Some(super::OLLAMA_TAGS_PATH)),
            humanitl_core::FixAction::CopyCommand(
                "curl -sS http://127.0.0.1:1/api/tags".to_owned()
            ),
            "a plain address stays readable"
        );
    }

    /// Benutzer, Passwort, Query und Fragment des Endpunkts stehen in keinem
    /// Befund (HUM-261): Sie gehen mit keiner Anfrage hinaus und können ein
    /// Geheimnis tragen.
    #[test]
    fn no_diagnostic_repeats_a_secret_from_the_endpoint() {
        let url =
            Url::parse("https://user:pw-geheim@model.example.com/v1/?token=geheim#frag-geheim")
                .unwrap();
        let target = Target::parse(&url).unwrap();
        let mut texts = vec![
            super::not_private(&url).why,
            super::not_private_by_name(&url).unwrap().why,
            super::unknown_api(&target).why,
        ];
        let ftp = Url::parse("ftp://user:pw-geheim@model.example.com/?token=geheim").unwrap();
        texts.push(Target::parse(&ftp).unwrap_err().why);
        for why in texts {
            assert!(!why.contains("geheim"), "{why}");
            assert!(!why.contains("user"), "{why}");
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
