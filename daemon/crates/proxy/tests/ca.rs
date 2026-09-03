//! CA-Verwaltung und Env-Kit (HUM-014).
//!
//! Die CA entsteht einmal, wird danach Byte für Byte gleich zurückgelesen,
//! stellt Leafs aus, die rustls gegen sie prüft, hält den Leaf-Speicher in
//! seiner Grenze und setzt das Bundle mit sich selbst an erster Stelle
//! zusammen. Das Env-Kit im Code und in den ausgelieferten Profilen ist
//! dasselbe.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use humanitl_core::diagnostics::codes::{TLS_004, TLS_005};
use humanitl_core::{FixAction, HostName, SessionId};
use humanitl_proxy::ca::{
    BUNDLE_FILE, CERT_FILE, CERT_MODE, CaStore, DIR_MODE, ENV_KIT, ENV_KIT_SESSION_KEY, KEY_FILE,
    KEY_MODE, LeafCache, SANDBOX_CA_PATH, env_kit, read_system_bundle,
};
use rustls::pki_types::pem::PemObject as _;
use rustls::pki_types::{CertificateDer, UnixTime};
use tempfile::TempDir;

fn ca_dir() -> (TempDir, PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("humanitl").join("ca");
    (tmp, dir)
}

fn mode_of(path: &Path) -> u32 {
    fs::metadata(path).expect("metadata").permissions().mode() & 0o777
}

fn dns(host: &str) -> HostName {
    HostName::parse(host).expect("a valid host")
}

fn running_as_root() -> bool {
    fs::metadata("/proc/self").is_ok_and(|meta| meta.uid() == 0)
}

/// Die PEM-Blöcke einer Datei, als DER.
fn certs_in(pem: &[u8]) -> Vec<CertificateDer<'static>> {
    CertificateDer::pem_slice_iter(pem)
        .collect::<Result<Vec<_>, _>>()
        .expect("every block parses")
}

/// Ein selbstsigniertes Fremdzertifikat, wie es in einem System-Bundle steht.
fn foreign_root(name: &str) -> String {
    let key = rcgen::KeyPair::generate().expect("key");
    let mut params = rcgen::CertificateParams::new(vec![name.to_owned()]).expect("params");
    params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    params.self_signed(&key).expect("cert").pem()
}

fn profile_env(name: &str) -> BTreeMap<String, String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../profiles/sandbox")
        .join(format!("{name}.toml"));
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("{} is unreadable: {err}", path.display()));
    let table: toml::Table = text.parse().expect("the profile is TOML");
    table["env"]
        .as_table()
        .expect("[env] is a table")
        .iter()
        .map(|(key, value)| {
            (
                key.clone(),
                value.as_str().expect("env values are strings").to_owned(),
            )
        })
        .collect()
}

#[test]
fn ca_create_then_load_is_byte_identical() {
    let (_tmp, dir) = ca_dir();

    let first = CaStore::load_or_create(&dir).expect("first start creates the CA");
    assert!(first.was_created());
    let key_bytes = fs::read(dir.join(KEY_FILE)).expect("ca.key exists");
    let cert_bytes = fs::read(dir.join(CERT_FILE)).expect("ca.crt exists");
    assert_eq!(first.cert_pem().as_bytes(), cert_bytes.as_slice());
    assert_eq!(first.key_pem().as_bytes(), key_bytes.as_slice());
    assert!(first.cert_pem().starts_with("-----BEGIN CERTIFICATE-----"));
    assert!(first.key_pem().starts_with("-----BEGIN PRIVATE KEY-----"));

    let second = CaStore::load_or_create(&dir).expect("second start loads the CA");
    assert!(!second.was_created());
    assert_eq!(second.fingerprint_sha256(), first.fingerprint_sha256());
    assert_eq!(second.cert_pem(), first.cert_pem());
    assert_eq!(second.key_pem().as_str(), first.key_pem().as_str());
    assert_eq!(second.cert_der(), first.cert_der());
    assert_eq!(fs::read(dir.join(KEY_FILE)).unwrap(), key_bytes);
    assert_eq!(fs::read(dir.join(CERT_FILE)).unwrap(), cert_bytes);

    // Zwei Verzeichnisse sind zwei CAs.
    let (_other_tmp, other_dir) = ca_dir();
    let other = CaStore::load_or_create(&other_dir).unwrap();
    assert_ne!(other.fingerprint_sha256(), first.fingerprint_sha256());
}

