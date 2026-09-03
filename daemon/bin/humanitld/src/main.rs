//! Hintergrunddienst: Proxy, Sandbox-Verwaltung, Aufzeichnung, gRPC-Server. Nur Verdrahtung, keine Fachlogik.
//!
//! Zwei Betriebsarten, eine Schnittstelle:
//!
//! - Ohne Argumente der echte Daemon (HUM-018): Konfiguration laden, CA
//!   öffnen, Registry und Halte-Warteschlange anlegen, eine Proxy-Sitzung auf
//!   `$XDG_RUNTIME_DIR/humanitl/proxy/proxy.sock` starten und den gRPC-Dienst
//!   auf `daemon.sock` bedienen, bis `SIGTERM` oder `SIGINT` kommt.
//! - Mit `--fake <session.jsonl>` derselbe Socket, dieselbe Token-Datei,
//!   dieselbe Schnittstelle, aber eine aufgezeichnete Sitzung statt eines
//!   Proxys — die Oberfläche merkt den Unterschied nicht (HUM-005,
//!   `fixtures/sessions/README.md`).
//!
//! Die Reihenfolge beim Start ist festgelegt und wichtig: Pfade, Konfiguration,
//! `tracing`, CA, Registry und Warteschlange, Proxy, gRPC-Dienst. Was danach
//! kommt, ist das Warten auf das Signal; danach werden die Sitzungen gestoppt
//! und Socket und Token entfernt.
//!
//! Jeder Fehlerpfad hier ist ein [`Diagnostic`]: Code, Überschrift, Grund und,
//! wo es einen gibt, ein Vorschlag zur Behebung. `main` schreibt ihn als eine
//! Zeile (plus eine für den Vorschlag) und endet mit Status 1.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::fs::{self, Permissions};
use std::io;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use humanitl_config::{AskMode, Config, DIR_MODE, Paths as XdgPaths};
use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, SessionId, Severity};
use humanitl_ipc::fake::{FakeDaemon, FakeOptions, Session};
use humanitl_ipc::{DaemonService, IpcServer, auth, bind_socket, v1};
use humanitl_proxy::ca::{CaStore, DEFAULT_LEAF_CAPACITY, LeafCache};
use humanitl_proxy::egress::Direct;
use humanitl_proxy::handler::ProxyLimits;
use humanitl_proxy::pipeline::FlowPipeline;
use humanitl_proxy::upstream::ClientTls;
use humanitl_proxy::{
    AskPipeline, ConnMeta, FlowHandler, FlowRegistry, HoldQueue, ProxyCore, SystemResolver,
    Upstream,
};
use tokio::net::UnixListener;
// tonic bringt `tokio-stream` mit dem Feature `net` bereits mit (über sein
// `server`-Feature); der Wrapper von dort erspart diesem Binary eine eigene
// Abhängigkeit außerhalb von `[workspace.dependencies]`.
use tonic::codegen::tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::Server;

/// Der Hintergrunddienst von Humanitl.
#[derive(Debug, Parser)]
#[command(name = "humanitld", version, about, long_about = None)]
struct Cli {
    /// Spielt eine aufgezeichnete Sitzung statt eines echten Proxys.
    #[arg(long, value_name = "SESSION.JSONL")]
    fake: Option<PathBuf>,

    /// Zeitraffer: teilt alle Zeitstempel der Sitzung durch diesen Wert.
    /// Eine endliche Zahl über null.
    #[arg(long, default_value_t = 1.0, value_name = "N", value_parser = parse_speed)]
    speed: f64,

    /// Startet die Sitzung nach dem Ende neu, mit neuen Flow-Ids.
    #[arg(long = "loop")]
    repeat: bool,

    /// Rafft auch die Wartezeiten der `hold`-Zeilen mit `--speed`.
    #[arg(long)]
    scale_timeouts: bool,

    /// Wartezeit für `hold`-Zeilen ohne eigenen Wert, in Sekunden.
    #[arg(long, default_value_t = 300, value_name = "SECS")]
    hold_timeout_secs: u64,

