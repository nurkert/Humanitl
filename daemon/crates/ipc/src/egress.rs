//! `Egress`: der Entscheidungs-Proxy des Nutzers hinter Humanitl (HUM-511,
//! ADR-0022).
//!
//! Vier Operationen, und keine gibt Zugangsdaten zurück:
//!
//! - `status` sagt, was `config.toml` jetzt über `egress.*` sagt, welchen Weg
//!   der laufende Daemon seit seinem Start nimmt, und ob die Zugangsdaten im
//!   Speicher liegen.
//! - `check` baut über `egress.proxy` einen Tunnel zu einem festen Ziel und
//!   den TLS-Handschlag darin auf; das macht der Weiterleiter
//!   (`Upstream::check_decision_proxy`), dieses Modul setzt nur den Stand
//!   dazu.
//! - `set_auth` legt `NUTZER:PASSWORT` im Schlüsselbund ab, sonst in der
//!   Datei des Rückfalls (`CONFIG_019`), und richtet `egress.proxy_auth_ref`
//!   darauf; erst der Speicher, dann der Verweis, wie bei `LlmKey`.
//! - `remove_auth` entfernt beides.
//!
//! Dazu die Zeile `egress` für `humanitl doctor` ([`doctor_line`]): Sie liest
//! nur die Konfiguration, verbindet nichts.

use humanitl_config::secret::{ApiKey, KeyRef, KeyStore};
use humanitl_config::{Config, EgressConfig, Paths, ProfileSelection, ProxyResolve};
use humanitl_core::diagnostics::codes::{CONFIG_020, PROXY_017};
use humanitl_core::{Diagnostic, FixAction, Severity};
use zeroize::Zeroizing;

use crate::{convert, v1};

/// Der Pfad von `egress.proxy_auth_ref` in `config.toml`.
const KEY_REF_PATH: [&str; 2] = ["egress", "proxy_auth_ref"];

/// Der Weg hinaus, den `egress` beschreibt.
#[must_use]
pub fn route(egress: &EgressConfig) -> v1::EgressRoute {
    match egress.proxy_address() {
        Some(proxy) => v1::EgressRoute {
            proxy,
            resolve: egress.proxy_resolve.as_str().to_owned(),
        },
        None => v1::EgressRoute {
            proxy: String::new(),
            resolve: ProxyResolve::Humanitl.as_str().to_owned(),
        },
    }
}

/// Die Konfiguration ohne Profilwunsch, frisch aus den Dateien.
///
/// # Errors
///
/// Der Befund des Ladens, wenn `config.toml` nicht lesbar ist.
pub fn configured(paths: &Paths) -> Result<Config, Diagnostic> {
    humanitl_config::resolve(&ProfileSelection::default(), None, paths.env(), &[])
        .map(|resolved| resolved.config)
}

/// Beantwortet `status`, `set_auth` und `remove_auth`.
///
/// Blockiert: Es startet das Werkzeug des Schlüsselbunds und schreibt
/// `config.toml`. Der Dienst ruft es in [`tokio::task::spawn_blocking`].
/// `check` beantwortet der Dienst selbst und holt den Stand danach hier.
///
/// # Errors
///
/// `CONFIG_020` ohne Operation und für Zugangsdaten, die keine sind oder sich
/// nirgends ablegen lassen; die Befunde von
/// [`humanitl_config::edit::set_value`], wenn `config.toml` nicht geschrieben
/// wurde.
pub fn answer(
    paths: &Paths,
    keys: &KeyStore,
    active: &v1::EgressRoute,
    op: Option<v1::egress_request::Op>,
) -> Result<v1::EgressStatus, Diagnostic> {
    match op {
        Some(v1::egress_request::Op::Status(_) | v1::egress_request::Op::CheckConnection(_)) => {
            Ok(status(paths, keys, active, Vec::new()))
        }
        Some(v1::egress_request::Op::SetAuth(mut set)) => {
            // Der Text der Nachricht wird beim Fallenlassen überschrieben.
            let text = Zeroizing::new(std::mem::take(&mut set.credentials));
            let credentials = ApiKey::credentials(&text)?;
            let stored = keys.store_named(EgressConfig::CREDENTIALS_NAME, &credentials)?;
            write_ref(paths, Some(&stored.key_ref))?;
            let notes = stored
                .note
                .iter()
                .map(convert::diagnostic_to_proto)
                .collect();
            Ok(status(paths, keys, active, notes))
        }
        Some(v1::egress_request::Op::RemoveAuth(_)) => {
            if let Some(key_ref) = configured(paths)
                .ok()
                .and_then(|config| config.egress.proxy_auth_ref)
                .as_deref()
                .and_then(KeyRef::parse)
            {
                keys.clear(&key_ref)?;
            }
            write_ref(paths, None)?;
            Ok(status(paths, keys, active, Vec::new()))
        }
        None => Err(no_operation()),
    }
}

