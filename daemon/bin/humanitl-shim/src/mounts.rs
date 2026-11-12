//! The mount table equals the plan, and every checked folder is the one that
//! was checked (HUM-437).
//!
//! The launcher checks every folder it mounts and opens it once (HUM-263).
//! bwrap mounts the opened descriptor; a container engine takes a path and
//! resolves it later, in another process (HUM-292, measurement P13: a
//! descriptor is no mount source there). And an engine reads configuration of
//! its own that can add a mount to every sandbox (`mounts.conf`, OCI hooks).
//! A mounted directory of secrets holds no socket, so the socket walk of the
//! second guarantee would not see it. Two checks close both gaps, before the
//! agent exists, under every backend alike:
//!
//! - `mounts_match`: `/proc/self/mountinfo` is compared with the expected
//!   table as a **multiset** of entries. An entry is the mount point, the
//!   file system type, the source, read-only or writable, and the flags
//!   `nosuid`, `nodev`, `noexec`. The source of a bind is the device and
//!   inode of the bound object, read with `lstat` on the mount point; it
//!   counts only for the topmost mount on that point, so a second mount
//!   stacked onto an expected point is always one entry too many. The source
//!   of every other mount is its type. An entry the backend itself adds and
//!   whose backing file the launcher cannot know (a device node, a masked
//!   path of `/proc`) carries the source "any", and a field the launcher
//!   cannot predict carries `*`. A missing, an extra or a different entry is
//!   `fail`.
//! - `mounts_pinned`: `lstat` (never following a symbolic link) of every
//!   checked target, `/work`, the entries of `mounts.ro`, `extra_ro`,
//!   `extra_rw` and the agent program, gives the device and inode the
//!   launcher saw when it opened the source. A different pair is `fail`, with
//!   the path, the expected and the found pair.
//!
//! Both are written as `CHECK` lines before any other, and either `fail` ends
//! the start with exit 126: the agent never runs. The launcher reads them as
//! part of the second guarantee (`SANDBOX_049`, `SANDBOX_042`).
//!
//! **The descriptor.** `--pins-fd <n>` names a sealed memfd with one JSON
//! object, all values strings, read with the shim's own small reader
//! (`json.rs`):
//!
//! ```text
//! {"pins":[{"path":"/work","dev":"2049","ino":"131"}],
//!  "mounts":[{"point":"/work","type":"*","source":"inode:2049:131",
//!             "mode":"rw","flags":"nosuid,nodev","count":"1"}]}
//! ```
//!
//! `source` is `inode:<dev>:<ino>`, `type` or `any`; `mode` is `ro`, `rw` or
//! `*`; `flags` is a comma list of `nosuid`, `nodev`, `noexec` (empty for
//! none) or `*`; `count` is `n` or `min-max`. A descriptor that is missing,
//! not sealed or not of this shape refuses the start like a mismatch.

use std::fmt::{self, Write as _};
use std::os::fd::RawFd;
use std::os::unix::fs::MetadataExt as _;

use crate::json::{self, Parser};
use crate::linux::envfd;
use crate::report::{Check, Report};

/// Where the source of an expected mount is known from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// A bind: device and inode of the bound object.
    Inode(u64, u64),
    /// Any other mount: its file system type is its source.
    Type,
    /// A backend's own entry whose backing object the launcher cannot know.
    Any,
}

/// Read-only, writable, or either.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// `ro`.
    ReadOnly,
    /// `rw`.
    ReadWrite,
    /// `*`.
    Any,
}

/// The three flags that decide what a mount can be used for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Flags {
    /// `nosuid`.
    pub nosuid: bool,
    /// `nodev`.
    pub nodev: bool,
    /// `noexec`.
    pub noexec: bool,
}

impl Flags {
    /// The flags in a comma list of mount options; anything else is ignored.
    fn of_options(options: &str) -> Self {
        let mut flags = Self::default();
        for option in options.split(',') {
            match option {
                "nosuid" => flags.nosuid = true,
                "nodev" => flags.nodev = true,
                "noexec" => flags.noexec = true,
                _ => {}
            }
        }
        flags
    }
}

impl fmt::Display for Flags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = [
            (self.nosuid, "nosuid"),
            (self.nodev, "nodev"),
            (self.noexec, "noexec"),
        ]
        .into_iter()
        .filter_map(|(set, name)| set.then_some(name))
        .collect();
        if names.is_empty() {
            write!(f, "-")
        } else {
            write!(f, "{}", names.join("+"))
        }
    }
}

