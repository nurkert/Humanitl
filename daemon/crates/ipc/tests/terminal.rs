//! Die `Terminal`-RPC an einer laufenden Sitzung (HUM-042).
//!
//! Ein Schreiber, beliebig viele Leser, ein Ringpuffer mit gefilterten Bytes.
//! Die Fragen, die sich nur hier beantworten lassen und nicht am Filter
//! allein: Bekommt ein zweiter Schreiber `TERM_001`? Sieht ein Leser dasselbe
//! wie der Schreiber, ohne selbst schreiben zu können? Spielt ein Anhängender
//! den Rückstand ab, und ist er gefiltert?
//!
//! Der Test braucht `bwrap`, einen Kernel mit unprivilegierten
//! Nutzer-Namensräumen und den gebauten Shim neben dem Testbinary. Fehlt
//! eines, sagt er es auf stderr und endet grün: „kein `bwrap` auf dieser
//! Maschine" ist eine Aussage über die Maschine, nicht über den Dienst.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::os::unix::fs::PermissionsExt as _;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::time::Duration;

use humanitl_config::{Config, Env, Paths};
use humanitl_core::SessionId;
use humanitl_ipc::sandbox::SandboxPorts;
use humanitl_ipc::session::SessionResolver;
use humanitl_ipc::{SandboxService, TerminalHub, v1};
use tokio::sync::mpsc;
use tokio_stream::StreamExt as _;
use tokio_stream::wrappers::ReceiverStream;

/// Das mitgelieferte Profil, damit die Kommandozeile dieselbe ist, die im
/// Produkt startet.
const PROFILE: &str = "default";

/// Der Agent dieses Tests: Er schreibt eine Marke, versucht die Zwischenablage
/// des Menschen zu beschreiben und liest danach, was der Schreiber tippt.
///
/// `printf` schreibt die Folgen wörtlich; `echo -e` ist nicht portabel.
const AGENT: &str = "printf 'READY\\r\\n'; \
                     printf '\\033]52;c;c2VjcmV0\\007'; \
                     printf '\\033[2J'; \
                     while read -r line; do printf 'GOT %s\\r\\n' \"$line\"; done";

/// Kein Test wartet länger auf ein Stück Ausgabe.
const WAIT: Duration = Duration::from_secs(20);

struct Fixture {
    _dir: tempfile::TempDir,
    paths: Paths,
    work: PathBuf,
    _proxy: UnixListener,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let home = dir.path().join("home");
        let runtime = dir.path().join("runtime");
        std::fs::create_dir_all(&home).expect("home");
        std::fs::create_dir_all(&runtime).expect("runtime");
        let paths = Paths::new(
            Env::from_process()
                .with("HOME", home.to_string_lossy())
                .with("XDG_RUNTIME_DIR", runtime.to_string_lossy())
                .with("XDG_CONFIG_HOME", home.join(".config").to_string_lossy())
                .with("XDG_DATA_HOME", home.join(".local/share").to_string_lossy()),
        );

        let profiles = paths.profiles_dir().join("sandbox");
        std::fs::create_dir_all(&profiles).expect("profile directory");
        let bundled = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../profiles/sandbox")
            .join(format!("{PROFILE}.toml"));
        std::fs::copy(&bundled, profiles.join(format!("{PROFILE}.toml")))
            .unwrap_or_else(|err| panic!("{} is readable: {err}", bundled.display()));

        std::fs::create_dir_all(paths.proxy_socket_dir()).expect("proxy directory");
        let proxy = UnixListener::bind(paths.proxy_socket()).expect("bind the proxy socket");
        std::fs::set_permissions(paths.proxy_socket(), std::fs::Permissions::from_mode(0o600))
            .expect("chmod the proxy socket");

        let ca = paths.ca_dir();
        std::fs::create_dir_all(&ca).expect("ca directory");
        std::fs::write(paths.ca_cert_path(), b"-----BEGIN CERTIFICATE-----\n").expect("ca");
        std::fs::write(ca.join("ca-bundle.crt"), b"-----BEGIN CERTIFICATE-----\n")
            .expect("ca bundle");

        let work = home.join("project");
        std::fs::create_dir_all(&work).expect("work");

