//! Wertebereiche. Was das Schema nicht ausdrückt, steht hier.
//!
//! Ein Typ sagt „Zahl", nicht „mindestens eine Sekunde". Die Prüfung läuft
//! nach dem Zusammensetzen aller Ebenen, weil erst dann feststeht, was
//! tatsächlich gilt: Ein Profil darf einen Wert setzen, den die globale Datei
//! später wieder heilt.
//!
//! Jeder Verstoß ist ein [`Diagnostic`] mit `CONFIG_003`, dem Pfad, dem
//! gefundenen Wert, dem erlaubten Bereich und einem Vorschlag, der den
//! Vorgabewert einsetzt.
//!
//! Zwei Prüfungen sehen aufs Dateisystem und aufs Netz, nicht nur auf Zahlen
//! (`backlog/CONVENTIONS.md` 4.11): `llm.endpoint` spricht nur `http` oder
//! `https`, und `sandbox.work_dir` muss ein absolutes, existierendes
//! Verzeichnis ohne `..` sein, das sich kanonisieren lässt.

use std::path::{Component, Path};

use humanitl_core::diagnostics::codes::CONFIG_003;
use humanitl_core::{Diagnostic, FixAction, Severity};

use crate::model::{
    AgentRef, Config, Experimental, FindingsConfig, Limits, LlmConfig, RecorderConfig,
    ResolverConfig, SandboxRef,
};
use crate::schema;

/// Ein Tag in Sekunden: die Obergrenze der langen Fristen.
const DAY: u64 = 86_400;

/// Ein Verstoß gegen einen Wertebereich.
fn out_of_range(key: &str, found: &str, allowed: &str) -> Diagnostic {
    let suggestion =
        schema::field(key).map_or_else(|| "-".to_owned(), schema::Field::default_literal);
    Diagnostic::builder(CONFIG_003, Severity::Error)
        .why(format!(
            "{key} = {found} is out of range; allowed is {allowed} (default {suggestion})"
        ))
        .fix(FixAction::ChangeSetting {
            key: key.to_owned(),
            value: suggestion,
        })
        .build()
}

fn at_least(key: &str, value: u64, min: u64) -> Result<(), Diagnostic> {
    if value < min {
        return Err(out_of_range(
            key,
            &value.to_string(),
            &format!("{min} or more"),
        ));
    }
    Ok(())
}

fn between(key: &str, value: u64, min: u64, max: u64) -> Result<(), Diagnostic> {
    if value < min || value > max {
        return Err(out_of_range(
            key,
            &value.to_string(),
            &format!("{min} to {max}"),
        ));
    }
    Ok(())
}

/// `sandbox.work_dir` wird als `/work` in die Sandbox eingehängt. Ein relativer
/// Pfad hinge vom Arbeitsverzeichnis des Daemons ab, ein `..` liefe aus dem
/// Verzeichnis hinaus, das der Nutzer meinte; beides ist `CONFIG_003`. Der Pfad
/// muss sich kanonisieren lassen (er existiert, jeder Symlink darin führt
/// irgendwohin) und dahinter ein Verzeichnis sein.
///
/// Fehlt das Verzeichnis, trägt der Befund den Befehl, der es anlegt.
fn work_dir_is_a_directory(work_dir: &Path) -> Result<(), Diagnostic> {
    const KEY: &str = "sandbox.work_dir";
    let shown = work_dir.display().to_string();

    if !work_dir.is_absolute() {
        return Err(out_of_range(KEY, &shown, "an absolute path"));
    }
    if work_dir
        .components()
        .any(|segment| segment == Component::ParentDir)
    {
        return Err(out_of_range(
            KEY,
            &shown,
            "an absolute path without .. segments",
        ));
    }
    let canonical = match std::fs::canonicalize(work_dir) {
        Ok(canonical) => canonical,
        Err(err) => {
            return Err(Diagnostic::builder(CONFIG_003, Severity::Error)
                .why(format!(
                    "{KEY} = {shown} cannot be resolved ({err}); it must be an existing directory"
                ))
                .fix(FixAction::CopyCommand(format!(
                    "mkdir -p {}",
                    shell_word(work_dir)
                )))
                .build());
        }
    };
    if !canonical.is_dir() {
        return Err(out_of_range(
            KEY,
            &format!("{shown} (resolves to {})", canonical.display()),
            "an existing directory, not a file",
        ));
    }
    Ok(())
}

