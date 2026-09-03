//! Der Vertrag zwischen Launcher und Shim (HUM-011, HUM-012, HUM-013).
//!
//! Der Launcher auf dem Host und der Shim in der Sandbox teilen sich keinen
//! Code: der Shim hat außer `libc` und `seccompiler` keine Abhängigkeit und
//! kennt diese Crate nicht. Was er wissen muss, bekommt er über die Umgebung,
//! die `bwrap` nach `--clearenv` mit `--setenv` aufbaut, und über einen
//! geerbten Deskriptor. Dieses Modul ist die eine Stelle, an der der Vertrag
//! steht; der Shim (`daemon/bin/humanitl-shim/src/main.rs`) zitiert ihn.
//!
//! # Die Argumentliste
//!
//! Die `bwrap`-Argumente kommen aus [`SandboxProfile::to_bwrap_args`] und enden
//! mit `-- <shim_dst> --proxy-port <port> -- <command...>`, wobei `shim_dst`
//! [`crate::profile::SHIM_DST`] ist (`/run/humanitl/humanitl-shim`). Der Launcher hängt drei
//! Dateien ein, jede nur lesbar: den Shim selbst nach `shim_dst`, die
//! Proxy-Socket-DATEI (nie ihr Verzeichnis) nach
//! `/run/humanitl/proxy.sock` und das CA-Zertifikat nach
//! `/etc/humanitl/ca.crt`.
//!
//! # Die Umgebung des Shims
//!
//! | Variable | Inhalt |
//! |---|---|
//! | [`ENV_BRIDGES`] | JSON-Liste aus `[network].bridges`: `[{"name":"proxy","dir":"in","listen":"127.0.0.1:3128","socket":"/run/humanitl/proxy.sock"}]` |
//! | [`ENV_SECCOMP_FAMILIES`] | `[seccomp].allow_families` als Kommaliste, Standard `AF_INET,AF_INET6` |
//! | [`ENV_SECCOMP_TYPES`] | `[seccomp].allow_types` als Kommaliste, Standard `SOCK_STREAM` |
//! | [`ENV_SECCOMP_DENY`] | `[seccomp].deny_syscalls` als Kommaliste |
//! | [`ENV_REPORT_FD`] | optional: die Nummer eines geerbten Deskriptors (Schreibende einer Pipe), auf den der Shim seinen Bericht schreibt; fehlt sie, gibt es keinen Bericht |
//!
//! Die Werte stammen ausschließlich aus dem Profil, nie aus der Umgebung des
//! Hosts; ein `[env]`-Eintrag des Profils mit demselben Namen wird vom
//! Launcher überschrieben ([`RESERVED_ENV`]).
//!
//! # Was der Shim tut, in dieser Reihenfolge
//!
//! ```text
//! humanitl-shim --proxy-port <n> -- <command> [args...]
//! ```
//!
//! 1. Argumente lesen.
//! 2. Für jede Bridge mit `dir = "in"`: im Shim-Prozess einen TCP-Listener auf
//!    `listen` binden und je angenommener Verbindung einen Thread starten, der
//!    sich mit dem Unix-Socket `socket` verbindet und Bytes in beide Richtungen
//!    kopiert. Kein tokio. `dir = "out"` beendet mit 126 und der Meldung
//!    `bridge direction out not supported yet`.
//! 3. Prüfen, dass der Listener eine Verbindung annimmt (Selbstverbindung),
//!    dass außer `lo` kein Interface existiert (`/sys/class/net` oder
//!    `/proc/net/dev`) und dass im Dateisystem der Sandbox kein Unix-Socket
//!    liegt außer denen der Brücken (begrenzter Suchlauf, siehe
//!    [`CHECK_SINGLE_SOCKET`]).
//! 4. `fork`. Das KIND setzt `PR_SET_NO_NEW_PRIVS`, baut den seccomp-Filter mit
//!    seccompiler (alles erlaubt; `socket()` nur, wenn `arg0` in den Familien
//!    UND `arg1 & 0xff` in den Typen liegt, sonst `EPERM`; `socketpair()`
//!    unberührt; jeder Name aus [`ENV_SECCOMP_DENY`] `EPERM`; falsche
//!    Architektur beendet den Prozess; ein handgeschriebenes BPF-Präludium vor
//!    dem seccompiler-Programm antwortet `EPERM` auf `nr & 0x40000000`, den
//!    x32-Aufrufen), lädt ihn mit TSYNC und ruft `execvp(command)`. Der ELTERN
//!    hält die Bridge-Threads am Leben, wartet auf das Kind und endet mit
//!    dessen Status (Signal: 128 + Nummer).
//! 5. Exit-Codes: 125 Gebrauchsfehler, 126 Bridge oder seccomp ließ sich nicht
//!    einrichten (die Meldung sagt, was), 127 `exec` gescheitert, sonst der
//!    Code des Kindes ([`EXIT_USAGE`], [`EXIT_SETUP`], [`EXIT_EXEC`]).
//!
//! # Der Bericht
//!
//! Vor `exec` schreibt der Shim je Prüfung eine Zeile auf [`ENV_REPORT_FD`]:
//!
//! ```text
//! CHECK <name> <ok|fail> <evidence>
//! ```
//!
//! mit den Namen aus [`CHECK_NAMES`]: `bridge_listening`, `single_socket`,
//! `seccomp_applied`, `families`, `no_interfaces`. [`parse_check_line`] liest
//! sie, der Isolation-Check des Backends
//! ([`crate::SandboxBackend::isolation_check`]) ordnet sie den drei Garantien
//! zu. Zwei Dinge muss der Shim einhalten, damit der Bericht etwas wert ist:
//! Er schreibt jede Zeile, bevor der Agent läuft, und er schließt den
//! Deskriptor vor `exec` (oder setzt `FD_CLOEXEC`), damit der Agent keine
//! eigenen Zeilen nachschieben kann. Der Launcher wartet nicht auf das Ende
//! der Pipe, sondern auf die fünf Namen; `bwrap` selbst erbt die Schreibseite
//! und hält sie, solange die Sandbox läuft.

