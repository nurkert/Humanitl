//! The guest mode: the shim as PID 1 of a guest kernel (HUM-465, ADR-0021).
//!
//! In the kernel level the agent runs in a micro-VM with a kernel of its own.
//! The guest's root file system is the plan's root, shared read-only from the
//! host; the kernel starts the shim as `init` with exactly two arguments,
//! `--guest <path of the guest plan>`. The mode is chosen by that argument
//! and nothing else, never by a variable: the kernel puts unknown parameters
//! of its command line into the environment of `init`, and the shim empties
//! the environment before it reads anything.
//!
//! Before the steps of the bwrap mode run, the shim does in order:
//!
//! 1. Empty its environment; mount `proc`, `sysfs` and `devtmpfs` (the last
//!    only when the kernel has not mounted it already); bring `lo` up, which
//!    a fresh kernel leaves down and bwrap's `--unshare-net` brings up for
//!    the shared kernel: the agent's door is `127.0.0.1`.
//! 2. Read the guest plan ([`Plan`]), one JSON object in the read-only root.
//! 3. Load the door's kernel modules and the plan's extra ones, then lock
//!    module loading for good (`modules.rs`). The agent never runs on a
//!    kernel that still loads modules.
//! 4. Listen on the plan's report port and take exactly one connection from
//!    the host, waiting at most [`REPORT_WAIT`]; it becomes the report the
//!    bwrap mode gets as a descriptor. Its first line says what step 3 did
//!    (`MODULES vsock=loaded,...,modules_disabled=1`); a failure of step 3 is
//!    written there as `CHECK seccomp_applied fail modules:...` instead.
//!    Without a report nothing starts.
//! 5. Mount `/work` from the plan's tag (`virtiofs`, `nosuid`, `nodev`).
//! 6. Set the shim's own variables and then the agent's environment, both
//!    from the plan, with the rules of `--env-fd` (`envfd.rs`).
//!
//! Then the mount table is compared with the plan as under `--pins-fd`
//! (`mounts.rs`, with the guest forms `tag:<tag>:<ino>` and a pin on any
//! device, HUM-442 Q5), and the bwrap mode follows unchanged: bridges, checks,
//! fork, filters, `exec` of the agent. Two differences: the bridge forwards
//! to the door's vsock port of the host instead of a Unix socket, and the
//! filter of PID 1 allows `AF_VSOCK` instead of `AF_UNIX`; the agent's filter
//! refuses both.
//!
//! PID 1 of a kernel must not return. When the shim is done, or a step above
//! fails, it hands the host the rest of the report ([`linger`]) and powers
//! the guest off ([`power_off`]). Should that fail too, its exit panics the
//! kernel, and `panic=-1` with the VM's `-no-reboot` ends it all the same:
//! that is the lifeline inside the guest.

use std::ffi::{CString, OsString};
use std::fs;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::path::{Path, PathBuf};
use std::ptr;
use std::time::Duration;

use crate::json::Parser;
use crate::linux::envfd;
use crate::linux::modules::{self, Running};
use crate::linux::mounts::{self, Pins};
use crate::linux::vsock;
use crate::linux::{SHIM_VARS, say};
use crate::report::{Check, Report};

/// How long the shim waits for the host to take the report.
pub const REPORT_WAIT: Duration = Duration::from_secs(30);

/// How long the shim waits, after its last line, for the host to close the
/// report before it powers off.
const LINGER: Duration = Duration::from_secs(2);

/// The most bytes a guest plan may have; as for `--env-fd`.
const MAX_PLAN_BYTES: u64 = 1 << 20;

/// Where the project folder appears in the guest.
pub const WORK: &str = "/work";

/// The shim's own variables a guest plan may set; the report comes from the
/// report port, never from a descriptor number.
const PLAN_SHIM_VARS: [&str; 4] = [
    "HUMANITL_BRIDGES",
    "HUMANITL_SECCOMP_FAMILIES",
    "HUMANITL_SECCOMP_TYPES",
    "HUMANITL_SECCOMP_DENY",
];