/// `llm.endpoint` spricht nur `http` oder `https`: der Proxy reicht Verkehr
/// dorthin durch, und ein anderes Schema (`file`, `unix`, `ftp`) wäre kein
/// Endpunkt, sondern ein Weg an der Prüfung vorbei. Die Passthrough-Präfixe
/// sind absolute Pfade, und mindestens einer muss da sein.
fn llm_is_well_formed(llm: &LlmConfig) -> Result<(), Diagnostic> {
    if let Some(endpoint) = &llm.endpoint
        && !matches!(endpoint.scheme(), "http" | "https")
    {
        return Err(out_of_range(
            "llm.endpoint",
            &format!("{:?}", endpoint.as_str()),
            "a URL with the scheme http or https",
        ));
    }
    if llm.passthrough_paths.is_empty() {
        return Err(out_of_range(
            "llm.passthrough_paths",
            "[]",
            "at least one path prefix",
        ));
    }
    for path in &llm.passthrough_paths {
        if !path.starts_with('/') {
            return Err(out_of_range(
                "llm.passthrough_paths",
                &format!("{path:?}"),
                "path prefixes that start with /",
            ));
        }
    }
    Ok(())
}

/// Eine ignorierte Prüfsumme ist SHA-256 in Hex; eine freigegebene Domain ist
/// eine Domain, keine Adresse.
fn findings_are_well_formed(findings: &FindingsConfig) -> Result<(), Diagnostic> {
    for hash in &findings.ignored_hashes {
        if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(out_of_range(
                "findings.ignored_hashes",
                &format!("{hash:?}"),
                "64 hex digits (a SHA-256 checksum)",
            ));
        }
    }
    for domain in &findings.email_allow_domains {
        if domain.is_empty() || domain.contains('@') {
            return Err(out_of_range(
                "findings.email_allow_domains",
                &format!("{domain:?}"),
                "a domain without an @",
            ));
        }
    }
    Ok(())
}

