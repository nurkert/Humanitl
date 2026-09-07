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

/// Ein Agent, der auf jede Zeile seine Fenstergröße meldet (HUM-042).
///
/// `stty size` und nicht `tput cols`: Es druckt Zeilen und Spalten in einer
/// Zeile, liest sie aus dem Terminal selbst und braucht keine
/// terminfo-Datenbank in der Sandbox.
const AGENT_SIZE: &str = "printf 'READY\\r\\n'; \
                          while read -r line; do \
                          printf 'SIZE %s\\r\\n' \"$(stty size)\"; done";

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

    /// Schreibt eine `config.toml` in dieses Fixture.
    ///
    /// Nötig, weil der Dienst seine Konfiguration beim Start **neu auflöst**
    /// (`SessionResolver::resolve` liest Dateien und Umgebung); ein Wert, den
    /// nur `for_config` kennt, wäre nach dem ersten Start wieder die Vorgabe.
    fn write_config(&self, body: &str) {
        let path = self.paths.config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("the config directory");
        }
        std::fs::write(&path, body).expect("the config file");
    }

    /// Wie [`Fixture::start`], aber mit einem anderen Agenten.
    fn start_with(&self, agent: &str) -> v1::SandboxRequest {
        let mut request = self.start();
        if let Some(v1::sandbox_request::Op::Start(start)) = request.op.as_mut() {
            start.command = vec!["/bin/sh".to_owned(), "-c".to_owned(), agent.to_owned()];
        }
        request
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
    /// Jede Hinweiszeile, die dieser Client bekommen hat.
    ///
    /// Sie kommt als eigener Rahmen und nicht als Bytes; wer die Bytes
    /// abschreibt (`wait_for`), sieht sie deshalb nie. Genau das ist die
    /// Zusage, und deshalb steht sie hier getrennt.
    notices: Vec<String>,
}

