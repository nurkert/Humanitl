//! Die elf Urteile des Doctors, je eines je [`CheckId`].
//!
//! Jede Funktion hier ist rein: Sie bekommt die Tatsachen einer Prüfung und
//! gibt eine Zeile zurück. Kein Dateizugriff, kein Prozess, kein Netz. Das ist
//! der Grund, aus dem sich jede Prüfung testen lässt, ohne den Rechner in den
//! Zustand zu bringen, um den es geht: Ein Test baut die Tatsachen.
//!
//! Drei Regeln gelten für alle:
//!
//! 1. Wo nichts gemessen wurde, steht [`CheckOutcome::unmeasured`], nie `ok`.
//! 2. Der Beleg nennt, **was** gefunden wurde, nicht bloß das Urteil. Ein
//!    fehlender Kernel-Schalter und einer mit unbrauchbarem Wert sind zwei
//!    verschiedene Sätze.
//! 3. Jede nicht-grüne Zeile trägt einen [`FixAction`], auch wenn er nur der
//!    Befehl ist, mit dem ein Mensch dasselbe nachsieht.

use humanitl_core::diagnostics::codes::{
    DOCTOR_001, DOCTOR_002, DOCTOR_003, DOCTOR_004, DOCTOR_005, DOCTOR_006, DOCTOR_007, DOCTOR_008,
    DOCTOR_009, DOCTOR_010, DOCTOR_011,
};
use humanitl_core::{Diagnostic, DiagnosticCode, FixAction, Severity};

use super::{
    AgentFacts, BwrapFacts, CheckId, CheckOutcome, CommandRun, DaemonFacts, DiskFacts, LlmFacts,
    Reading, RendererFacts, RunOutcome, RuntimeDirFacts, SeccompFacts, SeccompLine, SystemdFacts,
    TrayFacts, UsernsFacts, not_contacted,
};
use crate::agent::opencode;
use crate::bwrap::{INSTALL_COMMAND, MIN_BWRAP_VERSION, USERNS_DOCS_URL, USERNS_SYSCTL_COMMAND};

/// Die kleinste Kernel-Fassung, gegen die der seccomp-Filter des Shims
/// geprüft ist (`major`, `minor`).
///
/// Ältere Kernel kennen seccomp längst; 5.4 ist die Fassung, ab der die
/// Filterform des Shims (`SECCOMP_RET_ERRNO` je Architektur samt x32-Vorspann)
/// im Projekt gemessen wurde. Darunter läuft es womöglich, nur weiß es
/// niemand — deshalb `warn` und nicht `fail`.
const MIN_KERNEL: (u32, u32) = (5, 4);

/// Wo die AppIndicator-Erweiterung für GNOME liegt.
const GNOME_EXTENSION_URL: &str =
    "https://extensions.gnome.org/extension/615/appindicator-support/";

/// Der Befehl, der die Tray-Bibliothek auf Debian und Ubuntu nachinstalliert.
const TRAY_INSTALL_COMMAND: &str = "sudo apt install libayatana-appindicator3-1";

/// Wo der Renderer der Oberfläche erklärt ist.
const IMPELLER_DOCS_URL: &str = "https://docs.flutter.dev/perf/impeller";

/// Der Schalter, mit dem die Oberfläche ohne Impeller startet.
const IMPELLER_OFF_FLAG: &str = "--no-enable-impeller";

/// Wo die Manpage zu seccomp steht.
const SECCOMP_DOCS_URL: &str = "https://man7.org/linux/man-pages/man2/seccomp.2.html";

/// Wie viel freier Platz als „genug" gilt, als Text für die Befunde.
const MIN_FREE_TEXT: &str = "1 GiB";

/// Der Modus, den `$XDG_RUNTIME_DIR` genau haben muss.
///
/// Derselbe Wert wie [`humanitl_config::DIR_MODE`], mit dem der Daemon seine
/// eigenen Verzeichnisse anlegt; der Doctor prueft, was der Daemon spaeter
/// voraussetzt.
const DIR_MODE: u32 = humanitl_config::DIR_MODE;

/// Ein Befund mit Code, Stufe, Grund und Vorschlag.
fn finding(
    code: DiagnosticCode,
    severity: Severity,
    why: impl Into<String>,
    fix: FixAction,
) -> Diagnostic {
    Diagnostic::builder(code, severity)
        .why(why.into())
        .fix(fix)
        .build()
}

/// Derselbe Befund mit einer Adresse, unter der mehr dazu steht.
fn finding_with_docs(
    code: DiagnosticCode,
    severity: Severity,
    why: impl Into<String>,
    fix: FixAction,
    docs: &str,
) -> Diagnostic {
    Diagnostic::builder(code, severity)
        .why(why.into())
        .fix(fix)
        .docs(docs.to_owned())
        .build()
}

/// Der Befund eines anderen Bereichs, in die Zeile des Doctors gefasst.
///
/// Der Doctor führt je Zeile genau einen Code, damit sich `DOCTOR_006` und die
/// Zeile `daemon` nicht auseinanderentwickeln können. Was der Daemon oder die
/// Endpunkt-Probe selbst gemeldet hat, geht dabei nicht verloren: Code und
/// Grund stehen im `why`, und der Vorschlag wird übernommen, wenn es einen
/// gab.
fn wrapping(code: DiagnosticCode, inner: &Diagnostic, fallback: FixAction) -> Diagnostic {
    finding(
        code,
        Severity::Warning,
        format!("{}: {}", inner.code, inner.why),
        inner.fix.clone().unwrap_or(fallback),
    )
}

/// Prüfung 1: `bwrap` ist da und mindestens [`MIN_BWRAP_VERSION`].
pub(super) fn bwrap(facts: &BwrapFacts) -> CheckOutcome {
    match facts {
        BwrapFacts::Missing { searched } => CheckOutcome::fail(
            CheckId::Bwrap,
            format!("no executable bwrap in PATH={searched}"),
            finding_with_docs(
                DOCTOR_001,
                Severity::Blocking,
                format!(
                    "no executable bwrap in PATH={searched}; bubblewrap is not installed, \
                     and without it there is no sandbox"
                ),
                FixAction::CopyCommand(INSTALL_COMMAND.to_owned()),
                "https://github.com/containers/bubblewrap",
            ),
        ),
        BwrapFacts::Unreadable { program, error } => CheckOutcome::unmeasured(
            CheckId::Bwrap,
            format!("{} --version did not answer: {error}", program.display()),
            &[&program.to_string_lossy(), "--version"],
        ),
        BwrapFacts::Found { program, version } => {
            let evidence = format!("bubblewrap {version} at {}", program.display());
            if *version >= MIN_BWRAP_VERSION {
                CheckOutcome::ok(CheckId::Bwrap, evidence)
            } else {
                CheckOutcome::fail(
                    CheckId::Bwrap,
                    evidence,
                    finding(
                        DOCTOR_001,
                        Severity::Blocking,
                        format!(
                            "{} is bubblewrap {version}; the launcher needs at least \
                             {MIN_BWRAP_VERSION}",
                            program.display()
                        ),
                        FixAction::CopyCommand(INSTALL_COMMAND.to_owned()),
                    ),
                )
            }
        }
    }
}

/// Prüfung 2: Ein unprivilegierter Nutzer-Namensraum lässt sich aufmachen.
///
/// Gemessen wird mit `bwrap` und nicht mit `unshare -Ur`: Ubuntu bringt seit
/// 23.10 eine `AppArmor`-Beschränkung unprivilegierter Namensräume mit und
/// erzwingt sie ab 24.04; das ausgelieferte `bwrap` trägt dafür ein Profil.
/// Wer `unshare` fragte, bekäme dort ein Nein über einen Weg, den Humanitl gar
/// nicht geht (HUM-075, Fallstricke).
pub(super) fn userns(facts: &UsernsFacts) -> CheckOutcome {
    let Some(run) = facts.probe.found() else {
        let why = facts
            .probe
            .missing_because()
            .unwrap_or_else(|| "was not tried".to_owned());
        return CheckOutcome::unmeasured(
            CheckId::Userns,
            format!("bwrap --unshare-user was not run: bwrap {why}"),
            &[
                "bwrap",
                "--unshare-user",
                "--unshare-pid",
                "--unshare-net",
                "--",
                "/bin/true",
            ],
        );
    };

    if run.outcome.is_success() {
        return CheckOutcome::ok(
            CheckId::Userns,
            format!("{} --unshare-user opened a namespace", run.program()),
        );
    }

    let evidence = format!(
        "{} --unshare-user: {}",
        run.program(),
        run.outcome.describe()
    );
    CheckOutcome::fail(CheckId::Userns, evidence, userns_finding(facts, run))
}