/// The guest plan: one JSON object, written by the daemon into the guest's
/// read-only root.
///
/// ```text
/// {"proxy_port":"3128", "door_port":"5000", "report_port":"5001",
///  "modules_dir":"/run/humanitl/modules", "modules":["overlay"],
///  "work_tag":"work",
///  "shim":{"HUMANITL_SECCOMP_DENY":"mount"},
///  "env":{"PATH":"/usr/bin:/bin"},
///  "pins":{"pins":[...],"mounts":[...]},
///  "command":["/bin/bash","-l"]}
/// ```
///
/// Every key is required but `modules` and `shim`; the ports are decimal
/// strings. `env` follows the rules of `--env-fd`, `pins` the shape of
/// `--pins-fd` with the guest forms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// The port of the agent's `HTTP_PROXY`, on `127.0.0.1`.
    pub proxy_port: u16,
    /// The vsock port of the host that leads to the proxy.
    pub door_port: u32,
    /// The vsock port of the guest on which the host takes the report.
    pub report_port: u32,
    /// Where the modules of the guest kernel are, inside the guest.
    pub modules_dir: PathBuf,
    /// Modules to load after the door's, before the lock (HUM-446: `overlay`).
    pub modules: Vec<String>,
    /// The tag of the shared project folder.
    pub work_tag: String,
    /// The shim's own variables.
    pub shim_env: Vec<(String, String)>,
    /// The agent's environment.
    pub env: Vec<(String, String)>,
    /// The expected mount table and pins.
    pub pins: Pins,
    /// The agent's command.
    pub command: Vec<OsString>,
}

/// The keys a guest plan may have.
const KEYS: [&str; 10] = [
    "proxy_port",
    "door_port",
    "report_port",
    "modules_dir",
    "modules",
    "work_tag",
    "shim",
    "env",
    "pins",
    "command",
];

/// A reason without spaces: it becomes evidence on a report line.
fn fail(what: &str, err: &dyn std::fmt::Display) -> String {
    format!("{what}:{err}").replace(' ', "_")
}

/// The keys of the top-level object with their values unparsed; every key
/// known and at most once.
fn top_level(text: &str) -> Result<Vec<(String, &str)>, String> {
    let mut parser = Parser::new(text);
    let mut fields: Vec<(String, &str)> = Vec::new();
    parser.skip_ws();
    parser
        .expect('{', "expected '{'")
        .map_err(|e| fail("json", &e))?;
    loop {
        parser.skip_ws();
        let key = parser.string().map_err(|e| fail("json", &e))?;
        parser.skip_ws();
        parser
            .expect(':', "expected ':'")
            .map_err(|e| fail("json", &e))?;
        let raw = parser.raw_value().map_err(|e| fail(&key, &e))?;
        if !KEYS.contains(&key.as_str()) {
            return Err(format!("{key}:unknown-key"));
        }
        if fields.iter().any(|(seen, _)| *seen == key) {
            return Err(format!("{key}:given-twice"));
        }
        fields.push((key, raw));
        parser.skip_ws();
        match parser.next() {
            Some(',') => {}
            Some('}') => break,
            _ => return Err("json:expected-','-or-'}'".to_owned()),
        }
    }
    parser.end().map_err(|e| fail("json", &e))?;
    Ok(fields)
}