impl Client {
    /// Meldet einen Client an diesem Terminal an.
    fn attach(hub: &TerminalHub, cols: u32, rows: u32, read_only: bool) -> Self {
        let (tx, rx) = mpsc::channel(16);
        let hub = hub.clone();
        let output =
            humanitl_ipc::terminal::serve(move |_| Ok(hub), Box::pin(ReceiverStream::new(rx)));
        let client = Self {
            input: tx,
            output,
            notices: Vec::new(),
        };
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

    /// Schickt eine Nachricht, mit Frist.
    ///
    /// Der Eingangskanal fasst sechzehn Nachrichten. Liest die Sitzung nicht
    /// mehr, laeuft er voll, und ein `send` ohne Frist wartet dann fuer immer
    /// auf Platz, der nie frei wird.
    async fn send(&mut self, input: v1::terminal_input::Input) {
        within(
            "the session to take the message",
            WAIT,
            self.input.send(v1::TerminalInput { input: Some(input) }),
        )
        .await
        .expect("the session takes the message");
    }

    /// Liest, bis die Ausgabe `needle` enthält; `false`, wenn die Frist um ist
    /// oder der Strom endet.
    ///
    /// **Eine Frist, und sie gilt dem ganzen Vorgang, nicht dem einzelnen
    /// Stueck.** Vorher stand sie je Stueck: Ein Agent, der ununterbrochen
    /// etwas anderes ausgibt als `needle`, hielt die Schleife damit endlos am
    /// Leben, weil immer wieder rechtzeitig etwas ankam, und `seen` wuchs
    /// unbegrenzt. `timeout_at` auf einen festen Zeitpunkt schneidet den
    /// ganzen Aufruf ab, gleich wie oft etwas eintrifft.
    ///
    /// Das ist strenger als vorher, und das ist beabsichtigt. Die Suite
    /// braucht unter zwei Sekunden, `WAIT` sind zwanzig.
    async fn wait_for(&mut self, seen: &mut String, needle: &str) -> bool {
        let until = tokio::time::Instant::now() + WAIT;
        while !seen.contains(needle) {
            if tokio::time::Instant::now() >= until {
                return false;
            }
            let Ok(Some(output)) = tokio::time::timeout_at(until, self.output.next()).await else {
                return false;
            };
            self.take(output, seen);
        }
        true
    }

    /// Liest, bis eine Hinweiszeile `needle` enthält.
    ///
    /// Bytes, die dabei vorbeikommen, landen in `seen`: Ein Test, der auf
    /// einen Hinweis wartet, soll die Ausgabe des Agenten nicht verschlucken.
    async fn wait_for_notice(&mut self, seen: &mut String, needle: &str) -> bool {
        let until = tokio::time::Instant::now() + WAIT;
        while !self.notices.iter().any(|line| line.contains(needle)) {
            if tokio::time::Instant::now() >= until {
                return false;
            }
            let Ok(Some(output)) = tokio::time::timeout_at(until, self.output.next()).await else {
                return false;
            };
            self.take(output, seen);
        }
        true
    }

    /// Schreibt eine Nachricht dorthin, wo sie hingehört.
    fn take(&mut self, output: v1::TerminalOutput, seen: &mut String) {
        match output.output {
            Some(v1::terminal_output::Output::Data(data)) => {
                seen.push_str(&String::from_utf8_lossy(&data));
            }
            Some(v1::terminal_output::Output::Notice(line)) => self.notices.push(line),
            _ => {}
        }
    }

    /// Wartet, bis der Dienst die Größe `cols` mal `rows` bestätigt.
    ///
    /// Die Bestätigung kommt aus `TerminalHub::resize` und erst, nachdem
    /// `SandboxHandle::resize` sie am Pseudoterminal gesetzt hat
    /// (`terminal.rs`: `handle.resize(..)`, dann `frames.send(Frame::Resize)`).
    /// Ohne dieses Warten läge zwischen einem `Resize` und der nächsten Zeile
    /// an den Agenten ein Wettlauf: Der Wunsch geht über einen `watch`-Kanal an
    /// eine eigene Aufgabe, die Zeile über `spawn_blocking` an dasselbe
    /// Pseudoterminal, und welche von beiden zuerst drankommt, entscheidet der
    /// Scheduler. Bytes, die dabei vorbeikommen, landen in `seen`, damit dieses
    /// Warten nichts verschluckt.
    async fn wait_for_size(&mut self, seen: &mut String, cols: u32, rows: u32) -> bool {
        let until = tokio::time::Instant::now() + WAIT;
        loop {
            if tokio::time::Instant::now() >= until {
                return false;
            }
            let Ok(Some(output)) = tokio::time::timeout_at(until, self.output.next()).await else {
                return false;
            };
            match output.output {
                Some(v1::terminal_output::Output::Resize(size))
                    if size.cols == cols && size.rows == rows =>
                {
                    return true;
                }
                other => self.take(v1::TerminalOutput { output: other }, seen),
            }
        }
    }

    /// Die nächste Nachricht, oder `None` nach `WAIT`.
    async fn next(&mut self) -> Option<v1::TerminalOutput> {
        self.next_at(tokio::time::Instant::now() + WAIT).await
    }

    /// Dasselbe gegen einen festen Zeitpunkt.
    ///
    /// Für Schleifen: Eine Frist je Durchlauf summiert sich, und die Meldung
    /// am Ende nennt dann eine Zeit, die nicht die gewartete ist -- derselbe
    /// Grund, aus dem `wait_for` `timeout_at` nimmt.
    async fn next_at(&mut self, until: tokio::time::Instant) -> Option<v1::TerminalOutput> {
        tokio::time::timeout_at(until, self.output.next())
            .await
            .ok()?
    }
}

/// Die Frist ist echt, und sie sagt, worauf sie gewartet hat.
///
/// Ohne diesen Test wäre [`within`] eine Zusage ohne Beleg: Ein `timeout`, das
/// nie zuschlägt, sieht genauso aus wie gar keines. Deshalb hier ein Warten,
/// das von sich aus nie endet.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_wait_without_an_answer_ends_and_names_itself() {
    let outcome = tokio::spawn(async {
        within(
            "the thing that never comes",
            Duration::from_secs(30),
            std::future::pending::<()>(),
        )
        .await;
    })
    .await;

