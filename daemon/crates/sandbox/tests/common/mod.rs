//! Werkzeug, das die Integrationstests dieser Crate teilen.
//!
//! Dasselbe steht als `crate::test_support` für die Unit-Tests in `src/`;
//! ein Integrationstest ist eine eigene Crate und sieht jenes Modul nicht.

/// Legt ein ausführbares Programm an, ohne dass ein Deskriptor dieses
/// Prozesses darauf offen bleibt.
///
/// **Warum nicht einfach `std::fs::write`.** Ein Testbinary hat viele Fäden,
/// und ein `fork` in einem anderen Faden erbt jeden offenen Deskriptor. Fällt
/// er in das Fenster, in dem die Datei hier zum Schreiben offen ist, hält das
/// Kind sie bis zu seinem eigenen `exec` offen, und unser `exec` bekommt
/// `ETXTBSY` -- „Text file busy". `O_CLOEXEC` hilft nicht, weil das Fenster
/// zwischen `fork` und `exec` liegt. Am 2026-09-07 sind die CI und
/// `verify-commit` daran rot geworden
/// (`find_program_reads_path_from_the_given_env_only` und
/// `a_hanging_call_is_cut_off_and_reported_as_a_timeout`), lokal war es unter
/// Last nie zu sehen; die Suche nach der Ursache kostete beide Male dieselbe
/// Zeit (HUM-134).
///
/// Geschrieben wird deshalb über einen eigenen Prozess: Sein Deskriptor gehört
/// ihm, kein anderer Faden dieses Prozesses kann ihn erben, und wenn `cp`
/// zurück ist, hat niemand die Datei mehr zum Schreiben offen.
///
/// Drei Feinheiten, jede aus dem Review dieses Standes:
/// * Der Name der Quelle wird **angehängt** und nicht ersetzt: Mit
///   `with_extension` trügen `test.sh` und `test.py` denselben Quellnamen, und
///   ein `foo.source` wäre seine eigene Quelle -- das `cp` scheiterte, und das
///   Aufräumen löschte das Programm.
/// * Ein vorhandenes Ziel wird vorher entfernt, damit `cp` eine neue Datei
///   anlegt statt die alte zu leeren; die alte könnte gerade ausgeführt
///   werden, und dann wäre `ETXTBSY` zurück.
/// * Die Quelle wird gelöscht, damit im Verzeichnis nur steht, was der Test
///   angelegt hat.
pub fn write_program(path: &std::path::Path, body: &str) {
    use std::os::unix::fs::PermissionsExt as _;

    let mut name = path.file_name().expect("a file name").to_os_string();
    name.push(".source");
    let source = path.with_file_name(name);
    std::fs::write(&source, body).expect("the source of the program");
    let _ = std::fs::remove_file(path);
    let status = std::process::Command::new("cp")
        .arg(&source)
        .arg(path)
        .status()
        .expect("cp runs");
    assert!(status.success(), "cp failed: {status}");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .expect("the program is executable");
    std::fs::remove_file(&source).expect("the source is gone");
}
