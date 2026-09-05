//! Was ein Sandbox-Lauf im Projektverzeichnis hinterlassen hat: der
//! Schnappschuss davor, der danach, der Fundscan dazwischen (HUM-043).
//!
//! `/work` mit Schreibrecht ist der erste der beiden offenen Kanäle
//! (BACKLOG.md 4.2, `docs/SECURITY.md` Abschnitt 3.2). Er wird nicht
//! geschlossen, sondern beobachtet. Die Beobachtung besteht aus drei Teilen,
//! und dieses Modul hält sie zusammen:
//!
//! 1. **Vor dem Start** nimmt [`SummaryWatch::start`] einen Schnappschuss des
//!    Baums ([`humanitl_sandbox::worktree`]).
//! 2. **Nach dem Ende** nimmt [`SummaryWatch::finish`] einen zweiten, bildet
//!    den Unterschied und liest die neuen und geänderten Textdateien noch
//!    einmal, um die Detektoren aus `humanitl-findings` darauf laufen zu
//!    lassen.
//! 3. Heraus kommt eine [`SessionSummary`], die der Dienst in die
//!    Aufzeichnung legt und als Ereignis schickt.
//!
//! # Warum die Orchestrierung hier liegt und nicht in `humanitl-sandbox`
//!
//! `tools/deps-allow.toml` erlaubt `humanitl-sandbox` nur `humanitl-core` und
//! `humanitl-config`. Der Schnappschuss, der Diff und die Zusammenfassung
//! selbst gehören dorthin, der Fundscan nicht: Er braucht
//! `humanitl-findings`. `humanitl-ipc` darf beide, und hier laufen sie
//! zusammen.
//!
//! # Was der Host beim Lesen niemals tut
//!
//! Der zweite Schnappschuss läuft unter dem Konto des Daemons über denselben
//! Baum, in den der Agent gerade noch geschrieben hat. Jeder Zugriff geht
//! deshalb über einen Deskriptor auf die Wurzel und
//! `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS | RESOLVE_NO_MAGICLINKS`
//! ([`humanitl_sandbox::worktree::open_beneath`]); ein `x -> /etc`, das der
//! Agent angelegt hat, führt zu `EXDEV` und nicht in das Heimatverzeichnis des
//! Menschen. Der Fundscan liest deshalb auch **nur** die rohen Pfade, die
//! [`SessionSummary::add_changes`] zurückgibt, nie die angezeigten: Die
//! Anzeige ist durch `sanitize_note` gegangen und benennt eine andere Datei
//! oder keine.
//!
//! # Budgets, und warum jede Lücke einen Namen hat
//!
//! Wie viele Dateien ein Lauf ändert, wie groß sie sind und welche Rechte sie
//! tragen, bestimmt der Agent. Drei Grenzen halten den Scan endlich:
//! [`SCAN_MAX_BYTES`] je Datei, [`SCAN_TOTAL_MAX_BYTES`] für den ganzen Lauf
//! und die Listengrenzen der Zusammenfassung selbst.
//!
//! Zwei Sätze gelten dabei, und beide sind teuer erkauft:
//!
//! **Ein Budget, das erst nach der Arbeit greift, ist kein Budget.** Die Größe
//! einer Datei wird mit dem **Rest** des Budgets verglichen, bevor sie gelesen
//! wird (`read_candidate`). Erst danach zu vergleichen hieße, das Budget zu
//! überschreiten und die Überschreitung als eingehalten zu melden.
//!
//! **Ein Überspringen, das niemand sieht, ist eine Lücke.** Jede geänderte
//! Datei, die der Scan nicht liest — zu groß, unlesbar, Budget aufgebraucht —,
//! trägt den Grund an ihrer Zeile ([`SessionSummary::mark_unscanned`]) und
//! zählt in `SANDBOX_028`. Ohne das läse sich „kein Fund" wie „sauber",
//! obwohl gar nicht nachgesehen wurde, und der Agent hätte einen Weg an der
//! Prüfung vorbei: eine Datei so hinterlassen, dass sie sich nicht lesen lässt.

use std::os::fd::{AsFd as _, BorrowedFd};
use std::path::{Path, PathBuf};

use humanitl_config::{Config, WorkMode};
use humanitl_core::ids::{SandboxId, SessionId};
use humanitl_core::{Diagnostic, Finding, FindingLocation};
use humanitl_findings::{Detector, FindingsSettings, ScanInput};
use humanitl_sandbox::summary::{
    SCAN_MAX_BYTES, SCAN_TOTAL_MAX_BYTES, ScanCandidate, ScanSkip, SessionSummary, SummaryFinding,
    executable_on_host, looks_like_text,
};
use humanitl_sandbox::worktree::{self, SnapshotLimits, TreeSnapshot};

