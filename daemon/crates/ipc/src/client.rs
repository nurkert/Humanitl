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
//!
//! Fehlt das Token, öffnet [`connect`] den Socket trotzdem einmal und wartet
//! höchstens [`WAKE_TIMEOUT`] darauf (HUM-164). Hinter `humanitld.socket`
//! startet systemd den Dienst erst bei der ersten Verbindung, und das Token
//! schreibt erst der laufende Dienst; ohne diesen Weckruf fände ein Client nie
//! ein Token, und der Dienst startete nie.

use std::path::Path;
use std::time::Duration;

use humanitl_config::Paths;
use humanitl_config::private_dir::{Entry, Refusal, check_private};
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

/// Wie lange ein Client nach dem Weckruf höchstens auf das Token wartet
/// (HUM-164).
///
/// Der Dienst hinter `humanitld.socket` schreibt das Token, sobald er steht;
/// beim ersten Start legt er dabei noch seine CA an, und auf einem
/// ausgelasteten Rechner dauert das mehr als einen Augenblick. Ohne Frist
/// hinge `daemon status` an einem Socket, hinter dem kein Dienst mehr startet.
pub const WAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// Der Abstand, in dem während des Wartens nach dem Token gesehen wird.
const WAKE_PAUSE: Duration = Duration::from_millis(50);

/// Verbindet sich mit dem Daemon an den Pfaden aus `paths`.
///
/// Fehlt nur das Token, weckt [`token_or_wake`] einen Dienst hinter dem
/// Socket und wartet höchstens [`WAKE_TIMEOUT`] auf sein Token.
///
/// # Errors
///
/// [`Diagnostic`] mit `DAEMON_001`, wenn die Token-Datei fehlt und auch nach
/// dem Weckruf ausbleibt oder der Socket nicht antwortet: dann läuft kein
/// Daemon. Ebenso, wenn Laufzeitverzeichnis oder Token-Datei nicht privat
/// sind ([`trusted_token`]).
pub async fn connect(paths: &Paths) -> Result<Client, Diagnostic> {
    let socket = paths.daemon_socket();
    let token = token_or_wake(paths, WAKE_TIMEOUT).await?;
    connect_at(&socket, &token).await
}

/// Wie [`connect`], aber ohne Weckruf, solange kein Token da ist.
///
/// Für Fragen, deren Antwort sich durch das Wecken änderte, etwa ob ein
/// neuer Konfigurationswert erst beim nächsten Start wirkt (`config set`).
/// Fehlt das Token, läuft kein Daemon, und es wird nicht mit dem Socket
/// verbunden: Hinter `humanitld.socket` startete schon diese Verbindung den
/// Dienst.
///
/// **Ein Token beweist keinen laufenden Daemon.** Nach einem Absturz kann ein
/// altes Token liegen bleiben; dann verbindet diese Funktion, und hinter
/// `humanitld.socket` weckt genau diese Verbindung den Dienst. Wer wissen will,
/// ob ein Daemon lief, fragt danach `GetInfo`: Der geweckte Daemon schreibt ein
/// neues Token und weist das alte mit `Unauthenticated` ab.
///
/// # Errors
///
/// [`Diagnostic`] mit `DAEMON_001` wie bei [`trusted_token`] und
/// [`connect_at`].
pub async fn connect_running(paths: &Paths) -> Result<Client, Diagnostic> {
    let token = trusted_token(paths)?;
    connect_at(&paths.daemon_socket(), &token).await
}

