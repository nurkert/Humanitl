//! Die Befunde der Grenzen aus `limits.*` und der Regelvorschlag mit
//! `stream: true` (HUM-057).
//!
//! Die Grenzen selbst setzen andere Stellen durch: das Cap des Anfrage-Rumpfs
//! der [`handler`](crate::handler), die Uhr des Antwort-Rumpfs
//! [`body::tee`](crate::body::tee), die Kopf-Frist hyper. Hier steht nur, was
//! ein Mensch danach liest. Jeder Befund nennt die Grenze mit ihrem Schlüssel
//! und dem Wert, gegen den gemessen wurde, damit „zu groß" oder „zu still"
//! nie ohne Zahl dasteht.
//!
//! # `stream: true`
//!
//! Eine Anfrage über `limits.hold_body_cap_bytes` lässt sich nicht so halten,
//! wie Humanitl sonst hält: Ihr Rumpf passt nicht in den Puffer, und ohne
//! Rumpf gibt es weder Suche nach Geheimnissen noch Vorschau. Geblockt wird
//! sie deshalb, **außer** eine Regel mit `stream: true` trifft sie. Dann
//! entscheidet die Regel über den Kopf: `allow` lässt den Rumpf sofort
//! gestreamt hinaus, `ask` hält die Anfrage mit ihrem Kopf, und nach einer
//! Freigabe folgt der Rumpf gestreamt. Der Rumpf geht in beiden Fällen
//! ungeprüft hinaus und wird mitgeschnitten wie eine Antwort. Das ist eine
//! Ausnahme, die ein Mensch in seine Regeln schreibt, und sie steht als
//! solche in `docs/SECURITY.md`.

use std::time::Duration;

use humanitl_core::diagnostics::codes::{LIMIT_001, LIMIT_004};
use humanitl_core::{Action, Diagnostic, FixAction, HttpRequest, Rule, RuleId, Severity};

use crate::body::Stall;
use crate::handler::{NoRule, request_matcher};

/// Wie groß ein Rumpf über der Grenze war, soweit der Proxy es weiß.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverCap {
    /// Der Client hat die Länge über `Content-Length` angekündigt.
    Declared(u64),
    /// Der Rumpf kam ohne Länge (`chunked`); bemerkt wurde er beim Puffern,
    /// als er das Cap überschritt. Mehr als die gelesenen Bytes sind nicht
    /// bekannt.
    AtLeast(u64),
}

impl OverCap {
    /// Die Größe als Satzteil: genau, oder „mehr als".
    fn phrase(self) -> String {
        match self {
            Self::Declared(size) => format!("{size} bytes"),
            Self::AtLeast(size) => format!("more than {size} bytes"),
        }
    }

    /// Die angekündigte Länge, falls es eine gab.
    #[must_use]
    pub const fn declared(self) -> Option<u64> {
        match self {
            Self::Declared(size) => Some(size),
            Self::AtLeast(_) => None,
        }
    }
}