    /// Kapazität des Ereignis-Rundfunks (`limits.event_buffer`), mindestens 1.
    #[arg(
        long,
        default_value_t = 1024,
        value_name = "N",
        value_parser = clap::builder::RangedU64ValueParser::<usize>::new().range(1..)
    )]
    event_buffer: usize,

    /// Abweichender Pfad des gRPC-Sockets; die Token-Datei liegt daneben.
    #[arg(long, value_name = "PATH")]
    socket: Option<PathBuf>,
}

/// `--speed` war keine endliche Zahl über null.
#[derive(Debug, thiserror::Error)]
#[error("speed must be a finite number greater than 0, got {0:?}")]
struct SpeedError(String);

/// Liest `--speed`. `nan`, `inf`, null und negative Werte enden hier mit
/// einer klaren Meldung statt später als Panik in `Duration::div_f64`.
fn parse_speed(text: &str) -> Result<f64, SpeedError> {
    match text.trim().parse::<f64>() {
        Ok(speed) if speed.is_finite() && speed > 0.0 => Ok(speed),
        _ => Err(SpeedError(text.to_owned())),
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(diagnostic) => {
            eprintln!("humanitld: {diagnostic}");
            if let Some(hint) = fix_hint(diagnostic.fix.as_ref()) {
                eprintln!("humanitld: fix: {hint}");
            }
            ExitCode::FAILURE
        }
    }
}

/// Ein Behebungsvorschlag als eine Zeile für das Terminal.
fn fix_hint(fix: Option<&FixAction>) -> Option<String> {
    Some(match fix? {
        FixAction::SetEnv { key, value } => format!("export {key}={value}"),
        FixAction::ChangeSetting { key, value } => format!("{key} = {value}"),
        FixAction::CopyCommand(command) | FixAction::OpenUrl(command) => command.clone(),
        FixAction::RemountReadOnly(path) => format!("remount read-only: {}", path.display()),
        other @ (FixAction::AddRule(_) | FixAction::InstallService) => other.as_str().to_owned(),
    })
}

/// Schaltet `tracing` auf JSON nach `stderr`.
///
/// Eine Zeile je Ereignis, damit `journald` und `humanitl doctor` sie ohne
/// Zwischenschritt lesen können. Die Stufe kommt aus `RUST_LOG`, sonst `info`.
fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .json()
        .with_env_filter(filter)
        .with_writer(io::stderr)
        .try_init();
}

/// Der Lauf selbst; jeder Fehler kommt als Befund zurück.
async fn run() -> Result<(), Diagnostic> {
    let cli = Cli::parse();
    init_tracing();
    match cli.fake.clone() {
        Some(path) => run_fake(&cli, &path).await,
        None => run_daemon(&cli).await,
    }
}

/// Der echte Daemon (HUM-018).
///
/// Reihenfolge wie im Modul-Kommentar. Der Proxy startet vor dem gRPC-Dienst,
/// damit ein Client, der auf `GetInfo` antwortet bekommt, auch eine Sitzung
/// vorfindet; der gRPC-Dienst räumt am Ende Socket und Token weg, diese
/// Funktion die Proxy-Sitzung.
async fn run_daemon(cli: &Cli) -> Result<(), Diagnostic> {
    let xdg = XdgPaths::from_process();

    // Zuerst die Frage, ob hier schon ein Daemon läuft, und erst dann alles,
    // was Dateien anlegt. Ein zweiter Start darf dem ersten nichts wegnehmen:
    // der Proxy-Socket wird beim Binden ersetzt, und ein später abgebrochener
    // zweiter Lauf hätte dem ersten damit den Weg in die Sandbox abgeschnitten.
    let paths = Runtime::resolve(cli.socket.clone())?;
    free_socket(&paths.socket, ADVICE_DAEMON_SOCKET)?;
    free_socket(&xdg.proxy_socket(), ADVICE_PROXY_SOCKET)?;

    let config = load_config(&xdg)?;

    let ca = Arc::new(CaStore::open(&xdg)?);
    tracing::info!(
        dir = %ca.dir().display(),
        fingerprint = %ca.fingerprint_sha256(),
        created = ca.was_created(),
        "certificate authority ready"
    );

    let registry = Arc::new(FlowRegistry::new(&config.limits));
    let queue = Arc::new(HoldQueue::with_registry(&config.limits, registry));

    let proxy = ProxyCore::new();
    let session = SessionId::new();
    let proxy_socket = proxy.start_session(
        session,
        &xdg.proxy_socket(),
        build_handler(&config, &queue, &ca)?,
        ConnMeta::plain(session),
    )?;
    tracing::info!(
        socket = %proxy_socket.display(),
        session = %session,
        "proxy session started"
    );

    let server = IpcServer::new(Arc::clone(&queue), &config, Some(session));
    let result = humanitl_ipc::serve(&paths.socket, &paths.token, server, shutdown()).await;

    // Erst die Sitzungen, dann zurückkehren: der Accept-Loop endet, und mit
    // ihm verschwindet der Socket, den die Sandbox eingehängt hätte.
    proxy.stop_session(session);
    tracing::info!(session = %session, "proxy session stopped");
    result
}