/// One entry of the expected table, with how often it may occur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expected {
    /// The mount point.
    pub point: String,
    /// The file system type; `None` is `*`.
    pub fstype: Option<String>,
    /// The source.
    pub source: Source,
    /// Read-only or writable.
    pub mode: Mode,
    /// The three flags; `None` is `*`.
    pub flags: Option<Flags>,
    /// At least this often.
    pub min: u32,
    /// At most this often.
    pub max: u32,
}

impl fmt::Display for Expected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let source = match self.source {
            Source::Inode(dev, ino) => format!("inode={dev}:{ino}"),
            Source::Type => "type".to_owned(),
            Source::Any => "any".to_owned(),
        };
        let mode = match self.mode {
            Mode::ReadOnly => "ro",
            Mode::ReadWrite => "rw",
            Mode::Any => "*",
        };
        let flags = self
            .flags
            .map_or_else(|| "*".to_owned(), |flags| flags.to_string());
        write!(
            f,
            "{}:{}:{source}:{mode}:{flags}",
            self.point,
            self.fstype.as_deref().unwrap_or("*")
        )?;
        if (self.min, self.max) != (1, 1) {
            write!(f, ":{}-{}", self.min, self.max)?;
        }
        Ok(())
    }
}

/// A checked target and the device and inode it must have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pin {
    /// The path in the sandbox.
    pub path: String,
    /// `st_dev`.
    pub dev: u64,
    /// `st_ino`.
    pub ino: u64,
}

/// Everything behind `--pins-fd`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Pins {
    /// The checked targets.
    pub pins: Vec<Pin>,
    /// The expected mount table.
    pub mounts: Vec<Expected>,
}

/// One line of `/proc/self/mountinfo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actual {
    /// The mount ID.
    pub id: u64,
    /// The ID of the mount below.
    pub parent: u64,
    /// The mount point, unescaped.
    pub point: String,
    /// The file system type.
    pub fstype: String,
    /// Mounted read-only.
    pub read_only: bool,
    /// The three flags of the mount.
    pub flags: Flags,
    /// Nothing is stacked onto this mount on the same point.
    pub top: bool,
}

impl fmt::Display for Actual {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}:{}",
            self.point,
            self.fstype,
            if self.read_only { "ro" } else { "rw" },
            self.flags
        )?;
        if !self.top {
            write!(f, ":covered")?;
        }
        Ok(())
    }
}

/// The result of the table comparison.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Mismatch {
    /// Expected entries that did not occur often enough, with how often they
    /// did.
    pub missing: Vec<String>,
    /// Mounts no expected entry took.
    pub extra: Vec<String>,
}

impl Mismatch {
    /// Nothing missing, nothing extra.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.missing.is_empty() && self.extra.is_empty()
    }
}

/// Reads and checks the descriptor, compares, writes both `CHECK` lines and
/// on failure the two tables on stderr. `true` when the agent may start.
pub fn verify(fd: RawFd, report: &Report) -> bool {
    let pins = match read(fd) {
        Ok(pins) => pins,
        Err(why) => {
            let evidence = format!("pins-unreadable:{why}");
            report.check(Check::MountsMatch, false, &evidence);
            report.check(Check::MountsPinned, false, &evidence);
            crate::linux::say(&format!(
                "humanitl-shim: mount check failed: --pins-fd: {why}\n"
            ));
            return false;
        }
    };
    let stat = |path: &str| {
        std::fs::symlink_metadata(path)
            .ok()
            .map(|meta| (meta.dev(), meta.ino()))
    };
    let mountinfo = std::fs::read_to_string("/proc/self/mountinfo");
    let table_ok = match &mountinfo {
        Ok(text) => {
            let actual = parse_mountinfo(text);
            let mismatch = compare(&pins.mounts, &actual, &stat);
            if mismatch.is_empty() {
                report.check(
                    Check::MountsMatch,
                    true,
                    &format!("entries={}", actual.len()),
                );
                true
            } else {
                report.check(
                    Check::MountsMatch,
                    false,
                    &format!(
                        "missing={};extra={}",
                        list(&mismatch.missing),
                        list(&mismatch.extra)
                    ),
                );
                // The whole picture, once, for whoever has to correct the
                // plan or find the intruder: every line as the kernel wrote
                // it, and every expectation.
                let mut text_out =
                    String::from("humanitl-shim: mount table differs from the plan\n");
                for line in text.lines() {
                    text_out.push_str("  mountinfo: ");
                    text_out.push_str(line);
                    text_out.push('\n');
                }
                for entry in &pins.mounts {
                    let _ = writeln!(text_out, "  expected: {entry}");
                }
                let _ = writeln!(
                    text_out,
                    "  missing: {}\n  extra: {}",
                    list(&mismatch.missing),
                    list(&mismatch.extra)
                );
                crate::linux::say(&text_out);
                false
            }
        }
        Err(err) => {
            report.check(
                Check::MountsMatch,
                false,
                &format!("mountinfo-unreadable:{err}"),
            );
            crate::linux::say(&format!(
                "humanitl-shim: mount check failed: /proc/self/mountinfo: {err}\n"
            ));
            false
        }
    };
    let wrong = check_pins(&pins.pins, &stat);
    let pins_ok = wrong.is_empty();
    if pins_ok {
        report.check(
            Check::MountsPinned,
            true,
            &format!("pins={}", pins.pins.len()),
        );
    } else {
        report.check(Check::MountsPinned, false, &wrong.join(","));
        crate::linux::say(&format!(
            "humanitl-shim: a mounted folder is not the checked one: {}\n",
            wrong.join(", ")
        ));
    }
    table_ok && pins_ok
}