use std::os::fd::RawFd;

use crate::profile::{Bridge, SandboxProfile};

/// Die Bridges als JSON-Liste, siehe Modulbeschreibung.
pub const ENV_BRIDGES: &str = "HUMANITL_BRIDGES";

/// Erlaubte Socket-Familien für `socket(2)`, Kommaliste.
pub const ENV_SECCOMP_FAMILIES: &str = "HUMANITL_SECCOMP_FAMILIES";

/// Erlaubte Socket-Typen für `socket(2)` (`arg1 & 0xff`), Kommaliste.
pub const ENV_SECCOMP_TYPES: &str = "HUMANITL_SECCOMP_TYPES";

/// Syscalls, die `EPERM` liefern, Kommaliste.
pub const ENV_SECCOMP_DENY: &str = "HUMANITL_SECCOMP_DENY";

/// Der geerbte Deskriptor für den Bericht; fehlt er, gibt es keinen.
pub const ENV_REPORT_FD: &str = "HUMANITL_REPORT_FD";

/// Die Namen, die der Launcher setzt und die kein Profil belegen kann.
pub const RESERVED_ENV: &[&str] = &[
    ENV_BRIDGES,
    ENV_SECCOMP_FAMILIES,
    ENV_SECCOMP_TYPES,
    ENV_SECCOMP_DENY,
    ENV_REPORT_FD,
];

/// Die Prüfungen, die der Shim meldet, in der Reihenfolge seiner Schritte.
pub const CHECK_NAMES: [&str; 5] = [
    CHECK_BRIDGE_LISTENING,
    CHECK_SINGLE_SOCKET,
    CHECK_SECCOMP_APPLIED,
    CHECK_FAMILIES,
    CHECK_NO_INTERFACES,
];

/// Der Listener der Bridge nimmt eine Verbindung an.
pub const CHECK_BRIDGE_LISTENING: &str = "bridge_listening";

/// Im Dateisystem der Sandbox liegt kein Unix-Socket außer dem der Bridge.
///
/// Der Shim läuft dafür vor dem `exec` einen begrenzten Suchlauf über das
/// Dateisystem, ohne `/proc`, `/sys` und `/dev`, ohne Symlinks zu folgen, mit
/// denselben Schranken wie die Profil-Prüfung auf dem Host
/// ([`crate::SOCKET_WALK_MAX_DEPTH`], [`crate::SOCKET_WALK_MAX_ENTRIES`]). Die
/// Evidenz nennt jeden gefundenen Socket und sagt, ob eine Schranke griff.
///
/// Das ist der Beweis für die zweite Garantie: `bridge_listening` zeigt nur,
/// dass die eine Tür offen ist und antwortet, nicht, dass es keine zweite gibt
/// (Review-Befund vom 2026-09-03).
pub const CHECK_SINGLE_SOCKET: &str = "single_socket";