/// Der Blick auf das Projektverzeichnis über einen Sandbox-Lauf hinweg.
///
/// Ein Wert davon entsteht vor dem Start und wird nach dem Ende verbraucht.
/// Er hält den Schnappschuss von vorher; ohne ihn gäbe es nach dem Lauf keinen
/// Vergleich mehr, und nachträglich ist er nicht zu bekommen.
#[derive(Debug)]
pub struct SummaryWatch {
    work_dir: PathBuf,
    before: TreeSnapshot,
    unprotected: Vec<PathBuf>,
    limits: SnapshotLimits,
}

impl SummaryWatch {
    /// Nimmt den Schnappschuss vor dem Start.
    ///
    /// **Der Aufruf gehört zwischen `plan` und `launch`.** Später wäre er
    /// falsch: Was der Agent zwischen Start und Schnappschuss schreibt, stünde
    /// als „war schon da" im Baum und fehlte am Ende im Unterschied. Früher
    /// ginge nicht, denn [`humanitl_sandbox::LaunchPlan::unprotected`] entsteht
    /// erst beim Planen.
    ///
    /// `None` bei [`WorkMode::Ro`]: Dann hängt `bwrap` das Projekt als
    /// `--ro-bind` ein, der Agent kann nichts darin ändern, und ein Lauf über
    /// den ganzen Baum kostete Zeit für einen Unterschied, den es nicht geben
    /// kann. Ohne Schnappschuss gibt es später auch keine Zusammenfassung —
    /// eine leere zu melden hieße zu behaupten, es sei gemessen worden.
    ///
    /// # Errors
    ///
    /// `SANDBOX_011`, wenn sich das Projektverzeichnis nicht öffnen lässt. Der
    /// Start läuft trotzdem weiter; der Aufrufer meldet den Befund und
    /// verzichtet auf die Zusammenfassung.
    pub fn start(
        work_dir: &Path,
        work_mode: WorkMode,
        unprotected: &[PathBuf],
    ) -> Result<Option<Self>, Diagnostic> {
        if work_mode == WorkMode::Ro {
            return Ok(None);
        }
        let limits = SnapshotLimits::default();
        let before = worktree::snapshot(work_dir, &limits)?;
        Ok(Some(Self {
            work_dir: work_dir.to_path_buf(),
            before,
            unprotected: unprotected.to_vec(),
            limits,
        }))
    }

    /// Wie viele Einträge der erste Schnappschuss erfasst hat.
    ///
    /// Für die Protokollzeile des Starts: Was der Lauf über das
    /// Projektverzeichnis gekostet hat, soll nachlesbar sein und nicht geraten
    /// werden müssen.
    #[must_use]
    pub fn entries(&self) -> usize {
        self.before.len()
    }

    /// Nimmt den zweiten Schnappschuss, scannt die geänderten Dateien und
    /// liefert die Zusammenfassung.
    ///
    /// **Der Aufruf gehört hinter das Ende des Prozesses.** Solange der Agent
    /// noch läuft, wäre der zweite Schnappschuss eine Aussage über einen
    /// Zeitpunkt, den der Lauf gleich wieder überholt.
    ///
    /// Blockierend: Er läuft über den Baum und liest Dateien.
    ///
    /// # Errors
    ///
    /// `SANDBOX_011`, wenn sich das Projektverzeichnis nicht mehr öffnen lässt.
    pub fn finish(
        self,
        session: SessionId,
        sandbox: SandboxId,
        settings: &FindingsSettings,
    ) -> Result<SessionSummary, Diagnostic> {
        // Ein Deskriptor auf die Wurzel für alles Weitere: Der zweite
        // Schnappschuss und jede gelesene Datei hängen an demselben, und keiner
        // von beiden löst je einen Pfad als Zeichenkette auf.
        let root = worktree::open_root(&self.work_dir)?;
        let after = worktree::snapshot_at(root.as_fd(), &self.limits);

        let mut summary = SessionSummary::new(session, sandbox, &self.work_dir);
        // Vor `add_changes`, sonst bliebe `unprotected_by` überall `None`.
        summary.set_unprotected(&self.unprotected);
        let candidates = summary.add_changes(&self.work_dir, &self.before, &after);
        scan(
            &mut summary,
            root.as_fd(),
            &candidates,
            settings,
            SCAN_TOTAL_MAX_BYTES,
        )?;
        Ok(summary)
    }
}

