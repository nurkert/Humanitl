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
//! 6. The profile's masks over the overlay on `/work`, as bwrap lays them
//!    over its overlay (`guest_masks.rs`, HUM-481); their empty file comes
//!    from the staging tmpfs and is taken before step 5 detaches it.
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
use crate::linux::guest_masks::{self, Mask};
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
    /// The size of each tmpfs mask in MiB (HUM-481), as on the host.
    pub mask_mib: u64,
    /// The profile's masks over the overlay, in their order (the plan's
    /// `work_masks`, `guest_masks.rs`).
    pub masks: Vec<Mask>,
}

/// Reads
/// `"disposable":{"layer_tag":"layer","lower_ino":"131","mask_mib":"64"}`
/// and the plan's `work_masks` (`masks`); absent means a direct session,
/// which takes no masks (the host lays them under its file daemon).
/// `modules` are the plan's extra modules; a disposable plan must name
/// [`OVERLAY_MODULE`].
///
/// # Errors
///
/// A reason without spaces for anything but that shape.
pub fn parse(
    raw: Option<&str>,
    masks: Option<&str>,
    modules: &[String],
) -> Result<Option<Disposable>, String> {
    let Some(text) = raw else {
        if masks.is_some() {
            return Err("work_masks:without-disposable".to_owned());
        }
        return Ok(None);
    };
    let mut parser = Parser::new(text);
    let fields = parser
        .string_object()
        .and_then(|fields| parser.end().map(|()| fields))
        .map_err(|err| format!("disposable:{err}").replace(' ', "_"))?;
    let mut layer_tag = None;
    let mut lower_ino = None;
    let mut mask_mib = None;
    for (key, value) in fields {
        let slot = match key.as_str() {
            "layer_tag" => &mut layer_tag,
            "lower_ino" => &mut lower_ino,
            "mask_mib" => &mut mask_mib,
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
    let mask_mib = mask_mib
        .and_then(|text| text.parse::<u64>().ok())
        .filter(|mib| (1..=1 << 20).contains(mib))
        .ok_or_else(|| "disposable:mask_mib:bad".to_owned())?;
    let masks = guest_masks::parse(masks)?;
    if !modules
        .iter()
        .any(|name| name.replace('-', "_") == OVERLAY_MODULE)
    {
        return Err("disposable:modules-without-overlay".to_owned());
    }
    Ok(Some(Disposable {
        layer_tag,
        lower_ino,
        mask_mib,
        masks,
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

/// Steps 1 to 6 of the module documentation, on `work`; the masks' tmpfs
/// and files belong to `uid` and `gid`, the agent's.
///
/// # Errors
///
/// The step that failed, as evidence without spaces.
pub fn mount(
    work: &str,
    work_tag: &str,
    plan: &Disposable,
    uid: u32,
    gid: u32,
) -> Result<(), Failure> {
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
    let held =
        guest_masks::hold(staging, &plan.masks, uid, gid).map_err(|err| step("hold", &err))?;
    detach(work).map_err(|err| step("detach", &err))?;
    guest_tmpfs::move_tree(&tree, staging).map_err(|err| step("move", &err))?;
    guest_masks::lay(staging, &plan.masks, held, plan.mask_mib, uid, gid)
        .map_err(|why| Failure::Mount(format!("disposable:{why}")))
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

    use std::path::PathBuf;

    use super::*;

    fn overlay() -> Vec<String> {
        vec!["overlay".to_owned()]
    }

    #[test]
    fn absent_means_a_direct_session() {
        assert_eq!(parse(None, None, &[]), Ok(None));
    }

    /// HUM-481: masks belong to a disposable plan; a direct one gets its
    /// masks from the host.
    #[test]
    fn masks_without_a_disposable_plan_are_refused() {
        assert_eq!(
            parse(None, Some(r#"["empty:.env"]"#), &overlay()),
            Err("work_masks:without-disposable".to_owned())
        );
    }

    #[test]
    fn the_documented_shape_reads() {
        assert_eq!(
            parse(
                Some(r#"{"layer_tag":"layer","lower_ino":"131","mask_mib":"64"}"#),
                Some(r#"["tmpfs:.git/hooks","empty:.env"]"#),
                &overlay()
            ),
            Ok(Some(Disposable {
                layer_tag: "layer".to_owned(),
                lower_ino: 131,
                mask_mib: 64,
                masks: vec![
                    Mask::Tmpfs(PathBuf::from(".git/hooks")),
                    Mask::Empty(PathBuf::from(".env")),
                ],
            }))
        );
        assert_eq!(
            parse(
                Some(r#"{"layer_tag":"layer","lower_ino":"131","mask_mib":"64"}"#),
                None,
                &overlay()
            )
            .unwrap()
            .unwrap()
            .masks,
            Vec::new()
        );
    }

    /// Every deviation is refused with its key, and so is a disposable plan
    /// whose modules do not load the overlay before the lock.
    #[test]
    fn every_deviation_is_refused() {
        for (text, why) in [
            (
                r#"{"layer_tag":"layer","mask_mib":"64"}"#,
                "disposable:lower_ino:bad",
            ),
            (
                r#"{"lower_ino":"5","mask_mib":"64"}"#,
                "disposable:layer_tag:bad",
            ),
            (
                r#"{"layer_tag":"","lower_ino":"5","mask_mib":"64"}"#,
                "disposable:layer_tag:bad",
            ),
            (
                r#"{"layer_tag":"a b","lower_ino":"5","mask_mib":"64"}"#,
                "disposable:layer_tag:bad",
            ),
            (
                r#"{"layer_tag":"layer","lower_ino":"0","mask_mib":"64"}"#,
                "disposable:lower_ino:bad",
            ),
            (
                r#"{"layer_tag":"layer","lower_ino":"x","mask_mib":"64"}"#,
                "disposable:lower_ino:bad",
            ),
            (
                r#"{"layer_tag":"layer","lower_ino":"5"}"#,
                "disposable:mask_mib:bad",
            ),
            (
                r#"{"layer_tag":"layer","lower_ino":"5","mask_mib":"0"}"#,
                "disposable:mask_mib:bad",
            ),
            (
                r#"{"layer_tag":"layer","lower_ino":"5","mask_mib":"x"}"#,
                "disposable:mask_mib:bad",
            ),
            (
                r#"{"layer_tag":"layer","lower_ino":"5","mask_mib":"64","extra":"1"}"#,
                "disposable:unknown-key:extra",
            ),
            (
                r#"{"layer_tag":"layer","layer_tag":"x","lower_ino":"5","mask_mib":"64"}"#,
                "disposable:given-twice:layer_tag",
            ),
        ] {
            assert_eq!(
                parse(Some(text), None, &overlay()),
                Err(why.to_owned()),
                "{text}"
            );
        }
        assert!(parse(Some("[]"), None, &overlay()).is_err());
        assert!(
            parse(
                Some(r#"{"layer_tag":"layer","lower_ino":"5","mask_mib":"64"} x"#),
                None,
                &overlay()
            )
            .is_err()
        );
        let good = r#"{"layer_tag":"layer","lower_ino":"5","mask_mib":"64"}"#;
        assert_eq!(
            parse(Some(good), None, &[]),
            Err("disposable:modules-without-overlay".to_owned())
        );
        assert_eq!(
            parse(Some(good), Some(r#"["empty:../x"]"#), &overlay()),
            Err("work_masks:not-a-relative-path".to_owned())
        );
    }
}
