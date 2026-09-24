//! Der Client zum Daemon, für CLI, Oberfläche und Tests (HUM-018 Schritt 5).
//!
//! Der Daemon lauscht auf einem Unix-Socket, nicht auf einem Port. tonic 0.14
//! kennt dafür `unix://<pfad>` in der Endpunkt-URI und baut den passenden
//! Connector selbst; die Hinweise älterer Fassungen, eine Platzhalter-URI mit
//! eigenem Connector zu bauen, sind damit hinfällig (`backlog/sprint-1.md`,
//! Fallstricke von HUM-018).
//!
//! Das Token aus `$XDG_RUNTIME_DIR/humanitl/token` hängt als Interceptor an
//! jedem Aufruf. Ohne Token antwortet der Daemon auf jede RPC mit
//! `Unauthenticated`, auch auf `GetInfo`.

use std::path::Path;

use humanitl_config::Paths;
use humanitl_config::private_dir::{Entry, check_private};
use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};
use tonic::metadata::{Ascii, MetadataValue};
use tonic::service::Interceptor;
use tonic::service::interceptor::InterceptedService;
use tonic::transport::{Channel, Endpoint};
use tonic::{Request, Status};

use crate::auth::read_token;
use crate::{TOKEN_METADATA_KEY, v1};

/// Der Client, den [`connect`] liefert.
pub type Client = v1::humanitl_client::HumanitlClient<InterceptedService<Channel, TokenSender>>;

/// Hängt das Sitzungs-Token an jeden ausgehenden Aufruf.
#[derive(Debug, Clone)]
pub struct TokenSender {
    token: MetadataValue<Ascii>,
}

impl Interceptor for TokenSender {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        request
            .metadata_mut()
            .insert(TOKEN_METADATA_KEY, self.token.clone());
        Ok(request)
    }
}

/// Verbindet sich mit dem Daemon an den Pfaden aus `paths`.
///
/// # Errors
///
/// [`Diagnostic`] mit `DAEMON_001`, wenn die Token-Datei fehlt oder der Socket
/// nicht antwortet: dann läuft kein Daemon. Ebenso, wenn Laufzeitverzeichnis
/// oder Token-Datei nicht privat sind ([`trusted_token`]).
pub async fn connect(paths: &Paths) -> Result<Client, Diagnostic> {
    let socket = paths.daemon_socket();
    let token = trusted_token(paths)?;
    connect_at(&socket, &token).await
}

/// Liest das Token erst, nachdem Laufzeitverzeichnis und Token-Datei per
/// `lstat` geprüft sind: kein Symlink, eigene UID, keine Rechte für Gruppe oder
/// Andere (HUM-212).
///
/// Im Rückfall nach `$TMPDIR/humanitl-<uid>` ist der Pfad vorhersagbar. Ein
/// anderes Konto könnte dort Socket und Token vorab ablegen; ein Client, der
/// ihnen traut, schickte ihm Tastenanschläge, Arbeitsverzeichnis und
/// Einstellungen. Ist das Verzeichnis geprüft, kann niemand sonst den Socket
/// darin ersetzen, `SO_PEERCRED` braucht es dann nicht.
///
/// # Errors
///
/// [`Diagnostic`] mit `DAEMON_001`, wenn eine der Prüfungen scheitert oder das
/// Token fehlt oder leer ist.
pub fn trusted_token(paths: &Paths) -> Result<String, Diagnostic> {
    let uid = paths.env().uid();
    let token = paths.token_path();
    // Nur im `/tmp`-Rückfall hilft ein eigenes Laufzeitverzeichnis; in einer
    // Sitzung mit `/run/user/<uid>` bleibt der Vorschlag des Befunds.
    // Fehlt der Pfad, läuft bloß kein Daemon, und der Vorschlag bleibt, ihn zu
    // starten.
    let fallback = paths.runtime_dir().diagnostic.is_some();
    let check = |path: &Path, entry: Entry| {
        check_private(path, entry, uid).map_err(|refusal| refusal.into_diagnostic(fallback))
    };
    if let Some(dir) = token.parent() {
        check(dir, Entry::Dir)?;
    }
    check(&token, Entry::File)?;
    read_token(&token)
}

