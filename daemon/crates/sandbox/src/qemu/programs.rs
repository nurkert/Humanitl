//! Die Programme der Unit einer Sitzung, erzwungen vom Dienstverwalter
//! (HUM-480).
//!
//! Die Host-Prüfung (HUM-468) sieht einmal vor der Zulassung, welche
//! Programme in der Unit laufen; was ein übernommener Helfer danach startet,
//! sähe sie nicht. Deshalb trägt die Unit `NoExecPaths=/` und
//! `ExecPaths=` mit genau den Dateien, die ihre Prozesse ausführen oder als
//! ausführbaren Code abbilden: `env`, das die Umgebung leert
//! ([`crate::user_unit::ENV_PROGRAM`], der erste Prozess), der Shim, die Shell des
//! Skripts, `sleep` (das Warten des Skripts auf die Sockets), bubblewrap,
//! virtiofsd, das vsock-Gerät und der Emulator, jedes unter seinem
//! aufgelösten Pfad, dazu je dynamisch gebundenem Programm sein Lader und
//! jede Bibliothek, die der Lader für es findet.
//!
//! **Wie es wirkt.** systemd hängt in der Unit alles unter `/` mit `noexec`
//! ein und bindet jede Datei der Liste ohne `noexec` über sich selbst. Ein
//! `execve` auf eine andere Datei endet mit `EACCES`, ein `mmap` mit
//! `PROT_EXEC` mit `EPERM`; auch der Umweg über den Lader
//! (`ld.so /usr/bin/id`) scheitert daran. Die Gefängnisse von bubblewrap
//! erben das: Sie entstehen in einem Nutzer-Namensraum unter dem der Unit,
//! und der Kernel sperrt dort `noexec` jeder geerbten Einhängung
//! (`MNT_LOCK_NOEXEC`); `--ro-bind /usr /usr` nimmt die Einhängungen der
//! Dateien rekursiv mit. Gemessen am 2026-11-19 mit systemd 262 und
//! bubblewrap 0.13 (`backlog/sprint-7.md`, HUM-480).
//!
//! **Warum Dateien und nicht Verzeichnisse.** Die Bibliotheken liegen in
//! einem Verzeichnis mit vielen weiteren Programmen (auf dem gemessenen
//! Debian-Rechner 193 unter
//! `/usr/lib/x86_64-linux-gnu`); ein Verzeichnis in der Liste ließe sie alle
//! zu. Die Bibliotheken kommen vom Lader des Programms selbst
//! (`<Lader> --list <Programm>`), der dabei nichts vom Programm ausführt.
//! Was ein Programm erst zur Laufzeit mit `dlopen` lädt, fehlt; gemessen an
//! einer echten Sitzung bildet keiner der Prozesse eine Datei ab, die nicht
//! auf der Liste steht.
//!
//! Ein Programm, das kein ELF ist (ein Skript mit `#!`), lehnt der Plan ab:
//! Was es seinerseits startet, kennt die Liste nicht, und die Unit endete
//! mitten im Start.

use std::collections::BTreeSet;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use humanitl_core::Diagnostic;

use super::prerequisites::Resolved;
use super::{UNIT_PATH, lines, property_path, setup_failed};

/// Das Programm, mit dem das Skript auf die Sockets der Dienste wartet
/// ([`lines::script`]); die Shell findet es über den Suchpfad der Unit.
pub(crate) const SCRIPT_SLEEP: &str = "sleep";

/// So lange darf der Lader für die Liste eines Programms brauchen.
const LIST_TIMEOUT: Duration = Duration::from_secs(10);

/// Was der Kopf eines Programms über seinen Start sagt.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Start {
    /// Ein ELF mit diesem Lader (`PT_INTERP`).
    Dynamic(PathBuf),
    /// Ein ELF ohne Lader.
    Static,
    /// Kein ELF, etwa ein Skript mit `#!`.
    NotElf,
}

