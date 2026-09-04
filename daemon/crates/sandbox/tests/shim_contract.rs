//! Der Vertrag zwischen Launcher und Shim, am gebauten Binary geprüft.
//!
//! Der Shim darf laut `backlog/CONVENTIONS.md` 3.1 nur `libc` und
//! `seccompiler` kennen. Er kann die Konstanten des Launchers deshalb nicht
//! importieren, und der Vertrag steht zweimal da: die fünf Umgebungsnamen, die
//! drei Exit-Codes, die fünf Check-Namen, die Grammatik der `CHECK`-Zeilen, das
//! Bridge-JSON, der Socket-Pfad, die erlaubten Familien und Typen und der Boden
//! der verbotenen Syscalls. Die Doppelung bleibt; was fehlte, ist der Test über
//! die Crate-Grenze.
//!
//! Diese Datei teilt deshalb keinen Code, sondern ruft das **gebaute
//! Shim-Binary** auf und vergleicht, was es tut, mit den Konstanten des
//! Launchers. Bewegt sich eine der beiden Fassungen, bricht sie.
//!
//! Der wichtigste Fall ist der Syscall-Boden. `DEFAULT_DENY_SYSCALLS` in
//! `daemon/crates/sandbox/src/profile.rs` und `FLOOR` in
//! `daemon/bin/humanitl-shim/src/seccomp.rs` sind zwei von Hand getippte
//! Listen derselben siebzehn Namen, und vor diesem Test verglich sie nichts:
//! Jede Zusicherung auf der Launcher-Seite war auf `DEFAULT_DENY_SYSCALLS`
//! selbst bezogen, jede auf der Shim-Seite auf `FLOOR` selbst. Ein Name, der
//! aus einer der beiden Listen verschwindet, hätte alle bestehenden Tests grün
//! gelassen — der Boden kann nach unten driften, ohne dass es auffällt. Das ist
//! die zweite Stelle, an der das möglich ist: die Laufzeitprobe des Shims
//! (`CHECK families`) beweist genau einen der siebzehn Aufrufe
//! (`io_uring_setup`), und der Launcher macht daraus `SANDBOX_016`. Der volle
//! Laufzeitbeweis steht in `tests/escape/esc-1-sockets.sh` und braucht `bwrap`;
//! hier wird stattdessen die Regeltabelle des Binaries verglichen, die derselbe
//! `policy_from_env` erzeugt, aus dem auch das installierte BPF-Programm
//! entsteht.
//!
//! Die Tests laufen auf dem Host, nicht in der Sandbox. `no_interfaces` und
//! `single_socket` melden hier deshalb `fail`; geprüft werden die Namen und die
//! Grammatik, nie das Ergebnis einer Prüfung.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeSet;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use humanitl_sandbox::{
    Bridge, CHECK_BRIDGE_LISTENING, CHECK_FAMILIES, CHECK_NAMES, CHECK_PREFIX,
    DEFAULT_DENY_SYSCALLS, ENV_BRIDGES, ENV_REPORT_FD, ENV_SECCOMP_DENY, EXIT_USAGE, PROXY_BRIDGE,
    PROXY_SOCKET_DST, REQUIRED_SOCKET_FAMILIES, REQUIRED_SOCKET_TYPES, RESERVED_ENV,
    SandboxProfile, ShimCheck, bridges_json, parse_check_line, shim_env,
};

/// Der Deskriptor, über den der Shim in diesen Tests seinen Bericht schreibt.
///
/// Eine einstellige Nummer, damit die Umleitung in jeder POSIX-Shell steht
/// (`exec 3>datei`); der Shim verlangt nur, dass sie mindestens `3` ist.
const REPORT_FD: &str = "3";

