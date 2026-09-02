//! Vom Profil zur Kommandozeile.
//!
//! [`SandboxProfile::to_bwrap_args`] erzeugt aus Profil und [`SessionContext`]
//! genau die Argumente, mit denen `bwrap` später startet — nichts wird beim
//! Start noch dazugetan. Deshalb ist die Zeile, die die Oberfläche unter
//! „Sandbox" zeigt (`humanitl sandbox argv`), die Wahrheit und nicht eine
//! Beschreibung davon.
//!
//! Die Reihenfolge steht fest (HUM-010) und wird von
//! `tests/snapshots/default.argv.txt` festgehalten:
//!
//! 1. `--unshare-*` in der festen Reihenfolge von [`Namespace::ALL`], nicht
//!    der des Profils, damit ein Profil die Flags weder umordnen noch doppeln
//!    kann,
//! 2. `--die-with-parent`, `--new-session`, `--cap-drop ALL`, `--hostname`;
//!    die ersten drei stehen immer da, das Profil kann sie nicht abwählen,
//! 3. `--ro-bind` je `mounts.ro`, dann die `--symlink`,
//! 4. `--proc`, `--dev`, dann die `--tmpfs` außerhalb von `/work`,
//! 5. `mounts.extra_ro` und `mounts.extra_rw`, die Erweiterungen des Nutzers,
//! 6. der Bind des Projektverzeichnisses, dann die `--tmpfs` darunter,
//! 7. Proxy-Socket, CA, Shim, dann die Maskierungen
//!    ([`SandboxProfile::effective_masked_files`]: die Pflichteinträge zuerst),
//! 8. `--clearenv`, die `--setenv` alphabetisch,
//! 9. `--chdir`, `--`, der Shim mit `--proxy-port`, `--`, der Befehl.
//!
//! Der Grund für die Reihenfolge: `bwrap` arbeitet die Argumente der Reihe nach
//! ab, ein späterer Mount verdeckt einen früheren. Ein `--tmpfs
//! /work/.git/hooks` vor dem Bind von `/work` wäre danach wieder verdeckt. Und
//! die Erweiterungen des Nutzers stehen vor allem, was die Sitzung einhängt:
//! ein `extra_ro = ["/usr"]` darf den Shim unter `/usr/local/bin` nicht
//! verdecken, sonst zeigt die Zeile eine Sandbox, die so nicht läuft.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::Path;

use humanitl_config::WorkMode;

use crate::profile::{Namespace, SandboxProfile, SessionContext};

/// Zeichen, die eine Shell unverändert liest und die deshalb ohne
/// Anführungszeichen auskommen.
fn is_safe(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(
            c,
            '_' | '@' | '%' | '+' | '=' | ':' | ',' | '.' | '/' | '-'
        )
}

impl SandboxProfile {
    /// Die vollständige Argumentliste für `bwrap`, ohne `bwrap` selbst.
    ///
    /// Endet mit `--`, dem Shim in der Sandbox, seinem `--proxy-port`, einem
    /// weiteren `--` und dem Befehl aus dem [`SessionContext`].
    #[must_use]
    pub fn to_bwrap_args(&self, ctx: &SessionContext) -> Vec<OsString> {
        let mut args = Args::default();

        // Die feste Reihenfolge, nicht die des Profils: `parse` verlangt, dass
        // alle sechs genannt sind, also ist die Liste hier vollständig.
        for namespace in Namespace::ALL {
            args.flag(namespace.flag());
        }
        // Nicht abschaltbar, auch nicht über die Felder des Profils: eine
        // Sandbox, die den Daemon überlebt oder ins Terminal des Nutzers
        // schreibt, ist keine, und eine mit Capabilities auch nicht
        // (`backlog/CONVENTIONS.md` 4.10, 4.11).
        args.flag("--die-with-parent");
        args.flag("--new-session");
        args.pair("--cap-drop", "ALL");
        args.pair("--hostname", &self.sandbox.hostname);

        for path in &self.mounts.ro {
            args.bind("--ro-bind", path, path);
        }
        for link in &self.mounts.symlinks {
            args.bind("--symlink", Path::new(&link.target), &link.link);
        }
        if let Some(proc) = self.mounts.proc.as_deref() {
            args.path("--proc", proc);
        }
        if let Some(dev) = self.mounts.dev.as_deref() {
            args.path("--dev", dev);
        }

        let work_dst = self.mounts.work.dst.as_path();
        let (under_work, elsewhere): (Vec<_>, Vec<_>) = self
            .mounts
            .tmpfs
            .iter()
            .partition(|path| path.starts_with(work_dst) && path.as_path() != work_dst);
        for path in elsewhere {
            args.path("--tmpfs", path);
        }

        // Die Erweiterungen des Nutzers kommen vor dem, was die Sitzung
        // einhängt: was danach kommt, liegt obenauf und kann nicht verdeckt
        // werden.
        for path in &self.mounts.extra_ro {
            args.bind("--ro-bind", path, path);
        }
        for path in &self.mounts.extra_rw {
            args.bind("--bind", path, path);
        }

        let work_flag = match self.effective_work_mode(ctx.work_mode) {
            WorkMode::Ro => "--ro-bind",
            WorkMode::Rw => "--bind",
        };
        args.bind(work_flag, &ctx.work_src, work_dst);
        for path in under_work {
            args.path("--tmpfs", path);
        }

        // Nur lesbar eingehängt: `connect(2)` auf einen Unix-Socket bleibt
        // erlaubt, weil der Kernel `EROFS` nur für reguläre Dateien,
        // Verzeichnisse und Symlinks liefert.
        args.bind(
            "--ro-bind",
            &ctx.proxy_socket_src,
            &self.network.proxy_socket_dst,
        );
        args.bind("--ro-bind", &ctx.ca_cert_src, &self.network.ca_cert_dst);
        args.bind("--ro-bind", &ctx.shim_src, &self.network.shim_dst);

        // Platzhalter bis HUM-011: eine leere, nur lesbare Datei über der
        // Quelle. `--file FD` bräuchte einen offenen Deskriptor aus dem
        // LaunchPlan; `/dev/null` tut dasselbe, solange niemand die Datei liest.
        // Die Pflichteinträge kommen immer, auch bei `masked_files = []`.
        for path in self.effective_masked_files() {
            args.bind("--ro-bind", Path::new("/dev/null"), &path);
        }

        args.flag("--clearenv");
        for (key, value) in &self.env {
            args.setenv(key, value);
        }

        args.path("--chdir", work_dst);
        args.flag("--");
        args.push(&self.network.shim_dst);
        args.pair("--proxy-port", self.network.proxy_port.to_string());
        args.flag("--");
        for part in &ctx.command {
            args.push(part);
        }

        args.0
    }