        Self {
            _dir: dir,
            paths,
            work,
            _proxy: proxy,
        }
    }

    fn service(&self) -> SandboxService {
        SandboxService::new(
            SessionResolver::for_config(self.paths.clone(), Config::default()),
            SessionId::new(),
            SandboxPorts::none(),
        )
    }

    fn start(&self) -> v1::SandboxRequest {
        v1::SandboxRequest {
            op: Some(v1::sandbox_request::Op::Start(v1::sandbox_request::Start {
                profile: PROFILE.to_owned(),
                work_dir: self.work.display().to_string(),
                work_mode: "rw".to_owned(),
                command: vec!["/bin/sh".to_owned(), "-c".to_owned(), AGENT.to_owned()],
                session_profile: String::new(),
                ask_mode: String::new(),
                cli_overrides: Vec::new(),
            })),
        }
    }
}

/// Die Marke, an der `tests/escape/esc-5-filesystem.sh` einen übersprungenen
/// Fall von einem bestandenen unterscheidet.
///
/// „Das Werkzeug fehlt" darf nie als „die Sandbox hat gehalten" gelesen werden
/// (`tests/escape/lib.sh`).
const SKIP_MARKER: &str = "ESC5-SKIP";

/// Ob dieser Rechner den Test tragen kann; sonst die Begründung auf stderr.
///
/// **Unter `CI` ist das Fehlen ein Fehler und kein Grund zu überspringen.** Wer
/// hier `false` bekommt, kehrt zurück, und ein zurückkehrender Test gilt dem
/// Testläufer als bestanden: Diese Datei meldete dann `ok` mit null
/// Zusicherungen, und die beiden ESC-5-Fälle des Terminals (Kanal 3, OSC 52 und
/// OSC 8) wären nie geprüft worden — während der Bericht sie als bestanden
/// führte. Auf einer Entwicklermaschine darf `bwrap` fehlen, auf dem Runner
/// nicht; dieselbe Regel wie in `daemon/bin/humanitl/tests/cli.rs` und
/// `daemon/crates/sandbox/tests/shim_contract.rs`.
fn usable(fixture: &Fixture) -> bool {
    if let Err(diagnostic) = humanitl_sandbox::BwrapBackend::detect(fixture.paths.clone()) {
        return refuse_under_ci(
            &format!("bwrap is not usable here: {}", diagnostic.why),
            "install it (apt-get install -y bubblewrap) and allow unprivileged user namespaces \
             (sysctl -w kernel.apparmor_restrict_unprivileged_userns=0)",
        );
    }
    let found = std::env::current_exe().ok().is_some_and(|exe| {
        exe.parent().is_some_and(|dir| {
            dir.join("humanitl-shim").is_file()
                || dir
                    .parent()
                    .is_some_and(|up| up.join("humanitl-shim").is_file())
        })
    });
    if !found {
        return refuse_under_ci(
            "humanitl-shim is not built next to the test binary",
            "build the workspace first (cargo build --workspace --all-targets, or \
             cargo test --workspace)",
        );
    }
    true
}

/// Meldet, warum dieser Test nicht laufen kann — und scheitert unter `CI`.
///
/// Liefert immer `false`; der Rückgabewert ist nur die Bequemlichkeit des
/// Aufrufers. Die Zeile trägt [`SKIP_MARKER`], damit ESC-5 einen
/// übersprungenen Fall von einem bestandenen unterscheiden kann.
fn refuse_under_ci(why: &str, remedy: &str) -> bool {
    assert!(
        std::env::var_os("CI").is_none(),
        "under CI this test must run: {why}; {remedy}"
    );
    eprintln!("{SKIP_MARKER} {why}");
    false
}

/// Ein Client am Terminal: sein Eingangskanal und sein Strom.
struct Client {
    input: mpsc::Sender<v1::TerminalInput>,
    output: std::pin::Pin<Box<dyn tokio_stream::Stream<Item = v1::TerminalOutput> + Send>>,
}

impl Client {
    /// Meldet einen Client an diesem Terminal an.
    fn attach(hub: &TerminalHub, cols: u32, rows: u32, read_only: bool) -> Self {
        let (tx, rx) = mpsc::channel(16);
        let hub = hub.clone();
        let output =
            humanitl_ipc::terminal::serve(move |_| Ok(hub), Box::pin(ReceiverStream::new(rx)));
        let client = Self { input: tx, output };
        client.blocking_open(cols, rows, read_only);
        client
    }

