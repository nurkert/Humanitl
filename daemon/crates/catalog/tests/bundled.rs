//! Der ausgelieferte Katalog unter `catalog/`.
//!
//! Diese Tests lesen genau die Dateien, die mit dem Programm ausgeliefert
//! werden. Sie halten fest, was ein Mensch nachher im Domain-Panel sieht: dass
//! die bekannten Ziele eines Coding-Agenten getroffen werden, dass kein
//! benachbarter Name mitgetroffen wird, dass jede Beschreibung eine prüfbare
//! Quelle hat und dass die Symbole wirklich im Verzeichnis liegen.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeSet;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use chrono::Utc;
use humanitl_catalog::RANKS_FILE;
use humanitl_catalog::{Catalog, RANKS_LICENSE_FILE};
use humanitl_core::HostName;
use sha2::{Digest as _, Sha256};

/// Das ausgelieferte Datenverzeichnis, vom Quellbaum aus.
///
/// Der Pfad steht nur im Test: Der Daemon bekommt das Verzeichnis aus der
/// Konfiguration beziehungsweise aus dem Installationspfad, und ein
/// Bauzeit-Pfad hat in der Bibliothek nichts zu suchen.
fn bundled_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../catalog")
}

fn catalog() -> Catalog {
    Catalog::load(&bundled_dir())
        .unwrap_or_else(|err| panic!("the bundled catalog must load: {err}"))
}

fn host(text: &str) -> HostName {
    HostName::parse(text).unwrap()
}

#[test]
fn the_bundled_catalog_loads_without_a_finding() {
    let catalog = catalog();
    assert!(
        catalog.entries().len() >= 30,
        "HUM-031 ships about thirty entries, found {}",
        catalog.entries().len()
    );
}

#[test]
fn the_bundled_ranking_loads() {
    let catalog = catalog();
    assert_eq!(
        catalog.ranked_domains(),
        100_000,
        "the bundled list holds the first 100 000 ranks"
    );
    assert_eq!(catalog.rank("google.com"), Some(1));
    assert!(catalog.rank("github.com").is_some());
    assert_eq!(catalog.rank("this-name-is-not-in-the-list.invalid"), None);
}

#[test]
fn the_rank_licence_ships_next_to_the_data() {
    let path = bundled_dir().join(RANKS_LICENSE_FILE);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("{} must exist: {err}", path.display()));
    // Herkunft, Datum und Pruefsummen sind der Beleg dafuer, welche Daten in
    // einem Build steckten; ohne sie ist der Hinweis nur Prosa.
    assert!(
        text.contains("Majestic Million"),
        "the licence names the list that is actually shipped"
    );
    assert!(text.contains("CC BY 3.0"), "the licence names its terms");
    assert!(
        text.contains("downloads.majestic.com"),
        "the licence names where the data came from"
    );
    assert!(
        text.contains("SHA-256"),
        "the checksums belong in the licence"
    );

    // Die Pruefsummen des ausgelieferten Ausschnitts, gepackt und entpackt.
    // Sie stehen im Hinweis und muessen zur Datei passen, sonst behauptet der
    // Hinweis etwas ueber Daten, die gar nicht ausgeliefert werden.
    let packed = std::fs::read(bundled_dir().join(RANKS_FILE)).unwrap();
    assert!(
        text.contains(&sha256_hex(&packed)),
        "the licence names another checksum than the shipped file has"
    );
    let mut unpacked = Vec::new();
    flate2::read::GzDecoder::new(packed.as_slice())
        .read_to_end(&mut unpacked)
        .unwrap();
    assert!(
        text.contains(&sha256_hex(&unpacked)),
        "the licence names another unpacked checksum than the shipped file has"
    );
}

/// Der SHA-256 als Hex, wie ihn `sha256sum` schreibt.
fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

#[test]
fn nothing_in_the_bundle_still_claims_tranco() {
    // Die Standardliste von Tranco mischt Cloudflare Radar unter CC BY-NC 4.0
    // bei. Eine Nicht-kommerziell-Klausel passt nicht in ein Produkt unter
    // GPL-3.0-only, deshalb liegt hier die Majestic Million. Der Name der
    // verworfenen Liste darf nur noch in der Begruendung vorkommen.
    for name in ["domains.yaml", "README.md"] {
        let text = std::fs::read_to_string(bundled_dir().join(name)).unwrap();
        assert!(
            !text.to_lowercase().contains("tranco"),
            "{name} still claims Tranco"
        );
    }
    assert!(
        !bundled_dir().join("TRANCO-LICENSE").exists(),
        "the old licence file must be gone"
    );
    assert!(
        !bundled_dir().join("tranco-top100k.csv.gz").exists(),
        "the old data file must be gone"
    );
}

#[test]
fn every_entry_names_an_icon_that_exists() {
    let icons = bundled_dir().join("icons");
    for entry in catalog().entries() {
        let path = icons.join(&entry.icon);
        assert!(
            path.is_file(),
            "entry {:?} names the icon {:?}, which is not in catalog/icons",
            entry.id,
            entry.icon
        );
    }
    assert!(
        icons.join("globe.svg").is_file(),
        "the fallback icon must exist"
    );
}

