//! Programme als vorübergehende Unit des Dienstverwalters des Nutzers
//! (HUM-434, ADR-0010 Nachtrag).
//!
//! Unter seiner gehärteten Nutzer-Unit läuft der Daemon in einem eigenen
//! Nutzer-Namensraum (`PrivateUsers=yes`, und jede Eigenschaft mit eigenem
//! Mount-Namensraum zieht in einer Nutzer-Unit einen nach sich). Dort kann
//! `newuidmap` keine untergeordneten Kennungen abbilden (HUM-292 Messung P8:
//! `write to uid_map failed`), und eine Container-Engine ohne Root startet
//! nicht. Statt die Härtung zu lockern, startet der Daemon den Client der
//! Engine über den Dienstverwalter des Nutzers als vorübergehende Unit, die
//! außerhalb seines Namensraums läuft, mit `systemd-run --user`.
//!
//! **Feste Zeile, geprüfte Werte.** Jede Unit heißt `humanitl-<kennung>`
//! ([`UNIT_PREFIX`], [`UnitName`]); einen anderen Namen baut dieser Code
//! nicht. Die Zeile von `systemd-run` ist fest ([`UserUnits::argv`]):
//! `--pipe --wait` reichen die drei Standardströme des Daemons durch und
//! warten auf das Ende, mit dem Exit-Status des Programms;
//! `--collect` räumt die Unit danach weg, auch nach einem Fehlschlag;
//! `--expand-environment=no` (systemd 254) lässt `$` in Pfaden stehen;
//! `KillMode=mixed` und `TimeoutStopSec` gleich [`crate::KILL_GRACE`]
//! beenden sie beim Stopp wie der Daemon eine Sandbox. `Delegate=yes`
//! gibt der Engine die cgroup-Controller, die der Nutzer hat: Ohne meldet
//! sie in einer transienten Unit nur `memory pids`, und eine Sandbox mit
//! `--cpus` startet nicht (gemessen 2026-11-13, systemd 262, `SANDBOX_040`). Eine Sitzung hat
//! keine Laufzeitgrenze; eine Frage an die Engine bekommt `RuntimeMaxSec`.
//! Gestartet wird das Programm über `/usr/bin/env -i` mit genau der
//! Umgebung, die der Daemon ihm gibt: Die Umgebung des Dienstverwalters
//! (alles, was `systemctl --user import-environment` je hineintrug)
//! erreicht es nicht. Jedes Argument muss UTF-8 sein, weil die Zeile als
//! D-Bus-Zeichenkette reist. Bezeichner mit `%` ersetzt systemd in den
//! Argumenten einer vorübergehenden Unit nicht (gemessen 2026-11-13,
//! systemd 262: `systemd-run --user --pipe --wait --expand-environment=no
//! /bin/echo 'x%ny' 'p%%q'` gibt `x%ny p%%q` aus); maskiert wird deshalb
//! nichts.
//!
//! **Wann.** Nur wenn der Daemon nicht im ersten Nutzer-Namensraum läuft
//! ([`needs_user_manager`]), also unter seiner Unit. Aus einem Terminal, in
//! der CI und in den Tests startet der Client weiter als Kind
//! ([`ClientStart::Direct`]). Keine Unit bleibt nach dem Abmelden: Der
//! Daemon schaltet kein `linger` ein.
//!
//! **Was ein übernommener Daemon dadurch gewinnt**: Er kann vorübergehende
//! Units des Nutzers starten, also Programme außerhalb seiner Härtung. Das
//! steht in `docs/THREAT-MODEL.md`; dieser Code begrenzt es auf Units mit
//! dem Präfix, aber die Grenze hält nur gegen Fehler, nicht gegen einen
//! übernommenen Prozess.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use humanitl_core::diagnostics::codes::SANDBOX_050;
use humanitl_core::{Diagnostic, FixAction, Severity};

