//! `docs/CONFIG.md` wird aus dem Schema erzeugt.
//!
//! Wie bei `docs/DIAGNOSTICS.md` (HUM-063) gilt: Die Datei ist nie veraltet.
//! Wer ein Feld hinzufügt, lässt den Test einmal mit `UPDATE_CONFIG_DOCS=1`
//! laufen und legt die Änderung mit ins Commit.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use humanitl_config::scope::ProjectScope;
use humanitl_config::tier::Tier;
use humanitl_config::{alias, schema};

fn docs_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../docs/CONFIG.md")
}

fn env_name(path: &str) -> String {
    format!("HUMANITL_{}", path.to_uppercase().replace('.', "__"))
}

fn escape(text: &str) -> String {
    text.replace('|', "\\|").replace('\n', " ")
}

fn render() -> String {
    let mut out = String::new();
    render_preamble(&mut out);
    render_fields(&mut out);
    render_aliases_and_paths(&mut out);
    out
}

/// Titel, Sichtbarkeitsstufen, Projekt-Profil und Reihenfolge der Quellen.
fn render_preamble(out: &mut String) {
    out.push_str("# Konfiguration\n\n");
    out.push_str(
        "<!-- Erzeugt aus daemon/crates/config/src/model.rs.\n     \
         Nicht von Hand ändern: `UPDATE_CONFIG_DOCS=1 cargo test -p humanitl-config --test config_docs` \
         schreibt die Datei neu. -->\n\n",
    );
    out.push_str(
        "Humanitl hat eine Konfigurationsquelle: die Rust-Typen in `humanitl-config`. Aus ihnen\n\
         entstehen das JSON-Schema, die Prüfung beim Laden, der Einstellungs-Bildschirm und diese\n\
         Seite. Wer ein Feld ändern will, ändert den Typ.\n\n",
    );

    out.push_str("## Sichtbarkeitsstufen\n\n");
    out.push_str("| Stufe | Wo sie erscheint |\n|---|---|\n");
    out.push_str("| `basic` | Immer sichtbar, im ersten Bild der Einstellungen. |\n");
    out.push_str("| `advanced` | Hinter „Mehr anzeigen\". |\n");
    out.push_str("| `expert` | Nur in `config.toml` und hier. |\n\n");

    out.push_str("## Projekt-Profil\n\n");
    out.push_str(
        "`<projekt>/.humanitl/profile.toml` liegt im geklonten Repository und ist damit\n\
         Angreifer-beeinflusst: Wer ein Repository klont, führt dessen Profil aus. Die Spalte\n\
         „Projekt\" sagt für jedes Feld, ob das Projekt-Profil es setzen darf (im JSON-Schema\n\
         `x-project-scope`).\n\n",
    );
    out.push_str("| Projekt | Bedeutung |\n|---|---|\n");
    out.push_str("| `allowed` | Das Projekt-Profil darf den Wert setzen. |\n");
    out.push_str(
        "| `denied` | Nur Vorgabewerte, `config.toml`, globales Profil, Umgebung oder \
         Kommandozeile dürfen den Wert setzen. Im Projekt-Profil ist der Schlüssel \
         `CONFIG_003`, auch unter einem alten Namen. |\n\n",
    );
    out.push_str(
        "Zwei Werte werden beim Laden über ihren Typ hinaus geprüft: `llm.endpoint` nimmt nur\n\
         `http` oder `https`; `sandbox.work_dir` muss absolut sein, ohne `..`, und ein\n\
         existierendes Verzeichnis, das sich kanonisieren lässt. Beides sonst `CONFIG_003`.\n\n",
    );

    out.push_str("## Wirkung\n\n");
    out.push_str(
        "Diese Seite entsteht aus dem Schema und kann deshalb keinen Schlüssel vergessen. Ob ein\n\
         Schlüssel etwas bewirkt, steht dem Schema nicht an: Ein Wert, der beschrieben und geprüft\n\
         wird und den niemand liest, sähe hier aus wie jeder andere. Die Spalte „Wirkung\" sagt es\n\
         deshalb ausdrücklich (im JSON-Schema `x-pending-issue`, HUM-101).\n\n",
    );
    out.push_str("| Wirkung | Bedeutung |\n|---|---|\n");
    out.push_str("| `ja` | Der Schlüssel wird gelesen und wirkt. |\n");
    out.push_str(
        "| `offen (HUM-xxx)` | Der Schlüssel hat heute keinen Leser. Das genannte Issue \
         entscheidet ihn, durch Einbau oder durch Streichung; bis dahin ändert sein Wert nichts. \
         Das Register `daemon/crates/config/tests/config_readers.rs` hält für jeden Schlüssel \
         fest, welcher der beiden Fälle gilt. |\n\n",
    );

    out.push_str("## Reihenfolge der Quellen\n\n");
    out.push_str(
        "Von unten nach oben; die obere Ebene gewinnt. Jedes Feld merkt sich, aus welcher Ebene\n\
         sein Wert stammt, und die Oberfläche zeigt es an.\n\n",
    );
    out.push_str("| Ebene | Quelle |\n|---|---|\n");
    out.push_str("| 1 | eingebaute Vorgabewerte |\n");
    out.push_str("| 2 | `$XDG_CONFIG_HOME/humanitl/config.toml` |\n");
    out.push_str(
        "| 3 | Profil `default`: `$XDG_CONFIG_HOME/humanitl/profiles/default.toml`, sonst die \
         eingebettete Fassung |\n",
    );
    out.push_str(
        "| 4 | das gewählte Profil, falls es nicht `default` ist; Datei, sonst eingebettet |\n",
    );
    out.push_str(
        "| 5 | `<projekt>/.humanitl/profile.toml`, Block `[config]`; nur Felder mit Projekt \
         `allowed` |\n",
    );
    out.push_str("| 6 | Umgebungsvariablen `HUMANITL_*` |\n");
    out.push_str("| 7 | Argumente der Kommandozeile |\n\n");
    out.push_str(
        "Ein Profil hat neben `[config]` nur `name`, `description` und `[rules]` (HUM-066,\n\
         `docs/profiles.md`). Jeder andere Block auf der obersten Ebene ist `CONFIG_002`; eine\n\
         Gruppe wie `[hold]` gehört im Profil unter `[config.hold]`. Das mitgelieferte Profil\n\
         `default` setzt mit Absicht keinen Wert: es liegt über `config.toml` und machte sie\n\
         sonst für jeden Schlüssel wirkungslos, den es nennt.\n\n",
    );
    out.push_str(
        "`<projekt>` ist `sandbox.work_dir`, sonst das aktuelle Verzeichnis — nicht umgekehrt:\n\
         Wer mit `--work` aus einem fremden Verzeichnis heraus arbeitet, bekommt das Profil des\n\
         Projekts, an dem er arbeitet. Der Schlüssel ist auf der Projekt-Ebene gesperrt, deshalb\n\
         kann das Projekt-Profil nicht bestimmen, wo nach ihm gesucht wird. Sein `name` wählt nur\n\
         unter den mitgelieferten Profilen; jeder andere Wunsch wird übergangen und mit\n\
         `CONFIG_009` gemeldet (`docs/profiles.md`).\n\n",
    );
    out.push_str(
        "Eine Umgebungsvariable heißt wie ihr Pfad in Großbuchstaben, mit `__` zwischen den\n\
         Ebenen: `hold.timeout_secs` wird zu `HUMANITL_HOLD__TIMEOUT_SECS`. Der Wert wird nach dem\n\
         Typ des Feldes gelesen: für ein Textfeld bleibt er Text, sonst wird er als Wahrheitswert,\n\
         dann als Zahl, sonst als Zeichenkette gelesen; eine Liste steht in eckigen Klammern.\n\
         Ein unbekannter Schlüssel in einer Datei oder auf der Kommandozeile ist ein Fehler\n\
         (`CONFIG_002`), in der Umgebung eine Warnung, die das Laden nicht abbricht. Variablen\n\
         ohne `__` im Namen (`HUMANITL_GALLERY`, `HUMANITL_ESCAPE_MARKER`) gehören anderen\n\
         Werkzeugen und werden übergangen.\n\n",
    );
}