#[test]
fn every_entry_has_a_source_and_a_description_in_both_languages() {
    for entry in catalog().entries() {
        assert!(
            entry.source.starts_with("https://") || entry.source.starts_with("http://"),
            "entry {:?} has no checkable source",
            entry.id
        );
        assert!(!entry.description.en.trim().is_empty(), "{}", entry.id);
        assert!(!entry.description.de.trim().is_empty(), "{}", entry.id);
        assert!(
            !entry.typical.is_empty(),
            "entry {:?} says nothing about how an agent gets there",
            entry.id
        );
        if let Some(note) = &entry.risk_note {
            assert!(!note.en.trim().is_empty(), "{}", entry.id);
            assert!(!note.de.trim().is_empty(), "{}", entry.id);
        }
    }
}

#[test]
fn no_two_entries_claim_the_same_pattern() {
    let catalog = catalog();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for entry in catalog.entries() {
        for pattern in &entry.hosts {
            assert!(
                seen.insert(pattern.as_str()),
                "the pattern {pattern:?} appears twice; the second entry would never win"
            );
        }
    }
}

#[test]
fn the_targets_an_agent_reaches_are_recognised() {
    let catalog = catalog();
    let now = Utc::now();
    let expected = [
        ("api.github.com", "github"),
        ("raw.githubusercontent.com", "github"),
        ("gitlab.com", "gitlab"),
        ("registry.npmjs.org", "npm"),
        ("pypi.org", "pypi"),
        ("files.pythonhosted.org", "pypi"),
        ("static.crates.io", "crates-io"),
        ("index.crates.io", "crates-io"),
        ("proxy.golang.org", "go-modules"),
        ("repo1.maven.org", "maven-central"),
        ("pub.dev", "pub-dev"),
        ("registry-1.docker.io", "dockerhub"),
        ("ghcr.io", "ghcr"),
        ("deb.debian.org", "debian"),
        ("dl-cdn.alpinelinux.org", "alpine"),
        ("static.rust-lang.org", "rust-toolchain"),
        ("huggingface.co", "huggingface"),
        ("registry.ollama.ai", "ollama"),
        ("models.dev", "models-dev"),
        ("api.openai.com", "openai"),
        ("api.anthropic.com", "anthropic"),
        ("generativelanguage.googleapis.com", "google-ai"),
        ("openrouter.ai", "openrouter"),
        ("docs.rs", "docs-rs"),
        ("developer.mozilla.org", "mdn"),
        ("stackoverflow.com", "stackoverflow"),
        ("de.wikipedia.org", "wikipedia"),
        ("cdn.jsdelivr.net", "jsdelivr"),
        ("duckduckgo.com", "duckduckgo"),
    ];
    for (name, id) in expected {
        assert_eq!(
            catalog.info(&host(name), now).catalog_id.as_deref(),
            Some(id),
            "{name} should be {id}"
        );
    }
}

#[test]
fn a_lookalike_is_not_recognised() {
    let catalog = catalog();
    let now = Utc::now();
    // Genau die Namen, mit denen jemand auf einen Katalogtreffer hoffen würde.
    for name in [
        "evil-github.com",
        "github.com.evil.io",
        "npmjs.org.attacker.net",
        "pypi.org.cn",
        "crates.io.evil.example",
        "api-openai.com",
        "anthropic.com.evil.io",
        "huggingface.co.attacker.net",
        "notgithub.com",
    ] {
        assert_eq!(
            catalog.info(&host(name), now).catalog_id,
            None,
            "{name} must stay unknown"
        );
    }
}

#[test]
fn an_address_never_gets_a_catalog_entry() {
    let catalog = catalog();
    let now = Utc::now();
    for name in ["140.82.112.3", "169.254.169.254", "[::1]"] {
        let info = catalog.info(&host(name), now);
        assert_eq!(info.catalog_id, None, "{name}");
        assert_eq!(info.apex, None, "{name} has no apex");
        assert_eq!(info.popularity_rank, None, "{name} has no rank");
    }
}

#[test]
fn the_catalog_carries_no_verdict() {
    // Der Katalog darf keine Wertung tragen. Ein Feld wie `trust` oder `safe`
    // wäre eine Behauptung, die niemand belegen kann, und es gibt keines:
    // die Struktur hat nur Kennung, Name, Muster, Kategorie, Texte, Symbol,
    // Startseite, Quelle und den Hinweis. Dieser Test hält die Absicht fest,
    // indem er die Datei nach den Wörtern durchsucht.
    let text = std::fs::read_to_string(bundled_dir().join("domains.yaml")).unwrap();
    for word in ["trust:", "safe:", "score:", "reputation:", "verdict:"] {
        assert!(
            !text.contains(word),
            "the catalog must not carry a verdict, found {word:?}"
        );
    }
}