/// Das Token, und wenn es in einem geprüften Laufzeitverzeichnis fehlt, das
/// Token nach einem Weckruf über den Socket (HUM-164).
///
/// Der Weckruf ist eine gewöhnliche Verbindung zum Socket, die offen bleibt,
/// bis das Token da ist oder `limit` abläuft. Hält systemd den Socket, startet
/// es daraufhin den Dienst. Antwortet niemand auf dem Socket, kommt der Befund
/// sofort, ohne zu warten: dann läuft weder ein Daemon noch eine Socket-Unit.
///
/// Verbunden wird nur mit einem Socket in einem Verzeichnis, das
/// [`trusted_token`] schon geprüft hat; niemand sonst kann ihn dort abgelegt
/// haben (HUM-212). Das Token selbst bleibt eine Datei mit `0600` und kommt nie
/// über den Socket.
///
/// # Errors
///
/// [`Diagnostic`] mit `DAEMON_001` wie bei [`trusted_token`], dazu, wenn
/// niemand auf dem Socket antwortet oder das Token nach dem Weckruf nicht
/// binnen `limit` erscheint.
pub async fn token_or_wake(paths: &Paths, limit: Duration) -> Result<String, Diagnostic> {
    let absent = match token_state(paths) {
        TokenState::Ready(token) => return Ok(token),
        TokenState::Failed(diagnostic) => return Err(diagnostic),
        TokenState::Absent(diagnostic) => diagnostic,
    };
    let socket = paths.daemon_socket();
    // Die Verbindung bleibt bis zum Ende des Wartens offen: Sie ist die, die
    // der geweckte Dienst als erste annimmt.
    let _wake = match tokio::net::UnixStream::connect(&socket).await {
        Ok(stream) => stream,
        Err(error) => {
            let mut diagnostic = absent;
            diagnostic.why = format!(
                "{}; nothing listens on {} either ({error})",
                diagnostic.why,
                socket.display()
            );
            return Err(diagnostic);
        }
    };
    let deadline = tokio::time::Instant::now() + limit;
    loop {
        tokio::time::sleep(WAKE_PAUSE).await;
        match token_state(paths) {
            TokenState::Ready(token) => return Ok(token),
            TokenState::Failed(diagnostic) => return Err(diagnostic),
            TokenState::Absent(_) if tokio::time::Instant::now() >= deadline => {
                return Err(woke_nobody(&socket, &paths.token_path(), limit));
            }
            TokenState::Absent(_) => {}
        }
    }
}

/// `DAEMON_001`: Der Socket nahm den Weckruf an, aber kein Daemon schrieb
/// binnen der Frist ein Token.
fn woke_nobody(socket: &Path, token: &Path, limit: Duration) -> Diagnostic {
    Diagnostic::builder(codes::DAEMON_001, Severity::Blocking)
        .why(format!(
            "{} accepted a connection, but no daemon wrote the session token {} within {} ms; \
             whatever holds the socket, such as humanitld.socket, did not start a working \
             daemon",
            socket.display(),
            token.display(),
            limit.as_millis()
        ))
        .fix(FixAction::CopyCommand("humanitl daemon logs".to_owned()))
        .build()
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
    match token_state(paths) {
        TokenState::Ready(token) => Ok(token),
        TokenState::Absent(diagnostic) | TokenState::Failed(diagnostic) => Err(diagnostic),
    }
}

/// Wie es um das Token steht.
enum TokenState {
    /// Geprüft und gelesen.
    Ready(String),
    /// Das Laufzeitverzeichnis ist geprüft, das Token fehlt aber oder ist noch
    /// leer: `auth::write_token` legt die Datei an und schreibt erst danach.
    /// Ein Weckruf kann das ändern.
    Absent(Diagnostic),
    /// Alles andere, auch ein fehlendes Laufzeitverzeichnis: Darin liegt kein
    /// Socket, den ein Weckruf erreichte.
    Failed(Diagnostic),
}