/// Parses a guest plan.
///
/// # Errors
///
/// A short reason without spaces for anything but the documented shape.
pub fn parse(text: &str) -> Result<Plan, String> {
    let fields = top_level(text)?;
    let raw = |key: &'static str| fields.iter().find(|(k, _)| k == key).map(|(_, raw)| *raw);
    let required = |key: &'static str| raw(key).ok_or_else(|| format!("{key}:missing"));
    let string = |key: &'static str| -> Result<String, String> {
        Parser::new(required(key)?)
            .string()
            .map_err(|e| fail(key, &e))
    };
    let port = |key: &'static str| -> Result<u32, String> {
        let text = string(key)?;
        text.parse::<u32>()
            .ok()
            .filter(|port| *port != 0 && *port != u32::MAX)
            .ok_or_else(|| format!("{key}:not-a-port:{text}"))
    };

    let proxy_port = string("proxy_port")?;
    let proxy_port = proxy_port
        .parse::<u16>()
        .ok()
        .filter(|port| *port != 0)
        .ok_or_else(|| format!("proxy_port:not-a-port:{proxy_port}"))?;
    let door_port = port("door_port")?;
    let report_port = port("report_port")?;
    if door_port == report_port {
        return Err("report_port:same-as-door_port".to_owned());
    }
    let modules_dir = PathBuf::from(string("modules_dir")?);
    if !modules_dir.is_absolute() {
        return Err("modules_dir:not-absolute".to_owned());
    }
    let modules = match raw("modules") {
        None => Vec::new(),
        Some(text) => Parser::new(text)
            .string_array()
            .map_err(|e| fail("modules", &e))?,
    };
    let work_tag = string("work_tag")?;
    if work_tag.is_empty() || work_tag.contains(|c: char| c.is_whitespace() || c == '\0') {
        return Err("work_tag:bad".to_owned());
    }
    let shim_env = match raw("shim") {
        None => Vec::new(),
        Some(text) => Parser::new(text)
            .string_object()
            .map_err(|e| fail("shim", &e))?,
    };
    for (index, (key, value)) in shim_env.iter().enumerate() {
        if !PLAN_SHIM_VARS.contains(&key.as_str()) {
            return Err(format!("shim:not-a-shim-variable:{key}"));
        }
        if value.contains('\0') || shim_env[..index].iter().any(|(k, _)| k == key) {
            return Err(format!("shim:bad:{key}"));
        }
    }
    let env = envfd::parse(required("env")?, &SHIM_VARS).map_err(|e| fail("env", &e))?;
    let pins = mounts::parse_guest_pins(required("pins")?).map_err(|e| fail("pins", &e))?;
    let command: Vec<OsString> = Parser::new(required("command")?)
        .string_array()
        .map_err(|e| fail("command", &e))?
        .into_iter()
        .map(OsString::from)
        .collect();
    if command.is_empty() {
        return Err("command:empty".to_owned());
    }
    Ok(Plan {
        proxy_port,
        door_port,
        report_port,
        modules_dir,
        modules,
        work_tag,
        shim_env,
        env,
        pins,
        command,
    })
}

/// Everything the guest mode prepared: the report and the plan.
pub struct Booted {
    /// The report, on the connection the host made.
    pub report: Report,
    /// The plan.
    pub plan: Plan,
}

/// Steps 1 to 6 of the module documentation. `None` when one failed; then
/// the reason is on stderr and, once the report exists, on the report.
pub fn boot(path: &Path) -> Option<Booted> {
    // SAFETY: still single-threaded: nothing reads the environment
    // concurrently, and nothing holds a pointer into it.
    unsafe {
        libc::clearenv();
    }
    if let Err(why) = mount_basics() {
        say(&format!("humanitl-shim: guest: {why}\n"));
        return None;
    }
    let plan = match read_plan(path) {
        Ok(plan) => plan,
        Err(why) => {
            say(&format!(
                "humanitl-shim: guest: the plan {} is unusable: {why}\n",
                path.display()
            ));
            return None;
        }
    };
    let modules = Running::open(&plan.modules_dir)
        .and_then(|mut kernel| modules::load_and_lock(&mut kernel, &plan.modules));
    let report = match vsock::accept_one(plan.report_port, REPORT_WAIT) {
        Ok(stream) => Report::from_fd(stream.into_fd()),
        Err(err) => {
            let modules = match &modules {
                Ok(_) => String::new(),
                Err(why) => format!(" (modules: {why})"),
            };
            say(&format!(
                "humanitl-shim: guest: the host did not take the report on port {}: {err}{modules}\n",
                plan.report_port
            ));
            return None;
        }
    };
    match modules {
        // Not a check of its own: the CHECK lines stay those of every
        // backend. A reader that knows no `MODULES` line skips it.
        Ok(done) => report.line(&format!("MODULES {}", modules::evidence(&done))),
        Err(why) => {
            report.check(Check::SeccompApplied, false, &format!("modules:{why}"));
            say(&format!("humanitl-shim: guest: modules: {why}\n"));
            linger(&report);
            return None;
        }
    }
    if let Err(why) = guest_families(&plan.shim_env) {
        report.check(Check::SeccompApplied, false, &why);
        say(&format!("humanitl-shim: guest: {why}\n"));
        linger(&report);
        return None;
    }
    if let Err(err) = mount(
        &plan.work_tag,
        WORK,
        "virtiofs",
        libc::MS_NOSUID | libc::MS_NODEV,
    ) {
        let evidence = format!("work-mount:{}:{err}", plan.work_tag);
        report.check(Check::MountsMatch, false, &evidence);
        report.check(Check::MountsPinned, false, &evidence);
        say(&format!("humanitl-shim: guest: {evidence}\n"));
        linger(&report);
        return None;
    }
    for (key, value) in &plan.shim_env {
        // SAFETY: single-threaded (above); `parse` allowed only the shim's
        // own names and refused a NUL in a value.
        unsafe { std::env::set_var(key, value) };
    }
    // SAFETY: single-threaded (above).
    unsafe { envfd::apply(&plan.env, &SHIM_VARS) };
    Some(Booted { report, plan })
}

