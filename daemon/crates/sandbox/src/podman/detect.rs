//! Ob Podman auf diesem Rechner eine Sandbox der gemeinsamen Stufe tragen
//! kann (HUM-433).
//!
//! Gefragt wird in dieser Reihenfolge, und die erste Lücke beendet die Suche
//! mit ihrem Code: das Programm (`SANDBOX_034`), seine Fassung
//! (`SANDBOX_037`), rootless und lokal (`SANDBOX_038`), die Laufzeit `crun`
//! (`SANDBOX_037`), die delegierten cgroup-Controller (`SANDBOX_040`), die
//! untergeordneten IDs (`SANDBOX_039`) und `SELinux` (`SANDBOX_044`). Jeder
//! Befund trägt ein `why` und einen kopierbaren `fix`; keiner fällt auf ein
//! anderes Backend zurück.
//!
//! Podman läuft dabei mit derselben Umgebung wie beim Start
//! ([`super::EngineDir::command`]): Was der Nutzer in seiner eigenen
//! `containers.conf` stehen hat, ändert die Antwort nicht.
//!
//! Die Deutung der Antworten steht in reinen Funktionen ([`parse_info`],
//! [`has_subordinate_ids`], [`missing_controllers`]); die Tests prüfen sie
//! ohne Podman.

use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;

use humanitl_config::{Env, PodmanConfig};
use humanitl_core::diagnostics::codes::{
    SANDBOX_034, SANDBOX_037, SANDBOX_038, SANDBOX_039, SANDBOX_040, SANDBOX_044,
};
use humanitl_core::{Diagnostic, FixAction, Severity};

use crate::bwrap::Version;
use crate::os_release::PackageManager;

/// Die kleinste Fassung von Podman, mit der der Adapter arbeitet: die von
/// Ubuntu 24.04 (HUM-292, Abschnitt 8).
pub const MIN_PODMAN_VERSION: Version = Version(4, 9, 0);

/// Die einzige Laufzeit der gemeinsamen Stufe.
pub(crate) const CRUN: &str = "crun";

/// Die Controller, ohne die `--memory`, `--cpus` und `--pids-limit` nichts
/// begrenzen.
pub(crate) const CONTROLLERS: [&str; 3] = ["memory", "pids", "cpu"];

/// Wo der Kern sagt, ob `SELinux` erzwingt.
const SELINUX_ENFORCE: &str = "/sys/fs/selinux/enforce";

/// Der Suchpfad, wenn die Umgebung keinen hat.
pub(crate) const DEFAULT_PATH: &str = "/usr/local/bin:/usr/bin:/bin";

/// Wie lange eine Frage an Podman höchstens dauern darf.
pub(crate) const QUERY_TIMEOUT: Duration = Duration::from_secs(30);

/// Was `podman info` über den Rechner sagt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HostInfo {
    /// Läuft Podman ohne Root?
    pub(crate) rootless: bool,
    /// Ist dieses Podman nur der Client eines entfernten Dienstes (eine
    /// Podman-Maschine, `podman-remote`)?
    pub(crate) remote: bool,
    /// Der Name der OCI-Laufzeit.
    pub(crate) runtime: String,
    /// `v1` oder `v2`.
    pub(crate) cgroups: String,
    /// Die Controller, die dem Nutzer delegiert sind.
    pub(crate) controllers: Vec<String>,
}

/// Die Schablone für `podman info --format`: fünf Felder, durch `|`
/// getrennt, die Controller als Go-Liste (`[cpu memory pids]`).
pub(crate) const INFO_FORMAT: &str = "{{.Host.Security.Rootless}}|{{.Host.ServiceIsRemote}}|\
{{.Host.OCIRuntime.Name}}|{{.Host.CgroupsVersion}}|{{.Host.CgroupControllers}}";

/// Liest die Antwort auf [`INFO_FORMAT`]; `None`, wenn sie diese Form nicht
/// hat.
pub(crate) fn parse_info(text: &str) -> Option<HostInfo> {
    let mut fields = text.trim().split('|');
    let rootless = parse_bool(fields.next()?)?;
    let remote = parse_bool(fields.next()?)?;
    let runtime = fields.next()?.trim().to_owned();
    let cgroups = fields.next()?.trim().to_owned();
    let controllers = fields
        .next()?
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    if fields.next().is_some() {
        return None;
    }
    Some(HostInfo {
        rootless,
        remote,
        runtime,
        cgroups,
        controllers,
    })
}