/// The descriptor's content, parsed.
fn read(fd: RawFd) -> Result<Pins, String> {
    let bytes = envfd::read_sealed(fd).map_err(|err| match err {
        envfd::Error::NotOpen(fd) => format!("{fd}-is-not-open"),
        envfd::Error::NotSealed => "not-a-sealed-memfd".to_owned(),
        envfd::Error::TooLarge => "too-large".to_owned(),
        other => other.to_string(),
    })?;
    let text = String::from_utf8(bytes).map_err(|_| "not-utf8".to_owned())?;
    parse_pins(&text).map_err(|err| err.to_string())
}

/// A comma list for one field of the evidence.
fn list(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_owned()
    } else {
        values.join(",")
    }
}

/// Why the JSON behind `--pins-fd` was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinsError {
    /// Not the shape of the module documentation.
    Json(json::Error),
    /// A field is missing or its value is not what the field takes.
    Field(&'static str, String),
}

impl fmt::Display for PinsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(err) => write!(f, "{err}"),
            Self::Field(field, value) => write!(f, "field {field}: {value:?}"),
        }
    }
}

impl From<json::Error> for PinsError {
    fn from(err: json::Error) -> Self {
        Self::Json(err)
    }
}

/// Parses the JSON behind `--pins-fd`.
///
/// # Errors
///
/// [`PinsError`] for anything but the shape of the module documentation.
pub fn parse_pins(text: &str) -> Result<Pins, PinsError> {
    let mut parser = Parser::new(text);
    let mut pins = Pins::default();
    let (mut seen_pins, mut seen_mounts) = (false, false);
    parser.skip_ws();
    parser.expect('{', "expected '{'")?;
    loop {
        parser.skip_ws();
        let key = parser.string()?;
        parser.skip_ws();
        parser.expect(':', "expected ':'")?;
        let objects = object_list(&mut parser)?;
        match key.as_str() {
            "pins" if !seen_pins => {
                seen_pins = true;
                for object in &objects {
                    pins.pins.push(Pin {
                        path: absolute(field(object, "path")?)?,
                        dev: number(object, "dev")?,
                        ino: number(object, "ino")?,
                    });
                }
            }
            "mounts" if !seen_mounts => {
                seen_mounts = true;
                for object in &objects {
                    pins.mounts.push(expected(object)?);
                }
            }
            _ => return Err(PinsError::Field("key", key)),
        }
        parser.skip_ws();
        match parser.next() {
            Some(',') => {}
            Some('}') => break,
            _ => return Err(parser.error("expected ',' or '}'").into()),
        }
    }
    parser.end()?;
    if !seen_pins || !seen_mounts {
        return Err(PinsError::Field("pins/mounts", "missing".to_owned()));
    }
    Ok(pins)
}

/// An array of flat objects of strings.
fn object_list(parser: &mut Parser<'_>) -> Result<Vec<Vec<(String, String)>>, json::Error> {
    parser.skip_ws();
    parser.expect('[', "expected '['")?;
    let mut objects = Vec::new();
    parser.skip_ws();
    if parser.peek() == Some(']') {
        parser.next();
        return Ok(objects);
    }
    loop {
        parser.skip_ws();
        parser.expect('{', "expected '{'")?;
        let mut pairs = Vec::new();
        loop {
            parser.skip_ws();
            let key = parser.string()?;
            parser.skip_ws();
            parser.expect(':', "expected ':'")?;
            parser.skip_ws();
            let value = parser.string()?;
            pairs.push((key, value));
            parser.skip_ws();
            match parser.next() {
                Some(',') => {}
                Some('}') => break,
                _ => return Err(parser.error("expected ',' or '}'")),
            }
        }
        objects.push(pairs);
        parser.skip_ws();
        match parser.next() {
            Some(',') => {}
            Some(']') => return Ok(objects),
            _ => return Err(parser.error("expected ',' or ']'")),
        }
    }
}

