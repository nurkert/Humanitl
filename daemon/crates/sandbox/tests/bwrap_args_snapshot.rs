//! Die erzeugte `bwrap`-Kommandozeile, Argument für Argument.
//!
//! `tests/snapshots/default.argv.txt` hält fest, was `profiles/sandbox/default.toml`
//! mit der Vorschau ([`LaunchInputs::preview`]: Deskriptoren 10, 11, 12, alles
//! unter `/work` vorhanden) ergibt: ein Argument je Zeile. Wer die Reihenfolge
//! ändert, sieht es hier und muss die Änderung bewusst übernehmen:
//!
//! ```text
//! UPDATE_ARGV_SNAPSHOT=1 cargo test -p humanitl-sandbox --test bwrap_args_snapshot
//! ```
//!
//! Der Schnappschuss hängt auch am `[env]` des Profils: eine neue Variable
//! dort (HUM-014) ändert ihn genauso wie eine neue Zeile hier.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use humanitl_config::WorkMode;
use humanitl_core::ids::SessionId;
use humanitl_sandbox::{
    LaunchInputs, Namespace, PREVIEW_MASK_FD_FIRST, SandboxProfile, SessionContext,
};

fn profile(name: &str) -> SandboxProfile {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../profiles/sandbox")
        .join(format!("{name}.toml"));
    SandboxProfile::load(&path)
        .unwrap_or_else(|err| panic!("{} does not load: {err}", path.display()))
}

/// Ein Profil aus einem Textfragment, das nur `[mounts]` oder `[sandbox]` setzt.
fn probe(body: &str) -> SandboxProfile {
    let text = format!("version = 1\nname = \"probe\"\n{body}");
    SandboxProfile::parse(&text, Path::new("<probe>")).expect("the probe profile parses")
}

/// Die Stelle, an der das Fenster `window` in `args` beginnt.
fn window_at(args: &[String], window: &[&str]) -> Option<usize> {
    args.windows(window.len())
        .position(|w| w.iter().zip(window).all(|(a, b)| a == b))
}

/// Die Stelle von `--ro-bind-data <fd> dst`, mit dem Deskriptor der Vorschau.
fn mask_at(args: &[String], dst: &str) -> Option<usize> {
    args.windows(3).position(|w| {
        w[0] == "--ro-bind-data"
            && w[1]
                .parse::<i32>()
                .is_ok_and(|fd| fd >= PREVIEW_MASK_FD_FIRST)
            && w[2] == dst
    })
}

/// Ein fester Kontext, damit der Schnappschuss auf jeder Maschine gleich ist.
///
/// Der Befehl enthält absichtlich ein Argument mit Leerzeichen: die Zeile für
/// die Oberfläche muss es zitieren, und `shlex` muss es wieder zusammensetzen.
fn context(work_mode: WorkMode) -> SessionContext {
    SessionContext {
        session: SessionId::nil(),
        work_src: PathBuf::from("/home/u/proj"),
        work_mode,
        proxy_socket_src: PathBuf::from("/run/user/1000/humanitl/proxy/proxy.sock"),
        ca_cert_src: PathBuf::from("/home/u/.local/share/humanitl/ca/ca.crt"),
        ca_bundle_src: PathBuf::from("/home/u/.local/share/humanitl/ca/ca-bundle.crt"),
        shim_src: PathBuf::from("/usr/lib/humanitl/humanitl-shim"),
        // Das Env-Kit der Sitzung, gekürzt auf die eine Variable, die nur die
        // Sitzung kennt: der Rest steht im `[env]` des Profils.
        session_env: vec![("HUMANITL_SESSION".to_owned(), SessionId::nil().to_string())],
        command: vec![
            OsString::from("sh"),
            OsString::from("-c"),
            OsString::from("echo hello world"),
        ],
        files: Vec::new(),
    }
}

fn strings(args: &[OsString]) -> Vec<String> {
    args.iter()
        .map(|arg| arg.to_str().expect("every argument is UTF-8").to_owned())
        .collect()
}

fn index_of(args: &[String], needle: &str) -> usize {
    args.iter()
        .position(|arg| arg == needle)
        .unwrap_or_else(|| panic!("{needle} is missing from the argument list"))
}