/// Der Befund einer gescheiterten Namensraum-Probe, samt dem, was die beiden
/// Kernel-Schalter dazu sagen.
///
/// Der Grund bleibt kurz, und zwar aus einem harten Grund: Auf dem Weg zur
/// Oberfläche geht jedes `why` durch
/// [`sanitize_note`](humanitl_core::sanitize_note) und ist danach höchstens
/// [`NOTE_MAX_CHARS`](humanitl_core::block::NOTE_MAX_CHARS) Zeichen lang. Ein
/// längerer Satz würde mitten im Wort abgeschnitten — gemessen am
/// 2026-09-05, als die volle `bwrap`-Zeile im Grund stand. Die ganze Zeile
/// steht deshalb im Vorschlag, wo ein Mensch sie kopiert, und nicht im Grund.
fn userns_finding(facts: &UsernsFacts, run: &CommandRun) -> Diagnostic {
    let head = format!(
        "{} --unshare-user ended with {}: {}",
        run.program(),
        run.outcome.describe(),
        run.first_message()
    );
    let switch = "/proc/sys/kernel/apparmor_restrict_unprivileged_userns";

    match facts.apparmor_restrict.found().map(String::as_str) {
        Some("1") => finding_with_docs(
            DOCTOR_002,
            Severity::Blocking,
            format!(
                "{head}. {switch} is 1: this kernel restricts unprivileged user \
                 namespaces by AppArmor. A bubblewrap from the distribution carries a \
                 profile and is allowed even so; one from elsewhere is not"
            ),
            FixAction::CopyCommand(USERNS_SYSCTL_COMMAND.to_owned()),
            USERNS_DOCS_URL,
        ),
        Some(other) => finding_with_docs(
            DOCTOR_002,
            Severity::Blocking,
            format!(
                "{head}. {switch} is {other}, so AppArmor is not the reason; {}",
                userns_clone_note(&facts.userns_clone)
            ),
            userns_clone_fix(&facts.userns_clone, run),
            USERNS_DOCS_URL,
        ),
        None => finding_with_docs(
            DOCTOR_002,
            Severity::Blocking,
            format!(
                "{head}. {switch} {}, so the AppArmor restriction of Ubuntu 23.10 and \
                 later is neither confirmed nor ruled out; {}",
                facts
                    .apparmor_restrict
                    .missing_because()
                    .unwrap_or_else(|| "was not read".to_owned()),
                userns_clone_note(&facts.userns_clone)
            ),
            userns_clone_fix(&facts.userns_clone, run),
            USERNS_DOCS_URL,
        ),
    }
}

/// Was `/proc/sys/kernel/unprivileged_userns_clone` beisteuert.
fn userns_clone_note(reading: &Reading<String>) -> String {
    let switch = "/proc/sys/kernel/unprivileged_userns_clone";
    match reading.found().map(String::as_str) {
        Some("0") => format!("{switch} is 0, which turns them off for everyone"),
        Some(other) => format!("{switch} is {other}, so that switch is not the reason either"),
        None => format!(
            "{switch} {}, which is normal without the Debian patch",
            reading
                .missing_because()
                .unwrap_or_else(|| "was not read".to_owned())
        ),
    }
}

/// Der Vorschlag, wenn `AppArmor` nicht der Grund ist.
///
/// Ohne den Kernel-Schalter bleibt der Aufruf selbst: die vollständige Zeile,
/// mit der der Doctor gemessen hat, zum Nachfahren von Hand.
fn userns_clone_fix(reading: &Reading<String>, run: &CommandRun) -> FixAction {
    if reading.found().map(String::as_str) == Some("0") {
        FixAction::CopyCommand("sudo sysctl -w kernel.unprivileged_userns_clone=1".to_owned())
    } else {
        // Der Aufruf besteht aus Woertern und wird erst hier zu einer Zeile;
        // enthaelt der Pfad des Programms etwas, das sich nicht beweisbar
        // zitieren laesst, gibt es keinen Befehl.
        run.fix()
    }
}

/// Prüfung 3: Der Kernel kennt seccomp, und er ist neu genug.
pub(super) fn seccomp(facts: &SeccompFacts) -> CheckOutcome {
    let line = match &facts.line {
        Reading::Found(line) => line,
        other => {
            let why = other
                .missing_because()
                .unwrap_or_else(|| "was not read".to_owned());
            return CheckOutcome::unmeasured(
                CheckId::Seccomp,
                format!("/proc/self/status {why}"),
                &["grep", "Seccomp", "/proc/self/status"],
            );
        }
    };

    let SeccompLine::Present(value) = line else {
        return CheckOutcome::fail(
            CheckId::Seccomp,
            "/proc/self/status carries no Seccomp field",
            finding_with_docs(
                DOCTOR_003,
                Severity::Blocking,
                "/proc/self/status carries no Seccomp field; this kernel was built without \
                 CONFIG_SECCOMP, and the shim cannot install its filter. Boot a kernel with \
                 seccomp support",
                FixAction::OpenUrl(SECCOMP_DOCS_URL.to_owned()),
                SECCOMP_DOCS_URL,
            ),
        );
    };

    let Some(release) = facts.kernel_release.found() else {
        let why = facts
            .kernel_release
            .missing_because()
            .unwrap_or_else(|| "was not read".to_owned());
        return CheckOutcome::unmeasured(
            CheckId::Seccomp,
            format!(
                "seccomp is available (Seccomp: {value}), but /proc/sys/kernel/osrelease {why}"
            ),
            &["cat", "/proc/sys/kernel/osrelease"],
        );
    };

    let evidence = format!("seccomp available (Seccomp: {value}), kernel {release}");
    match kernel_version(release) {
        None => CheckOutcome::unmeasured(
            CheckId::Seccomp,
            format!(
                "seccomp is available (Seccomp: {value}), but the kernel version could not \
                     be read from {release:?}"
            ),
            &["cat", "/proc/sys/kernel/osrelease"],
        ),
        Some(version) if version >= MIN_KERNEL => CheckOutcome::ok(CheckId::Seccomp, evidence),
        Some((major, minor)) => CheckOutcome::warn(
            CheckId::Seccomp,
            evidence,
            finding_with_docs(
                DOCTOR_003,
                Severity::Warning,
                format!(
                    "the kernel is {major}.{minor}; the seccomp filter of the shim is only \
                     measured against {}.{} and newer. It may still work, but nobody has \
                     checked it here",
                    MIN_KERNEL.0, MIN_KERNEL.1
                ),
                FixAction::OpenUrl(SECCOMP_DOCS_URL.to_owned()),
                SECCOMP_DOCS_URL,
            ),
        ),
    }
}

/// Die ersten beiden Zahlen einer Kernel-Fassung wie `6.1.0-18-amd64`.
fn kernel_version(release: &str) -> Option<(u32, u32)> {
    let mut parts = release
        .split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .map(|part| part.parse::<u32>().ok());
    let major = parts.next()??;
    let minor = parts.next()?.unwrap_or(0);
    Some((major, minor))
}

/// Prüfung 4: `$XDG_RUNTIME_DIR` ist gesetzt, privat und uns.
pub(super) fn runtime_dir(facts: &RuntimeDirFacts) -> CheckOutcome {
    match facts {
        RuntimeDirFacts::Unset { expected } => CheckOutcome::fail(
            CheckId::RuntimeDir,
            "XDG_RUNTIME_DIR is not set",
            finding(
                DOCTOR_004,
                Severity::Blocking,
                format!(
                    "XDG_RUNTIME_DIR is not set; there is no private directory for the daemon \
                     socket and the token. A logind session sets it to {}; over ssh without a \
                     session, lingering has to be enabled first",
                    expected.display()
                ),
                FixAction::CopyCommand("loginctl enable-linger".to_owned()),
            ),
        ),
        RuntimeDirFacts::Missing { path } => CheckOutcome::fail(
            CheckId::RuntimeDir,
            format!("XDG_RUNTIME_DIR={} does not exist", path.display()),
            finding(
                DOCTOR_004,
                Severity::Blocking,
                format!(
                    "XDG_RUNTIME_DIR points at {}, and that path does not exist; the variable \
                     is set, but nothing created the directory",
                    path.display()
                ),
                FixAction::CopyCommand("loginctl enable-linger".to_owned()),
            ),
        ),
        RuntimeDirFacts::Unreadable { path, error } => CheckOutcome::unmeasured(
            CheckId::RuntimeDir,
            format!("{} could not be read: {error}", path.display()),
            &["ls", "-ld", &path.to_string_lossy()],
        ),
        RuntimeDirFacts::Present {
            path,
            mode,
            owner_uid,
            our_uid,
            is_dir,
        } => runtime_dir_present(path, *mode, *owner_uid, *our_uid, *is_dir),
    }
}

