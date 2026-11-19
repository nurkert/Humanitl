//! Die Masken und erweiterten Attribute einer Wegwerf-Sitzung im Gast
//! (HUM-481).
//!
//! Unter bwrap liegen die Masken über dem Overlay; im Gast legt der Shim sie
//! ebenso darüber (`guest_masks.rs` des Shims), und der Dienst der Schicht
//! nimmt nur `user.overlay.*` an (`--xattrmap`,
//! [`super::super::lines::LAYER_XATTRMAP`]). Wie die anderen KVM-Tests
//! überspringt der Test ohne KVM und scheitert unter `HUMANITL_REQUIRE_KVM=1`;
//! er braucht keinen Eintritt in fremde Namensräume, nur die Ausgabe des
//! Gasts und die Schicht auf dem Host.

use std::os::fd::AsFd as _;
use std::path::Path;
use std::time::{Duration, Instant};

use super::{Harness, assert_admitted, masked_layer, wait_output};

/// Der Agent: macht die maskierte Datei `.env` beschreibbar (ihm gehört die
/// leere Datei, die der Host in die untere Schicht legt, nur lesbar) und
/// schreibt darauf, schreibt in den maskierten Ordner `.git/hooks`, setzt ein fremdes und ein eigenes Attribut des
/// Overlays auf einer eigenen Datei und ersetzt einen Ordner (ein opakes
/// Verzeichnis, das `user.overlay.opaque` braucht).
const MASK_AGENT: &str = r#"echo "env-mode=$(stat -c %a /work/.env)"
chmod u+w /work/.env 2>/dev/null; echo "chmod-rc=$?"
echo leaked > /work/.env 2>/dev/null; echo "env-rc=$?"
echo "env-bytes=$(wc -c < /work/.env)"
echo hook > /work/.git/hooks/pre-commit; echo "hook-rc=$?"
echo "nested-bytes=$(wc -c < /work/custom/secret)"
echo leaked > /work/custom/secret 2>/dev/null; echo "nested-rc=$?"
echo y > /work/custom/inner/new; echo "nested-inner-rc=$?"
echo new > /work/new.txt
python3 -c '
import os
try:
    os.setxattr("/work/new.txt", "user.foreign", b"x")
    print("xattr-foreign=ok")
except OSError as err:
    print("xattr-foreign=errno%d" % err.errno)
' 2>&1 || echo "xattr-foreign=no-python"
rm -r /work/dir && mkdir /work/dir && echo again > /work/dir/x
grep ' /work/' /proc/self/mountinfo | awk '{for (i = 7; i <= NF; i++) if ($i == "-") print "mask-mount:" $5 ":" $(i + 1) ":" $6}'
echo masks-done-$((6*7))
"#;

/// Die Namen aller erweiterten Attribute unter `dir`, ohne einem Verweis zu
/// folgen, als `<relativ>:<name>`. Jeder Fehler beim Lesen lässt den Test
/// scheitern, damit „nur `user.overlay.*`" nie leer gilt; ist die Liste
/// gewachsen (`ERANGE`), wird mit ihrer neuen Länge erneut gelesen.
fn xattr_names(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let meta = std::fs::symlink_metadata(&current).unwrap();
        if !meta.file_type().is_symlink() {
            let buf = list_xattrs(&current);
            let len = buf.len();
            let rel = current.strip_prefix(dir).unwrap().display().to_string();
            for name in buf[..len]
                .split(|byte| *byte == 0)
                .filter(|n| !n.is_empty())
            {
                found.push(format!("{rel}:{}", String::from_utf8_lossy(name)));
            }
        }
        if meta.is_dir() {
            for entry in std::fs::read_dir(&current).unwrap() {
                stack.push(entry.unwrap().path());
            }
        }
    }
    found.sort();
    found
}

/// Die Namensliste von `llistxattr(2)` für `path`, ganz: ist der Puffer
/// zu klein (`ERANGE`), wird er verdoppelt.
fn list_xattrs(path: &Path) -> Vec<u8> {
    let mut size = 4096;
    loop {
        let mut buf = vec![0_u8; size];
        match rustix::fs::llistxattr(path, &mut buf) {
            Ok(len) => {
                buf.truncate(len);
                return buf;
            }
            Err(rustix::io::Errno::RANGE) if size < 1 << 24 => size *= 2,
            Err(err) => panic!("llistxattr {}: {err}", path.display()),
        }
    }
}

/// Das Profil des Tests: die Vorgabe und dazu verschachtelte Masken, ein
/// tmpfs über `custom`, darin ein tmpfs über `custom/inner` und eine Maske
/// über die Datei `custom/secret`.
fn nested_profile() -> crate::SandboxProfile {
    let mut profile = crate::SandboxProfile::parse(
        include_str!("../../../../../../profiles/sandbox/default.toml"),
        Path::new("default.toml"),
    )
    .unwrap();
    profile.mounts.tmpfs.push("/work/custom".into());
    profile.mounts.tmpfs.push("/work/custom/inner".into());
    profile
        .mounts
        .masked_files
        .push("/work/custom/secret".into());
    profile
}