/// Der Filter ist geladen (`Seccomp: 2` im Kind).
pub const CHECK_SECCOMP_APPLIED: &str = "seccomp_applied";

/// Eine Familie außerhalb der Liste antwortet `EPERM`.
pub const CHECK_FAMILIES: &str = "families";

/// Außer `lo` gibt es kein Interface.
pub const CHECK_NO_INTERFACES: &str = "no_interfaces";

/// Das Präfix jeder Berichtszeile.
pub const CHECK_PREFIX: &str = "CHECK";

/// Exit-Code des Shims bei einem Gebrauchsfehler.
pub const EXIT_USAGE: i32 = 125;

/// Exit-Code des Shims, wenn Bridge oder seccomp nicht eingerichtet werden konnten.
pub const EXIT_SETUP: i32 = 126;

/// Exit-Code des Shims, wenn `execvp` scheiterte.
pub const EXIT_EXEC: i32 = 127;

/// Eine Zeile des Berichts, gelesen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShimCheck {
    /// Der Name aus [`CHECK_NAMES`]; ein fremder Name wird mitgeführt, nicht
    /// verworfen, damit ein neuerer Shim nichts verliert.
    pub name: String,
    /// `ok` oder nicht.
    pub ok: bool,
    /// Was der Shim gesehen hat, Freitext bis zum Zeilenende.
    pub evidence: String,
}

/// Liest eine Zeile `CHECK <name> <ok|fail> <evidence>`.
///
/// Alles andere ergibt `None`: eine leere Zeile, eine Zeile ohne das Präfix,
/// ein drittes Wort, das weder `ok` noch `fail` ist. Die Evidenz darf fehlen.
#[must_use]
pub fn parse_check_line(line: &str) -> Option<ShimCheck> {
    let line = line.trim_end_matches(['\r', '\n']);
    let mut words = line.splitn(4, ' ');
    if words.next()? != CHECK_PREFIX {
        return None;
    }
    let name = words.next().filter(|name| !name.is_empty())?;
    let ok = match words.next()? {
        "ok" => true,
        "fail" => false,
        _ => return None,
    };
    let evidence = words.next().unwrap_or_default().trim().to_owned();
    Some(ShimCheck {
        name: name.to_owned(),
        ok,
        evidence,
    })
}

/// Die Bridges als JSON, wie [`ENV_BRIDGES`] sie trägt.
///
/// Feldreihenfolge `name`, `dir`, `listen`, `socket`, ohne Leerraum; die
/// Zeile ist Teil der Kommandozeile, die die Oberfläche zeigt, und soll dort
/// stabil aussehen.
#[must_use]
pub fn bridges_json(bridges: &[Bridge]) -> String {
    // Die Typen sind `String`, ein Enum, `SocketAddr` und `PathBuf`: nichts
    // davon kann beim Serialisieren scheitern, außer ein Pfad wäre kein UTF-8,
    // und der käme aus einer TOML-Datei, die es dann nicht wäre.
    serde_json::to_string(bridges).unwrap_or_else(|_| "[]".to_owned())
}