/// Der Anfang jedes Unit-Namens, den der Daemon startet.
pub(crate) const UNIT_PREFIX: &str = "humanitl-";

/// Das Programm, das die Units startet.
pub(crate) const RUN_PROGRAM: &str = "systemd-run";

/// Das Programm, das sie stoppt und den Dienstverwalter befragt.
pub(crate) const CTL_PROGRAM: &str = "systemctl";

/// Das Programm vor dem eigentlichen, das die Umgebung leert.
pub(crate) const ENV_PROGRAM: &str = "/usr/bin/env";

/// Ab dieser Fassung kennt `systemd-run` `--expand-environment=no`.
pub(crate) const MIN_SYSTEMD_VERSION: u32 = 254;

/// So lang darf ein Name höchstens sein (systemd erlaubt 255 mit Endung).
const MAX_NAME: usize = 200;

/// So viel länger als ihre Frist darf eine Frage an die Engine als Unit
/// leben, bevor der Dienstverwalter sie selbst beendet.
const RUNTIME_SLACK: Duration = Duration::from_secs(5);

/// Der Befehl, mit dem ein Mensch nachsieht, was der Dienstverwalter sagt.
const STATUS_COMMAND: &str = "systemctl --user status";

/// Wie der Client einer Engine startet.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum ClientStart {
    /// Als Kind des Daemons, mit geerbten Deskriptoren.
    #[default]
    Direct,
    /// Als vorübergehende Unit des Dienstverwalters des Nutzers.
    UserUnit(UserUnits),
}

/// Wie lange eine Unit leben darf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lifetime {
    /// Eine Sitzung: so lange sie läuft.
    Session,
    /// Eine Frage an die Engine: höchstens so lange, und etwas Luft.
    Bounded(Duration),
}

/// Der Name einer Unit, geprüft: `humanitl-`, dann `a-z`, `0-9` und `-`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnitName(String);

impl UnitName {
    /// Prüft `name`.
    ///
    /// # Errors
    ///
    /// Ein Satz, wenn der Name nicht mit [`UNIT_PREFIX`] beginnt, danach
    /// nichts kommt, er zu lang ist oder ein anderes Zeichen trägt.
    pub(crate) fn new(name: &str) -> Result<Self, String> {
        let Some(rest) = name.strip_prefix(UNIT_PREFIX) else {
            return Err(format!(
                "the unit name {name:?} does not begin with {UNIT_PREFIX:?}"
            ));
        };
        let fits = !rest.is_empty()
            && name.len() <= MAX_NAME
            && name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if fits {
            Ok(Self(name.to_owned()))
        } else {
            Err(format!(
                "the unit name {name:?} may only hold a-z, 0-9 and - after {UNIT_PREFIX:?}, \
                 at most {MAX_NAME} characters"
            ))
        }
    }

    /// Der Name mit der Endung `.service`.
    pub(crate) fn service(&self) -> String {
        format!("{}.service", self.0)
    }

    /// Der Name ohne Endung.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// Wo `systemd-run` und `systemctl` liegen und womit sie den
/// Dienstverwalter des Nutzers finden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UserUnits {
    run: PathBuf,
    ctl: PathBuf,
    /// Die ganze Umgebung von `systemd-run` und `systemctl`: das
    /// Laufzeitverzeichnis des Nutzers, sein Sitzungsbus, `PATH`.
    env: Vec<(String, String)>,
    /// Der Anfang jedes Namens: [`UNIT_PREFIX`], in den Live-Tests
    /// `humanitl-test-`.
    prefix: String,
}