    let panic = outcome.expect_err("the wait ends the test").into_panic();
    let text = panic
        .downcast_ref::<String>()
        .map_or_else(String::new, Clone::clone);
    assert!(
        text.contains("the thing that never comes"),
        "the message names what was waited for: {text:?}"
    );
    assert!(text.contains("30s"), "and how long: {text:?}");
}

/// Und die Frist wirkt auch dort, wo `running_with` sie benutzt: an einem
/// Strom, der nie etwas liefert.
///
/// Der Strom hier ist `tokio_stream::pending`, also genau der Fall, den die CI
/// dreimal gezeigt hat -- ein Warten, auf das nichts antwortet.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_stream_that_never_answers_ends_the_start() {
    let outcome = tokio::spawn(async {
        let mut stream = tokio_stream::pending::<v1::SandboxEvent>();
        until_running(&mut stream).await;
    })
    .await;

    let panic = outcome.expect_err("the start ends the test").into_panic();
    let text = panic
        .downcast_ref::<String>()
        .map_or_else(String::new, Clone::clone);
    assert!(
        text.contains("the sandbox to report Running"),
        "the message names what was waited for: {text:?}"
    );
    assert!(
        text.contains("0 events so far") && text.contains("none yet"),
        "and what had been seen until then: {text:?}"
    );
}

/// Startet die Sitzung und gibt ihr Terminal zurück.
async fn running(service: &SandboxService, fixture: &Fixture) -> Option<TerminalHub> {
    running_with(service, fixture.start()).await
}

/// Die Frist, innerhalb der eine Sandbox hier `Running` melden muss.
///
/// Grosszuegig, und das ist Absicht: Ein Start kostet auf dieser Maschine
/// wenige Sekunden, unter der Enge eines CI-Laeufers mehr, und eine Frist, die
/// unter Last zuschlaegt, waere ein zweiter flatternder Test statt einer
/// Diagnose. Sechzig Sekunden, die etwas sagen, sind besser als kein Limit,
/// das nichts sagt.
const START_WAIT: Duration = Duration::from_secs(60);

/// Ein Warten mit Frist. Läuft sie ab, endet der Test und sagt, worauf er
/// gewartet hat.
///
/// `label` wird gebaut, **bevor** gewartet wird, und darf deshalb den Zustand
/// von diesem Moment tragen — wie viele Ereignisse schon da waren, welcher
/// Zustand zuletzt kam. Genau das will der Mensch wissen, der später den
/// roten Lauf liest.
async fn within<T>(
    label: &str,
    deadline: Duration,
    work: impl std::future::Future<Output = T>,
) -> T {
    within_at(
        &format!("{label} for {deadline:?}"),
        tokio::time::Instant::now() + deadline,
        work,
    )
    .await
}

/// Dasselbe mit einem festen Zeitpunkt statt einer Dauer.
///
/// Für ein Warten in einer Schleife ist das der Unterschied zwischen einer
/// Frist für den ganzen Vorgang und einer je Durchlauf. `until_running` wartet
/// auf `Running` und sieht davor sechs bis sieben andere Ereignisse; mit einer
/// Dauer je Durchlauf wäre die wirkliche Schranke deren Vielfaches, und die
/// Meldung „ein Start, der länger als 60 s braucht, hängt" wäre falsch.
async fn within_at<T>(
    label: &str,
    deadline: tokio::time::Instant,
    work: impl std::future::Future<Output = T>,
) -> T {
    match tokio::time::timeout_at(deadline, work).await {
        Ok(value) => value,
        Err(elapsed) => panic!("waited for {label} without an answer ({elapsed})"),
    }
}

/// Die Frist des Starts ist endlich, und der Test dazu nennt die Zahl nicht,
/// sondern eine Schranke.
///
/// Die Mutationsprobe, die `backlog/sprint-5.md` unter HUM-124 verlangt --
/// „die Frist auf `Duration::MAX` setzen" --, fällt genau hier auf. Sie ist
/// die einzige Stelle, an der die Zahl steht: [`until_running`] nimmt sie
/// nicht als Argument, sonst gäbe es eine zweite, und eine Mutation dort ginge
/// an diesem Test vorbei.
#[test]
fn the_start_deadline_is_a_deadline() {
    assert!(
        START_WAIT <= Duration::from_secs(300),
        "a start that takes longer than {START_WAIT:?} is a hang, not a slow machine"
    );
}

