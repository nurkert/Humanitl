//! Werkzeug, das nur die Unit-Tests dieser Crate benutzen.

/// Legt ein ausführbares Skript an, ohne dass ein Deskriptor dieses Prozesses
/// darauf zum Schreiben offen bleibt.
///
/// **Warum nicht einfach `std::fs::write`.** Ein Testbinary hat viele Fäden,
/// und ein `fork` in einem anderen Faden erbt jeden offenen Deskriptor. Fällt
/// er in das Fenster, in dem das Skript hier zum Schreiben offen ist, hält das
/// Kind es bis zu seinem eigenen `exec` offen, und unser `exec` des Skripts
/// bekommt `ETXTBSY`, „Text file busy". `O_CLOEXEC` hilft nicht, weil das
/// Fenster zwischen `fork` und `exec` liegt. Die CI ist daran zweimal rot
/// geworden (HUM-267); dieselbe Ursache hatte HUM-134 in der Sandbox-Crate.
///
/// Geschrieben wird deshalb über einen eigenen Prozess: Der Inhalt landet erst
/// in einer Quelldatei, die nie ausgeführt wird, und `cp` legt das Skript an.
/// Den Schreib-Deskriptor auf das Skript hat nur `cp`, kein Faden dieses
/// Prozesses kann ihn erben, und wenn `cp` zurück ist, hält niemand das
/// Skript mehr zum Schreiben offen. Ein vorhandenes Ziel wird vorher entfernt,
/// damit `cp` eine neue Datei anlegt, statt eine womöglich gerade laufende zu
/// überschreiben. Ein Umbenennen oder harter Link der Quelle hülfe nicht: Es
/// bliebe dieselbe Inode, auf die ein geerbter Deskriptor zeigen kann.
#[allow(clippy::expect_used)]
pub(crate) fn write_executable(path: &std::path::Path, body: &str) {
    use std::os::unix::fs::PermissionsExt as _;

    let mut name = path.file_name().expect("a file name").to_os_string();
    name.push(".source");
    let source = path.with_file_name(name);
    std::fs::write(&source, body).expect("the source of the script");
    let _ = std::fs::remove_file(path);
    let status = std::process::Command::new("cp")
        .arg(&source)
        .arg(path)
        .status()
        .expect("cp runs");
    assert!(status.success(), "cp failed: {status}");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .expect("the script is executable");
    std::fs::remove_file(&source).expect("the source is gone");
}