/// Liest die neuen und geänderten Dateien und trägt ein, was in ihnen steckt.
///
/// `candidates` sind die **rohen** Pfade aus [`SessionSummary::add_changes`],
/// relativ zum Projektverzeichnis. Zwei Arten von Fund entstehen hier:
///
/// - was die Detektoren aus `humanitl-findings` sehen (Geheimnisse, eigene
///   Begriffe, personenbezogene Daten), und
/// - eine Datei, die dieser Rechner von sich aus ausführt
///   ([`executable_on_host`]) — der Kern von Kanal 1: nicht jede geschriebene
///   Datei ist gefährlich, aber eine, die eine Werkzeugkette von selbst
///   startet, ist es.
///
/// Binärdateien laufen nicht durch die Detektoren ([`looks_like_text`]); die
/// Heuristik ist ein `NUL`-Byte in den ersten Kilobytes, dieselbe, die `grep`
/// und `git` benutzen. [`executable_on_host`] fragt trotzdem: Ein Git-Hook,
/// der eine `ELF`-Datei ist, wird beim nächsten Commit genauso ausgeführt wie
/// ein Shell-Skript.
///
/// `budget` ist [`SCAN_TOTAL_MAX_BYTES`], wenn der Aufruf aus
/// [`SummaryWatch::finish`] kommt; als Argument steht es hier, damit ein Test
/// die Grenze erreichen kann, ohne 256 MiB zu schreiben. Eine Datei wird
/// **ganz oder gar nicht** gelesen: Ein halb gelesener Inhalt könnte ein
/// Geheimnis in der Mitte durchschneiden, und ein Fund, den es nicht gibt,
/// sähe aus wie eine saubere Datei. Das Budget kann deshalb um höchstens eine
/// Datei überzogen werden; sobald es leer ist und noch etwas ansteht, endet
/// der Scan und [`SessionSummary::truncated`] sagt es.
///
/// # Errors
///
/// `FINDINGS_001`, wenn sich das eingebaute Regel-Set der Detektoren nicht
/// übersetzen lässt. Das ist ein Fehler im Daemon, und er wird gemeldet: Eine
/// Suche, die stillschweigend ausfällt, ließe ein leeres Ergebnis wie ein
/// sauberes aussehen.
fn scan(
    summary: &mut SessionSummary,
    root: BorrowedFd<'_>,
    candidates: &[ScanCandidate],
    settings: &FindingsSettings,
    mut budget: u64,
) -> Result<(), Diagnostic> {
    let detectors = humanitl_findings::detectors::tier1(settings)?;
    for candidate in candidates {
        let bytes = read_candidate(summary, root, candidate, budget);
        // Der Pfadteil von `executable_on_host` läuft **immer**, auch wenn die
        // Datei ungelesen blieb: Ob etwas ein Git-Hook, ein `Makefile` oder
        // eine Workflow-Datei ist, hängt am Pfad und nicht am Inhalt. Ein
        // 5 MiB großer `pre-commit` ist derselbe Hook wie ein zwanzigzeiliger,
        // und eine `ELF`-Datei dort führt der Host genauso aus wie ein
        // Shell-Skript. Die Regeln, die in den Inhalt sehen (`package.json`
        // mit `postinstall`, `Cargo.toml` mit `build`), greifen dann nicht —
        // das ist der Preis, und `SANDBOX_028` nennt die Datei.
        let content: &[u8] = bytes.as_deref().unwrap_or(&[]);
        if executable_on_host(&candidate.path, content) {
            summary.add_finding(SummaryFinding::executable_on_host(&candidate.path));
        }
        let Some(bytes) = bytes else {
            continue;
        };
        let read = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        budget = budget.saturating_sub(read);
        summary.scanned_bytes = summary.scanned_bytes.saturating_add(read);
        // Die Detektoren sind Regex über Text; eine Binärdatei wären Megabytes
        // Arbeit ohne Ertrag. Das ist kein übersprungener Scan, sondern einer
        // mit dem Ergebnis „hier steht kein Text" — deshalb kein Vermerk.
        if !looks_like_text(&bytes) {
            continue;
        }
        for finding in detect(&detectors, settings, &bytes) {
            summary.add_finding(SummaryFinding::from_finding(
                &candidate.path,
                &bytes,
                &finding,
            ));
        }
    }
    Ok(())
}

/// Liest eine Kandidatendatei — oder vermerkt an ihrer Zeile, warum nicht.
///
/// **Jede Entscheidung fällt vor dem Lesen, und jede wird vermerkt.** Die drei
/// Wege daran vorbei waren je ein Loch in genau der Prüfung, die dieses Issue
/// liefert, und der Agent bestimmt alle drei Größen:
///
/// - **Zu groß.** Eine Datei über [`SCAN_MAX_BYTES`] wurde früher gar nicht
///   erst Kandidat und fiel spurlos aus der Zusammenfassung. Jetzt steht sie
///   darin, ungelesen und als ungelesen benannt.
/// - **Über dem Budget.** Verglichen wird die Größe der Datei mit dem **Rest**
///   des Budgets. Erst danach zu vergleichen hieße, es um bis zu
///   [`SCAN_MAX_BYTES`] zu überschreiten und die Überschreitung als
///   eingehalten zu melden. `continue` statt `break`, damit eine kleinere
///   Datei danach noch hineinpasst; benannt wird jede.
/// - **Nicht lesbar.** Rechte, verschwunden, kein gewöhnlicher Inhalt mehr.
///   Der Agent kann eine geänderte Datei so hinterlassen, dass sie sich nicht
///   lesen lässt; ohne Vermerk verschwände sie aus dem Geheimnis-Scan, während
///   der Bericht vollständig aussieht.
fn read_candidate(
    summary: &mut SessionSummary,
    root: BorrowedFd<'_>,
    candidate: &ScanCandidate,
    budget: u64,
) -> Option<Vec<u8>> {
    if candidate.size > SCAN_MAX_BYTES {
        summary.mark_unscanned(candidate.row, ScanSkip::TooLarge);
        return None;
    }
    if candidate.size > budget {
        summary.mark_unscanned(candidate.row, ScanSkip::Budget);
        return None;
    }
    match worktree::read_beneath(root, &candidate.path, SCAN_MAX_BYTES) {
        Ok(bytes) => Some(bytes),
        Err(_diagnostic) => {
            summary.mark_unscanned(candidate.row, ScanSkip::Unreadable);
            None
        }
    }
}