/// Wie ein Start ausging.
#[derive(Debug, PartialEq, Eq)]
enum Start {
    /// Die Sandbox meldete `Running`.
    Running,
    /// Sie meldete `Failed`: eine Umgebung ohne Sandbox, kein Fehler des Codes.
    Skipped,
    /// Der Strom endete, ohne dass eines von beidem kam.
    Ended,
}

/// Liest `stream`, bis eine Sandbox `Running` meldet.
///
/// **Eigene Funktion, damit die Frist prüfbar ist.** Steht die Schleife in
/// [`running_with`], braucht ein Test dafür einen Dienst, dessen Sandbox nie
/// startet, und den gibt es nicht. Über einen Strom lässt sie sich mit jedem
/// Strom prüfen, auch mit einem, der nie etwas liefert.
///
/// **Die Frist steht nur in [`START_WAIT`]**, und sie wird einmal vor der
/// Schleife in einen Zeitpunkt umgerechnet. Als Argument gäbe es zwei Stellen,
/// an denen sie stehen könnte; in der Schleife gälte sie je Ereignis, und ein
/// Start meldet vor `Running` sechs bis sieben andere — die wirkliche Schranke
/// wäre dann deren Vielfaches, und die Meldung „länger als 60 s ist ein
/// Hänger" wäre falsch.
///
/// Drei Ausgänge und nicht zwei: Ein Strom, der ohne `Running` und ohne
/// `Failed` endet, ist etwas anderes als ein übersprungener Test. Er entsteht
/// auf den Fehlerpfaden von `Inner::start`, die mit einem Befund enden, und er
/// gehört benannt, statt in ein `expect` weiter unten geleitet zu werden.
async fn until_running<S>(stream: &mut S) -> Start
where
    S: tokio_stream::Stream<Item = v1::SandboxEvent> + Unpin,
{
    let until = tokio::time::Instant::now() + START_WAIT;
    let mut seen = 0_usize;
    let mut last_state = String::from("none yet");
    loop {
        let label = format!(
            "the sandbox to report Running within {START_WAIT:?} ({seen} events so far, \
             last state {last_state})"
        );
        let Some(event) = within_at(&label, until, stream.next()).await else {
            return Start::Ended;
        };
        seen += 1;
        if let Some(v1::sandbox_event::Event::Status(status)) = &event.event {
            last_state = status.state.to_string();
            if status.state == v1::SandboxState::Running as i32 {
                return Start::Running;
            }
            if status.state == v1::SandboxState::Failed as i32 {
                eprintln!("{SKIP_MARKER} the sandbox did not start here");
                return Start::Skipped;
            }
        }
    }
}

