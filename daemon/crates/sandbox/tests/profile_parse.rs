//! Die ausgelieferten Profile und die Mount-Allowlist.
//!
//! Die Allowlist ist eine Sicherheitsgrenze, keine Bequemlichkeit: sie wird hier
//! tabellengetrieben geprüft, mit einer Zeile je Pfad, den ein Profil nie
//! einhängen darf.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use humanitl_config::{Env, WorkMode};
use humanitl_sandbox::{
    BridgeDirection, MountPolicy, Namespace, SOCKET_WALK_MAX_DEPTH, SandboxProfile, SocketFamily,
    SocketFloor, SocketType,
};

/// Das Heimatverzeichnis, gegen das die Tabelle prüft.
const HOME: &str = "/home/tester";

fn profiles_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../profiles/sandbox")
}

fn load(name: &str) -> SandboxProfile {
    let path = profiles_dir().join(format!("{name}.toml"));
    SandboxProfile::load(&path)
        .unwrap_or_else(|err| panic!("{} does not load: {err}", path.display()))
}

/// Ein Profil mit genau einer zusätzlichen Quelle.
fn with_extra_ro(path: &str) -> SandboxProfile {
    let text = format!("version = 1\nname = \"probe\"\n[mounts]\nextra_ro = [{path:?}]\n");
    SandboxProfile::parse(&text, Path::new("<probe>")).expect("the probe profile parses")
}

/// Die Schlüssel, die jedes Profil in der Umgebung setzen muss (HUM-014).
const ENV_KIT: &[&str] = &[
    "HOME",
    "USER",
    "TERM",
    "LANG",
    "PATH",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "http_proxy",
    "https_proxy",
    "NO_PROXY",
    "no_proxy",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "CURL_CA_BUNDLE",
    "REQUESTS_CA_BUNDLE",
    "NODE_EXTRA_CA_CERTS",
    "DENO_CERT",
    "GIT_SSL_CAINFO",
    "CARGO_HTTP_CAINFO",
    "PIP_CERT",
    "NPM_CONFIG_CAFILE",
    "NIX_SSL_CERT_FILE",
];

/// Die Überdeckungen unter `/work`, die den Kanal 1 härten (HUM-043,
/// `docs/SECURITY.md` 3.2).
///
/// Die Listen dürfen wachsen; keiner dieser Einträge darf still verschwinden.
fn assert_default_masks(profile: &SandboxProfile) {
    for required in [
        "/tmp",
        "/var/tmp",
        "/dev/shm",
        "/home/agent",
        "/work/.git/hooks",
        "/work/.vscode",
        "/work/.idea",
        "/work/.fleet",
        "/work/.github/workflows",
        "/work/.gitlab-ci.yml.d",
        "/work/.direnv",
        "/work/.humanitl",
    ] {
        assert!(
            profile.mounts.tmpfs.contains(&PathBuf::from(required)),
            "mounts.tmpfs misses {required}"
        );
    }
    assert_eq!(profile.mounts.proc, Some(PathBuf::from("/proc")));
    assert_eq!(profile.mounts.dev, Some(PathBuf::from("/dev")));
    assert_eq!(
        profile.mounts.masked_files,
        [
            "/work/.envrc",
            "/work/.env",
            "/work/.env.local",
            "/work/.git/config",
            "/work/.npmrc",
            "/work/.yarnrc",
            "/work/.yarnrc.yml",
            "/work/.pypirc",
            "/work/.gitlab-ci.yml",
            "/work/Jenkinsfile",
            "/work/.pre-commit-config.yaml",
        ]
        .map(PathBuf::from)
    );
    assert!(
        profile.mounts.unmask.is_empty(),
        "the shipped profile lifts no mask"
    );
}