/// `CONFIG_020`: der Auftrag nennt keine Operation.
#[must_use]
pub fn no_operation() -> Diagnostic {
    Diagnostic::builder(CONFIG_020, Severity::Error)
        .why(
            "Egress changed nothing: the request names no operation (status, check, set_auth or \
             remove_auth)"
                .to_owned(),
        )
        .build()
}

/// `PROXY_017`: ohne `egress.proxy` gibt es nichts zu prüfen. Dieselbe
/// Absage wie die des Weiterleiters, für den Fake.
#[must_use]
pub fn nothing_to_check() -> Diagnostic {
    Diagnostic::builder(PROXY_017, Severity::Error)
        .why(
            "egress.proxy is empty, so there is no decision proxy to check; requests leave \
             directly"
                .to_owned(),
        )
        .fix(FixAction::ChangeSetting {
            key: "egress.proxy".to_owned(),
            value: "http://127.0.0.1:8080".to_owned(),
        })
        .build()
}

/// Der Stand: Einstellungen, Weg des laufenden Daemons, Zugangsdaten.
///
/// Ob Zugangsdaten da sind, sagt der Speicher und nicht der Verweis; was
/// er dafür herausgibt, verlässt diese Funktion nicht.
#[must_use]
pub fn status(
    paths: &Paths,
    keys: &KeyStore,
    active: &v1::EgressRoute,
    mut diagnostics: Vec<v1::Diagnostic>,
) -> v1::EgressStatus {
    let egress = match configured(paths) {
        Ok(config) => config.egress,
        Err(diagnostic) => {
            diagnostics.push(convert::diagnostic_to_proto(&diagnostic));
            EgressConfig::default()
        }
    };
    let auth_ref = egress.proxy_auth_ref.clone().unwrap_or_default();
    let auth_set = match KeyRef::parse(&auth_ref) {
        None => false,
        Some(key_ref) => match keys.lookup_credentials(&key_ref) {
            Ok(_credentials) => true,
            Err(_unavailable) => {
                diagnostics.push(convert::diagnostic_to_proto(&credentials_missing(&key_ref)));
                false
            }
        },
    };
    v1::EgressStatus {
        configured: Some(route(&egress)),
        proxy_ca: egress
            .proxy_ca
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        auth_set,
        auth_ref,
        active: Some(active.clone()),
        check_target: String::new(),
        reachable: false,
        diagnostics,
    }
}

/// `PROXY_017`: der Verweis nennt Zugangsdaten, die der Speicher nicht
/// herausgibt.
fn credentials_missing(key_ref: &KeyRef) -> Diagnostic {
    Diagnostic::builder(PROXY_017, Severity::Warning)
        .why(format!(
            "egress.proxy_auth_ref names the credentials {key_ref}, and the keyring does not hand \
             them out; the decision proxy will answer 407 until they are stored again"
        ))
        .fix(FixAction::CopyCommand(
            "humanitl egress auth set".to_owned(),
        ))
        .build()
}