/// Prüft Laufzeitverzeichnis und Token-Datei und liest das Token.
fn token_state(paths: &Paths) -> TokenState {
    let uid = paths.env().uid();
    let token = paths.token_path();
    // Nur im `/tmp`-Rückfall hilft ein eigenes Laufzeitverzeichnis; in einer
    // Sitzung mit `/run/user/<uid>` bleibt der Vorschlag des Befunds.
    // Fehlt der Pfad, läuft bloß kein Daemon, und der Vorschlag bleibt, ihn zu
    // starten.
    let fallback = paths.runtime_dir().diagnostic.is_some();
    if let Some(dir) = token.parent()
        && let Err(refusal) = check_private(dir, Entry::Dir, uid)
    {
        return TokenState::Failed(refusal.into_diagnostic(fallback));
    }
    match check_private(&token, Entry::File, uid) {
        Ok(()) => {}
        Err(Refusal::Missing(diagnostic)) => return TokenState::Absent(diagnostic),
        Err(refusal) => return TokenState::Failed(refusal.into_diagnostic(fallback)),
    }
    // Leer heißt wie in der Oberfläche: nichts außer Leerraum.
    let empty = std::fs::read_to_string(&token).is_ok_and(|text| text.trim().is_empty());
    match read_token(&token) {
        Ok(text) => TokenState::Ready(text),
        Err(diagnostic) if empty => TokenState::Absent(diagnostic),
        Err(diagnostic) => TokenState::Failed(diagnostic),
    }
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
    use std::time::{Duration, Instant};

    use humanitl_config::private_dir::{own_runtime_dir_fix, process_uid};
    use humanitl_config::{Env, Paths};
    use humanitl_core::FixAction;
    use humanitl_core::diagnostics::codes;
    use humanitl_core::shell::shell_path;

    use super::{token_or_wake, trusted_token};

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

    /// Ein Laufzeitverzeichnis wie nach `systemctl --user start
    /// humanitld.socket`: `0700`, ein lauschender Socket, kein Token.
    ///
    /// Unter `/tmp`, damit der Socket-Pfad in `sun_path` passt (108 Bytes).
    fn activated_runtime() -> (tempfile::TempDir, tokio::net::UnixListener) {
        let tmp = tempfile::Builder::new()
            .prefix("hum164")
            .tempdir_in("/tmp")
            .unwrap();
        let dir = tmp.path().join("humanitl");
        fs::create_dir(&dir).unwrap();
        fs::set_permissions(&dir, Permissions::from_mode(0o700)).unwrap();
        let listener = tokio::net::UnixListener::bind(dir.join("daemon.sock")).unwrap();
        (tmp, listener)
    }

    /// Das Token, wie der geweckte Daemon es schreibt: `0600`.
    fn write_token(runtime: &Path) {
        let token = runtime.join("humanitl").join("token");
        fs::write(&token, "secret\n").unwrap();
        fs::set_permissions(&token, Permissions::from_mode(0o600)).unwrap();
    }

    /// Der Weg von HUM-164: Fehlt das Token, öffnet der Client den Socket, und
    /// erst diese Verbindung weckt den „Dienst", der dann sein Token schreibt.
    /// Ohne Weckruf käme keine Verbindung an, und das Token bliebe aus.
    #[tokio::test]
    async fn a_missing_token_wakes_the_socket_and_is_awaited() {
        let (tmp, listener) = activated_runtime();
        let runtime = tmp.path().to_owned();
        let service = tokio::spawn(async move {
            let (_first, _) = listener.accept().await.unwrap();
            write_token(&runtime);
            // Der Dienst hält die erste Verbindung, bis der Test endet.
            std::future::pending::<()>().await;
        });
        let token = token_or_wake(&paths(tmp.path(), process_uid()), Duration::from_secs(5)).await;
        service.abort();
        assert_eq!(
            token.map_err(|diagnostic| diagnostic.why),
            Ok("secret".to_owned())
        );
    }

    /// Die Frist aus den Fallstricken: Nimmt der Socket an, aber kein Dienst
    /// schreibt je ein Token, endet das Warten mit `DAEMON_001`, statt zu
    /// hängen.
    #[tokio::test]
    async fn a_socket_that_wakes_nobody_ends_at_the_deadline() {
        let (tmp, _listener) = activated_runtime();
        let limit = Duration::from_millis(300);
        let started = Instant::now();
        let waited = tokio::time::timeout(
            Duration::from_secs(5),
            token_or_wake(&paths(tmp.path(), process_uid()), limit),
        )
        .await;
        let elapsed = started.elapsed();
        assert!(waited.is_ok(), "the wait did not end within 5 s");
        let error = waited.unwrap().unwrap_err();
        assert_eq!(error.code, codes::DAEMON_001);
        assert!(
            error.why.contains("accepted a connection") && error.why.contains("within 300 ms"),
            "{}",
            error.why
        );
        assert!(
            elapsed >= limit,
            "gave up after {elapsed:?}, before {limit:?}"
        );
    }

    /// Lauscht niemand, gibt es nichts zu wecken: Der Befund kommt sofort,
    /// ohne die Frist abzuwarten, und sagt, dass auch der Socket fehlt.
    #[tokio::test]
    async fn without_a_listener_a_missing_token_fails_at_once() {
        let (tmp, listener) = activated_runtime();
        drop(listener);
        let started = Instant::now();
        let error = token_or_wake(&paths(tmp.path(), process_uid()), Duration::from_secs(5))
            .await
            .unwrap_err();
        assert_eq!(error.code, codes::DAEMON_001);
        assert!(error.why.contains("nothing listens on"), "{}", error.why);
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "waited {:?} for a socket nobody holds",
            started.elapsed()
        );
    }
}