fn parse_bool(text: &str) -> Option<bool> {
    match text.trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// Die Controller aus [`CONTROLLERS`], die nicht delegiert sind.
pub(crate) fn missing_controllers(info: &HostInfo) -> Vec<&'static str> {
    if info.cgroups != "v2" {
        return CONTROLLERS.to_vec();
    }
    CONTROLLERS
        .into_iter()
        .filter(|wanted| !info.controllers.iter().any(|have| have == wanted))
        .collect()
}

/// Ob eine Zeile aus `/etc/subuid` oder `/etc/subgid` diesem Nutzer
/// mindestens 65536 IDs gibt, beim Namen oder bei der Nummer
/// (`name:start:count`).
pub(crate) fn has_subordinate_ids(text: &str, user: Option<&str>, id: u32) -> bool {
    let id = id.to_string();
    text.lines().any(|line| {
        let mut fields = line.trim().split(':');
        let (Some(owner), Some(_start), Some(count)) =
            (fields.next(), fields.next(), fields.next())
        else {
            return false;
        };
        let ours = owner == id || user.is_some_and(|name| !name.is_empty() && owner == name);
        ours && count
            .trim()
            .parse::<u64>()
            .is_ok_and(|count| count >= 65_536)
    })
}

/// Das Programm `podman`: aus `sandbox.podman.program`, sonst das erste im
/// `PATH` der Umgebung des Daemons.
///
/// # Errors
///
/// `SANDBOX_034` mit dem Installationsbefehl, wenn es keines gibt.
pub(crate) fn find_program(config: &PodmanConfig, env: &Env) -> Result<PathBuf, Diagnostic> {
    if let Some(program) = config.program.as_deref() {
        if crate::agent::is_executable_file(program) {
            return Ok(program.to_path_buf());
        }
        return Err(missing(&format!(
            "sandbox.podman.program = {:?} is not an executable file",
            program.display().to_string()
        )));
    }
    let path = env.non_empty("PATH").unwrap_or(DEFAULT_PATH);
    std::env::split_paths(path)
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| dir.join(super::PROGRAM))
        .find(|candidate| crate::agent::is_executable_file(candidate))
        .ok_or_else(|| missing(&format!("there is no executable podman in PATH={path}")))
}

/// `SANDBOX_034`: Podman fehlt.
fn missing(why: &str) -> Diagnostic {
    Diagnostic::builder(SANDBOX_034, Severity::Blocking)
        .why(format!(
            "sandbox.backend = \"podman\" needs Podman, and {why}; the start does not switch to \
             another backend on its own. Install Podman, or start with --backend bwrap"
        ))
        .fix(FixAction::CopyCommand(install_command().to_owned()))
        .build()
}

/// Der Befehl, der Podman mit `crun` und den Werkzeugen für untergeordnete
/// IDs nachinstalliert, für den Paketverwalter dieses Rechners.
pub(crate) const fn install_command_for(manager: PackageManager) -> &'static str {
    match manager {
        PackageManager::Apt => "sudo apt install podman crun uidmap",
        PackageManager::Dnf => "sudo dnf install podman crun shadow-utils",
        PackageManager::Pacman => "sudo pacman -S podman crun",
        PackageManager::Zypper => "sudo zypper install podman crun",
    }
}

/// [`install_command_for`] auf diesem Rechner.
pub(crate) fn install_command() -> &'static str {
    install_command_for(PackageManager::detect())
}