#[test]
fn bwrap_args_snapshot_default() {
    let args = strings(
        &profile("default").to_bwrap_args(&context(WorkMode::Rw), &LaunchInputs::preview()),
    );
    for arg in &args {
        assert!(
            !arg.contains('\n'),
            "an argument with a newline cannot live in the snapshot: {arg:?}"
        );
    }
    let rendered = format!("{}\n", args.join("\n"));

    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/default.argv.txt");
    if std::env::var_os("UPDATE_ARGV_SNAPSHOT").is_some() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create the snapshot directory");
        }
        std::fs::write(&path, &rendered).expect("write the snapshot");
        return;
    }

    let expected = std::fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!(
            "{} is missing ({err}); run UPDATE_ARGV_SNAPSHOT=1 cargo test -p humanitl-sandbox --test bwrap_args_snapshot",
            path.display()
        )
    });
    assert_eq!(
        rendered, expected,
        "the argument list changed; run UPDATE_ARGV_SNAPSHOT=1 cargo test -p humanitl-sandbox --test bwrap_args_snapshot"
    );
}

/// Das erzeugte Bundle überdeckt den System-Vertrauensspeicher, und zwar nach
/// dem `--ro-bind /etc/ssl` des Profils: davor stünde es, wäre es danach
/// wieder verdeckt, und jeder TLS-Client in der Sandbox lehnte das Leaf des
/// Proxys ab (HUM-011, HUM-014).
#[test]
fn the_ca_bundle_overlays_the_system_store_after_the_etc_ssl_bind() {
    let args = strings(
        &profile("default").to_bwrap_args(&context(WorkMode::Rw), &LaunchInputs::preview()),
    );
    let etc_ssl = index_of(&args, "/etc/ssl");
    let bundle = index_of(&args, "/etc/ssl/certs/ca-certificates.crt");
    assert!(
        etc_ssl < bundle,
        "the overlay must come after the read-only bind of /etc/ssl: {args:?}"
    );
    assert_eq!(
        args[bundle - 1],
        "/home/u/.local/share/humanitl/ca/ca-bundle.crt"
    );
    assert_eq!(args[bundle - 2], "--ro-bind");
    // Und die Sitzung, nicht das Profil, sagt, welche Datei das ist.
    assert!(
        !args[..etc_ssl].contains(&"/etc/ssl/certs/ca-certificates.crt".to_owned()),
        "nothing binds the system store before the profile's /etc/ssl: {args:?}"
    );
}

/// Die Sitzung bringt ihre eigenen Variablen mit, hier `HUMANITL_SESSION`
/// (HUM-014); das Profil kennt sie nicht.
#[test]
fn the_session_environment_reaches_the_sandbox() {
    let args = strings(
        &profile("default").to_bwrap_args(&context(WorkMode::Rw), &LaunchInputs::preview()),
    );
    let at = index_of(&args, "HUMANITL_SESSION");
    assert_eq!(args[at - 1], "--setenv");
    assert_eq!(args[at + 1], SessionId::nil().to_string());
    assert!(
        !profile("default")
            .env_pairs()
            .contains_key("HUMANITL_SESSION"),
        "the profile has no session; the context does"
    );
}

#[test]
fn work_rw_uses_bind() {
    let args = strings(
        &profile("default").to_bwrap_args(&context(WorkMode::Rw), &LaunchInputs::preview()),
    );
    let at = index_of(&args, "/home/u/proj");
    assert_eq!(args[at - 1], "--bind");
    assert_eq!(args[at + 1], "/work");
}

#[test]
fn work_ro_uses_ro_bind() {
    let args = strings(
        &profile("default").to_bwrap_args(&context(WorkMode::Ro), &LaunchInputs::preview()),
    );
    let at = index_of(&args, "/home/u/proj");
    assert_eq!(args[at - 1], "--ro-bind");
    assert_eq!(args[at + 1], "/work");
    assert!(
        !args.contains(&"--bind".to_owned()),
        "a read-only session must not write anywhere: {args:?}"
    );
}