impl UserUnits {
    /// Sucht beide Programme in `path` (der Suchpfad wie in `PATH`).
    ///
    /// # Errors
    ///
    /// `SANDBOX_050`, wenn eines fehlt.
    pub(crate) fn find(path: &str, env: Vec<(String, String)>) -> Result<Self, Diagnostic> {
        let lookup = |name: &str| {
            std::env::split_paths(path)
                .filter(|dir| dir.is_absolute())
                .map(|dir| dir.join(name))
                .find(|candidate| crate::agent::is_executable_file(candidate))
                .ok_or_else(|| {
                    unreachable_manager(&format!("there is no executable {name} in PATH={path}"))
                })
        };
        Ok(Self {
            run: lookup(RUN_PROGRAM)?,
            ctl: lookup(CTL_PROGRAM)?,
            env,
            prefix: UNIT_PREFIX.to_owned(),
        })
    }

    /// Ohne Suche, mit festen Pfaden: für Tests, die die Zeile prüfen.
    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn fixed(run: &Path, ctl: &Path, env: Vec<(String, String)>) -> Self {
        Self {
            run: run.to_path_buf(),
            ctl: ctl.to_path_buf(),
            env,
            prefix: UNIT_PREFIX.to_owned(),
        }
    }

    /// Dieselben Programme mit einem anderen Anfang der Namen, der selbst mit
    /// [`UNIT_PREFIX`] beginnt (für die Live-Tests: `humanitl-test-`).
    #[cfg(test)]
    pub(crate) fn with_prefix(mut self, prefix: &str) -> Self {
        assert!(prefix.starts_with(UNIT_PREFIX), "{prefix}");
        prefix.clone_into(&mut self.prefix);
        self
    }

    /// `systemd-run`, aufgelöst.
    pub(crate) fn run_program(&self) -> &Path {
        &self.run
    }

    /// Die Umgebung von `systemd-run` und `systemctl`.
    pub(crate) fn own_env(&self) -> &[(String, String)] {
        &self.env
    }

    /// Die Unit einer Sitzung: Präfix und Kennung.
    ///
    /// # Errors
    ///
    /// Der Satz von [`UnitName::new`].
    pub(crate) fn named(&self, id: &str) -> Result<UnitName, String> {
        UnitName::new(&format!("{}{id}", self.prefix))
    }

    /// Eine neue Unit für eine Frage an die Engine.
    pub(crate) fn query_unit(&self) -> UnitName {
        let id = uuid::Uuid::now_v7().simple().to_string();
        // Präfix und 32 Hex-Ziffern: immer gültig, solange es das Präfix ist.
        UnitName(format!("{}q-{id}", self.prefix))
    }

    /// Die Zeile, die `program` mit `args` und genau `env` als Unit `unit`
    /// startet; `argv[0]` ist `systemd-run`.
    ///
    /// # Errors
    ///
    /// Ein Satz, wenn ein Argument oder Wert kein UTF-8 ist, ein Schlüssel
    /// anderes als `A-Z`, `0-9` und `_` trägt, oder das Programm kein
    /// absoluter Pfad ohne `=` ist (`env` hielte es sonst für eine
    /// Zuweisung).
    pub(crate) fn argv(
        &self,
        unit: &UnitName,
        lifetime: Lifetime,
        program: &Path,
        args: &[OsString],
        env: &[(String, String)],
    ) -> Result<Vec<OsString>, String> {
        let mut line: Vec<OsString> = vec![self.run.clone().into_os_string()];
        let mut push = |arg: String| line.push(OsString::from(arg));
        for flag in FIXED_FLAGS {
            push((*flag).to_owned());
        }
        push(format!(
            "--property=TimeoutStopSec={}s",
            crate::KILL_GRACE.as_secs()
        ));
        if let Lifetime::Bounded(limit) = lifetime {
            push(format!(
                "--property=RuntimeMaxSec={}s",
                limit.saturating_add(RUNTIME_SLACK).as_secs().max(1)
            ));
        }
        push(format!("--unit={}", unit.service()));
        push("--".to_owned());
        push(ENV_PROGRAM.to_owned());
        push("-i".to_owned());
        push("--".to_owned());
        for (key, value) in env {
            let valid = !key.is_empty()
                && key
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_');
            if !valid {
                return Err(format!("the variable name {key:?} cannot go into the unit"));
            }
            push(format!("{key}={value}"));
        }
        let text = utf8(program.as_os_str())?;
        if !program.is_absolute() || text.contains('=') {
            return Err(format!(
                "the program {text:?} must be an absolute path without '=' to start as a unit"
            ));
        }
        push(text.to_owned());
        for arg in args {
            push(utf8(arg)?.to_owned());
        }
        Ok(line)
    }