/// Dasselbe mit einer selbst gebauten Anfrage.
///
/// **Jedes Warten hier hat eine Frist.** Ohne sie wartet dieser Schritt
/// unbegrenzt auf ein `Status`, das nie kommt, und aus einem roten Test wird
/// ein haengender. Am 2026-09-05 und am 2026-09-06 stand der CI-Schritt
/// `rust-test` deshalb 2 144, 13 784 und 1 628 Sekunden, gegen 190 Sekunden im
/// gruenen Fall -- und weil `ci.yml` `cancel-in-progress` setzt, endete so ein
/// Lauf als `cancelled` und nicht als `failure`. Ein haengender Test sagt
/// niemandem etwas; ein roter, der nennt, worauf er gewartet hat, ist die
/// Diagnose (HUM-124).
async fn running_with(
    service: &SandboxService,
    request: v1::SandboxRequest,
) -> Option<TerminalHub> {
    let mut stream = service.stream(request);
    match until_running(&mut stream).await {
        Start::Running => {}
        Start::Skipped => return None,
        Start::Ended => {
            panic!("the start stream ended before any Status said Running or Failed")
        }
    }
    // Der Strom des Starts bleibt offen; er trägt die Ausgabe des Agenten in
    // den Ereignisstrom und speist dabei das Terminal.
    // Ohne Frist, und das ist hier richtig: Diese Aufgabe wartet auf nichts,
    // was ein Test braucht. Sie leert den Strom, damit die Ausgabe des Agenten
    // ins Terminal fliesst, und endet mit ihm. Bleibt der Strom offen, lebt sie
    // bis zum Ende des Testprozesses und haelt keinen Test auf.
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
    // **Der Stopp wird gemessen, nicht abgeschnitten.**
    //
    // Eine Frist, die hier abbräche, erzeugte genau den Hänger, den dieses
    // Issue behebt: Abbrechen heißt den Empfänger fallen lassen, und ein
    // fallen gelassener Empfänger lässt `Inner::stop` vor `handle.terminate`
    // zurückkehren (`ipc/src/sandbox.rs`). Die Sandbox überlebte dann, und die
    // Testbinärdatei bliebe beim Beenden im `wait` stehen.
    //
    // `tokio::spawn` um das Leeren herum hilft nicht, und das ist gemessen:
    // Läuft die Frist ab, kehrt diese Funktion zurück, der Test endet, und
    // `#[tokio::test]` fährt die Laufzeit herunter — die abgesetzte Aufgabe
    // wird mitten im Lauf verworfen, der Empfänger stirbt doch. Am 2026-09-06
    // mit einer Frist von einer Millisekunde geprüft: drei Tests standen
    // danach über sechzig Sekunden.
    //
    // Also bis zum Ende lesen und die Dauer hinterher beurteilen. Ein Stopp,
    // der wirklich nie zurückkommt, hängt weiter — das wäre ein Fehler des
    // Daemons und nicht des Gerüsts, und er hat sein eigenes Issue (HUM-128).
    let started = tokio::time::Instant::now();
    let count = drain(service.stream(stop())).await;
    let took = started.elapsed();
    // Die Aussage des Tests zuerst: Ein `panic!` im Rumpf ist der
    // interessantere Befund.
    if let Err(error) = outcome {
        std::panic::resume_unwind(error.into_panic());
    }
    assert!(count > 0, "the stop answered nothing at all");
    assert!(
        took < WAIT,
        "the stop took {took:?}, more than the {WAIT:?} it is allowed"
    );
}

/// Liest einen Strom bis zum Ende und zählt seine Ereignisse.
///
/// **Bis zum Ende, nicht bis zum ersten Ereignis, und das ist der ganze
/// Punkt.** `Sandbox(Stop)` meldet zuerst `Stopping` und tötet die Sandbox
/// **danach** (`ipc/src/sandbox.rs`, `Inner::stop`: erst
/// `status_event(Stopping)`, dann `handle.terminate(KILL_GRACE)`). Wer den
/// Strom nach dem ersten Ereignis fallen lässt, lässt `tx.send` scheitern, und
/// `stop` kehrt an dieser Stelle zurück — **vor** dem Töten.
///
/// Am 2026-09-06 hat genau das drei Testprozesse auf dieser Maschine stehen
/// lassen: `osc52_does_not_reach_host` und `osc8_and_title_are_inert` starten
/// Sandboxen, deren Skript mit `while :; do sleep 0.05; done` endet, also von
/// selbst nie aufhört. Ihre `bwrap`-Prozesse liefen 45 Minuten nach dem
/// letzten `test result: ok` weiter, und die Testbinärdatei stand im
/// `waitpid` darauf.
///
/// Das ist die Signatur, die die CI viermal gezeigt hat: jeder Test grün, und
/// danach ein Prozess, der nicht endet. Fristen um die Wartepunkte des
/// Gerüsts fangen das nicht — es ist kein Warten in `async`, sondern ein
/// blockierendes `wait` auf ein Kind, das niemand umgebracht hat.
async fn drain(mut stream: impl tokio_stream::Stream<Item = v1::SandboxEvent> + Unpin) -> usize {
    let mut seen = 0_usize;
    while stream.next().await.is_some() {
        seen += 1;
    }
    seen
}