/// Der Pfad als ein Wort für die Shell: in einfachen Anführungszeichen, ein
/// enthaltenes `'` als `'\''`.
fn shell_word(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

/// Die Obergrenzen und Fristen des Proxys.
fn limits_are_well_formed(limits: &Limits) -> Result<(), Diagnostic> {
    at_least(
        "limits.hold_body_cap_bytes",
        limits.hold_body_cap_bytes,
        1024,
    )?;
    at_least("limits.preview_cap_bytes", limits.preview_cap_bytes, 1024)?;
    at_least(
        "limits.event_buffer",
        u64::try_from(limits.event_buffer).unwrap_or(u64::MAX),
        1,
    )?;
    at_least(
        "limits.max_decompress_ratio",
        u64::from(limits.max_decompress_ratio),
        1,
    )?;
    at_least("limits.hold_max_flows", u64::from(limits.hold_max_flows), 1)?;
    at_least("limits.hold_max_bytes", limits.hold_max_bytes, 1024)?;
    between(
        "limits.connect_timeout_secs",
        limits.connect_timeout_secs,
        1,
        600,
    )?;
    between(
        "limits.header_timeout_secs",
        limits.header_timeout_secs,
        1,
        3600,
    )?;
    between("limits.body_timeout_secs", limits.body_timeout_secs, 1, DAY)?;
    at_least(
        "limits.recorder_max_body_bytes",
        limits.recorder_max_body_bytes,
        1024,
    )?;
    // Die Obergrenze ist grob und soll nur den Tippfehler fangen: Ein Wert über
    // der Zahl der Dateideskriptoren des Prozesses ist keine Grenze mehr.
    between(
        "limits.max_client_connections",
        u64::from(limits.max_client_connections),
        1,
        65_536,
    )?;

    if limits.hold_max_bytes < limits.hold_body_cap_bytes {
        return Err(out_of_range(
            "limits.hold_max_bytes",
            &limits.hold_max_bytes.to_string(),
            &format!(
                "at least limits.hold_body_cap_bytes ({}), otherwise not a single body fits \
                 into the hold budget",
                limits.hold_body_cap_bytes
            ),
        ));
    }
    Ok(())
}

/// Der Recorder, gemessen an den Grenzen des Proxys.
fn recorder_is_well_formed(recorder: &RecorderConfig, limits: &Limits) -> Result<(), Diagnostic> {
    at_least("recorder.inline_max_bytes", recorder.inline_max_bytes, 1)?;
    if recorder.inline_max_bytes > limits.recorder_max_body_bytes {
        return Err(out_of_range(
            "recorder.inline_max_bytes",
            &recorder.inline_max_bytes.to_string(),
            &format!(
                "at most limits.recorder_max_body_bytes ({})",
                limits.recorder_max_body_bytes
            ),
        ));
    }
    between(
        "recorder.retention_days",
        u64::from(recorder.retention_days),
        1,
        3650,
    )
}

/// Der Resolver: die Frist des Caches und die festen Adressen.
fn resolver_is_well_formed(resolver: &ResolverConfig) -> Result<(), Diagnostic> {
    between("resolver.cache_ttl_secs", resolver.cache_ttl_secs, 0, DAY)?;
    for (host, address) in &resolver.overrides {
        if address.parse::<std::net::IpAddr>().is_err() {
            return Err(out_of_range(
                "resolver.overrides",
                &format!("{host} = {address:?}"),
                "an IPv4 or IPv6 address",
            ));
        }
    }
    Ok(())
}

/// Die Sandbox: ein Profilname ohne Pfad, ein Arbeitsverzeichnis, das es gibt.
fn sandbox_is_well_formed(sandbox: &SandboxRef) -> Result<(), Diagnostic> {
    if sandbox.profile.is_empty() || sandbox.profile.contains('/') {
        return Err(out_of_range(
            "sandbox.profile",
            &format!("{:?}", sandbox.profile),
            "a profile name without a path separator",
        ));
    }
    if let Some(work_dir) = &sandbox.work_dir {
        work_dir_is_a_directory(work_dir)?;
    }
    // Die Paare aus `sandbox.env` gehen als `--setenv KEY VALUE` an bwrap. Ein
    // leerer Name, ein `=` oder ein Nullbyte darin ergäbe dort kein Paar mehr,
    // sondern eine zweite Variable oder ein abgeschnittenes Wort; das wird hier
    // abgelehnt und nicht erst in der Kommandozeile sichtbar.
    for (key, value) in &sandbox.env {
        if key.is_empty() || key.contains('=') || key.contains('\0') || value.contains('\0') {
            return Err(out_of_range(
                "sandbox.env",
                &format!("{key:?}"),
                "a variable name without \'=\' and without a NUL byte, and a value without a NUL byte",
            ));
        }
        if crate::env::is_loader_key(key) {
            return Err(loader_variable_refused("sandbox.env", key));
        }
    }
    Ok(())
}

/// Eine Linker-Variable in einer Sandbox-Umgebung: abgelehnt, mit Grund.
///
/// Der Weg, den das schließt, ist billig: `sandbox.env` ist gegen das
/// Projekt-Profil gesperrt, aber nicht gegen die Umgebung des Prozesses. Ein
/// `HUMANITL_SANDBOX__ENV='{ LD_PRELOAD = "/work/evil.so" }'` in derselben
/// Shell, in der ein `direnv` das `.envrc` eines geklonten Projekts ausführt,
/// setzt den Schlüssel — und dasselbe Profil behandelt `/work/.envrc`
/// ausdrücklich als angreiferbeeinflusst und überdeckt es
/// (`MANDATORY_MASKED_FILES`).
///
/// `key` ist der Konfigurationsschlüssel (`sandbox.env` oder `[env]` eines
/// Profils), `name` die beanstandete Variable.
#[must_use]
pub fn loader_variable_refused(key: &str, name: &str) -> Diagnostic {
    Diagnostic::builder(CONFIG_003, Severity::Blocking)
        .why(format!(
            "{key} sets {name}; the dynamic linker reads it before main runs, so the code it \
             loads would run in the shim and in the agent before the seccomp filter is installed, \
             and a process forked there would never inherit it. That is the third guarantee, and \
             no setting may take it away. Remove {name} from {key}."
        ))
        .fix(FixAction::ChangeSetting {
            key: key.to_owned(),
            value: format!("remove {name}"),
        })
        .build()
}

/// Der Agent: ein Adapter und, falls gesetzt, ein Befehl mit einem Programm.
fn agent_is_well_formed(agent: &AgentRef) -> Result<(), Diagnostic> {
    if agent.adapter.is_empty() {
        return Err(out_of_range(
            "agent.adapter",
            "\"\"",
            "the id of an adapter",
        ));
    }
    if agent.command.as_ref().is_some_and(Vec::is_empty) {
        return Err(out_of_range(
            "agent.command",
            "[]",
            "at least the program to run, or nothing at all",
        ));
    }
    Ok(())
}

/// Die Versuchsfelder: Portnummern als Schlüssel.
fn experimental_is_well_formed(experimental: &Experimental) -> Result<(), Diagnostic> {
    for (from, to) in &experimental.upstream_port_map {
        if from.parse::<u16>().is_err() {
            return Err(out_of_range(
                "experimental.upstream_port_map",
                &format!("{from:?} = {to}"),
                "a port number as the key",
            ));
        }
    }
    Ok(())
}

impl Config {
    /// Prüft alle Wertebereiche und Beziehungen zwischen Feldern.
    ///
    /// Gibt den ersten Verstoß zurück, damit die Meldung eine Ursache nennt und
    /// nicht eine Liste. Wer alles sehen will, behebt den ersten und ruft
    /// erneut.
    ///
    /// # Errors
    ///
    /// [`Diagnostic`] mit `CONFIG_003`, sobald ein Wert außerhalb seines
    /// Bereichs liegt oder zwei Werte einander widersprechen.
    pub fn validate(&self) -> Result<(), Diagnostic> {
        between("hold.timeout_secs", self.hold.timeout_secs, 1, DAY)?;
        limits_are_well_formed(&self.limits)?;
        recorder_is_well_formed(&self.recorder, &self.limits)?;
        resolver_is_well_formed(&self.resolver)?;
        at_least(
            "pseudonyms.max_response_bytes",
            self.pseudonyms.max_response_bytes,
            1024,
        )?;
        llm_is_well_formed(&self.llm)?;
        findings_are_well_formed(&self.findings)?;
        sandbox_is_well_formed(&self.sandbox)?;
        agent_is_well_formed(&self.agent)?;
        experimental_is_well_formed(&self.experimental)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::path::{Path, PathBuf};

    use humanitl_core::FixAction;

    use crate::model::Config;

    fn why_of(config: &Config) -> String {
        match config.validate() {
            Ok(()) => panic!("the config was expected to be invalid"),
            Err(diagnostic) => {
                assert_eq!(diagnostic.code.as_str(), "CONFIG_003");
                diagnostic.why
            }
        }
    }

    #[test]
    fn the_defaults_are_valid() {
        assert!(Config::default().validate().is_ok());
    }

    #[test]
    fn a_zero_timeout_is_out_of_range() {
        let mut config = Config::default();
        config.hold.timeout_secs = 0;
        let why = why_of(&config);
        assert!(why.contains("hold.timeout_secs = 0"), "{why}");
        assert!(why.contains("default 300"), "{why}");
    }

    #[test]
    fn a_body_cap_below_a_kibibyte_is_out_of_range() {
        let mut config = Config::default();
        config.limits.hold_body_cap_bytes = 512;
        assert!(why_of(&config).contains("limits.hold_body_cap_bytes = 512"));
    }

    #[test]
    fn the_hold_budget_must_hold_one_body() {
        let mut config = Config::default();
        config.limits.hold_max_bytes = 4096;
        let why = why_of(&config);
        assert!(why.contains("limits.hold_max_bytes"), "{why}");
        assert!(why.contains("hold_body_cap_bytes"), "{why}");
    }

    #[test]
    fn a_passthrough_path_needs_a_leading_slash() {
        let mut config = Config::default();
        config.llm.passthrough_paths = vec!["v1/".to_owned()];
        assert!(why_of(&config).contains("start with /"));
    }

    #[test]
    fn an_ignored_hash_is_a_sha256() {
        let mut config = Config::default();
        config.findings.ignored_hashes = vec!["deadbeef".to_owned()];
        assert!(why_of(&config).contains("64 hex digits"));
    }

    #[test]
    fn a_resolver_override_needs_an_address() {
        let mut config = Config::default();
        config
            .resolver
            .overrides
            .insert("api.example.com".to_owned(), "not-an-ip".to_owned());
        let why = why_of(&config);
        assert!(why.contains("api.example.com"), "{why}");
    }

    #[test]
    fn a_profile_is_a_name_not_a_path() {
        let mut config = Config::default();
        config.sandbox.profile = "../../etc/passwd".to_owned();
        assert!(why_of(&config).contains("without a path separator"));
    }

    fn endpoint(text: &str) -> url::Url {
        text.parse().expect("a well-formed URL")
    }

    #[test]
    fn an_endpoint_speaks_http_or_https() {
        for scheme in [
            "ftp://box.lan/v1",
            "file:///etc/passwd",
            "unix:/run/llm.sock",
        ] {
            let mut config = Config::default();
            config.llm.endpoint = Some(endpoint(scheme));
            let why = why_of(&config);
            assert!(why.contains("llm.endpoint"), "{why}");
            assert!(why.contains(scheme), "{why}");
            assert!(why.contains("http or https"), "{why}");
        }
    }

    #[test]
    fn an_http_and_an_https_endpoint_are_accepted() {
        for scheme in ["http://box.lan:8080/v1", "https://box.lan/v1"] {
            let mut config = Config::default();
            config.llm.endpoint = Some(endpoint(scheme));
            assert!(config.validate().is_ok(), "{scheme}");
        }
    }

    #[test]
    fn a_relative_work_dir_is_rejected() {
        let mut config = Config::default();
        config.sandbox.work_dir = Some(PathBuf::from("projects/nordlicht"));
        let why = why_of(&config);
        assert!(why.contains("sandbox.work_dir"), "{why}");
        assert!(why.contains("projects/nordlicht"), "{why}");
        assert!(why.contains("absolute"), "{why}");
    }

    #[test]
    fn a_work_dir_with_a_parent_segment_is_rejected() {
        let dir = tempfile::tempdir().expect("tempdir");
        // Existiert und ist absolut; nur das `..` stört.
        let name = dir.path().file_name().expect("a tempdir has a name");
        let sneaky = dir.path().join("..").join(name);
        let mut config = Config::default();
        config.sandbox.work_dir = Some(sneaky.clone());
        let why = why_of(&config);
        assert!(why.contains(&sneaky.display().to_string()), "{why}");
        assert!(why.contains(".."), "{why}");
    }

    #[test]
    fn a_missing_work_dir_is_rejected_with_a_command_to_create_it() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("not yet");
        let mut config = Config::default();
        config.sandbox.work_dir = Some(missing.clone());
        let diagnostic = config
            .validate()
            .expect_err("a missing directory is invalid");
        assert_eq!(diagnostic.code.as_str(), "CONFIG_003");
        assert!(
            diagnostic.why.contains(&missing.display().to_string()),
            "{}",
            diagnostic.why
        );
        assert!(
            diagnostic.why.contains("existing directory"),
            "{}",
            diagnostic.why
        );
        let Some(FixAction::CopyCommand(command)) = diagnostic.fix else {
            panic!(
                "expected a command that creates the directory, got {:?}",
                diagnostic.fix
            );
        };
        assert_eq!(command, format!("mkdir -p '{}'", missing.display()));
    }

    #[test]
    fn a_work_dir_that_is_a_file_is_rejected() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("notes.txt");
        std::fs::write(&file, "x").expect("write");
        let mut config = Config::default();
        config.sandbox.work_dir = Some(file.clone());
        let why = why_of(&config);
        assert!(why.contains(&file.display().to_string()), "{why}");
        assert!(why.contains("not a file"), "{why}");
    }

    #[test]
    fn an_existing_work_dir_is_accepted() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut config = Config::default();
        config.sandbox.work_dir = Some(dir.path().to_path_buf());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn a_shell_word_escapes_single_quotes() {
        assert_eq!(super::shell_word(Path::new("/a b")), "'/a b'");
        assert_eq!(super::shell_word(Path::new("/it's")), "'/it'\\''s'");
    }
}
