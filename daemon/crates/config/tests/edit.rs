//! Der Schreiber für `config.toml` (HUM-151): Er ändert genau den einen Wert
//! und sonst nichts, und wo er das nicht kann, bleibt die Datei unberührt.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use humanitl_config::edit::{Written, set_sandbox_env};
use humanitl_core::diagnostics::codes::{CONFIG_001, CONFIG_015};

const NAME: &str = "CURL_CA_BUNDLE";
const CA: &str = "/etc/humanitl/ca.crt";
const LINE: &str = r#"CURL_CA_BUNDLE = "/etc/humanitl/ca.crt""#;

/// Legt `config.toml` mit diesem Text in `dir` an.
fn config_in(dir: &Path, text: &str) -> PathBuf {
    let path = dir.join("config.toml");
    fs::write(&path, text).unwrap();
    path
}

/// Was nach dem Parsen unter `sandbox.env.<name>` steht.
fn env_value(path: &Path, name: &str) -> Option<String> {
    let table: toml::Table = fs::read_to_string(path).unwrap().parse().unwrap();
    table
        .get("sandbox")?
        .get("env")?
        .get(name)?
        .as_str()
        .map(str::to_owned)
}

/// Schreibt und liefert den Text danach.
fn written(text: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let path = config_in(dir.path(), text);
    assert_eq!(set_sandbox_env(&path, NAME, CA).unwrap(), Written::Changed);
    fs::read_to_string(&path).unwrap()
}

#[test]
fn a_missing_file_is_created_private_in_a_private_directory() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("humanitl");
    let path = home.join("config.toml");

    assert_eq!(set_sandbox_env(&path, NAME, CA).unwrap(), Written::Changed);

    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        format!("[sandbox.env]\n{LINE}\n")
    );
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(&home).unwrap().permissions().mode() & 0o777,
        0o700
    );
    let entries: Vec<_> = fs::read_dir(&home).unwrap().collect();
    assert_eq!(entries.len(), 1, "no side file is left behind: {entries:?}");
}

#[test]
fn comments_order_and_other_blocks_survive_and_the_line_joins_its_block() {
    let text = "# my settings\n\
                [hold]\n\
                # long enough to read a diff\n\
                timeout_secs = 42 # seconds\n\
                \n\
                [sandbox.env]\n\
                # mine\n\
                FOO = \"bar\"\n\
                \n\
                [ui]\n\
                theme = \"dark\"\n";

    let after = written(text);

    let expected = text.replace("FOO = \"bar\"\n", &format!("FOO = \"bar\"\n{LINE}\n"));
    assert_eq!(after, expected);
}

#[test]
fn an_existing_value_is_replaced_where_it_stands_with_its_comment() {
    let after = written(
        "[sandbox.env]\n\
         CURL_CA_BUNDLE = \"/old/ca.pem\" # from a colleague\n\
         OTHER = \"x\"\n",
    );

    assert_eq!(
        after,
        format!("[sandbox.env]\n{LINE} # from a colleague\nOTHER = \"x\"\n")
    );
}

#[test]
fn a_new_block_follows_the_rest_without_an_empty_sandbox_header() {
    let text = "[hold]\ntimeout_secs = 42\n";

    let after = written(text);

    assert!(after.starts_with(text), "{after}");
    assert!(
        after
            .trim_end()
            .ends_with(&format!("[sandbox.env]\n{LINE}")),
        "{after}"
    );
    assert!(!after.contains("[sandbox]\n"), "{after}");
}

#[test]
fn a_file_of_comments_keeps_them_above_the_new_block() {
    let after = written("# my settings\n# more later\n");

    assert_eq!(
        after,
        format!("# my settings\n# more later\n\n[sandbox.env]\n{LINE}\n")
    );
}

#[test]
fn an_inline_sandbox_without_env_is_extended_or_refused_but_never_broken() {
    let text = "sandbox = { work_dir = \"/w\" }\n";
    let dir = tempfile::tempdir().unwrap();
    let path = config_in(dir.path(), text);

    match set_sandbox_env(&path, NAME, CA) {
        Ok(Written::Changed) => {
            assert_eq!(env_value(&path, NAME).as_deref(), Some(CA));
            let table: toml::Table = fs::read_to_string(&path).unwrap().parse().unwrap();
            assert_eq!(table["sandbox"]["work_dir"].as_str(), Some("/w"));
        }
        Ok(Written::Unchanged) => panic!("the value was not there before"),
        Err(refused) => {
            assert_eq!(refused.code, CONFIG_015);
            assert_eq!(fs::read_to_string(&path).unwrap(), text);
        }
    }
}

#[test]
fn a_sandbox_block_with_other_keys_keeps_them() {
    let text = "[sandbox]\nwork_dir = \"/home/me/project\"\n";

    let after = written(text);

    assert!(after.starts_with(text), "{after}");
    let table: toml::Table = after.parse().unwrap();
    assert_eq!(
        table["sandbox"]["work_dir"].as_str(),
        Some("/home/me/project")
    );
    assert_eq!(table["sandbox"]["env"][NAME].as_str(), Some(CA));
}