/// Das Urteil über ein vorhandenes Laufzeitverzeichnis.
fn runtime_dir_present(
    path: &std::path::Path,
    mode: u32,
    owner_uid: u32,
    our_uid: u32,
    is_dir: bool,
) -> CheckOutcome {
    let shown = path.display();
    let evidence = format!("{shown} mode {mode:04o}, uid {owner_uid}");
    if !is_dir {
        return CheckOutcome::fail(
            CheckId::RuntimeDir,
            format!("{shown} is not a directory"),
            finding(
                DOCTOR_004,
                Severity::Blocking,
                format!("XDG_RUNTIME_DIR points at {shown}, which exists but is not a directory"),
                own_runtime_dir(our_uid),
            ),
        );
    }
    if owner_uid != our_uid {
        return CheckOutcome::fail(
            CheckId::RuntimeDir,
            evidence,
            finding(
                DOCTOR_004,
                Severity::Blocking,
                format!(
                    "{shown} belongs to uid {owner_uid}, but this process runs as uid {our_uid}; \
                     the daemon socket and the session token would lie in a stranger's directory"
                ),
                own_runtime_dir(our_uid),
            ),
        );
    }
    // Genau 0700, nicht bloss „nicht offener als 0700". Nach aussen ist die
    // Frage, wer hinein darf; nach innen, ob wir selbst noch hinein und darin
    // schreiben koennen. `0500` und `0600` bestehen eine Pruefung auf
    // `mode & 0o077` und lassen den Start trotzdem scheitern: ohne `+x` kommt
    // niemand in das Verzeichnis, ohne `+w` legt der Daemon dort weder
    // `daemon.sock` noch das Sitzungs-Token an. HUM-075 und `docs/cli.md`
    // verlangen beide 0700.
    if mode != DIR_MODE {
        return CheckOutcome::fail(
            CheckId::RuntimeDir,
            evidence,
            finding(
                DOCTOR_004,
                Severity::Blocking,
                format!("{shown} has mode {mode:04o}; {}", mode_problem(mode, path)),
                chmod_fix(path, our_uid),
            ),
        );
    }
    CheckOutcome::ok(CheckId::RuntimeDir, evidence)
}

/// Der Vorschlag, den Modus zu berichtigen.
///
/// Ein Pfad aus `$XDG_RUNTIME_DIR` wird nie durch Interpolation zu einem
/// Befehl: `XDG_RUNTIME_DIR='/tmp/h; touch /tmp/pwn'` ergaebe sonst die
/// kopierbare Zeile `chmod 700 /tmp/h; touch /tmp/pwn`. Laesst sich der Pfad
/// nicht beweisbar zitieren, bleibt der Vorschlag, die Variable auf das
/// eigene Laufzeitverzeichnis zu richten — das ist ein Weg heraus, der keinen
/// fremden Text braucht.
fn chmod_fix(path: &std::path::Path, our_uid: u32) -> FixAction {
    match super::shell_command(&["chmod", "700", &path.to_string_lossy()]) {
        Some(command) => FixAction::CopyCommand(command),
        None => own_runtime_dir(our_uid),
    }
}

/// Der Vorschlag, wenn `$XDG_RUNTIME_DIR` auf das Falsche zeigt.
///
/// Ein `ls -ld` stand hier zuerst und war keiner: Es zeigt noch einmal, was in
/// der Zeile schon steht. Was ein Mensch wirklich tun kann, ist die Variable
/// auf sein eigenes Laufzeitverzeichnis zu richten; `chown` braeuchte Root und
/// waere fuer ein fremdes Verzeichnis ohnehin die falsche Antwort.
fn own_runtime_dir(our_uid: u32) -> FixAction {
    FixAction::SetEnv {
        key: "XDG_RUNTIME_DIR".to_owned(),
        value: format!("/run/user/{our_uid}"),
    }
}

/// Was an einem Modus ungleich [`DIR_MODE`] nicht stimmt.
///
/// Zwei Richtungen, und sie brauchen zwei Saetze: Ein zu offenes Verzeichnis
/// laesst Fremde an Socket und Token, ein zu enges laesst uns selbst nicht
/// mehr hinein. Ein gemeinsamer Satz („it has to be 0700") sagte einem
/// Menschen mit `0500` nicht, was ihm fehlt.
fn mode_problem(mode: u32, path: &std::path::Path) -> String {
    if mode & 0o077 != 0 {
        return format!(
            "group or world may enter it. The daemon socket and the session token live there \
             and must stay private, so it has to be {DIR_MODE:04o}"
        );
    }
    let mut missing = Vec::new();
    if mode & 0o400 == 0 {
        missing.push("read (nothing can list it)");
    }
    if mode & 0o200 == 0 {
        missing.push("write (the daemon cannot create daemon.sock or the session token there)");
    }
    if mode & 0o100 == 0 {
        missing.push("execute (nothing can enter it)");
    }
    format!(
        "the owner is missing {}; {} has to be {DIR_MODE:04o}",
        missing.join(", "),
        path.display()
    )
}

/// Prüfung 5: Eine systemd-Nutzersitzung läuft.
///
/// Nur eine Warnung: Ohne systemd startet man `humanitld` von Hand, und
/// Humanitl läuft dann genauso. Was fehlt, ist der Start beim Anmelden.
pub(super) fn systemd_user(facts: &SystemdFacts) -> CheckOutcome {
    let run = match &facts.state {
        Reading::Found(run) => run,
        Reading::Absent => {
            return CheckOutcome::warn(
                CheckId::SystemdUser,
                format!("no systemctl in PATH={}", facts.searched),
                finding(
                    DOCTOR_005,
                    Severity::Warning,
                    format!(
                        "no executable systemctl in PATH={}; this machine has no systemd user \
                         session, so nothing starts humanitld at login. Started by hand it works \
                         the same",
                        facts.searched
                    ),
                    FixAction::CopyCommand("humanitld".to_owned()),
                ),
            );
        }
        Reading::Unreadable(error) => {
            return CheckOutcome::unmeasured(
                CheckId::SystemdUser,
                format!("systemctl --user is-system-running did not answer: {error}"),
                &["systemctl", "--user", "is-system-running"],
            );
        }
    };

    // Erst der Ausgang, dann die Ausgabe. Ein Aufruf, der in die Frist lief
    // oder von einem Signal beendet wurde, hat nichts gemessen — auch dann
    // nicht, wenn vorher schon `running` auf der Ausgabe stand. Wer hier nur
    // `stdout` liest, macht aus einer abgelaufenen Frist ein `ok`.
    match run.outcome {
        RunOutcome::TimedOut(_) | RunOutcome::Signalled(_) => {
            return CheckOutcome::unmeasured(
                CheckId::SystemdUser,
                format!(
                    "systemctl --user is-system-running {}",
                    run.outcome.describe()
                ),
                &run.parts(),
            );
        }
        RunOutcome::Exited(_) => {}
    }

    let state = run.stdout.trim();
    match state {
        // Nur `running` verlangt Exit 0: Fuer jeden anderen Zustand endet
        // `is-system-running` laut seiner eigenen Beschreibung mit einem
        // Fehlercode, und das ist dort die Auskunft und kein Fehlschlag.
        "running" if run.outcome.is_success() => {
            CheckOutcome::ok(CheckId::SystemdUser, "systemd user session running")
        }
        "running" => CheckOutcome::unmeasured(
            CheckId::SystemdUser,
            format!(
                "systemctl --user is-system-running said running but ended with {}; \
                 output and exit code contradict each other",
                run.outcome.describe()
            ),
            &run.parts(),
        ),
        "degraded" => CheckOutcome::warn(
            CheckId::SystemdUser,
            "systemd user session degraded",
            finding(
                DOCTOR_005,
                Severity::Warning,
                "the systemd user session is degraded: at least one unit of this user failed. \
                 humanitld can still be installed and started, but check which unit is broken",
                FixAction::CopyCommand("systemctl --user --failed".to_owned()),
            ),
        ),
        "" => CheckOutcome::unmeasured(
            CheckId::SystemdUser,
            format!(
                "systemctl --user is-system-running said nothing ({}): {}",
                run.outcome.describe(),
                run.first_message()
            ),
            &run.parts(),
        ),
        other => CheckOutcome::warn(
            CheckId::SystemdUser,
            format!("systemd user session {other}"),
            finding(
                DOCTOR_005,
                Severity::Warning,
                format!(
                    "systemctl --user is-system-running says {other}; there is no usable systemd \
                     user session, so nothing starts humanitld at login. Over ssh this usually \
                     means the session does not linger"
                ),
                FixAction::CopyCommand("loginctl enable-linger".to_owned()),
            ),
        ),
    }
}

/// Prüfung 6: Der Daemon antwortet und spricht denselben Vertrag.
pub(super) fn daemon(facts: &DaemonFacts) -> CheckOutcome {
    match facts {
        DaemonFacts::NotTried { socket, why } => CheckOutcome::unmeasured(
            CheckId::Daemon,
            format!("{} was not contacted: {why}", socket.display()),
            &["humanitl", "daemon", "status"],
        ),
        DaemonFacts::Unreachable { socket, diagnostic } => CheckOutcome::warn(
            CheckId::Daemon,
            format!("no daemon on {}", socket.display()),
            wrapping(DOCTOR_006, diagnostic, FixAction::InstallService),
        ),
        DaemonFacts::Reachable {
            socket,
            version,
            proto,
            expected_proto,
        } => {
            let evidence = format!(
                "humanitld {version}, contract {}.{}, on {}",
                proto.0,
                proto.1,
                socket.display()
            );
            if proto.0 == expected_proto.0 {
                CheckOutcome::ok(CheckId::Daemon, evidence)
            } else {
                CheckOutcome::fail(
                    CheckId::Daemon,
                    evidence,
                    finding(
                        DOCTOR_006,
                        Severity::Blocking,
                        format!(
                            "the daemon speaks contract {}.{}, this client speaks {}.{}; a \
                             different major means messages one side sends the other cannot \
                             read",
                            proto.0, proto.1, expected_proto.0, expected_proto.1
                        ),
                        FixAction::CopyCommand("systemctl --user restart humanitld".to_owned()),
                    ),
                )
            }
        }
    }
}

