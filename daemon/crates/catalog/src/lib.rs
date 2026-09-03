//! Domain-Katalog, Public Suffix List und Reputationsdaten.
//!
//! Diese Crate beantwortet die Frage, die im Domain-Panel rechts neben einer
//! angehaltenen Anfrage steht: *Wohin geht das hier eigentlich?* Sie beantwortet
//! sie aus mitgelieferten Daten und aus nichts sonst. Zur Laufzeit wird nie
//! etwas geholt, weder ein Favicon noch ein Titel noch eine Rangliste
//! (ADR-006); die Crate kennt weder einen HTTP-Client noch einen Resolver, und
//! `tools/check-deps.sh` hält sie dabei.
//!
//! Was sie liefert, steht in [`DomainInfo`]:
//!
//! - der **Apex** aus der einkompilierten Public Suffix List ([`psl`]),
//! - der **Katalogeintrag**, wenn eines der Host-Muster aus
//!   `catalog/domains.yaml` den Host trifft ([`entry`], [`pattern`]),
//! - der **Verbreitungsrang** des Apex aus der gebündelten Liste ([`ranks`]),
//! - **wann und wie oft** der Host in dieser Sitzung vorkam ([`store`]).
//!
//! # Was der Katalog nicht sagt
//!
//! Jedes Feld von [`DomainInfo`] ist eine Beobachtung, keine Bewertung. Es gibt
//! in dieser Crate kein „vertrauenswürdig", kein „sicher" und keine Punktzahl,
//! aus der die Oberfläche eines bauen könnte:
//!
//! - Ein Treffer im Katalog heißt: *dieser Name gehört zu diesem Dienst, und
//!   hier steht die Quelle, unter der das nachlesbar ist.* Er heißt nicht, dass
//!   die Anfrage in Ordnung ist. Ein Datenabfluss nach `api.github.com` sieht
//!   genauso aus wie ein `git push` (BACKLOG.md 4.3).
//! - Kein Treffer heißt: *unbekannt*. Nicht „neu", nicht „verdächtig", nicht
//!   „vermutlich harmlos". `catalog_id` ist dann `None`, und die Oberfläche
//!   zeigt die Unbekannt-Karte, statt eine leere Zeile, hinter der man Grün
//!   vermuten könnte (`backlog/CONVENTIONS.md` 4.13).
//! - Der Verbreitungsrang ist Reichweite, nicht Vertrauen. Siehe [`ranks`].
//!
//! # Fehlt eine Datei
//!
//! [`Catalog::load_or_empty`] ist der Weg, den der Daemon nimmt: Fehlt oder
//! bricht eine der beiden Dateien, läuft er mit dem weiter, was da ist, und
//! reicht [`humanitl_core::Diagnostic`]s mit `CATALOG_001` beziehungsweise
//! `CATALOG_002` als Warnung durch. Ohne Katalog ist dann jede Domain
//! unbekannt, ohne Rangliste ist jeder Rang unbekannt. Beides ist ehrlich; ein
//! stiller Fallback auf „alles bekannt" wäre es nicht.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod entry;
pub mod pattern;
pub mod psl;
pub mod ranks;
pub mod store;

use std::collections::BTreeSet;
use std::path::Path;

use chrono::{DateTime, Utc};
use humanitl_core::diagnostics::codes::CATALOG_001;
use humanitl_core::rule::HostPattern;
use humanitl_core::{Diagnostic, HostName, Severity};

pub use crate::entry::{CatalogEntry, CatalogFile, Category, FORMAT_VERSION, Text};
pub use crate::ranks::Ranks;
pub use crate::store::{SeenStats, SeenStore};

/// Dateiname des Katalogs im Datenverzeichnis.
pub const DOMAINS_FILE: &str = "domains.yaml";

/// Dateiname der Rangliste im Datenverzeichnis.
pub const RANKS_FILE: &str = "ranks-top100k.csv.gz";

/// Dateiname des Lizenzhinweises zur Rangliste.
///
/// Die Datei steht neben den Daten und nennt Herkunft, Datum, Prüfsummen und
/// Lizenz der ausgelieferten Liste. Die Oberfläche zeigt denselben Hinweis
/// unter dem ARB-Schlüssel `aboutRanks`.
pub const RANKS_LICENSE_FILE: &str = "RANKS-LICENSE";

