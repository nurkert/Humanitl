//! Die Doctor-Zeile `rules`: Lässt sich `rules.yaml` lesen (HUM-561)?
//!
//! Seit HUM-561 kostet eine unlesbare Regel nur sich selbst; die übrigen
//! gelten. Das ist leiser als vorher, und genau deshalb steht es hier: Wer
//! `humanitl doctor` fragt oder den Setup-Bildschirm öffnet, erfährt, welche
//! Regel fehlt und warum, auch ohne den Regel-Bildschirm. Die Zeile liest die
//! Datei mit demselben Parser wie der Speicher beim Start
//! ([`read_user_rules`]); sie verbindet nichts und schreibt nichts. Den
//! Speicher selbst nimmt sie nicht: Der Doctor läuft auch ohne Daemon, und
//! was vom Proxy-Crate abhängt, soll nicht wachsen (HUM-145).
//!
//! `WARN` und nie `FAIL`: Eine kaputte Regel hält keinen Start auf, und ohne
//! Regel wird gefragt, nie erlaubt.

use std::path::Path;

use humanitl_core::diagnostics::codes::RULES_001;
use humanitl_core::{Diagnostic, SessionId, Severity};
use humanitl_rules::{UserRules, read_user_rules, refused_finding};

use crate::{convert, v1};

/// Der Bezeichner der Zeile.
pub const DOCTOR_ID: &str = "rules";