/// Wie `Harness::plan_options` für eine Wegwerf-Sitzung, mit
/// [`nested_profile`].
fn nested_plan(
    harness: &Harness,
    layer: &crate::Layer,
    meta: &std::fs::Metadata,
) -> (
    crate::LaunchPlan,
    std::sync::mpsc::Receiver<crate::OutputChunk>,
) {
    use crate::SandboxBackend as _;
    let session = harness.session(MASK_AGENT, Vec::new());
    let (sink, output) = std::sync::mpsc::channel();
    let options = crate::LaunchOptions {
        stdio: crate::StdioMode::Pty { cols: 80, rows: 24 },
        terminal: crate::TerminalTransport::Stream,
        output_sink: Some(sink),
        report_timeout: Some(Duration::from_secs(60)),
        disposable: Some(crate::launcher::DisposableLayer::Overlay(layer.dirs())),
        expected_work_inode: Some((
            std::os::unix::fs::MetadataExt::dev(meta),
            std::os::unix::fs::MetadataExt::ino(meta),
        )),
        ..crate::LaunchOptions::default()
    };
    let plan = harness
        .backend
        .plan(&crate::PlanRequest::new(
            &nested_profile(),
            &session,
            &options,
        ))
        .unwrap_or_else(|d| panic!("plan: {}: {}", d.code, d.why));
    (plan, output)
}

/// HUM-481: Im Gast einer Wegwerf-Sitzung liegen die Masken des Profils
/// über dem Overlay wie unter bwrap. Ein Schreiben auf die maskierte Datei
/// `.env` scheitert und sie bleibt leer; was der Agent in den maskierten
/// Ordner `.git/hooks` schreibt, landet im tmpfs der Maske, nie in der
/// Schicht. Ein fremdes Attribut (`user.foreign`) nimmt der Dienst der
/// Schicht nicht an (`ENOTSUP`, 95), und in der oberen Schicht (`upper`;
/// das Arbeitsverzeichnis des Overlays daneben liest der Test nicht, es
/// enthält Ordner mit den Rechten 0000) steht danach kein Attribut außer
/// `user.overlay.*`; das opake Verzeichnis des Overlays
/// kommt durch, und der Scan sieht die Änderungen des Agenten.
///
/// Mutationsbeweise: der Gastplan ohne `work_masks` (und ohne ihre Zeilen
/// der Mounttabelle) lässt das Schreiben auf `.env` gelingen, und `.env`
/// und `.git/hooks/pre-commit` stehen in der Schicht, rot; der Dienst der
/// Schicht ohne `--xattrmap` nimmt `user.foreign` an, rot.
#[test]
fn a_masked_path_and_a_foreign_xattr_are_refused_in_the_disposable_guest() {
    use crate::SandboxBackend as _;
    let test = "a_masked_path_and_a_foreign_xattr_are_refused_in_the_disposable_guest";
    let Some(harness) = Harness::new(test) else {
        return;
    };
    let project = &harness.project;
    std::fs::create_dir_all(project.join(".git/hooks")).unwrap();
    std::fs::create_dir_all(project.join("custom/inner")).unwrap();
    std::fs::write(project.join("custom/secret"), "SECRET\n").unwrap();
    std::fs::create_dir_all(project.join("dir/sub")).unwrap();
    std::fs::write(project.join("dir/sub/f"), "f\n").unwrap();
    let before = crate::disposable::baseline(project).unwrap();
    let layer = masked_layer(&harness);
    let meta = std::fs::metadata(project).unwrap();
    let (plan, output) = nested_plan(&harness, &layer, &meta);
    let handle = harness
        .backend
        .launch(&plan)
        .unwrap_or_else(|d| panic!("launch: {}: {}", d.code, d.why));
    assert_admitted(&harness, &handle);
    let mut seen = Vec::new();
    let done = wait_output(&output, &mut seen, "masks-done-42", Duration::from_secs(60));
    let shown = String::from_utf8_lossy(&seen).into_owned();
    assert!(done, "{shown}\n{}", harness.consoles());
    let status = handle
        .wait_timeout(Duration::from_secs(30))
        .expect("the session ends")
        .expect("a status");
    assert_eq!(status.code(), Some(0), "{shown}");
    eprintln!("{test}: guest view:\n{shown}");

    the_guest_saw_its_masks(&shown);
    let deadline = Instant::now() + Duration::from_secs(10);
    while crate::disposable::ensure_ended(&handle, None).is_err() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
    crate::disposable::ensure_ended(&handle, None).expect("the unit has ended");
    the_layer_holds_no_mask_and_no_foreign_attribute(test, &layer, project, &before);
    let dir = layer.dir().to_path_buf();
    layer.discard().map_err(|(_, err)| err).unwrap();
    assert!(!dir.exists(), "the layer is gone");
}

