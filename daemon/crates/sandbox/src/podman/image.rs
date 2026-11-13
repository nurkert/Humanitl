//! Ein Abbild als Wurzel der Sandbox (HUM-439, HUM-292 Abschnitt 7).
//!
//! **Kein Start zieht.** Die Zeile trägt `--pull=never`, und vor jedem Start
//! fragt der Plan Podman mit `podman image inspect <name>@sha256:<digest>`,
//! ob genau dieses Abbild im lokalen Speicher liegt ([`verdict`]). Fehlt es
//! oder trägt das Abbild unter dem Namen einen anderen Digest, lehnt der
//! Start mit `SANDBOX_043` ab, und die Abhilfe ist der Befehl, der zieht
//! ([`pull_command_text`]). Gestartet wird danach mit dem Verweis samt Digest;
//! Podman löst ihn über den Digest auf, also über den Inhalt, nicht über
//! einen Namen, unter dem morgen ein anderes Abbild liegen könnte.
//!
//! **Ziehen nur auf Wunsch.** `humanitl sandbox image pull` (RPC
//! `SandboxRequest.image_pull`) ist der einzige Weg, auf dem der Adapter ein
//! Abbild holt ([`pull_args`]). Podman holt es dann selbst über das Netz des
//! Daemons, direkt bei der Registry, die der Name nennt: nicht durch den
//! Proxy von Humanitl, ohne die Proxy-Variablen des Hosts und ohne die
//! Spiegel und Suchlisten aus der `registries.conf` des Nutzers. Die
//! Signaturregel ist eine eigene Datei des Adapters
//! (`--signature-policy`), die jedes Abbild annimmt: Was gilt, ist der Digest,
//! den der Mensch genannt hat ([`SIGNATURE_POLICY`]). Ein kurzer Name ohne
//! Registry (`debian@sha256:...`) wird vorher abgelehnt, denn welche Registry
//! ihn bediente, stünde sonst in einer Datei, die der Daemon nicht schreibt.
//!
//! **Zugangsdaten nur aus einer Datei des Adapters (HUM-456).** Die Zeile
//! trägt immer `--authfile`. Liegt `<config>/humanitl/podman/auth.json` vor
//! ([`credentials_path`]), nennt das Flag eine private Kopie ihres geprüften
//! Inhalts im Verzeichnis der Engine, die nach dem Ziehen wieder verschwindet
//! ([`auth_file`]); sonst eine leere Datei des Adapters ([`ANONYMOUS_AUTH`]),
//! und das Ziehen ist anonym. Den Pfad des Menschen sieht die Engine nie: Ein
//! Link oder ein Tausch der Datei nach der Prüfung erreicht sie nicht. Ohne das Flag läse Podman die Dateien, die es selbst kennt:
//! `$XDG_RUNTIME_DIR/containers/auth.json`, `~/.config/containers/auth.json`,
//! `~/.docker/config.json` und, über die Umgebung, `REGISTRY_AUTH_FILE` und
//! `DOCKER_CONFIG`. Mit dem Flag liest `containers/image` nur die genannte
//! Datei (`getAuthFilePaths` in `pkg/docker/config`: die Vorgaben nur ohne
//! ausdrücklichen Pfad). Auf eine fehlende Datei zeigt das Flag nie, denn
//! dann bricht Podman ab (`credential file is not accessible`, `CheckAuthFile`
//! in `containers/common`), statt anonym zu ziehen; deshalb die leere Datei.
//! Die Umgebung leert [`super::EngineDir::command`] ohnehin, und die
//! `registries.conf` des Adapters nennt als einzige Quelle
//! `containers-auth.json`, also die Datei des Flags, keinen Schlüsselbund.
//! Diese Datei ersetzt aber nur die Hauptdatei: Die Ergänzungen in
//! `/etc/containers/registries.conf.d` und
//! `$HOME/.config/containers/registries.conf.d` liest Podman trotzdem
//! (gemessen mit Podman 5.8.6; `XDG_CONFIG_HOME` verdeckt das zweite nicht,
//! es hängt an `HOME`, und `HOME` bleibt, weil dort der rootless-Speicher des
//! Nutzers liegt). Eine Ergänzung mit `credential-helpers` ließe die Engine
//! neben der Datei des Adapters einen Helfer nach Zugangsdaten fragen. Vor
//! jedem Ziehen liest der Adapter deshalb diese Ergänzungen
//! ([`check_registry_dropins`]) und lehnt ab, wenn eine den Schlüssel setzt
//! oder sich nicht lesen lässt; ihr Inhalt erscheint nie in einem Befund.
//! Restrisiko: Zwischen dieser Prüfung und dem Lesen der Engine könnte ein
//! Prozess desselben Kontos eine Ergänzung anlegen. Ein solcher Prozess liest
//! Schlüsselbund und Zugangsdateien aber auch direkt; die Prüfung schützt vor
//! einem versehentlich eingerichteten Helfer, nicht vor ihm.
//! Die Datei des Nutzers muss seinem Konto gehören, darf für Gruppe und
//! andere weder lesbar noch schreibbar sein und darf weder `credHelpers` noch
//! `credsStore` setzen, denn damit fragte `containers/image` vor ihren
//! Einträgen einen Helfer oder Schlüsselbund ([`auth_file`]); ihr Inhalt
//! erscheint nie in einem Befund oder Log, nur ihr Pfad.
//!
//! **Was das Abbild nicht bestimmen darf.** Die Umgebung eines Abbilds legt
//! Podman über die der `containers.conf` (gemessen im Code von Podman,
//! `pkg/specgen/generate/container.go`: erst die Vorgaben, dann das Abbild).
//! Dort stehen die Variablen des Shims; ein Abbild, das eine Variable mit
//! `HUMANITL_` setzt, könnte sie ersetzen und wird abgelehnt. Nutzer,
//! Einstiegspunkt, Volumes und Gesundheitsprüfung des Abbilds hebt die Zeile
//! selbst auf ([`super::plan::line`]).
//!
//! **Abbrechen (HUM-457).** Das Ziehen läuft als eigene Prozessgruppe
//! ([`super::teardown::run_cancellable`]); ein Abbruch und die Frist
//! ([`PULL_TIMEOUT`]) beenden genau diese Gruppe mit `SIGKILL`, und der Befund
//! sagt, wie viele ihrer Prozesse danach noch liefen ([`pull_cancelled`]).
//! Das ist die Grenze: Ein Prozess, der sich mit `setsid` aus der Gruppe
//! löst, bleibt. Die Pausen-Instanz, die Podman ohne Root für alle Befehle
//! teilt, tut das und gehört bewusst nicht zum Ziehen; für einen Helfer des
//! Ziehens selbst ist kein solcher Fall bekannt. Eine Verfolgung über
//! Kontrollgruppen gibt es nicht.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;