/// Was der Katalog über den Ziel-Host einer Anfrage weiß.
///
/// Jedes `None` heißt „unbekannt" und nie „unbedenklich". Die Oberfläche
/// schreibt es auch so hin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainInfo {
    /// Die registrierbare Domain laut Public Suffix List.
    ///
    /// `None` bei einer IP-Adresse und bei einem Namen, dessen Suffix die Liste
    /// nicht kennt.
    pub apex: Option<String>,
    /// Die `id` des Katalogeintrags, dessen Muster den Host trifft.
    ///
    /// `None` heißt: nicht im Katalog. Die Texte zur `id` stehen in
    /// `catalog/domains.yaml`; über das Ereignis reist nur die Kennung.
    pub catalog_id: Option<String>,
    /// Der Verbreitungsrang des Apex, nicht des vollen Hosts.
    ///
    /// `None` heißt: steht nicht in den vorderen 100 000 Rängen der
    /// gebündelten Liste, oder es ist keine Liste geladen. Der Rang ist
    /// Reichweite, kein Urteil (siehe [`ranks`]).
    pub popularity_rank: Option<u32>,
    /// Wann dieser Host in dieser Sitzung zum ersten Mal vorkam.
    pub first_seen: Option<DateTime<Utc>>,
    /// Wie oft dieser Host in dieser Sitzung vorkam, diese Anfrage
    /// eingeschlossen. Nach [`Catalog::info`] immer mindestens 1.
    pub seen_count: u32,
}

impl DomainInfo {
    /// Was der Katalog über einen Host sagt, den er nicht kennt.
    ///
    /// Nur der Apex steht darin; alles andere ist unbekannt. Genau das liefert
    /// ein leerer Katalog, und genau so soll es im Panel aussehen.
    #[must_use]
    pub fn unknown(host: &HostName) -> Self {
        Self {
            apex: psl::apex(host),
            catalog_id: None,
            popularity_rank: None,
            first_seen: None,
            seen_count: 0,
        }
    }
}

/// Der gebündelte Katalog samt Rangliste und Sitzungszählern.
///
/// Ein Wert, den der Daemon einmal beim Start baut und danach nur noch liest;
/// die Zähler nehmen Beobachtungen über `&self` entgegen ([`SeenStore`]).
#[derive(Debug)]
pub struct Catalog {
    entries: Vec<CatalogEntry>,
    /// Muster in der Reihenfolge der Datei, mit dem Index ihres Eintrags. Bei
    /// zwei Treffern gewinnt das erste Muster, wie bei den Regeln.
    patterns: Vec<(HostPattern, usize)>,
    ranks: Ranks,
    seen: SeenStore,
}

impl Default for Catalog {
    fn default() -> Self {
        Self::empty()
    }
}

