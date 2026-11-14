//! The disposable `/work` of the kernel level (HUM-446).
//!
//! In a disposable session the daemon shares two directories with the
//! guest: the checked project folder, read-only (tag `work`), and the
//! session's layer directory with `upper` and `work` inside (the plan's
//! `layer_tag`). PID 1 builds the overlay that bwrap's `--overlay` builds on
//! the shared kernel, and mounts it on `/work`:
//!
//! 1. A small tmpfs on `/work` as a staging place, with three empty
//!    directories.
//! 2. The project read-only on the first; its root must carry the inode of
//!    the checked folder (`lower_ino`, HUM-442 Q5: a shared file system keeps
//!    the host's inode on the guest's own device). Another folder refuses
//!    the start before anything else is mounted.
//! 3. The layer on the second; `upper` and `work` must be directories in it.
//! 4. The overlay on the third, with the options of an unprivileged overlay
//!    as bwrap mounts it (`userxattr`, `redirect_dir=nofollow`, no
//!    `metacopy`, no index), so that the upper layer has the shape the
//!    daemon's scan reads (`user.overlay.opaque`, whiteouts `0:0`).
//! 5. A detached copy of the overlay's mount (`open_tree(2)`), the staging
//!    tmpfs detached with everything on it, and the copy moved onto `/work`
//!    (`move_mount(2)`). The overlay keeps its own private copies of its
//!    layers, so the table afterwards holds exactly one mount on `/work`,
//!    the overlay, and neither the project nor the layer is reachable by a
//!    path.
//!
//! The overlay module itself is loaded with the door's modules, before the
//! lock (the plan's `modules`); the shim refuses a disposable plan that does
//! not name it.
//!
//! Root in the guest can mount the layer again and write anything into it;
//! the daemon reads it only after the VM has ended and treats every byte as
//! written by the attacker. The steps here keep the agent's view the same as
//! under bwrap; they are not what holds the guarantee.

use std::ffi::CString;
use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt as _;
use std::path::Path;

use crate::json::Parser;
use crate::linux::guest_tmpfs;

/// The module the overlay needs, loaded before the lock.
pub const OVERLAY_MODULE: &str = "overlay";

/// The names of the staging directories on the tmpfs.
const LOWER: &str = "lower";
const LAYER: &str = "layer";
const MERGED: &str = "merged";

/// The overlay's options, as an unprivileged overlay of bwrap has them.
const OPTIONS: &str = "userxattr,redirect_dir=nofollow,metacopy=off,index=off";

/// What a disposable plan adds (`"disposable"` of the guest plan).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disposable {
    /// The tag of the session's layer directory, with `upper` and `work`.
    pub layer_tag: String,
    /// The inode of the checked project folder, the overlay's lower layer.
    pub lower_ino: u64,
}

/// Reads `"disposable":{"layer_tag":"layer","lower_ino":"131"}`; absent
/// means a direct session. `modules` are the plan's extra modules; a
/// disposable plan must name [`OVERLAY_MODULE`].
///
/// # Errors
///
/// A reason without spaces for anything but that shape.
pub fn parse(raw: Option<&str>, modules: &[String]) -> Result<Option<Disposable>, String> {
    let Some(text) = raw else {
        return Ok(None);
    };
    let fields = Parser::new(text)
        .string_object()
        .map_err(|err| format!("disposable:{err}").replace(' ', "_"))?;
    let mut layer_tag = None;
    let mut lower_ino = None;
    for (key, value) in fields {
        let slot = match key.as_str() {
            "layer_tag" => &mut layer_tag,
            "lower_ino" => &mut lower_ino,
            other => return Err(format!("disposable:unknown-key:{other}")),
        };
        if slot.replace(value).is_some() {
            return Err(format!("disposable:given-twice:{key}"));
        }
    }
    let layer_tag = layer_tag
        .filter(|tag| !tag.is_empty() && !tag.contains(|c: char| c.is_whitespace() || c == '\0'))
        .ok_or_else(|| "disposable:layer_tag:bad".to_owned())?;
    let lower_ino = lower_ino
        .and_then(|text| text.parse::<u64>().ok())
        .filter(|ino| *ino != 0)
        .ok_or_else(|| "disposable:lower_ino:bad".to_owned())?;
    if !modules
        .iter()
        .any(|name| name.replace('-', "_") == OVERLAY_MODULE)
    {
        return Err("disposable:modules-without-overlay".to_owned());
    }
    Ok(Some(Disposable {
        layer_tag,
        lower_ino,
    }))
}

/// Why the mount failed, and which check it fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// The lower layer is not the checked folder (`mounts_pinned`).
    Pin(String),
    /// Any other step (`mounts_match`).
    Mount(String),
}