/// Lädt die Konfiguration und meldet, was das Laden überlebt hat.
fn load_config(xdg: &XdgPaths) -> Result<Config, Diagnostic> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let sources = humanitl_config::discover_with(xdg.env(), &cwd, None);
    let resolved = humanitl_config::load(&sources)?;
    for diagnostic in &resolved.diagnostics {
        tracing::warn!(
            code = %diagnostic.code,
            why = %diagnostic.why,
            "configuration"
        );
    }
    if let Some(diagnostic) = xdg.runtime_dir().diagnostic {
        tracing::info!(code = %diagnostic.code, why = %diagnostic.why, "runtime directory");
    }
    if resolved.config.resolver.test_ca.is_some() {
        tracing::warn!("resolver.test_ca is set but the daemon does not read it yet (HUM-024)");
    }
    Ok(resolved.config)
}

/// Baut den Handler, der jede Verbindung der Sitzung bedient.
///
/// Die Ports kommen aus der Konfiguration: `Direct` als Egress (ADR-017),
/// der System-Resolver, die eigene CA für die Leaf-Zertifikate und die
/// Halte-Warteschlange als Pipeline. Ohne Frage (`hold.ask_mode = none`) ist
/// die Frist null, und die Warteschlange blockt jede Anfrage sofort — sie
/// lässt nie etwas ungefragt durch.
fn build_handler(
    config: &Config,
    queue: &Arc<HoldQueue>,
    ca: &Arc<CaStore>,
) -> Result<FlowHandler, Diagnostic> {
    let client_tls = ClientTls::new(&[], config.experimental.h2_upstream)?;
    let upstream = Upstream::new(
        Arc::new(Direct::new(Duration::from_secs(
            config.limits.connect_timeout_secs,
        ))),
        Arc::new(SystemResolver),
        client_tls,
        config.resolver.prefer,
        Duration::from_secs(config.limits.header_timeout_secs),
    );
    let timeout = match config.hold.ask_mode {
        AskMode::None => Duration::ZERO,
        AskMode::Ui | AskMode::Terminal => Duration::from_secs(config.hold.timeout_secs),
    };
    let pipeline: Arc<dyn FlowPipeline> = Arc::new(AskPipeline::new(Arc::clone(queue), timeout));
    Ok(FlowHandler::new(
        Arc::clone(queue),
        pipeline,
        upstream,
        Arc::new(LeafCache::new(Arc::clone(ca), DEFAULT_LEAF_CAPACITY)),
        ProxyLimits::from_config(&config.limits, &config.recorder),
    ))
}

