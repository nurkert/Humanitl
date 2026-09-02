//! `docs/DIAGNOSTICS.md` wird aus dem Register erzeugt.
//!
//! Die Datei ist damit nie veraltet: wer einen Code hinzufügt, lässt den Test
//! einmal mit `UPDATE_DIAG_DOCS=1` laufen und legt die Änderung mit ins Commit.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use humanitl_core::diagnostics::codes::{AREAS, CODES};

fn docs_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../docs/DIAGNOSTICS.md")
}

fn render() -> String {
    let mut out = String::new();
    out.push_str("# Diagnose-Codes\n\n");
    out.push_str(
        "<!-- Erzeugt aus daemon/crates/core-types/src/diagnostics/codes.rs.\n     \
         Nicht von Hand ändern: `UPDATE_DIAG_DOCS=1 cargo test -p humanitl-core --test diag_docs` \
         schreibt die Datei neu. -->\n\n",
    );
    out.push_str(
        "Jeder nicht-grüne Zustand trägt einen Code der Form `BEREICH_NNN`. Der Code steht in der\n\
         Meldung, in der Oberfläche und in `audit.jsonl`; er ist der kürzeste Weg von einer\n\
         Beobachtung zu ihrer Erklärung. Eine Nummer wird nie wiederverwendet, auch nicht nach dem\n\
         Entfernen eines Codes.\n\n",
    );

    out.push_str("## Reservierte Bereiche\n\n");
    out.push_str("| Bereich | Präfix | Von | Bis | Wofür |\n|---|---|---|---|---|\n");
    for area in AREAS {
        let _ = writeln!(
            out,
            "| {} | `{}` | {:03} | {:03} | {} |",
            area.area, area.prefix, area.first, area.last, area.note
        );
    }
    out.push('\n');

    out.push_str("## Codes\n\n");
    for area in AREAS {
        let codes: Vec<_> = CODES.iter().filter(|info| info.area == area.area).collect();
        if codes.is_empty() {
            continue;
        }
        let _ = writeln!(out, "### Bereich {}\n", area.area);
        for info in codes {
            let _ = writeln!(out, "#### {}\n", info.code);
            let _ = writeln!(out, "{}\n", info.title);
        }
    }

    out
}

#[test]
fn docs_in_sync() {
    let path = docs_path();
    let rendered = render();

    if std::env::var_os("UPDATE_DIAG_DOCS").is_some() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap_or_else(|err| panic!("{err}"));
        }
        std::fs::write(&path, &rendered).unwrap_or_else(|err| panic!("{err}"));
        return;
    }

    let current = std::fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!(
            "{} is missing ({err}); run UPDATE_DIAG_DOCS=1 cargo test -p humanitl-core --test diag_docs",
            path.display()
        )
    });

    assert_eq!(
        current, rendered,
        "docs/DIAGNOSTICS.md is stale; run UPDATE_DIAG_DOCS=1 cargo test -p humanitl-core --test diag_docs"
    );
}

#[test]
fn every_code_has_a_heading_in_the_rendered_docs() {
    let rendered = render();
    for info in CODES {
        assert!(
            rendered.contains(&format!("#### {}\n", info.code)),
            "{} is missing from the rendered docs",
            info.code
        );
        assert!(rendered.contains(info.title), "{} has no title", info.code);
    }
}
