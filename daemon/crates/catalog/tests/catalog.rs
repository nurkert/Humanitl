//! Die Tests aus `backlog/sprint-2.md` (HUM-031) über einem kleinen,
//! selbst geschriebenen Katalog.
//!
//! Der ausgelieferte Katalog wird in `bundled.rs` geprüft; hier steht das
//! Verhalten, unabhängig vom Inhalt der Datei.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::Write as _;
use std::path::Path;

use chrono::{DateTime, TimeZone, Utc};
use humanitl_catalog::{Catalog, DOMAINS_FILE, DomainInfo, RANKS_FILE, Ranks};
use humanitl_core::{HostName, Severity};

const SAMPLE: &str = r#"
version: 1
entries:
  - id: github
    name: GitHub
    hosts: ["github.com", "**.github.com", "**.githubusercontent.com"]
    category: scm
    description:
      en: "Source hosting."
      de: "Quellcode-Hosting."
    typical: ["git clone"]
    icon: scm.svg
    homepage: https://github.com
    source: https://docs.github.com/en/rest
  - id: crates-io
    name: crates.io
    hosts: ["crates.io", "static.crates.io"]
    category: registry
    description:
      en: "Rust packages."
      de: "Rust-Pakete."
    typical: ["cargo build"]
    icon: registry.svg
    homepage: https://crates.io
    source: https://doc.rust-lang.org/cargo/
"#;

const RANKS: &str = "1,google.com\n42,github.com\n9001,crates.io\n";

fn host(text: &str) -> HostName {
    HostName::parse(text).unwrap()
}

fn at(second: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 9, 3, 12, 0, second).unwrap()
}

/// Ein Verzeichnis mit beiden Dateien, wie es der Daemon vorfindet.
fn bundle(dir: &Path, catalog: Option<&str>, ranks: Option<&str>) {
    if let Some(catalog) = catalog {
        std::fs::write(dir.join(DOMAINS_FILE), catalog).unwrap();
    }
    if let Some(ranks) = ranks {
        let file = std::fs::File::create(dir.join(RANKS_FILE)).unwrap();
        let mut encoder = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
        encoder.write_all(ranks.as_bytes()).unwrap();
        encoder.finish().unwrap();
    }
}

fn loaded() -> Catalog {
    let dir = tempfile::tempdir().unwrap();
    bundle(dir.path(), Some(SAMPLE), Some(RANKS));
    Catalog::load(dir.path()).unwrap()
}

#[test]
fn catalog_pattern_match() {
    let catalog = loaded();

    assert_eq!(
        catalog.info(&host("api.github.com"), at(1)).catalog_id,
        Some("github".to_owned())
    );
    assert_eq!(
        catalog
            .info(&host("raw.githubusercontent.com"), at(1))
            .catalog_id,
        Some("github".to_owned())
    );
    assert_eq!(
        catalog.info(&host("static.crates.io"), at(1)).catalog_id,
        Some("crates-io".to_owned())
    );

    // Der Fehltreffer, um den es geht: ein Name, der wie GitHub aussieht.
    assert_eq!(
        catalog.info(&host("evil-github.com"), at(1)).catalog_id,
        None
    );
    assert_eq!(
        catalog.info(&host("github.com.evil.io"), at(1)).catalog_id,
        None
    );
    // `crates.io` steht exakt in der Datei; eine Unterdomain, die nicht dort
    // steht, trifft nicht.
    assert_eq!(
        catalog.info(&host("evil.crates.io"), at(1)).catalog_id,
        None
    );
}

#[test]
fn apex_psl() {
    let catalog = loaded();

    assert_eq!(
        catalog.info(&host("api.github.com"), at(1)).apex.as_deref(),
        Some("github.com")
    );
    assert_eq!(
        catalog.info(&host("a.b.github.io"), at(1)).apex.as_deref(),
        Some("b.github.io"),
        "github.io is in the private section of the list"
    );
    assert_eq!(catalog.info(&host("140.82.112.3"), at(1)).apex, None);
}

#[test]
fn rank_lookup_on_apex() {
    let catalog = loaded();

    // Der Rang gehört zum Apex, nicht zum vollen Host: `api.github.com` steht
    // in keiner Rangliste, `github.com` schon.
    assert_eq!(
        catalog.info(&host("api.github.com"), at(1)).popularity_rank,
        Some(42)
    );
    assert_eq!(
        catalog.info(&host("github.com"), at(1)).popularity_rank,
        Some(42)
    );
    assert_eq!(
        catalog
            .info(&host("nowhere.example"), at(1))
            .popularity_rank,
        None
    );
    assert_eq!(
        catalog.info(&host("140.82.112.3"), at(1)).popularity_rank,
        None
    );
}

#[test]
fn seen_count_increments() {
    let catalog = loaded();

    let first = catalog.info(&host("api.github.com"), at(1));
    assert_eq!(first.seen_count, 1);
    assert_eq!(first.first_seen, Some(at(1)));

    let second = catalog.info(&host("api.github.com"), at(5));
    assert_eq!(second.seen_count, 2);
    assert_eq!(
        second.first_seen,
        Some(at(1)),
        "`first seen` stays where it was"
    );

    // Ein anderer Host ist ein anderer Zähler, auch unter demselben Eintrag.
    assert_eq!(catalog.info(&host("github.com"), at(6)).seen_count, 1);
    assert_eq!(catalog.seen().hosts(), 2);
}