/// Der Abspieler einer aufgezeichneten Sitzung (HUM-005).
async fn run_fake(cli: &Cli, path: &Path) -> Result<(), Diagnostic> {
    let session = Session::load(path).map_err(|error| error.diagnostic())?;
    tracing::info!(
        file = %path.display(),
        lines = session.lines().len(),
        span_ms = session.span_ms(),
        "session loaded"
    );

    // Erst die Pfade, dann der Abspieler: scheitert der Start am Socket, hat
    // noch nichts angefangen zu laufen.
    let paths = Runtime::resolve(cli.socket.clone())?;

    let daemon = FakeDaemon::new(
        session,
        FakeOptions {
            speed: cli.speed,
            repeat: cli.repeat,
            scale_timeouts: cli.scale_timeouts,
            hold_timeout: Duration::from_secs(cli.hold_timeout_secs),
            event_buffer: cli.event_buffer,
        },
    );
    daemon.start();
    serve(daemon, &paths).await
}

/// Wem das Verzeichnis gehört, in dem der Socket liegt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DirOwner {
    /// Das Laufzeitverzeichnis des Daemons aus `humanitl-config`: anlegen,
    /// `0700` setzen, auch wenn es schon da ist.
    Daemon,
    /// Ein Verzeichnis, das der Nutzer über `--socket` gewählt hat: nur
    /// anlegen (dann `0700`), wenn es fehlt; ein vorhandenes behält seine
    /// Rechte, muss aber dem Nutzer gehören und `0700` sein.
    User,
}

/// Wo Socket und Token dieses Laufs liegen.
///
/// Die Pfade selbst kommen aus `humanitl-config`; hier wird nur angelegt und
/// aufgeräumt. Das eigene Verzeichnis bekommt `0700`, Socket und Token `0600`:
/// ein Socket, den die halbe Maschine öffnen darf, wäre der bequemste Weg an
/// jeder Entscheidung vorbei.
#[derive(Debug)]
struct Runtime {
    dir: PathBuf,
    socket: PathBuf,
    token: PathBuf,
}

impl Runtime {
    /// Bestimmt die Pfade und bereitet das Verzeichnis vor.
    ///
    /// Ohne `--socket` gilt `$XDG_RUNTIME_DIR/humanitl/daemon.sock` samt der
    /// Rückfallwege aus `humanitl-config`; mit `--socket` liegt die
    /// Token-Datei neben dem Socket, damit ein Client beides an einer Stelle
    /// findet.
    fn resolve(socket: Option<PathBuf>) -> Result<Self, Diagnostic> {
        if let Some(path) = socket {
            return Self::at(path, DirOwner::User);
        }
        let xdg = XdgPaths::from_process();
        let runtime = xdg.runtime_dir();
        if let Some(diagnostic) = &runtime.diagnostic {
            tracing::info!(
                code = %diagnostic.code,
                why = %diagnostic.why,
                "runtime directory"
            );
        }
        Self::at(xdg.daemon_socket(), DirOwner::Daemon)
    }

    /// Prüft den Pfad, bevor irgendetwas geschrieben wird, und legt dann an,
    /// was dem Daemon gehört.
    fn at(socket: PathBuf, owner: DirOwner) -> Result<Self, Diagnostic> {
        check_sun_path(&socket, owner)?;
        let runtime = Self::beside(socket);
        prepare_dir(&runtime.dir, owner)?;
        Ok(runtime)
    }

    /// Die Pfade neben einem Socket, ohne das Dateisystem anzufassen.
    ///
    /// Ein nackter Dateiname liegt im Arbeitsverzeichnis.
    fn beside(socket: PathBuf) -> Self {
        let dir = match socket.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
            _ => PathBuf::from("."),
        };
        Self {
            token: dir.join("token"),
            socket,
            dir,
        }
    }
}

/// Legt das Verzeichnis an, wenn es dem Daemon gehört oder fehlt.
///
/// Ein vorhandenes Verzeichnis des Nutzers wird nie umgestellt: `--socket
/// ~/x.sock` darf das Heimatverzeichnis nicht auf `0700` setzen. Es wird aber
/// auch nicht hingenommen, wenn es offen ist: Socket und Token gehören in ein
/// Verzeichnis, das nur der Nutzer öffnen kann ([`check_private_dir`]).
fn prepare_dir(dir: &Path, owner: DirOwner) -> Result<(), Diagnostic> {
    if owner == DirOwner::Daemon || !dir.exists() {
        fs::create_dir_all(dir)
            .map_err(|error| io_diagnostic("create the runtime directory", dir, &error))?;
        fs::set_permissions(dir, Permissions::from_mode(DIR_MODE))
            .map_err(|error| io_diagnostic("set 0700 on the runtime directory", dir, &error))?;
        return Ok(());
    }
    check_private_dir(dir, XdgPaths::from_process().env().uid())
}