    /// `command` als Unit: Programm, Argumente und die gesetzte Umgebung von
    /// `command` (es muss mit `env_clear` gebaut sein) wandern in die Zeile
    /// von [`Self::argv`]; `systemd-run` selbst startet mit [`Self::own_env`].
    ///
    /// # Errors
    ///
    /// `InvalidInput` mit dem Satz von [`Self::argv`].
    pub(crate) fn wrap(
        &self,
        command: &Command,
        unit: &UnitName,
        lifetime: Lifetime,
    ) -> std::io::Result<Command> {
        let env: Vec<(String, String)> = command
            .get_envs()
            .filter_map(|(key, value)| Some((key, value?)))
            .map(|(key, value)| Ok((utf8(key)?.to_owned(), utf8(value)?.to_owned())))
            .collect::<Result<_, String>>()
            .map_err(invalid)?;
        let engine_args: Vec<OsString> = command.get_args().map(OsStr::to_owned).collect();
        let line = self
            .argv(
                unit,
                lifetime,
                Path::new(command.get_program()),
                &engine_args,
                &env,
            )
            .map_err(invalid)?;
        let mut wrapped = Command::new(&self.run);
        wrapped
            .env_clear()
            .envs(self.env.iter().map(|(key, value)| (key, value)))
            .args(&line[1..]);
        Ok(wrapped)
    }

    /// `systemctl --user stop <unit>`: mit `wait` wartet es, bis die Unit
    /// gestoppt ist, sonst (`--no-block`) stößt es den Stopp nur an, und
    /// `KillMode` und `TimeoutStopSec` der Unit tun den Rest.
    pub(crate) fn stop_command(&self, unit: &UnitName, wait: bool) -> Command {
        let mut command = Command::new(&self.ctl);
        command
            .env_clear()
            .envs(self.env.iter().map(|(key, value)| (key, value)))
            .args(["--user", "stop"]);
        if !wait {
            command.arg("--no-block");
        }
        command.arg("--").arg(unit.service());
        command
    }

    /// `systemctl --user show --property=Version --value`: Antwortet der
    /// Dienstverwalter, und in welcher Fassung ([`check_probe`]).
    pub(crate) fn probe_command(&self) -> Command {
        let mut command = Command::new(&self.ctl);
        command
            .env_clear()
            .envs(self.env.iter().map(|(key, value)| (key, value)))
            .args(["--user", "show", "--property=Version", "--value"]);
        command
    }
}

/// Die Flags, die jede Zeile von `systemd-run` trägt, in dieser Reihenfolge.
const FIXED_FLAGS: &[&str] = &[
    "--user",
    "--quiet",
    "--collect",
    "--pipe",
    "--wait",
    "--service-type=exec",
    "--expand-environment=no",
    "--description=Humanitl sandbox engine client",
    "--property=KillMode=mixed",
    "--property=Delegate=yes",
];

/// Ob dieser Prozess den Client einer Engine über den Dienstverwalter
/// starten muss: wenn er nicht im ersten Nutzer-Namensraum läuft, dessen
/// `/proc/self/uid_map` die ganze Abbildung `0 0 4294967295` trägt. Eine
/// leere oder unlesbare Datei (kein Linux) heißt: nein.
pub(crate) fn needs_user_manager(uid_map: &str) -> bool {
    let initial = uid_map
        .lines()
        .any(|line| line.split_whitespace().collect::<Vec<_>>() == ["0", "0", "4294967295"]);
    !uid_map.trim().is_empty() && !initial
}