impl Catalog {
    /// Ein Katalog ohne Einträge und ohne Ränge.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
            patterns: Vec::new(),
            ranks: Ranks::empty(),
            seen: SeenStore::new(),
        }
    }

    /// Lädt Katalog und Rangliste aus einem Verzeichnis.
    ///
    /// Erwartet `<dir>/domains.yaml` und `<dir>/ranks-top100k.csv.gz`.
    ///
    /// # Errors
    ///
    /// Das erste [`Diagnostic`], das beim Laden anfällt: `CATALOG_001` für den
    /// Katalog, `CATALOG_002` für die Rangliste. Wer stattdessen weiterlaufen
    /// will, nimmt [`Catalog::load_or_empty`].
    pub fn load(dir: &Path) -> Result<Self, Diagnostic> {
        let entries = read_entries(&dir.join(DOMAINS_FILE))?;
        let ranks = Ranks::load(&dir.join(RANKS_FILE))?;
        Self::build(entries, ranks)
    }

    /// Lädt, was sich laden lässt, und meldet den Rest als Warnung.
    ///
    /// Das ist der Weg des Daemons: Ein fehlender Katalog ist kein Grund, die
    /// Sitzung nicht zu starten. Er ist ein Grund, jede Domain als unbekannt zu
    /// zeigen und den Befund sichtbar zu machen.
    #[must_use]
    pub fn load_or_empty(dir: &Path) -> (Self, Vec<Diagnostic>) {
        let mut diagnostics = Vec::new();

        let entries = match read_entries(&dir.join(DOMAINS_FILE)) {
            Ok(entries) => entries,
            Err(err) => {
                diagnostics.push(err);
                Vec::new()
            }
        };
        let ranks = match Ranks::load(&dir.join(RANKS_FILE)) {
            Ok(ranks) => ranks,
            Err(err) => {
                diagnostics.push(err);
                Ranks::empty()
            }
        };

        match Self::build(entries, ranks) {
            Ok(catalog) => (catalog, diagnostics),
            Err(err) => {
                diagnostics.push(err);
                (Self::empty(), diagnostics)
            }
        }
    }

    /// Baut einen Katalog aus schon eingelesenen Werten.
    ///
    /// # Errors
    ///
    /// `CATALOG_001`, wenn eine `id` doppelt vorkommt, ein Eintrag kein
    /// Host-Muster hat oder ein Muster kein Name und kein Label-Glob ist.
    pub fn build(entries: Vec<CatalogEntry>, ranks: Ranks) -> Result<Self, Diagnostic> {
        let patterns = compile(&entries)?;
        Ok(Self {
            entries,
            patterns,
            ranks,
            seen: SeenStore::new(),
        })
    }

    /// Verbucht eine Beobachtung des Hosts und beschreibt ihn.
    ///
    /// Genau einmal je eingetroffener Anfrage aufrufen: Der Zähler in
    /// [`DomainInfo::seen_count`] zählt Aufrufe dieser Funktion. Wer nur lesen
    /// will, ohne zu zählen, nimmt [`Catalog::describe`].
    ///
    /// `at` ist der Zeitpunkt der Anfrage. Der Katalog liest keine Uhr; er
    /// bekommt die Zeit gereicht, wie der Zustandsautomat auch
    /// (`backlog/CONVENTIONS.md` 4.11).
    #[must_use]
    pub fn info(&self, host: &HostName, at: DateTime<Utc>) -> DomainInfo {
        let stats = self.seen.observe(host, at);
        self.assemble(host, Some(stats))
    }

    /// Beschreibt den Host, ohne zu zählen.
    #[must_use]
    pub fn describe(&self, host: &HostName) -> DomainInfo {
        let stats = self.seen.peek(host);
        self.assemble(host, stats)
    }

    fn assemble(&self, host: &HostName, stats: Option<SeenStats>) -> DomainInfo {
        let apex = psl::apex(host);
        DomainInfo {
            catalog_id: self.lookup(host).map(|entry| entry.id.clone()),
            popularity_rank: apex.as_deref().and_then(|apex| self.ranks.rank(apex)),
            apex,
            first_seen: stats.map(|stats| stats.first_seen),
            seen_count: stats.map_or(0, |stats| stats.count),
        }
    }

    /// Der Eintrag, dessen Muster den Host trifft; der erste in Dateireihenfolge.
    #[must_use]
    pub fn lookup(&self, host: &HostName) -> Option<&CatalogEntry> {
        self.patterns
            .iter()
            .find(|(candidate, _)| pattern::matches(candidate, host))
            .and_then(|(_, index)| self.entries.get(*index))
    }

    /// Der Eintrag mit dieser `id`.
    ///
    /// Der Weg von einem `DomainInfo.catalog_id` zurück zu den Texten.
    #[must_use]
    pub fn entry(&self, id: &str) -> Option<&CatalogEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    /// Alle Einträge, in der Reihenfolge der Datei.
    #[must_use]
    pub fn entries(&self) -> &[CatalogEntry] {
        &self.entries
    }

    /// Der Rang eines Apex aus der gebündelten Liste.
    #[must_use]
    pub fn rank(&self, apex: &str) -> Option<u32> {
        self.ranks.rank(apex)
    }

    /// Wie viele Namen die geladene Rangliste kennt. `0` heißt: keine Liste.
    #[must_use]
    pub fn ranked_domains(&self) -> usize {
        self.ranks.len()
    }

    /// Die Sitzungszähler, für die Zusammenfassung ohne ausgewählten Flow.
    #[must_use]
    pub fn seen(&self) -> &SeenStore {
        &self.seen
    }
}