/// Weist ein vorhandenes Socket-Verzeichnis ab, das nicht `uid` gehört oder
/// nicht `0700` ist (`DAEMON_004`).
///
/// Ein Verzeichnis, das andere öffnen dürfen, verrät Socket und Token: die
/// Dateien selbst sind `0600`, doch ein Nachbar könnte sie unter dem Namen
/// ersetzen, bevor der Daemon sie anlegt. Der Vorschlag ist `chmod 700`, oder
/// ohne `--socket` das eigene Laufzeitverzeichnis zu nehmen.
fn check_private_dir(dir: &Path, uid: u32) -> Result<(), Diagnostic> {
    use std::os::unix::fs::MetadataExt as _;

    let metadata = fs::metadata(dir)
        .map_err(|error| io_diagnostic("read the socket directory", dir, &error))?;
    let refuse = |why: String| {
        Diagnostic::builder(codes::DAEMON_004, Severity::Blocking)
            .title("Socket-Verzeichnis nicht privat")
            .why(why)
    };
    if !metadata.is_dir() {
        return Err(refuse(format!("{} is not a directory", dir.display())).build());
    }
    if metadata.uid() != uid {
        let own = XdgPaths::from_process().daemon_socket();
        return Err(refuse(format!(
            "the socket directory {} belongs to uid {}, not to you (uid {uid}); \
             leave out --socket to use your own runtime directory",
            dir.display(),
            metadata.uid()
        ))
        .fix(FixAction::CopyCommand(format!(
            "humanitld --fake <session.jsonl> --socket {}",
            own.display()
        )))
        .build());
    }
    let mode = metadata.mode() & 0o777;
    if mode & 0o077 != 0 {
        return Err(refuse(format!(
            "the socket directory {} is mode {mode:04o}; socket and token need a directory \
             only you can open (0700)",
            dir.display()
        ))
        .fix(FixAction::CopyCommand(format!(
            "chmod 700 {}",
            dir.display()
        )))
        .build());
    }
    Ok(())
}

/// Die Grenze für den Pfad eines Unix-Sockets (`sun_path`), inklusive der Null.
const SUN_PATH_MAX: usize = 108;

/// Weist einen Socket-Pfad ab, der nicht in `sun_path` passt.
fn check_sun_path(socket: &Path, owner: DirOwner) -> Result<(), Diagnostic> {
    let len = socket.as_os_str().len();
    if len < SUN_PATH_MAX {
        return Ok(());
    }
    let diagnostic = Diagnostic::builder(codes::CONFIG_003, Severity::Blocking)
        .title("Socket-Pfad zu lang")
        .why(format!(
            "the socket path is {len} bytes, a unix socket allows {}: {}",
            SUN_PATH_MAX - 1,
            socket.display()
        ));
    let diagnostic = match owner {
        DirOwner::Daemon => diagnostic.fix(FixAction::SetEnv {
            key: "XDG_RUNTIME_DIR".to_owned(),
            value: format!("/run/user/{}", XdgPaths::from_process().env().uid()),
        }),
        DirOwner::User => diagnostic,
    };
    Err(diagnostic.build())
}

/// Ein Fehler des Dateisystems beim Start, als Befund (`DAEMON_004`).
///
/// Laufzeitverzeichnis, Socket oder Token ließen sich nicht anlegen; `what`
/// nennt den Schritt, `path` den Ort.
fn io_diagnostic(what: &str, path: &Path, error: &io::Error) -> Diagnostic {
    Diagnostic::builder(codes::DAEMON_004, Severity::Blocking)
        .why(format!("cannot {what} {}: {error}", path.display()))
        .build()
}