use humanitl_config::Paths;
use humanitl_core::diagnostics::codes::{CONFIG_003, SANDBOX_043};
use humanitl_core::{Diagnostic, FixAction, Severity};

/// So lange darf ein Ziehen dauern, bevor der Daemon es abbricht.
pub(super) const PULL_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// Die Signaturregel des Adapters für das ausdrückliche Ziehen: jedes
/// Abbild. Die Vertrauensaussage trägt der Digest, nicht eine Signatur.
pub(super) const SIGNATURE_POLICY: &str = "{\"default\":[{\"type\":\"insecureAcceptAnything\"}]}\n";

/// Die Zugangsdaten eines anonymen Ziehens: keine. Eine Datei mit diesem
/// Inhalt schreibt der Adapter in sein Verzeichnis, damit `--authfile` auch
/// ohne eingerichtete Zugangsdaten auf eine Datei zeigt, die es gibt.
pub(super) const ANONYMOUS_AUTH: &str = "{\"auths\":{}}\n";

/// Der Anfang jeder Variable, die der Shim liest oder die der Plan setzt.
const RESERVED_ENV_PREFIX: &str = "HUMANITL_";

/// Höchstens so viel der Fehlerausgabe von Podman steht in einem Befund.
const DETAIL_CHARS: usize = 400;

/// Was ein Abbild im lokalen Speicher ist, nachdem [`verdict`] es geprüft
/// hat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Found {
    /// Die Kennung des Abbilds (`Id`).
    pub(super) id: String,
    /// Der Digest, den der Verweis nennt, als `sha256:<hex>`.
    pub(super) digest: String,
}

/// Name und Digest (`sha256:<hex>`) eines Verweises `name@sha256:<hex>`.
pub(super) fn split(reference: &str) -> Option<(&str, String)> {
    let (name, hex) = reference.rsplit_once("@sha256:")?;
    Some((name, format!("sha256:{hex}")))
}

/// Die Abhilfe für ein fehlendes Abbild, als Befehl zum Kopieren.
pub(super) fn pull_command_text(reference: &str) -> String {
    format!("humanitl sandbox image pull {reference}")
}

/// `SANDBOX_043`: Das Abbild liegt nicht mit diesem Digest vor. `present`
/// ist der Digest, der unter dem Namen liegt, wenn es einen gibt.
pub(super) fn missing(reference: &str, present: Option<&str>, detail: &str) -> Diagnostic {
    let name = split(reference).map_or(reference, |(name, _)| name);
    let mut why = match present {
        Some(digest) => format!(
            "the image {reference} is not in the local storage of the container engine; under \
             the name {name} it holds {digest}, another image than the one pinned"
        ),
        None => {
            format!("the image {reference} is not in the local storage of the container engine")
        }
    };
    why.push_str(
        ". A start never pulls an image; pulling is a step of its own that uses the network \
         directly, not through the proxy",
    );
    let detail = detail.trim();
    if !detail.is_empty() {
        why.push_str(" (");
        why.extend(detail.chars().take(DETAIL_CHARS));
        why.push(')');
    }
    Diagnostic::builder(SANDBOX_043, Severity::Blocking)
        .why(why)
        .fix(FixAction::CopyCommand(pull_command_text(reference)))
        .build()
}

/// `SANDBOX_043`: Das Abbild liegt vor, taugt aber nicht als Wurzel.
fn unusable(reference: &str, why: &str) -> Diagnostic {
    Diagnostic::builder(SANDBOX_043, Severity::Blocking)
        .why(format!(
            "the image {reference} cannot be the root of the sandbox: {why}"
        ))
        .fix(FixAction::ChangeSetting {
            key: "sandbox.podman.rootfs".to_owned(),
            value: "host".to_owned(),
        })
        .build()
}

/// Das Urteil über die Antwort von `podman image inspect <reference>`: das
/// Abbild mit genau diesem Digest, oder `SANDBOX_043`.
///
/// Gezählt wird der Digest, den Podman für das Abbild selbst nennt (`Digest`
/// und `RepoDigests`), nicht der Name: Podman sucht einen Verweis mit Digest
/// zwar über den Digest, aber ein Urteil, das sich darauf verließe, hinge an
/// einer Fassung der Engine.
///
/// # Errors
///
/// `SANDBOX_043`, wenn Podman nicht antwortet, das Abbild nicht kennt,
/// es einen anderen Digest trägt oder seine Umgebung eine Variable des Shims
/// setzt.
pub(super) fn verdict(reference: &str, answer: &io::Result<Output>) -> Result<Found, Diagnostic> {
    let output = match answer {
        Ok(output) => output,
        Err(err) => return Err(missing(reference, None, &err.to_string())),
    };
    if !output.status.success() {
        return Err(missing(
            reference,
            None,
            &String::from_utf8_lossy(&output.stderr),
        ));
    }
    let Some((_, digest)) = split(reference) else {
        return Err(missing(
            reference,
            None,
            "the reference names no sha256 digest",
        ));
    };
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|err| missing(reference, None, &format!("unreadable answer: {err}")))?;
    let Some(image) = parsed.as_array().and_then(|list| list.first()) else {
        return Err(missing(reference, None, "the answer names no image"));
    };
    let text = |key: &str| image.get(key).and_then(serde_json::Value::as_str);
    let own = text("Digest");
    let repo_digests: Vec<&str> = image
        .get("RepoDigests")
        .and_then(serde_json::Value::as_array)
        .map(|list| list.iter().filter_map(serde_json::Value::as_str).collect())
        .unwrap_or_default();
    let matches = own == Some(digest.as_str())
        || repo_digests
            .iter()
            .any(|entry| entry.rsplit_once('@').is_some_and(|(_, d)| d == digest));
    if !matches {
        let present = own.or_else(|| {
            repo_digests
                .first()
                .and_then(|entry| entry.rsplit_once('@').map(|(_, d)| d))
        });
        return Err(missing(reference, present, ""));
    }
    let env: Vec<&str> = image
        .get("Config")
        .and_then(|config| config.get("Env"))
        .and_then(serde_json::Value::as_array)
        .map(|list| list.iter().filter_map(serde_json::Value::as_str).collect())
        .unwrap_or_default();
    if let Some(reserved) = env
        .iter()
        .map(|pair| pair.split_once('=').map_or(*pair, |(key, _)| key))
        .find(|key| key.starts_with(RESERVED_ENV_PREFIX))
    {
        return Err(unusable(
            reference,
            &format!(
                "its environment sets {reserved}, a name the shim of the sandbox reads, and the \
                 container engine would let the image's value win over the daemon's"
            ),
        ));
    }
    Ok(Found {
        id: text("Id").unwrap_or_default().to_owned(),
        digest,
    })
}