/// `ui.terminal_notices = false` schweigt im Bytestrom (HUM-042).
///
/// Die Zeile im Strom ist eine Bequemlichkeit für den, der am Terminal sitzt;
/// der Streifen über dem Terminal bleibt davon unberührt, weil er aus dem
/// Ereignisstrom kommt. Ohne diese Messung wäre der Schalter ein Schlüssel im
/// Schema, dessen Wirkung niemand geprüft hat.
///
/// Die Abwesenheit hängt an einer Anwesenheit: Nach dem Hinweis geht eine
/// Zeile an den Agenten, und erst wenn dessen Echo da ist, wird das Fehlen des
/// Hinweises behauptet (`backlog/CONVENTIONS.md` 4.22).
#[tokio::test(flavor = "multi_thread")]
async fn the_notice_switch_silences_the_stream() {
    let fixture = Fixture::new();
    if !usable(&fixture) {
        return;
    }
    fixture.write_config("[ui]\nterminal_notices = false\n");
    let service = fixture.service();
    let Some(hub) = running_with(&service, fixture.start()).await else {
        return;
    };

    with_session(service, async move {
        let mut client = Client::attach(&hub, 100, 30, false);
        let mut seen = String::new();
        assert!(
            client.wait_for(&mut seen, "READY").await,
            "the agent starts: {seen:?}"
        );
        assert!(!hub.notices(), "the switch is off for this session");

        // Erst an eine Grenze, dann der Hinweis. Steht der Filter mitten in
        // einer Folge des Agenten, legt `TerminalHub::notice` die Zeile nach
        // `pending`, und sie ginge erst mit dem nächsten `feed` hinaus — also
        // hinter dem Echo, auf das dieser Test wartet. Die Abwesenheit unten
        // wäre dann auch mit eingeschaltetem Schalter wahr.
        let deadline = tokio::time::Instant::now() + WAIT;
        while !hub.at_boundary() && tokio::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(
            hub.at_boundary(),
            "the agent stands at a boundary, so a notice would go out at once"
        );

        hub.notice("[humanitl] request held: GET example.com/ · waiting for you");
        client
            .send(v1::terminal_input::Input::Data(b"ping\n".to_vec()))
            .await;
        assert!(
            client.wait_for(&mut seen, "GOT ping").await,
            "the stream still carries the agent: {seen:?}"
        );
        assert!(
            !seen.contains("waiting for you"),
            "and nothing of the daemon in the bytes: {seen:?}"
        );
        assert!(
            client.notices.is_empty(),
            "and no notice of the daemon at all: {:?}",
            client.notices
        );
    })
    .await;
}

/// Der Hinweis steht neben den Bytes des Agenten, nie darin (HUM-042).
///
/// **Warum das eine eigene Messung ist.** Bis zum 2026-09-07 schrieb der
/// Daemon die Zeile in denselben Bytestrom, den der Agent malt. Am Terminal
/// eines Menschen ist das eine Bequemlichkeit; in der Oberfläche, die den
/// Agenten in einem Emulator zeigt und den Hinweis ohnehin als Streifen über
/// dem Terminal führt, stand die Zeile mitten im Bild eines Vollbild-TUI --
/// gemeldet von einem Menschen vor dem Bildschirm, nicht von einem Test. Seit
/// dem Umbau ist die Zeile ein eigener Rahmen: Wer eine Anzeige dafür hat,
/// benutzt sie, wer am Terminal sitzt, schreibt sie selbst dazwischen.
///
/// Die Abwesenheit in den Bytes hängt an einer Anwesenheit: Erst muss der
/// Hinweis als Rahmen da sein, dann erst wird behauptet, dass er in den Bytes
/// fehlt (`backlog/CONVENTIONS.md` 4.22).
#[tokio::test(flavor = "multi_thread")]
async fn a_notice_stands_beside_the_bytes_and_never_inside_them() {
    let Some((_fixture, service, mut client, seen)) = attacking_session(
        "printf 'READY\\r\\n'; while IFS= read -r line; do printf 'GOT %s\\r\\n' \"$line\"; done",
    )
    .await
    else {
        return;
    };
    let hub = service.terminal("").expect("the session runs");
    with_session(service, async move {
        let mut seen = seen;
        assert!(
            client.wait_for(&mut seen, "READY").await,
            "the agent starts: {seen:?}"
        );
        assert!(hub.notices(), "this session writes notices");

        // Erst an eine Grenze: Steht der Filter mitten in einer Folge des
        // Agenten, wartet der Hinweis auf den nächsten `feed`, und dieser Test
        // spräche dann über das Warten statt über den Weg.
        let deadline = tokio::time::Instant::now() + WAIT;
        while !hub.at_boundary() && tokio::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(
            hub.at_boundary(),
            "the agent stands at a boundary, so the notice goes out at once"
        );

        hub.notice("[humanitl] request held: GET example.com/ · waiting for you");
        assert!(
            client.wait_for_notice(&mut seen, "waiting for you").await,
            "the notice arrives as its own frame: {:?}",
            client.notices
        );
        assert_eq!(
            client.notices,
            vec!["[humanitl] request held: GET example.com/ · waiting for you".to_owned()],
            "sanitised, once, and without framing of its own"
        );

        // Und die Bytes des Agenten laufen weiter, ohne die Zeile getragen zu
        // haben. Das Echo danach ist der Beleg, dass der Strom überhaupt noch
        // etwas liefert -- sonst wäre die Abwesenheit unten die Abwesenheit
        // von allem.
        client
            .send(v1::terminal_input::Input::Data(b"ping\n".to_vec()))
            .await;
        assert!(
            client.wait_for(&mut seen, "GOT ping").await,
            "the stream still carries the agent: {seen:?}"
        );
        assert!(
            !seen.contains("waiting for you"),
            "and no byte of the notice was in it: {seen:?}"
        );
    })
    .await;
}