#[test]
fn parses_default_profile() {
    let profile = load("default");

    assert_eq!(profile.version, 1);
    assert_eq!(profile.name, "default");
    assert_eq!(profile.sandbox.backend, "bwrap");
    assert_eq!(profile.sandbox.hostname, "sandbox");
    assert_eq!(
        profile.sandbox.unshare,
        vec![
            Namespace::User,
            Namespace::Pid,
            Namespace::Net,
            Namespace::Ipc,
            Namespace::Uts,
            Namespace::Cgroup,
        ]
    );
    assert!(profile.sandbox.die_with_parent);
    assert!(profile.sandbox.new_session);
    assert_eq!(profile.sandbox.min_bwrap_version, "0.8.0");

    assert_eq!(profile.mounts.work.dst, PathBuf::from("/work"));
    assert_eq!(profile.mounts.work.mode, Some(WorkMode::Rw));
    assert_eq!(
        profile.mounts.ro,
        [
            "/usr",
            "/etc/ssl",
            "/etc/alternatives",
            "/etc/ld.so.cache",
            "/etc/localtime"
        ]
        .map(PathBuf::from)
    );
    assert_eq!(profile.mounts.symlinks.len(), 4);
    assert_eq!(profile.mounts.symlinks[0].target, "usr/lib");
    assert_eq!(profile.mounts.symlinks[0].link, PathBuf::from("/lib"));
    assert_default_masks(&profile);
    assert!(profile.mounts.extra_ro.is_empty());
    assert!(profile.mounts.extra_rw.is_empty());

    assert_eq!(
        profile.network.proxy_socket_dst,
        PathBuf::from("/run/humanitl/proxy.sock")
    );
    assert_eq!(profile.network.proxy_port, 3128);
    assert_eq!(
        profile.network.ca_cert_dst,
        PathBuf::from("/etc/humanitl/ca.crt")
    );
    assert_eq!(
        profile.network.shim_dst,
        PathBuf::from("/run/humanitl/humanitl-shim")
    );

    let bridge = profile.proxy_bridge().expect("the proxy bridge exists");
    assert_eq!(bridge.dir, BridgeDirection::In);
    assert_eq!(bridge.listen.to_string(), "127.0.0.1:3128");
    assert_eq!(bridge.socket, PathBuf::from("/run/humanitl/proxy.sock"));

    assert_eq!(
        profile.seccomp.allow_families,
        vec![SocketFamily::AfInet, SocketFamily::AfInet6]
    );
    assert_eq!(profile.seccomp.allow_types, vec![SocketType::SockStream]);
    assert_eq!(
        profile.seccomp.deny_syscalls,
        humanitl_sandbox::DEFAULT_DENY_SYSCALLS
            .iter()
            .map(|s| (*s).to_owned())
            .collect::<Vec<_>>()
    );

    profile
        .validate_with(&MountPolicy::new(HOME))
        .expect("the shipped profile passes its own allowlist");
}

#[test]
fn parses_test_profile() {
    let profile = load("test");

    assert_eq!(profile.name, "test");
    assert_eq!(profile.mounts.extra_ro, [PathBuf::from("/tests/escape")]);
    assert_eq!(
        profile.env.get("HUMANITL_TEST").map(String::as_str),
        Some("1")
    );
    profile
        .validate_with(&MountPolicy::new(HOME))
        .expect("the escape profile passes the allowlist");
}

#[test]
fn test_profile_differs_from_default_only_where_it_must() {
    let mut default = load("default");
    let test = load("test");

    default.name = test.name.clone();
    default.description = test.description.clone();
    default.mounts.extra_ro = test.mounts.extra_ro.clone();
    default
        .env
        .insert("HUMANITL_TEST".to_owned(), "1".to_owned());

    assert_eq!(
        default, test,
        "the escape profile must differ from default only in name, description, extra_ro and HUMANITL_TEST"
    );
}

#[test]
fn every_profile_carries_the_whole_env_kit() {
    for name in ["default", "test"] {
        let profile = load(name);
        for key in ENV_KIT {
            assert!(
                profile.env.contains_key(*key),
                "profile {name} misses the environment key {key}"
            );
        }
        assert_eq!(
            profile.env.get("SSL_CERT_FILE").map(String::as_str),
            Some("/etc/humanitl/ca.crt"),
            "profile {name} points SSL_CERT_FILE somewhere else"
        );
    }
}

#[test]
fn rejects_docker_socket_mount() {
    let err = with_extra_ro("/var/run/docker.sock")
        .validate_with(&MountPolicy::new(HOME))
        .expect_err("the docker socket is a root shell");
    assert_eq!(err.code.as_str(), "SANDBOX_006");
    assert!(err.why.contains("/var/run/docker.sock"), "{}", err.why);
    assert!(err.why.contains("mounts.extra_ro"), "{}", err.why);
}

#[test]
fn rejects_runtime_dir_mount() {
    let err = with_extra_ro("/run/user/1000")
        .validate_with(&MountPolicy::new(HOME))
        .expect_err("the runtime directory holds the daemon socket and the session token");
    assert_eq!(err.code.as_str(), "SANDBOX_006");
    assert!(err.why.contains("/run/user/1000"), "{}", err.why);
}