/// Der Digest, den ein Abbild unter dem Namen trägt, aus der Antwort von
/// `podman image inspect <name>`; `None`, wenn es keines gibt.
pub(super) fn present_digest(answer: &io::Result<Output>) -> Option<String> {
    let output = answer.as_ref().ok().filter(|out| out.status.success())?;
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    parsed
        .as_array()?
        .first()?
        .get("Digest")?
        .as_str()
        .filter(|digest| !digest.is_empty())
        .map(str::to_owned)
}

/// Prüft den Verweis eines ausdrücklichen Ziehens: mit Digest und mit der
/// Registry im Namen.
///
/// # Errors
///
/// `CONFIG_003`, wenn der Digest fehlt oder der Name keine Registry nennt.
pub(super) fn check_pull_reference(reference: &str) -> Result<(), Diagnostic> {
    if !humanitl_config::image_pinned_by_digest(reference) {
        return Err(Diagnostic::builder(CONFIG_003, Severity::Error)
            .why(format!(
                "{reference:?} is not pinned by digest; an image is pulled only as \
                 NAME@sha256: and 64 hex digits, so that the name cannot stand for another \
                 image tomorrow"
            ))
            .fix(FixAction::CopyCommand(pull_command_text(
                "registry.example/name@sha256:<64 hex digits>",
            )))
            .build());
    }
    let name = split(reference).map_or(reference, |(name, _)| name);
    if !humanitl_config::image_names_registry(reference) {
        return Err(Diagnostic::builder(CONFIG_003, Severity::Error)
            .why(format!(
                "{reference:?} names no registry; a short name would be resolved by a search \
                 list the daemon does not write. Name the registry, for example \
                 docker.io/library/{name}"
            ))
            .fix(FixAction::CopyCommand(pull_command_text(&format!(
                "docker.io/library/{reference}"
            ))))
            .build());
    }
    Ok(())
}

/// Die Datei, aus der ein Ziehen Zugangsdaten nimmt, wenn der Mensch sie
/// eingerichtet hat: `auth.json` unter `podman/` im Konfigurationsverzeichnis
/// des Daemons, nie eine Datei, deren Ort die Engine bestimmt.
pub(super) fn credentials_path(paths: &Paths) -> PathBuf {
    paths.config_dir().join("podman").join("auth.json")
}

/// Höchstens so groß darf die Datei der Zugangsdaten sein; eine echte
/// `auth.json` hat wenige KiB.
pub(super) const MAX_CREDENTIALS_BYTES: u64 = 1024 * 1024;

/// Schlüssel einer `auth.json`, mit denen `containers/image` vor `auths`
/// einen Helfer oder Schlüsselbund fragt. Go vergleicht Feldnamen in JSON
/// ohne Rücksicht auf Groß- und Kleinschreibung, die Prüfung deshalb auch.
const HELPER_KEYS: [&str; 2] = ["credHelpers", "credsStore"];

/// Wo die Datei für `--authfile` liegt.
#[derive(Debug, Clone, Copy)]
pub(super) struct AuthFiles<'a> {
    /// Die Datei des Menschen ([`credentials_path`]).
    pub(super) user: &'a Path,
    /// Die leere Datei des Adapters ([`ANONYMOUS_AUTH`]).
    pub(super) anonymous: &'a Path,
    /// Die private Kopie der Datei des Menschen im Verzeichnis des Adapters;
    /// der Aufrufer entfernt sie nach dem Ziehen.
    pub(super) copy: &'a Path,
}

/// Die Datei für `--authfile`: ohne Datei des Menschen die leere des
/// Adapters, sonst eine private Kopie ihres geprüften Inhalts.
///
/// Die Engine bekommt nie den Pfad des Menschen. Geöffnet wird er mit
/// `O_NOFOLLOW` (ein symbolischer Link wird abgelehnt), geprüft wird der
/// geöffnete Deskriptor (`fstat`), und gelesen wird aus ihm; was die Engine
/// liest, ist die Kopie dieses Inhalts. Ein Tausch der Datei nach der
/// Prüfung erreicht das Ziehen also nicht. Der Inhalt liegt nur in einem
/// Puffer, der beim Verlassen genullt wird.
///
/// # Errors
///
/// `SANDBOX_043`, wenn die Datei des Menschen da ist, aber ein symbolischer
/// Link, keine reguläre Datei des Kontos `uid`, für Gruppe oder andere
/// zugänglich, größer als [`MAX_CREDENTIALS_BYTES`], kein JSON-Objekt ist
/// oder einen Helfer nennt ([`HELPER_KEYS`]); `SANDBOX_011`, wenn die Kopie
/// sich nicht schreiben lässt. Ein Rückfall auf das anonyme Ziehen wäre
/// falsch: Der Mensch hat Zugangsdaten eingerichtet und bekäme sonst ein
/// `unauthorized`, dessen Grund er nicht sieht.
pub(super) fn auth_file(files: AuthFiles<'_>, uid: u32) -> Result<PathBuf, Diagnostic> {
    use rustix::fs::{FileType, Mode, OFlags};
    use std::io::Read as _;
    use zeroize::Zeroizing;

    let user = files.user;
    // `NONBLOCK`: Ein FIFO an dieser Stelle hält das Öffnen nicht auf; er
    // fällt danach an der Prüfung auf eine reguläre Datei.
    let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK;
    let fd = match rustix::fs::open(user, flags, Mode::empty()) {
        Ok(fd) => fd,
        Err(err) if err == rustix::io::Errno::NOENT => return Ok(files.anonymous.to_owned()),
        Err(err) if err == rustix::io::Errno::LOOP => {
            return Err(unusable_credentials(
                user,
                "it is a symbolic link; the file itself must be there, so that what is checked \
                 is what is used",
                None,
            ));
        }
        Err(err) => {
            return Err(unusable_credentials(
                user,
                &format!("it cannot be opened: {err}"),
                None,
            ));
        }
    };
    let stat = rustix::fs::fstat(&fd)
        .map_err(|err| unusable_credentials(user, &format!("it cannot be checked: {err}"), None))?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
        return Err(unusable_credentials(user, "it is not a regular file", None));
    }
    if stat.st_uid != uid {
        return Err(unusable_credentials(
            user,
            &format!(
                "it belongs to uid {}, not to the account of the daemon (uid {uid})",
                stat.st_uid
            ),
            None,
        ));
    }
    // `RawMode` ist auf Linux `u32`, auf macOS `u16` (HUM-312).
    #[allow(clippy::useless_conversion)]
    let mode = u32::from(stat.st_mode) & 0o7777;
    if mode & 0o077 != 0 {
        return Err(unusable_credentials(
            user,
            &format!("its mode {mode:04o} lets group or others read or change it"),
            Some(format!("chmod 600 '{}'", user.display())),
        ));
    }
    let mut content = Zeroizing::new(Vec::new());
    std::fs::File::from(fd)
        .take(MAX_CREDENTIALS_BYTES + 1)
        .read_to_end(&mut content)
        .map_err(|err| unusable_credentials(user, &format!("it cannot be read: {err}"), None))?;
    if content.len() as u64 > MAX_CREDENTIALS_BYTES {
        return Err(unusable_credentials(
            user,
            &format!("it is larger than {MAX_CREDENTIALS_BYTES} bytes"),
            None,
        ));
    }
    check_credentials_content(user, &content)?;
    write_private_copy(files.copy, &content)?;
    Ok(files.copy.to_owned())
}