/// Was der Mensch am Fenster tut, sieht der Agent (HUM-042).
///
/// Der Agent meldet auf jede Zeile `stty size`, also das, was **sein** Terminal
/// über sich weiß. Zuerst mit der Größe, mit der der Anschluss geöffnet wurde,
/// dann nach einer Veränderung mit der neuen. Ohne diese Messung wäre „ein
/// Resize erreicht den Agenten" eine Behauptung über eine Nachricht, die
/// irgendwo hätte enden können.
#[tokio::test(flavor = "multi_thread")]
async fn a_resize_reaches_the_agent() {
    let fixture = Fixture::new();
    if !usable(&fixture) {
        return;
    }
    let service = fixture.service();
    let Some(hub) = running_with(&service, fixture.start_with(AGENT_SIZE)).await else {
        return;
    };

    with_session(service, async move {
        let mut writer = Client::attach(&hub, 100, 30, false);
        let mut seen = String::new();
        assert!(
            writer.wait_for(&mut seen, "READY").await,
            "the agent starts: {seen:?}"
        );

        // Die Größe, mit der geöffnet wurde.
        writer
            .send(v1::terminal_input::Input::Data(b"?\n".to_vec()))
            .await;
        assert!(
            writer.wait_for(&mut seen, "SIZE 30 100").await,
            "the agent's terminal has the size the attach asked for: {seen:?}"
        );

        // Und die Größe danach. 132 mal 43 ist keine Vorgabe von irgendwo,
        // sondern eine Zahl, die im Text davor nicht vorkommt.
        writer
            .send(v1::terminal_input::Input::Resize(
                v1::terminal_input::Resize {
                    cols: 132,
                    rows: 43,
                },
            ))
            .await;
        assert!(
            writer.wait_for_size(&mut seen, 132, 43).await,
            "the service confirms the new size before the next line goes out: {seen:?}"
        );
        writer
            .send(v1::terminal_input::Input::Data(b"?\n".to_vec()))
            .await;
        assert!(
            writer.wait_for(&mut seen, "SIZE 43 132").await,
            "the resize reached the agent's terminal: {seen:?}"
        );
    })
    .await;
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
    // Der Platz gehört nicht diesem `Client`, sondern der Sitzung im Dienst:
    // `WriterSlot::drop` gibt ihn frei, während `session` das `Close`
    // abräumt, und weil Rust die eigenen Werte vor den Parametern fallen
    // lässt, ist er frei, bevor der Strom oben endet. Das Fallenlassen hier
    // schließt nur den eigenen Kanal.
    //
    // Gewartet wird trotzdem, und der Grund steht eine Zeile höher: Die
    // Zusicherung dort ist schwächer, als sie klingt, weil `Client::next`
    // auch für die eigene Frist `None` liefert. Ist das `Close` nur langsam
    // statt erledigt, hält die Sitzung den Platz noch -- und dann ist
    // Wiederholen richtig, gegen dieselbe Frist wie jedes andere Warten hier.
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
    let until = tokio::time::Instant::now() + WAIT;
    let mut tries = 0_usize;
    while tokio::time::Instant::now() < until {
        tries += 1;
        let mut candidate = Client::attach(hub, 100, 30, false);
        match candidate.next_at(until).await {
            Some(v1::TerminalOutput {
                output: Some(v1::terminal_output::Output::Diagnostic(_)),
            }) => tokio::time::sleep(Duration::from_millis(20)).await,
            Some(_) => return candidate,
            // Nur die Frist führt hierher: Die Sitzung schickt vor jedem Ende
            // etwas -- die Geometrie an einen angenommenen Anschluss, den
            // Befund an einen abgelehnten --, und der Empfänger lebt, solange
            // `candidate` lebt. Ohne `next_at` liefe der letzte Durchlauf
            // seine eigene Frist noch aus, und die Meldung unten nennte eine
            // Zeit, die nicht die gewartete ist.
            None => break,
        }
    }
    panic!("the writer slot never became free within {WAIT:?} ({tries} tries)");
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
        // Ohne Frist, aus dem Grund, der bei `with_session` steht: Ein
        // Abbruch liesse den Empfaenger fallen und die Sandbox stehen.
        drain(service.stream(stop())).await;
        // Die Sitzung ist weg, sobald `stop` zurück ist: `clear_running` läuft
        // dort vor der letzten Statusmeldung, und `drain` liest bis zum Ende
        // des Stroms.
        assert!(
            service.terminal("").is_err(),
            "round {round}: the session is gone before anything is counted"
        );
        // **Was danach noch läuft, sagt niemand an, und deshalb steht hier
        // eine Zahl statt einer Bedingung.** Gezählt werden Deskriptoren, die
        // Aufräum-Aufgaben halten -- Klone des Hubs, des Handles, die
        // Herrscherseite des Pseudoterminals --, und der Dienst hat keine
        // Aussage darüber, wann die durch sind. Eine halbe Sekunde ist auf
        // diesem Rechner reichlich; auf einem Läufer, der sich zwei Kerne mit
        // allem anderen teilt, ist sie eine Wette. Sie steht hier trotzdem,
        // weil die naheliegende Bedingung -- „keine Sitzung mehr" -- schon
        // wahr ist, bevor die Aufgaben laufen, und eine Bedingung, die nie
        // wartet, wäre die Zusage, die dieser Kommentar nicht macht.
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
    // Und er wartet wirklich: Was in den nächsten 300 ms kommt, wird
    // eingesammelt, und ein Hinweis ist nicht darunter. Der Agent schreibt
    // nichts mehr, also wäre alles, was jetzt käme, der Hinweis; eine kurze
    // Frist reicht, weil ein Rahmen, der hinausgeht, sofort hinausgeht.
    let until = tokio::time::Instant::now() + Duration::from_millis(300);
    while let Some(output) = client.next_at(until).await {
        client.take(output, &mut seen);
    }
    assert!(
        client.notices.is_empty(),
        "the notice waits for the boundary: {:?}",
        client.notices
    );

    // Erst das Ende der Sitzung, dann das Warten: Der Hinweis geht in
    // `finish` hinaus, und `finish` kommt mit dem Ende des Agenten.
    // Ohne Frist, aus dem Grund, der bei `with_session` steht: Ein Abbruch
    // liesse den Empfaenger fallen und die Sandbox stehen.
    drain(service.stream(stop())).await;
    assert!(
        client.wait_for_notice(&mut seen, "waiting for you").await,
        "the end of the session releases it: {:?}",
        client.notices
    );
    assert!(
        !seen.contains('\u{1b}'),
        "and the half-written sequence of the agent never left: {seen:?}"
    );
    assert!(
        !seen.contains("waiting for you"),
        "and it never went into the bytes of the agent: {seen:?}"
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