/// Die Funde aller Detektoren in einem Puffer, sortiert und ohne Dubletten.
///
/// Der Ort ist [`FindingLocation::Body`] und verlässt diese Funktion nicht: Eine
/// Datei ist keine Anfrage, und `FindingLocation` kennt keine Datei. Für die
/// Detektoren heißt `Body` „der ganze Inhalt", und genau das ist gemeint —
/// eine Regel, die nur für einen bestimmten Header gilt, greift damit
/// richtigerweise nicht. Die Zeile und der Pfad kommen später aus
/// [`SummaryFinding::from_finding`].
///
/// Ein Wert aus `findings.ignored_hashes` fällt weg: Der Mensch hat gesagt,
/// dass genau dieser Wert kein Fund ist, und das gilt in einer Datei wie in
/// einer Anfrage.
fn detect(
    detectors: &[Box<dyn Detector>],
    settings: &FindingsSettings,
    bytes: &[u8],
) -> Vec<Finding> {
    let input = ScanInput {
        location: FindingLocation::Body,
        bytes,
        content_type: None,
    };
    let mut found: Vec<Finding> = detectors
        .iter()
        .flat_map(|detector| detector.scan(&input))
        .filter(|finding| !settings.ignored_hashes.contains(&finding.value_hash))
        .collect();
    found.sort_by(|left, right| {
        left.span
            .start
            .cmp(&right.span.start)
            .then(left.span.end.cmp(&right.span.end))
            .then(left.kind.cmp(&right.kind))
    });
    found.dedup_by(|later, kept| later.span == kept.span && later.kind == kept.kind);
    found
}