/// Liest `domains.yaml` und prüft die Form.
fn read_entries(path: &Path) -> Result<Vec<CatalogEntry>, Diagnostic> {
    let text = std::fs::read_to_string(path).map_err(|err| {
        Diagnostic::builder(CATALOG_001, Severity::Warning)
            .why(format!(
                "the catalog {} could not be read: {err}",
                path.display()
            ))
            .build()
    })?;
    let file: CatalogFile = serde_yaml_ng::from_str(&text).map_err(|err| {
        Diagnostic::builder(CATALOG_001, Severity::Warning)
            .why(format!(
                "the catalog {} is not valid: {err}",
                path.display()
            ))
            .build()
    })?;
    if file.version != FORMAT_VERSION {
        return Err(Diagnostic::builder(CATALOG_001, Severity::Warning)
            .why(format!(
                "the catalog {} has version {}, this build reads version {FORMAT_VERSION}",
                path.display(),
                file.version
            ))
            .build());
    }
    validate(&file.entries, path)?;
    Ok(file.entries)
}

/// Prüft die Werte, die das Schema allein nicht prüfen kann.
fn validate(entries: &[CatalogEntry], path: &Path) -> Result<(), Diagnostic> {
    let complain = |why: String| {
        Diagnostic::builder(CATALOG_001, Severity::Warning)
            .why(format!("{}: {why}", path.display()))
            .build()
    };

    let mut ids: BTreeSet<&str> = BTreeSet::new();
    for entry in entries {
        if entry.id.is_empty()
            || !entry
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(complain(format!(
                "the id {:?} is not made of a-z, 0-9 and -",
                entry.id
            )));
        }
        if !ids.insert(entry.id.as_str()) {
            return Err(complain(format!("the id {:?} appears twice", entry.id)));
        }
        if entry.name.trim().is_empty() {
            return Err(complain(format!("entry {:?} has no name", entry.id)));
        }
        if entry.hosts.is_empty() {
            return Err(complain(format!(
                "entry {:?} has no host pattern",
                entry.id
            )));
        }
        if entry.description.en.trim().is_empty() || entry.description.de.trim().is_empty() {
            return Err(complain(format!(
                "entry {:?} needs a description in both en and de",
                entry.id
            )));
        }
        if !is_web_url(&entry.source) {
            return Err(complain(format!(
                "entry {:?} needs a source that starts with http:// or https://, so the \
                 description can be checked; it has {:?}",
                entry.id, entry.source
            )));
        }
        if !is_web_url(&entry.homepage) {
            return Err(complain(format!(
                "entry {:?} has a homepage that is not an http(s) address: {:?}",
                entry.id, entry.homepage
            )));
        }
        if entry.icon.trim().is_empty() || entry.icon.contains('/') || entry.icon.contains("..") {
            return Err(complain(format!(
                "entry {:?} has an icon that is not a plain file name under catalog/icons: {:?}",
                entry.id, entry.icon
            )));
        }
    }
    Ok(())
}

fn is_web_url(text: &str) -> bool {
    let rest = text
        .strip_prefix("https://")
        .or_else(|| text.strip_prefix("http://"));
    rest.is_some_and(|rest| !rest.is_empty())
}

/// Übersetzt die Host-Muster aller Einträge in ihre Prüfform.
fn compile(entries: &[CatalogEntry]) -> Result<Vec<(HostPattern, usize)>, Diagnostic> {
    let mut patterns = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        for host in &entry.hosts {
            let compiled = pattern::parse(host).map_err(|err| {
                Diagnostic::builder(CATALOG_001, Severity::Warning)
                    .why(format!(
                        "entry {:?} has the host pattern {host:?}, which is not usable: {}",
                        entry.id, err.reason
                    ))
                    .build()
            })?;
            patterns.push((compiled, index));
        }
    }
    Ok(patterns)
}