/// Ein Abschnitt je Gruppe, eine Zeile je Blattfeld.
fn render_fields(out: &mut String) {
    out.push_str("## Felder\n\n");
    for (group, fields) in schema::by_group() {
        let heading =
            schema::field(group).map_or_else(String::new, |field| field.description.clone());
        let _ = writeln!(out, "### `{group}`\n");
        if !heading.is_empty() {
            let _ = writeln!(out, "{heading}\n");
        }
        out.push_str(
            "| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Wirkung | Beschreibung |\n\
             |---|---|---|---|---|---|---|\n",
        );
        for field in fields {
            let _ = writeln!(
                out,
                "| `{}` | {} | `{}` | {} | {} | {} | {} |",
                field.path,
                escape(&field.type_label),
                field.default_literal(),
                field.tier,
                field.project_scope,
                field.readiness.doc_label(),
                escape(&field.description)
            );
        }
        out.push('\n');
    }
}

/// Alte Namen und Pfade.
fn render_aliases_and_paths(out: &mut String) {
    out.push_str("## Alte Namen\n\n");
    out.push_str(
        "Diese Schlüssel funktionieren weiter. Steht der alte neben dem heutigen Namen, gewinnt der\n\
         heutige, und das Laden legt einen Befund dazu.\n\n",
    );
    out.push_str("| Alt | Heute | Seit |\n|---|---|---|\n");
    for entry in alias::ALIASES {
        let _ = writeln!(
            out,
            "| `{}` | `{}` | {} |",
            entry.old, entry.canonical, entry.since
        );
    }
    out.push('\n');

    out.push_str("## Entfallene Schlüssel\n\n");
    out.push_str(
        "Diese Schlüssel gab es einmal und gibt es nicht mehr. Sie haben keinen Nachfolger. Wer\n\
         sie noch in einer Datei stehen hat, bekommt beim Laden eine Warnung (`CONFIG_005`) mit\n\
         dem Issue und dem Grund; der Wert wird übergangen, und der Daemon startet trotzdem. Ein\n\
         harter Fehler wäre hier die Strafe für eine Entscheidung, die nicht der Nutzer getroffen\n\
         hat (`backlog/CONVENTIONS.md` 4.25).\n\n",
    );
    out.push_str("| Schlüssel | Entfallen mit | Grund (Text des Befunds) |\n|---|---|---|\n");
    for entry in alias::RETIRED {
        let _ = writeln!(
            out,
            "| `{}` | {} | {} |",
            entry.path,
            entry.since,
            escape(entry.why)
        );
    }
    out.push('\n');

    out.push_str("## Pfade\n\n");
    out.push_str("| Was | Wo |\n|---|---|\n");
    out.push_str("| Konfiguration | `$XDG_CONFIG_HOME/humanitl/config.toml` |\n");
    out.push_str("| Regeln | `$XDG_CONFIG_HOME/humanitl/rules.yaml` |\n");
    out.push_str("| Profile | `$XDG_CONFIG_HOME/humanitl/profiles/<name>.toml` |\n");
    out.push_str("| Projekt-Profil | `<projekt>/.humanitl/profile.toml` |\n");
    out.push_str("| Datenbank | `$XDG_DATA_HOME/humanitl/humanitl.db` |\n");
    out.push_str("| Blobs | `$XDG_DATA_HOME/humanitl/blobs/<hex[0..2]>/<sha256-hex>` |\n");
    out.push_str("| Audit | `$XDG_DATA_HOME/humanitl/audit/audit.jsonl` |\n");
    out.push_str("| CA | `$XDG_DATA_HOME/humanitl/ca/ca.crt`, `ca.key` (0600) |\n");
    out.push_str(
        "| Daemon-Socket | `$XDG_RUNTIME_DIR/humanitl/daemon.sock` (0600, Verzeichnis 0700) |\n",
    );
    out.push_str("| Proxy-Socket | `$XDG_RUNTIME_DIR/humanitl/proxy/proxy.sock` |\n");
    out.push_str("| Token | `$XDG_RUNTIME_DIR/humanitl/token` (0600) |\n\n");
    out.push_str(
        "Fehlt `XDG_RUNTIME_DIR`, wird `/run/user/<uid>` benutzt; fehlt auch das, weicht Humanitl\n\
         auf `$TMPDIR/humanitl-<uid>` aus und meldet `CONFIG_004` als Hinweis.\n",
    );
}