/// Die Einstellungen des Scans aus der Konfiguration dieser Sitzung.
///
/// Sie stehen hier und nicht zweimal: `humanitld` baut damit den Scanner des
/// Proxys ([`humanitl_findings::DetectorRegistry`]), und die Zusammenfassung
/// eines Sandbox-Laufs baut damit dieselben Detektoren. Zwei Ableitungen aus
/// denselben vier Schlüsseln wären zwei Wahrheiten darüber, was als Fund gilt,
/// und die Oberfläche sähe den Unterschied erst an den Ergebnissen.
///
/// # Errors
///
/// `CONFIG_003`, wenn ein Eintrag in `findings.ignored_hashes` keine 64
/// Hex-Zeichen ist.
pub fn findings_settings(config: &Config) -> Result<FindingsSettings, Diagnostic> {
    let cap_bytes = usize::try_from(config.limits.preview_cap_bytes)
        .unwrap_or(humanitl_findings::settings::DEFAULT_CAP_BYTES);
    Ok(FindingsSettings::default()
        .with_enabled(config.findings.enabled)
        .with_user_terms(config.findings.user_terms.iter())
        .with_email_allow_domains(config.findings.email_allow_domains.iter())
        .with_ignored_hashes_hex(config.findings.ignored_hashes.iter())?
        .with_limits(cap_bytes, config.limits.max_decompress_ratio))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::path::PathBuf;

    use humanitl_config::WorkMode;
    use humanitl_core::ids::{SandboxId, SessionId};
    use humanitl_findings::FindingsSettings;

    use humanitl_sandbox::summary::ScanSkip;

    use super::SummaryWatch;

    /// Ein Lauf, der eine Datei mit einem Schlüssel und einen Git-Hook
    /// hinterlässt, ergibt beide Befunde und keinen dritten.
    #[test]
    fn a_run_that_writes_a_secret_and_a_hook_says_both() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        std::fs::write(root.join("README.md"), b"# hi\n").expect("readme");

        let watch = SummaryWatch::start(root, WorkMode::Rw, &[PathBuf::from(".git/hooks")])
            .expect("the snapshot works")
            .expect("read-write means a snapshot");

        // Der Wert wird zur Laufzeit zusammengesetzt, damit der Push-Schutz von
        // GitHub nicht auf einen Testwert anspringt (CONVENTIONS 4.13).
        let secret = format!("{}{}", "AKIA", "IOSFODNN7EXAMPLE");
        std::fs::write(root.join("creds.env"), format!("key = {secret}\n")).expect("creds");
        std::fs::create_dir_all(root.join(".git/hooks")).expect("hooks");
        std::fs::write(
            root.join(".git/hooks/pre-commit"),
            b"#!/bin/sh\ncurl evil\n",
        )
        .expect("hook");

        let summary = watch
            .finish(
                SessionId::nil(),
                SandboxId::nil(),
                &FindingsSettings::default(),
            )
            .expect("the second snapshot works");

        let codes: Vec<&str> = summary
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect();
        assert!(codes.contains(&"SANDBOX_023"), "{codes:?}");
        assert!(codes.contains(&"SANDBOX_025"), "{codes:?}");
        assert!(codes.contains(&"SANDBOX_026"), "{codes:?}");
        assert!(
            !codes.contains(&"SANDBOX_024"),
            "nothing was cut: {codes:?}"
        );

        // Die beiden Zahlen zählen zwei verschiedene Dinge. Zwei Funde stehen
        // in der Liste, aber nur einer ist ein mögliches Geheimnis; ein
        // `Makefile`, das der Agent geschrieben hat, als Geheimnis zu melden
        // wäre eine falsche Auskunft über beide Zahlen.
        assert_eq!(summary.findings.len(), 2, "{:?}", summary.findings);
        let secrets = summary
            .diagnostics()
            .into_iter()
            .find(|diagnostic| diagnostic.code.as_str() == "SANDBOX_023")
            .expect("the secret finding exists");
        assert!(
            secrets.why.contains("1 potential secret(s)"),
            "one secret, not two: {}",
            secrets.why
        );
        let executable = summary
            .diagnostics()
            .into_iter()
            .find(|diagnostic| diagnostic.code.as_str() == "SANDBOX_026")
            .expect("the executable finding exists");
        assert!(
            executable.why.contains("1 file(s)"),
            "one executable file: {}",
            executable.why
        );
        assert!(
            executable.why.contains(".git/hooks/pre-commit"),
            "and it is named: {}",
            executable.why
        );

        let secret_finding = summary
            .findings
            .iter()
            .find(|finding| finding.path == "creds.env")
            .expect("the secret is found");
        assert_eq!(secret_finding.kind, "api_key:aws");
        assert!(
            !secret_finding.display_prefix.contains(&secret),
            "the value never leaves the file"
        );
        let hook = summary
            .findings
            .iter()
            .find(|finding| finding.path == ".git/hooks/pre-commit")
            .expect("the hook is found");
        assert!(hook.is_executable_on_host());
        assert!(
            hook.value_hash.is_empty(),
            "there is no found value, so there is no hash of one"
        );
        assert!(
            summary.scanned_bytes > 0,
            "the scan says how much it has read"
        );

        // Die unveränderte Datei wird nicht gelesen: Sie kann nichts Neues
        // enthalten, und der Scan zahlt nicht für sie.
        assert!(
            !summary
                .changes
                .iter()
                .any(|change| change.path == "README.md"),
            "an untouched file is not a change: {:?}",
            summary.changes
        );
    }

    /// Ein Projekt, das nur gelesen werden darf, wird nicht durchlaufen.
    #[test]
    fn a_read_only_project_gets_no_snapshot() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(
            SummaryWatch::start(dir.path(), WorkMode::Ro, &[])
                .expect("no error")
                .is_none(),
            "nothing can change under --ro-bind, so nothing is measured"
        );
        assert!(
            SummaryWatch::start(dir.path(), WorkMode::Rw, &[])
                .expect("no error")
                .is_some()
        );
    }

    /// Der Scan der Zusammenfassung liest dieselben vier Schlüssel wie der
    /// Scan des Proxys.
    ///
    /// Beide bauen ihre Detektoren aus [`super::findings_settings`]; liefe das
    /// auseinander, gälte ein eigener Begriff des Nutzers auf der Leitung und
    /// nicht im Projekt, und niemand sähe den Unterschied außer an den
    /// Ergebnissen.
    #[test]
    fn the_findings_settings_come_from_the_configuration() {
        let mut config = humanitl_config::Config::default();
        config.findings.enabled = false;
        config.findings.user_terms = vec!["projektname".to_owned()];
        config.findings.email_allow_domains = vec!["example.org".to_owned()];
        config.findings.ignored_hashes = vec!["a".repeat(64)];

        let settings = super::findings_settings(&config).expect("64 hex characters are readable");
        assert!(!settings.enabled);
        assert_eq!(settings.user_terms, vec!["projektname".to_owned()]);
        assert_eq!(settings.email_allow_domains, vec!["example.org".to_owned()]);
        assert_eq!(settings.ignored_hashes.len(), 1);
        assert_eq!(
            settings.cap_bytes,
            usize::try_from(config.limits.preview_cap_bytes).expect("the cap fits")
        );

        // Ein Hash, der keiner ist, wird nicht übergangen: Sonst hielte der
        // Nutzer einen Wert für unterdrückt, den jeder Lauf weiter meldet.
        config.findings.ignored_hashes = vec!["nope".to_owned()];
        let diagnostic = super::findings_settings(&config).expect_err("that is no sha256");
        assert_eq!(diagnostic.code.as_str(), "CONFIG_003");
    }

    /// Ein Verzeichnis, das es nicht gibt, ist ein Befund und kein Absturz.
    #[test]
    fn a_missing_project_directory_is_a_diagnostic() {
        let dir = tempfile::tempdir().expect("tempdir");
        let gone = dir.path().join("not-here");
        let diagnostic =
            SummaryWatch::start(&gone, WorkMode::Rw, &[]).expect_err("there is no directory");
        assert_eq!(diagnostic.code.as_str(), "SANDBOX_011");
    }

    /// Ein Git-Hook, der eine Binärdatei ist, wird trotzdem gemeldet.
    ///
    /// Die Text-Heuristik hält die Detektoren von Megabytes ohne Ertrag fern.
    /// Sie darf aber nicht die Frage verdecken, um die es in Kanal 1 geht: Was
    /// in `.git/hooks/` liegt, führt Git beim nächsten Commit aus, ob es nun
    /// ein Shell-Skript ist oder eine `ELF`-Datei.
    #[test]
    fn a_hook_that_is_a_binary_is_still_reported() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        std::fs::create_dir_all(root.join(".git/hooks")).expect("hooks");
        let watch = SummaryWatch::start(root, WorkMode::Rw, &[])
            .expect("snapshot")
            .expect("read-write");
        std::fs::write(root.join(".git/hooks/pre-commit"), b"\x7fELF\0\0\0\x02\0").expect("hook");

        let summary = watch
            .finish(
                SessionId::nil(),
                SandboxId::nil(),
                &FindingsSettings::default(),
            )
            .expect("the second snapshot works");
        let hook = summary
            .findings
            .iter()
            .find(|finding| finding.path == ".git/hooks/pre-commit")
            .unwrap_or_else(|| panic!("the binary hook is reported: {:?}", summary.findings));
        assert!(hook.is_executable_on_host());
        let codes: Vec<&str> = summary
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect();
        assert!(codes.contains(&"SANDBOX_026"), "{codes:?}");
        assert!(
            !codes.contains(&"SANDBOX_023"),
            "and it is not a secret: {codes:?}"
        );
    }

    /// Das Budget des Laufs greift, und es greift sichtbar.
    ///
    /// Wie viele Dateien ein Lauf ändert, bestimmt der Agent:
    /// [`super::SCAN_MAX_BYTES`] deckelt die einzelne Datei, das Budget den
    /// ganzen Lauf. Ohne das zweite wäre die Obergrenze das Produkt aus beiden
    /// Grenzen. Geprüft wird mit einem kleinen Budget statt mit 256 MiB auf der
    /// Platte — die Grenze ist dieselbe, nur die Zahl ist es nicht.
    #[test]
    fn the_scan_stops_at_its_budget_and_says_so() {
        use std::os::fd::AsFd as _;

        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        let limits = humanitl_sandbox::worktree::SnapshotLimits::default();
        let before = humanitl_sandbox::worktree::snapshot(root, &limits).expect("before");

        let secret = format!("{}{}", "AKIA", "IOSFODNN7EXAMPLE");
        let line = format!("key = {secret}\n");
        for n in 0..4 {
            std::fs::write(root.join(format!("f{n}.env")), &line).expect("write");
        }
        let after = humanitl_sandbox::worktree::snapshot(root, &limits).expect("after");
        let one = u64::try_from(line.len()).expect("a line fits in u64");
        let fd = humanitl_sandbox::worktree::open_root(root).expect("root");

        // Ein Budget, das für eine Datei reicht und für die zweite **fast**.
        // Das eine Byte Rest ist der ganze Punkt: Wer das Budget mit sich
        // selbst vergleicht („ist noch etwas übrig?"), liest die zweite Datei
        // und überschreitet die Grenze um fast eine ganze Datei; wer es mit
        // der Datei vergleicht, hört auf und sagt es.
        let budget = one + 1;
        let mut summary = humanitl_sandbox::summary::SessionSummary::new(
            SessionId::nil(),
            SandboxId::nil(),
            root,
        );
        let candidates = summary.add_changes(root, &before, &after);
        assert_eq!(candidates.len(), 4, "{candidates:?}");
        super::scan(
            &mut summary,
            fd.as_fd(),
            &candidates,
            &FindingsSettings::default(),
            budget,
        )
        .expect("the detectors build");

        assert_eq!(summary.findings.len(), 1, "{:?}", summary.findings);
        assert!(summary.truncated, "a budget that cut something says so");
        // **Jede übersprungene Datei ist benannt.** Ohne das sähe die
        // Zusammenfassung aus wie eine, in der drei Dateien sauber waren.
        let skipped: Vec<&str> = summary
            .changes
            .iter()
            .filter(|change| change.unscanned == Some(ScanSkip::Budget))
            .map(|change| change.path.as_str())
            .collect();
        assert_eq!(skipped.len(), 3, "{:?}", summary.changes);
        assert_eq!(summary.scanned_bytes, one, "exactly one file was read");
        assert!(
            summary.scanned_bytes <= budget,
            "a budget is a limit, not a counter: {} read of {budget}",
            summary.scanned_bytes
        );
        let codes: Vec<&str> = summary
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect();
        assert!(codes.contains(&"SANDBOX_024"), "{codes:?}");
        assert!(codes.contains(&"SANDBOX_028"), "{codes:?}");

        // Und die Gegenprobe: Mit genug Budget wird alles gelesen, nichts
        // abgeschnitten und nichts vermerkt. Ohne sie wäre der Test auch dann
        // grün, wenn der Scan immer nach der ersten Datei aufhörte.
        let mut whole = humanitl_sandbox::summary::SessionSummary::new(
            SessionId::nil(),
            SandboxId::nil(),
            root,
        );
        let candidates = whole.add_changes(root, &before, &after);
        super::scan(
            &mut whole,
            fd.as_fd(),
            &candidates,
            &FindingsSettings::default(),
            super::SCAN_TOTAL_MAX_BYTES,
        )
        .expect("the detectors build");
        assert_eq!(whole.findings.len(), 4, "{:?}", whole.findings);
        assert!(!whole.truncated);
        assert!(
            whole
                .changes
                .iter()
                .all(|change| change.unscanned.is_none()),
            "{:?}",
            whole.changes
        );
    }

    /// Eine geänderte Datei, die sich nicht lesen lässt, verschwindet nicht.
    ///
    /// Der Agent bestimmt die Rechte an dem, was er schreibt. Ohne Vermerk
    /// fiele eine Datei, die er unlesbar zurücklässt, aus dem Geheimnis-Scan,
    /// während der Bericht vollständig aussieht — das ist kein Randfall,
    /// sondern der Weg an der Prüfung vorbei.
    #[test]
    fn an_unreadable_change_is_named_and_not_dropped() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        let watch = SummaryWatch::start(root, WorkMode::Rw, &[])
            .expect("snapshot")
            .expect("read-write");

        let secret = format!("{}{}", "AKIA", "IOSFODNN7EXAMPLE");
        std::fs::write(root.join("hidden.env"), format!("key = {secret}\n")).expect("write");
        std::fs::write(root.join("plain.env"), b"nothing here\n").expect("write");
        std::fs::set_permissions(
            root.join("hidden.env"),
            std::fs::Permissions::from_mode(0o000),
        )
        .expect("chmod");

        let summary = watch
            .finish(
                SessionId::nil(),
                SandboxId::nil(),
                &FindingsSettings::default(),
            )
            .expect("the second snapshot works");

        // Als `root` ist auch eine Datei mit Modus 000 lesbar; dann gibt es
        // den Fall auf dieser Maschine nicht. **Gefragt wird die Maschine, nicht
        // das Ergebnis**: Ein Überspringen, das am Ergebnis hängt, wäre grün,
        // sobald der Vermerk fehlt — also genau dann, wenn der Test anschlagen
        // müsste.
        if std::fs::read(root.join("hidden.env")).is_ok() {
            eprintln!("skipping: this user reads a file with mode 000 (root?)");
            return;
        }

        let hidden = summary
            .changes
            .iter()
            .find(|change| change.path == "hidden.env")
            .expect("the file is listed either way");
        assert_eq!(hidden.unscanned, Some(ScanSkip::Unreadable));
        assert!(summary.truncated, "an unread change is not a full summary");
        assert!(
            summary.findings.is_empty(),
            "nothing was found because nothing was read: {:?}",
            summary.findings
        );
        let diagnostic = summary
            .diagnostics()
            .into_iter()
            .find(|diagnostic| diagnostic.code.as_str() == "SANDBOX_028")
            .expect("the unread file is reported");
        assert!(diagnostic.why.contains("hidden.env"), "{}", diagnostic.why);
        assert!(
            diagnostic.why.contains("could not be read"),
            "{}",
            diagnostic.why
        );

        // Die lesbare Datei daneben wurde sehr wohl gelesen.
        let plain = summary
            .changes
            .iter()
            .find(|change| change.path == "plain.env")
            .expect("the readable file is listed");
        assert_eq!(plain.unscanned, None);
    }

    /// Eine Datei über [`super::SCAN_MAX_BYTES`] fällt nicht mehr spurlos aus
    /// der Zusammenfassung.
    ///
    /// Sie stand früher nicht einmal in der Kandidatenliste: kein Scan, kein
    /// Vermerk, ein Bericht, der vollständig aussieht. Die Größe bestimmt der
    /// Agent.
    #[test]
    fn a_file_over_the_scan_limit_is_named_and_still_checked_by_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        std::fs::create_dir_all(root.join(".git/hooks")).expect("hooks");
        let watch = SummaryWatch::start(root, WorkMode::Rw, &[])
            .expect("snapshot")
            .expect("read-write");

        // Ein Hook, größer als der Scan liest. Der Inhalt ist gleichgültig; es
        // geht um die Größe und den Pfad.
        let big = vec![b'x'; usize::try_from(super::SCAN_MAX_BYTES).expect("fits") + 1];
        std::fs::write(root.join(".git/hooks/pre-commit"), &big).expect("hook");

        let summary = watch
            .finish(
                SessionId::nil(),
                SandboxId::nil(),
                &FindingsSettings::default(),
            )
            .expect("the second snapshot works");

        let hook = summary
            .changes
            .iter()
            .find(|change| change.path == ".git/hooks/pre-commit")
            .expect("the big file is listed");
        assert_eq!(hook.unscanned, Some(ScanSkip::TooLarge));
        assert!(summary.truncated);
        assert_eq!(
            summary.scanned_bytes, 0,
            "nothing was read, so nothing counts as read"
        );

        // Und der Pfadteil läuft trotzdem: Ob etwas ein Git-Hook ist, hängt
        // nicht an seiner Größe.
        let finding = summary
            .findings
            .iter()
            .find(|finding| finding.path == ".git/hooks/pre-commit")
            .expect("a hook is a hook whatever its size");
        assert!(finding.is_executable_on_host());
        let codes: Vec<&str> = summary
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect();
        assert!(codes.contains(&"SANDBOX_026"), "{codes:?}");
        assert!(codes.contains(&"SANDBOX_028"), "{codes:?}");
    }

    /// Eine benannte Röhre im Projekt lässt den Daemon nicht hängen.
    ///
    /// `mkfifo` darf jeder, der in `/work` schreiben darf, und ein `open` einer
    /// Röhre ohne Schreiber wartet — der Daemon bliebe im zweiten Schnappschuss
    /// stehen, bis jemand hineinschreibt. Der Schnappschuss sieht sie als
    /// `Kind::Other` und macht sie nicht zum Kandidaten; `read_beneath` öffnet
    /// zusätzlich mit `O_NONBLOCK`.
    #[test]
    fn a_named_pipe_in_the_project_does_not_hang_the_scan() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        let watch = SummaryWatch::start(root, WorkMode::Rw, &[])
            .expect("snapshot")
            .expect("read-write");

        let fifo = root.join("pipe");
        let status = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .expect("mkfifo runs");
        if !status.success() {
            eprintln!("skipping: mkfifo is not available here");
            return;
        }

        // Ohne den Schutz kehrt dieser Aufruf nie zurück.
        let summary = watch
            .finish(
                SessionId::nil(),
                SandboxId::nil(),
                &FindingsSettings::default(),
            )
            .expect("the second snapshot works");
        let pipe = summary
            .changes
            .iter()
            .find(|change| change.path == "pipe")
            .expect("the pipe is listed as a change");
        assert_eq!(
            pipe.unscanned, None,
            "a pipe is not a file the scan skipped; it was never a candidate"
        );
        assert!(summary.findings.is_empty(), "{:?}", summary.findings);
    }

    /// Ein Wert aus `findings.ignored_hashes` ist auch in einer Datei kein Fund.
    #[test]
    fn an_ignored_value_stays_ignored_in_a_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        let watch = SummaryWatch::start(root, WorkMode::Rw, &[])
            .expect("snapshot")
            .expect("read-write");

        let secret = format!("{}{}", "AKIA", "IOSFODNN7EXAMPLE");
        std::fs::write(root.join("creds.env"), format!("key = {secret}\n")).expect("creds");

        // Der Hash, den `findings.ignored_hashes` trägt, ist derselbe, den ein
        // Fund über denselben Wert trägt.
        let hash = humanitl_core::Finding::new(
            humanitl_core::FindingKind::ApiKey("aws".to_owned()),
            0..0,
            humanitl_core::FindingLocation::Body,
            humanitl_core::Tier::Regex,
            &secret,
        )
        .value_hash_hex();
        let settings = FindingsSettings::default()
            .with_ignored_hashes_hex([hash])
            .expect("the hash is 64 hex characters");
        let summary = watch
            .finish(SessionId::nil(), SandboxId::nil(), &settings)
            .expect("the second snapshot works");
        assert!(
            summary.findings.is_empty(),
            "the user said this value is not a finding: {:?}",
            summary.findings
        );
    }
}