/// Was ein belegter gRPC-Socket dem Nutzer rät.
const ADVICE_DAEMON_SOCKET: &str = "stop it or pass --socket with another path";

/// Was ein belegter Proxy-Socket dem Nutzer rät.
///
/// Für ihn gibt es keinen zweiten Pfad: er ist der eine Socket, den der
/// Launcher in die Sandbox einhängt (HUM-011).
const ADVICE_PROXY_SOCKET: &str = "stop the running daemon before starting another one";

/// Räumt einen verwaisten Socket weg, weigert sich aber bei einem lebenden
/// (`DAEMON_003`).
///
/// Der Verbindungsversuch ist die einzige verlässliche Prüfung: eine
/// Socket-Datei bleibt liegen, wenn ein Daemon abstürzt, und eine PID-Datei
/// wäre eine zweite Wahrheit. `advice` sagt, was für diesen Socket zu tun ist.
fn free_socket(path: &Path, advice: &str) -> Result<(), Diagnostic> {
    // `symlink_metadata`, nicht `exists`: `exists` folgt einem Symlink und
    // meldet fuer einen haengenden Link "nicht da", der Eintrag bliebe liegen
    // und der Bind darauf schluege fehl.
    if fs::symlink_metadata(path).is_err() {
        return Ok(());
    }
    if std::os::unix::net::UnixStream::connect(path).is_ok() {
        return Err(Diagnostic::builder(codes::DAEMON_003, Severity::Blocking)
            .why(format!(
                "a daemon is already listening on {}; {advice}",
                path.display()
            ))
            .build());
    }
    fs::remove_file(path).map_err(|error| io_diagnostic("remove the stale socket", path, &error))
}

/// Bedient die Schnittstelle, bis `SIGTERM` oder `SIGINT` kommt.
///
/// Aufgeräumt wird nur, was dieser Lauf selbst angelegt hat: die Token-Datei,
/// sobald sie geschrieben ist, der Socket, sobald er gebunden ist. Ein Lauf,
/// der am belegten Socket eines anderen Daemons scheitert, lässt dessen Socket
/// und Token stehen. Eine liegen gebliebene eigene Token-Datei wäre dagegen
/// ein Schlüssel zu einem Dienst, den es nicht gibt; darum verschwindet sie
/// auch, wenn der Start nach ihr scheitert.
async fn serve(daemon: FakeDaemon, paths: &Runtime) -> Result<(), Diagnostic> {
    free_socket(&paths.socket, ADVICE_DAEMON_SOCKET)?;
    let token = auth::new_token()?;
    auth::write_token(&paths.token, &token)?;
    let result = serve_bound(daemon, paths, token).await;
    let _ = fs::remove_file(&paths.token);
    tracing::info!("fake daemon stopped, socket and token removed");
    result
}

/// Bindet den Socket und entfernt ihn wieder, sobald der Dienst endet.
async fn serve_bound(daemon: FakeDaemon, paths: &Runtime, token: String) -> Result<(), Diagnostic> {
    let listener = bind_socket(&paths.socket)?;
    tracing::info!(
        socket = %paths.socket.display(),
        token = %paths.token.display(),
        dir = %paths.dir.display(),
        "fake daemon listening"
    );
    let result = serve_listener(daemon, paths, listener, token).await;
    let _ = fs::remove_file(&paths.socket);
    result
}

/// Der Dienst selbst auf einem gebundenen Socket.
async fn serve_listener(
    daemon: FakeDaemon,
    paths: &Runtime,
    listener: UnixListener,
    token: String,
) -> Result<(), Diagnostic> {
    let service =
        v1::humanitl_server::HumanitlServer::new(DaemonService::new(Arc::new(daemon), token));
    Server::builder()
        .add_service(service)
        .serve_with_incoming_shutdown(UnixListenerStream::new(listener), shutdown())
        .await
        .map_err(|error| {
            Diagnostic::builder(codes::DAEMON_001, Severity::Blocking)
                .title("gRPC-Server abgebrochen")
                .why(format!(
                    "serving {} failed: {error}",
                    paths.socket.display()
                ))
                .build()
        })
}