/// Wie [`connect`], aber mit ausdrücklichem Socket und Token.
///
/// Gedacht für Tests und für `--socket`.
///
/// # Errors
///
/// [`Diagnostic`] mit `DAEMON_001`, wenn der Socket nicht antwortet, und mit
/// `IPC_001`, wenn das Token keine gültige Kopfzeile ergibt.
pub async fn connect_at(socket: &Path, token: &str) -> Result<Client, Diagnostic> {
    let token = MetadataValue::try_from(token).map_err(|error| {
        Diagnostic::builder(codes::IPC_001, Severity::Blocking)
            .why(format!(
                "the session token is not a valid header value: {error}"
            ))
            .build()
    })?;
    let channel = channel(socket).await?;
    Ok(v1::humanitl_client::HumanitlClient::with_interceptor(
        channel,
        TokenSender { token },
    ))
}

/// Der Kanal zum Socket, ohne Token.
///
/// Damit lässt sich prüfen, ob überhaupt jemand auf dem Socket antwortet
/// (`humanitl doctor`, HUM-075). Zum Arbeiten taugt er nicht: der Daemon
/// beantwortet jede RPC ohne gültiges Token mit `Unauthenticated`, auch
/// `GetInfo`.
///
/// # Errors
///
/// [`Diagnostic`] mit `DAEMON_001`, wenn der Socket nicht antwortet.
pub async fn channel(socket: &Path) -> Result<Channel, Diagnostic> {
    let uri = format!("unix://{}", socket.display());
    let unreachable = |why: String| {
        Diagnostic::builder(codes::DAEMON_001, Severity::Blocking)
            .why(why)
            .fix(FixAction::CopyCommand("humanitld".to_owned()))
            .build()
    };
    Endpoint::from_shared(uri.clone())
        .map_err(|error| unreachable(format!("{uri} is not a usable endpoint: {error}")))?
        .connect()
        .await
        .map_err(|error| {
            unreachable(format!(
                "cannot reach the daemon on {}: {error}",
                socket.display()
            ))
        })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::fs::{self, Permissions};
    use std::os::unix::fs::PermissionsExt as _;
    use std::path::Path;

    use humanitl_config::private_dir::{own_runtime_dir_fix, process_uid};
    use humanitl_config::{Env, Paths};
    use humanitl_core::FixAction;
    use humanitl_core::diagnostics::codes;
    use humanitl_core::shell::shell_path;

    use super::trusted_token;

    /// Legt `<base>/humanitl/token` so an, wie der Daemon es tut: Verzeichnis
    /// `0700`, Token `0600`.
    fn runtime_with_token(base: &Path) {
        let dir = base.join("humanitl");
        fs::create_dir(&dir).unwrap();
        fs::set_permissions(&dir, Permissions::from_mode(0o700)).unwrap();
        let token = dir.join("token");
        fs::write(&token, "secret\n").unwrap();
        fs::set_permissions(&token, Permissions::from_mode(0o600)).unwrap();
    }

    fn paths(runtime: &Path, uid: u32) -> Paths {
        Paths::new(Env::from_pairs([("XDG_RUNTIME_DIR", runtime.to_str().unwrap())]).with_uid(uid))
    }

    #[test]
    fn an_own_private_runtime_dir_yields_the_token() {
        let tmp = tempfile::tempdir().unwrap();
        runtime_with_token(tmp.path());
        assert_eq!(
            trusted_token(&paths(tmp.path(), process_uid())).unwrap(),
            "secret"
        );
    }

    /// Der Weg des Befunds: Verzeichnis und Token gehören einem anderen Konto.
    /// Simuliert, indem der Client eine andere UID als die tatsächliche hat.
    #[test]
    fn a_runtime_dir_of_another_account_is_not_trusted() {
        let tmp = tempfile::tempdir().unwrap();
        runtime_with_token(tmp.path());
        let error = trusted_token(&paths(tmp.path(), process_uid() + 1)).unwrap_err();
        assert_eq!(error.code, codes::DAEMON_001);
        assert!(error.why.contains("belongs to uid"), "{}", error.why);
    }

    #[test]
    fn an_open_runtime_dir_is_not_trusted() {
        let tmp = tempfile::tempdir().unwrap();
        runtime_with_token(tmp.path());
        fs::set_permissions(tmp.path().join("humanitl"), Permissions::from_mode(0o755)).unwrap();
        let error = trusted_token(&paths(tmp.path(), process_uid())).unwrap_err();
        assert_eq!(error.code, codes::DAEMON_001);
        assert!(error.why.contains("is mode 0755"), "{}", error.why);
    }

    /// Eine gesunde Sitzung mit `XDG_RUNTIME_DIR` und offenem Token bekommt
    /// `chmod`, nicht den Befehl, der `XDG_RUNTIME_DIR` umstellt: der zerlegte
    /// die grafische Sitzung.
    #[test]
    fn a_healthy_session_with_an_open_token_gets_chmod() {
        let tmp = tempfile::tempdir().unwrap();
        runtime_with_token(tmp.path());
        let token = tmp.path().join("humanitl").join("token");
        fs::set_permissions(&token, Permissions::from_mode(0o644)).unwrap();
        let error = trusted_token(&paths(tmp.path(), process_uid())).unwrap_err();
        assert_eq!(error.code, codes::DAEMON_001);
        assert_eq!(
            error.fix,
            Some(FixAction::CopyCommand(format!(
                "chmod go-rwx {}",
                shell_path(&token)
            )))
        );
        assert_ne!(error.fix, Some(own_runtime_dir_fix()));
    }

    /// Im `/tmp`-Rückfall (kein `XDG_RUNTIME_DIR`, kein `/run/user/<uid>`)
    /// schlägt ein fremdes Verzeichnis das eigene unter dem Heimatverzeichnis
    /// vor.
    #[test]
    fn a_foreign_fallback_dir_proposes_an_own_runtime_dir() {
        let tmp = tempfile::tempdir().unwrap();
        // Eine UID ohne `/run/user/<uid>`; das Verzeichnis gehört dem echten
        // Konto und ist für diese UID damit fremd.
        let uid = 3_999_999_999;
        let dir = tmp.path().join(format!("humanitl-{uid}"));
        fs::create_dir(&dir).unwrap();
        fs::set_permissions(&dir, Permissions::from_mode(0o700)).unwrap();
        let paths =
            Paths::new(Env::from_pairs([("TMPDIR", tmp.path().to_str().unwrap())]).with_uid(uid));
        assert!(
            paths.runtime_dir().diagnostic.is_some(),
            "the fallback is taken"
        );
        let error = trusted_token(&paths).unwrap_err();
        assert_eq!(error.code, codes::DAEMON_001);
        assert_eq!(error.fix, Some(own_runtime_dir_fix()));
        assert!(error.why.contains("belongs to uid"), "{}", error.why);
    }

    /// Im Rückfall ohne Verzeichnis und ohne Token läuft bloß kein Daemon:
    /// Der Vorschlag ist `humanitld`, nicht die Anleitung zum eigenen Verzeichnis.
    #[test]
    fn a_missing_fallback_dir_proposes_starting_the_daemon() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(
            Env::from_pairs([("TMPDIR", tmp.path().to_str().unwrap())]).with_uid(3_999_999_999),
        );
        assert!(
            paths.runtime_dir().diagnostic.is_some(),
            "the fallback is taken"
        );
        let error = trusted_token(&paths).unwrap_err();
        assert_eq!(error.code, codes::DAEMON_001);
        assert_eq!(
            error.fix,
            Some(FixAction::CopyCommand("humanitld".to_owned()))
        );
    }

    #[test]
    fn a_symlinked_runtime_dir_is_not_trusted() {
        let tmp = tempfile::tempdir().unwrap();
        let elsewhere = tmp.path().join("elsewhere");
        fs::create_dir(&elsewhere).unwrap();
        runtime_with_token(&elsewhere);
        let runtime = tmp.path().join("runtime");
        fs::create_dir(&runtime).unwrap();
        std::os::unix::fs::symlink(elsewhere.join("humanitl"), runtime.join("humanitl")).unwrap();
        let error = trusted_token(&paths(&runtime, process_uid())).unwrap_err();
        assert_eq!(error.code, codes::DAEMON_001);
        assert!(error.why.contains("symlink"), "{}", error.why);
    }
}