/// Der Inhalt muss ein JSON-Objekt ohne Helfer sein. Gelesen werden nur die
/// Schlüssel der obersten Ebene; Werte werden übersprungen, nie kopiert,
/// und ein Befund nennt höchstens Zeile und Spalte, nie Text daraus.
fn check_credentials_content(user: &Path, content: &[u8]) -> Result<(), Diagnostic> {
    // Werte ohne Größe mit Absicht: Sie werden übersprungen, damit kein
    // Geheimnis außerhalb des genullten Puffers liegt.
    #[allow(clippy::zero_sized_map_values)]
    let keys: std::collections::BTreeMap<String, serde::de::IgnoredAny> =
        serde_json::from_slice(content).map_err(|err| {
            unusable_credentials(
                user,
                &format!(
                    "it is not a JSON object (line {}, column {})",
                    err.line(),
                    err.column()
                ),
                Some(format!(
                    "podman login --authfile '{}' <registry>",
                    user.display()
                )),
            )
        })?;
    // Go vergleicht Feldnamen mit Unicode-Faltung (`ſ` gilt als `s`, das
    // Kelvin-Zeichen als `k`). Ein Schlüssel außerhalb von ASCII könnte so
    // ein Helfer sein; keiner der Schlüssel, die `podman login` schreibt,
    // braucht ein solches Zeichen.
    if keys.keys().any(|key| !key.is_ascii()) {
        return Err(unusable_credentials(
            user,
            "a top-level key holds a character outside ASCII, which the container engine \
             could read as a helper key",
            Some(format!(
                "podman login --authfile '{}' <registry>",
                user.display()
            )),
        ));
    }
    if let Some(helper) = HELPER_KEYS
        .iter()
        .find(|helper| keys.keys().any(|key| key.eq_ignore_ascii_case(helper)))
    {
        return Err(unusable_credentials(
            user,
            &format!(
                "it sets {helper}, and the container engine would ask that helper or keyring \
                 for credentials before the file's own entries; remove the {helper} key"
            ),
            Some(format!(
                "podman login --authfile '{}' <registry>",
                user.display()
            )),
        ));
    }
    Ok(())
}

/// Schreibt die Kopie `0600` neu: eine alte erst weg, dann `O_EXCL`, damit
/// nie ein Link oder eine fremde Datei an ihrer Stelle beschrieben wird.
fn write_private_copy(copy: &Path, content: &[u8]) -> Result<(), Diagnostic> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;

    // `create_new` ist `O_EXCL`: Es folgt keinem Link an dieser Stelle.
    match std::fs::remove_file(copy) {
        Ok(()) => {}
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => return Err(super::not_written(copy, &err)),
    }
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(copy)
        .and_then(|mut file| file.write_all(content))
        .map_err(|err| {
            let _ = std::fs::remove_file(copy);
            super::not_written(copy, &err)
        })
}

/// `SANDBOX_043`: Die Datei der Zugangsdaten taugt nicht; gezogen wird
/// nicht. Ohne eigene Abhilfe ist es das Wegräumen der Datei, mit Rückfrage.
fn unusable_credentials(path: &Path, why: &str, fix: Option<String>) -> Diagnostic {
    let fix = fix.unwrap_or_else(|| format!("rm -i '{}'", path.display()));
    Diagnostic::builder(SANDBOX_043, Severity::Blocking)
        .why(format!(
            "registry credentials for a pull are read only from {}, and {why}; nothing was \
             pulled",
            path.display()
        ))
        .fix(FixAction::CopyCommand(fix))
        .build()
}

/// Das Verzeichnis der Ergänzungen zur `registries.conf`, das Podman auch
/// dann liest, wenn `CONTAINERS_REGISTRIES_CONF` die Hauptdatei ersetzt.
pub(super) const SYSTEM_REGISTRIES_DROPINS: &str = "/etc/containers/registries.conf.d";

/// Ein Verzeichnis mit Ergänzungen zur `registries.conf` und ob es dem
/// System gehört (dann braucht die Abhilfe `sudo`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DropinDir {
    pub(super) path: PathBuf,
    pub(super) system: bool,
}

/// Die Verzeichnisse der Ergänzungen in der Reihenfolge, in der
/// `containers/image` sie liest (`sysregistriesv2`: erst das des Systems,
/// dann das unter `home`, dem `HOME` der Engine).
pub(super) fn registry_dropin_dirs(system: &Path, home: &Path) -> [DropinDir; 2] {
    [
        DropinDir {
            path: system.to_owned(),
            system: true,
        },
        DropinDir {
            path: home.join(".config/containers/registries.conf.d"),
            system: false,
        },
    ]
}

/// Lehnt ein Ziehen ab, wenn eine Ergänzung zur `registries.conf` den
/// Schlüssel `credential-helpers` setzt oder sich nicht prüfen lässt. Gezählt
/// wird jede Datei mit der Endung `.conf`, wie bei Podman; ein fehlendes
/// Verzeichnis ist keine Ergänzung. Ein leerer Wert zählt mit, denn ob die
/// Engine ihn als „keiner“ oder als „wie vorher“ nimmt, entscheidet eine
/// Fassung, auf die sich die Prüfung nicht verlassen soll.
///
/// # Errors
///
/// `SANDBOX_043` mit der Datei, nie mit ihrem Inhalt.
pub(super) fn check_registry_dropins(dirs: &[DropinDir]) -> Result<(), Diagnostic> {
    for dir in dirs {
        let entries = match std::fs::read_dir(&dir.path) {
            Ok(entries) => entries,
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => return Err(dropin_dir_unlisted(&dir.path, &err)),
        };
        let mut files = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|err| dropin_dir_unlisted(&dir.path, &err))?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "conf") && !path.is_dir() {
                files.push(path);
            }
        }
        files.sort();
        for file in files {
            check_dropin(&file, dir.system)?;
        }
    }
    Ok(())
}

