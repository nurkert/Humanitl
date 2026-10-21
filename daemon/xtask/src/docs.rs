//! Die erzeugten Referenzseiten außer der Paritäts-Tabelle (HUM-059).
//!
//! Jede dieser Seiten hat ihren Generator dort, wo ihre Quelle liegt, als
//! Test der Crate, die die Quelle kennt: die Konfiguration in
//! `humanitl-config` (`tests/config_docs.rs`), das Register der Diagnosen in
//! `humanitl-core` (`tests/diag_docs.rs`) und die Kommandozeile im Binary
//! `humanitl` (`src/cli_docs.rs`, die `clap`-Struktur ist nur dort sichtbar).
//! Ohne Umgebungsvariable vergleicht jeder dieser Tests die Datei auf der
//! Platte mit dem, was er erzeugt, und fällt bei einer Abweichung; mit ihr
//! schreibt er die Datei neu.
//!
//! Die Generatoren bleiben dort, statt hierher zu ziehen: `xtask` müsste
//! sonst gegen alle drei Crates bauen und ihre Typen nachbilden, und der
//! Vergleich liefe zweimal, einmal in `cargo test --workspace` und einmal
//! hier. `cargo xtask docs` ist der eine Einstieg, der alle Seiten erzeugt;
//! `cargo xtask docs --check` ist der eine Vergleich, den der CI-Job
//! `parity-check` fährt.

use std::error::Error;
use std::path::Path;
use std::process::Command;

/// Eine erzeugte Seite und der Test, der sie schreibt oder prüft.
pub struct Generated {
    /// Die Datei, vom Repository aus gesehen.
    pub file: &'static str,
    /// Das Paket, in dem der Generator liegt.
    pub package: &'static str,
    /// Die Auswahl des Tests für `cargo test -p <package>`.
    pub selector: &'static [&'static str],
    /// Die Variable, mit der der Test die Datei schreibt statt sie zu prüfen.
    pub update_env: &'static str,
}

/// Alle Seiten, die `cargo xtask docs` außer `parity.md` erzeugt.
pub const GENERATED: &[Generated] = &[
    Generated {
        file: "docs/CONFIG.md",
        package: "humanitl-config",
        selector: &["--test", "config_docs"],
        update_env: "UPDATE_CONFIG_DOCS",
    },
    Generated {
        file: "docs/DIAGNOSTICS.md",
        package: "humanitl-core",
        selector: &["--test", "diag_docs"],
        update_env: "UPDATE_DIAG_DOCS",
    },
    Generated {
        file: "docs/reference/cli.md",
        package: "humanitl",
        selector: &["--bin", "humanitl", "cli_docs::"],
        update_env: "UPDATE_CLI_DOCS",
    },
];

/// Erzeugt (`check == false`) oder prüft (`check == true`) jede Seite aus
/// [`GENERATED`], indem der zuständige Test als eigener `cargo`-Lauf startet.
///
/// # Errors
///
/// Wenn ein Lauf scheitert: beim Prüfen heißt das, die Seite fehlt oder
/// weicht ab, beim Erzeugen, dass der Generator nicht übersetzt oder die
/// Datei nicht schreiben kann. Jede Seite wird versucht, bevor der Fehler
/// kommt, damit ein Lauf alle Abweichungen auf einmal nennt.
pub fn run(daemon_dir: &Path, check: bool, github: bool) -> Result<(), Box<dyn Error>> {
    // `cargo xtask` setzt `CARGO` auf das cargo, das den Lauf gestartet hat;
    // dasselbe nimmt auch die Unterläufe, damit Toolchain und Ziel gleich
    // bleiben.
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut failed = Vec::new();
    for page in GENERATED {
        let mut command = Command::new(&cargo);
        command
            .current_dir(daemon_dir)
            .args(["test", "--quiet", "-p", page.package])
            .args(page.selector);
        if check {
            command.env_remove(page.update_env);
        } else {
            command.env(page.update_env, "1");
        }
        let status = command.status()?;
        if status.success() {
            let verb = if check { "checked" } else { "generated" };
            println!("docs: {} ({verb})", page.file);
        } else {
            let message = if check {
                format!(
                    "{} is missing or stale; run `cargo xtask docs` and commit the result",
                    page.file
                )
            } else {
                format!("{} could not be generated", page.file)
            };
            if github {
                eprintln!("::error::docs: {message}");
            } else {
                eprintln!("error: docs: {message}");
            }
            failed.push(page.file);
        }
    }
    if failed.is_empty() {
        Ok(())
    } else {
        Err(format!("generated pages failed: {}", failed.join(", ")).into())
    }
}