/// Die Fassung aus `podman version --format {{.Client.Version}}`.
///
/// # Errors
///
/// `SANDBOX_034`, wenn Podman nicht antwortet; `SANDBOX_037` unter
/// [`MIN_PODMAN_VERSION`].
pub(crate) fn check_version(
    program: &Path,
    output: &std::io::Result<Output>,
) -> Result<Version, Diagnostic> {
    let output = match output {
        Ok(output) if output.status.success() => output,
        Ok(output) => {
            return Err(missing(&format!(
                "{} version ended with {}: {}",
                program.display(),
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        Err(err) => {
            return Err(missing(&format!(
                "{} cannot be run: {err}",
                program.display()
            )));
        }
    };
    let version = Version::parse(String::from_utf8_lossy(&output.stdout).trim());
    if version < MIN_PODMAN_VERSION {
        return Err(Diagnostic::builder(SANDBOX_037, Severity::Blocking)
            .why(format!(
                "{} is Podman {version}; the adapter needs at least {MIN_PODMAN_VERSION}",
                program.display()
            ))
            .fix(FixAction::CopyCommand(install_command().to_owned()))
            .build());
    }
    Ok(version)
}

/// Deutet die Antwort von `podman info`.
///
/// # Errors
///
/// `SANDBOX_038`, wenn Podman nicht rootless oder nur ein entfernter Client
/// ist; `SANDBOX_037`, wenn die Laufzeit nicht `crun` ist oder die Antwort
/// nicht lesbar; `SANDBOX_040`, wenn ein Controller nicht delegiert ist.
pub(crate) fn check_info(
    program: &Path,
    output: &std::io::Result<Output>,
) -> Result<HostInfo, Diagnostic> {
    let text = match output {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).into_owned()
        }
        Ok(output) => {
            return Err(runtime_missing(&format!(
                "{} info ended with {}: {}",
                program.display(),
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        Err(err) => {
            return Err(runtime_missing(&format!(
                "{} info cannot be run: {err}",
                program.display()
            )));
        }
    };
    let info = parse_info(&text)
        .ok_or_else(|| runtime_missing(&format!("podman info answered {:?}", text.trim())))?;
    if info.remote || !info.rootless {
        let what = if info.remote {
            "a client of a remote Podman service (a Podman machine or podman-remote)"
        } else {
            "running as root"
        };
        return Err(Diagnostic::builder(SANDBOX_038, Severity::Blocking)
            .why(format!(
                "{} is {what}; the sandbox needs rootless Podman on this machine, started by the \
                 daemon itself, so that the container sees the same files and the descriptors the \
                 daemon hands over",
                program.display()
            ))
            .fix(FixAction::ChangeSetting {
                key: "sandbox.backend".to_owned(),
                value: crate::AUTO_BACKEND.to_owned(),
            })
            .build());
    }
    if info.runtime != CRUN {
        return Err(runtime_missing(&format!(
            "the OCI runtime is {:?}",
            info.runtime
        )));
    }
    let missing = missing_controllers(&info);
    if !missing.is_empty() {
        return Err(Diagnostic::builder(SANDBOX_040, Severity::Blocking)
            .why(format!(
                "the cgroup controllers {} are not delegated to this user (cgroups {}, delegated: \
                 [{}]); --memory, --cpus and --pids-limit would not hold, and a sandbox without \
                 its limits does not start",
                missing.join(", "),
                info.cgroups,
                info.controllers.join(" ")
            ))
            .fix(FixAction::CopyCommand(DELEGATE_COMMAND.to_owned()))
            .docs("https://github.com/containers/podman/blob/main/troubleshooting.md")
            .build());
    }
    Ok(info)
}

/// Der systemd-Eintrag, der die drei Controller an die Sitzungen der Nutzer
/// delegiert.
const DELEGATE_COMMAND: &str = "sudo mkdir -p /etc/systemd/system/user@.service.d && printf \
     '[Service]\\nDelegate=cpu cpuset io memory pids\\n' | sudo tee \
     /etc/systemd/system/user@.service.d/delegate.conf && sudo systemctl daemon-reload";

/// `SANDBOX_037`: Die Laufzeit ist nicht `crun`, oder Podman sagt nichts
/// Lesbares über sie.
fn runtime_missing(why: &str) -> Diagnostic {
    Diagnostic::builder(SANDBOX_037, Severity::Blocking)
        .why(format!(
            "{why}; the sandbox runs under the OCI runtime crun, which runc does not replace in \
             rootless mode"
        ))
        .fix(FixAction::CopyCommand(install_command().to_owned()))
        .build()
}

/// Das Konto, für das `/etc/subuid` und `/etc/subgid` gelten: die Kennung
/// dieses Prozesses und ihr Name.
///
/// Der Name kommt aus der Kennung, nie aus `USER` oder `LOGNAME`: Die
/// Umgebung kann einen anderen Nutzer nennen, und `newuidmap` prüft die
/// Bereiche des Kontos, unter dem der Prozess läuft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Account {
    /// Die Kennung.
    pub(crate) uid: u32,
    /// Der Name, wenn `/etc/passwd` oder `getent` ihn kennt.
    pub(crate) name: Option<String>,
}

impl Account {
    /// Das Konto dieses Prozesses: der Name aus `/etc/passwd`, sonst aus
    /// `getent passwd <uid>` (NSS, etwa LDAP), begrenzt.
    pub(crate) fn of_this_process() -> Self {
        let uid = rustix::process::getuid().as_raw();
        let passwd = std::fs::read_to_string("/etc/passwd").unwrap_or_default();
        let name = name_of(&passwd, uid).or_else(|| {
            let mut getent = std::process::Command::new("getent");
            getent.env_clear().args(["passwd", &uid.to_string()]);
            let output = super::teardown::run_bounded(getent, Duration::from_secs(5)).ok()?;
            output
                .status
                .success()
                .then(|| name_of(&String::from_utf8_lossy(&output.stdout), uid))
                .flatten()
        });
        Self { uid, name }
    }
}

/// Der Name zur Kennung `uid` in Zeilen im Format von `/etc/passwd`.
pub(crate) fn name_of(passwd: &str, uid: u32) -> Option<String> {
    passwd.lines().find_map(|line| {
        let mut fields = line.split(':');
        let name = fields.next()?;
        let id: u32 = fields.nth(1)?.parse().ok()?;
        (id == uid && !name.is_empty()).then(|| name.to_owned())
    })
}

/// Die Werkzeuge für untergeordnete IDs, die im `PATH` und in `/usr/bin`
/// fehlen.
pub(crate) fn missing_id_tools(env: &Env) -> Vec<&'static str> {
    let path = env.non_empty("PATH").unwrap_or(DEFAULT_PATH);
    ["newuidmap", "newgidmap"]
        .into_iter()
        .filter(|tool| {
            !std::env::split_paths(path)
                .chain([PathBuf::from("/usr/bin")])
                .any(|dir| dir.join(tool).is_file())
        })
        .collect()
}

/// Der Befehl, der `newuidmap` und `newgidmap` nachinstalliert.
pub(crate) const fn install_id_tools_for(manager: PackageManager) -> &'static str {
    match manager {
        PackageManager::Apt => "sudo apt install uidmap",
        PackageManager::Dnf => "sudo dnf install shadow-utils",
        PackageManager::Pacman => "sudo pacman -S shadow",
        PackageManager::Zypper => "sudo zypper install shadow",
    }
}

/// Die untergeordneten IDs für `--userns keep-id`, für das Konto dieses
/// Prozesses (beide Dateien sind nach dem Konto geschlüsselt, beim Namen
/// oder bei der Kennung).
///
/// # Errors
///
/// `SANDBOX_039`, wenn `/etc/subuid` oder `/etc/subgid` dem Konto keine
/// 65536 IDs geben oder `newuidmap` und `newgidmap` fehlen. Der Fix nennt,
/// was fehlt: den Installationsbefehl, `usermod`, oder beide mit `&&`.
pub(crate) fn check_subordinate_ids(
    account: &Account,
    user_ids: &str,
    group_ids: &str,
    missing_tools: &[&str],
    manager: PackageManager,
) -> Result<(), Diagnostic> {
    let name = account.name.as_deref();
    let mut gaps = Vec::new();
    if !has_subordinate_ids(user_ids, name, account.uid) {
        gaps.push("no entry with 65536 IDs in /etc/subuid".to_owned());
    }
    if !has_subordinate_ids(group_ids, name, account.uid) {
        gaps.push("no entry with 65536 IDs in /etc/subgid".to_owned());
    }
    let ranges_missing = !gaps.is_empty();
    if !missing_tools.is_empty() {
        gaps.push(format!("{} missing", missing_tools.join(" and ")));
    }
    if gaps.is_empty() {
        return Ok(());
    }
    let login = name.map_or_else(|| "\"$(id -un)\"".to_owned(), str::to_owned);
    let usermod =
        format!("sudo usermod --add-subuids 100000-165535 --add-subgids 100000-165535 {login}");
    let install = install_id_tools_for(manager);
    let fix = match (ranges_missing, missing_tools.is_empty()) {
        (true, true) => usermod,
        (false, _) => install.to_owned(),
        (true, false) => format!("{install} && {usermod}"),
    };
    let mut why = format!(
        "--userns keep-id needs subordinate IDs for the account {} (uid {}): {}",
        name.unwrap_or("without a name"),
        account.uid,
        gaps.join("; ")
    );
    if ranges_missing {
        why.push_str(
            "; the range 100000-165535 in the fix may overlap the range of another account in \
             /etc/subuid or /etc/subgid, choose a free one then",
        );
    }
    Err(Diagnostic::builder(SANDBOX_039, Severity::Blocking)
        .why(why)
        .fix(FixAction::CopyCommand(fix))
        .build())
}

/// `SELinux`, wenn es erzwingt.
///
/// # Errors
///
/// `SANDBOX_044`: Ein Einhängen des Projekts verlangte, es umzuetikettieren
/// (`:z`), und das änderte die Dateien des Nutzers. Bis es dafür einen
/// Entwurf gibt, startet Podman auf solchen Rechnern nicht.
pub(crate) fn check_selinux(enforce: Option<&str>) -> Result<(), Diagnostic> {
    if enforce.is_some_and(|text| text.trim() == "1") {
        return Err(Diagnostic::builder(SANDBOX_044, Severity::Blocking)
            .why(
                "SELinux enforces labels on this machine; binding the project into a container \
                 would need relabelling it (:z), which changes the user's files, so the Podman \
                 backend does not start here",
            )
            .fix(FixAction::ChangeSetting {
                key: "sandbox.backend".to_owned(),
                value: crate::AUTO_BACKEND.to_owned(),
            })
            .build());
    }
    Ok(())
}

/// Liest `/sys/fs/selinux/enforce`; `None` ohne `SELinux`.
pub(crate) fn selinux_enforce() -> Option<String> {
    std::fs::read_to_string(SELINUX_ENFORCE).ok()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn the_info_line_is_read_field_by_field() {
        let info = parse_info("true|false|crun|v2|[cpu memory pids]\n").expect("five fields");
        assert!(info.rootless && !info.remote);
        assert_eq!(info.runtime, "crun");
        assert_eq!(info.controllers, ["cpu", "memory", "pids"]);
        assert!(missing_controllers(&info).is_empty());
        for broken in [
            "",
            "true|false|crun|v2",
            "yes|no|crun|v2|[]",
            "true|false|crun|v2|[]|x",
        ] {
            assert!(parse_info(broken).is_none(), "{broken:?}");
        }
    }

    #[test]
    fn a_controller_that_is_not_delegated_is_named() {
        let mut info = parse_info("true|false|crun|v2|[cpu pids]").unwrap();
        assert_eq!(missing_controllers(&info), ["memory"]);
        info.cgroups = "v1".to_owned();
        assert_eq!(missing_controllers(&info), CONTROLLERS);
        let err = check_info(
            Path::new("podman"),
            &Ok(ok_output("true|false|crun|v2|[cpu]")),
        )
        .expect_err("two missing");
        assert_eq!(err.code.as_str(), "SANDBOX_040", "{}", err.why);
        assert!(err.why.contains("memory, pids"), "{}", err.why);
        assert!(matches!(err.fix, Some(FixAction::CopyCommand(_))));
    }

    #[test]
    fn root_remote_and_runc_are_refused_with_their_code() {
        for (line, code) in [
            ("false|false|crun|v2|[cpu memory pids]", "SANDBOX_038"),
            ("true|true|crun|v2|[cpu memory pids]", "SANDBOX_038"),
            ("true|false|runc|v2|[cpu memory pids]", "SANDBOX_037"),
            ("garbage", "SANDBOX_037"),
        ] {
            let err = check_info(Path::new("podman"), &Ok(ok_output(line))).expect_err(line);
            assert_eq!(err.code.as_str(), code, "{line}: {}", err.why);
            assert!(err.fix.is_some(), "{line}");
        }
        check_info(
            Path::new("podman"),
            &Ok(ok_output("true|false|crun|v2|[cpuset cpu io memory pids]")),
        )
        .expect("all there");
    }

    #[test]
    fn a_version_below_four_nine_is_sandbox_037() {
        let err = check_version(Path::new("podman"), &Ok(ok_output("4.8.3\n"))).expect_err("old");
        assert_eq!(err.code.as_str(), "SANDBOX_037", "{}", err.why);
        assert_eq!(
            check_version(Path::new("podman"), &Ok(ok_output("4.9.3\n"))).unwrap(),
            Version(4, 9, 3)
        );
        assert_eq!(
            check_version(Path::new("podman"), &Ok(ok_output("5.8.6"))).unwrap(),
            Version(5, 8, 6)
        );
        let err = check_version(
            Path::new("/nowhere/podman"),
            &Err(std::io::Error::from(std::io::ErrorKind::NotFound)),
        )
        .expect_err("cannot run");
        assert_eq!(err.code.as_str(), "SANDBOX_034", "{}", err.why);
    }

    #[test]
    fn subordinate_ids_match_by_name_or_number_with_enough_ids() {
        let text = "root:100000:65536\nalice:165536:65536\n1000:231072:1000\n";
        assert!(has_subordinate_ids(text, Some("alice"), 1001));
        assert!(
            !has_subordinate_ids(text, Some("bob"), 1000),
            "1000 has too few"
        );
        assert!(!has_subordinate_ids("", Some("alice"), 1000));
        assert!(has_subordinate_ids("1000:100000:65536", None, 1000));
        assert!(!has_subordinate_ids("alice:x:y", Some("alice"), 1));
    }

    fn fix_of(err: &Diagnostic) -> String {
        match &err.fix {
            Some(FixAction::CopyCommand(command)) => command.clone(),
            other => panic!("{other:?}"),
        }
    }

    /// Der Fix nennt genau, was fehlt: `usermod`, das Paket der Werkzeuge,
    /// oder beides mit `&&`.
    #[test]
    fn missing_subordinate_ids_are_sandbox_039_with_the_fitting_fix() {
        let account = Account {
            uid: 1234,
            name: Some("alice".to_owned()),
        };
        let ranges = "alice:100000:65536\n";
        check_subordinate_ids(&account, ranges, ranges, &[], PackageManager::Apt)
            .expect("all there");
        let err = check_subordinate_ids(&account, "", "", &[], PackageManager::Apt)
            .expect_err("no ranges");
        assert_eq!(err.code.as_str(), "SANDBOX_039", "{}", err.why);
        assert!(err.why.contains("/etc/subuid") && err.why.contains("/etc/subgid"));
        assert!(err.why.contains("overlap"), "{}", err.why);
        assert!(fix_of(&err).starts_with("sudo usermod") && fix_of(&err).ends_with(" alice"));
        let err = check_subordinate_ids(
            &account,
            ranges,
            ranges,
            &["newuidmap"],
            PackageManager::Apt,
        )
        .expect_err("no helper");
        assert_eq!(fix_of(&err), "sudo apt install uidmap");
        assert!(!err.why.contains("overlap"), "{}", err.why);
        let err = check_subordinate_ids(&account, "", ranges, &["newgidmap"], PackageManager::Dnf)
            .expect_err("both");
        assert!(
            fix_of(&err).starts_with("sudo dnf install shadow-utils && sudo usermod"),
            "{}",
            fix_of(&err)
        );
        // Die Gruppe zählt beim Konto, nicht bei der Kennung der Gruppe.
        let numeric = Account {
            uid: 1234,
            name: None,
        };
        check_subordinate_ids(
            &numeric,
            "1234:100000:65536",
            "1234:100000:65536",
            &[],
            PackageManager::Apt,
        )
        .expect("by number");
        let err =
            check_subordinate_ids(&numeric, "", "", &[], PackageManager::Apt).expect_err("none");
        assert!(fix_of(&err).ends_with("\"$(id -un)\""), "{}", fix_of(&err));
    }

    /// Der Name kommt aus der Kennung, nicht aus `USER`.
    #[test]
    fn the_account_name_comes_from_the_uid() {
        let passwd = "root:x:0:0::/root:/bin/sh\nalice:x:1234:1234::/home/alice:/bin/sh\n";
        assert_eq!(name_of(passwd, 1234).as_deref(), Some("alice"));
        assert_eq!(name_of(passwd, 99), None);
        for manager in PackageManager::ALL {
            assert!(install_id_tools_for(manager).starts_with("sudo "));
        }
    }

    #[test]
    fn enforcing_selinux_is_sandbox_044() {
        let err = check_selinux(Some("1\n")).expect_err("enforcing");
        assert_eq!(err.code.as_str(), "SANDBOX_044", "{}", err.why);
        check_selinux(Some("0\n")).expect("permissive");
        check_selinux(None).expect("no SELinux");
    }

    #[test]
    fn a_missing_program_is_sandbox_034_with_the_install_command() {
        let dir = tempfile::tempdir().expect("tempdir");
        let env = Env::from_pairs([("PATH", dir.path().to_string_lossy().into_owned())]);
        let err = find_program(&PodmanConfig::default(), &env).expect_err("no podman");
        assert_eq!(err.code.as_str(), "SANDBOX_034", "{}", err.why);
        assert!(err.why.contains("--backend bwrap"), "{}", err.why);
        for manager in PackageManager::ALL {
            assert!(install_command_for(manager).contains("podman"));
        }
        let config = PodmanConfig {
            program: Some(dir.path().join("podman")),
            ..PodmanConfig::default()
        };
        let err = find_program(&config, &env).expect_err("not executable");
        assert_eq!(err.code.as_str(), "SANDBOX_034", "{}", err.why);
    }

    fn ok_output(stdout: &str) -> Output {
        use std::os::unix::process::ExitStatusExt as _;
        Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        }
    }
}