/// Der Befund [`LIMIT_001`] zu einer Anfrage, die am Cap geblockt wurde.
///
/// `Warning`, mit dem Vorschlag einer Regel aus [`stream_rule`]. Gibt es zu
/// dieser Anfrage keine wirksame Regel ([`NoRule`]), zeigt der `fix` auf
/// `limits.hold_body_cap_bytes`, und der `why` sagt, warum kein Regelvorschlag
/// dasteht.
///
/// Mit `hard_block` (`hold.hard_block_checksum_secrets` steht an) gibt es
/// **nie** einen Vorschlag `stream: true`: Ein ungeprüfter Strom kann die harte
/// Sperre nicht einhalten, also greift eine solche Regel dann nicht, und ein
/// Vorschlag, der nichts bewirkt, wäre die Falle aus
/// `backlog/CONVENTIONS.md` 4.10. Der `fix` zeigt dann auf das Cap.
#[must_use]
pub fn body_over_cap(
    request: &HttpRequest,
    size: OverCap,
    cap: u64,
    hard_block: bool,
) -> Diagnostic {
    let mut why = format!(
        "the request body to {host} is {size}, over limits.hold_body_cap_bytes ({cap}). \
         Humanitl only decides about a request whose whole body it holds, so it was blocked \
         with 413 and nothing left the host. ",
        host = request.authority.host.display(),
        size = size.phrase(),
    );
    let diagnostic = Diagnostic::builder(LIMIT_001, Severity::Warning);
    let change_cap = FixAction::ChangeSetting {
        key: "limits.hold_body_cap_bytes".to_owned(),
        value: cap.saturating_mul(2).to_string(),
    };
    if hard_block {
        why.push_str(
            "A rule with stream: true does not apply while hold.hard_block_checksum_secrets is \
             on: an unchecked stream cannot keep the hard block, because nobody looks for \
             secrets in a body that is never held. Raise the limit if requests this large are \
             expected.",
        );
        return diagnostic.why(why).fix(change_cap).build();
    }
    match stream_rule(request) {
        Ok(rule) => {
            why.push_str(
                "The suggested rule asks you about requests like this one by their head only \
                 and streams the body once you allow: it then leaves without a scan for secrets \
                 and without a preview, and is recorded up to limits.recorder_max_body_bytes.",
            );
            if rule.matcher.path_prefixes.is_empty() {
                why.push_str(
                    " It carries no path prefix, because this request has none that would draw \
                     a boundary, so it covers every path of this host on this port.",
                );
            }
            why.push_str(
                " Put it in front of your other rules for this host: the first matching rule \
                 decides.",
            );
            diagnostic
                .why(why)
                .fix(FixAction::AddRule(Box::new(rule)))
                .build()
        }
        Err(reason) => {
            why.push_str(match reason {
                NoRule::PortZero => {
                    "There is no rule to suggest: the request names port 0, which no rule can \
                     carry."
                }
                NoRule::UnknownMethod => {
                    "There is no rule to suggest: the request uses a method the rule engine does \
                     not know, so no rule would ever match it."
                }
            });
            diagnostic.why(why).fix(change_cap).build()
        }
    }
}

/// Der Befund [`LIMIT_001`] zu einer Anfrage über dem Cap, die eine Regel mit
/// `stream: true` getroffen hat.
///
/// `Info` und ohne `fix`: Nichts ist schiefgegangen, aber der Rumpf geht
/// ungeprüft hinaus, und das soll am Fluss stehen, nicht nur in der Regel.
#[must_use]
pub fn body_streamed(request: &HttpRequest, size: OverCap, cap: u64, rule: RuleId) -> Diagnostic {
    Diagnostic::builder(LIMIT_001, Severity::Info)
        .why(format!(
            "the request body to {host} is {size}, over limits.hold_body_cap_bytes ({cap}), and \
             rule {rule} streams such bodies (stream: true). The decision is about the head \
             only: if the request goes out, its body is streamed without a scan for secrets and \
             without a preview, and recorded up to limits.recorder_max_body_bytes.",
            host = request.authority.host.display(),
            size = size.phrase(),
        ))
        .build()
}

/// Die Regel, die [`body_over_cap`] vorschlägt: `action: ask` mit
/// `stream: true`, zugeschnitten auf genau diese Anfrage.
///
/// `ask` und nicht `allow`, aus demselben Grund wie beim Vorschlag zu einer
/// privaten Zieladresse ([`crate::handler::private_address_rule`]): Der
/// Mensch hat diese Anfrage nicht freigegeben, er hat sie gar nicht gesehen.
/// Ein `allow` machte daraus „jede künftige Anfrage dieser Art geht ungefragt
/// und ungeprüft hinaus". Mit `ask` sieht er jede einzelne, nur eben ohne
/// Rumpf.
///
/// Der Zuschnitt (Host, Schema, Port, Methode, Pfadpräfix, Protokollwechsel)
/// und seine Wertebereiche stehen an [`crate::handler::private_address_rule`]; ein
/// Vorschlag, den ein Mensch anklickt, muss durch `parse_rules` passen und
/// danach genau diese Anfrage treffen (`backlog/CONVENTIONS.md` 4.10).
///
/// # Errors
///
/// [`NoRule`], wenn sich dazu keine wirksame Regel bauen lässt.
pub fn stream_rule(request: &HttpRequest) -> Result<Rule, NoRule> {
    let matcher = request_matcher(request)?;
    Ok(Rule::new(RuleId::new(), Action::Ask, matcher)
        .with_stream(true)
        .with_note(format!(
            "suggested after Humanitl blocked a request body over limits.hold_body_cap_bytes for \
             {host}; matching requests are asked about by their head, and their body is streamed \
             without a scan for secrets. This rule has to stand in front of any other rule for \
             the same host to take effect",
            host = request.authority.host.display(),
        )))
}