/// Wertet die Antwort von [`UserUnits::probe_command`] aus: der
/// Dienstverwalter antwortet, und seine Fassung ist mindestens
/// [`MIN_SYSTEMD_VERSION`].
///
/// # Errors
///
/// `SANDBOX_050` mit dem, was fehlt.
pub(crate) fn check_probe(
    answer: &std::io::Result<std::process::Output>,
) -> Result<u32, Diagnostic> {
    let output = match answer {
        Ok(output) => output,
        Err(err) => {
            return Err(unreachable_manager(&format!(
                "systemctl --user did not answer: {err}"
            )));
        }
    };
    if !output.status.success() {
        return Err(unreachable_manager(&format!(
            "systemctl --user ended with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let digits: String = text
        .trim()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    match digits.parse::<u32>() {
        Ok(version) if version >= MIN_SYSTEMD_VERSION => Ok(version),
        Ok(version) => Err(unreachable_manager(&format!(
            "the user's service manager is systemd {version}, and starting the engine as a \
             unit needs {MIN_SYSTEMD_VERSION} or newer"
        ))),
        Err(_) => Err(unreachable_manager(&format!(
            "the user's service manager named no version it runs ({:?})",
            text.trim()
        ))),
    }
}

/// `SANDBOX_050`: Der Dienstverwalter des Nutzers ist nicht erreichbar oder
/// zu alt, und unter der gehärteten Unit startet die Engine nur über ihn.
pub(crate) fn unreachable_manager(why: &str) -> Diagnostic {
    Diagnostic::builder(SANDBOX_050, Severity::Blocking)
        .why(format!(
            "under its hardened unit the daemon starts the container engine through the user's \
             service manager (systemd-run --user), and {why}. The daemon's hardening stays as \
             it is; the start stops and never takes another backend"
        ))
        .fix(FixAction::CopyCommand(STATUS_COMMAND.to_owned()))
        .build()
}

fn utf8(value: &OsStr) -> Result<&str, String> {
    value.to_str().ok_or_else(|| {
        format!(
            "{:?} is not UTF-8 and cannot go into a unit",
            value.to_string_lossy()
        )
    })
}

fn invalid(why: String) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, why)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::os::unix::process::ExitStatusExt as _;
    use std::process::{ExitStatus, Output};

    use super::*;

    fn units() -> UserUnits {
        UserUnits {
            run: PathBuf::from("/usr/bin/systemd-run"),
            ctl: PathBuf::from("/usr/bin/systemctl"),
            env: vec![("XDG_RUNTIME_DIR".to_owned(), "/run/user/1000".to_owned())],
            prefix: UNIT_PREFIX.to_owned(),
        }
    }

    fn strings(argv: &[OsString]) -> Vec<String> {
        argv.iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    /// Mutationsbeweis für das Präfix: Jeder Name außerhalb von
    /// `humanitl-` wird abgelehnt, auch einer, der es nur enthält.
    #[test]
    fn names_outside_the_prefix_or_with_other_characters_are_refused() {
        for name in [
            "humanitld",
            "humanitld-x",
            "x-humanitl-1",
            "humanitl-",
            "humanitl-A",
            "humanitl-a.b",
            "humanitl-a/b",
            "humanitl-a b",
            "humanitl-a\n",
            "dbus",
            "",
        ] {
            assert!(UnitName::new(name).is_err(), "{name:?} was taken");
        }
        let long = format!("{UNIT_PREFIX}{}", "a".repeat(MAX_NAME));
        assert!(UnitName::new(&long).is_err());
        let session = "0190f5c2-7a3e-7d10-8e1c-1234567890ab";
        let unit = units().named(session).unwrap();
        assert_eq!(unit.as_str(), format!("humanitl-{session}"));
        assert_eq!(unit.service(), format!("humanitl-{session}.service"));
        assert!(units().named("../x").is_err());
        let query = units().query_unit();
        assert!(UnitName::new(query.as_str()).is_ok(), "{query:?}");
        assert!(query.as_str().starts_with("humanitl-q-"));
        let test = units().with_prefix("humanitl-test-");
        assert!(
            test.named("abc")
                .unwrap()
                .as_str()
                .starts_with("humanitl-test-")
        );
    }

    /// Die Zeile einer Sitzung ist genau diese: feste Flags, keine
    /// Laufzeitgrenze, `env -i` mit genau der gegebenen Umgebung, dann
    /// Programm und Argumente unverändert, auch mit `$` und `%`.
    #[test]
    fn the_session_line_is_built_exactly() {
        let unit = UnitName::new("humanitl-abc").unwrap();
        let argv = units()
            .argv(
                &unit,
                Lifetime::Session,
                Path::new("/usr/bin/engine"),
                &[
                    OsString::from("run"),
                    OsString::from("--mount=type=bind,source=/home/u/$HOME%n,destination=/work"),
                ],
                &[
                    ("CONTAINERS_CONF".to_owned(), "/run/x=y/c.conf".to_owned()),
                    ("HOME".to_owned(), "/home/u".to_owned()),
                ],
            )
            .unwrap();
        assert_eq!(
            strings(&argv),
            [
                "/usr/bin/systemd-run",
                "--user",
                "--quiet",
                "--collect",
                "--pipe",
                "--wait",
                "--service-type=exec",
                "--expand-environment=no",
                "--description=Humanitl sandbox engine client",
                "--property=KillMode=mixed",
                "--property=Delegate=yes",
                "--property=TimeoutStopSec=5s",
                "--unit=humanitl-abc.service",
                "--",
                "/usr/bin/env",
                "-i",
                "--",
                "CONTAINERS_CONF=/run/x=y/c.conf",
                "HOME=/home/u",
                "/usr/bin/engine",
                "run",
                "--mount=type=bind,source=/home/u/$HOME%n,destination=/work",
            ]
        );
    }

    #[test]
    fn a_query_is_bounded_and_a_session_is_not() {
        let unit = units().query_unit();
        let argv = strings(
            &units()
                .argv(
                    &unit,
                    Lifetime::Bounded(Duration::from_secs(30)),
                    Path::new("/usr/bin/engine"),
                    &[],
                    &[],
                )
                .unwrap(),
        );
        assert!(argv.contains(&"--property=RuntimeMaxSec=35s".to_owned()));
        let session = strings(
            &units()
                .argv(
                    &unit,
                    Lifetime::Session,
                    Path::new("/usr/bin/engine"),
                    &[],
                    &[],
                )
                .unwrap(),
        );
        assert!(!session.iter().any(|arg| arg.contains("RuntimeMaxSec")));
    }

    #[test]
    fn what_env_would_misread_or_dbus_cannot_carry_is_refused() {
        use std::os::unix::ffi::OsStringExt as _;

        let unit = UnitName::new("humanitl-abc").unwrap();
        let line = |program: &str, args: Vec<OsString>, env: &[(&str, &str)]| {
            let env: Vec<(String, String)> = env
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect();
            units().argv(&unit, Lifetime::Session, Path::new(program), &args, &env)
        };
        assert!(line("engine", vec![], &[]).is_err(), "relative program");
        assert!(
            line("/opt/a=b/engine", vec![], &[]).is_err(),
            "'=' in program"
        );
        assert!(line("/usr/bin/engine", vec![OsString::from_vec(vec![0xff])], &[]).is_err());
        assert!(line("/usr/bin/engine", vec![], &[("-u", "x")]).is_err());
        assert!(line("/usr/bin/engine", vec![], &[("A=B", "x")]).is_err());
        assert!(line("/usr/bin/engine", vec![], &[("", "x")]).is_err());
        assert!(line("/usr/bin/engine", vec![], &[("PATH", "/usr/bin")]).is_ok());
    }

    /// Eine Frage an die Engine wandert ganz in die Unit: Programm,
    /// Argumente, ihre Umgebung; `systemd-run` selbst bekommt nur die eigene.
    #[test]
    fn a_wrapped_command_carries_its_program_args_and_env_into_the_unit() {
        let mut command = Command::new("/usr/bin/engine");
        command.env_clear().env("CONTAINERS_CONF", "/c").args([
            "container",
            "exists",
            "humanitl-x",
        ]);
        let unit = units().query_unit();
        let wrapped = units()
            .wrap(&command, &unit, Lifetime::Bounded(Duration::from_secs(10)))
            .unwrap();
        assert_eq!(wrapped.get_program(), "/usr/bin/systemd-run");
        let args: Vec<String> = wrapped
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        let tail = args.iter().position(|arg| arg == "/usr/bin/env").unwrap();
        assert_eq!(
            args[tail..],
            [
                "/usr/bin/env",
                "-i",
                "--",
                "CONTAINERS_CONF=/c",
                "/usr/bin/engine",
                "container",
                "exists",
                "humanitl-x",
            ]
        );
        assert!(args.contains(&format!("--unit={}", unit.service())));
        let env: Vec<(String, Option<String>)> = wrapped
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.map(|v| v.to_string_lossy().into_owned()),
                )
            })
            .collect();
        assert_eq!(
            env,
            [(
                "XDG_RUNTIME_DIR".to_owned(),
                Some("/run/user/1000".to_owned())
            )]
        );
    }

    #[test]
    fn stopping_names_exactly_the_unit_and_never_blocks() {
        let unit = UnitName::new("humanitl-abc").unwrap();
        let stop = units().stop_command(&unit, false);
        assert_eq!(stop.get_program(), "/usr/bin/systemctl");
        let args: Vec<&OsStr> = stop.get_args().collect();
        assert_eq!(
            args,
            ["--user", "stop", "--no-block", "--", "humanitl-abc.service"]
        );
        let waiting = units().stop_command(&unit, true);
        let args: Vec<&OsStr> = waiting.get_args().collect();
        assert_eq!(args, ["--user", "stop", "--", "humanitl-abc.service"]);
    }

    #[test]
    fn only_a_process_outside_the_first_user_namespace_needs_the_manager() {
        assert!(!needs_user_manager("         0          0 4294967295\n"));
        assert!(needs_user_manager(
            "         0          0          1\n      1000       1000          1\n"
        ));
        assert!(needs_user_manager("      1000       1000          1\n"));
        assert!(!needs_user_manager(""));
    }

    fn output(code: i32, stdout: &str, stderr: &str) -> std::io::Result<Output> {
        let output = Output {
            status: ExitStatus::from_raw(code << 8),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        };
        if code < 0 {
            return Err(std::io::Error::other("no answer"));
        }
        Ok(output)
    }

    #[test]
    fn the_probe_takes_a_reachable_manager_of_254_or_newer() {
        assert_eq!(
            check_probe(&output(0, "257.9-1~deb13u1\n", "")).unwrap(),
            257
        );
        assert_eq!(check_probe(&output(0, "254\n", "")).unwrap(), 254);
        for answer in [
            output(0, "253.5\n", ""),
            output(0, "\n", ""),
            output(1, "", "Failed to connect to bus: No medium found"),
            Err(std::io::Error::other("no such file")),
        ] {
            let diagnostic = check_probe(&answer).unwrap_err();
            assert_eq!(diagnostic.code, SANDBOX_050);
            assert!(
                matches!(&diagnostic.fix, Some(FixAction::CopyCommand(c)) if c == STATUS_COMMAND),
                "{diagnostic:?}"
            );
        }
        let diagnostic = check_probe(&output(1, "", "Failed to connect to bus")).unwrap_err();
        assert!(
            diagnostic.why.contains("Failed to connect to bus"),
            "{}",
            diagnostic.why
        );
    }
}