    fn blocking_open(&self, cols: u32, rows: u32, read_only: bool) {
        self.input
            .try_send(v1::TerminalInput {
                input: Some(v1::terminal_input::Input::Open(v1::terminal_input::Open {
                    sandbox_id: String::new(),
                    cols,
                    rows,
                    read_only,
                })),
            })
            .expect("the session takes its Open");
    }

    async fn send(&mut self, input: v1::terminal_input::Input) {
        self.input
            .send(v1::TerminalInput { input: Some(input) })
            .await
            .expect("the session takes the message");
    }

    /// Liest, bis die Ausgabe `needle` enthält; `false`, wenn die Frist um ist
    /// oder der Strom endet.
    async fn wait_for(&mut self, seen: &mut String, needle: &str) -> bool {
        while !seen.contains(needle) {
            let Ok(Some(output)) = tokio::time::timeout(WAIT, self.output.next()).await else {
                return false;
            };
            if let Some(v1::terminal_output::Output::Data(data)) = output.output {
                seen.push_str(&String::from_utf8_lossy(&data));
            }
        }
        true
    }

    /// Die nächste Nachricht, oder `None` nach `WAIT`.
    async fn next(&mut self) -> Option<v1::TerminalOutput> {
        tokio::time::timeout(WAIT, self.output.next()).await.ok()?
    }
}

/// Startet die Sitzung und gibt ihr Terminal zurück.
async fn running(service: &SandboxService, fixture: &Fixture) -> Option<TerminalHub> {
    running_with(service, fixture.start()).await
}

/// Dasselbe mit einer selbst gebauten Anfrage.
async fn running_with(
    service: &SandboxService,
    request: v1::SandboxRequest,
) -> Option<TerminalHub> {
    let mut stream = service.stream(request);
    while let Some(event) = stream.next().await {
        if let Some(v1::sandbox_event::Event::Status(status)) = &event.event {
            if status.state == v1::SandboxState::Running as i32 {
                break;
            }
            if status.state == v1::SandboxState::Failed as i32 {
                eprintln!("{SKIP_MARKER} the sandbox did not start here");
                return None;
            }
        }
    }
    // Der Strom des Starts bleibt offen; er trägt die Ausgabe des Agenten in
    // den Ereignisstrom und speist dabei das Terminal.
    tokio::spawn(async move { while stream.next().await.is_some() {} });
    Some(
        service
            .terminal("")
            .expect("a running session has a terminal"),
    )
}

/// Führt `body` aus und beendet die Sitzung danach — auch nach einem `panic!`.
///
/// Der Rumpf läuft als eigene Aufgabe, und das ist keine Zierde: Ein
/// fehlgeschlagenes `assert!` mitten in den Zusicherungen ließe die Sandbox
/// sonst stehen, und der blockierende Leser ihrer Ausgabe hielte die Laufzeit
/// beim Abbau fest. Aus einem roten Test würde ein hängender, und ein
/// hängender Test sagt niemandem etwas.
async fn with_session<F>(service: SandboxService, body: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    let outcome = tokio::spawn(body).await;
    service.stream(stop()).next().await;
    if let Err(error) = outcome {
        std::panic::resume_unwind(error.into_panic());
    }
}