#[test]
fn rejects_ssh_dir_mount() {
    let err = with_extra_ro(&format!("{HOME}/.ssh"))
        .validate_with(&MountPolicy::new(HOME))
        .expect_err("private keys never enter the sandbox");
    assert_eq!(err.code.as_str(), "SANDBOX_006");
    assert!(err.why.contains("/home/tester/.ssh"), "{}", err.why);
    assert!(err.why.contains("private keys"), "{}", err.why);
}

#[test]
fn forbidden_mount_sources() {
    let home = Path::new(HOME);
    let forbidden = [
        ("/proc", "the kernel process table"),
        ("/proc/1/environ", "a single file below /proc"),
        ("/sys", "kernel objects"),
        ("/sys/class/net", "the host network devices"),
        ("/dev", "host devices"),
        ("/dev/kvm", "a single host device"),
        ("/run", "the whole runtime tree"),
        ("/run/user/1000", "XDG_RUNTIME_DIR"),
        ("/run/user/1000/bus", "the D-Bus session bus"),
        ("/run/user/1000/wayland-0", "the Wayland socket"),
        ("/run/docker.sock", "the docker socket"),
        ("/var/run", "the runtime tree under its old name"),
        ("/var/run/docker.sock", "the docker socket, old name"),
        ("/tmp", "the host /tmp as a whole"),
        ("/tmp/.X11-unix", "the X11 socket directory"),
        ("/tmp/.X11-unix/X0", "a single X11 socket"),
        ("/var/tmp", "the host /var/tmp as a whole"),
        ("/home/tester", "the home directory itself"),
        ("/home/tester/.ssh", "private keys"),
        ("/home/tester/.ssh/id_ed25519", "a single private key"),
        ("/home/tester/.gnupg", "the GnuPG keyring"),
        ("/home/tester/.gitconfig", "the git credential helper"),
        ("/home/tester/.netrc", "credentials in plain text"),
        ("/home/tester/.config/humanitl", "Humanitl's own rules"),
        (
            "/home/tester/.local/share/humanitl",
            "Humanitl's own database",
        ),
        (
            "/home/tester/projects/other",
            "another project of the same user",
        ),
        (
            "/home/tester/../tester/.ssh",
            "private keys behind a detour",
        ),
        ("/run/dbus/system_bus_socket", "the D-Bus system bus"),
        ("/var/run/dbus", "the D-Bus system bus, old name"),
        ("/root", "the superuser's home"),
        ("/root/.ssh", "the superuser's keys"),
        ("/", "the whole host: it contains every entry above"),
        ("/home", "every home directory, the user's included"),
        ("/var", "it contains /var/run/docker.sock"),
        ("etc/ssl", "a relative source"),
    ];

    for (path, why) in forbidden {
        let Err(err) = with_extra_ro(path).validate_with(&MountPolicy::new(home)) else {
            panic!("{path} ({why}) was accepted by the mount allowlist");
        };
        assert_eq!(err.code.as_str(), "SANDBOX_006", "{path} ({why})");
        assert!(
            err.why.contains(path),
            "{path} ({why}) is not named: {}",
            err.why
        );
    }
}

#[test]
fn allowed_mount_sources() {
    let home = Path::new(HOME);
    for path in [
        "/usr",
        "/etc/ssl",
        "/opt/toolchain",
        "/srv/data",
        "/nix/store",
    ] {
        with_extra_ro(path)
            .validate_with(&MountPolicy::new(home))
            .unwrap_or_else(|err| panic!("{path} must stay allowed: {err}"));
    }
}

#[test]
fn a_symlink_cannot_smuggle_a_forbidden_source() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = std::fs::canonicalize(temp.path()).expect("canonical tempdir");
    let home = root.join("home");
    std::fs::create_dir_all(home.join(".ssh")).expect("create ~/.ssh");
    std::os::unix::fs::symlink(home.join(".ssh"), root.join("keys")).expect("symlink");

    let profile = with_extra_ro(root.join("keys").to_str().expect("utf-8 path"));
    let err = profile
        .validate_with(&MountPolicy::new(&home))
        .expect_err("the source is canonicalised before it is checked");
    assert_eq!(err.code.as_str(), "SANDBOX_006");
    assert!(err.why.contains("resolves to"), "{}", err.why);
    assert!(err.why.contains(".ssh"), "{}", err.why);
}