/// Steps 1 to 5 of the module documentation, on `work`.
///
/// # Errors
///
/// The step that failed, as evidence without spaces.
pub fn mount(work: &str, work_tag: &str, plan: &Disposable) -> Result<(), Failure> {
    let step = |what: &str, err: &dyn std::fmt::Display| {
        Failure::Mount(format!("disposable:{what}:{err}").replace(' ', "_"))
    };
    let staging = Path::new(work);
    super::guest::mount_with(
        "tmpfs",
        work,
        "tmpfs",
        libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC,
        "mode=0700,size=16k",
    )
    .map_err(|err| step("staging", &err))?;
    for name in [LOWER, LAYER, MERGED] {
        fs::create_dir(staging.join(name)).map_err(|err| step(name, &err))?;
    }
    let lower = staging.join(LOWER);
    let layer = staging.join(LAYER);
    let merged = staging.join(MERGED);
    let text = |path: &Path| path.to_string_lossy().into_owned();
    super::guest::mount_with(
        work_tag,
        &text(&lower),
        "virtiofs",
        libc::MS_RDONLY | libc::MS_NOSUID | libc::MS_NODEV,
        "",
    )
    .map_err(|err| step("lower", &err))?;
    let found = fs::symlink_metadata(&lower).map_err(|err| step("lower-stat", &err))?;
    if found.ino() != plan.lower_ino {
        return Err(Failure::Pin(format!(
            "disposable:lower-pin:expected={};found={}",
            plan.lower_ino,
            found.ino()
        )));
    }
    super::guest::mount_with(
        &plan.layer_tag,
        &text(&layer),
        "virtiofs",
        libc::MS_NOSUID | libc::MS_NODEV,
        "",
    )
    .map_err(|err| step("layer", &err))?;
    for name in ["upper", "work"] {
        let meta = fs::symlink_metadata(layer.join(name)).map_err(|err| step(name, &err))?;
        if !meta.is_dir() {
            return Err(step(name, &"not-a-directory"));
        }
    }
    let options = format!(
        "lowerdir={},upperdir={},workdir={},{OPTIONS}",
        text(&lower),
        text(&layer.join("upper")),
        text(&layer.join("work"))
    );
    super::guest::mount_with(
        "overlay",
        &text(&merged),
        "overlay",
        libc::MS_NOSUID | libc::MS_NODEV,
        &options,
    )
    .map_err(|err| step("overlay", &err))?;
    let tree = guest_tmpfs::clone_tree(&merged).map_err(|err| step("clone", &err))?;
    detach(work).map_err(|err| step("detach", &err))?;
    guest_tmpfs::move_tree(&tree, staging).map_err(|err| step("move", &err))
}

/// `umount2(2)` with `MNT_DETACH`.
fn detach(path: &str) -> io::Result<()> {
    let path =
        CString::new(path).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL byte"))?;
    // SAFETY: a NUL-terminated path that outlives the call.
    if unsafe { libc::umount2(path.as_ptr(), libc::MNT_DETACH) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn overlay() -> Vec<String> {
        vec!["overlay".to_owned()]
    }

    #[test]
    fn absent_means_a_direct_session() {
        assert_eq!(parse(None, &[]), Ok(None));
    }

    #[test]
    fn the_documented_shape_reads() {
        assert_eq!(
            parse(
                Some(r#"{"layer_tag":"layer","lower_ino":"131"}"#),
                &overlay()
            ),
            Ok(Some(Disposable {
                layer_tag: "layer".to_owned(),
                lower_ino: 131,
            }))
        );
    }

    /// Every deviation is refused with its key, and so is a disposable plan
    /// whose modules do not load the overlay before the lock.
    #[test]
    fn every_deviation_is_refused() {
        for (text, why) in [
            (r#"{"layer_tag":"layer"}"#, "disposable:lower_ino:bad"),
            (r#"{"lower_ino":"5"}"#, "disposable:layer_tag:bad"),
            (
                r#"{"layer_tag":"","lower_ino":"5"}"#,
                "disposable:layer_tag:bad",
            ),
            (
                r#"{"layer_tag":"a b","lower_ino":"5"}"#,
                "disposable:layer_tag:bad",
            ),
            (
                r#"{"layer_tag":"layer","lower_ino":"0"}"#,
                "disposable:lower_ino:bad",
            ),
            (
                r#"{"layer_tag":"layer","lower_ino":"x"}"#,
                "disposable:lower_ino:bad",
            ),
            (
                r#"{"layer_tag":"layer","lower_ino":"5","extra":"1"}"#,
                "disposable:unknown-key:extra",
            ),
            (
                r#"{"layer_tag":"layer","layer_tag":"x","lower_ino":"5"}"#,
                "disposable:given-twice:layer_tag",
            ),
        ] {
            assert_eq!(parse(Some(text), &overlay()), Err(why.to_owned()), "{text}");
        }
        assert!(parse(Some("[]"), &overlay()).is_err());
        assert_eq!(
            parse(Some(r#"{"layer_tag":"layer","lower_ino":"5"}"#), &[]),
            Err("disposable:modules-without-overlay".to_owned())
        );
    }
}