/// Ein Schreiber, beliebig viele Leser — und die Grenze steht im Daemon.
///
/// Der Test hängt vier Fragen an eine Sitzung, weil jede davon einen echten
/// Start braucht und ein Start hier zwei Sekunden kostet.
#[tokio::test(flavor = "multi_thread")]
async fn one_writer_many_readers() {
    let fixture = Fixture::new();
    if !usable(&fixture) {
        return;
    }
    let service = fixture.service();
    let Some(hub) = running(&service, &fixture).await else {
        return;
    };

    with_session(service, async move {
    let mut writer = Client::attach(&hub, 100, 30, false);
    let mut seen = String::new();
    assert!(
        writer.wait_for(&mut seen, "READY").await,
        "the writer sees the agent: {seen:?}"
    );
    assert!(
        !seen.contains("c2VjcmV0"),
        "and the clipboard sequence stays inside: {seen:?}"
    );
    assert!(
        !seen.contains('\u{1b}') || seen.contains("\u{1b}[2J"),
        "what a full-screen agent needs passes: {seen:?}"
    );

    // Ein zweiter Schreiber: `TERM_001`, und sein Strom endet.
    let mut second = Client::attach(&hub, 100, 30, false);
    let refused = second.next().await.expect("the second writer hears back");
    match refused.output {
        Some(v1::terminal_output::Output::Diagnostic(diagnostic)) => {
            assert_eq!(diagnostic.code, "TERM_001", "{diagnostic:?}");
        }
        other => panic!("the second writer is refused, not served: {other:?}"),
    }
    assert!(
        second.next().await.is_none(),
        "and the refused stream ends there"
    );

    // Ein Leser wird angenommen und sieht denselben Rückstand.
    let mut reader = Client::attach(&hub, 40, 10, true);
    let mut read = String::new();
    assert!(
        reader.wait_for(&mut read, "READY").await,
        "the reader gets the scrollback: {read:?}"
    );
    assert!(
        !read.contains("c2VjcmV0"),
        "the ring holds filtered bytes only, so a re-attach cannot replay the raw stream: {read:?}"
    );

    // Was der Leser schickt, fällt hier weg — nicht im Client.
    reader
        .send(v1::terminal_input::Input::Data(b"reader\n".to_vec()))
        .await;
    reader
        .send(v1::terminal_input::Input::Resize(
            v1::terminal_input::Resize { cols: 9, rows: 9 },
        ))
        .await;
    // Und was der Schreiber schickt, kommt an. Die Reihenfolge ist der Beleg:
    // Der Agent echot jede Zeile, die er liest.
    writer
        .send(v1::terminal_input::Input::Data(b"writer\n".to_vec()))
        .await;
    assert!(
        writer.wait_for(&mut seen, "GOT writer").await,
        "the writer reaches the agent: {seen:?}"
    );
    assert!(
        !seen.contains("GOT reader"),
        "and the reader never did: {seen:?}"
    );

    // Der Leser sieht dieselbe Ausgabe wie der Schreiber.
    assert!(
        reader.wait_for(&mut read, "GOT writer").await,
        "the reader sees what the writer typed: {read:?}"
    );

    // `close` beendet den Strom, nicht die Sitzung: Der Platz des Schreibers
    // wird frei, und ein neuer Schreiber bekommt ihn.
    writer.send(v1::terminal_input::Input::Close(())).await;
    assert!(
        writer.next().await.is_none(),
        "the closed stream ends without an exit"
    );
    // Der Platz wird beim Fallenlassen frei; das braucht einen Umlauf.
    drop(writer);
    let mut third = wait_for_writer_slot(&hub).await;
    let mut again = String::new();
    assert!(
        third.wait_for(&mut again, "READY").await,
        "and the scrollback is still there after a re-attach: {again:?}"
    );
    })
    .await;
}

/// Wartet, bis der Platz des Schreibers frei ist, und nimmt ihn.
async fn wait_for_writer_slot(hub: &TerminalHub) -> Client {
    for _ in 0..50 {
        let mut candidate = Client::attach(hub, 100, 30, false);
        match candidate.next().await {
            Some(v1::TerminalOutput {
                output: Some(v1::terminal_output::Output::Diagnostic(_)),
            }) => tokio::time::sleep(Duration::from_millis(20)).await,
            Some(_) => return candidate,
            None => panic!("the stream ended before the terminal answered"),
        }
    }
    panic!("the writer slot never became free");
}

fn stop() -> v1::SandboxRequest {
    v1::SandboxRequest {
        op: Some(v1::sandbox_request::Op::Stop(())),
    }
}

/// ESC-5: Die Zwischenablage des Menschen bleibt zu.
///
/// Der Agent schreibt `\e]52;c;…\a` an sein Terminal — die Folge, mit der ein
/// Terminal in die Zwischenablage des Hosts schreibt. Sie darf den Daemon
/// nicht verlassen; erreicht sie keinen Client, erreicht sie auch kein
/// Terminal, das sie ausführen könnte (`BACKLOG.md` 4.2, `docs/SECURITY.md`
/// 3.3, `tests/escape/esc-5-filesystem.sh`).
#[tokio::test(flavor = "multi_thread")]
async fn osc52_does_not_reach_host() {
    let Some((_fixture, service, mut client, seen)) = attacking_session(
        "printf 'MARK-A\\r\\n'; \
         printf '\\033]52;c;c2VjcmV0\\007'; \
         printf '\\235052;c;c2VjcmV0\\007'; \
         printf '\\302\\2352;c;eA==\\007'; \
         printf 'MARK-B\\r\\n'; \
         while :; do sleep 0.05; done",
    )
    .await
    else {
        return;
    };
    with_session(service, async move {
        let mut seen = seen;
        assert!(
            client.wait_for(&mut seen, "MARK-B").await,
            "the agent wrote both marks: {seen:?}"
        );
        assert!(seen.contains("MARK-A"), "{seen:?}");
        for forbidden in ["c2VjcmV0", "\u{1b}]52", "\u{9d}52", "eA=="] {
            assert!(
                !seen.contains(forbidden),
                "{forbidden:?} must not reach a terminal: {seen:?}"
            );
        }
        assert!(
            !seen.contains('\u{1b}'),
            "and nothing of the sequences at all: {seen:?}"
        );
    })
    .await;
}