/// Schreibt `egress.proxy_auth_ref`, oder nimmt ihn heraus.
fn write_ref(paths: &Paths, key_ref: Option<&KeyRef>) -> Result<(), Diagnostic> {
    let segments: Vec<String> = KEY_REF_PATH.iter().map(|part| (*part).to_owned()).collect();
    let value = key_ref.map(|key_ref| toml::Value::String(key_ref.to_string()));
    humanitl_config::edit::set_value(&paths.config_path(), &segments, value.as_ref(), |_| Ok(()))?;
    Ok(())
}

/// Die Zeile `egress` von `humanitl doctor` (HUM-511, ADR-0022).
///
/// Ohne `egress.proxy` sagt sie „direkt"; mit ihm, über welchen Proxy, wer
/// auflöst, und dass die Anfragen des Daemons selbst direkt bleiben. Eine
/// CA-Datei, die sich nicht lesen lässt, ist eine Warnung mit `PROXY_017`;
/// ob der Proxy antwortet, prüft `humanitl egress check`, nicht der Doctor.
#[must_use]
pub fn doctor_line(egress: &EgressConfig) -> v1::DoctorCheck {
    let Some(proxy) = egress.proxy_address() else {
        return v1::DoctorCheck {
            id: "egress".to_owned(),
            status: v1::CheckStatus::Ok.into(),
            evidence: "direct; Humanitl resolves after its decision".to_owned(),
            diagnostic: None,
        };
    };
    let resolver = match egress.proxy_resolve {
        ProxyResolve::Proxy => {
            "the proxy resolves, Humanitl refuses only local names and \
                                private address literals"
        }
        ProxyResolve::Humanitl => "Humanitl resolves and checks every address",
    };
    let evidence = format!(
        "through {proxy}; {resolver}; the daemon's own requests (model search, update check) \
         stay direct"
    );
    let problem = egress.proxy_ca.as_ref().and_then(|path| {
        std::fs::read(path).err().map(|err| {
            Diagnostic::builder(PROXY_017, Severity::Warning)
                .why(format!(
                    "egress.proxy_ca = {} cannot be read ({err}); every request through the \
                     decision proxy fails until it can",
                    path.display()
                ))
                .fix(FixAction::ChangeSetting {
                    key: "egress.proxy_ca".to_owned(),
                    value: String::new(),
                })
                .build()
        })
    });
    match problem {
        None => v1::DoctorCheck {
            id: "egress".to_owned(),
            status: v1::CheckStatus::Ok.into(),
            evidence,
            diagnostic: None,
        },
        Some(diagnostic) => v1::DoctorCheck {
            id: "egress".to_owned(),
            status: v1::CheckStatus::Warn.into(),
            evidence,
            diagnostic: Some(convert::diagnostic_to_proto(&diagnostic)),
        },
    }
}