/// Das gebaute Shim-Binary neben dem Testbinary.
///
/// Fehlt es, ist das ein Fehler und kein Grund, den Test zu überspringen: ein
/// Vertragstest, der sich selbst abschaltet, prüft den Vertrag nie. `make
/// check` und `cargo test --workspace` bauen das Binary vorher, `cargo test -p
/// humanitl-sandbox` allein nicht.
fn shim_binary() -> PathBuf {
    let mut dir = std::env::current_exe().unwrap_or_else(|err| panic!("no test binary: {err}"));
    dir.pop();
    if dir.ends_with("deps") {
        dir.pop();
    }
    let shim = dir.join("humanitl-shim");
    assert!(
        shim.is_file(),
        "{} is missing; build the workspace first \
         (cargo build --workspace --all-targets, or cargo test --workspace)",
        shim.display()
    );
    shim
}

/// Der Shim ohne eine einzige der fünf Vertragsvariablen.
fn shim() -> Command {
    let mut command = Command::new(shim_binary());
    for name in RESERVED_ENV {
        command.env_remove(name);
    }
    command.stdin(Stdio::null());
    command
}

fn run(mut command: Command) -> Output {
    command
        .output()
        .unwrap_or_else(|err| panic!("the shim must be runnable: {err}"))
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Eine Zeile der Regeltabelle: `subject | condition | verdict | origin`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RuleRow {
    subject: String,
    condition: String,
    verdict: String,
    origin: String,
}

/// Die Regeltabelle, die das Binary für diese Umgebung meldet (`--rules`).
fn rule_table(env: &[(&str, &str)]) -> Vec<RuleRow> {
    let mut command = shim();
    for (name, value) in env {
        command.env(name, value);
    }
    command.arg("--rules");
    let output = run(command);
    assert_eq!(
        output.status.code(),
        Some(0),
        "--rules must describe this environment: {}",
        stderr_of(&output)
    );
    stdout_of(&output)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let mut fields = line.split('|').map(str::trim);
            let mut next = |what: &str| {
                fields
                    .next()
                    .unwrap_or_else(|| panic!("rule line {line:?} has no {what}"))
                    .to_owned()
            };
            RuleRow {
                subject: next("subject"),
                condition: next("condition"),
                verdict: next("verdict"),
                origin: next("origin"),
            }
        })
        .collect()
}

/// Die Syscalls, die die Regeltabelle bedingungslos mit `EPERM` beantwortet.
fn denied_syscalls(rules: &[RuleRow]) -> BTreeSet<String> {
    rules
        .iter()
        .filter(|row| row.condition == "always" && row.verdict == "EPERM")
        .map(|row| row.subject.clone())
        .collect()
}

/// Die Menge in `… not in {A, B}` aus der Bedingung einer Prelude-Zeile.
fn set_in_condition(rules: &[RuleRow], needle: &str) -> BTreeSet<String> {
    let row = rules
        .iter()
        .find(|row| row.subject == "socket" && row.condition.starts_with(needle))
        .unwrap_or_else(|| panic!("no socket rule starting with {needle:?}: {rules:#?}"));
    let inner = row
        .condition
        .split_once('{')
        .and_then(|(_, rest)| rest.split_once('}'))
        .unwrap_or_else(|| panic!("condition {:?} carries no set", row.condition))
        .0;
    inner
        .split(',')
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .collect()
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Ein Pfad direkt unter `/tmp`: ein Unix-Socket-Pfad ist auf gut 100 Zeichen
/// begrenzt, und `TMPDIR` kann tiefer liegen.
fn tmp_path(tag: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    PathBuf::from("/tmp").join(format!(
        "humanitl-shim-contract-{}-{tag}-{n}",
        std::process::id()
    ))
}

/// Ein Unix-Socket, der Verbindungen annimmt, solange der Test läuft.
///
/// Der Shim baut seine Bridge gegen diesen Pfad; ohne ihn scheitert die
/// Einrichtung, und der Bericht bräche ab, bevor alle fünf Zeilen dastehen.
fn accepting_socket(path: &Path) {
    let _ = fs::remove_file(path);
    let listener = UnixListener::bind(path)
        .unwrap_or_else(|err| panic!("cannot bind {}: {err}", path.display()));
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            thread::spawn(move || {
                let mut buf = [0u8; 1024];
                while let Ok(n) = stream.read(&mut buf) {
                    if n == 0 || stream.write_all(&buf[..n]).is_err() {
                        break;
                    }
                }
            });
        }
    });
}