/// Prüft eine Ergänzung; ihr Inhalt verlässt die Funktion nie, auch nicht
/// über die Fehlermeldung des TOML-Lesers, die eine Zeile daraus zitierte.
fn check_dropin(file: &Path, system: bool) -> Result<(), Diagnostic> {
    let text = std::fs::read_to_string(file).map_err(|err| {
        dropin_refused(
            file,
            system,
            &format!(
                "it cannot be read ({err}), so whether it names a credential helper is unknown"
            ),
        )
    })?;
    let table = text.parse::<toml::Table>().map_err(|err| {
        let line = err.span().map_or(0, |span| {
            text[..span.start.min(text.len())].matches('\n').count() + 1
        });
        dropin_refused(
            file,
            system,
            &format!(
                "it is not valid TOML (line {line}), so whether it names a credential helper is \
                 unknown"
            ),
        )
    })?;
    // Die Engine liest die Schlüssel ohne Unterschied zwischen Groß- und
    // Kleinschreibung (gemessen: `BLOCKED = true` wirkt unter Podman 5.8.6);
    // `Credential-Helpers` muss deshalb genauso zählen.
    // Wie beim JSON der Zugangsdaten: Die Engine faltet Schlüssel auch über
    // ASCII hinaus, ein Schlüssel außerhalb davon könnte der Helfer sein.
    if table
        .keys()
        .any(|key| !key.is_ascii() || key.eq_ignore_ascii_case("credential-helpers"))
    {
        return Err(dropin_refused(
            file,
            system,
            "it sets credential-helpers, and the container engine would ask that helper for \
             registry credentials besides the adapter's file",
        ));
    }
    Ok(())
}

/// `SANDBOX_043`: Das Verzeichnis der Ergänzungen lässt sich nicht lesen;
/// was darin steht, bliebe ungeprüft. Die Abhilfe zeigt seine Rechte, denn
/// das Verzeichnis wegzuräumen nähme dem System auch harmlose Ergänzungen.
fn dropin_dir_unlisted(dir: &Path, err: &io::Error) -> Diagnostic {
    Diagnostic::builder(SANDBOX_043, Severity::Blocking)
        .why(format!(
            "the container engine reads registry drop-ins from {} besides the adapter's \
             configuration, and the directory cannot be listed ({err}), so whether one names a \
             credential helper is unknown; nothing was pulled",
            dir.display()
        ))
        .fix(FixAction::CopyCommand(format!(
            "ls -ld '{}'",
            dir.display()
        )))
        .build()
}

/// `SANDBOX_043`: Eine Ergänzung zur `registries.conf` hält das Ziehen auf.
/// Die Abhilfe benennt die Datei um, sodass Podman sie nicht mehr liest; wer
/// die Ergänzung braucht, entfernt stattdessen den Schlüssel.
fn dropin_refused(path: &Path, system: bool, why: &str) -> Diagnostic {
    let sudo = if system { "sudo " } else { "" };
    Diagnostic::builder(SANDBOX_043, Severity::Blocking)
        .why(format!(
            "the container engine reads the registry drop-in {} besides the adapter's \
             configuration, and {why}; nothing was pulled. Move it aside, or remove the \
             credential-helpers key from it",
            path.display()
        ))
        .fix(FixAction::CopyCommand(format!(
            "{sudo}mv -i '{0}' '{0}.disabled'",
            path.display()
        )))
        .build()
}

/// Die Argumente von `podman` nach den globalen Flags für ein Ziehen, mit der
/// Signaturregel und der Datei der Zugangsdaten des Adapters ([`auth_file`]).
pub(super) fn pull_args(reference: &str, signature_policy: &Path, auth_file: &Path) -> Vec<String> {
    vec![
        "pull".to_owned(),
        "--quiet".to_owned(),
        format!("--signature-policy={}", signature_policy.display()),
        format!("--authfile={}", auth_file.display()),
        "--".to_owned(),
        reference.to_owned(),
    ]
}

/// `SANDBOX_043`: Das Ziehen scheiterte. `credentials` ist der Ort, an dem
/// eingerichtete Zugangsdaten stünden: Bei einem `unauthorized` der Registry
/// soll der Befund sagen, wo.
pub(super) fn pull_failed(
    reference: &str,
    answer: &io::Result<Output>,
    credentials: &Path,
) -> Diagnostic {
    let detail = match answer {
        Ok(output) => String::from_utf8_lossy(&output.stderr).into_owned(),
        Err(err) => err.to_string(),
    };
    let mut why = format!(
        "the container engine could not pull {reference} from its registry; nothing was \
         started. Registry credentials are read only from {}",
        credentials.display()
    );
    let detail = detail.trim();
    if !detail.is_empty() {
        why.push_str(" (");
        why.extend(detail.chars().take(DETAIL_CHARS));
        why.push(')');
    }
    Diagnostic::builder(SANDBOX_043, Severity::Blocking)
        .why(why)
        .fix(FixAction::CopyCommand(pull_command_text(reference)))
        .build()
}

/// `SANDBOX_043`: Das Ziehen wurde auf Wunsch abgebrochen (HUM-457). Der
/// Prozess der Engine und seine Prozessgruppe sind beendet; `alive` sagt, wie
/// viele Prozesse der Gruppe danach trotzdem noch liefen, und der Satz
/// verschweigt sie nicht. Was die Engine bis dahin in den Speicher schrieb,
/// ist kein vollständiges Abbild, und kein Start nimmt es, solange der
/// Digest nicht vorliegt.
pub(super) fn pull_cancelled(reference: &str, alive: usize) -> Diagnostic {
    let ended = if alive == 0 {
        "and no process of it runs any more".to_owned()
    } else {
        format!("but {alive} process(es) of it still ran after the container engine was killed")
    };
    Diagnostic::builder(SANDBOX_043, Severity::Error)
        .why(format!(
            "the pull of {reference} was cancelled on request {ended}; nothing was started, \
             and the image is not there yet"
        ))
        .fix(FixAction::CopyCommand(pull_command_text(reference)))
        .build()
}