/// Was die Datei hergab: [`None`], wenn es keine gibt.
fn read(path: &Path) -> Option<Result<UserRules, Vec<Diagnostic>>> {
    if path.as_os_str().is_empty() {
        return None;
    }
    match std::fs::read_to_string(path) {
        Ok(text) => Some(read_user_rules(&text, SessionId::new())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => Some(Err(vec![
            Diagnostic::builder(RULES_001, Severity::Error)
                .why(format!("cannot read {}: {error}", path.display()))
                .build(),
        ])),
    }
}

/// Die Zeile für die Regel-Datei unter `path`.
#[must_use]
pub fn doctor_line(path: &Path) -> v1::DoctorCheck {
    let shown = path.display();
    let line = |status: v1::CheckStatus, evidence: String, diagnostic: Option<&Diagnostic>| {
        v1::DoctorCheck {
            id: DOCTOR_ID.to_owned(),
            status: status.into(),
            evidence,
            diagnostic: diagnostic.map(convert::diagnostic_to_proto),
        }
    };
    match read(path) {
        None if path.as_os_str().is_empty() => line(
            v1::CheckStatus::Ok,
            "this daemon keeps no rules file".to_owned(),
            None,
        ),
        None => line(v1::CheckStatus::Ok, format!("{shown}: none yet"), None),
        Some(Err(diagnostics)) => line(
            v1::CheckStatus::Warn,
            format!("{shown} cannot be read; none of your persistent rules apply"),
            Some(&refused_finding(&diagnostics)),
        ),
        Some(Ok(read)) if !read.unreadable.is_empty() => {
            // Jede unlesbare Regel mit ihrem Grund: Die Zeile trägt nur einen
            // Befund, und der zweite Fehler soll nicht erst nach der Reparatur
            // des ersten auffallen.
            let reasons: Vec<&str> = read
                .unreadable
                .iter()
                .map(|entry| first_reason(&entry.diagnostic().why))
                .collect();
            line(
                v1::CheckStatus::Warn,
                format!(
                    "{shown}: {} rules apply, {} left out: {}",
                    read.rules.len(),
                    read.unreadable.len(),
                    reasons.join("; ")
                ),
                Some(read.unreadable[0].diagnostic()),
            )
        }
        Some(Ok(read)) => line(
            v1::CheckStatus::Ok,
            format!("{shown}: {} rules", read.rules.len()),
            None,
        ),
    }
}

/// Der Grund aus dem Befund einer unlesbaren Regel, ohne die Folgen.
///
/// Der Befund beginnt mit Feld, Zeile und Grund und hängt dann an, was bis zur
/// Reparatur gilt; die Doctor-Zeile braucht nur den Anfang.
fn first_reason(why: &str) -> &str {
    why.split_once(". rules[").map_or(why, |(reason, _)| reason)
}

/// Setzt die Zeile `rules` hinter `inference`, sonst hinter `egress`, sonst
/// hinter `llm`, sonst ans Ende.
pub fn insert_doctor_line(report: &mut v1::DoctorReport, path: &Path) {
    let line = doctor_line(path);
    let after = |id: &str| report.checks.iter().position(|check| check.id == id);
    let at = after(crate::cloud::DOCTOR_ID)
        .or_else(|| after("egress"))
        .or_else(|| after("llm"))
        .map_or(report.checks.len(), |index| index + 1);
    report.checks.insert(at, line);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn line_for(text: Option<&str>) -> v1::DoctorCheck {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rules.yaml");
        if let Some(text) = text {
            std::fs::write(&path, text).unwrap();
        }
        doctor_line(&path)
    }

    #[test]
    fn every_rule_that_cannot_be_read_is_named_with_its_line_and_reason() {
        let line = line_for(Some(
            "version: 1\nrules:\n  - action: allow\n    match:\n      host: \"a.example\"\n  \
             - action: block\n    match:\n      host: \"*foo.com\"\n  \
             - action: ask\n    match:\n      host: \"b.example\"\n      port: 0\n",
        ));
        assert_eq!(line.status, i32::from(v1::CheckStatus::Warn));
        // Beide Gründe stehen da, nicht nur der erste.
        for part in [
            "1 rules apply, 2 left out: ",
            "rules[1].match.host (line 8)",
            "*foo.com",
            "; rules[2].match.port (line 12): 0 is not a port",
        ] {
            assert!(
                line.evidence.contains(part),
                "{part:?} in {}",
                line.evidence
            );
        }
        assert!(!line.evidence.contains("is left out"), "{}", line.evidence);
        let diagnostic = line.diagnostic.unwrap();
        assert_eq!(diagnostic.code, "RULES_003");
        assert!(diagnostic.why.contains("*foo.com"), "{}", diagnostic.why);
    }

    #[test]
    fn a_file_that_cannot_be_read_warns_and_a_good_or_missing_one_is_ok() {
        let refused = line_for(Some("version: 7\nrules: []\n"));
        assert_eq!(refused.status, i32::from(v1::CheckStatus::Warn));
        assert!(
            refused
                .evidence
                .contains("none of your persistent rules apply")
        );

        let good = line_for(Some(
            "version: 1\nrules:\n  - action: allow\n    match:\n      host: \"a.example\"\n",
        ));
        assert_eq!(good.status, i32::from(v1::CheckStatus::Ok));
        assert!(good.evidence.ends_with(": 1 rules"), "{}", good.evidence);
        assert!(good.diagnostic.is_none());

        let missing = line_for(None);
        assert_eq!(missing.status, i32::from(v1::CheckStatus::Ok));
        assert!(
            missing.evidence.ends_with("none yet"),
            "{}",
            missing.evidence
        );
    }

    #[test]
    fn the_line_follows_inference() {
        let mut report = v1::DoctorReport {
            checks: ["llm", "egress", "inference", "tray"]
                .into_iter()
                .map(|id| v1::DoctorCheck {
                    id: id.to_owned(),
                    ..v1::DoctorCheck::default()
                })
                .collect(),
            ..v1::DoctorReport::default()
        };
        insert_doctor_line(&mut report, Path::new("/nonexistent/rules.yaml"));
        let ids: Vec<&str> = report
            .checks
            .iter()
            .map(|check| check.id.as_str())
            .collect();
        assert_eq!(ids, vec!["llm", "egress", "inference", "rules", "tray"]);
    }
}