/// The families the agent may use in a guest kernel: those of the shared
/// kernel without `AF_UNIX`. The door is the bridge's alone; a profile that
/// asks for `AF_UNIX` (or anything else) is refused before anything starts,
/// and the agent's policy drops `AF_UNIX` regardless
/// (`Policy::for_guest_agent`).
const GUEST_FAMILIES: [&str; 2] = ["AF_INET", "AF_INET6"];

/// Checks `HUMANITL_SECCOMP_FAMILIES` of the plan against
/// [`GUEST_FAMILIES`]; absent means the default, which fits.
///
/// # Errors
///
/// The evidence of the refusal, without spaces.
pub fn guest_families(shim_env: &[(String, String)]) -> Result<(), String> {
    let Some((_, list)) = shim_env
        .iter()
        .find(|(key, _)| key == "HUMANITL_SECCOMP_FAMILIES")
    else {
        return Ok(());
    };
    let refused: Vec<&str> = list
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty() && !GUEST_FAMILIES.contains(name))
        .collect();
    if refused.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "families:not-in-a-guest:{}",
            refused.join(",").replace(' ', "_")
        ))
    }
}

/// Reads and parses the plan, bounded.
fn read_plan(path: &Path) -> Result<Plan, String> {
    let size = fs::metadata(path)
        .map_err(|err| format!("stat:{err}"))?
        .len();
    if size > MAX_PLAN_BYTES {
        return Err(format!("larger-than-{MAX_PLAN_BYTES}-bytes"));
    }
    let text = fs::read_to_string(path).map_err(|err| format!("read:{err}"))?;
    parse(&text)
}

/// `proc`, `sysfs` and `devtmpfs`, each `nosuid` and `noexec`, the first two
/// also `nodev`. `devtmpfs` only when the kernel has not mounted it already
/// (`CONFIG_DEVTMPFS_MOUNT`); a second one on `/dev` would be one mount more
/// than the plan says.
fn mount_basics() -> Result<(), String> {
    let quiet = libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC;
    mount("proc", "/proc", "proc", quiet).map_err(|err| format!("mount /proc: {err}"))?;
    mount("sysfs", "/sys", "sysfs", quiet).map_err(|err| format!("mount /sys: {err}"))?;
    let mountinfo =
        fs::read_to_string("/proc/self/mountinfo").map_err(|err| format!("mountinfo: {err}"))?;
    let dev_mounted = mounts::parse_mountinfo(&mountinfo)
        .iter()
        .any(|mount| mount.point == "/dev");
    if !dev_mounted {
        mount(
            "devtmpfs",
            "/dev",
            "devtmpfs",
            libc::MS_NOSUID | libc::MS_NOEXEC,
        )
        .map_err(|err| format!("mount /dev: {err}"))?;
    }
    loopback_up().map_err(|err| format!("lo up: {err}"))
}