/// Die Dateien, die die Unit einer Sitzung ausführen darf, aufgelöst,
/// sortiert und je einmal; `shim` folgt auf `env` und übergibt an die Shell.
///
/// # Errors
///
/// `SANDBOX_011`, wenn ein Programm fehlt, kein ELF ist, eine Datei der Liste
/// nach dem Auflösen keine gewöhnliche Datei ist oder der Lader eine
/// Bibliothek nicht findet; `SANDBOX_006`, wenn ein Pfad nicht in die
/// Eigenschaft passt.
pub(crate) fn exec_paths(resolved: &Resolved, shim: &Path) -> Result<Vec<String>, Diagnostic> {
    let mut files = BTreeSet::new();
    for program in configured_programs(resolved, shim)? {
        files.extend(program_files(&program, &loader_list).map_err(setup_failed)?);
    }
    files.iter().map(|file| property_path(file)).collect()
}

/// Die Programme der Unit, wie sie eingestellt sind: `env`, der Shim, die
/// Shell des Skripts, `sleep` (im Suchpfad der Unit), bubblewrap,
/// virtiofsd, das vsock-Gerät und der Emulator.
fn configured_programs(resolved: &Resolved, shim: &Path) -> Result<Vec<PathBuf>, Diagnostic> {
    let sleep = in_unit_path(SCRIPT_SLEEP).ok_or_else(|| {
        setup_failed(format!(
            "the script of the kernel level waits with {SCRIPT_SLEEP}, and there is no \
             executable {SCRIPT_SLEEP} in {UNIT_PATH}"
        ))
    })?;
    Ok(vec![
        PathBuf::from(crate::user_unit::ENV_PROGRAM),
        shim.to_path_buf(),
        PathBuf::from(lines::SCRIPT_SHELL),
        sleep,
        resolved.bubblewrap.clone(),
        resolved.file_daemon.clone(),
        resolved.vsock_device.clone(),
        resolved.emulator.clone(),
    ])
}

/// Ein Programm der Unit: wie es eingestellt ist und wohin es aufgelöst ist
/// (HUM-557).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Program {
    /// Der Pfad aus der Einstellung oder dem Suchpfad.
    pub(crate) configured: PathBuf,
    /// Die Datei, die `ExecPaths=` nennt.
    pub(crate) canonical: PathBuf,
}

/// Die Programme der Unit, je aufgelöst; die Gefängnisse verdecken sie
/// einander ([`hidden_in_jail`]).
///
/// # Errors
///
/// `SANDBOX_011` wie [`exec_paths`].
pub(crate) fn unit_programs(resolved: &Resolved, shim: &Path) -> Result<Vec<Program>, Diagnostic> {
    configured_programs(resolved, shim)?
        .into_iter()
        .map(|configured| {
            let canonical = canonical(&configured).map_err(setup_failed)?;
            Ok(Program {
                configured,
                canonical,
            })
        })
        .collect()
}