    /// Dieselbe Liste als eine Zeile, wie eine Shell sie lesen würde.
    ///
    /// Für die Anzeige und für `humanitl sandbox argv`. Zitiert wird nach POSIX
    /// mit einfachen Anführungszeichen; `shlex::split` der Ausgabe ergibt wieder
    /// genau [`SandboxProfile::to_bwrap_args`].
    ///
    /// Das Programm selbst steht nicht in der Zeile, so wie es auch nicht in der
    /// Liste steht: welche `bwrap`-Datei gestartet wird, entscheidet der
    /// Launcher (HUM-011) beim Auflösen über `PATH`. Wer die Zeile anzeigt,
    /// schreibt `bwrap ` davor.
    ///
    /// Ein Argument, das kein gültiges UTF-8 ist, erscheint in der Zeile ersetzt
    /// (`to_string_lossy`). Die Zeile ist Anzeige, nicht Ausführung: gestartet
    /// wird immer die Liste.
    #[must_use]
    pub fn argv_line(&self, ctx: &SessionContext) -> String {
        self.to_bwrap_args(ctx)
            .iter()
            .map(|arg| shell_quote(&arg.to_string_lossy()))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Die Umgebung, die das Profil in der Sandbox setzt.
    ///
    /// Dieselben Paare, die `--setenv` in der Kommandozeile trägt; für
    /// `LaunchPlan.env` in HUM-011.
    #[must_use]
    pub fn env_pairs(&self) -> &BTreeMap<String, String> {
        &self.env
    }
}

/// Setzt ein Argument so in einfache Anführungszeichen, dass `sh` es wieder
/// als genau ein Wort liest.
fn shell_quote(arg: &str) -> String {
    if !arg.is_empty() && arg.chars().all(is_safe) {
        return arg.to_owned();
    }
    let mut out = String::with_capacity(arg.len() + 2);
    out.push('\'');
    for c in arg.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

/// Sammelt die Argumente und hält die Schreibweise an einer Stelle.
#[derive(Debug, Default)]
struct Args(Vec<OsString>);

impl Args {
    fn push(&mut self, value: impl AsRef<OsStr>) {
        self.0.push(value.as_ref().to_os_string());
    }

    fn flag(&mut self, flag: &str) {
        self.push(flag);
    }

    fn pair(&mut self, flag: &str, value: impl AsRef<OsStr>) {
        self.push(flag);
        self.push(value);
    }

    fn path(&mut self, flag: &str, path: &Path) {
        self.push(flag);
        self.push(path);
    }

    fn bind(&mut self, flag: &str, src: &Path, dst: &Path) {
        self.push(flag);
        self.push(src);
        self.push(dst);
    }

    fn setenv(&mut self, key: &str, value: &str) {
        self.push("--setenv");
        self.push(key);
        self.push(value);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::shell_quote;

    #[test]
    fn quoting_leaves_ordinary_words_alone() {
        assert_eq!(shell_quote("--ro-bind"), "--ro-bind");
        assert_eq!(shell_quote("/etc/ssl"), "/etc/ssl");
        assert_eq!(shell_quote("http://127.0.0.1:3128"), "http://127.0.0.1:3128");
    }

    #[test]
    fn quoting_wraps_what_a_shell_would_read_differently() {
        assert_eq!(shell_quote(""), "''");
        assert_eq!(shell_quote("echo hello"), "'echo hello'");
        assert_eq!(shell_quote("a*b"), "'a*b'");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
    }
}