#[test]
fn env_is_cleared_then_set() {
    let args = strings(
        &profile("default").to_bwrap_args(&context(WorkMode::Rw), &LaunchInputs::preview()),
    );
    let clearenv = index_of(&args, "--clearenv");
    let first_setenv = index_of(&args, "--setenv");
    assert!(
        clearenv < first_setenv,
        "--clearenv must come before the first --setenv"
    );

    let mut keys: Vec<&str> = Vec::new();
    for (at, arg) in args.iter().enumerate() {
        if arg == "--setenv" {
            keys.push(&args[at + 1]);
        }
    }
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    assert_eq!(keys, sorted, "--setenv must be alphabetical");
    assert!(keys.contains(&"SSL_CERT_FILE"));
    assert!(keys.contains(&"HTTP_PROXY"));
    for shim_key in [
        "HUMANITL_BRIDGES",
        "HUMANITL_SECCOMP_FAMILIES",
        "HUMANITL_SECCOMP_TYPES",
        "HUMANITL_SECCOMP_DENY",
        "HUMANITL_REPORT_FD",
    ] {
        assert!(keys.contains(&shim_key), "{shim_key} is missing");
    }
    let report = args
        .windows(3)
        .position(|w| w[0] == "--setenv" && w[1] == "HUMANITL_REPORT_FD")
        .expect("HUMANITL_REPORT_FD is set");
    assert_eq!(
        args[report + 2],
        "10",
        "the preview reports on descriptor 10"
    );
}

#[test]
fn tmpfs_under_work_comes_after_the_work_bind() {
    let args = strings(
        &profile("default").to_bwrap_args(&context(WorkMode::Rw), &LaunchInputs::preview()),
    );
    let work_bind = index_of(&args, "/home/u/proj");
    for under_work in ["/work/.git/hooks", "/work/.vscode", "/work/.idea"] {
        let at = index_of(&args, under_work);
        assert_eq!(args[at - 1], "--tmpfs", "{under_work} is not a tmpfs");
        assert!(
            at > work_bind,
            "{under_work} would be covered again by the bind of /work"
        );
    }
    assert!(
        index_of(&args, "/tmp") < work_bind,
        "a tmpfs outside /work keeps its place in the profile order"
    );
}

#[test]
fn masked_files_come_after_the_work_bind() {
    let args = strings(
        &profile("default").to_bwrap_args(&context(WorkMode::Rw), &LaunchInputs::preview()),
    );
    let work_bind = index_of(&args, "/home/u/proj");
    for masked in ["/work/.envrc", "/work/.git/config"] {
        let at = mask_at(&args, masked).unwrap_or_else(|| {
            panic!("{masked} does not come from the launcher's memfd: {args:?}")
        });
        assert!(
            at > work_bind,
            "{masked} would be uncovered by the bind of /work"
        );
    }
}