/// Prüfung 7: Das Kommando des Agenten liegt auf dem Host.
pub(super) fn agent(facts: &AgentFacts) -> CheckOutcome {
    match facts {
        AgentFacts::Missing {
            adapter,
            command,
            searched,
            install,
        } => CheckOutcome::warn(
            CheckId::Agent,
            format!("no {command} in PATH={searched}"),
            finding_with_docs(
                DOCTOR_007,
                Severity::Warning,
                format!(
                    "the adapter {adapter} starts {command}, and no executable of that name lies \
                     in PATH={searched}. The sandbox starts even so, because the path inside it \
                     may be another one; without the command the agent does not run"
                ),
                FixAction::CopyCommand(install.clone()),
                opencode::DOCS_URL,
            ),
        ),
        AgentFacts::Found {
            adapter,
            command,
            program,
            version,
        } => match version {
            Reading::Found(text) => CheckOutcome::ok(
                CheckId::Agent,
                format!("{adapter} {text} at {}", program.display()),
            ),
            other => CheckOutcome::unmeasured(
                CheckId::Agent,
                format!(
                    "{command} lies at {}, but its version was not read: {}",
                    program.display(),
                    version_reason(other)
                ),
                &[&program.to_string_lossy(), "--version"],
            ),
        },
    }
}

/// Prüfung 8: Das Sprachmodell antwortet.
///
/// Diese Zeile ist die einzige, hinter der eine Verbindung stünde, und deshalb
/// die einzige, die nicht von allein misst. Ohne Messung steht `DOCTOR_013`
/// samt dem Befehl, der sie auslöst; siehe die Beschreibung des Moduls
/// [`super`].
pub(super) fn llm(facts: &LlmFacts) -> CheckOutcome {
    match facts {
        LlmFacts::NoEndpoint => CheckOutcome::warn(
            CheckId::Llm,
            "llm.endpoint is not set",
            finding(
                DOCTOR_008,
                Severity::Warning,
                "llm.endpoint is not set; there is no language model for the agent, and no \
                 passthrough rule to declare",
                FixAction::ChangeSetting {
                    key: "llm.endpoint".to_owned(),
                    value: opencode::EXAMPLE_ENDPOINT.to_owned(),
                },
            ),
        ),
        LlmFacts::NotContacted { endpoint, command } => CheckOutcome::warn(
            CheckId::Llm,
            format!("{endpoint} was not contacted"),
            not_contacted(endpoint, command),
        ),
        LlmFacts::Silent {
            endpoint,
            diagnostic,
        } => CheckOutcome::warn(
            CheckId::Llm,
            format!("{endpoint} did not answer"),
            wrapping(
                DOCTOR_008,
                diagnostic,
                FixAction::ChangeSetting {
                    key: "llm.endpoint".to_owned(),
                    value: opencode::EXAMPLE_ENDPOINT.to_owned(),
                },
            ),
        ),
        LlmFacts::Answered {
            endpoint,
            flavor,
            models,
            latency_ms,
            diagnostics,
        } => {
            let evidence = format!("{flavor} at {endpoint}, {models} models, {latency_ms} ms");
            match diagnostics.first() {
                None => CheckOutcome::ok(CheckId::Llm, evidence),
                Some(first) => CheckOutcome::warn(
                    CheckId::Llm,
                    format!("{evidence} ({})", codes_of(diagnostics)),
                    wrapping(
                        DOCTOR_008,
                        first,
                        FixAction::ChangeSetting {
                            key: "llm.endpoint".to_owned(),
                            value: opencode::EXAMPLE_ENDPOINT.to_owned(),
                        },
                    ),
                ),
            }
        }
    }
}