/// ESC-5: Ein Verweis unter sichtbarem Text und der Fenstertitel bleiben
/// wirkungslos.
///
/// OSC 8 legt eine fremde Adresse unter harmlosen Text, OSC 0 und OSC 2 setzen
/// den Fenstertitel — beides sind Wege, mit denen die Ausgabe des Agenten
/// etwas behauptet, das nicht von ihm kommt. Sichtbar bleibt der Text, die
/// Folge nicht.
#[tokio::test(flavor = "multi_thread")]
async fn osc8_and_title_are_inert() {
    let Some((_fixture, service, mut client, seen)) = attacking_session(
        "printf 'MARK-A\\r\\n'; \
         printf '\\033]8;;https://evil.example/\\007click me\\033]8;;\\007\\r\\n'; \
         printf '\\033]0;All Checks Passed\\007'; \
         printf '\\033]2;All Checks Passed\\033\\\\'; \
         printf 'MARK-B\\r\\n'; \
         while :; do sleep 0.05; done",
    )
    .await
    else {
        return;
    };
    with_session(service, async move {
        let mut seen = seen;
        assert!(
            client.wait_for(&mut seen, "MARK-B").await,
            "the agent wrote both marks: {seen:?}"
        );
        assert!(
            seen.contains("click me"),
            "the text stays, only the link goes: {seen:?}"
        );
        for forbidden in ["evil.example", "All Checks Passed", "\u{1b}]"] {
            assert!(
                !seen.contains(forbidden),
                "{forbidden:?} must not reach a terminal: {seen:?}"
            );
        }
    })
    .await;
}

/// Startet eine Sitzung mit diesem Angriffsskript und hängt einen Schreiber
/// an; `None`, wenn diese Maschine den Test nicht tragen kann.
async fn attacking_session(script: &str) -> Option<(Fixture, SandboxService, Client, String)> {
    let fixture = Fixture::new();
    if !usable(&fixture) {
        return None;
    }
    let service = fixture.service();
    let mut start = fixture.start();
    if let Some(v1::sandbox_request::Op::Start(inner)) = start.op.as_mut() {
        inner.command = vec!["/bin/sh".to_owned(), "-c".to_owned(), script.to_owned()];
    }
    let hub = running_with(&service, start).await?;
    let client = Client::attach(&hub, 100, 30, false);
    // Das Fixture reist mit: Sein Verzeichnis verschwindet erst, wenn der Test
    // es fallen lässt, und darin liegen Profil, Socket und Projekt.
    Some((fixture, service, client, String::new()))
}