#[test]
fn docs_in_sync() {
    let path = docs_path();
    let rendered = render();

    if std::env::var_os("UPDATE_CONFIG_DOCS").is_some() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create docs dir");
        }
        std::fs::write(&path, &rendered).expect("write docs");
        return;
    }

    let current = std::fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!(
            "{} is missing ({err}); run UPDATE_CONFIG_DOCS=1 cargo test -p humanitl-config --test config_docs",
            path.display()
        )
    });

    assert_eq!(
        current, rendered,
        "docs/CONFIG.md is stale; run UPDATE_CONFIG_DOCS=1 cargo test -p humanitl-config --test config_docs"
    );
}

#[test]
fn every_field_appears_with_its_environment_variable_rule() {
    let rendered = render();
    for field in schema::leaves() {
        assert!(
            rendered.contains(&format!("| `{}` |", field.path)),
            "{} is missing from the rendered docs",
            field.path
        );
    }
    assert!(rendered.contains(&env_name("hold.timeout_secs")));
    for tier in Tier::ALL {
        assert!(
            rendered.contains(&format!("`{tier}`")),
            "{tier} is not explained"
        );
    }
    for scope in ProjectScope::ALL {
        assert!(
            rendered.contains(&format!("`{scope}`")),
            "{scope} is not explained"
        );
    }
}