#[test]
fn a_home_behind_a_symlink_is_protected_under_both_names() {
    // Silverblue and friends: /home is a symlink to /var/home, $HOME says /home/u.
    let temp = tempfile::tempdir().expect("tempdir");
    let root = std::fs::canonicalize(temp.path()).expect("canonical tempdir");
    std::fs::create_dir_all(root.join("var/home/u/.ssh")).expect("create the real home");
    std::os::unix::fs::symlink(root.join("var/home"), root.join("home")).expect("symlink /home");
    let home = root.join("home/u");

    let resolved_keys = root.join("var/home/u/.ssh");
    let err = with_extra_ro(resolved_keys.to_str().expect("utf-8 path"))
        .validate_with(&MountPolicy::new(&home))
        .expect_err("the resolved spelling of ~/.ssh is still ~/.ssh");
    assert_eq!(err.code.as_str(), "SANDBOX_006");
    assert!(err.why.contains("private keys"), "{}", err.why);

    let resolved_project = root.join("var/home/u/proj");
    let err = with_extra_ro(resolved_project.to_str().expect("utf-8 path"))
        .validate_with(&MountPolicy::new(&home))
        .expect_err("anything under the resolved home is under home");
    assert_eq!(err.code.as_str(), "SANDBOX_006");
}

/// Ein frisches, kanonisches Verzeichnis außerhalb jeder verbotenen Basis.
fn scratch_dir() -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = std::fs::canonicalize(temp.path()).expect("canonical tempdir");
    (temp, root)
}

fn utf8(path: &Path) -> &str {
    path.to_str().expect("utf-8 path")
}

#[test]
fn a_socket_anywhere_is_forbidden() {
    // A session bus does not have to live under /run: some systems put it in /tmp.
    let (_temp, root) = scratch_dir();
    let socket = root.join("dbus-abc123");
    let _listener = std::os::unix::net::UnixListener::bind(&socket).expect("bind a socket");

    let err = with_extra_ro(utf8(&socket))
        .validate_with(&MountPolicy::new(HOME))
        .expect_err("a socket is a door out of the sandbox");
    assert_eq!(err.code.as_str(), "SANDBOX_006");
    assert!(err.why.contains("socket"), "{}", err.why);
}

#[test]
fn a_directory_that_contains_a_socket_is_forbidden() {
    // Guarantee 2: exactly one Unix socket enters the sandbox. A directory
    // source with a socket inside would bring a second one, so the walk finds it.
    let (_temp, root) = scratch_dir();
    let socket = root.join("dbus-abc123");
    let _listener = std::os::unix::net::UnixListener::bind(&socket).expect("bind a socket");

    for whence in ["ro", "extra_ro", "extra_rw"] {
        let text = format!(
            "version = 1\nname = \"probe\"\n[mounts]\n{whence} = [{:?}]\n",
            utf8(&root)
        );
        let profile = SandboxProfile::parse(&text, Path::new("<probe>")).expect("parses");
        let err = profile
            .validate_with(&MountPolicy::new(HOME))
            .expect_err("a directory with a socket inside is a door out of the sandbox");
        assert_eq!(err.code.as_str(), "SANDBOX_006", "mounts.{whence}");
        assert!(err.why.contains(&format!("mounts.{whence}")), "{}", err.why);
        assert!(err.why.contains(utf8(&socket)), "{}", err.why);
        assert!(err.why.contains("proxy socket"), "{}", err.why);
    }
}

#[test]
fn a_socket_below_the_walk_depth_is_found_and_deeper_is_not() {
    let (_temp, root) = scratch_dir();
    let mut dir = root.clone();
    for level in 1..SOCKET_WALK_MAX_DEPTH {
        dir.push(format!("d{level}"));
    }
    std::fs::create_dir_all(&dir).expect("nest the directories");
    // `root/d1/d2/sock` lies at depth 3 and is seen.
    let socket = dir.join("sock");
    let _listener = std::os::unix::net::UnixListener::bind(&socket).expect("bind a socket");
    let err = with_extra_ro(utf8(&root))
        .validate_with(&MountPolicy::new(HOME))
        .expect_err("a socket within the walk depth is found");
    assert_eq!(err.code.as_str(), "SANDBOX_006");
    assert!(err.why.contains(utf8(&socket)), "{}", err.why);

    // `root/d1/d2/d3/sock` lies at depth 4: the walk is bounded and does not
    // reach it. The isolation check inside the sandbox (HUM-041) counts sockets
    // for real; this test documents the bound, it does not bless the socket.
    let (_temp, root) = scratch_dir();
    let mut dir = root.clone();
    for level in 1..=SOCKET_WALK_MAX_DEPTH {
        dir.push(format!("d{level}"));
    }
    std::fs::create_dir_all(&dir).expect("nest the directories");
    let _listener = std::os::unix::net::UnixListener::bind(dir.join("sock")).expect("bind");
    with_extra_ro(utf8(&root))
        .validate_with(&MountPolicy::new(HOME))
        .expect("beyond the walk depth the check is not a proof");
}