#[test]
fn an_inline_table_and_a_dotted_key_are_extended_in_their_own_form() {
    // Die Leerzeichen innerhalb der Inline-Tabelle regelt `toml_edit`; was hier
    // zählt, ist die Form: eine Zeile, beide Werte.
    let inline = written("[sandbox]\nenv = { FOO = \"bar\" }\n");
    assert!(
        inline.starts_with("[sandbox]\nenv = { FOO = \"bar\""),
        "{inline}"
    );
    assert_eq!(inline.lines().count(), 2, "{inline}");
    let table: toml::Table = inline.parse().unwrap();
    assert_eq!(table["sandbox"]["env"]["FOO"].as_str(), Some("bar"));
    assert_eq!(table["sandbox"]["env"][NAME].as_str(), Some(CA));

    let dotted = written("sandbox.env.FOO = \"bar\"\n");
    let table: toml::Table = dotted.parse().unwrap();
    assert_eq!(table["sandbox"]["env"]["FOO"].as_str(), Some("bar"));
    assert_eq!(table["sandbox"]["env"][NAME].as_str(), Some(CA));
    assert!(
        !dotted.contains("[sandbox"),
        "the dotted form stays dotted: {dotted}"
    );
}

#[test]
fn a_sandbox_or_env_that_is_not_a_table_is_config_015_and_the_file_is_untouched() {
    for text in [
        "sandbox = \"nope\"\n",
        "[[sandbox]]\nwork_dir = \"/w\"\n",
        "[sandbox]\nenv = \"FOO=bar\"\n",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = config_in(dir.path(), text);

        let refused = set_sandbox_env(&path, NAME, CA).unwrap_err();

        assert_eq!(refused.code, CONFIG_015, "{text}");
        assert!(
            refused.why.contains(LINE),
            "the finding names the line: {}",
            refused.why
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), text);
    }
}

#[test]
fn a_file_that_is_no_toml_is_config_001_and_is_left_alone() {
    let text = "[sandbox\nFOO =\n";
    let dir = tempfile::tempdir().unwrap();
    let path = config_in(dir.path(), text);

    let refused = set_sandbox_env(&path, NAME, CA).unwrap_err();

    assert_eq!(refused.code, CONFIG_001);
    assert_eq!(fs::read_to_string(&path).unwrap(), text);
}

#[test]
fn the_same_value_again_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = config_in(dir.path(), &format!("[sandbox.env]\n{LINE}\n"));
    let before = fs::metadata(&path).unwrap().ino();

    assert_eq!(
        set_sandbox_env(&path, NAME, CA).unwrap(),
        Written::Unchanged
    );

    // `rename` gäbe der Datei einen neuen Inode; derselbe heißt: nicht angefasst.
    assert_eq!(fs::metadata(&path).unwrap().ino(), before);
}

#[test]
fn the_rights_of_an_existing_file_stay() {
    let dir = tempfile::tempdir().unwrap();
    let path = config_in(dir.path(), "");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();

    set_sandbox_env(&path, NAME, CA).unwrap();

    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o640
    );
}

#[test]
fn a_linked_file_stays_linked_and_its_target_changes() {
    let dir = tempfile::tempdir().unwrap();
    let dotfiles = dir.path().join("dotfiles");
    fs::create_dir(&dotfiles).unwrap();
    let real = config_in(&dotfiles, "# kept\n");
    let config_dir = dir.path().join("config");
    fs::create_dir(&config_dir).unwrap();
    let link = config_dir.join("config.toml");
    std::os::unix::fs::symlink(&real, &link).unwrap();

    set_sandbox_env(&link, NAME, CA).unwrap();

    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(env_value(&real, NAME).as_deref(), Some(CA));
    assert!(fs::read_to_string(&real).unwrap().starts_with("# kept\n"));
}

#[test]
fn writers_at_the_same_time_lose_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = config_in(dir.path(), "");
    let names: Vec<String> = (0..8).map(|n| format!("CA_VARIABLE_{n}")).collect();

    std::thread::scope(|scope| {
        for name in &names {
            let path = &path;
            scope.spawn(move || set_sandbox_env(path, name, CA).unwrap());
        }
    });

    for name in &names {
        assert_eq!(env_value(&path, name).as_deref(), Some(CA), "{name}");
    }
}

#[test]
fn a_file_with_crlf_line_ends_keeps_them() {
    let text = "# kept\r\n[hold]\r\ntimeout_secs = 42\r\n";

    let after = written(text);

    assert!(after.starts_with(text), "{after:?}");
    assert!(
        after.ends_with(&format!("[sandbox.env]\r\n{LINE}\r\n")),
        "{after:?}"
    );
    assert!(
        !after.replace("\r\n", "").contains('\n'),
        "no bare line end: {after:?}"
    );
}

#[test]
fn a_leading_byte_order_mark_stays() {
    let text = "\u{feff}# kept\n[hold]\ntimeout_secs = 42\n";

    let after = written(text);

    assert!(after.starts_with(text), "{after:?}");
    assert_eq!(env_value_of(&after, NAME).as_deref(), Some(CA));
}

/// Was nach dem Parsen eines Textes mit Markierung unter `sandbox.env.<name>`
/// steht.
fn env_value_of(text: &str, name: &str) -> Option<String> {
    let table: toml::Table = text.trim_start_matches('\u{feff}').parse().unwrap();
    table
        .get("sandbox")?
        .get("env")?
        .get(name)?
        .as_str()
        .map(str::to_owned)
}