#[test]
fn describe_does_not_count() {
    let catalog = loaded();

    let untouched = catalog.describe(&host("api.github.com"));
    assert_eq!(untouched.seen_count, 0);
    assert_eq!(untouched.first_seen, None);
    assert_eq!(untouched.catalog_id, Some("github".to_owned()));
    assert_eq!(catalog.seen().hosts(), 0);
}

#[test]
fn catalog_load_error_is_warning() {
    let dir = tempfile::tempdir().unwrap();
    // Kein `domains.yaml`, keine Rangliste: beide Befunde, beide als Warnung.
    let (catalog, diagnostics) = Catalog::load_or_empty(dir.path());

    let codes: Vec<&str> = diagnostics.iter().map(|d| d.code.as_str()).collect();
    assert_eq!(codes, vec!["CATALOG_001", "CATALOG_002"]);
    for diagnostic in &diagnostics {
        assert_eq!(diagnostic.severity, Severity::Warning);
        assert!(!diagnostic.why.is_empty(), "a finding names its cause");
    }

    // Der Apex kommt trotzdem: er hängt an der einkompilierten Liste, nicht an
    // der Datei.
    let info = catalog.info(&host("api.github.com"), at(1));
    assert_eq!(info.apex.as_deref(), Some("github.com"));
    assert_eq!(info.catalog_id, None, "unknown stays unknown");
    assert_eq!(info.popularity_rank, None);
    assert_eq!(info.seen_count, 1);
}

#[test]
fn a_broken_catalog_leaves_the_ranks_usable() {
    let dir = tempfile::tempdir().unwrap();
    bundle(
        dir.path(),
        Some("version: 1\nentries: [oops]\n"),
        Some(RANKS),
    );

    let (catalog, diagnostics) = Catalog::load_or_empty(dir.path());
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code.as_str(), "CATALOG_001");
    assert_eq!(catalog.entries().len(), 0);
    assert_eq!(catalog.rank("github.com"), Some(42));
}

#[test]
fn an_unknown_format_version_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    bundle(dir.path(), Some("version: 2\nentries: []\n"), Some(RANKS));

    let err = Catalog::load(dir.path()).unwrap_err();
    assert_eq!(err.code.as_str(), "CATALOG_001");
    assert!(err.why.contains("version 2"), "{}", err.why);
}

#[test]
fn a_duplicate_id_is_refused() {
    let doubled = SAMPLE.replace("id: crates-io", "id: github");
    let dir = tempfile::tempdir().unwrap();
    bundle(dir.path(), Some(&doubled), Some(RANKS));

    let err = Catalog::load(dir.path()).unwrap_err();
    assert_eq!(err.code.as_str(), "CATALOG_001");
    assert!(err.why.contains("twice"), "{}", err.why);
}

#[test]
fn an_entry_without_a_source_is_refused() {
    let without = SAMPLE.replace(
        "    source: https://docs.github.com/en/rest\n",
        "    source: \"\"\n",
    );
    let dir = tempfile::tempdir().unwrap();
    bundle(dir.path(), Some(&without), Some(RANKS));

    let err = Catalog::load(dir.path()).unwrap_err();
    assert_eq!(err.code.as_str(), "CATALOG_001");
    assert!(err.why.contains("source"), "{}", err.why);
}

#[test]
fn an_address_pattern_is_refused() {
    let address = SAMPLE.replace(
        r#"hosts: ["crates.io", "static.crates.io"]"#,
        r#"hosts: ["ip:140.82.112.3"]"#,
    );
    let dir = tempfile::tempdir().unwrap();
    bundle(dir.path(), Some(&address), Some(RANKS));

    let err = Catalog::load(dir.path()).unwrap_err();
    assert_eq!(err.code.as_str(), "CATALOG_001");
    assert!(err.why.contains("addresses"), "{}", err.why);
}

#[test]
fn the_first_matching_entry_wins() {
    let ordered = r#"
version: 1
entries:
  - id: first
    name: First
    hosts: ["**.example.com"]
    category: other
    description: { en: "First.", de: "Erster." }
    typical: ["curl"]
    icon: other.svg
    homepage: https://example.com
    source: https://example.com/docs
  - id: second
    name: Second
    hosts: ["api.example.com"]
    category: other
    description: { en: "Second.", de: "Zweiter." }
    typical: ["curl"]
    icon: other.svg
    homepage: https://example.com
    source: https://example.com/docs
"#;
    let catalog = Catalog::build(
        serde_yaml_ng::from_str::<humanitl_catalog::CatalogFile>(ordered)
            .unwrap()
            .entries,
        Ranks::empty(),
    )
    .unwrap();

    assert_eq!(
        catalog
            .lookup(&host("api.example.com"))
            .map(|e| e.id.as_str()),
        Some("first")
    );
}

#[test]
fn an_unknown_domain_is_only_an_apex() {
    let info = DomainInfo::unknown(&host("intern.beispiel.de"));
    assert_eq!(info.apex.as_deref(), Some("beispiel.de"));
    assert_eq!(info.catalog_id, None);
    assert_eq!(info.popularity_rank, None);
    assert_eq!(info.first_seen, None);
    assert_eq!(info.seen_count, 0);
}