#[test]
fn a_clean_directory_passes_and_symlinks_are_not_followed() {
    let (_temp, root) = scratch_dir();
    std::fs::create_dir_all(root.join("share/doc")).expect("create some directories");
    std::fs::write(root.join("share/doc/README"), "hello").expect("write a file");
    with_extra_ro(utf8(&root))
        .validate_with(&MountPolicy::new(HOME))
        .expect("a plain directory outside every denied tree stays allowed");

    // A socket elsewhere, reached only through a symlink inside the source:
    // the link is not followed, and the socket itself is not in the source.
    let (_elsewhere, other) = scratch_dir();
    let socket = other.join("sock");
    let _listener = std::os::unix::net::UnixListener::bind(&socket).expect("bind a socket");
    std::os::unix::fs::symlink(&socket, root.join("share/link-to-socket")).expect("symlink");
    std::os::unix::fs::symlink(&other, root.join("share/link-to-dir")).expect("symlink");
    with_extra_ro(utf8(&root))
        .validate_with(&MountPolicy::new(HOME))
        .expect("a symlink is neither a socket nor a directory to descend into");
}

#[test]
fn a_protected_dir_outside_home_is_forbidden_with_its_parents() {
    // $XDG_DATA_HOME on another disk: the CA key must stay unreachable there too.
    let policy = MountPolicy::new(HOME).with_protected_dir(
        "/mnt/data/xdg/humanitl",
        "Humanitl's own data, including the CA key",
    );
    for path in [
        "/mnt/data/xdg/humanitl",
        "/mnt/data/xdg/humanitl/ca",
        "/mnt/data/xdg",
        "/mnt",
    ] {
        let Err(err) = with_extra_ro(path).validate_with(&policy) else {
            panic!("{path} reaches the protected directory");
        };
        assert_eq!(err.code.as_str(), "SANDBOX_006", "{path}");
        assert!(err.why.contains("CA key"), "{path}: {}", err.why);
    }
    with_extra_ro("/mnt/data/other")
        .validate_with(&policy)
        .expect("a sibling of the protected directory stays allowed");
}

#[test]
fn unknown_field_is_diagnostic() {
    let err = SandboxProfile::parse(
        "version = 1\nname = \"x\"\n[mounts]\nro_mounts = [\"/usr\"]\n",
        Path::new("<probe>"),
    )
    .expect_err("a typo in a profile is a typo, not a default");
    assert_eq!(err.code.as_str(), "CONFIG_002");
    assert!(err.why.contains("ro_mounts"), "{}", err.why);
}

#[test]
fn unknown_section_is_diagnostic() {
    let err = SandboxProfile::parse(
        "version = 1\nname = \"x\"\n[netwrok]\nproxy_port = 3128\n",
        Path::new("<probe>"),
    )
    .expect_err("a misspelled section would silently do nothing");
    assert_eq!(err.code.as_str(), "CONFIG_002");
    assert!(err.why.contains("netwrok"), "{}", err.why);
}

#[test]
fn bridge_direction_out_is_not_supported_yet() {
    let text = concat!(
        "version = 1\nname = \"browser\"\n[network]\n",
        "bridges = [{ name = \"proxy\", dir = \"in\", listen = \"127.0.0.1:3128\", ",
        "socket = \"/run/humanitl/proxy.sock\" },\n",
        "{ name = \"cdp\", dir = \"out\", listen = \"127.0.0.1:9222\", ",
        "socket = \"/run/humanitl/cdp.sock\" }]\n"
    );
    let err = SandboxProfile::parse(text, Path::new("<probe>"))
        .expect_err("the shim cannot serve a socket inside the sandbox yet");
    assert_eq!(err.code.as_str(), "SANDBOX_007");
    assert!(
        err.why.contains("direction out not supported yet"),
        "{}",
        err.why
    );
    assert!(err.why.contains("cdp"), "{}", err.why);
}