#[test]
fn the_argv_ends_with_the_shim_and_the_command() {
    let args = strings(
        &profile("default").to_bwrap_args(&context(WorkMode::Rw), &LaunchInputs::preview()),
    );
    let expected: Vec<String> = [
        "--chdir",
        "/work",
        "--",
        "/run/humanitl/humanitl-shim",
        "--proxy-port",
        "3128",
        "--",
        "sh",
        "-c",
        "echo hello world",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    assert_eq!(
        &args[args.len() - expected.len()..],
        expected.as_slice(),
        "{args:?}"
    );
}

#[test]
fn the_sockets_and_certificates_come_from_the_session() {
    let args = strings(
        &profile("default").to_bwrap_args(&context(WorkMode::Rw), &LaunchInputs::preview()),
    );
    for (src, dst) in [
        (
            "/run/user/1000/humanitl/proxy/proxy.sock",
            "/run/humanitl/proxy.sock",
        ),
        (
            "/home/u/.local/share/humanitl/ca/ca.crt",
            "/etc/humanitl/ca.crt",
        ),
        (
            "/usr/lib/humanitl/humanitl-shim",
            "/run/humanitl/humanitl-shim",
        ),
    ] {
        let at = index_of(&args, src);
        assert_eq!(args[at - 1], "--ro-bind", "{src} must be read-only");
        assert_eq!(args[at + 1], dst);
    }
}

#[test]
fn no_network_namespace_is_never_optional() {
    let args = strings(
        &profile("default").to_bwrap_args(&context(WorkMode::Rw), &LaunchInputs::preview()),
    );
    assert_eq!(args[0], "--unshare-user");
    assert!(args.contains(&"--unshare-net".to_owned()));
    let cap_drop = index_of(&args, "--cap-drop");
    assert_eq!(args[cap_drop + 1], "ALL");
}

/// Was in jeder Zeile stehen muss, egal welches Profil (CONVENTIONS.md 3.4,
/// 4.10, HUM-010): die Namensräume von `--unshare-all` ausgeschrieben, keine
/// Capabilities, eigene Session, Ende mit dem Daemon, frisches `/proc` und
/// `/dev`, tmpfs auf `/dev/shm`, der Hostname, kein Host-Environment.
#[test]
fn the_hard_floor_is_in_every_shipped_profile() {
    for name in ["default", "test"] {
        let args =
            strings(&profile(name).to_bwrap_args(&context(WorkMode::Rw), &LaunchInputs::preview()));
        for flag in [
            "--unshare-user",
            "--unshare-pid",
            "--unshare-net",
            "--unshare-ipc",
            "--unshare-uts",
            "--unshare-cgroup",
            "--die-with-parent",
            "--new-session",
            "--disable-userns",
            "--clearenv",
        ] {
            assert!(
                args.iter().any(|arg| arg == flag),
                "{name}: {flag} is missing"
            );
        }
        for (flag, value) in [
            ("--cap-drop", "ALL"),
            ("--hostname", "sandbox"),
            ("--json-status-fd", "11"),
            ("--proc", "/proc"),
            ("--dev", "/dev"),
            ("--tmpfs", "/dev/shm"),
            ("--tmpfs", "/tmp"),
            ("--chdir", "/work"),
        ] {
            assert!(
                args.windows(2).any(|w| w[0] == flag && w[1] == value),
                "{name}: {flag} {value} is missing"
            );
        }
        for forbidden in ["--share-net", "--dev-bind", "--dev-bind-try", "--cap-add"] {
            assert!(
                !args.iter().any(|arg| arg == forbidden),
                "{name}: {forbidden} must never appear"
            );
        }
        // Erst das minimale /dev, dann das tmpfs darin, sonst verdeckt --dev es wieder.
        let dev = index_of(&args, "--dev");
        let shm = index_of(&args, "/dev/shm");
        assert!(dev < shm, "{name}: --tmpfs /dev/shm must come after --dev");
        // Die Namensräume stehen vorn, bevor irgendetwas eingehängt wird.
        let first_mount = index_of(&args, "--ro-bind");
        for ns in Namespace::ALL {
            assert!(
                index_of(&args, ns.flag()) < first_mount,
                "{name}: {} after a mount",
                ns.flag()
            );
        }
    }
}

#[test]
fn user_extras_come_before_everything_the_session_mounts() {
    let args =
        strings(&profile("test").to_bwrap_args(&context(WorkMode::Rw), &LaunchInputs::preview()));
    let extra = index_of(&args, "/tests/escape");
    for session_bind in [
        "/home/u/proj",
        "/run/user/1000/humanitl/proxy/proxy.sock",
        "/home/u/.local/share/humanitl/ca/ca.crt",
        "/usr/lib/humanitl/humanitl-shim",
        "/work/.envrc",
    ] {
        assert!(
            extra < index_of(&args, session_bind),
            "{session_bind} must be mounted after the user's extras so it stays on top"
        );
    }
    assert!(
        index_of(&args, "--proc") < extra,
        "/proc, /dev and the tmpfs come before the extras, so an extra can sit inside /tmp"
    );
}

#[test]
fn the_test_profile_binds_the_escape_scripts() {
    let args =
        strings(&profile("test").to_bwrap_args(&context(WorkMode::Rw), &LaunchInputs::preview()));
    let at = index_of(&args, "/tests/escape");
    assert_eq!(args[at - 1], "--ro-bind");
    assert_eq!(args[at + 1], "/tests/escape");
    let setenv = args
        .windows(3)
        .position(|w| w[0] == "--setenv" && w[1] == "HUMANITL_TEST")
        .expect("HUMANITL_TEST is set");
    assert_eq!(args[setenv + 2], "1");
}

/// `extra_rw` wird als `--bind <src> <dst>` mit gleicher Quelle und gleichem
/// Ziel gerendert, vor dem Projektverzeichnis und nach `/proc` und `/dev`.
#[test]
fn extra_rw_renders_as_bind_with_the_same_path_twice() {
    let profile = probe(
        "[mounts]\nextra_ro = [\"/opt/toolchain\"]\nextra_rw = [\"/srv/data\", \"/srv/cache\"]\n",
    );
    let args = strings(&profile.to_bwrap_args(&context(WorkMode::Rw), &LaunchInputs::preview()));

    let data = window_at(&args, &["--bind", "/srv/data", "/srv/data"])
        .expect("extra_rw renders as --bind src dst");
    let cache = window_at(&args, &["--bind", "/srv/cache", "/srv/cache"])
        .expect("every extra_rw entry renders");
    let ro = window_at(&args, &["--ro-bind", "/opt/toolchain", "/opt/toolchain"])
        .expect("extra_ro renders as --ro-bind src dst");
    assert!(
        ro < data && data < cache,
        "profile order: extra_ro, then extra_rw in order: {args:?}"
    );
    assert!(
        index_of(&args, "--dev") < ro,
        "the extras come after /proc and /dev"
    );
    assert!(
        cache < index_of(&args, "/home/u/proj"),
        "the extras come before the work bind"
    );

    // Ein `extra_rw` macht aus einer `ro`-Sitzung keine schreibende: nur die
    // Erweiterung selbst ist `--bind`.
    let args = strings(&profile.to_bwrap_args(&context(WorkMode::Ro), &LaunchInputs::preview()));
    let work = index_of(&args, "/home/u/proj");
    assert_eq!(args[work - 1], "--ro-bind");
    assert!(window_at(&args, &["--bind", "/srv/data", "/srv/data"]).is_some());
}

/// `--die-with-parent` und `--new-session` stehen in jeder Zeile, auch wenn die
/// Felder des Profils an `parse` vorbei auf `false` gesetzt wurden.
#[test]
fn die_with_parent_and_new_session_are_never_optional() {
    let mut profile = profile("default");
    profile.sandbox.die_with_parent = false;
    profile.sandbox.new_session = false;
    let args = strings(&profile.to_bwrap_args(&context(WorkMode::Rw), &LaunchInputs::preview()));
    assert_eq!(
        &args[6..11],
        [
            "--die-with-parent",
            "--new-session",
            "--cap-drop",
            "ALL",
            "--disable-userns"
        ]
    );
}

/// Die Pflichtmasken kommen auch bei `masked_files = []`; ein Profil kann
/// Masken hinzufügen, nicht wegnehmen.
#[test]
fn mandatory_masks_survive_an_empty_masked_files() {
    let profile = probe("[mounts]\nmasked_files = []\n");
    let args = strings(&profile.to_bwrap_args(&context(WorkMode::Rw), &LaunchInputs::preview()));
    let work = index_of(&args, "/home/u/proj");
    for masked in humanitl_sandbox::MANDATORY_MASKED_FILES {
        let at =
            mask_at(&args, masked).unwrap_or_else(|| panic!("{masked} is not masked: {args:?}"));
        assert!(at > work, "{masked} must be masked after the work bind");
    }

    let profile = probe("[mounts]\nmasked_files = [\"/work/.npmrc\"]\n");
    let args = strings(&profile.to_bwrap_args(&context(WorkMode::Rw), &LaunchInputs::preview()));
    let envrc = mask_at(&args, "/work/.envrc").expect("mandatory");
    let git = mask_at(&args, "/work/.git/config").expect("mandatory");
    let npmrc = mask_at(&args, "/work/.npmrc").expect("addition");
    assert!(
        envrc < git && git < npmrc,
        "mandatory masks first, then the profile's: {args:?}"
    );
}

/// Die `--unshare-*`-Flags folgen `Namespace::ALL`, nicht dem Profil: ein
/// Profil kann sie weder umordnen noch doppeln.
#[test]
fn unshare_flags_follow_the_fixed_order_once_each() {
    let profile = probe(
        "[sandbox]\nunshare = [\"cgroup\", \"uts\", \"ipc\", \"net\", \"pid\", \"user\", \"net\", \"net\"]\n",
    );
    let args = strings(&profile.to_bwrap_args(&context(WorkMode::Rw), &LaunchInputs::preview()));
    let expected: Vec<&str> = Namespace::ALL.iter().map(|ns| ns.flag()).collect();
    assert_eq!(&args[..6], expected.as_slice());
    for ns in Namespace::ALL {
        assert_eq!(
            args.iter().filter(|arg| *arg == ns.flag()).count(),
            1,
            "{} must appear exactly once",
            ns.flag()
        );
    }
}

#[test]
fn argv_line_is_shell_parsable() {
    let profile = profile("default");
    let ctx = context(WorkMode::Rw);
    let line = profile.argv_line(&ctx, &LaunchInputs::preview());

    assert!(
        line.contains("'echo hello world'"),
        "an argument with spaces must be quoted: {line}"
    );
    let split = shlex::split(&line).expect("the line is a valid shell word list");
    assert_eq!(
        split,
        strings(&profile.to_bwrap_args(&ctx, &LaunchInputs::preview()))
    );
}