/// Der Grund nach abgelaufener Frist ([`PULL_TIMEOUT`]): beendet, und wie
/// viele Prozesse des Ziehens danach noch liefen (HUM-457).
pub(super) fn deadline_detail(alive: usize) -> String {
    let minutes = PULL_TIMEOUT.as_secs() / 60;
    if alive == 0 {
        format!("the pull did not end within {minutes} minutes and was killed")
    } else {
        format!(
            "the pull did not end within {minutes} minutes and was killed; {alive} \
             process(es) of it still ran afterwards"
        )
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::os::unix::process::ExitStatusExt as _;
    use std::process::ExitStatus;

    use super::*;

    const PINNED: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90";
    const OTHER: &str = "0000000000000000000000000000000000000000000000000000000000000001";

    fn reference() -> String {
        format!("localhost/humanitl-test@sha256:{PINNED}")
    }

    /// Die Antwort von Podman; ein negativer `code` ist ein Aufruf, der gar
    /// nicht antwortete (Frist abgelaufen), mit `stderr` als Grund.
    fn answer(code: i32, stdout: &str, stderr: &str) -> io::Result<Output> {
        if code < 0 {
            return Err(io::Error::other(stderr.to_owned()));
        }
        Ok(Output {
            status: ExitStatus::from_raw(code << 8),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        })
    }

    fn inspect(digest: &str, env: &[&str]) -> String {
        serde_json::json!([{
            "Id": "f00d",
            "Digest": format!("sha256:{digest}"),
            "RepoDigests": [format!("localhost/humanitl-test@sha256:{digest}")],
            "Config": { "Env": env },
        }])
        .to_string()
    }

    /// Das Abbild mit genau dem Digest geht durch; ein anderer Digest unter
    /// demselben Namen ist `SANDBOX_043` mit dem Befehl, der zieht. Das ist
    /// die Prüfung, ohne die ein Start ein anderes Abbild nähme.
    #[test]
    fn only_the_pinned_digest_passes() {
        let found = verdict(
            &reference(),
            &answer(0, &inspect(PINNED, &["PATH=/bin"]), ""),
        )
        .expect("pinned");
        assert_eq!(found.digest, format!("sha256:{PINNED}"));
        assert_eq!(found.id, "f00d");

        let err = verdict(&reference(), &answer(0, &inspect(OTHER, &[]), ""))
            .expect_err("another digest");
        assert_eq!(err.code.as_str(), "SANDBOX_043", "{}", err.why);
        assert!(err.why.contains(&format!("sha256:{OTHER}")), "{}", err.why);
        assert!(err.why.contains("never pulls"), "{}", err.why);
        assert_eq!(
            err.fix,
            Some(FixAction::CopyCommand(format!(
                "humanitl sandbox image pull {}",
                reference()
            )))
        );
    }

    /// Ein Verweis auf eine Liste (mehrere Plattformen) steht nur in
    /// `RepoDigests`; das genügt.
    #[test]
    fn a_digest_in_repo_digests_counts() {
        let body = serde_json::json!([{
            "Id": "f00d",
            "Digest": format!("sha256:{OTHER}"),
            "RepoDigests": [
                format!("localhost/humanitl-test@sha256:{OTHER}"),
                format!("localhost/humanitl-test@sha256:{PINNED}"),
            ],
        }])
        .to_string();
        verdict(&reference(), &answer(0, &body, "")).expect("listed digest");
    }

    #[test]
    fn a_missing_image_or_a_silent_engine_is_sandbox_043() {
        for answer in [
            answer(125, "", "Error: localhost/humanitl-test: image not known"),
            answer(0, "[]", ""),
            answer(0, "not json", ""),
            answer(-1, "", "no answer within 30s"),
        ] {
            let err = verdict(&reference(), &answer).expect_err("missing");
            assert_eq!(err.code.as_str(), "SANDBOX_043", "{}", err.why);
            assert!(err.fix.is_some());
        }
        let err = verdict(&reference(), &answer(125, "", "image not known")).unwrap_err();
        assert!(err.why.contains("image not known"), "{}", err.why);
    }

    /// Die Umgebung eines Abbilds geht in Podman vor die der
    /// `containers.conf`; eine Variable des Shims darin wird abgelehnt.
    #[test]
    fn an_image_that_sets_a_shim_variable_is_refused() {
        let err = verdict(
            &reference(),
            &answer(
                0,
                &inspect(PINNED, &["PATH=/bin", "HUMANITL_BRIDGES=x"]),
                "",
            ),
        )
        .expect_err("reserved");
        assert_eq!(err.code.as_str(), "SANDBOX_043");
        assert!(err.why.contains("HUMANITL_BRIDGES"), "{}", err.why);
        verdict(
            &reference(),
            &answer(0, &inspect(PINNED, &["MY_HUMANITL_X=1"]), ""),
        )
        .expect("only the prefix counts");
    }

    #[test]
    fn the_present_digest_is_read_from_a_name_only_answer() {
        assert_eq!(
            present_digest(&answer(0, &inspect(OTHER, &[]), "")),
            Some(format!("sha256:{OTHER}"))
        );
        assert_eq!(present_digest(&answer(125, "", "image not known")), None);
        let err = missing(&reference(), Some("sha256:abc"), "");
        assert!(
            err.why
                .contains("under the name localhost/humanitl-test it holds sha256:abc")
        );
    }

    #[test]
    fn a_pull_needs_a_digest_and_a_registry() {
        check_pull_reference(&format!("docker.io/library/debian@sha256:{PINNED}"))
            .expect("qualified");
        check_pull_reference(&reference()).expect("localhost");
        check_pull_reference(&format!("registry:5000/x@sha256:{PINNED}")).expect("port");
        let err = check_pull_reference("docker.io/library/debian:trixie").expect_err("tag");
        assert_eq!(err.code.as_str(), "CONFIG_003");
        let err = check_pull_reference(&format!("debian@sha256:{PINNED}")).expect_err("short");
        assert_eq!(err.code.as_str(), "CONFIG_003");
        assert!(err.why.contains("docker.io/library/debian"), "{}", err.why);
        let err = check_pull_reference(&format!("library/debian@sha256:{PINNED}"))
            .expect_err("no registry");
        assert_eq!(err.code.as_str(), "CONFIG_003");
    }

    /// Die Zeile nennt Signaturregel und Zugangsdaten des Adapters. Ohne
    /// `--authfile` läse die Engine die Zugangsdaten, die sie selbst findet
    /// (HUM-456).
    #[test]
    fn the_pull_line_names_the_adapters_signature_policy_and_credentials() {
        let args = pull_args(
            &reference(),
            Path::new("/run/e/policy.json"),
            Path::new("/run/e/auth.json"),
        );
        assert_eq!(
            args,
            [
                "pull".to_owned(),
                "--quiet".to_owned(),
                "--signature-policy=/run/e/policy.json".to_owned(),
                "--authfile=/run/e/auth.json".to_owned(),
                "--".to_owned(),
                reference(),
            ]
        );
        let policy: serde_json::Value = serde_json::from_str(SIGNATURE_POLICY).expect("json");
        assert_eq!(policy["default"][0]["type"], "insecureAcceptAnything");
        let anonymous: serde_json::Value = serde_json::from_str(ANONYMOUS_AUTH).expect("json");
        assert_eq!(anonymous, serde_json::json!({ "auths": {} }));
    }

    /// Der Ort der Zugangsdaten liegt im Konfigurationsverzeichnis des
    /// Daemons, nicht dort, wo die Engine von selbst sucht.
    #[test]
    fn the_credentials_live_in_the_daemons_config_dir() {
        let paths = Paths::new(humanitl_config::Env::from_pairs([
            ("HOME", "/home/u"),
            ("XDG_CONFIG_HOME", "/home/u/.config"),
            ("XDG_RUNTIME_DIR", "/run/user/1000"),
            ("REGISTRY_AUTH_FILE", "/home/u/elsewhere.json"),
        ]));
        assert_eq!(
            credentials_path(&paths),
            Path::new("/home/u/.config/humanitl/podman/auth.json")
        );
    }

    /// Ohne Datei des Nutzers zieht es anonym; mit einer privaten Datei
    /// bekommt die Engine eine Kopie ihres Inhalts, nie ihren Pfad; eine
    /// lesbare, fremde, verlinkte oder zu große Datei lehnt es ab, statt
    /// still anonym zu ziehen.
    #[test]
    fn the_credentials_file_is_used_only_when_private_and_own() {
        use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

        let dir = tempfile::tempdir().unwrap();
        let user = dir.path().join("auth.json");
        let anonymous = dir.path().join("anonymous.json");
        let copy = dir.path().join("engine/pull-auth.json");
        std::fs::create_dir(copy.parent().unwrap()).unwrap();
        let files = AuthFiles {
            user: &user,
            anonymous: &anonymous,
            copy: &copy,
        };
        let uid = rustix::process::getuid().as_raw();

        assert_eq!(auth_file(files, uid).unwrap(), anonymous);
        assert!(!copy.exists());

        let secret = "{\"auths\":{\"r.example\":{\"auth\":\"c2VjcmV0\"}}}";
        std::fs::write(&user, secret).unwrap();
        std::fs::set_permissions(&user, std::fs::Permissions::from_mode(0o600)).unwrap();
        // Eine liegengebliebene Kopie wird ersetzt, nicht beschrieben.
        std::fs::write(&copy, "stale").unwrap();
        assert_eq!(auth_file(files, uid).unwrap(), copy);
        assert_eq!(std::fs::read_to_string(&copy).unwrap(), secret);
        assert_eq!(std::fs::metadata(&copy).unwrap().mode() & 0o777, 0o600);
        // Ein Tausch der Datei des Nutzers nach der Prüfung erreicht die
        // Kopie nicht.
        std::fs::write(&user, "{\"credHelpers\":{\"r.example\":\"evil\"}}").unwrap();
        assert_eq!(std::fs::read_to_string(&copy).unwrap(), secret);
        std::fs::write(&user, secret).unwrap();
        std::fs::remove_file(&copy).unwrap();

        // Ein symbolischer Link wird nie verfolgt, auch nicht auf eine
        // private Datei.
        let target = dir.path().join("target.json");
        std::fs::write(&target, secret).unwrap();
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600)).unwrap();
        let linked = dir.path().join("linked.json");
        std::os::unix::fs::symlink(&target, &linked).unwrap();
        let err = auth_file(
            AuthFiles {
                user: &linked,
                ..files
            },
            uid,
        )
        .expect_err("a symbolic link");
        assert_eq!(err.code.as_str(), "SANDBOX_043", "{}", err.why);
        assert!(err.why.contains("symbolic link"), "{}", err.why);
        assert!(!copy.exists());

        // Zu groß.
        let big = dir.path().join("big.json");
        std::fs::write(
            &big,
            vec![b' '; usize::try_from(MAX_CREDENTIALS_BYTES).unwrap() + 1],
        )
        .unwrap();
        std::fs::set_permissions(&big, std::fs::Permissions::from_mode(0o600)).unwrap();
        let err = auth_file(
            AuthFiles {
                user: &big,
                ..files
            },
            uid,
        )
        .expect_err("too large");
        assert!(err.why.contains("larger than"), "{}", err.why);

        for mode in [0o640, 0o604, 0o620] {
            std::fs::set_permissions(&user, std::fs::Permissions::from_mode(mode)).unwrap();
            let err = auth_file(files, uid).expect_err("readable");
            assert_eq!(err.code.as_str(), "SANDBOX_043", "{}", err.why);
            assert!(err.why.contains(&format!("{mode:04o}")), "{}", err.why);
            assert!(!err.why.contains("c2VjcmV0"), "{}", err.why);
            assert_eq!(
                err.fix,
                Some(FixAction::CopyCommand(format!(
                    "chmod 600 '{}'",
                    user.display()
                )))
            );
        }

        std::fs::set_permissions(&user, std::fs::Permissions::from_mode(0o600)).unwrap();
        let err = auth_file(files, uid.wrapping_add(1)).expect_err("foreign");
        assert!(err.why.contains("belongs to uid"), "{}", err.why);

        std::fs::remove_file(&user).unwrap();
        std::fs::create_dir(&user).unwrap();
        let err = auth_file(files, uid).expect_err("a directory");
        assert!(err.why.contains("not a regular file"), "{}", err.why);
    }

    /// Eine `auth.json` mit Helfer oder Schlüsselbund ließe die Engine vor
    /// den eigenen Einträgen woanders fragen; Go liest die Schlüssel ohne
    /// Rücksicht auf Groß- und Kleinschreibung. Kein JSON-Objekt wird
    /// abgelehnt, ohne Text daraus zu zitieren.
    #[test]
    fn a_credentials_file_with_a_helper_or_no_json_object_is_refused() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = tempfile::tempdir().unwrap();
        let user = dir.path().join("auth.json");
        let copy = dir.path().join("pull-auth.json");
        let files = AuthFiles {
            user: &user,
            anonymous: &dir.path().join("anonymous.json"),
            copy: &copy,
        };
        let uid = rustix::process::getuid().as_raw();
        let write = |text: &str| {
            std::fs::write(&user, text).unwrap();
            std::fs::set_permissions(&user, std::fs::Permissions::from_mode(0o600)).unwrap();
        };

        for (text, key) in [
            (
                "{\"auths\":{},\"credHelpers\":{\"r.example\":\"s3cr3t-helper\"}}",
                "credHelpers",
            ),
            (
                "{\"CredHelpers\":{\"r.example\":\"s3cr3t-helper\"}}",
                "credHelpers",
            ),
            ("{\"credsStore\":\"s3cr3t-helper\"}", "credsStore"),
            ("{\"CREDSSTORE\":\"s3cr3t-helper\"}", "credsStore"),
        ] {
            write(text);
            let err = auth_file(files, uid).expect_err(text);
            assert_eq!(err.code.as_str(), "SANDBOX_043", "{}", err.why);
            assert!(err.why.contains(key), "{}", err.why);
            assert!(err.why.contains(&user.display().to_string()), "{}", err.why);
            assert!(!err.why.contains("s3cr3t-helper"), "{}", err.why);
            assert!(!copy.exists(), "no copy of a refused file");
        }

        // Go faltet `ſ` zu `s` und das Kelvin-Zeichen zu `k`: Ein Schlüssel
        // außerhalb von ASCII könnte ein Helfer sein und wird abgelehnt.
        for text in [
            "{\"credHelper\u{17f}\":{\"r.example\":\"s3cr3t-helper\"}}",
            "{\"auth\u{17f}\":{}}",
        ] {
            write(text);
            let err = auth_file(files, uid).expect_err(text);
            assert!(err.why.contains("outside ASCII"), "{}", err.why);
            assert!(!err.why.contains("s3cr3t-helper"), "{}", err.why);
            assert!(!copy.exists());
        }

        for text in [
            "{\"auths\":{\"r.example\":{\"auth\":\"s3cr3t-value",
            "[\"s3cr3t-value\"]",
            "",
        ] {
            write(text);
            let err = auth_file(files, uid).expect_err("not a JSON object");
            assert!(err.why.contains("not a JSON object (line"), "{}", err.why);
            assert!(!err.why.contains("s3cr3t-value"), "{}", err.why);
            assert!(!copy.exists());
        }

        write("{\"auths\":{\"r.example\":{\"auth\":\"c2VjcmV0\"}},\"Auths\":{}}");
        assert_eq!(auth_file(files, uid).unwrap(), copy);
    }

    /// Die Ergänzungen zur `registries.conf` liest Podman trotz der eigenen
    /// Hauptdatei des Adapters: Nennt eine einen Credential-Helfer oder lässt
    /// sie sich nicht lesen, wird nicht gezogen. Das Verzeichnis des Systems
    /// ist hier ein temporäres, damit der Test nicht vom Host abhängt.
    #[test]
    fn a_registry_dropin_with_a_credential_helper_refuses_the_pull() {
        let root = tempfile::tempdir().unwrap();
        let system = root.path().join("etc/registries.conf.d");
        let home = root.path().join("home");
        let dirs = registry_dropin_dirs(&system, &home);
        assert_eq!(
            dirs[1].path,
            home.join(".config/containers/registries.conf.d")
        );
        assert!(dirs[0].system && !dirs[1].system);

        // Keine Verzeichnisse: nichts zu prüfen.
        check_registry_dropins(&dirs).expect("no drop-ins");

        // Harmlose Ergänzungen, auch eine Datei ohne `.conf`, gehen durch.
        std::fs::create_dir_all(&system).unwrap();
        std::fs::create_dir_all(&dirs[1].path).unwrap();
        std::fs::write(
            system.join("000-shortnames.conf"),
            "[aliases]\n\"debian\" = \"docker.io/library/debian\"\n",
        )
        .unwrap();
        std::fs::write(
            dirs[1].path.join("mirror.conf"),
            "[[registry]]\nlocation = \"docker.io\"\n[[registry.mirror]]\nlocation = \"m.example\"\n",
        )
        .unwrap();
        std::fs::write(
            dirs[1].path.join("notes.txt"),
            "credential-helpers = [\"secretservice\"]\n",
        )
        .unwrap();
        check_registry_dropins(&dirs).expect("harmless drop-ins");

        // Ein Helfer in der Ergänzung des Nutzers: abgelehnt, mit der Datei.
        let helper = dirs[1].path.join("50-keyring.conf");
        std::fs::write(&helper, "credential-helpers = [\"secretservice\"]\n").unwrap();
        let err = check_registry_dropins(&dirs).expect_err("a credential helper");
        assert_eq!(err.code.as_str(), "SANDBOX_043", "{}", err.why);
        assert!(
            err.why.contains(&helper.display().to_string()),
            "{}",
            err.why
        );
        assert!(err.why.contains("credential-helpers"), "{}", err.why);
        assert!(!err.why.contains("secretservice"), "{}", err.why);
        assert_eq!(
            err.fix,
            Some(FixAction::CopyCommand(format!(
                "mv -i '{0}' '{0}.disabled'",
                helper.display()
            )))
        );
        std::fs::remove_file(&helper).unwrap();

        // Die Engine liest Schlüssel ohne Rücksicht auf die Schreibweise.
        let shouted = dirs[1].path.join("51-shouted.conf");
        std::fs::write(&shouted, "Credential-Helpers = [\"secretservice\"]\n").unwrap();
        let err = check_registry_dropins(&dirs).expect_err("the key in other case");
        assert!(
            err.why.contains(&shouted.display().to_string()),
            "{}",
            err.why
        );
        std::fs::remove_file(&shouted).unwrap();
        let folded = dirs[1].path.join("52-folded.conf");
        std::fs::write(
            &folded,
            "\"credential-helper\u{17f}\" = [\"secretservice\"]\n",
        )
        .unwrap();
        let err = check_registry_dropins(&dirs).expect_err("a folded key");
        assert!(
            err.why.contains(&folded.display().to_string()),
            "{}",
            err.why
        );
        std::fs::remove_file(&folded).unwrap();

        // Ein Helfer im Verzeichnis des Systems: die Abhilfe braucht `sudo`,
        // auch ein leerer Wert zählt.
        let system_helper = system.join("99-empty.conf");
        std::fs::write(&system_helper, "credential-helpers = []\n").unwrap();
        let err = check_registry_dropins(&dirs).expect_err("system helper");
        assert!(
            matches!(&err.fix, Some(FixAction::CopyCommand(cmd)) if cmd.starts_with("sudo mv -i")),
            "{:?}",
            err.fix
        );
        std::fs::remove_file(&system_helper).unwrap();

        // Unlesbares TOML: abgelehnt, ohne eine Zeile daraus zu zitieren.
        let broken = dirs[1].path.join("broken.conf");
        std::fs::write(&broken, "ok = 1\nsecret-token = \"s3cr3t-value\n").unwrap();
        let err = check_registry_dropins(&dirs).expect_err("unparsable");
        assert!(
            err.why.contains(&broken.display().to_string()),
            "{}",
            err.why
        );
        assert!(err.why.contains("not valid TOML (line 2)"), "{}", err.why);
        assert!(!err.why.contains("s3cr3t-value"), "{}", err.why);
    }

    /// Ein gescheitertes Ziehen sagt, woher Zugangsdaten kämen.
    /// Der Befund eines Abbruchs verschweigt keine Prozesse, die das
    /// `SIGKILL` überlebten, und die Frist nennt sie ebenso (HUM-457).
    #[test]
    fn a_cancel_says_whether_processes_of_the_pull_still_ran() {
        let clean = pull_cancelled(&reference(), 0);
        assert_eq!(clean.code.as_str(), "SANDBOX_043");
        assert!(
            clean.why.contains("no process of it runs any more"),
            "{}",
            clean.why
        );
        let left = pull_cancelled(&reference(), 2);
        assert!(
            left.why.contains("but 2 process(es) of it still ran"),
            "{}",
            left.why
        );
        assert!(!left.why.contains("no process of it runs"), "{}", left.why);
        assert!(!deadline_detail(0).contains("still ran"));
        assert!(deadline_detail(3).contains("3 process(es) of it still ran"));
    }

    #[test]
    fn a_failed_pull_names_where_credentials_come_from() {
        let err = pull_failed(
            &reference(),
            &answer(125, "", "unauthorized: authentication required"),
            Path::new("/c/humanitl/podman/auth.json"),
        );
        assert_eq!(err.code.as_str(), "SANDBOX_043");
        assert!(
            err.why
                .contains("credentials are read only from /c/humanitl/podman/auth.json"),
            "{}",
            err.why
        );
        assert!(err.why.contains("unauthorized"), "{}", err.why);
    }
}