/// Ein Profil kann den Socket-Boden nicht aufweichen: `AF_UNIX` neben dem
/// Proxy-Socket wäre eine zweite Tür, die der leere Netz-Namensraum nicht mehr
/// auffängt (dritte Garantie, Review-Befund vom 2026-09-03).
#[test]
fn a_profile_cannot_widen_the_socket_families() {
    let text = concat!(
        "version = 1\nname = \"x\"\n[seccomp]\n",
        "allow_families = [\"AF_INET\", \"AF_INET6\", \"AF_UNIX\"]\n"
    );
    let err = SandboxProfile::parse(text, Path::new("<probe>"))
        .expect_err("AF_UNIX is not a profile's decision");
    assert_eq!(err.code.as_str(), "CONFIG_003");
    assert!(err.why.contains("seccomp.allow_families"), "{}", err.why);
    assert!(err.why.contains("AF_UNIX"), "{}", err.why);
    assert!(err.why.contains("<probe>"), "{}", err.why);
}

/// Auch enger darf ein Profil die Liste nicht machen: ohne `AF_INET6` erreichte
/// der Agent den Proxy auf einer IPv6-Loopback-Adresse nicht mehr, und die
/// Abweichung fiele erst zur Laufzeit auf.
#[test]
fn a_profile_cannot_narrow_the_socket_families() {
    let text = concat!(
        "version = 1\nname = \"x\"\n[seccomp]\n",
        "allow_families = [\"AF_INET\"]\n"
    );
    let err = SandboxProfile::parse(text, Path::new("<probe>"))
        .expect_err("the floor holds in both directions");
    assert_eq!(err.code.as_str(), "CONFIG_003");
    assert!(err.why.contains("AF_INET6"), "{}", err.why);
}

/// `SOCK_DGRAM` bleibt in jedem Profil gesperrt: UDP wäre DNS und QUIC an der
/// Aufzeichnung vorbei.
#[test]
fn a_profile_cannot_widen_the_socket_types() {
    let text = concat!(
        "version = 1\nname = \"x\"\n[seccomp]\n",
        "allow_types = [\"SOCK_STREAM\", \"SOCK_DGRAM\"]\n"
    );
    let err = SandboxProfile::parse(text, Path::new("<probe>"))
        .expect_err("SOCK_DGRAM is not a profile's decision");
    assert_eq!(err.code.as_str(), "CONFIG_003");
    assert!(err.why.contains("seccomp.allow_types"), "{}", err.why);
    assert!(err.why.contains("SOCK_DGRAM"), "{}", err.why);
}

/// Die eine Ausnahme steht im Code, nicht in der Datei: derselbe Text, den
/// [`SandboxProfile::parse`] ablehnt, geht mit
/// `SocketFloor::BrowserUnixIpc` durch, und der Typ bleibt auch dort
/// `SOCK_STREAM`.
#[test]
fn the_browser_escape_hatch_is_the_only_way_to_af_unix() {
    let text = concat!(
        "version = 1\nname = \"browser\"\n[seccomp]\n",
        "allow_families = [\"AF_INET\", \"AF_INET6\", \"AF_UNIX\"]\n"
    );
    let profile =
        SandboxProfile::parse_with_floor(text, Path::new("<probe>"), SocketFloor::BrowserUnixIpc)
            .expect("the hatch is what the browser profile will use");
    assert_eq!(
        profile.seccomp.allow_families,
        vec![
            SocketFamily::AfInet,
            SocketFamily::AfInet6,
            SocketFamily::AfUnix
        ]
    );
    assert_eq!(profile.seccomp.allow_types, vec![SocketType::SockStream]);

    let dgram = concat!(
        "version = 1\nname = \"browser\"\n[seccomp]\n",
        "allow_types = [\"SOCK_STREAM\", \"SOCK_DGRAM\"]\n"
    );
    let err =
        SandboxProfile::parse_with_floor(dgram, Path::new("<probe>"), SocketFloor::BrowserUnixIpc)
            .expect_err("the hatch is one family, not a free pass");
    assert_eq!(err.code.as_str(), "CONFIG_003");
}

/// Der Boden steht auch dann in der Datei, wenn das Profil ihn in anderer
/// Reihenfolge oder doppelt schreibt: der Launcher reicht immer dieselbe
/// Kommaliste an den Shim.
#[test]
fn the_socket_lists_come_back_canonical() {
    let text = concat!(
        "version = 1\nname = \"x\"\n[seccomp]\n",
        "allow_families = [\"AF_INET6\", \"AF_INET\", \"AF_INET6\"]\n"
    );
    let profile = SandboxProfile::parse(text, Path::new("<probe>")).expect("order does not matter");
    assert_eq!(
        profile.seccomp.allow_families,
        vec![SocketFamily::AfInet, SocketFamily::AfInet6]
    );
}