/// Eine beendete Sitzung lässt nichts stehen — keine Aufgabe, keinen
/// Deskriptor.
///
/// Der Zuhörer der Warteschlange (`HeldNotices::run`) hängt an einem Kanal,
/// der länger lebt als die Sitzung: Er endet erst, wenn die Warteschlange
/// schließt, und die gehört dem Daemon. Ohne Abbruch am Sitzungsende bliebe je
/// Sitzung eine Aufgabe stehen, die einen `TerminalHub` hält — und mit ihm den
/// `SandboxHandle` und die Herrscherseite des Pseudoterminals.
///
/// Gemessen wird an den offenen Deskriptoren des Prozesses, und verglichen
/// werden **zwei** beendete Sitzungen: Was beim ersten Start einmalig entsteht
/// (Fäden, Zwischenspeicher), fällt aus der Differenz heraus, was je Sitzung
/// liegen bleibt, nicht.
#[tokio::test(flavor = "multi_thread")]
async fn two_sessions_leave_nothing_behind() {
    let fixture = Fixture::new();
    if !usable(&fixture) {
        return;
    }
    let limits = humanitl_config::Limits::default();
    let registry = std::sync::Arc::new(humanitl_proxy::FlowRegistry::new(&limits));
    let queue = std::sync::Arc::new(humanitl_proxy::HoldQueue::with_registry(
        &limits,
        std::sync::Arc::clone(&registry),
    ));
    let service = SandboxService::new(
        SessionResolver::for_config(fixture.paths.clone(), Config::default()),
        SessionId::new(),
        SandboxPorts::none().with_notices(humanitl_ipc::HeldNotices::new(queue, registry)),
    );

    let mut open = Vec::new();
    for round in 0..2 {
        let Some(hub) = running(&service, &fixture).await else {
            return;
        };
        let mut client = Client::attach(&hub, 100, 30, false);
        let mut seen = String::new();
        assert!(
            client.wait_for(&mut seen, "READY").await,
            "round {round}: the agent runs: {seen:?}"
        );
        drop(client);
        drop(hub);
        service.stream(stop()).next().await;
        // Der Abbau läuft über mehrere Aufgaben; ein paar Umläufe genügen.
        for _ in 0..20 {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        open.push(open_descriptors());
    }

    assert!(
        open[1] <= open[0],
        "a finished session keeps nothing: {} descriptors after the first, {} after the second",
        open[0],
        open[1]
    );
    assert!(
        service.terminal("").is_err(),
        "and no terminal is left to attach to"
    );
}

/// Ein Hinweis, der auf eine Grenze wartet, geht mit dem Ende der Sitzung
/// noch hinaus.
///
/// Der Fall ist der interessanteste, den es gibt: Der Agent steht mitten in
/// einer Folge, eine Anfrage von ihm wartet auf einen Menschen, und dann endet
/// er. Wer den Hinweis hier wegwirft, verschweigt genau den Fluss, der beim
/// Ende noch offen war.
#[tokio::test(flavor = "multi_thread")]
async fn a_pending_notice_still_leaves_when_the_agent_ends() {
    // Der Agent schreibt eine halbe Folge und wartet: Der Filter steht danach
    // mitten in einer CSI-Folge, und dort darf kein Hinweis hinein.
    let Some((_fixture, service, mut client, seen)) =
        attacking_session("printf 'READY\\r\\n'; printf '\\033['; while :; do sleep 0.05; done")
            .await
    else {
        return;
    };
    let hub = service.terminal("").expect("the session runs");
    let mut seen = seen;
    assert!(
        client.wait_for(&mut seen, "READY").await,
        "the agent wrote its mark: {seen:?}"
    );
    // Warten, bis die halbe Folge wirklich im Filter steht; ohne das wäre der
    // Hinweis sofort hinausgegangen und der Test bewiese nichts.
    let deadline = tokio::time::Instant::now() + WAIT;
    while hub.at_boundary() && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(
        !hub.at_boundary(),
        "the agent stands inside a sequence, so a notice has to wait"
    );

    hub.notice("[humanitl] request held: GET example.com/ · waiting for you");
    assert!(
        !seen.contains("waiting for you"),
        "and it did wait: {seen:?}"
    );

    // Erst das Ende der Sitzung, dann das Warten: Der Hinweis geht in
    // `finish` hinaus, und `finish` kommt mit dem Ende des Agenten.
    service.stream(stop()).next().await;
    assert!(
        client.wait_for(&mut seen, "waiting for you").await,
        "the end of the session releases it: {seen:?}"
    );
    assert!(
        !seen.contains('\u{1b}'),
        "and the half-written sequence of the agent never left: {seen:?}"
    );
}

/// Wie viele Deskriptoren dieser Prozess gerade offen hat.
fn open_descriptors() -> usize {
    std::fs::read_dir("/proc/self/fd").map_or(0, std::iter::Iterator::count)
}

/// Ohne laufende Sitzung antwortet die RPC wie `Sandbox`: `IPC_006`.
#[tokio::test(flavor = "multi_thread")]
async fn a_terminal_without_a_session_says_so() {
    let fixture = Fixture::new();
    let service = fixture.service();
    let error = service.terminal("").expect_err("nothing runs here");
    assert_eq!(error.code.as_str(), "IPC_006", "{error}");
    assert!(error.why.contains("no sandbox is running"), "{}", error.why);
}