fn field<'a>(object: &'a [(String, String)], name: &'static str) -> Result<&'a str, PinsError> {
    let mut found = object.iter().filter(|(key, _)| key == name);
    match (found.next(), found.next()) {
        (Some((_, value)), None) => Ok(value),
        _ => Err(PinsError::Field(name, "missing or given twice".to_owned())),
    }
}

fn number(object: &[(String, String)], name: &'static str) -> Result<u64, PinsError> {
    let value = field(object, name)?;
    value
        .parse()
        .map_err(|_| PinsError::Field(name, value.to_owned()))
}

fn absolute(path: &str) -> Result<String, PinsError> {
    if path.starts_with('/') {
        Ok(path.to_owned())
    } else {
        Err(PinsError::Field("path", path.to_owned()))
    }
}

fn expected(object: &[(String, String)]) -> Result<Expected, PinsError> {
    for (key, _) in object {
        if !["point", "type", "source", "mode", "flags", "count"].contains(&key.as_str()) {
            return Err(PinsError::Field("key", key.clone()));
        }
    }
    let point = absolute(field(object, "point")?)?;
    let fstype = match field(object, "type")? {
        "*" => None,
        "" => return Err(PinsError::Field("type", String::new())),
        other => Some(other.to_owned()),
    };
    let source_text = field(object, "source")?;
    let source = match source_text {
        "type" => Source::Type,
        "any" => Source::Any,
        other => {
            let bad = || PinsError::Field("source", other.to_owned());
            let rest = other.strip_prefix("inode:").ok_or_else(bad)?;
            let (dev, ino) = rest.split_once(':').ok_or_else(bad)?;
            Source::Inode(
                dev.parse().map_err(|_| bad())?,
                ino.parse().map_err(|_| bad())?,
            )
        }
    };
    if source == Source::Type && fstype.is_none() {
        return Err(PinsError::Field("source", "type without a type".to_owned()));
    }
    let mode = match field(object, "mode")? {
        "ro" => Mode::ReadOnly,
        "rw" => Mode::ReadWrite,
        "*" => Mode::Any,
        other => return Err(PinsError::Field("mode", other.to_owned())),
    };
    let flags = match field(object, "flags")? {
        "*" => None,
        text => {
            let mut flags = Flags::default();
            for name in text.split(',').filter(|name| !name.is_empty()) {
                match name {
                    "nosuid" => flags.nosuid = true,
                    "nodev" => flags.nodev = true,
                    "noexec" => flags.noexec = true,
                    other => return Err(PinsError::Field("flags", other.to_owned())),
                }
            }
            Some(flags)
        }
    };
    let count = field(object, "count")?;
    let bad_count = || PinsError::Field("count", count.to_owned());
    let (min, max) = if let Some((min, max)) = count.split_once('-') {
        (
            min.parse().map_err(|_| bad_count())?,
            max.parse().map_err(|_| bad_count())?,
        )
    } else {
        let n: u32 = count.parse().map_err(|_| bad_count())?;
        (n, n)
    };
    if max == 0 || min > max {
        return Err(bad_count());
    }
    Ok(Expected {
        point,
        fstype,
        source,
        mode,
        flags,
        min,
        max,
    })
}

/// The lines of `/proc/self/mountinfo` (proc(5)); a line that is not of the
/// documented shape is kept with what could be read, so that it still counts
/// as an entry and cannot hide.
#[must_use]
pub fn parse_mountinfo(text: &str) -> Vec<Actual> {
    let mut actual: Vec<Actual> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let fields: Vec<&str> = line.split(' ').collect();
            let after_dash = fields.iter().position(|field| *field == "-");
            let options = fields.get(5).copied().unwrap_or("");
            Actual {
                id: fields.first().and_then(|f| f.parse().ok()).unwrap_or(0),
                parent: fields.get(1).and_then(|f| f.parse().ok()).unwrap_or(0),
                point: unescape(fields.get(4).copied().unwrap_or("?")),
                fstype: after_dash
                    .and_then(|at| fields.get(at + 1))
                    .map_or_else(|| "?".to_owned(), |f| unescape(f)),
                read_only: options.split(',').any(|option| option == "ro"),
                flags: Flags::of_options(options),
                top: true,
            }
        })
        .collect();
    let stacked: Vec<bool> = actual
        .iter()
        .map(|mount| {
            actual.iter().any(|other| {
                other.point == mount.point && other.parent == mount.id && other.id != mount.id
            })
        })
        .collect();
    for (mount, covered) in actual.iter_mut().zip(stacked) {
        mount.top = !covered;
    }
    actual
}