/// Zweite Garantie: genau eine Tür. Eine zweite Bridge mit `dir = "in"` wäre
/// ein zweiter Listener auf einen zweiten Unix-Socket; der Shim öffnet jede
/// Bridge, die er bekommt (Review-Befund vom 2026-09-03).
#[test]
fn a_second_bridge_is_a_second_door() {
    let text = concat!(
        "version = 1\nname = \"x\"\n[network]\n",
        "bridges = [{ name = \"proxy\", dir = \"in\", listen = \"127.0.0.1:3128\", ",
        "socket = \"/run/humanitl/proxy.sock\" },\n",
        "{ name = \"side\", dir = \"in\", listen = \"127.0.0.1:9222\", ",
        "socket = \"/run/humanitl/side.sock\" }]\n"
    );
    let err =
        SandboxProfile::parse(text, Path::new("<probe>")).expect_err("two doors are one too many");
    assert_eq!(err.code.as_str(), "CONFIG_003");
    assert!(err.why.contains("network.bridges"), "{}", err.why);
    assert!(err.why.contains("side"), "{}", err.why);
}

/// Auch die eine Bridge muss die Proxy-Bridge sein: ein anderer Name, ein
/// anderes Ziel oder eine andere Adresse ist eine andere Tür.
#[test]
fn the_only_bridge_must_be_the_proxy_bridge() {
    let cases: [(&str, &str); 3] = [
        (
            concat!(
                "bridges = [{ name = \"cdp\", dir = \"in\", listen = \"127.0.0.1:3128\", ",
                "socket = \"/run/humanitl/proxy.sock\" }]\n"
            ),
            "cdp",
        ),
        (
            concat!(
                "bridges = [{ name = \"proxy\", dir = \"in\", listen = \"127.0.0.1:3128\", ",
                "socket = \"/work/proxy.sock\" }]\n"
            ),
            "/work/proxy.sock",
        ),
        (
            concat!(
                "bridges = [{ name = \"proxy\", dir = \"in\", listen = \"0.0.0.0:3128\", ",
                "socket = \"/run/humanitl/proxy.sock\" }]\n"
            ),
            "0.0.0.0:3128",
        ),
    ];
    for (bridges, needle) in cases {
        let text = format!("version = 1\nname = \"x\"\n[network]\n{bridges}");
        let err = SandboxProfile::parse(&text, Path::new("<probe>"))
            .expect_err("a bridge that is not the proxy bridge must not pass");
        assert_eq!(err.code.as_str(), "CONFIG_003", "{bridges}");
        assert!(err.why.contains(needle), "{bridges}: {}", err.why);
    }
}

/// Das Ziel der Bridge und `network.proxy_socket_dst` sind derselbe Pfad, und
/// dieser Pfad ist fest: der Launcher hängt genau dorthin die Socket-Datei der
/// Sitzung (HUM-013).
#[test]
fn the_proxy_socket_destination_is_fixed() {
    let text = concat!(
        "version = 1\nname = \"x\"\n[network]\n",
        "proxy_socket_dst = \"/run/humanitl/other.sock\"\n",
        "bridges = [{ name = \"proxy\", dir = \"in\", listen = \"127.0.0.1:3128\", ",
        "socket = \"/run/humanitl/other.sock\" }]\n"
    );
    let err =
        SandboxProfile::parse(text, Path::new("<probe>")).expect_err("the one door has one path");
    assert_eq!(err.code.as_str(), "CONFIG_003");
    assert!(err.why.contains("/run/humanitl/proxy.sock"), "{}", err.why);
}

#[test]
fn an_unknown_bridge_direction_does_not_parse() {
    let text = concat!(
        "version = 1\nname = \"x\"\n[network]\n",
        "bridges = [{ name = \"proxy\", dir = \"sideways\", listen = \"127.0.0.1:3128\", ",
        "socket = \"/run/humanitl/proxy.sock\" }]\n"
    );
    let err =
        SandboxProfile::parse(text, Path::new("<probe>")).expect_err("there are two directions");
    assert_eq!(err.code.as_str(), "CONFIG_001");
}

#[test]
fn a_missing_file_names_itself() {
    let err = SandboxProfile::load(Path::new("/nonexistent/profile.toml"))
        .expect_err("a missing profile is a blocking diagnostic");
    assert_eq!(err.code.as_str(), "CONFIG_001");
    assert!(err.why.contains("/nonexistent/profile.toml"), "{}", err.why);
}