#[test]
fn every_retired_key_is_listed_with_its_issue() {
    let rendered = render();
    for entry in alias::RETIRED {
        assert!(
            rendered.contains(&format!("| `{}` | {} |", entry.path, entry.since)),
            "{} is not listed among the retired keys",
            entry.path
        );
    }
    assert!(rendered.contains("## Entfallene Schlüssel"));
    assert!(
        rendered.contains("CONFIG_005"),
        "the page must say what a retired key does at load time"
    );
}

#[test]
fn the_effect_column_names_the_issue_of_every_key_without_a_reader() {
    // Die Erwartung steht hier als Wort, nicht als Aufruf von `doc_label()`:
    // Sonst prüfte der Test den Generator gegen sich selbst, und aus „ja" dürfte
    // still „offen" werden, ohne dass etwas rot wird.
    let rendered = render();
    let mut pending = 0_usize;
    for field in schema::leaves() {
        let row = rendered
            .lines()
            .find(|line| line.starts_with(&format!("| `{}` |", field.path)))
            .unwrap_or_else(|| panic!("{} has no row", field.path));
        match field.readiness.issue() {
            None => assert!(
                row.contains("| ja |"),
                "{} has a reader and the column does not say `ja`: {row}",
                field.path
            ),
            Some(issue) => {
                pending += 1;
                assert!(
                    row.contains(&format!("| offen ({issue}) |")),
                    "{} waits for {issue} and the column does not say so: {row}",
                    field.path
                );
            }
        }
    }
    assert!(pending > 0, "no key waits for an issue; the column is dead");
    // Die beiden Wörter der Legende, ebenfalls wörtlich.
    assert!(
        rendered.contains("| `ja` | Der Schlüssel wird gelesen und wirkt. |"),
        "the effective case is not explained"
    );
    assert!(
        rendered.contains("`offen (HUM-xxx)`"),
        "the column is not explained"
    );
    // Und die beiden Fälle je einmal am bekannten Schlüssel.
    assert!(
        rendered.contains("| `hold.timeout_secs` | integer | `300` | basic | allowed | ja |"),
        "the row of an effective key changed shape"
    );
    assert!(
        rendered.contains("| offen (HUM-079) |"),
        "no row carries the pending issue of the pseudonym keys"
    );
}

#[test]
fn the_project_column_follows_the_schema() {
    let rendered = render();
    for field in schema::leaves() {
        let row = rendered
            .lines()
            .find(|line| line.starts_with(&format!("| `{}` |", field.path)))
            .unwrap_or_else(|| panic!("{} has no row", field.path));
        assert!(
            row.contains(&format!("| {} |", field.project_scope)),
            "{}: {row}",
            field.path
        );
    }
}