/// Der Befund [`LIMIT_004`] zu einem Antwort-Rumpf, der verstummt ist.
///
/// Der `fix` zeigt auf `limits.body_timeout_secs` mit dem doppelten Wert,
/// wie [`crate::handler::idle_request_body`] für die andere Richtung derselben
/// Grenze.
#[must_use]
pub fn response_went_silent(idle: Duration) -> Diagnostic {
    let secs = idle.as_secs();
    Diagnostic::builder(LIMIT_004, Severity::Warning)
        .why(format!(
            "the target sent its response head and then went silent in the middle of the body \
             for more than {secs}s (limits.body_timeout_secs), so the stream was cut: the agent \
             got a truncated response, and the recording marks it as truncated. The limit \
             measures the silence between two chunks, never the total duration, so a long \
             stream from a language model is not affected as long as it keeps sending."
        ))
        .fix(FixAction::ChangeSetting {
            key: "limits.body_timeout_secs".to_owned(),
            value: secs.saturating_mul(2).to_string(),
        })
        .build()
}

/// Der Befund [`LIMIT_004`] zu einem gestreamten Upload, der beim Ziel hängen
/// blieb (HUM-057).
///
/// Zwei Spannen, zwei Grenzen: Das Ziel nimmt kein Stück mehr an
/// (`limits.body_timeout_secs`), oder es hat den ganzen Rumpf und schickt nie
/// Kopfzeilen (`limits.header_timeout_secs`). Der `fix` verdoppelt die Grenze,
/// die abgelaufen ist.
#[must_use]
pub fn upload_stalled(stall: Stall, idle: Duration, head: Duration) -> Diagnostic {
    let (why, key, secs) = match stall {
        Stall::NotReading => (
            format!(
                "the target stopped reading the streamed request body for more than {}s \
                 (limits.body_timeout_secs), so the upload was cut and the request failed. The \
                 limit measures the time without progress, never the total duration of the \
                 upload.",
                idle.as_secs()
            ),
            "limits.body_timeout_secs",
            idle.as_secs(),
        ),
        Stall::NoResponseHead => (
            format!(
                "the target received the whole streamed request body and then sent no response \
                 head for more than {}s (limits.header_timeout_secs), so the request failed.",
                head.as_secs()
            ),
            "limits.header_timeout_secs",
            head.as_secs(),
        ),
    };
    Diagnostic::builder(LIMIT_004, Severity::Warning)
        .why(why)
        .fix(FixAction::ChangeSetting {
            key: key.to_owned(),
            value: secs.saturating_mul(2).to_string(),
        })
        .build()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_core::{Authority, HostName, Method, Scheme, SessionId};
    use humanitl_rules::{RequestKey, RuleSet, Verdict, parse_rules, serialize_rules};

    use super::*;

    fn post(path: &str) -> HttpRequest {
        let host = HostName::Dns("upload.example.com".to_owned());
        HttpRequest::new(
            Method::POST,
            Scheme::Https,
            Authority::with_scheme(host, Scheme::Https),
            path,
        )
    }

    /// Der Vorschlag am Block ist `ask` mit `stream: true`, passt durch
    /// `parse_rules` und trifft danach genau die geblockte Anfrage.
    #[test]
    fn the_suggestion_streams_asks_and_matches_the_blocked_request() {
        let request = post("/v1/files?token=t0p5ecr3t");
        let diagnostic = body_over_cap(&request, OverCap::Declared(40 << 20), 32 << 20, false);
        assert_eq!(diagnostic.code, LIMIT_001);
        assert_eq!(diagnostic.severity, Severity::Warning);
        assert!(
            diagnostic.why.contains("41943040 bytes"),
            "{}",
            diagnostic.why
        );
        assert!(diagnostic.why.contains("(33554432)"), "{}", diagnostic.why);
        let Some(FixAction::AddRule(rule)) = diagnostic.fix else {
            panic!("expected AddRule, got {:?}", diagnostic.fix);
        };
        assert_eq!(rule.action, Action::Ask);
        assert!(rule.stream);
        assert!(!rule.allow_private);
        assert_eq!(rule.matcher.path_prefixes, vec!["/v1/files".to_owned()]);

        let yaml = serialize_rules(&RuleSet::from_rules([(*rule).clone()]));
        assert!(
            !yaml.contains("t0p5ecr3t"),
            "the query stays out of rules.yaml"
        );
        let (set, warnings) = parse_rules(&yaml)
            .unwrap_or_else(|diagnostics| panic!("the suggestion must parse: {diagnostics:?}"));
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(set.get(rule.id).expect("the rule keeps its id").stream);
        let key = RequestKey::new(
            &request.authority.host,
            &request.method,
            &request.path_and_query,
            request.scheme,
            request.authority.port,
        );
        assert_eq!(
            set.evaluate(&key, chrono::Utc::now(), SessionId::new()),
            Verdict::Matched {
                rule: rule.id,
                action: Action::Ask
            },
            "the suggested rule matches the request it came from"
        );
    }

    /// Ohne wirksame Regel zeigt der Fix auf das Cap, und der Satz sagt warum.
    #[test]
    fn without_a_rule_the_fix_is_the_cap() {
        let mut request = post("/");
        request.method = Method::from_bytes(b"FROB").unwrap();
        let diagnostic = body_over_cap(&request, OverCap::AtLeast(1024), 1024, false);
        assert!(
            diagnostic.why.contains("more than 1024 bytes"),
            "{}",
            diagnostic.why
        );
        assert!(diagnostic.why.contains("no rule"), "{}", diagnostic.why);
        assert_eq!(
            diagnostic.fix,
            Some(FixAction::ChangeSetting {
                key: "limits.hold_body_cap_bytes".to_owned(),
                value: "2048".to_owned(),
            })
        );
    }

    /// Unter der harten Sperre gibt es keinen Vorschlag `stream: true`, nur
    /// das Cap, und der Satz nennt den Grund.
    #[test]
    fn under_the_hard_block_the_fix_is_the_cap_never_a_stream_rule() {
        let diagnostic = body_over_cap(&post("/v1/files"), OverCap::Declared(2048), 1024, true);
        assert!(
            diagnostic.why.contains("hold.hard_block_checksum_secrets"),
            "{}",
            diagnostic.why
        );
        assert_eq!(
            diagnostic.fix,
            Some(FixAction::ChangeSetting {
                key: "limits.hold_body_cap_bytes".to_owned(),
                value: "2048".to_owned(),
            })
        );
    }

    #[test]
    fn a_streamed_body_is_info_without_a_fix() {
        let rule = RuleId::new();
        let diagnostic = body_streamed(&post("/x"), OverCap::Declared(10), 5, rule);
        assert_eq!(diagnostic.severity, Severity::Info);
        assert!(diagnostic.fix.is_none());
        assert!(diagnostic.why.contains(&rule.to_string()));
    }

    #[test]
    fn the_silent_response_names_the_limit_and_doubles_it() {
        let diagnostic = response_went_silent(Duration::from_secs(7));
        assert_eq!(diagnostic.code, LIMIT_004);
        assert!(diagnostic.why.contains("7s (limits.body_timeout_secs)"));
        assert_eq!(
            diagnostic.fix,
            Some(FixAction::ChangeSetting {
                key: "limits.body_timeout_secs".to_owned(),
                value: "14".to_owned(),
            })
        );
    }
}