/// Setzt die Zeile `egress` hinter die Zeile `llm` des Berichts, oder ans
/// Ende, wenn es keine gibt.
pub fn insert_doctor_line(report: &mut v1::DoctorReport, egress: &EgressConfig) {
    let line = doctor_line(egress);
    let at = report
        .checks
        .iter()
        .position(|check| check.id == "llm")
        .map_or(report.checks.len(), |index| index + 1);
    report.checks.insert(at, line);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::path::PathBuf;

    use super::*;

    fn paths(dir: &std::path::Path) -> Paths {
        Paths::new(humanitl_config::Env::from_pairs([
            ("HOME", dir.display().to_string()),
            ("XDG_CONFIG_HOME", dir.join("config").display().to_string()),
            ("XDG_DATA_HOME", dir.join("data").display().to_string()),
        ]))
    }

    /// Ein Speicher ohne Schlüsselbund: nur die Datei des Rückfalls im
    /// privaten Verzeichnis des Tests.
    fn keys(paths: &Paths) -> KeyStore {
        KeyStore::for_paths(paths)
            .with_secret_tool(None)
            .with_keychain_tool(None)
    }

    #[test]
    fn credentials_are_stored_referenced_and_never_answered() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        let keys = keys(&paths);
        let active = v1::EgressRoute::default();

        let set = answer(
            &paths,
            &keys,
            &active,
            Some(v1::egress_request::Op::SetAuth(
                v1::egress_request::SetAuth {
                    credentials: "alice:pw".to_owned(),
                },
            )),
        )
        .unwrap();
        assert!(set.auth_set, "{set:?}");
        assert_eq!(set.auth_ref, "file:egress-proxy");
        assert!(!format!("{set:?}").contains("alice:pw"));
        let file = std::fs::read_to_string(paths.config_path()).unwrap();
        assert!(
            file.contains("proxy_auth_ref = \"file:egress-proxy\""),
            "{file}"
        );
        assert!(!file.contains("alice"), "{file}");

        let refused = answer(
            &paths,
            &keys,
            &active,
            Some(v1::egress_request::Op::SetAuth(
                v1::egress_request::SetAuth {
                    credentials: "no-colon".to_owned(),
                },
            )),
        )
        .unwrap_err();
        assert_eq!(refused.code, CONFIG_020);

        let removed = answer(
            &paths,
            &keys,
            &active,
            Some(v1::egress_request::Op::RemoveAuth(
                v1::egress_request::RemoveAuth {},
            )),
        )
        .unwrap();
        assert!(!removed.auth_set);
        assert!(removed.auth_ref.is_empty());
        assert!(!keys.file_path("egress-proxy").exists());
    }

    #[test]
    fn the_status_shows_the_file_and_the_running_way_apart() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        std::fs::create_dir_all(paths.config_path().parent().unwrap()).unwrap();
        std::fs::write(
            paths.config_path(),
            "[egress]\nproxy = \"http://127.0.0.1:8080\"\nproxy_resolve = \"humanitl\"\n",
        )
        .unwrap();
        let status = answer(
            &paths,
            &keys(&paths),
            &v1::EgressRoute::default(),
            Some(v1::egress_request::Op::Status(
                v1::egress_request::Status {},
            )),
        )
        .unwrap();
        let configured = status.configured.unwrap();
        assert_eq!(configured.proxy, "127.0.0.1:8080");
        assert_eq!(configured.resolve, "humanitl");
        assert!(status.active.unwrap().proxy.is_empty(), "restart pending");
    }

    #[test]
    fn the_doctor_line_says_direct_or_through_whom() {
        let direct = doctor_line(&EgressConfig::default());
        assert_eq!(direct.id, "egress");
        assert!(direct.evidence.starts_with("direct"), "{}", direct.evidence);

        let egress = EgressConfig {
            proxy: Some(url::Url::parse("http://127.0.0.1:8080").unwrap()),
            proxy_ca: Some(PathBuf::from("/nonexistent/hum-511/ca.pem")),
            ..EgressConfig::default()
        };
        let line = doctor_line(&egress);
        assert!(
            line.evidence.contains("through 127.0.0.1:8080"),
            "{}",
            line.evidence
        );
        assert!(line.evidence.contains("stay direct"), "{}", line.evidence);
        assert_eq!(line.status, i32::from(v1::CheckStatus::Warn));
        assert_eq!(line.diagnostic.unwrap().code, PROXY_017.as_str());

        let mut report = v1::DoctorReport {
            checks: vec![
                v1::DoctorCheck {
                    id: "llm".to_owned(),
                    ..v1::DoctorCheck::default()
                },
                v1::DoctorCheck {
                    id: "tray".to_owned(),
                    ..v1::DoctorCheck::default()
                },
            ],
            backends: Vec::new(),
        };
        insert_doctor_line(&mut report, &EgressConfig::default());
        let ids: Vec<&str> = report
            .checks
            .iter()
            .map(|check| check.id.as_str())
            .collect();
        assert_eq!(ids, ["llm", "egress", "tray"]);
    }
}