#[test]
fn key_is_0600_cert_0644_and_dir_0700() {
    let (_tmp, dir) = ca_dir();
    let store = CaStore::load_or_create(&dir).unwrap();
    assert_eq!(mode_of(&dir), DIR_MODE);
    assert_eq!(mode_of(&store.key_path()), KEY_MODE);
    assert_eq!(mode_of(&store.cert_path()), CERT_MODE);
    assert_eq!(store.key_path(), dir.join("ca.key"));
    assert_eq!(store.cert_path(), dir.join("ca.crt"));
    assert_eq!(store.bundle_path(), dir.join("ca-bundle.crt"));
    assert_eq!(store.dir(), dir.as_path());

    // Keine temporäre Datei bleibt liegen.
    let leftovers: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains(".tmp-"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[test]
fn the_ca_never_leaves_its_directory() {
    // Kein Pfad des Moduls zeigt in einen Trust-Store des Hosts; die einzigen
    // Dateien liegen im CA-Verzeichnis.
    let (tmp, dir) = ca_dir();
    let store = CaStore::load_or_create(&dir).unwrap();
    store.write_bundle(None).unwrap();
    let mut names: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, [BUNDLE_FILE, CERT_FILE, KEY_FILE]);
    let outside: Vec<String> = fs::read_dir(tmp.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(outside, ["humanitl"]);
}

#[test]
fn fingerprint_is_colon_separated_sha256() {
    let (_tmp, dir) = ca_dir();
    let store = CaStore::load_or_create(&dir).unwrap();
    let fingerprint = store.fingerprint_sha256();
    assert_eq!(fingerprint.len(), 32 * 3 - 1, "{fingerprint}");
    let pairs: Vec<&str> = fingerprint.split(':').collect();
    assert_eq!(pairs.len(), 32);
    for pair in pairs {
        assert_eq!(pair.len(), 2);
        assert!(
            pair.chars()
                .all(|c| c.is_ascii_digit() || ('A'..='F').contains(&c)),
            "{pair}"
        );
    }
}

#[test]
fn debug_output_never_shows_the_key() {
    let (_tmp, dir) = ca_dir();
    let store = CaStore::load_or_create(&dir).unwrap();
    let text = format!("{store:?}");
    assert!(!text.contains("PRIVATE KEY"), "{text}");
    assert!(!text.contains(store.key_pem().as_str()));
    assert!(text.contains("fingerprint_sha256"));

    let leaf = store.issue_leaf(&dns("example.com")).unwrap();
    let text = format!("{leaf:?}");
    assert!(text.contains("example.com"));
    assert!(text.contains("[elided]"), "{text}");

    let cache = LeafCache::new(Arc::new(store), 4);
    assert!(format!("{cache:?}").contains("capacity: 4"));
}

#[test]
fn leaf_for_example_com_validates_against_the_ca() {
    let (_tmp, dir) = ca_dir();
    let store = CaStore::load_or_create(&dir).unwrap();
    let host = dns("example.com");
    let leaf = store.issue_leaf(&host).unwrap();
    assert_eq!(leaf.host, host);

    store
        .verify_leaf(&leaf.cert, &host, UnixTime::now())
        .expect("the leaf chains to the CA and names example.com");

    // Für einen anderen Host gilt es nicht.
    let err = store
        .verify_leaf(&leaf.cert, &dns("example.org"), UnixTime::now())
        .expect_err("a leaf for example.com is no leaf for example.org");
    assert_eq!(err.code, TLS_005);
    assert!(err.why.contains("example.org"), "{}", err.why);

    // Eine fremde CA erkennt es nicht an.
    let (_other_tmp, other_dir) = ca_dir();
    let other = CaStore::load_or_create(&other_dir).unwrap();
    other
        .verify_leaf(&leaf.cert, &host, UnixTime::now())
        .expect_err("another CA does not vouch for this leaf");

    // Jeder Aufruf ist ein frisches Zertifikat mit frischem Schlüssel.
    let again = store.issue_leaf(&host).unwrap();
    assert_ne!(again.cert, leaf.cert);
    assert_ne!(again.key, leaf.key);
}

#[test]
fn leaf_is_backdated_one_day_and_valid_one_year() {
    let (_tmp, dir) = ca_dir();
    let store = CaStore::load_or_create(&dir).unwrap();
    let host = dns("example.com");
    let leaf = store.issue_leaf(&host).unwrap();
    let now = UnixTime::now().as_secs();
    let at = |offset: i64| {
        let secs = u64::try_from(i64::try_from(now).unwrap() + offset).unwrap();
        UnixTime::since_unix_epoch(Duration::from_secs(secs))
    };

    let day = 24 * 3600;
    store
        .verify_leaf(&leaf.cert, &host, at(-12 * 3600))
        .expect("valid twelve hours ago (clock skew)");
    store
        .verify_leaf(&leaf.cert, &host, at(-2 * day))
        .expect_err("not valid two days ago");
    store
        .verify_leaf(&leaf.cert, &host, at(364 * day))
        .expect("valid in 364 days");
    store
        .verify_leaf(&leaf.cert, &host, at(366 * day))
        .expect_err("not valid in 366 days");
}

#[test]
fn leaf_for_an_ip_literal_carries_an_ip_san() {
    let (_tmp, dir) = ca_dir();
    let store = CaStore::load_or_create(&dir).unwrap();
    for literal in ["192.168.1.50", "[::1]"] {
        let host = dns(literal);
        assert!(host.as_ip().is_some(), "{literal} is an IP host");
        let leaf = store.issue_leaf(&host).unwrap();
        store
            .verify_leaf(&leaf.cert, &host, UnixTime::now())
            .unwrap_or_else(|err| panic!("{literal}: {err}"));
        store
            .verify_leaf(&leaf.cert, &dns("192.168.1.51"), UnixTime::now())
            .expect_err("another address does not match");
    }
}

#[test]
fn leaf_config_offers_only_http1() {
    let (_tmp, dir) = ca_dir();
    let store = CaStore::load_or_create(&dir).unwrap();
    let leaf = store.issue_leaf(&dns("example.com")).unwrap();
    let config = leaf.server_config(store.provider()).unwrap();
    assert_eq!(config.alpn_protocols, vec![b"http/1.1".to_vec()]);
}

#[test]
fn leaf_cache_is_bounded_and_evicts_the_least_recently_used() {
    let (_tmp, dir) = ca_dir();
    let store = Arc::new(CaStore::load_or_create(&dir).unwrap());
    let cache = LeafCache::new(Arc::clone(&store), 3);
    assert_eq!(cache.capacity(), 3);
    assert!(cache.is_empty());

    let a = dns("a.example");
    let b = dns("b.example");
    let c = dns("c.example");
    let d = dns("d.example");

    let first_a = cache.server_config(&a).unwrap();
    cache.server_config(&b).unwrap();
    cache.server_config(&c).unwrap();
    assert_eq!(cache.len(), 3);

    // Ein Treffer liefert dieselbe Konfiguration und frischt den Eintrag auf.
    let again_a = cache.server_config(&a).unwrap();
    assert!(
        Arc::ptr_eq(&first_a, &again_a),
        "a hit returns the cached Arc"
    );

    // Der vierte Host verdrängt den am längsten unbenutzten: b, nicht a.
    cache.server_config(&d).unwrap();
    assert_eq!(cache.len(), 3);
    assert!(cache.contains(&a));
    assert!(!cache.contains(&b), "b was the least recently used");
    assert!(cache.contains(&c));
    assert!(cache.contains(&d));

    // b kommt neu, jetzt fliegt c.
    let new_b = cache.server_config(&b).unwrap();
    assert_eq!(cache.len(), 3);
    assert!(!cache.contains(&c));
    assert!(Arc::ptr_eq(&new_b, &cache.server_config(&b).unwrap()));

    // Viele Hosts: die Grenze hält.
    for i in 0..50 {
        cache.server_config(&dns(&format!("h{i}.example"))).unwrap();
        assert!(cache.len() <= 3);
    }
    assert_eq!(cache.len(), 3);

    // Eine Kapazität von 0 wird zu 1: ein Eintrag, nie keiner.
    let tiny = LeafCache::new(store, 0);
    assert_eq!(tiny.capacity(), 1);
    tiny.server_config(&a).unwrap();
    tiny.server_config(&b).unwrap();
    assert_eq!(tiny.len(), 1);
    assert!(tiny.contains(&b));
}

#[test]
fn bundle_puts_our_ca_first_then_the_system_roots() {
    let (_tmp, dir) = ca_dir();
    let store = CaStore::load_or_create(&dir).unwrap();

    let roots = [
        foreign_root("root-one.test"),
        foreign_root("root-two.test"),
        foreign_root("root-three.test"),
    ];
    let system_pem = format!(
        "# Label: \"Root One\"\n{}\n\n{}garbage between blocks\n{}",
        roots[0], roots[1], roots[2]
    );

    let (text, count) = store.render_bundle(system_pem.as_bytes());
    assert_eq!(count, 3);
    assert!(
        text.starts_with(store.cert_pem().trim_end()),
        "our CA is the first block"
    );
    assert!(!text.contains("garbage"), "only PEM blocks survive");
    assert!(!text.contains("# Label"), "comments are dropped");

    let ders = certs_in(text.as_bytes());
    assert_eq!(ders.len(), 4);
    assert_eq!(&ders[0], store.cert_der());
    for (der, root) in ders[1..].iter().zip(&roots) {
        assert_eq!(
            der,
            &certs_in(root.as_bytes())[0],
            "system roots keep their order"
        );
    }

    let bundle = store.write_bundle(Some(system_pem.as_bytes())).unwrap();
    assert_eq!(bundle.path, dir.join(BUNDLE_FILE));
    assert_eq!(bundle.system_certs, 3);
    assert_eq!(bundle.system_source, None);
    assert_eq!(mode_of(&bundle.path), CERT_MODE);
    assert_eq!(fs::read_to_string(&bundle.path).unwrap(), text);

    // Ein neuer Aufruf ersetzt das Bundle, statt anzuhängen.
    let bundle = store.write_bundle(None).unwrap();
    assert_eq!(bundle.system_certs, 0);
    let only_ours = fs::read_to_string(&bundle.path).unwrap();
    assert_eq!(
        certs_in(only_ours.as_bytes()),
        vec![store.cert_der().clone()]
    );
}

#[test]
fn bundle_with_the_hosts_roots_has_our_ca_first() {
    let Some((source, system_pem)) = read_system_bundle() else {
        eprintln!("no system CA bundle on this host; skipping");
        return;
    };
    let (_tmp, dir) = ca_dir();
    let store = CaStore::load_or_create(&dir).unwrap();
    let bundle = store.write_bundle_with_system_roots().unwrap();
    assert_eq!(bundle.system_source, Some(source));
    assert!(
        bundle.system_certs > 100,
        "a distribution bundle has well over 100 roots, found {}",
        bundle.system_certs
    );
    assert_eq!(bundle.system_certs, certs_in(&system_pem).len());
    let ders = certs_in(&fs::read(&bundle.path).unwrap());
    assert_eq!(&ders[0], store.cert_der());
    assert_eq!(ders.len(), bundle.system_certs + 1);
}

#[test]
fn a_corrupt_key_is_tls_005_with_a_remove_fix() {
    let (_tmp, dir) = ca_dir();
    let store = CaStore::load_or_create(&dir).unwrap();
    fs::write(
        store.key_path(),
        "-----BEGIN PRIVATE KEY-----\nnope\n-----END PRIVATE KEY-----\n",
    )
    .unwrap();

    let err = CaStore::load_or_create(&dir).expect_err("garbage is no key");
    assert_eq!(err.code, TLS_005);
    assert_eq!(err.title, "CA-Dateien unbrauchbar");
    assert!(err.why.contains("ca.key"), "{}", err.why);
    let Some(FixAction::CopyCommand(command)) = err.fix else {
        panic!("a remove command is the fix: {err:?}");
    };
    assert_eq!(command, format!("rm -r {}", dir.display()));
}

#[test]
fn a_corrupt_certificate_is_tls_005() {
    let (_tmp, dir) = ca_dir();
    let store = CaStore::load_or_create(&dir).unwrap();
    fs::write(store.cert_path(), "not a certificate\n").unwrap();
    let err = CaStore::load_or_create(&dir).expect_err("garbage is no certificate");
    assert_eq!(err.code, TLS_005);
    assert!(err.why.contains("ca.crt"), "{}", err.why);

    fs::write(store.cert_path(), [0u8, 159, 146, 150]).unwrap();
    let err = CaStore::load_or_create(&dir).expect_err("binary garbage is no certificate");
    assert_eq!(err.code, TLS_005);
}

#[test]
fn one_file_without_the_other_is_tls_005_not_a_silent_recreate() {
    for missing in [KEY_FILE, CERT_FILE] {
        let (_tmp, dir) = ca_dir();
        let store = CaStore::load_or_create(&dir).unwrap();
        fs::remove_file(dir.join(missing)).unwrap();
        let err = CaStore::load_or_create(&dir).expect_err("half a CA is no CA");
        assert_eq!(err.code, TLS_005);
        assert!(err.why.contains(missing), "{}", err.why);
        assert!(err.why.contains("is missing"), "{}", err.why);
        assert_eq!(store.cert_path().exists(), missing == KEY_FILE);
    }
}

#[test]
fn a_key_readable_by_others_is_refused_with_tls_005() {
    let (_tmp, dir) = ca_dir();
    let store = CaStore::load_or_create(&dir).unwrap();
    for mode in [0o640, 0o604, 0o644, 0o660] {
        fs::set_permissions(store.key_path(), fs::Permissions::from_mode(mode)).unwrap();
        let err = CaStore::load_or_create(&dir).expect_err("a readable key is burnt");
        assert_eq!(err.code, TLS_005, "mode {mode:04o}");
        assert!(err.why.contains(&format!("{mode:04o}")), "{}", err.why);
        assert!(err.why.contains("0600"), "{}", err.why);
    }
    // 0400 ist strenger als 0600 und bleibt erlaubt.
    fs::set_permissions(store.key_path(), fs::Permissions::from_mode(0o400)).unwrap();
    CaStore::load_or_create(&dir).expect("a read-only key for the owner is fine");
}

#[test]
fn a_key_that_does_not_match_the_certificate_is_refused() {
    let (_tmp_a, dir_a) = ca_dir();
    let (_tmp_b, dir_b) = ca_dir();
    let a = CaStore::load_or_create(&dir_a).unwrap();
    let b = CaStore::load_or_create(&dir_b).unwrap();

    // B's Schlüssel unter A's Zertifikat: beides für sich gültig, zusammen nicht.
    fs::write(a.key_path(), b.key_pem().as_bytes()).unwrap();
    let err = CaStore::load_or_create(&dir_a).expect_err("the self-check catches the mismatch");
    assert_eq!(err.code, TLS_005);
    assert!(err.why.contains("do not belong together"), "{}", err.why);
}

#[test]
fn an_unwritable_parent_is_tls_004_with_a_mkdir_fix() {
    if running_as_root() {
        eprintln!("root ignores directory permissions; skipping");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let locked = tmp.path().join("locked");
    fs::create_dir(&locked).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o500)).unwrap();
    let dir = locked.join("ca");

    let err = CaStore::load_or_create(&dir).expect_err("cannot create below a read-only parent");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o700)).unwrap();

    assert_eq!(err.code, TLS_004);
    assert_eq!(err.title, "CA-Verzeichnis nicht beschreibbar");
    assert!(err.why.contains(&dir.display().to_string()), "{}", err.why);
    let Some(FixAction::CopyCommand(command)) = err.fix else {
        panic!("a mkdir command is the fix: {err:?}");
    };
    assert_eq!(
        command,
        format!("mkdir -p {d} && chmod 700 {d}", d = dir.display())
    );
}