/// Undoes the octal escapes of mountinfo (`\040` for a space and so on).
fn unescape(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\\'
            && index + 3 < bytes.len()
            && bytes[index + 1..=index + 3]
                .iter()
                .all(|b| (b'0'..=b'7').contains(b))
        {
            let value = (bytes[index + 1] - b'0') * 64
                + (bytes[index + 2] - b'0') * 8
                + (bytes[index + 3] - b'0');
            out.push(value);
            index += 4;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Compares the table as a multiset. The most specific expectations take
/// their mounts first (an inode before a type before "any"), each at most
/// `max` times; what stays below `min` is missing, every mount no
/// expectation took is extra. `stat` gives device and inode of a path
/// without following a link.
#[must_use]
pub fn compare(
    expected: &[Expected],
    actual: &[Actual],
    stat: &dyn Fn(&str) -> Option<(u64, u64)>,
) -> Mismatch {
    let rank = |entry: &Expected| match entry.source {
        Source::Inode(..) => 0,
        Source::Type => 1,
        Source::Any => 2,
    };
    let mut order: Vec<&Expected> = expected.iter().collect();
    order.sort_by_key(|entry| rank(entry));
    let mut taken = vec![false; actual.len()];
    let mut mismatch = Mismatch::default();
    for entry in order {
        let mut found = 0;
        for (index, mount) in actual.iter().enumerate() {
            if found >= entry.max {
                break;
            }
            if !taken[index] && fits(entry, mount, stat) {
                taken[index] = true;
                found += 1;
            }
        }
        if found < entry.min {
            mismatch.missing.push(format!("{entry}:found={found}"));
        }
    }
    for (mount, taken) in actual.iter().zip(taken) {
        if !taken {
            let inode = stat(&mount.point)
                .filter(|_| mount.top)
                .map_or_else(String::new, |(dev, ino)| format!(":inode={dev}:{ino}"));
            mismatch.extra.push(format!("{mount}{inode}"));
        }
    }
    mismatch
}

fn fits(entry: &Expected, mount: &Actual, stat: &dyn Fn(&str) -> Option<(u64, u64)>) -> bool {
    if entry.point != mount.point {
        return false;
    }
    if entry
        .fstype
        .as_ref()
        .is_some_and(|fstype| *fstype != mount.fstype)
    {
        return false;
    }
    let mode_fits = match entry.mode {
        Mode::ReadOnly => mount.read_only,
        Mode::ReadWrite => !mount.read_only,
        Mode::Any => true,
    };
    if !mode_fits || entry.flags.is_some_and(|flags| flags != mount.flags) {
        return false;
    }
    match entry.source {
        // Only the topmost mount on a point is what `lstat` sees; one below
        // another can never be the expected bind.
        Source::Inode(dev, ino) => mount.top && stat(&mount.point) == Some((dev, ino)),
        Source::Type | Source::Any => true,
    }
}

/// The targets whose device and inode differ from the expected pair, as
/// `path=<p>;expected=<dev>:<ino>;found=<dev>:<ino>|missing`.
#[must_use]
pub fn check_pins(pins: &[Pin], stat: &dyn Fn(&str) -> Option<(u64, u64)>) -> Vec<String> {
    pins.iter()
        .filter_map(|pin| {
            let found = stat(&pin.path);
            (found != Some((pin.dev, pin.ino))).then(|| {
                format!(
                    "path={};expected={}:{};found={}",
                    pin.path,
                    pin.dev,
                    pin.ino,
                    found.map_or_else(|| "missing".to_owned(), |(dev, ino)| format!("{dev}:{ino}"))
                )
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// A mount table as bwrap leaves it, shortened: the root, `/proc`,
    /// `/work` with a tmpfs over `.git/hooks` and a read-only file over
    /// `.git/config`.
    const MOUNTINFO: &str = "\
100 99 0:50 /newroot / rw,nosuid,nodev,relatime - tmpfs tmpfs rw\n\
101 100 0:51 / /proc rw,nosuid,nodev,noexec,relatime - proc proc rw\n\
102 100 8:2 /home/u/proj /work rw,nosuid,nodev,relatime - ext4 /dev/sda2 rw\n\
103 102 0:52 / /work/.git/hooks rw,nosuid,nodev,relatime - tmpfs tmpfs rw\n\
104 102 0:50 /bindfile1 /work/.git/config ro,nosuid,nodev,relatime - tmpfs tmpfs rw\n";

    fn table() -> Vec<Expected> {
        let typed = |point: &str, fstype: &str, mode, flags| Expected {
            point: point.to_owned(),
            fstype: Some(fstype.to_owned()),
            source: Source::Type,
            mode,
            flags: Some(flags),
            min: 1,
            max: 1,
        };
        let nosuid_nodev = Flags {
            nosuid: true,
            nodev: true,
            noexec: false,
        };
        vec![
            typed("/", "tmpfs", Mode::ReadWrite, nosuid_nodev),
            typed(
                "/proc",
                "proc",
                Mode::ReadWrite,
                Flags {
                    noexec: true,
                    ..nosuid_nodev
                },
            ),
            Expected {
                point: "/work".to_owned(),
                fstype: None,
                source: Source::Inode(2050, 131),
                mode: Mode::ReadWrite,
                flags: Some(nosuid_nodev),
                min: 1,
                max: 1,
            },
            typed("/work/.git/hooks", "tmpfs", Mode::ReadWrite, nosuid_nodev),
            typed("/work/.git/config", "tmpfs", Mode::ReadOnly, nosuid_nodev),
        ]
    }

    fn stat(path: &str) -> Option<(u64, u64)> {
        match path {
            "/work" => Some((2050, 131)),
            "/work/.git/hooks" => Some((52, 1)),
            "/work/.git/config" => Some((50, 7)),
            "/" => Some((50, 2)),
            "/proc" => Some((51, 1)),
            "/run/secrets" => Some((60, 1)),
            _ => None,
        }
    }

    fn with_line(line: &str) -> String {
        format!("{MOUNTINFO}{line}\n")
    }

    #[test]
    fn the_planned_table_matches() {
        let mismatch = compare(&table(), &parse_mountinfo(MOUNTINFO), &stat);
        assert!(mismatch.is_empty(), "{mismatch:?}");
    }

    /// An engine's configuration source adds a directory of secrets: one
    /// entry too many.
    #[test]
    fn an_extra_mount_point_fails() {
        let text = with_line("105 100 0:60 / /run/secrets ro,nosuid,nodev - tmpfs tmpfs rw");
        let mismatch = compare(&table(), &parse_mountinfo(&text), &stat);
        assert!(mismatch.missing.is_empty(), "{mismatch:?}");
        assert_eq!(mismatch.extra.len(), 1, "{mismatch:?}");
        assert!(
            mismatch.extra[0].starts_with("/run/secrets:tmpfs:ro"),
            "{mismatch:?}"
        );
    }

    /// A mask that did not take: `.git/hooks` is missing.
    #[test]
    fn a_missing_mount_point_fails() {
        let text: String = MOUNTINFO
            .lines()
            .filter(|line| !line.contains("/work/.git/hooks"))
            .flat_map(|line| [line, "\n"])
            .collect();
        let mismatch = compare(&table(), &parse_mountinfo(&text), &stat);
        assert_eq!(mismatch.missing.len(), 1, "{mismatch:?}");
        assert!(
            mismatch.missing[0].starts_with("/work/.git/hooks:"),
            "{mismatch:?}"
        );
        assert!(mismatch.extra.is_empty(), "{mismatch:?}");
    }

    #[test]
    fn rw_instead_of_ro_fails() {
        let text = MOUNTINFO.replace(
            "/bindfile1 /work/.git/config ro,",
            "/bindfile1 /work/.git/config rw,",
        );
        let mismatch = compare(&table(), &parse_mountinfo(&text), &stat);
        assert!(!mismatch.is_empty(), "{mismatch:?}");
        assert!(
            mismatch.extra[0].contains("/work/.git/config:tmpfs:rw"),
            "{mismatch:?}"
        );
    }

    #[test]
    fn a_wrong_type_fails() {
        let text = MOUNTINFO.replace(
            "/work/.git/hooks rw,nosuid,nodev,relatime - tmpfs tmpfs",
            "/work/.git/hooks rw,nosuid,nodev,relatime - ext4 /dev/sda2",
        );
        let mismatch = compare(&table(), &parse_mountinfo(&text), &stat);
        assert!(!mismatch.is_empty(), "{mismatch:?}");
    }

    #[test]
    fn a_missing_flag_fails() {
        let text = MOUNTINFO.replace("/proc rw,nosuid,nodev,noexec", "/proc rw,nosuid,nodev");
        let mismatch = compare(&table(), &parse_mountinfo(&text), &stat);
        assert!(!mismatch.is_empty(), "{mismatch:?}");
    }

    /// The same bind a second time on `/work`, stacked: the topmost one is
    /// the expected entry, the one below is one too many. Compared as a set
    /// of mount points, both would vanish into one.
    #[test]
    fn a_second_mount_stacked_on_an_expected_point_fails() {
        let text = with_line(
            "106 102 8:2 /home/u/proj /work rw,nosuid,nodev,relatime - ext4 /dev/sda2 rw",
        );
        let actual = parse_mountinfo(&text);
        assert!(!actual.iter().find(|m| m.id == 102).unwrap().top);
        assert!(actual.iter().find(|m| m.id == 106).unwrap().top);
        let mismatch = compare(&table(), &actual, &stat);
        assert!(mismatch.missing.is_empty(), "{mismatch:?}");
        assert_eq!(mismatch.extra.len(), 1, "{mismatch:?}");
        assert!(mismatch.extra[0].ends_with(":covered"), "{mismatch:?}");
    }

    /// A tmpfs over `/work`: the bind below no longer is what `lstat` sees,
    /// and the tmpfs is not the bind.
    #[test]
    fn a_tmpfs_over_work_fails() {
        let text = with_line("107 102 0:61 / /work rw,nosuid,nodev - tmpfs tmpfs rw");
        let stat_over = |path: &str| {
            if path == "/work" {
                Some((61, 1))
            } else {
                stat(path)
            }
        };
        let mismatch = compare(&table(), &parse_mountinfo(&text), &stat_over);
        assert_eq!(mismatch.missing.len(), 1, "{mismatch:?}");
        assert_eq!(mismatch.extra.len(), 2, "{mismatch:?}");
    }

    #[test]
    fn counts_and_optional_entries_are_respected() {
        let mut table = table();
        table.push(Expected {
            point: "/proc/sys".to_owned(),
            fstype: Some("proc".to_owned()),
            source: Source::Type,
            mode: Mode::ReadOnly,
            flags: None,
            min: 0,
            max: 1,
        });
        let actual = parse_mountinfo(MOUNTINFO);
        assert!(compare(&table, &actual, &stat).is_empty());
        let text = with_line("108 101 0:51 /sys /proc/sys ro,nosuid,nodev,noexec - proc proc rw");
        assert!(compare(&table, &parse_mountinfo(&text), &stat).is_empty());
        let twice =
            format!("{text}109 101 0:51 /sys /proc/sys ro,nosuid,nodev,noexec - proc proc rw\n");
        let mismatch = compare(&table, &parse_mountinfo(&twice), &stat);
        assert_eq!(mismatch.extra.len(), 1, "{mismatch:?}");
    }

    /// A mask the engine owes is pinned to `/dev/null`, read-only: a
    /// writable bind of a host directory onto `/proc/kcore` does not satisfy
    /// it and is one entry too many.
    #[test]
    fn a_writable_bind_onto_a_masked_proc_path_fails() {
        let mut table = table();
        table.push(Expected {
            point: "/proc/kcore".to_owned(),
            fstype: None,
            source: Source::Inode(9, 4),
            mode: Mode::ReadOnly,
            flags: None,
            min: 1,
            max: 1,
        });
        let masked = with_line("110 101 0:9 /null /proc/kcore ro,nosuid - devtmpfs udev rw");
        let stat_null = |path: &str| {
            if path == "/proc/kcore" {
                Some((9, 4))
            } else {
                stat(path)
            }
        };
        assert!(compare(&table, &parse_mountinfo(&masked), &stat_null).is_empty());
        let hostile =
            with_line("110 101 8:2 /home/u/.ssh /proc/kcore rw,nosuid,nodev - ext4 /dev/sda2 rw");
        let stat_dir = |path: &str| {
            if path == "/proc/kcore" {
                Some((2050, 777))
            } else {
                stat(path)
            }
        };
        let mismatch = compare(&table, &parse_mountinfo(&hostile), &stat_dir);
        assert_eq!(mismatch.missing.len(), 1, "{mismatch:?}");
        assert_eq!(mismatch.extra.len(), 1, "{mismatch:?}");
        assert!(
            mismatch.extra[0].starts_with("/proc/kcore:ext4:rw"),
            "{mismatch:?}"
        );
    }

    /// A point whose type depends on the host (a container engine's `/run/.containerenv`
    /// from a `runroot` on ext4, the console of a text terminal) takes any
    /// type, but still exactly once, read-only and with its flags: a second
    /// mount stacked on it is one too many.
    #[test]
    fn a_point_of_any_type_still_counts_once() {
        let mut table = table();
        table.push(Expected {
            point: "/run/.containerenv".to_owned(),
            fstype: None,
            source: Source::Any,
            mode: Mode::ReadOnly,
            flags: Some(Flags {
                nosuid: true,
                nodev: true,
                noexec: true,
            }),
            min: 1,
            max: 1,
        });
        let on_ext4 = with_line(
            "110 101 8:2 /c/userdata/.containerenv /run/.containerenv ro,nosuid,nodev,noexec - ext4 /dev/sda2 rw",
        );
        assert!(compare(&table, &parse_mountinfo(&on_ext4), &stat).is_empty());
        let stacked = format!(
            "{on_ext4}111 110 0:50 / /run/.containerenv ro,nosuid,nodev,noexec - tmpfs tmpfs rw\n"
        );
        let mismatch = compare(&table, &parse_mountinfo(&stacked), &stat);
        assert_eq!(mismatch.extra.len(), 1, "{mismatch:?}");
    }

    #[test]
    fn escaped_mount_points_are_unescaped() {
        let actual = parse_mountinfo("5 1 0:1 / /work/a\\040b rw - tmpfs tmpfs rw\n");
        assert_eq!(actual[0].point, "/work/a b");
        assert_eq!(unescape("a\\134b"), "a\\b");
        assert_eq!(unescape("tail\\04"), "tail\\04");
    }

    /// Only the inode, only the device, or both differ: each is a failure
    /// with path, expected and found pair.
    #[test]
    fn a_pin_differing_in_inode_device_or_both_fails() {
        let pin = |dev, ino| Pin {
            path: "/work".to_owned(),
            dev,
            ino,
        };
        assert!(check_pins(&[pin(2050, 131)], &stat).is_empty());
        for (dev, ino) in [(2050, 132), (2051, 131), (2051, 132)] {
            let wrong = check_pins(&[pin(dev, ino)], &stat);
            assert_eq!(
                wrong,
                [format!("path=/work;expected={dev}:{ino};found=2050:131")]
            );
        }
        let gone = check_pins(
            &[Pin {
                path: "/nothing".to_owned(),
                dev: 1,
                ino: 2,
            }],
            &stat,
        );
        assert_eq!(gone, ["path=/nothing;expected=1:2;found=missing"]);
    }

    #[test]
    fn the_descriptor_json_reads_in_the_documented_shape() {
        let pins = parse_pins(
            r#"{"pins":[{"path":"/work","dev":"2050","ino":"131"}],
                "mounts":[{"point":"/work","type":"*","source":"inode:2050:131","mode":"rw","flags":"nosuid,nodev","count":"1"},
                          {"point":"/proc/sys","type":"proc","source":"type","mode":"ro","flags":"*","count":"0-1"},
                          {"point":"/dev/null","type":"*","source":"any","mode":"*","flags":"","count":"1"}]}"#,
        )
        .unwrap();
        assert_eq!(
            pins.pins,
            [Pin {
                path: "/work".to_owned(),
                dev: 2050,
                ino: 131
            }]
        );
        assert_eq!(pins.mounts.len(), 3);
        assert_eq!(pins.mounts[0].source, Source::Inode(2050, 131));
        assert_eq!((pins.mounts[1].min, pins.mounts[1].max), (0, 1));
        assert_eq!(pins.mounts[2].flags, Some(Flags::default()));
        assert_eq!(
            parse_pins(r#"{"mounts":[],"pins":[]}"#).unwrap(),
            Pins::default()
        );
    }

    #[test]
    fn anything_else_behind_the_descriptor_is_refused() {
        for text in [
            "",
            "{}",
            r#"{"pins":[]}"#,
            r#"{"pins":[],"mounts":[],"pins":[]}"#,
            r#"{"pins":[],"mounts":[],"x":[]}"#,
            r#"{"pins":[{"path":"work","dev":"1","ino":"2"}],"mounts":[]}"#,
            r#"{"pins":[{"path":"/w","dev":"x","ino":"2"}],"mounts":[]}"#,
            r#"{"pins":[],"mounts":[{"point":"/","type":"*","source":"type","mode":"rw","flags":"*","count":"1"}]}"#,
            r#"{"pins":[],"mounts":[{"point":"/","type":"tmpfs","source":"type","mode":"rx","flags":"*","count":"1"}]}"#,
            r#"{"pins":[],"mounts":[{"point":"/","type":"tmpfs","source":"type","mode":"rw","flags":"nosetuid","count":"1"}]}"#,
            r#"{"pins":[],"mounts":[{"point":"/","type":"tmpfs","source":"type","mode":"rw","flags":"*","count":"0"}]}"#,
            r#"{"pins":[],"mounts":[{"point":"/","type":"tmpfs","source":"type","mode":"rw","flags":"*","count":"2-1"}]}"#,
            r#"{"pins":[],"mounts":[{"point":"/","type":"tmpfs","source":"inode:1","mode":"rw","flags":"*","count":"1"}]}"#,
            r#"{"pins":[],"mounts":[{"point":"/","type":"tmpfs","source":"type","mode":"rw","flags":"*","count":"1","extra":"x"}]}"#,
        ] {
            assert!(parse_pins(text).is_err(), "{text:?} parsed");
        }
    }
}