/// Sets `IFF_UP` on `lo`, the one interface a guest without a network
/// device has.
fn loopback_up() -> io::Result<()> {
    // SAFETY: socket takes three integers.
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM | libc::SOCK_CLOEXEC, 0) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: socket just returned this descriptor, and nothing else owns it.
    let socket = unsafe { OwnedFd::from_raw_fd(fd) };
    // SAFETY: ifreq is plain data; all zero is a valid value.
    let mut request: libc::ifreq = unsafe { std::mem::zeroed() };
    for (slot, byte) in request.ifr_name.iter_mut().zip(b"lo") {
        *slot = libc::c_char::from_ne_bytes([*byte]);
    }
    // SAFETY: SIOCGIFFLAGS fills the flags of the named interface in a valid
    // ifreq.
    if unsafe {
        libc::ioctl(
            socket.as_raw_fd(),
            libc::SIOCGIFFLAGS as libc::Ioctl,
            &raw mut request,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the union member the call above filled.
    let flags = unsafe { request.ifr_ifru.ifru_flags };
    request.ifr_ifru.ifru_flags = flags | libc::c_short::try_from(libc::IFF_UP).unwrap_or(1);
    // SAFETY: SIOCSIFFLAGS reads a valid ifreq.
    if unsafe {
        libc::ioctl(
            socket.as_raw_fd(),
            libc::SIOCSIFFLAGS as libc::Ioctl,
            &raw const request,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// `mount(2)` without data.
fn mount(source: &str, target: &str, fstype: &str, flags: libc::c_ulong) -> io::Result<()> {
    let text = |value: &str| {
        CString::new(value).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL byte"))
    };
    let (source, target, fstype) = (text(source)?, text(target)?, text(fstype)?);
    // SAFETY: three NUL-terminated strings that outlive the call, no data.
    let rc = unsafe {
        libc::mount(
            source.as_ptr(),
            target.as_ptr(),
            fstype.as_ptr(),
            flags,
            ptr::null(),
        )
    };
    if rc == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// A second descriptor on the report's connection, for [`linger`] after
/// the report itself is gone.
pub fn keep(report: &Report) -> Option<OwnedFd> {
    let fd = report.fd()?;
    // SAFETY: F_DUPFD_CLOEXEC copies an open descriptor this process owns.
    let copy = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 3) };
    if copy < 0 {
        return None;
    }
    // SAFETY: fcntl just returned this descriptor, and nothing else owns it.
    Some(unsafe { OwnedFd::from_raw_fd(copy) })
}

/// Hands the host the end of the report: half-closes it and waits at most
/// [`LINGER`] for the host to close its side, so that no line is still
/// queued in the device when the guest powers off.
pub fn linger(report: &Report) {
    if let Some(fd) = report.fd() {
        linger_fd(fd);
    }
}

/// [`linger`] on a descriptor of the report's connection.
pub fn linger_fd(fd: RawFd) {
    // SAFETY: shutdown takes a descriptor and a constant; a descriptor that is
    // no socket answers ENOTSOCK, which changes nothing.
    unsafe {
        libc::shutdown(fd, libc::SHUT_WR);
    }
    let mut poll = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    let millis = libc::c_int::try_from(LINGER.as_millis()).unwrap_or(2000);
    let mut buffer = [0u8; 256];
    // SAFETY: one pollfd, valid for the call.
    while unsafe { libc::poll(&raw mut poll, 1, millis) } > 0 {
        // SAFETY: the buffer is valid for its length and outlives the call.
        let n = unsafe { libc::read(fd, buffer.as_mut_ptr().cast(), buffer.len()) };
        if n <= 0 {
            break;
        }
    }
}

/// Ends the guest: flushes the file systems and powers off. Returns only when
/// the kernel refused (a profile may deny `reboot` to PID 1 as well); the
/// caller then exits, and the kernel's panic ends the VM.
pub fn power_off() {
    // SAFETY: sync and reboot take no pointers; RB_POWER_OFF is a constant.
    unsafe {
        libc::sync();
        libc::reboot(libc::RB_POWER_OFF);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    const PLAN: &str = r#"{
        "proxy_port": "3128", "door_port": "5000", "report_port": "5001",
        "modules_dir": "/run/humanitl/modules", "modules": ["overlay"],
        "work_tag": "work",
        "shim": {"HUMANITL_SECCOMP_DENY": "mount"},
        "env": {"PATH": "/usr/bin:/bin", "HOME": "/work"},
        "pins": {"pins": [{"path": "/work", "dev": "*", "ino": "131"}],
                 "mounts": [{"point": "/work", "type": "virtiofs", "source": "tag:work:131",
                             "mode": "rw", "flags": "nosuid,nodev", "count": "1"}]},
        "command": ["/bin/bash", "-c", "echo hi"]
    }"#;

    #[test]
    fn the_documented_plan_reads() {
        let plan = parse(PLAN).unwrap();
        assert_eq!(plan.proxy_port, 3128);
        assert_eq!((plan.door_port, plan.report_port), (5000, 5001));
        assert_eq!(plan.modules_dir, PathBuf::from("/run/humanitl/modules"));
        assert_eq!(plan.modules, ["overlay"]);
        assert_eq!(plan.work_tag, "work");
        assert_eq!(
            plan.shim_env,
            [("HUMANITL_SECCOMP_DENY".to_owned(), "mount".to_owned())]
        );
        assert_eq!(plan.env.len(), 2);
        assert_eq!(plan.pins.pins[0].dev, None);
        assert_eq!(
            plan.command,
            ["/bin/bash", "-c", "echo hi"].map(OsString::from)
        );
        let bare = PLAN
            .replace(r#""modules": ["overlay"],"#, "")
            .replace(r#""shim": {"HUMANITL_SECCOMP_DENY": "mount"},"#, "");
        let plan = parse(&bare).unwrap();
        assert!(plan.modules.is_empty() && plan.shim_env.is_empty());
    }

    /// Point 4 of the specification: the agent's filter in a guest refuses
    /// `AF_UNIX` and `AF_VSOCK`; a plan that would allow either is refused.
    #[test]
    fn a_plan_that_allows_the_agent_unix_or_vsock_is_refused() {
        let env =
            |families: &str| vec![("HUMANITL_SECCOMP_FAMILIES".to_owned(), families.to_owned())];
        assert_eq!(guest_families(&[]), Ok(()));
        assert_eq!(guest_families(&env("AF_INET,AF_INET6")), Ok(()));
        assert_eq!(guest_families(&env(" AF_INET6 ")), Ok(()));
        assert_eq!(
            guest_families(&env("AF_INET,AF_UNIX")),
            Err("families:not-in-a-guest:AF_UNIX".to_owned())
        );
        assert_eq!(
            guest_families(&env("AF_VSOCK")),
            Err("families:not-in-a-guest:AF_VSOCK".to_owned())
        );
        assert!(guest_families(&env("AF_NETLINK,AF_INET")).is_err());
    }

    #[test]
    fn every_deviation_is_refused_with_its_key() {
        let cases: &[(&str, &str, &str)] = &[
            (
                r#""door_port": "5000""#,
                r#""door_port": "5001""#,
                "same-as-door_port",
            ),
            (
                r#""door_port": "5000""#,
                r#""door_port": "0""#,
                "door_port:not-a-port",
            ),
            (
                r#""proxy_port": "3128""#,
                r#""proxy_port": "70000""#,
                "proxy_port:not-a-port",
            ),
            (
                r#""report_port": "5001""#,
                r#""report_port": 5001"#,
                "report_port:",
            ),
            (
                r#""/run/humanitl/modules""#,
                r#""modules""#,
                "modules_dir:not-absolute",
            ),
            (
                r#""work_tag": "work""#,
                r#""work_tag": "a b""#,
                "work_tag:bad",
            ),
            (
                r#""HUMANITL_SECCOMP_DENY""#,
                r#""HUMANITL_REPORT_FD""#,
                "shim:not-a-shim-variable:HUMANITL_REPORT_FD",
            ),
            (r#""HOME": "/work""#, r#""LD_PRELOAD": "/x.so""#, "env:"),
            (r#""HOME": "/work""#, r#""HUMANITL_BRIDGES": "[]""#, "env:"),
            (r#""dev": "*""#, r#""dev": "x""#, "pins:"),
            (r#"["/bin/bash", "-c", "echo hi"]"#, "[]", "command:empty"),
            (
                r#""work_tag": "work","#,
                r#""work_tag": "work", "x": "y","#,
                "x:unknown-key",
            ),
            (
                r#""work_tag": "work","#,
                r#""work_tag": "work", "work_tag": "w","#,
                "given-twice",
            ),
            (r#""work_tag": "work","#, "", "work_tag:missing"),
        ];
        for (from, to, needle) in cases {
            assert!(PLAN.contains(from), "{from}");
            let text = PLAN.replacen(from, to, 1);
            let err = parse(&text).unwrap_err();
            assert!(err.contains(needle), "{from} -> {to}: {err}");
            assert!(!err.contains(' '), "{err}");
        }
        assert!(parse("").is_err());
        assert!(parse(&format!("{PLAN} x")).is_err());
    }
}