/// Wartet auf das Signal, das den Dienst beendet.
async fn shutdown() {
    use tokio::signal::unix::{SignalKind, signal};

    let mut terminate = match signal(SignalKind::terminate()) {
        Ok(stream) => stream,
        Err(error) => {
            tracing::warn!(%error, "cannot listen for SIGTERM, waiting for SIGINT only");
            let _ = tokio::signal::ctrl_c().await;
            return;
        }
    };
    tokio::select! {
        _ = terminate.recv() => tracing::info!("SIGTERM received"),
        result = tokio::signal::ctrl_c() => {
            if result.is_ok() {
                tracing::info!("SIGINT received");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::fs::{self, Permissions};
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
    use std::path::Path;

    use humanitl_core::FixAction;

    use super::{
        ADVICE_DAEMON_SOCKET, DirOwner, Runtime, check_private_dir, fix_hint, free_socket,
        parse_speed,
    };

    fn mode_of(path: &Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn speed_must_be_finite_and_positive() {
        assert!((parse_speed("10").unwrap() - 10.0).abs() < f64::EPSILON);
        assert!((parse_speed(" 0.5 ").unwrap() - 0.5).abs() < f64::EPSILON);
        for bad in ["nan", "inf", "-inf", "0", "-1", "fast", ""] {
            assert!(parse_speed(bad).is_err(), "{bad:?} must be refused");
        }
    }

    #[test]
    fn a_private_user_directory_is_accepted_as_it_is() {
        let dir = tempfile::tempdir().unwrap();
        fs::set_permissions(dir.path(), Permissions::from_mode(0o700)).unwrap();

        let runtime = Runtime::at(dir.path().join("d.sock"), DirOwner::User).unwrap();

        assert_eq!(mode_of(dir.path()), 0o700);
        assert_eq!(runtime.socket, dir.path().join("d.sock"));
        assert_eq!(runtime.token, dir.path().join("token"));
        assert_eq!(runtime.dir, dir.path());
    }

    #[test]
    fn a_user_directory_open_to_others_is_refused_and_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        fs::set_permissions(dir.path(), Permissions::from_mode(0o755)).unwrap();

        let error = Runtime::at(dir.path().join("d.sock"), DirOwner::User).unwrap_err();

        assert_eq!(error.code.as_str(), "DAEMON_004");
        assert_eq!(error.title, "Socket-Verzeichnis nicht privat");
        assert!(error.why.contains("0755"), "{}", error.why);
        assert!(error.why.contains("0700"), "{}", error.why);
        assert_eq!(
            fix_hint(error.fix.as_ref()),
            Some(format!("chmod 700 {}", dir.path().display()))
        );
        assert_eq!(
            mode_of(dir.path()),
            0o755,
            "a refused directory is not changed"
        );
        assert!(
            !dir.path().join("token").exists(),
            "nothing is written into it"
        );
    }

    #[test]
    fn a_user_directory_of_someone_else_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        fs::set_permissions(dir.path(), Permissions::from_mode(0o700)).unwrap();
        let own = fs::metadata(dir.path()).unwrap().uid();

        check_private_dir(dir.path(), own).unwrap();

        let error = check_private_dir(dir.path(), own.wrapping_add(1)).unwrap_err();
        assert_eq!(error.code.as_str(), "DAEMON_004");
        assert!(
            error.why.contains(&format!("belongs to uid {own}")),
            "{}",
            error.why
        );
        assert!(error.why.contains("--socket"), "{}", error.why);
        let hint = fix_hint(error.fix.as_ref()).unwrap();
        assert!(hint.starts_with("humanitld --fake"), "{hint}");
    }

    #[test]
    fn a_file_in_place_of_the_socket_directory_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("plain");
        fs::write(&file, b"").unwrap();

        let error = Runtime::at(file.join("d.sock"), DirOwner::User).unwrap_err();

        assert_eq!(error.code.as_str(), "DAEMON_004");
        assert!(error.why.contains("not a directory"), "{}", error.why);
    }

    #[test]
    fn a_user_directory_that_is_missing_is_created_as_0700() {
        let dir = tempfile::tempdir().unwrap();
        let fresh = dir.path().join("fresh");

        Runtime::at(fresh.join("d.sock"), DirOwner::User).unwrap();

        assert_eq!(mode_of(&fresh), 0o700);
    }

    #[test]
    fn the_daemons_own_directory_is_always_0700() {
        let dir = tempfile::tempdir().unwrap();
        let own = dir.path().join("humanitl");
        fs::create_dir(&own).unwrap();
        fs::set_permissions(&own, Permissions::from_mode(0o755)).unwrap();

        Runtime::at(own.join("daemon.sock"), DirOwner::Daemon).unwrap();

        assert_eq!(mode_of(&own), 0o700);
    }

    #[test]
    fn a_socket_path_too_long_for_sun_path_is_refused_before_anything_is_created() {
        let dir = tempfile::tempdir().unwrap();
        let deep = dir.path().join("x".repeat(120));

        let error = Runtime::at(deep.join("d.sock"), DirOwner::User).unwrap_err();

        assert_eq!(error.code.as_str(), "CONFIG_003");
        assert!(error.why.contains("107"));
        assert!(!deep.exists(), "nothing may be created for a refused path");

        let error = Runtime::at(deep.join("daemon.sock"), DirOwner::Daemon).unwrap_err();
        assert!(
            matches!(error.fix, Some(FixAction::SetEnv { ref key, .. }) if key == "XDG_RUNTIME_DIR")
        );
        assert!(!deep.exists());
    }

    #[test]
    fn a_live_socket_is_daemon_003_and_a_stale_one_is_removed() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("d.sock");

        assert!(
            free_socket(&socket, ADVICE_DAEMON_SOCKET).is_ok(),
            "nothing there, nothing to do"
        );

        let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        let error = free_socket(&socket, ADVICE_DAEMON_SOCKET).unwrap_err();
        assert_eq!(error.code.as_str(), "DAEMON_003");
        assert_eq!(error.title, "Socket bereits belegt");
        assert!(error.why.contains("--socket"), "{}", error.why);
        assert!(socket.exists(), "a living socket is left alone");

        drop(listener);
        // Die Datei bleibt nach dem Schließen liegen; niemand hört mehr zu.
        assert!(socket.exists());
        assert!(free_socket(&socket, ADVICE_DAEMON_SOCKET).is_ok());
        assert!(!socket.exists(), "a stale socket is removed");
    }

    #[test]
    fn a_directory_that_cannot_be_created_is_daemon_004() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("not-a-directory");
        fs::write(&file, b"").unwrap();

        let error = Runtime::at(file.join("sub").join("d.sock"), DirOwner::User).unwrap_err();

        assert_eq!(error.code.as_str(), "DAEMON_004");
        assert_eq!(
            error.title,
            "Laufzeitverzeichnis oder Socket nicht anlegbar"
        );
        assert!(
            error.why.contains("create the runtime directory"),
            "{}",
            error.why
        );
    }

    #[test]
    fn a_bare_file_name_lives_in_the_working_directory() {
        let runtime = Runtime::beside("d.sock".into());
        assert_eq!(runtime.dir, Path::new("."));
        assert_eq!(runtime.token, Path::new("./token"));
        assert_eq!(runtime.socket, Path::new("d.sock"));
    }

    #[test]
    fn fix_hints_are_one_line() {
        assert_eq!(fix_hint(None), None);
        assert_eq!(
            fix_hint(Some(&FixAction::SetEnv {
                key: "XDG_RUNTIME_DIR".to_owned(),
                value: "/run/user/1000".to_owned(),
            })),
            Some("export XDG_RUNTIME_DIR=/run/user/1000".to_owned())
        );
        assert_eq!(
            fix_hint(Some(&FixAction::CopyCommand(
                "humanitl daemon status".to_owned()
            ))),
            Some("humanitl daemon status".to_owned())
        );
        assert_eq!(
            fix_hint(Some(&FixAction::InstallService)),
            Some("install_service".to_owned())
        );
    }
}