/// Ein vollständiger Lauf mit Bericht; der rohe Text des Berichts.
///
/// Der Bericht geht über einen geerbten Deskriptor in eine Datei. Die
/// Umleitung macht eine Shell (`exec 3>datei`), damit dieser Test ohne `unsafe`
/// und ohne `libc` auskommt — die Crate verbietet beides
/// (`#![forbid(unsafe_code)]`).
///
/// Ohne `bridge` fehlt [`ENV_BRIDGES`], und der Shim greift auf seine eigene
/// Vorgabe zurück; genau das prüft
/// `the_shim_falls_back_to_the_launchers_socket_path`.
fn run_with_report(with_bridge: bool) -> String {
    let socket = tmp_path("bridge.sock");
    let report = tmp_path("report.txt");

    let mut command = Command::new("/bin/sh");
    command
        .arg("-c")
        .arg(r#"exec 3>"$1"; shift; exec "$@""#)
        .arg("sh")
        .arg(&report)
        .arg(shim_binary())
        .args(["--proxy-port", "0", "--", "/bin/true"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    for name in RESERVED_ENV {
        command.env_remove(name);
    }
    command.env(ENV_REPORT_FD, REPORT_FD);
    if with_bridge {
        accepting_socket(&socket);
        let bridge = Bridge {
            socket: socket.clone(),
            ..Bridge::proxy_on(0)
        };
        command.env(ENV_BRIDGES, bridges_json(std::slice::from_ref(&bridge)));
    }

    let output = run(command);
    let text = fs::read_to_string(&report).unwrap_or_default();
    let _ = fs::remove_file(&report);
    let _ = fs::remove_file(&socket);
    assert_eq!(
        output.status.code(),
        Some(0),
        "the shim must run /bin/true: {}",
        stderr_of(&output)
    );
    text
}

/// Die `CHECK`-Zeilen eines Laufs, gelesen mit dem Parser des Launchers.
fn report_lines(with_bridge: bool) -> Vec<ShimCheck> {
    BufReader::new(run_with_report(with_bridge).into_bytes().as_slice())
        .lines()
        .map_while(Result::ok)
        .map(|line| {
            parse_check_line(&line).unwrap_or_else(|| {
                panic!("the launcher's parser does not read the shim's line {line:?}")
            })
        })
        .collect()
}

/// Das mitgelieferte Profil, gelesen mit dem Parser des Launchers.
fn default_profile() -> SandboxProfile {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../profiles/sandbox/default.toml");
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("cannot read {}: {err}", path.display()));
    SandboxProfile::parse(&text, &path)
        .unwrap_or_else(|diagnostic| panic!("the shipped profile must parse: {}", diagnostic.why))
}

// ---------------------------------------------------------------------------
// Exit-Codes
// ---------------------------------------------------------------------------

/// Ohne Argumente endet der Shim mit dem Code, den der Launcher erwartet.
///
/// `daemon/crates/sandbox/src/bin/escape-launch.rs` entscheidet an genau
/// diesem Code, ob das Binary am anderen Ende der Kommandozeile der Shim ist.
#[test]
fn the_shim_without_arguments_exits_with_the_usage_code() {
    let output = run(shim());
    assert_eq!(
        output.status.code(),
        Some(EXIT_USAGE),
        "stderr was: {}",
        stderr_of(&output)
    );
    assert!(
        stdout_of(&output).is_empty(),
        "a usage error belongs on stderr, not on stdout"
    );
    assert!(
        stderr_of(&output).contains("usage:"),
        "the usage error says how to call it: {}",
        stderr_of(&output)
    );
}

/// Die Hilfe des Shims nennt jede der fünf Variablen, die der Launcher setzt.
///
/// Ein Name, den nur eine der beiden Seiten kennt, wäre eine Einstellung, die
/// stillschweigend nicht ankommt.
#[test]
fn the_usage_text_names_every_reserved_variable() {
    let mut command = shim();
    command.arg("--help");
    let output = run(command);
    assert_eq!(output.status.code(), Some(0));
    let text = stdout_of(&output);
    for name in RESERVED_ENV {
        assert!(
            text.contains(name),
            "the shim's help does not mention {name}: {text}"
        );
    }
}

// ---------------------------------------------------------------------------
// Der Syscall-Boden
// ---------------------------------------------------------------------------

/// Die Regeltabelle des Shims verbietet genau den Boden des Launchers.
///
/// Beide Richtungen: kein Name des Launchers fehlt im Shim, und der Shim
/// verbietet ohne Profil keinen Namen, den der Launcher nicht kennt. Ohne
/// gesetztes `HUMANITL_SECCOMP_DENY` meldet die Tabelle genau `FLOOR`, also
/// die Liste, die der Shim selbst mitbringt.
#[test]
fn the_rule_table_denies_exactly_the_launcher_floor() {
    let rules = rule_table(&[]);
    let denied = denied_syscalls(&rules);
    let floor: BTreeSet<String> = DEFAULT_DENY_SYSCALLS
        .iter()
        .map(|name| (*name).to_owned())
        .collect();

    let missing: Vec<&String> = floor.difference(&denied).collect();
    assert!(
        missing.is_empty(),
        "the shim does not refuse {missing:?}; DEFAULT_DENY_SYSCALLS and FLOOR have drifted apart"
    );
    let extra: Vec<&String> = denied.difference(&floor).collect();
    assert!(
        extra.is_empty(),
        "the shim refuses {extra:?} without a profile saying so; \
         DEFAULT_DENY_SYSCALLS is missing them"
    );
    assert_eq!(
        denied.len(),
        DEFAULT_DENY_SYSCALLS.len(),
        "the floor has {} names on one side and {} on the other",
        DEFAULT_DENY_SYSCALLS.len(),
        denied.len()
    );
}

/// Ein Profil kann den Boden erweitern, nie senken.
///
/// Der Shim vereinigt `HUMANITL_SECCOMP_DENY` mit seinem eigenen `FLOOR`. Ein
/// Profil, das nur `mount` nennt, verbietet damit achtzehn Aufrufe, nicht
/// einen — und ein Shim, der das Vereinigen verlöre, fiele hier auf.
#[test]
fn a_profile_extends_the_floor_and_never_lowers_it() {
    let rules = rule_table(&[(ENV_SECCOMP_DENY, "mount")]);
    let denied = denied_syscalls(&rules);
    for name in DEFAULT_DENY_SYSCALLS {
        assert!(
            denied.contains(*name),
            "a profile naming only `mount` dropped {name} from the floor"
        );
    }
    assert!(
        denied.contains("mount"),
        "the profile's own entry did not reach the filter"
    );
    assert_eq!(denied.len(), DEFAULT_DENY_SYSCALLS.len() + 1);
}

/// Was der Launcher aus dem mitgelieferten Profil an den Shim reicht, ergibt
/// im Shim dieselbe Politik.
///
/// Hier laufen beide Seiten wirklich zusammen: `shim_env` erzeugt die
/// Umgebung, das Binary liest sie, und die Tabelle muss den Boden und die
/// Socket-Politik des Profils zeigen.
#[test]
fn the_shipped_profile_reaches_the_shim_unchanged() {
    let profile = default_profile();
    let env = shim_env(&profile, None);
    let pairs: Vec<(&str, &str)> = env
        .iter()
        .filter(|(name, _)| name != ENV_BRIDGES)
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    let rules = rule_table(&pairs);

    let denied = denied_syscalls(&rules);
    for name in &profile.seccomp.deny_syscalls {
        assert!(
            denied.contains(name),
            "the profile denies {name}, the shim does not"
        );
    }
    assert_eq!(
        denied.len(),
        profile.seccomp.deny_syscalls.len(),
        "the shim's rule table and the profile's list have different lengths"
    );

    let families = set_in_condition(&rules, "family (arg0) not in");
    let expected: BTreeSet<String> = profile
        .seccomp
        .allow_families
        .iter()
        .map(|family| family.as_str().to_owned())
        .collect();
    assert_eq!(families, expected, "the allowed families differ");
}

// ---------------------------------------------------------------------------
// Familien, Typen und die Maske
// ---------------------------------------------------------------------------

/// Ohne Profil erlaubt der Shim genau die Familien und Typen des Bodens.
///
/// `REQUIRED_SOCKET_FAMILIES` und `REQUIRED_SOCKET_TYPES` sind die dritte
/// Garantie in Listenform; der Shim schreibt sie ein zweites Mal auf.
#[test]
fn the_rule_table_allows_exactly_the_required_families_and_types() {
    let rules = rule_table(&[]);

    let families = set_in_condition(&rules, "family (arg0) not in");
    let expected: BTreeSet<String> = REQUIRED_SOCKET_FAMILIES
        .iter()
        .map(|family| family.as_str().to_owned())
        .collect();
    assert_eq!(families, expected);

    let types = set_in_condition(&rules, "type (arg1 & 0xff) not in");
    let expected: BTreeSet<String> = REQUIRED_SOCKET_TYPES
        .iter()
        .map(|kind| kind.as_str().to_owned())
        .collect();
    assert_eq!(types, expected);

    // Die Maske `0xff` gehört zur Aussage: ohne sie fielen `SOCK_NONBLOCK` und
    // `SOCK_CLOEXEC` durch die Prüfung (`docs/SECURITY.md`, Garantie 3).
    assert!(
        rules
            .iter()
            .any(|row| row.condition.contains("arg1 & 0xff")),
        "the socket type is compared without the 0xff mask: {rules:#?}"
    );
    // Und der x32-Umweg bleibt gesperrt.
    assert!(
        rules
            .iter()
            .any(|row| row.condition.contains("0x40000000") && row.verdict == "EPERM"),
        "the x32 prelude is gone: {rules:#?}"
    );
}

// ---------------------------------------------------------------------------
// Der Bericht
// ---------------------------------------------------------------------------

/// Ein Lauf mit Bericht liefert genau die Check-Namen des Launchers.
///
/// Und jede Zeile liest [`parse_check_line`], der Parser des Launchers — die
/// Grammatik `CHECK <name> <ok|fail> <evidence>` steht damit nicht mehr nur in
/// zwei Kommentaren, sondern in einem Test.
#[test]
fn the_report_carries_exactly_the_launcher_check_names() {
    let checks = report_lines(true);
    let seen: BTreeSet<&str> = checks.iter().map(|check| check.name.as_str()).collect();
    let expected: BTreeSet<&str> = CHECK_NAMES.iter().copied().collect();
    assert_eq!(
        seen, expected,
        "the shim reports other checks than the launcher reads"
    );
    assert_eq!(
        checks.len(),
        CHECK_NAMES.len(),
        "every check appears exactly once: {checks:#?}"
    );
    for check in &checks {
        assert!(
            !check.evidence.is_empty(),
            "{} carries no evidence",
            check.name
        );
    }
}

/// Die Zeilen tragen das Präfix des Launchers und sonst nichts.
#[test]
fn every_report_line_starts_with_the_check_prefix() {
    let text = run_with_report(true);
    assert!(!text.is_empty(), "the report is empty");
    for line in text.lines() {
        assert!(
            line.starts_with(&format!("{CHECK_PREFIX} ")),
            "line {line:?} is not a report line"
        );
        assert!(
            !line.contains('\r'),
            "line {line:?} carries a carriage return"
        );
    }
}

/// Die Laufzeitprobe meldet die Marken, aus denen der Launcher `SANDBOX_016`
/// baut — und nur die.
///
/// Das ist zugleich die Stelle, an der der Boden nach unten driften könnte,
/// ohne dass jemand es sähe: `CHECK families` beweist von den siebzehn
/// verbotenen Aufrufen genau einen (`io_uring_setup`). Fällt die Probe weg oder
/// wechselt ihr Aufruf, bricht dieser Test; dass die anderen sechzehn wirklich
/// verboten sind, prüft `the_rule_table_denies_exactly_the_launcher_floor`
/// gegen die Regeltabelle und `tests/escape/esc-1-sockets.sh` zur Laufzeit.
#[test]
fn the_runtime_probe_reports_the_marks_the_launcher_reads() {
    let checks = report_lines(true);
    let families = checks
        .iter()
        .find(|check| check.name == CHECK_FAMILIES)
        .unwrap_or_else(|| panic!("no {CHECK_FAMILIES} check: {checks:#?}"));

    // Die positive Kontrolle nennt genau die Familie und den Typ, die der
    // Launcher als Boden vorschreibt.
    let allowed = format!(
        "socket({},{})=ok",
        REQUIRED_SOCKET_FAMILIES[0].as_str(),
        REQUIRED_SOCKET_TYPES[0].as_str()
    );
    assert!(
        families.evidence.contains(&allowed),
        "the probe does not confirm {allowed}: {}",
        families.evidence
    );
    for needle in ["x32:socket=EPERM", "io_uring_setup=EPERM"] {
        assert!(
            families.evidence.contains(needle),
            "the probe no longer covers {needle}: {}",
            families.evidence
        );
    }
    assert!(
        families.evidence.split(';').count() >= 5,
        "the probe lost a case: {}",
        families.evidence
    );
}

// ---------------------------------------------------------------------------
// Bridge-JSON und Socket-Pfad
// ---------------------------------------------------------------------------

/// Was der Launcher schreibt, liest der Shim.
///
/// `bridges_json` ist die einzige Stelle, die das JSON erzeugt; der Shim hat
/// dafür einen eigenen, handgeschriebenen Parser. Ein neues Feld auf der einen
/// Seite ist auf der anderen ein `UnknownField` und beendet den Shim mit `126`.
#[test]
fn the_shim_reads_the_launchers_bridge_json() {
    let checks = report_lines(true);
    let bridge = checks
        .iter()
        .find(|check| check.name == CHECK_BRIDGE_LISTENING)
        .unwrap_or_else(|| panic!("no bridge check: {checks:#?}"));
    assert!(
        bridge.ok,
        "the shim did not accept the launcher's bridge: {}",
        bridge.evidence
    );
    assert!(
        bridge.evidence.starts_with(&format!("{PROXY_BRIDGE}=")),
        "the bridge is not the one the launcher named: {}",
        bridge.evidence
    );
}

/// Der Socket-Pfad in der Sandbox steht auf beiden Seiten gleich.
///
/// Der Launcher bindet [`PROXY_SOCKET_DST`] als einzige Tür in die Sandbox; der
/// Shim benutzt denselben Pfad, wenn [`ENV_BRIDGES`] fehlt. Läuft einer der
/// beiden weg, führt die eine Tür ins Leere, ohne dass etwas scheitert.
#[test]
fn the_shim_falls_back_to_the_launchers_socket_path() {
    let checks = report_lines(false);
    let bridge = checks
        .iter()
        .find(|check| check.name == CHECK_BRIDGE_LISTENING)
        .unwrap_or_else(|| panic!("no bridge check: {checks:#?}"));
    assert!(
        bridge.evidence.ends_with(PROXY_SOCKET_DST),
        "without {ENV_BRIDGES} the shim must fall back to {PROXY_SOCKET_DST}: {}",
        bridge.evidence
    );
    assert!(
        bridge.evidence.starts_with(&format!("{PROXY_BRIDGE}=")),
        "the fallback bridge is not named {PROXY_BRIDGE}: {}",
        bridge.evidence
    );
}