/// Was der Agent im Gast sah: `.env` nur lesbar und leer, `.git/hooks`
/// beschreibbar, beide als eigene Einhängung, und `user.foreign` mit
/// `ENOTSUP` (95) abgelehnt.
fn the_guest_saw_its_masks(shown: &str) {
    let value = |key: &str| {
        shown
            .lines()
            .find_map(|line| line.trim().strip_prefix(key))
            .unwrap_or_else(|| panic!("no {key} line: {shown}"))
            .to_owned()
    };
    assert_eq!(
        value("env-mode="),
        "600",
        "the mask over .env has bwrap's mode: {shown}"
    );
    assert_ne!(
        value("chmod-rc="),
        "0",
        "the mask over .env took a chmod: {shown}"
    );
    assert_ne!(value("env-rc="), "0", "a write to .env succeeded: {shown}");
    assert_eq!(
        value("env-bytes="),
        "0",
        "the mask over .env is empty: {shown}"
    );
    assert_eq!(
        value("hook-rc="),
        "0",
        "the tmpfs over .git/hooks is writable: {shown}"
    );
    assert!(
        shown.contains("mask-mount:/work/.env:tmpfs:ro,"),
        "the empty file lies over .env: {shown}"
    );
    assert!(
        shown.contains("mask-mount:/work/.git/hooks:tmpfs:rw,"),
        "a tmpfs lies over .git/hooks: {shown}"
    );
    // Masks inside a tmpfs mask, as a profile may nest them: their mount
    // points lie in that tmpfs, as bwrap creates them.
    assert_eq!(value("nested-bytes="), "0", "{shown}");
    assert_ne!(
        value("nested-rc="),
        "0",
        "a write to custom/secret: {shown}"
    );
    assert_eq!(value("nested-inner-rc="), "0", "{shown}");
    for (point, mode) in [
        ("/work/custom", "rw"),
        ("/work/custom/inner", "rw"),
        ("/work/custom/secret", "ro"),
    ] {
        assert!(
            shown.contains(&format!("mask-mount:{point}:tmpfs:{mode},")),
            "{point} is a mask: {shown}"
        );
    }
    assert_eq!(
        value("xattr-foreign="),
        "errno95",
        "the layer's file daemon took a foreign attribute: {shown}"
    );
}

/// Die Schicht nach dem Ende der Unit: weder `.env` noch der Hook darin,
/// kein Attribut außer `user.overlay.*`, das opake Verzeichnis da, und der
/// Scan sieht die Änderungen des Agenten, keine auf einer Maske.
fn the_layer_holds_no_mask_and_no_foreign_attribute(
    test: &str,
    layer: &crate::Layer,
    project: &Path,
    before: &crate::TreeSnapshot,
) {
    let upper = layer.upper();
    assert!(
        std::fs::symlink_metadata(upper.join(".env")).is_err(),
        "the masked .env reached the layer"
    );
    assert!(
        std::fs::symlink_metadata(upper.join(".git/hooks/pre-commit")).is_err(),
        "the write into the masked .git/hooks reached the layer"
    );
    assert!(
        std::fs::symlink_metadata(upper.join("custom")).is_err(),
        "a write under the nested masks reached the layer"
    );
    let names = xattr_names(&upper);
    eprintln!("{test}: attributes in the layer: {names:?}");
    assert!(
        names.iter().all(|name| name
            .split_once(':')
            .is_some_and(|(_, n)| n.starts_with("user.overlay."))),
        "only the overlay's own attributes are in the layer: {names:?}"
    );
    assert!(
        names.contains(&"dir:user.overlay.opaque".to_owned()),
        "the overlay's opaque directory came through: {names:?}"
    );

    let upper_fd = crate::worktree::open_root(&upper).unwrap();
    let lower = crate::worktree::open_root(project).unwrap();
    let set =
        crate::disposable::scan_masked(upper_fd.as_fd(), lower.as_fd(), before, layer.masks());
    let listed: Vec<String> = set
        .changes
        .iter()
        .map(|change| change.rel.to_string_lossy().into_owned())
        .collect();
    for expected in ["new.txt", "dir/sub", "dir/x"] {
        assert!(
            listed.iter().any(|rel| rel == expected),
            "{expected} is missing from {listed:?}"
        );
    }
    assert!(
        listed
            .iter()
            .all(|rel| rel != ".env" && !rel.starts_with(".git")),
        "no change on a mask: {listed:?}"
    );
}