#[test]
fn an_existing_directory_open_to_others_is_tightened() {
    let (_tmp, dir) = ca_dir();
    fs::create_dir_all(&dir).unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    CaStore::load_or_create(&dir).unwrap();
    assert_eq!(mode_of(&dir), DIR_MODE);
}

#[test]
fn envkit_complete_and_identical_in_the_shipped_profiles() {
    let kit: BTreeMap<&str, &str> = ENV_KIT.iter().copied().collect();
    assert_eq!(kit.len(), ENV_KIT.len(), "no key twice");

    // Jede Zeile der Tabelle aus HUM-014, die im MVP gesetzt wird.
    for key in [
        "HTTP_PROXY",
        "http_proxy",
        "HTTPS_PROXY",
        "https_proxy",
        "ALL_PROXY",
        "NO_PROXY",
        "no_proxy",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
        "CURL_CA_BUNDLE",
        "REQUESTS_CA_BUNDLE",
        "PIP_CERT",
        "NODE_EXTRA_CA_CERTS",
        "NPM_CONFIG_CAFILE",
        "DENO_CERT",
        "GIT_SSL_CAINFO",
        "CARGO_HTTP_CAINFO",
        "NIX_SSL_CERT_FILE",
        "AWS_CA_BUNDLE",
        "HUMANITL",
    ] {
        assert!(kit.contains_key(key), "{key} is missing from ENV_KIT");
    }
    assert_eq!(kit["NO_PROXY"], "");
    assert_eq!(kit["no_proxy"], "");
    assert_eq!(kit["SSL_CERT_FILE"], SANDBOX_CA_PATH);
    assert_eq!(kit["HUMANITL"], "1");
    assert!(!kit.contains_key("GOFLAGS"), "Go reads SSL_CERT_FILE");
    assert!(
        !kit.contains_key("JAVA_TOOL_OPTIONS"),
        "no truststore in the MVP, so no JAVA_TOOL_OPTIONS"
    );

    // Acceptance: `env | grep -c -E 'PROXY|CA|CERT'` >= 16 in the sandbox.
    let matching = kit
        .keys()
        .filter(|key| key.contains("PROXY") || key.contains("CA") || key.contains("CERT"))
        .count();
    assert!(matching >= 16, "{matching} keys match PROXY|CA|CERT");

    for name in ["default", "test"] {
        let env = profile_env(name);
        for (key, value) in ENV_KIT {
            assert_eq!(
                env.get(*key).map(String::as_str),
                Some(*value),
                "profile {name}: {key}"
            );
        }
        for (key, value) in &env {
            assert!(
                !value.contains("/.local/share")
                    && !value.contains("XDG")
                    && !value.contains("/run/user")
                    && (!value.starts_with("/home/") || value == "/home/agent"),
                "profile {name}: {key}={value} leaks a host path"
            );
        }
    }
}