/// Was ein Gefängnis mit dem Helfer `own` verdeckt (HUM-557): jedes andere
/// Programm der Unit, das es unter `/usr` sieht, unter seinem aufgelösten
/// Pfad, sortiert und je einmal. Nur dort ist es ausführbar (`ExecPaths=`
/// nennt nur diesen Pfad), und nur `/usr` bindet jedes Gefängnis; ein
/// Programm außerhalb von `/usr` bindet nur das Gefängnis, dessen Helfer es
/// ist. Der Helfer selbst bleibt sichtbar, auch wenn ein anderes Programm
/// auf dieselbe Datei aufgelöst ist.
pub(crate) fn hidden_in_jail(programs: &[Program], own: &Path) -> Vec<PathBuf> {
    let own = programs
        .iter()
        .find(|program| program.configured == own)
        .map_or(own, |program| program.canonical.as_path());
    programs
        .iter()
        .map(|program| program.canonical.clone())
        .filter(|canonical| canonical.starts_with("/usr") && canonical != own)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Die Dateien, die ein weiteres Programm in der Unit bräuchte, für einen
/// Test, der es dort zulässt (HUM-557): das Programm, sein Lader und seine
/// Bibliotheken.
#[cfg(test)]
#[allow(clippy::panic)]
pub(crate) fn exec_files_of(program: &Path) -> Vec<String> {
    program_files(program, &loader_list)
        .unwrap_or_else(|why| panic!("{why}"))
        .iter()
        .map(|file| file.to_string_lossy().into_owned())
        .collect()
}

/// Was die Vorschau als `ExecPaths=` zeigt: die Liste aus [`exec_paths`],
/// oder [`PREVIEW_PROGRAMS`], wenn sie sich nicht bilden lässt. Die
/// Vorschau lehnt nichts ab; der Plan derselben Sitzung scheiterte dann mit
/// dem Grund.
pub(crate) fn shown_in_preview(resolved: &Resolved, shim: &Path) -> Vec<String> {
    exec_paths(resolved, shim).unwrap_or_else(|_| vec![PREVIEW_PROGRAMS.to_owned()])
}

/// Der Platzhalter der Vorschau für eine Liste, die sich nicht bilden ließ.
pub(crate) const PREVIEW_PROGRAMS: &str = "<programs>";

/// Das erste ausführbare `name` im Suchpfad der Unit, wie die Shell des
/// Skripts es findet.
fn in_unit_path(name: &str) -> Option<PathBuf> {
    std::env::split_paths(UNIT_PATH)
        .map(|dir| dir.join(name))
        .find(|candidate| crate::agent::is_executable_file(candidate))
}

/// Ein Programm und, wenn es dynamisch gebunden ist, sein Lader und seine
/// Bibliotheken, jedes aufgelöst. `list` fragt den Lader nach den
/// Bibliotheken (in den Tests eine Nachbildung).
fn program_files(
    program: &Path,
    list: &dyn Fn(&Path, &Path) -> Result<String, String>,
) -> Result<Vec<PathBuf>, String> {
    let canonical = canonical(program)?;
    let mut files = vec![canonical.clone()];
    match start_of(&canonical)? {
        Start::Static => {}
        Start::NotElf => {
            return Err(format!(
                "{} is no ELF program (a script?); the unit of the kernel level runs only the \
                 programs it lists, and a script starts others",
                program.display()
            ));
        }
        Start::Dynamic(loader) => {
            let output = list(&loader, &canonical)?;
            files.push(canonical_path(&loader)?);
            for library in libraries(&output)
                .map_err(|why| format!("the libraries of {}: {why}", program.display()))?
            {
                files.push(canonical_path(&library)?);
            }
        }
    }
    Ok(files)
}

fn canonical(program: &Path) -> Result<PathBuf, String> {
    canonical_path(program).map_err(|why| format!("the program {why}"))
}

/// Der aufgelöste Pfad, nur wenn er eine gewöhnliche Datei ist. Ein Verweis,
/// der zur Zeit des Plans auf ein Verzeichnis zeigt (etwa eine Bibliothek,
/// die auf `/usr/bin` verweist), setzte sonst das ganze Verzeichnis auf die
/// Liste, und jedes Programm darin liefe.
fn canonical_path(path: &Path) -> Result<PathBuf, String> {
    let canonical =
        std::fs::canonicalize(path).map_err(|err| format!("{}: {err}", path.display()))?;
    let meta =
        std::fs::metadata(&canonical).map_err(|err| format!("{}: {err}", canonical.display()))?;
    if !meta.is_file() {
        return Err(format!(
            "{} resolves to {}, which is no regular file; the unit lists only files",
            path.display(),
            canonical.display()
        ));
    }
    Ok(canonical)
}

/// Der Lader aus dem Kopf eines ELF (64 Bit, little-endian, wie jedes
/// Programm, das dieses Backend auf `x86_64` startet).
fn start_of(path: &Path) -> Result<Start, String> {
    const PT_INTERP: u32 = 3;
    /// Mehr Programmköpfe hat kein Programm, das wir starten.
    const MAX_HEADERS: u64 = 256;
    let bytes = std::fs::File::open(path)
        .and_then(|file| {
            let mut bytes = Vec::new();
            file.take(1 << 20).read_to_end(&mut bytes)?;
            Ok(bytes)
        })
        .map_err(|err| format!("{}: {err}", path.display()))?;
    if bytes.get(..4) != Some(b"\x7fELF".as_slice()) {
        return Ok(Start::NotElf);
    }
    let unreadable = || {
        format!(
            "{} has an ELF header this backend cannot read",
            path.display()
        )
    };
    if bytes.get(4) != Some(&2) || bytes.get(5) != Some(&1) {
        return Err(unreadable());
    }
    let u16_at = |at: u64| -> Option<u64> {
        let at = usize::try_from(at).ok()?;
        Some(u64::from(u16::from_le_bytes(
            bytes.get(at..at.checked_add(2)?)?.try_into().ok()?,
        )))
    };
    let u32_at = |at: u64| -> Option<u32> {
        let at = usize::try_from(at).ok()?;
        Some(u32::from_le_bytes(
            bytes.get(at..at.checked_add(4)?)?.try_into().ok()?,
        ))
    };
    let u64_at = |at: u64| -> Option<u64> {
        let at = usize::try_from(at).ok()?;
        Some(u64::from_le_bytes(
            bytes.get(at..at.checked_add(8)?)?.try_into().ok()?,
        ))
    };
    let offset = u64_at(0x20).ok_or_else(unreadable)?;
    let size = u16_at(0x36).ok_or_else(unreadable)?;
    let count = u16_at(0x38).ok_or_else(unreadable)?;
    if count > MAX_HEADERS {
        return Err(unreadable());
    }
    for index in 0..count {
        let header = offset
            .checked_add(index.checked_mul(size).ok_or_else(unreadable)?)
            .ok_or_else(unreadable)?;
        if u32_at(header).ok_or_else(unreadable)? != PT_INTERP {
            continue;
        }
        let start = u64_at(header + 8).ok_or_else(unreadable)?;
        let length = u64_at(header + 32).ok_or_else(unreadable)?;
        let range = usize::try_from(start)
            .ok()
            .zip(usize::try_from(start.saturating_add(length)).ok())
            .and_then(|(from, to)| bytes.get(from..to))
            .ok_or_else(unreadable)?;
        let name = range.split(|byte| *byte == 0).next().unwrap_or_default();
        let name = std::str::from_utf8(name).map_err(|_| unreadable())?;
        if !name.starts_with('/') {
            return Err(unreadable());
        }
        return Ok(Start::Dynamic(PathBuf::from(name)));
    }
    Ok(Start::Static)
}

/// Die Ausgabe von `<loader> --list <program>`, leere Umgebung, keine
/// Eingabe. Der Lader bindet dabei nur die Bibliotheken und führt nichts
/// vom Programm aus.
fn loader_list(loader: &Path, program: &Path) -> Result<String, String> {
    let mut command = std::process::Command::new(loader);
    command
        .arg("--list")
        .arg(program)
        .env_clear()
        .env("LC_ALL", "C");
    let output = crate::user_unit::run_bounded(command, LIST_TIMEOUT)
        .map_err(|err| format!("{} --list {}: {err}", loader.display(), program.display()))?;
    if !output.status.success() {
        return Err(format!(
            "{} --list {} ended with {}: {}",
            loader.display(),
            program.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Die Bibliotheken aus der Liste des Laders (glibc und musl schreiben
/// dieselbe Form): `name => /pfad (0x...)` oder `/pfad (0x...)`; ein Eintrag
/// ohne Pfad (`linux-vdso.so.1`) ist keine Datei. Eine Bibliothek, die der
/// Lader nicht findet, ist ein Fehler.
fn libraries(output: &str) -> Result<Vec<PathBuf>, String> {
    let mut found = Vec::new();
    for line in output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let (name, target) = match line.split_once("=>") {
            Some((name, target)) => (name.trim(), target.trim()),
            None => (line, line),
        };
        if target.starts_with("not found") {
            return Err(format!("{name} not found"));
        }
        let path = target
            .rsplit_once(" (0x")
            .map_or(target, |(path, _)| path)
            .trim();
        if path.starts_with('/') {
            found.push(PathBuf::from(path));
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    const GLIBC: &str = "\tlinux-vdso.so.1 (0x00007ffd3b1f2000)\n\
        \tlibseccomp.so.2 => /lib/x86_64-linux-gnu/libseccomp.so.2 (0x00007f3c1a2e0000)\n\
        \tlibc.so.6 => /lib/x86_64-linux-gnu/libc.so.6 (0x00007f3c1a000000)\n\
        \t/lib64/ld-linux-x86-64.so.2 (0x00007f3c1a330000)\n";

    const MUSL: &str = "\t/lib/ld-musl-x86_64.so.1 (0x7f0e8c5d4000)\n\
        \tlibc.musl-x86_64.so.1 => /lib/ld-musl-x86_64.so.1 (0x7f0e8c5d4000)\n";

    /// Die Liste des Laders: jede Bibliothek mit Pfad, nie der vDSO; eine
    /// fehlende ist ein Fehler.
    #[test]
    fn the_loader_list_names_every_library_with_a_path() {
        assert_eq!(
            libraries(GLIBC).unwrap(),
            [
                "/lib/x86_64-linux-gnu/libseccomp.so.2",
                "/lib/x86_64-linux-gnu/libc.so.6",
                "/lib64/ld-linux-x86-64.so.2",
            ]
            .map(PathBuf::from)
        );
        assert_eq!(
            libraries(MUSL).unwrap(),
            ["/lib/ld-musl-x86_64.so.1", "/lib/ld-musl-x86_64.so.1"].map(PathBuf::from)
        );
        assert_eq!(
            libraries("\tlibfoo.so.1 => not found\n").unwrap_err(),
            "libfoo.so.1 not found"
        );
    }

    /// Ein minimales ELF64: Kopf, ein Programmkopf vom Typ `kind`, danach
    /// der Name des Laders.
    fn elf(kind: u32, loader: &str) -> Vec<u8> {
        let mut bytes = vec![0_u8; 64 + 56];
        bytes[..4].copy_from_slice(b"\x7fELF");
        bytes[4] = 2;
        bytes[5] = 1;
        bytes[0x20..0x28].copy_from_slice(&64_u64.to_le_bytes());
        bytes[0x36..0x38].copy_from_slice(&56_u16.to_le_bytes());
        bytes[0x38..0x3a].copy_from_slice(&1_u16.to_le_bytes());
        bytes[64..68].copy_from_slice(&kind.to_le_bytes());
        let name = format!("{loader}\0");
        let at = bytes.len() as u64;
        bytes[64 + 8..64 + 16].copy_from_slice(&at.to_le_bytes());
        bytes[64 + 32..64 + 40].copy_from_slice(&(name.len() as u64).to_le_bytes());
        bytes.extend_from_slice(name.as_bytes());
        bytes
    }

    /// Der Kopf sagt, ob und mit welchem Lader ein Programm startet; ein
    /// Skript ist kein ELF, ein abgeschnittener Kopf ein Fehler.
    #[test]
    fn the_header_names_the_loader() {
        let dir = tempfile::tempdir().unwrap();
        let write = |name: &str, bytes: &[u8]| {
            let path = dir.path().join(name);
            std::fs::write(&path, bytes).unwrap();
            path
        };
        assert_eq!(
            start_of(&write("dynamic", &elf(3, "/lib64/ld-linux-x86-64.so.2"))).unwrap(),
            Start::Dynamic(PathBuf::from("/lib64/ld-linux-x86-64.so.2"))
        );
        assert_eq!(
            start_of(&write("static", &elf(1, "unused"))).unwrap(),
            Start::Static
        );
        assert_eq!(
            start_of(&write("script", b"#!/bin/sh\nexec qemu \"$@\"\n")).unwrap(),
            Start::NotElf
        );
        assert!(start_of(&write("cut", &elf(3, "/x")[..40])).is_err());
        assert!(start_of(&write("relative", &elf(3, "ld.so"))).is_err());
    }

    /// Ein dynamisches Programm bringt Lader und Bibliotheken mit, jedes
    /// aufgelöst; ein statisches nur sich selbst; ein Skript wird
    /// abgelehnt. Der Lader ist hier eine Nachbildung.
    #[test]
    fn a_program_brings_its_loader_and_libraries() {
        let dir = tempfile::tempdir().unwrap();
        let path = |name: &str| dir.path().join(name);
        std::fs::write(path("ld.so"), "").unwrap();
        std::fs::write(path("libc.so.6"), "").unwrap();
        std::os::unix::fs::symlink(path("libc.so.6"), path("libc-link.so")).unwrap();
        std::fs::write(
            path("program"),
            elf(3, &path("ld.so").display().to_string()),
        )
        .unwrap();
        std::os::unix::fs::symlink(path("program"), path("program-link")).unwrap();
        let listed = format!(
            "\tlinux-vdso.so.1 (0x1)\n\tlibc.so.6 => {} (0x2)\n",
            path("libc-link.so").display()
        );
        let list = |loader: &Path, program: &Path| {
            assert_eq!(loader, path("ld.so"));
            assert_eq!(program, std::fs::canonicalize(path("program")).unwrap());
            Ok(listed.clone())
        };
        let canonical = |name: &str| std::fs::canonicalize(path(name)).unwrap();
        assert_eq!(
            program_files(&path("program-link"), &list).unwrap(),
            [
                canonical("program"),
                canonical("ld.so"),
                canonical("libc.so.6")
            ]
        );

        std::fs::write(path("static"), elf(1, "")).unwrap();
        assert_eq!(
            program_files(&path("static"), &|_: &Path, _: &Path| panic!("no loader")).unwrap(),
            [canonical("static")]
        );

        std::fs::write(path("wrapper"), "#!/bin/sh\n").unwrap();
        let refused = program_files(&path("wrapper"), &list).unwrap_err();
        assert!(refused.contains("no ELF program"), "{refused}");

        // Eine Bibliothek, ein Lader oder ein Programm, das auf ein
        // Verzeichnis verweist, setzte das Verzeichnis auf die Liste.
        std::os::unix::fs::symlink(dir.path(), path("libdir.so")).unwrap();
        let to_dir = format!("\tlibdir.so => {} (0x2)\n", path("libdir.so").display());
        let refused =
            program_files(&path("program"), &|_: &Path, _: &Path| Ok(to_dir.clone())).unwrap_err();
        assert!(refused.contains("no regular file"), "{refused}");
        std::fs::write(
            path("dir-loader"),
            elf(3, &path("libdir.so").display().to_string()),
        )
        .unwrap();
        let refused = program_files(&path("dir-loader"), &|_: &Path, _: &Path| Ok(String::new()))
            .unwrap_err();
        assert!(refused.contains("no regular file"), "{refused}");
        let refused = program_files(&path("libdir.so"), &list).unwrap_err();
        assert!(refused.contains("no regular file"), "{refused}");

        let missing = |_: &Path, _: &Path| Ok("\tlibgone.so => not found\n".to_owned());
        let refused = program_files(&path("program"), &missing).unwrap_err();
        assert!(refused.contains("libgone.so not found"), "{refused}");
    }

    /// Die Vorschau zeigt die Liste, wo sie sich bilden lässt, sonst den
    /// Platzhalter; der Plan bekäme dann den Fehler. Mutationsbeweis: eine
    /// Vorschau, die immer den Platzhalter zeigt, oder eine, die einen
    /// Fehler verschluckt und eine leere Liste zeigt, färbt den Test rot.
    #[test]
    fn the_preview_shows_the_list_or_its_placeholder() {
        let missing = Resolved {
            emulator: PathBuf::from("/nonexistent-hum480/emulator"),
            file_daemon: PathBuf::from("/nonexistent-hum480/files"),
            vsock_device: PathBuf::from("/nonexistent-hum480/vsock"),
            bubblewrap: PathBuf::from("/nonexistent-hum480/bubblewrap"),
            kernel: PathBuf::from("/nonexistent-hum480/kernel"),
            modules: PathBuf::from("/nonexistent-hum480/modules"),
            no_announce_submounts: true,
        };
        let shell = PathBuf::from(lines::SCRIPT_SHELL);
        assert_eq!(shown_in_preview(&missing, &shell), [PREVIEW_PROGRAMS]);
        assert!(exec_paths(&missing, &shell).is_err());
        // Jede Rolle mit der Shell besetzt: Programme, die es hier gibt.
        let present = Resolved {
            emulator: shell.clone(),
            file_daemon: shell.clone(),
            vsock_device: shell.clone(),
            bubblewrap: shell.clone(),
            ..missing
        };
        let shown = shown_in_preview(&present, &shell);
        let canonical = std::fs::canonicalize(&shell).unwrap();
        assert!(
            shown.iter().any(|file| Path::new(file) == canonical),
            "{shown:?}"
        );
        assert_eq!(shown, exec_paths(&present, &shell).unwrap());
    }

    /// Auf diesem Rechner: Die Shell des Skripts und `sleep` stehen mit
    /// ihrem Lader und der C-Bibliothek auf der Liste, die der Lader selbst
    /// nennt; kein Verzeichnis steht darauf.
    #[test]
    fn the_shell_of_the_script_comes_with_its_libraries() {
        let sleep = in_unit_path(SCRIPT_SLEEP).expect("sleep in the unit's path");
        for program in [Path::new(lines::SCRIPT_SHELL), &sleep] {
            let files = program_files(program, &loader_list).unwrap();
            assert_eq!(files[0], std::fs::canonicalize(program).unwrap());
            if matches!(start_of(&files[0]).unwrap(), Start::Dynamic(_)) {
                assert!(
                    files.iter().any(|file| file
                        .file_name()
                        .is_some_and(|name| name.to_string_lossy().starts_with("libc."))),
                    "{files:?}"
                );
            }
            assert!(files.iter().all(|file| file.is_file()), "{files:?}");
        }
    }

    /// HUM-557: Ein Gefängnis verdeckt jedes andere Programm der Unit unter
    /// `/usr`, unter seinem aufgelösten Pfad; den eigenen Helfer nie, auch
    /// wenn er über einen Verweis eingestellt ist, und kein Programm
    /// außerhalb von `/usr`, das es gar nicht sieht.
    ///
    /// Mutationsbeweis: Vergleicht [`hidden_in_jail`] den eingestellten Pfad
    /// statt des aufgelösten, verdeckt das Gefängnis von virtiofsd seinen
    /// eigenen Helfer, und der Test wird rot.
    #[test]
    fn a_jail_hides_every_other_program_of_the_unit() {
        let program = |configured: &str, canonical: &str| Program {
            configured: PathBuf::from(configured),
            canonical: PathBuf::from(canonical),
        };
        let programs = [
            program("/usr/bin/env", "/usr/bin/env"),
            program("/opt/humanitl/humanitl-shim", "/opt/humanitl/humanitl-shim"),
            program("/bin/sh", "/usr/bin/dash"),
            program("/usr/bin/sleep", "/usr/bin/sleep"),
            program("/usr/bin/bwrap", "/usr/bin/bwrap"),
            program("/usr/lib/qemu/virtiofsd", "/usr/libexec/virtiofsd"),
            program("/usr/bin/vhost-device-vsock", "/usr/bin/vhost-device-vsock"),
            program("/usr/bin/qemu-system-x86_64", "/usr/bin/qemu-system-x86_64"),
        ];
        assert_eq!(
            hidden_in_jail(&programs, Path::new("/usr/lib/qemu/virtiofsd")),
            [
                "/usr/bin/bwrap",
                "/usr/bin/dash",
                "/usr/bin/env",
                "/usr/bin/qemu-system-x86_64",
                "/usr/bin/sleep",
                "/usr/bin/vhost-device-vsock",
            ]
            .map(PathBuf::from)
        );
        let machine = hidden_in_jail(&programs, Path::new("/usr/bin/qemu-system-x86_64"));
        assert!(machine.contains(&PathBuf::from("/usr/libexec/virtiofsd")));
        assert!(!machine.contains(&PathBuf::from("/usr/bin/qemu-system-x86_64")));
        assert!(!machine.iter().any(|path| path.starts_with("/opt")));
    }
}