/// Die Paare, die der Launcher zusätzlich zum `[env]` des Profils setzt.
///
/// `report_fd` ist die Nummer der geerbten Schreibseite; ohne sie fehlt
/// [`ENV_REPORT_FD`], und der Shim schreibt keinen Bericht.
///
/// Was hier an den Shim geht, ist die Politik des Profils, und die ist beim
/// Lesen auf ihren Boden festgelegt worden: `allow_families` und
/// `allow_types` sind nach [`crate::SandboxProfile::parse`] genau
/// [`crate::REQUIRED_SOCKET_FAMILIES`] und [`crate::REQUIRED_SOCKET_TYPES`]
/// (plus `AF_UNIX` unter [`crate::SocketFloor::BrowserUnixIpc`]), und
/// `network.bridges` ist genau die Proxy-Bridge. Ein Profil kann die Listen
/// hier also nicht mehr aufweiten; diese Funktion gibt weiter, was geprüft
/// wurde, sie prüft nicht selbst.
#[must_use]
pub fn shim_env(profile: &SandboxProfile, report_fd: Option<RawFd>) -> Vec<(String, String)> {
    let families = profile
        .seccomp
        .allow_families
        .iter()
        .map(|family| family.as_str())
        .collect::<Vec<_>>()
        .join(",");
    let types = profile
        .seccomp
        .allow_types
        .iter()
        .map(|kind| kind.as_str())
        .collect::<Vec<_>>()
        .join(",");
    let deny = profile.seccomp.deny_syscalls.join(",");

    let mut out = vec![
        (
            ENV_BRIDGES.to_owned(),
            bridges_json(&profile.network.bridges),
        ),
        (ENV_SECCOMP_FAMILIES.to_owned(), families),
        (ENV_SECCOMP_TYPES.to_owned(), types),
        (ENV_SECCOMP_DENY.to_owned(), deny),
    ];
    if let Some(fd) = report_fd {
        out.push((ENV_REPORT_FD.to_owned(), fd.to_string()));
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::path::Path;

    use super::{
        CHECK_NAMES, ENV_BRIDGES, ENV_REPORT_FD, ENV_SECCOMP_DENY, ENV_SECCOMP_FAMILIES,
        ENV_SECCOMP_TYPES, RESERVED_ENV, ShimCheck, bridges_json, parse_check_line, shim_env,
    };
    use crate::profile::{Bridge, SandboxProfile};

    #[test]
    fn the_proxy_bridge_serializes_in_the_documented_shape() {
        assert_eq!(
            bridges_json(&[Bridge::proxy()]),
            r#"[{"name":"proxy","dir":"in","listen":"127.0.0.1:3128","socket":"/run/humanitl/proxy.sock"}]"#
        );
        assert_eq!(bridges_json(&[]), "[]");
    }

    #[test]
    fn the_shim_env_carries_the_profile_and_only_the_profile() {
        let profile = SandboxProfile::parse("version = 1\nname = \"x\"\n", Path::new("<t>"))
            .expect("minimal profile");
        let env = shim_env(&profile, Some(11));
        let get = |key: &str| {
            env.iter()
                .find(|(k, _)| k == key)
                .map_or_else(|| panic!("{key} missing"), |(_, v)| v.as_str())
        };
        assert!(get(ENV_BRIDGES).starts_with("[{\"name\":\"proxy\""));
        assert_eq!(get(ENV_SECCOMP_FAMILIES), "AF_INET,AF_INET6");
        assert_eq!(get(ENV_SECCOMP_TYPES), "SOCK_STREAM");
        assert!(get(ENV_SECCOMP_DENY).starts_with("ptrace,io_uring_setup,"));
        assert!(get(ENV_SECCOMP_DENY).contains(",request_key,"));
        assert!(get(ENV_SECCOMP_DENY).ends_with(",userfaultfd"));
        assert_eq!(get(ENV_REPORT_FD), "11");
        for (key, _) in &env {
            assert!(
                RESERVED_ENV.contains(&key.as_str()),
                "{key} is not reserved"
            );
        }

        let without = shim_env(&profile, None);
        assert!(
            !without.iter().any(|(k, _)| k == ENV_REPORT_FD),
            "without a pipe there is no report variable"
        );
        assert_eq!(without.len(), 4);
    }

    #[test]
    fn check_lines_parse_and_everything_else_does_not() {
        assert_eq!(
            parse_check_line("CHECK bridge_listening ok 127.0.0.1:3128 accepted\n"),
            Some(ShimCheck {
                name: "bridge_listening".to_owned(),
                ok: true,
                evidence: "127.0.0.1:3128 accepted".to_owned(),
            })
        );
        assert_eq!(
            parse_check_line("CHECK families fail AF_UNIX: created"),
            Some(ShimCheck {
                name: "families".to_owned(),
                ok: false,
                evidence: "AF_UNIX: created".to_owned(),
            })
        );
        assert_eq!(
            parse_check_line("CHECK no_interfaces ok"),
            Some(ShimCheck {
                name: "no_interfaces".to_owned(),
                ok: true,
                evidence: String::new(),
            })
        );
        for bad in [
            "",
            "check families ok",
            "CHECK",
            "CHECK families",
            "CHECK families maybe x",
            "CHECK  ok x",
            "humanitl-shim 0.0.0",
        ] {
            assert_eq!(parse_check_line(bad), None, "{bad:?} must not parse");
        }
        assert_eq!(CHECK_NAMES.len(), 5);
        assert_eq!(
            parse_check_line(
                "CHECK single_socket fail sockets=/run/humanitl/proxy.sock,/tmp/x.sock;unexpected=/tmp/x.sock;entries=2000;limit=entries"
            ),
            Some(ShimCheck {
                name: "single_socket".to_owned(),
                ok: false,
                evidence: "sockets=/run/humanitl/proxy.sock,/tmp/x.sock;unexpected=/tmp/x.sock;entries=2000;limit=entries"
                    .to_owned(),
            })
        );
    }
}