/// Die Codes einer Liste von Befunden, durch Komma getrennt.
fn codes_of(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Prüfung 9: Die Arbeitsumgebung hat einen Platz für das Anzeigesymbol.
pub(super) fn tray(facts: &TrayFacts) -> CheckOutcome {
    if facts.readable_dirs == 0 {
        return CheckOutcome::unmeasured(
            CheckId::Tray,
            format!(
                "none of the {} library directories could be read, so a missing \
                 libayatana-appindicator3 would prove nothing",
                facts.searched_dirs
            ),
            &["ldconfig", "-p"],
        );
    }

    let desktop = facts.desktop.as_deref().unwrap_or_default();
    let gnome = desktop.to_ascii_uppercase().contains("GNOME");
    let seat = match &facts.desktop {
        Some(desktop) => format!("XDG_CURRENT_DESKTOP={desktop}"),
        None => "XDG_CURRENT_DESKTOP is not set".to_owned(),
    };

    let Some(library) = facts.library.as_ref() else {
        return CheckOutcome::warn(
            CheckId::Tray,
            format!(
                "no libayatana-appindicator3 in {} directories, {seat}",
                facts.readable_dirs
            ),
            finding(
                DOCTOR_009,
                Severity::Warning,
                format!(
                    "no libayatana-appindicator3 in the {} readable library directories ({seat}). \
                     The application runs without a tray icon; the number of waiting requests \
                     then only shows in the window title",
                    facts.readable_dirs
                ),
                FixAction::CopyCommand(TRAY_INSTALL_COMMAND.to_owned()),
            ),
        );
    };

    let evidence = format!("{}, {seat}", library.display());
    if gnome {
        return CheckOutcome::warn(
            CheckId::Tray,
            evidence,
            finding_with_docs(
                DOCTOR_009,
                Severity::Warning,
                format!(
                    "{} is installed, but {seat}: GNOME has had no tray of its own since 3.26. \
                     The AppIndicator extension puts it back; without it the number of waiting \
                     requests only shows in the window title",
                    library.display()
                ),
                FixAction::OpenUrl(GNOME_EXTENSION_URL.to_owned()),
                GNOME_EXTENSION_URL,
            ),
        );
    }
    CheckOutcome::ok(CheckId::Tray, evidence)
}

/// Prüfung 10: Der Renderer der Oberfläche verträgt sich mit dieser Grafik.
///
/// Der bekannte Fall ist Impeller auf einem NVIDIA-Treiber unter Wayland: Die
/// Oberfläche startet, zeigt aber ein schwarzes Fenster. Ist keine NVIDIA
/// geladen, kann der Fall nicht eintreten, und die Sitzungsart ist dann
/// gleichgültig.
pub(super) fn renderer(facts: &RendererFacts) -> CheckOutcome {
    let engine = match &facts.flutter_engine {
        Some(engine) => format!(", FLUTTER_ENGINE={engine}"),
        None => String::new(),
    };
    let session = facts.session_type.as_deref().unwrap_or("unknown");

    let nvidia = match &facts.nvidia {
        Reading::Found(nvidia) => *nvidia,
        other => {
            let why = other
                .missing_because()
                .unwrap_or_else(|| "was not read".to_owned());
            return CheckOutcome::unmeasured(
                CheckId::Renderer,
                format!("/proc/modules {why}, so the graphics driver is unknown"),
                &["grep", "-c", "nvidia", "/proc/modules"],
            );
        }
    };

    if !nvidia {
        return CheckOutcome::ok(
            CheckId::Renderer,
            format!("session {session}, no nvidia module loaded{engine}"),
        );
    }

    let evidence = format!("session {session}, nvidia module loaded{engine}");
    if facts.session_type.is_none() {
        return CheckOutcome::unmeasured(
            CheckId::Renderer,
            format!(
                "an nvidia module is loaded, but XDG_SESSION_TYPE is not set, so it is unknown \
                 whether this is a Wayland session{engine}"
            ),
            &["printenv", "XDG_SESSION_TYPE"],
        );
    }
    if session.eq_ignore_ascii_case("wayland") {
        return CheckOutcome::warn(
            CheckId::Renderer,
            evidence,
            finding_with_docs(
                DOCTOR_010,
                Severity::Warning,
                format!(
                    "an nvidia module is loaded and XDG_SESSION_TYPE is {session}: Impeller, the \
                     renderer of the desktop application, is known to show a black window on \
                     this combination. Start the application with {IMPELLER_OFF_FLAG} if it \
                     stays black{engine}"
                ),
                FixAction::OpenUrl(IMPELLER_DOCS_URL.to_owned()),
                IMPELLER_DOCS_URL,
            ),
        );
    }
    CheckOutcome::ok(CheckId::Renderer, evidence)
}

/// Prüfung 11: Im Datenverzeichnis ist noch Platz für die Aufzeichnung.
pub(super) fn disk_space(facts: &DiskFacts) -> CheckOutcome {
    match facts {
        DiskFacts::Unreadable { path, error } => CheckOutcome::unmeasured(
            CheckId::DiskSpace,
            format!(
                "the free space of {} could not be read: {error}",
                path.display()
            ),
            &["df", "-h", &path.to_string_lossy()],
        ),
        DiskFacts::Measured {
            path,
            available_bytes,
        } => {
            let evidence = format!("{} free in {}", size(*available_bytes), path.display());
            if *available_bytes >= super::MIN_FREE_BYTES {
                CheckOutcome::ok(CheckId::DiskSpace, evidence)
            } else {
                CheckOutcome::warn(
                    CheckId::DiskSpace,
                    evidence,
                    finding(
                        DOCTOR_011,
                        Severity::Warning,
                        format!(
                            "only {} are free in {}; the recording keeps flows and bodies there, \
                             and below {MIN_FREE_TEXT} it will stop writing before long. Either \
                             free the file system or shorten the retention",
                            size(*available_bytes),
                            path.display()
                        ),
                        FixAction::ChangeSetting {
                            key: "recorder.retention_days".to_owned(),
                            value: "7".to_owned(),
                        },
                    ),
                )
            }
        }
    }
}

/// Warum eine Fassung nicht gelesen wurde, als Halbsatz ohne Vorderglied.
fn version_reason(reading: &Reading<String>) -> String {
    match reading {
        Reading::Found(_) => "it was read".to_owned(),
        Reading::Absent => "the call was not made".to_owned(),
        Reading::Unreadable(error) => error.clone(),
    }
}

/// Bytes in der Einheit, in der ein Mensch sie liest.
///
/// Unter einem Gibibyte in MiB, darueber in GiB: Die Zeile soll die Frage
/// beantworten, ob der Platz reicht, und `151684.9 MiB` beantwortet sie nicht.
fn size(bytes: u64) -> String {
    #[allow(clippy::cast_precision_loss)]
    let mib = bytes as f64 / (1024.0 * 1024.0);
    if bytes < super::MIN_FREE_BYTES {
        format!("{mib:.1} MiB")
    } else {
        format!("{:.1} GiB", mib / 1024.0)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::path::PathBuf;

    use humanitl_core::diagnostics::codes::{DAEMON_001, LLM_001};
    use humanitl_core::{Diagnostic, FixAction, Severity};

    use super::super::{
        AgentFacts, BwrapFacts, CheckOutcome, CheckStatus, CommandRun, DaemonFacts, DiskFacts,
        LlmFacts, Reading, RendererFacts, RunOutcome, RuntimeDirFacts, SeccompFacts, SeccompLine,
        SystemdFacts, TrayFacts, UsernsFacts,
    };
    use super::{
        agent, bwrap, daemon, disk_space, kernel_version, llm, renderer, runtime_dir, seccomp,
        systemd_user, tray, userns,
    };
    use crate::bwrap::Version;
    use std::time::Duration;

    /// Eine Zeile in Woerter, fuer die Vorgaben der Tests.
    fn words(line: &str) -> Vec<String> {
        line.split_whitespace().map(str::to_owned).collect()
    }

    fn failed_probe(stderr: &str) -> CommandRun {
        CommandRun::new(
            words("bwrap --unshare-user -- /bin/true"),
            RunOutcome::Exited(1),
            String::new(),
            stderr.to_owned(),
        )
    }

    fn userns_facts(apparmor: Reading<String>, clone: Reading<String>) -> UsernsFacts {
        UsernsFacts {
            probe: Reading::Found(failed_probe("bwrap: setting up uid map: Permission denied")),
            apparmor_restrict: apparmor,
            userns_clone: clone,
        }
    }

    fn why_of(outcome: &CheckOutcome) -> String {
        outcome.diagnostic().expect("a finding").why.clone()
    }

    #[test]
    fn bwrap_missing_is_fail() {
        let outcome = bwrap(&BwrapFacts::Missing {
            searched: "/usr/bin:/bin".to_owned(),
        });
        assert_eq!(outcome.status(), CheckStatus::Fail);
        let diagnostic = outcome.diagnostic().expect("a finding");
        assert_eq!(diagnostic.code.as_str(), "DOCTOR_001");
        assert!(
            diagnostic.why.contains("/usr/bin:/bin"),
            "{}",
            diagnostic.why
        );
        assert_eq!(
            diagnostic.fix,
            Some(FixAction::CopyCommand(
                "sudo apt install bubblewrap".to_owned()
            ))
        );
        assert!(!outcome.is_unmeasured(), "a missing bwrap was measured");
    }

    #[test]
    fn bwrap_too_old_is_a_different_sentence_than_bwrap_missing() {
        let old = bwrap(&BwrapFacts::Found {
            program: PathBuf::from("/usr/bin/bwrap"),
            version: Version(0, 6, 2),
        });
        assert_eq!(old.status(), CheckStatus::Fail);
        assert!(
            why_of(&old).contains("is bubblewrap 0.6.2"),
            "{}",
            why_of(&old)
        );
        assert!(old.evidence().contains("0.6.2"), "{}", old.evidence());

        let new = bwrap(&BwrapFacts::Found {
            program: PathBuf::from("/usr/bin/bwrap"),
            version: Version(0, 11, 0),
        });
        assert_eq!(new.status(), CheckStatus::Ok);
        assert!(new.diagnostic().is_none());
    }

    #[test]
    fn a_bwrap_that_does_not_answer_is_not_a_bwrap_that_is_too_old() {
        let outcome = bwrap(&BwrapFacts::Unreadable {
            program: PathBuf::from("/usr/bin/bwrap"),
            error: "no digits in its answer".to_owned(),
        });
        assert_eq!(outcome.status(), CheckStatus::Warn);
        assert!(outcome.is_unmeasured());
        assert_eq!(
            outcome.diagnostic().expect("a finding").code.as_str(),
            "DOCTOR_012"
        );
    }

    #[test]
    fn userns_blocked_apparmor_hint() {
        let outcome = userns(&userns_facts(
            Reading::Found("1".to_owned()),
            Reading::Absent,
        ));
        assert_eq!(outcome.status(), CheckStatus::Fail);
        let diagnostic = outcome.diagnostic().expect("a finding");
        assert_eq!(diagnostic.code.as_str(), "DOCTOR_002");
        assert!(
            diagnostic
                .why
                .contains("apparmor_restrict_unprivileged_userns is 1"),
            "{}",
            diagnostic.why
        );
        assert_eq!(
            diagnostic.fix,
            Some(FixAction::CopyCommand(
                "sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0".to_owned()
            ))
        );
        assert!(diagnostic.docs.is_some());
    }

    #[test]
    fn a_kernel_without_the_apparmor_switch_is_not_a_kernel_that_has_it_off() {
        let absent = userns(&userns_facts(Reading::Absent, Reading::Absent));
        assert!(
            why_of(&absent).contains("neither confirmed nor ruled out"),
            "{}",
            why_of(&absent)
        );

        let off = userns(&userns_facts(
            Reading::Found("0".to_owned()),
            Reading::Found("0".to_owned()),
        ));
        assert!(
            why_of(&off).contains("AppArmor is not the reason"),
            "{}",
            why_of(&off)
        );
        assert_eq!(
            off.diagnostic().expect("a finding").fix,
            Some(FixAction::CopyCommand(
                "sudo sysctl -w kernel.unprivileged_userns_clone=1".to_owned()
            ))
        );
    }

    #[test]
    fn userns_without_bwrap_is_unmeasured_and_never_green() {
        let outcome = userns(&UsernsFacts {
            probe: Reading::Absent,
            apparmor_restrict: Reading::Absent,
            userns_clone: Reading::Absent,
        });
        assert_eq!(outcome.status(), CheckStatus::Warn);
        assert!(outcome.is_unmeasured());
    }

    #[test]
    fn a_hanging_userns_probe_is_a_failure_and_says_the_deadline() {
        let outcome = userns(&UsernsFacts {
            probe: Reading::Found(CommandRun::new(
                words("bwrap --unshare-user -- /bin/true"),
                RunOutcome::TimedOut(std::time::Duration::from_secs(2)),
                String::new(),
                String::new(),
            )),
            apparmor_restrict: Reading::Found("1".to_owned()),
            userns_clone: Reading::Absent,
        });
        assert_eq!(outcome.status(), CheckStatus::Fail);
        assert!(
            outcome.evidence().contains("no answer within 2000 ms"),
            "{}",
            outcome.evidence()
        );
    }

    #[test]
    fn a_kernel_without_seccomp_fails_and_an_unreadable_status_does_not() {
        let without = seccomp(&SeccompFacts {
            line: Reading::Found(SeccompLine::Missing),
            kernel_release: Reading::Found("6.1.0".to_owned()),
        });
        assert_eq!(without.status(), CheckStatus::Fail);
        assert_eq!(
            without.diagnostic().expect("a finding").code.as_str(),
            "DOCTOR_003"
        );

        let unreadable = seccomp(&SeccompFacts {
            line: Reading::Unreadable("EACCES".to_owned()),
            kernel_release: Reading::Found("6.1.0".to_owned()),
        });
        assert_eq!(unreadable.status(), CheckStatus::Warn);
        assert!(unreadable.is_unmeasured());
        assert!(
            why_of(&unreadable).contains("EACCES"),
            "{}",
            why_of(&unreadable)
        );
    }

    #[test]
    fn seccomp_needs_both_halves_before_it_is_green() {
        let green = seccomp(&SeccompFacts {
            line: Reading::Found(SeccompLine::Present("0".to_owned())),
            kernel_release: Reading::Found("6.1.0-18-amd64".to_owned()),
        });
        assert_eq!(green.status(), CheckStatus::Ok);

        let old = seccomp(&SeccompFacts {
            line: Reading::Found(SeccompLine::Present("2".to_owned())),
            kernel_release: Reading::Found("4.19.0".to_owned()),
        });
        assert_eq!(old.status(), CheckStatus::Warn);
        assert_eq!(
            old.diagnostic().expect("a finding").code.as_str(),
            "DOCTOR_003"
        );

        let half = seccomp(&SeccompFacts {
            line: Reading::Found(SeccompLine::Present("0".to_owned())),
            kernel_release: Reading::Absent,
        });
        assert_eq!(half.status(), CheckStatus::Warn);
        assert!(half.is_unmeasured(), "{}", half.evidence());
    }

    #[test]
    fn kernel_versions_are_read_from_what_the_kernel_writes() {
        assert_eq!(kernel_version("6.1.0-18-amd64"), Some((6, 1)));
        assert_eq!(kernel_version("7.1.10+deb14-amd64"), Some((7, 1)));
        assert_eq!(kernel_version("5"), None);
        assert_eq!(kernel_version("not a version"), None);
    }

    #[test]
    fn xdg_runtime_missing() {
        let unset = runtime_dir(&RuntimeDirFacts::Unset {
            expected: PathBuf::from("/run/user/1000"),
        });
        assert_eq!(unset.status(), CheckStatus::Fail);
        let diagnostic = unset.diagnostic().expect("a finding");
        assert_eq!(diagnostic.code.as_str(), "DOCTOR_004");
        assert!(
            diagnostic.why.contains("/run/user/1000"),
            "{}",
            diagnostic.why
        );

        let missing = runtime_dir(&RuntimeDirFacts::Missing {
            path: PathBuf::from("/run/user/1000"),
        });
        assert_eq!(missing.status(), CheckStatus::Fail);
        assert!(
            why_of(&missing).contains("does not exist"),
            "{}",
            why_of(&missing)
        );
        assert_ne!(why_of(&missing), diagnostic.why, "two different machines");
    }

    #[test]
    fn a_runtime_dir_that_others_may_enter_fails() {
        let loose = runtime_dir(&RuntimeDirFacts::Present {
            path: PathBuf::from("/run/user/1000"),
            mode: 0o755,
            owner_uid: 1000,
            our_uid: 1000,
            is_dir: true,
        });
        assert_eq!(loose.status(), CheckStatus::Fail);
        assert!(why_of(&loose).contains("0755"), "{}", why_of(&loose));
        assert_eq!(
            loose.diagnostic().expect("a finding").fix,
            Some(FixAction::CopyCommand(
                "chmod 700 /run/user/1000".to_owned()
            ))
        );

        let stranger = runtime_dir(&RuntimeDirFacts::Present {
            path: PathBuf::from("/run/user/1000"),
            mode: 0o700,
            owner_uid: 0,
            our_uid: 1000,
            is_dir: true,
        });
        assert_eq!(stranger.status(), CheckStatus::Fail);
        assert!(why_of(&stranger).contains("uid 0"), "{}", why_of(&stranger));

        let good = runtime_dir(&RuntimeDirFacts::Present {
            path: PathBuf::from("/run/user/1000"),
            mode: 0o700,
            owner_uid: 1000,
            our_uid: 1000,
            is_dir: true,
        });
        assert_eq!(good.status(), CheckStatus::Ok);
    }

    #[test]
    fn a_machine_without_systemd_warns_and_says_how_to_start_by_hand() {
        let outcome = systemd_user(&SystemdFacts {
            state: Reading::Absent,
            searched: "/usr/bin:/bin".to_owned(),
        });
        assert_eq!(outcome.status(), CheckStatus::Warn);
        assert_eq!(
            outcome.diagnostic().expect("a finding").fix,
            Some(FixAction::CopyCommand("humanitld".to_owned()))
        );

        let running = systemd_user(&SystemdFacts {
            state: Reading::Found(CommandRun::new(
                words("systemctl --user is-system-running"),
                RunOutcome::Exited(0),
                "running".to_owned(),
                String::new(),
            )),
            searched: "/usr/bin".to_owned(),
        });
        assert_eq!(running.status(), CheckStatus::Ok);

        let offline = systemd_user(&SystemdFacts {
            state: Reading::Found(CommandRun::new(
                words("systemctl --user is-system-running"),
                RunOutcome::Exited(1),
                "offline".to_owned(),
                String::new(),
            )),
            searched: "/usr/bin".to_owned(),
        });
        assert_eq!(offline.status(), CheckStatus::Warn);
        assert_eq!(
            offline.diagnostic().expect("a finding").code.as_str(),
            "DOCTOR_005"
        );
    }

    /// Ein Lauf von `systemctl --user is-system-running` mit gegebenem Ausgang.
    fn systemd_run(outcome: RunOutcome, stdout: &str) -> SystemdFacts {
        SystemdFacts {
            state: Reading::Found(CommandRun::new(
                words("/usr/bin/systemctl --user is-system-running"),
                outcome,
                stdout,
                "",
            )),
            searched: "/usr/bin".to_owned(),
        }
    }

    /// Eine abgelaufene Frist ist keine Messung — auch nicht mit `running`
    /// auf der Ausgabe.
    ///
    /// Der erste Entwurf las nur `stdout`. Ein `systemctl`, das an seiner
    /// Frist starb, nachdem es `running` geschrieben hatte, wurde damit zu
    /// einem gruenen Haken; eines, das an der Frist starb und etwas anderes
    /// geschrieben hatte, zu `DOCTOR_005` — also „die Sitzung ist in einem
    /// seltsamen Zustand" statt „ich konnte nicht messen".
    #[test]
    fn a_systemd_call_that_did_not_finish_is_never_a_measurement() {
        for (outcome, stdout) in [
            (RunOutcome::TimedOut(Duration::from_secs(2)), "running"),
            (RunOutcome::TimedOut(Duration::from_secs(2)), "degraded"),
            (RunOutcome::TimedOut(Duration::from_secs(2)), ""),
            (RunOutcome::Signalled(9), "running"),
            (RunOutcome::Signalled(15), "offline"),
        ] {
            let outcome_line = systemd_user(&systemd_run(outcome, stdout));
            assert_eq!(
                outcome_line.status(),
                CheckStatus::Warn,
                "{outcome:?} with {stdout:?}"
            );
            assert!(
                outcome_line.is_unmeasured(),
                "{outcome:?} with {stdout:?}: {}",
                outcome_line.evidence()
            );
            assert_eq!(
                outcome_line.diagnostic().expect("a finding").code.as_str(),
                "DOCTOR_012",
                "{outcome:?} with {stdout:?}"
            );
        }
    }

    /// `running` wird nur gruen, wenn der Aufruf auch mit 0 endete.
    ///
    /// Fuer jeden anderen Zustand endet `is-system-running` von sich aus mit
    /// einem Fehlercode; dort ist er die Auskunft. Ausgabe `running` **und**
    /// ein Fehlercode widersprechen sich dagegen, und aus einem Widerspruch
    /// wird hier keine Messung.
    #[test]
    fn running_is_green_only_when_the_call_really_succeeded() {
        let green = systemd_user(&systemd_run(RunOutcome::Exited(0), "running"));
        assert_eq!(green.status(), CheckStatus::Ok);
        assert!(green.diagnostic().is_none());

        let contradiction = systemd_user(&systemd_run(RunOutcome::Exited(1), "running"));
        assert!(
            contradiction.is_unmeasured(),
            "{}",
            contradiction.evidence()
        );
        assert!(
            why_of(&contradiction).contains("contradict"),
            "{}",
            why_of(&contradiction)
        );

        // Und andersherum: `degraded` mit Exit 1 ist die normale Auskunft von
        // systemd und bleibt eine gemessene Warnung.
        let degraded = systemd_user(&systemd_run(RunOutcome::Exited(1), "degraded"));
        assert_eq!(degraded.status(), CheckStatus::Warn);
        assert!(!degraded.is_unmeasured());
        assert_eq!(
            degraded.diagnostic().expect("a finding").code.as_str(),
            "DOCTOR_005"
        );
    }

    /// Das Laufzeitverzeichnis muss genau `0700` sein, nicht nur „nicht
    /// offener".
    ///
    /// Die erste Fassung prueft `mode & 0o077`. `0500`, `0600` und `0000`
    /// bestehen das und lassen den Start trotzdem scheitern: ohne `+x` kommt
    /// niemand hinein, ohne `+w` legt der Daemon dort weder `daemon.sock` noch
    /// das Sitzungs-Token an.
    #[test]
    fn the_runtime_directory_has_to_be_exactly_0700() {
        let line = |mode: u32| {
            runtime_dir(&RuntimeDirFacts::Present {
                path: PathBuf::from("/run/user/1000"),
                mode,
                owner_uid: 1000,
                our_uid: 1000,
                is_dir: true,
            })
        };

        assert_eq!(line(0o700).status(), CheckStatus::Ok);
        assert!(line(0o700).diagnostic().is_none());

        // Zu offen: die Zeile nennt Gruppe und Welt.
        for mode in [0o755_u32, 0o770, 0o707, 0o701] {
            let outcome = line(mode);
            assert_eq!(outcome.status(), CheckStatus::Fail, "{mode:04o}");
            assert!(
                why_of(&outcome).contains("group or world"),
                "{mode:04o}: {}",
                why_of(&outcome)
            );
        }

        // Zu eng: die Zeile nennt, welches Recht dem Eigentuemer fehlt.
        for (mode, missing) in [
            (0o600_u32, "execute"),
            (0o500, "write"),
            (0o400, "write"),
            (0o000, "read"),
        ] {
            let outcome = line(mode);
            assert_eq!(outcome.status(), CheckStatus::Fail, "{mode:04o}");
            let why = why_of(&outcome);
            assert!(why.contains("the owner is missing"), "{mode:04o}: {why}");
            assert!(why.contains(missing), "{mode:04o}: {why}");
            assert!(
                !why.contains("group or world"),
                "{mode:04o} is too tight, not too open: {why}"
            );
            assert_eq!(
                outcome.diagnostic().expect("a finding").fix,
                Some(FixAction::CopyCommand(
                    "chmod 700 /run/user/1000".to_owned()
                ))
            );
        }

        // Und `0000` fehlt alles drei.
        let none = why_of(&line(0o000));
        for missing in ["read", "write", "execute"] {
            assert!(none.contains(missing), "{none}");
        }
    }

    /// Der kopierbare Befehl nimmt keine Einschleusung an.
    ///
    /// `XDG_RUNTIME_DIR='/tmp/h; touch /tmp/humanitl-pwn'` ergaebe durch
    /// Interpolation die Zeile `chmod 700 /tmp/h; touch /tmp/humanitl-pwn`,
    /// und ein Mensch kopiert sie und fuehrt sie aus.
    #[test]
    fn the_chmod_of_a_hostile_path_stays_one_word() {
        let hostile = "/tmp/h; touch /tmp/humanitl-pwn";
        let outcome = runtime_dir(&RuntimeDirFacts::Present {
            path: PathBuf::from(hostile),
            mode: 0o755,
            owner_uid: 1000,
            our_uid: 1000,
            is_dir: true,
        });
        let Some(FixAction::CopyCommand(command)) =
            outcome.diagnostic().expect("a finding").fix.clone()
        else {
            panic!("a quotable path yields a command");
        };
        assert_eq!(command, "chmod 700 '/tmp/h; touch /tmp/humanitl-pwn'");
        let words = shlex::split(&command).expect("the line parses");
        assert_eq!(
            words,
            vec!["chmod", "700", hostile],
            "three words, not five"
        );

        // Und ein Pfad, der sich nicht beweisbar zitieren laesst, ergibt gar
        // keinen Befehl, sondern den Weg zum eigenen Laufzeitverzeichnis.
        let unquotable = runtime_dir(&RuntimeDirFacts::Present {
            path: PathBuf::from("/tmp/it's"),
            mode: 0o755,
            owner_uid: 1000,
            our_uid: 4242,
            is_dir: true,
        });
        // Der fremde Eigentuemer kommt zuerst; deshalb hier ein eigener Fall
        // mit demselben Pfad und passender Kennung.
        let _ = unquotable;
        let unquotable = runtime_dir(&RuntimeDirFacts::Present {
            path: PathBuf::from("/tmp/it's"),
            mode: 0o755,
            owner_uid: 4242,
            our_uid: 4242,
            is_dir: true,
        });
        assert_eq!(
            unquotable.diagnostic().expect("a finding").fix,
            Some(FixAction::SetEnv {
                key: "XDG_RUNTIME_DIR".to_owned(),
                value: "/run/user/4242".to_owned(),
            })
        );
    }

    /// Ein Vorschlag, der den Zustand nur noch einmal anzeigt, ist keiner.
    ///
    /// `DOCTOR_003` hat keinen distributionsunabhaengigen Befehl, der etwas
    /// behebt — einen Kernel tauscht man nicht mit einer Zeile. `grep Seccomp
    /// /proc/self/status` und `uname -r` lesen bloss noch einmal nach, was in
    /// der Zeile schon steht. Der Vorschlag ist deshalb die Erklaerung.
    #[test]
    fn a_seccomp_finding_points_at_an_explanation_and_not_at_a_second_look() {
        let without = seccomp(&SeccompFacts {
            line: Reading::Found(SeccompLine::Missing),
            kernel_release: Reading::Found("6.1.0".to_owned()),
        });
        let old = seccomp(&SeccompFacts {
            line: Reading::Found(SeccompLine::Present("0".to_owned())),
            kernel_release: Reading::Found("4.19.0".to_owned()),
        });
        for outcome in [&without, &old] {
            let fix = outcome.diagnostic().expect("a finding").fix.clone();
            assert_eq!(
                fix,
                Some(FixAction::OpenUrl(
                    "https://man7.org/linux/man-pages/man2/seccomp.2.html".to_owned()
                )),
                "{}",
                outcome.evidence()
            );
        }
    }

    #[test]
    fn a_daemon_that_is_not_there_keeps_the_reason_it_gave() {
        let outcome = daemon(&DaemonFacts::Unreachable {
            socket: PathBuf::from("/run/user/1000/humanitl/daemon.sock"),
            diagnostic: Box::new(
                Diagnostic::builder(DAEMON_001, Severity::Blocking)
                    .why("the socket does not exist")
                    .build(),
            ),
        });
        assert_eq!(outcome.status(), CheckStatus::Warn);
        let diagnostic = outcome.diagnostic().expect("a finding");
        assert_eq!(diagnostic.code.as_str(), "DOCTOR_006");
        assert!(diagnostic.why.contains("DAEMON_001"), "{}", diagnostic.why);
        assert_eq!(diagnostic.fix, Some(FixAction::InstallService));
    }

    #[test]
    fn a_daemon_of_another_major_fails_and_one_of_the_same_is_green() {
        let other = daemon(&DaemonFacts::Reachable {
            socket: PathBuf::from("/run/user/1000/humanitl/daemon.sock"),
            version: "0.0.0".to_owned(),
            proto: (2, 0),
            expected_proto: (1, 4),
        });
        assert_eq!(other.status(), CheckStatus::Fail);

        let same = daemon(&DaemonFacts::Reachable {
            socket: PathBuf::from("/run/user/1000/humanitl/daemon.sock"),
            version: "0.0.0".to_owned(),
            proto: (1, 2),
            expected_proto: (1, 4),
        });
        assert_eq!(same.status(), CheckStatus::Ok);

        let untried = daemon(&DaemonFacts::NotTried {
            socket: PathBuf::from("/run/user/1000/humanitl/daemon.sock"),
            why: "the report came from the daemon itself".to_owned(),
        });
        assert!(untried.is_unmeasured());
    }

    #[test]
    fn an_agent_without_a_version_is_not_reported_as_a_version() {
        let outcome = agent(&AgentFacts::Found {
            adapter: "opencode".to_owned(),
            command: "opencode".to_owned(),
            program: PathBuf::from("/usr/local/bin/opencode"),
            version: Reading::Unreadable("exit 1".to_owned()),
        });
        assert!(outcome.is_unmeasured(), "{}", outcome.evidence());

        let found = agent(&AgentFacts::Found {
            adapter: "opencode".to_owned(),
            command: "opencode".to_owned(),
            program: PathBuf::from("/usr/local/bin/opencode"),
            version: Reading::Found("1.18.25".to_owned()),
        });
        assert_eq!(found.status(), CheckStatus::Ok);
        assert!(found.evidence().contains("1.18.25"));

        let missing = agent(&AgentFacts::Missing {
            adapter: "opencode".to_owned(),
            command: "opencode".to_owned(),
            searched: "/usr/bin".to_owned(),
            install: "curl -fsSL https://opencode.ai/install | bash".to_owned(),
        });
        assert_eq!(missing.status(), CheckStatus::Warn);
        assert_eq!(
            missing.diagnostic().expect("a finding").code.as_str(),
            "DOCTOR_007"
        );
    }

    #[test]
    fn the_llm_line_is_never_green_without_a_measurement() {
        let outcome = llm(&LlmFacts::NotContacted {
            endpoint: "http://192.168.1.50:11434".to_owned(),
            command: "humanitl doctor --probe-llm".to_owned(),
        });
        assert_eq!(outcome.status(), CheckStatus::Warn);
        let diagnostic = outcome.diagnostic().expect("a finding");
        assert_eq!(diagnostic.code.as_str(), "DOCTOR_013");
        assert!(
            diagnostic.why.contains("does not open a connection"),
            "{}",
            diagnostic.why
        );
        assert_eq!(
            diagnostic.fix,
            Some(FixAction::CopyCommand(
                "humanitl doctor --probe-llm".to_owned()
            ))
        );
    }

    #[test]
    fn a_measured_llm_is_green_and_a_silent_one_keeps_its_code() {
        let green = llm(&LlmFacts::Answered {
            endpoint: "http://192.168.1.50:11434".to_owned(),
            flavor: "ollama".to_owned(),
            models: 3,
            latency_ms: 12,
            diagnostics: Vec::new(),
        });
        assert_eq!(green.status(), CheckStatus::Ok);
        assert!(green.evidence().contains("3 models"));

        let silent = llm(&LlmFacts::Silent {
            endpoint: "http://192.168.1.50:11434".to_owned(),
            diagnostic: Box::new(
                Diagnostic::builder(LLM_001, Severity::Warning)
                    .why("no answer within 3000 ms")
                    .build(),
            ),
        });
        assert_eq!(silent.status(), CheckStatus::Warn);
        assert!(why_of(&silent).contains("LLM_001"), "{}", why_of(&silent));

        let noisy = llm(&LlmFacts::Answered {
            endpoint: "http://8.8.8.8:11434".to_owned(),
            flavor: "ollama".to_owned(),
            models: 1,
            latency_ms: 40,
            diagnostics: vec![
                Diagnostic::builder(LLM_001, Severity::Warning)
                    .why("not private")
                    .build(),
            ],
        });
        assert_eq!(noisy.status(), CheckStatus::Warn);
        assert!(noisy.evidence().contains("LLM_001"), "{}", noisy.evidence());
    }

    #[test]
    fn a_tray_that_was_not_searched_is_not_a_tray_that_is_missing() {
        let blind = tray(&TrayFacts {
            library: None,
            readable_dirs: 0,
            searched_dirs: 4,
            desktop: Some("KDE".to_owned()),
        });
        assert!(blind.is_unmeasured(), "{}", blind.evidence());

        let missing = tray(&TrayFacts {
            library: None,
            readable_dirs: 4,
            searched_dirs: 4,
            desktop: Some("KDE".to_owned()),
        });
        assert_eq!(missing.status(), CheckStatus::Warn);
        assert!(!missing.is_unmeasured());
        assert_eq!(
            missing.diagnostic().expect("a finding").code.as_str(),
            "DOCTOR_009"
        );
    }

    #[test]
    fn gnome_with_the_library_still_needs_the_extension() {
        let gnome = tray(&TrayFacts {
            library: Some(PathBuf::from(
                "/usr/lib/x86_64-linux-gnu/libayatana-appindicator3.so.1",
            )),
            readable_dirs: 4,
            searched_dirs: 4,
            desktop: Some("ubuntu:GNOME".to_owned()),
        });
        assert_eq!(gnome.status(), CheckStatus::Warn);
        assert!(why_of(&gnome).contains("GNOME"), "{}", why_of(&gnome));

        let kde = tray(&TrayFacts {
            library: Some(PathBuf::from(
                "/usr/lib/x86_64-linux-gnu/libayatana-appindicator3.so.1",
            )),
            readable_dirs: 4,
            searched_dirs: 4,
            desktop: Some("KDE".to_owned()),
        });
        assert_eq!(kde.status(), CheckStatus::Ok);
    }

    #[test]
    fn impeller_only_warns_where_the_combination_is_actually_there() {
        let bad = renderer(&RendererFacts {
            session_type: Some("wayland".to_owned()),
            nvidia: Reading::Found(true),
            flutter_engine: None,
        });
        assert_eq!(bad.status(), CheckStatus::Warn);
        assert_eq!(
            bad.diagnostic().expect("a finding").code.as_str(),
            "DOCTOR_010"
        );
        assert!(
            why_of(&bad).contains("--no-enable-impeller"),
            "{}",
            why_of(&bad)
        );

        let x11 = renderer(&RendererFacts {
            session_type: Some("x11".to_owned()),
            nvidia: Reading::Found(true),
            flutter_engine: None,
        });
        assert_eq!(x11.status(), CheckStatus::Ok);

        let no_card = renderer(&RendererFacts {
            session_type: None,
            nvidia: Reading::Found(false),
            flutter_engine: None,
        });
        assert_eq!(no_card.status(), CheckStatus::Ok);

        let blind = renderer(&RendererFacts {
            session_type: Some("wayland".to_owned()),
            nvidia: Reading::Unreadable("EACCES".to_owned()),
            flutter_engine: None,
        });
        assert!(blind.is_unmeasured(), "{}", blind.evidence());

        let half = renderer(&RendererFacts {
            session_type: None,
            nvidia: Reading::Found(true),
            flutter_engine: Some("/opt/engine".to_owned()),
        });
        assert!(half.is_unmeasured(), "{}", half.evidence());
        assert!(why_of(&half).contains("/opt/engine"), "{}", why_of(&half));
    }

    /// Die lange Kommandozeile, wie `doctor::probe` sie wirklich startet.
    fn long_userns_run() -> CommandRun {
        CommandRun::new(
            words(
                "/usr/bin/bwrap --unshare-user --ro-bind /usr /usr --ro-bind-try /lib /lib \
                 -- /bin/true",
            ),
            RunOutcome::Exited(1),
            "",
            "bwrap: setting up uid map: Permission denied",
        )
    }

    #[test]
    fn the_userns_evidence_names_the_program_and_not_its_twenty_arguments() {
        let outcome = userns(&UsernsFacts {
            probe: Reading::Found(long_userns_run()),
            apparmor_restrict: Reading::Found("1".to_owned()),
            userns_clone: Reading::Absent,
        });
        assert!(
            !outcome.evidence().contains("--ro-bind"),
            "the evidence is a table cell: {}",
            outcome.evidence()
        );
        assert!(
            outcome.evidence().contains("/usr/bin/bwrap --unshare-user"),
            "{}",
            outcome.evidence()
        );
        // Und der Grund auch nicht: Er wird auf dem Weg zur Oberfläche auf
        // `NOTE_MAX_CHARS` gekappt, und die ganze Zeile passte dort nicht
        // hinein.
        assert!(
            !why_of(&outcome).contains("--ro-bind"),
            "the why is capped at NOTE_MAX_CHARS: {}",
            why_of(&outcome)
        );
        // Bei AppArmor ist der bessere Vorschlag der Schalter, nicht der Aufruf.
        assert_eq!(
            outcome.diagnostic().expect("a finding").fix,
            Some(FixAction::CopyCommand(
                "sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0".to_owned()
            ))
        );
    }

    #[test]
    fn without_a_switch_to_flip_the_whole_call_is_handed_over_to_be_run_by_hand() {
        // Kein AppArmor, kein `unprivileged_userns_clone`: Dann gibt es nichts
        // umzulegen, und das Beste, was der Befund bieten kann, ist die Zeile,
        // mit der der Doctor gemessen hat.
        let outcome = userns(&UsernsFacts {
            probe: Reading::Found(long_userns_run()),
            apparmor_restrict: Reading::Found("0".to_owned()),
            userns_clone: Reading::Absent,
        });
        assert_eq!(
            outcome.diagnostic().expect("a finding").fix,
            Some(long_userns_run().fix())
        );
    }

    #[test]
    fn a_large_amount_of_free_space_is_shown_in_gibibytes() {
        let roomy = disk_space(&DiskFacts::Measured {
            path: PathBuf::from("/home/u/.local/share/humanitl"),
            available_bytes: 150 * 1024 * 1024 * 1024,
        });
        assert!(
            roomy.evidence().contains("150.0 GiB"),
            "{}",
            roomy.evidence()
        );
    }

    #[test]
    fn a_full_disk_warns_and_an_unreadable_one_is_not_a_full_one() {
        let full = disk_space(&DiskFacts::Measured {
            path: PathBuf::from("/home/u/.local/share/humanitl"),
            available_bytes: 100 * 1024 * 1024,
        });
        assert_eq!(full.status(), CheckStatus::Warn);
        assert_eq!(
            full.diagnostic().expect("a finding").code.as_str(),
            "DOCTOR_011"
        );
        assert!(full.evidence().contains("100.0 MiB"), "{}", full.evidence());

        let roomy = disk_space(&DiskFacts::Measured {
            path: PathBuf::from("/home/u/.local/share/humanitl"),
            available_bytes: 8 * 1024 * 1024 * 1024,
        });
        assert_eq!(roomy.status(), CheckStatus::Ok);

        let blind = disk_space(&DiskFacts::Unreadable {
            path: PathBuf::from("/home/u/.local/share/humanitl"),
            error: "ENOENT".to_owned(),
        });
        assert!(blind.is_unmeasured());
    }
}