#[test]
fn load_validated_refuses_a_forbidden_mount() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("bad.toml");
    std::fs::write(
        &path,
        "version = 1\nname = \"bad\"\n[mounts]\nextra_rw = [\"/var/run/docker.sock\"]\n",
    )
    .expect("write the probe profile");

    let err = SandboxProfile::load_validated(&path, &MountPolicy::new(HOME))
        .expect_err("loading and validating is one step for the launcher");
    assert_eq!(err.code.as_str(), "SANDBOX_006");
    assert!(err.why.contains("mounts.extra_rw"), "{}", err.why);
}

#[test]
fn load_validated_protects_the_xdg_directories_outside_home() {
    // $XDG_DATA_HOME on another disk, $XDG_RUNTIME_DIR outside /run: a policy
    // built from the home directory alone would let both through.
    let temp = tempfile::tempdir().expect("tempdir");
    let env = Env::from_pairs([
        ("HOME", HOME),
        ("XDG_RUNTIME_DIR", "/var/lib/xdg-1000"),
        ("XDG_CONFIG_HOME", "/mnt/cfg"),
        ("XDG_DATA_HOME", "/mnt/data"),
    ])
    .with_uid(1000);
    let policy = MountPolicy::from_env(&env);

    for (whence, source, expect) in [
        ("extra_ro", "/mnt/data/humanitl/ca", "data directory"),
        (
            "extra_ro",
            "/mnt/cfg/humanitl/profiles",
            "configuration directory",
        ),
        (
            "extra_rw",
            "/var/lib/xdg-1000/humanitl",
            "runtime directory",
        ),
    ] {
        let path = temp.path().join(format!("{whence}.toml"));
        std::fs::write(
            &path,
            format!("version = 1\nname = \"bad\"\n[mounts]\n{whence} = [{source:?}]\n"),
        )
        .expect("write the probe profile");
        let err = SandboxProfile::load_validated(&path, &policy)
            .expect_err("the launcher's policy knows every protected directory");
        assert_eq!(err.code.as_str(), "SANDBOX_006", "{source}");
        assert!(err.why.contains(source), "{source}: {}", err.why);
        assert!(err.why.contains(expect), "{source}: {}", err.why);

        SandboxProfile::load_validated(&path, &MountPolicy::new(HOME)).unwrap_or_else(|err| {
            panic!("{source} is exactly what a home-only policy misses: {err}")
        });
    }

    let path = temp.path().join("good.toml");
    std::fs::write(
        &path,
        "version = 1\nname = \"good\"\n[mounts]\nextra_ro = [\"/mnt/data/other\"]\n",
    )
    .expect("write the probe profile");
    SandboxProfile::load_validated(&path, &policy)
        .expect("a sibling of the protected directories stays allowed");
}

#[test]
fn a_custom_runtime_directory_is_forbidden_too() {
    let policy = MountPolicy::new(HOME).with_runtime_dir("/var/lib/xdg-1000");
    let err = with_extra_ro("/var/lib/xdg-1000/humanitl")
        .validate_with(&policy)
        .expect_err("a runtime directory outside /run is still a runtime directory");
    assert_eq!(err.code.as_str(), "SANDBOX_006");
}

#[test]
fn load_validated_refuses_a_loader_variable_in_the_profile_env() {
    // Dieselbe Sperre wie für `sandbox.env` in der Konfiguration: Ein
    // `LD_PRELOAD` läuft im Shim vor `main` und damit vor dem seccomp-Filter.
    // Welche Datei die Zeile trägt, ändert daran nichts.
    for name in humanitl_config::LOADER_ENV_KEYS {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("preload.toml");
        std::fs::write(
            &path,
            format!("version = 1\nname = \"preload\"\n[env]\n{name} = \"/work/evil.so\"\n"),
        )
        .expect("write the probe profile");

        let err = SandboxProfile::load_validated(&path, &MountPolicy::new(HOME))
            .expect_err("a profile must not steer the dynamic linker");
        assert_eq!(err.code.as_str(), "CONFIG_003", "{name}");
        assert!(err.why.contains(name), "{}", err.why);
        assert!(err.why.contains("seccomp"), "{}", err.why);
    }
}

#[test]
fn the_shipped_profiles_carry_no_loader_variable() {
    for name in ["default", "test"] {
        let profile = load(name);
        for key in profile.env.keys() {
            assert!(
                !humanitl_config::is_loader_key(key),
                "{name}.toml sets {key}"
            );
        }
    }
}