#[test]
fn env_kit_adds_the_session() {
    let session = SessionId::new();
    let kit = env_kit(session);
    assert_eq!(kit.len(), ENV_KIT.len() + 1);
    for (key, value) in ENV_KIT {
        assert!(
            kit.contains(&((*key).to_owned(), (*value).to_owned())),
            "{key}"
        );
    }
    assert_eq!(
        kit.last(),
        Some(&(ENV_KIT_SESSION_KEY.to_owned(), session.to_string()))
    );
    assert_eq!(ENV_KIT_SESSION_KEY, "HUMANITL_SESSION");
}

/// Ein Symlink an der Stelle des Schlüssels wird abgelehnt, nicht verfolgt.
///
/// Ohne diese Prüfung liefe die Rechteprüfung gegen das Ziel des Symlinks,
/// gelesen und geschrieben würde aber ebenfalls dort: Wer den Pfad in ein
/// beschreibbares Verzeichnis umlenkt, bekäme den Schlüssel dorthin.
#[test]
fn a_symlinked_key_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let ca_dir = dir.path().join("ca");
    std::fs::create_dir_all(&ca_dir).unwrap();
    let elsewhere = dir.path().join("elsewhere.key");
    std::fs::write(&elsewhere, "not a key").unwrap();
    std::fs::set_permissions(&elsewhere, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::os::unix::fs::symlink(&elsewhere, ca_dir.join("ca.key")).unwrap();
    std::fs::write(ca_dir.join("ca.crt"), "not a certificate").unwrap();

    let err = CaStore::load_or_create(&ca_dir).expect_err("a symlinked key must be refused");
    assert_eq!(err.code.as_str(), "TLS_005");
    assert!(
        err.why.contains("not a regular file"),
        "the reason must name the symlink: {}",
        err.why
    );
}
