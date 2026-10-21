//! Die Aufbewahrung der Kette: `audit.retention_days` (HUM-157) und
//! `audit.max_bytes` (HUM-194).
//!
//! Aus einer Hash-Kette zu löschen bricht sie: Der erste Record, der bleibt,
//! nennt in `prev` den Hash eines Records, den es nicht mehr gibt. Eine
//! Löschung ist deshalb nur ehrlich, wenn die Kette sie selbst dokumentiert.
//! Das tut der Record `audit.pruned`: Er nennt Nummer und Hash des letzten
//! gelöschten Records, und [`crate::verify`] erkennt einen Anfang, dessen
//! `prev` genau darauf zeigt, als erlaubten Neuanfang. Ein Anfang ohne einen
//! solchen Record bleibt ein Bruch (`SeqGap`), wie vor HUM-157.
//!
//! Hier stehen der Record und die Grenzen eines Laufs. Den Lauf selbst, mit
//! Datei, Sperre und Umbenennen, führt der Schreiber in `humanitl-audit-log`
//! aus (HUM-155), denn diese Crate öffnet keine Datei.
//!
//! **Ein Lauf, in dieser Reihenfolge**, im Schreib-Thread und damit ohne
//! parallelen Schreiber:
//!
//! 1. Alles Geschickte auf die Platte, dann die ganze Kette prüfen, mit
//!    Schlüssel und Ankern. Eine Kette, die nicht hält, wird nicht gekürzt,
//!    weder nach Alter noch nach Größe: Die Löschung würde den Beleg der
//!    Manipulation gleich mitlöschen und eine rote Kette grün machen. Der Lauf
//!    endet dann mit `AUDIT_001`.
//! 2. Den Schnitt suchen ([`PruneLimits`]): von vorn die Records, deren `ts`
//!    vor der Grenze in der Zeit liegt, bis zum ersten, der es nicht tut
//!    (`find_cut`), und von vorn so viele Records, dass die Datei nach dem
//!    Lauf höchstens `audit.max_bytes` groß ist (`find_byte_cut`). Beide
//!    Schnitte sind ein zusammenhängender Anfang; es gilt der weitere.
//! 3. `audit.pruned` anhängen und gleich dahinter einen Anker, in der Datei
//!    und in `audit_anchors`, und beides auf die Platte. Der dokumentierende
//!    Record steht damit nie im unverankerten Ende, das jemand unbemerkt
//!    abschneiden könnte.
//! 4. Den Rest der Datei in eine neue Datei daneben kopieren, sie sperren,
//!    auf die Platte bringen und über `audit.jsonl` umbenennen (`rewrite`).
//!    Der Schreiber hängt danach an die neue Datei an.
//!
//! Ein Absturz zwischen 3 und 4 lässt eine vollständige Kette mit einem
//! `audit.pruned` zurück, dessen Schnitt nicht stattfand. Das ist kein Bruch;
//! der nächste Lauf schneidet erneut.
//!
//! **Die Grenze in Bytes** (HUM-194). Gelöscht wird in der Reihenfolge des
//! Schreibens, das Älteste zuerst, immer ganze Records und nur am Anfang. Der
//! Schnitt rechnet den Record des Laufs und seinen Anker mit ein, beide mit
//! der größten Länge, die sie haben können; die Datei liegt nach dem Lauf
//! deshalb höchstens bei `audit.max_bytes`, meist einige Bytes darunter,
//! sofern die Grenze mindestens [`MIN_MAX_BYTES`] beträgt und der neueste
//! Record höchstens `MAX_LINE_BYTES` lang ist. Der
//! neueste Record vor dem Lauf geht nach dieser Regel nie, und der Record des
//! Laufs samt Anker steht ohnehin hinter dem Schnitt: Ist der neueste Record
//! allein schon zu groß, bleiben er, `audit.pruned` und dessen Anker stehen,
//! die Datei liegt über der Grenze, und [`PruneOutcome::size`] sagt es. Die
//! Grenze wirkt nur in diesem Lauf, also beim Start des Daemons und danach
//! täglich; dazwischen wächst die Datei weiter.
//!
//! **Bei fast voller Platte.** Schritt 4 braucht neben der alten Datei Platz
//! für den Rest, bei gesetzter Grenze also höchstens `audit.max_bytes`.
//! Scheitert das Schreiben der Nebendatei (etwa `ENOSPC`), geht sie wieder,
//! `audit.jsonl` bleibt, wie es war, mit dem `audit.pruned` aus Schritt 3, und
//! der Lauf meldet `AUDIT_006`; der nächste Lauf schneidet erneut. Scheitert
//! schon Schritt 3, wird nichts gelöscht.
//!
//! **Die Anker bleiben.** `audit_anchors` wird nicht gekürzt: Die Anker sind
//! der Beleg gegen das Kürzen der Datei, und wer sie mitlöscht, löscht den
//! Beleg. Anker unter dem Schnitt prüft `verify` nicht mehr, weil ihre Records
//! fehlen; ein Anker genau auf dem letzten gelöschten Record muss dessen Hash
//! nennen, und das prüft `verify` weiter.

use std::time::SystemTime;

use serde_json::{Value, json};

use crate::record::{AuditRecord, MAX_LINE_BYTES};

/// Der Name der Art, wie er in `kind` steht.
pub const PRUNED_KIND: &str = "audit.pruned";

/// Die kleinste Grenze in Bytes, für die ein Lauf das Log immer unter sie
/// bringt: doppelt so groß wie [`MAX_LINE_BYTES`]. Der neueste Record bleibt
/// stehen, und mit höchstens [`MAX_LINE_BYTES`] passt er samt `audit.pruned`
/// und Anker darunter. Die Konfiguration nimmt für `audit.max_bytes` nichts
/// zwischen `0` und dieser Zahl an.
///
/// Das Anhängen selbst begrenzt die Länge einer Zeile nicht. Ist der neueste
/// Record länger als [`MAX_LINE_BYTES`], bleibt das Log mit ihm und dem
/// Beleg über der Grenze, und [`PruneOutcome::over`] sagt es.
pub const MIN_MAX_BYTES: u64 = 2 * MAX_LINE_BYTES;

/// `audit.pruned`: Ein Lauf von `audit.retention_days` oder `audit.max_bytes`
/// hat den Anfang der Kette gelöscht.
///
/// Der Record steht **hinter** dem Schnitt, am Ende der Kette, und ist wie
/// jeder andere gehasht und mit dem Schlüssel versiegelt; ohne den Schlüssel
/// lässt er sich nicht fälschen. Was in den gelöschten Records stand, nennt er
/// nicht: Hosts und Sitzungen der alten Records sind eben die Daten, die der
/// Lauf loswerden sollte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditPruned {
    /// Die Nummer des letzten gelöschten Records.
    pub through_seq: u64,
    /// Sein Hash: der `prev` des ersten Records, der bleibt.
    pub through_hash: String,
    /// Wie viele Records der Lauf gelöscht hat.
    pub records: u64,
    /// Die Grenze in der Zeit, falls der Lauf eine hatte: Gelöscht ist, was
    /// davor geschrieben wurde, nach [`crate::record::format_ts`]. Fehlt in `data`, wenn nur
    /// `audit.max_bytes` galt.
    pub cutoff: Option<String>,
    /// Die Grenze in Bytes, falls der Lauf eine hatte (`audit.max_bytes`).
    /// Fehlt in `data`, wenn nur `audit.retention_days` galt; ein solcher Lauf
    /// schreibt damit denselben Record wie vor HUM-194.
    pub max_bytes: Option<u64>,
}

impl AuditPruned {
    /// `data` dieses Records.
    #[must_use]
    pub fn data(&self) -> Value {
        let mut data = json!({
            "through_seq": self.through_seq,
            "through_hash": self.through_hash,
            "records": self.records,
        });
        if let Some(cutoff) = &self.cutoff {
            data["cutoff"] = json!(cutoff);
        }
        if let Some(max_bytes) = self.max_bytes {
            data["max_bytes"] = json!(max_bytes);
        }
        data
    }

    /// Ob `record` ein `audit.pruned` ist, der genau diesen Schnitt nennt:
    /// letzter gelöschter Record `through_seq` mit dem Hash `through_hash`.
    #[must_use]
    pub fn documents(record: &AuditRecord, through_seq: u64, through_hash: &str) -> bool {
        record.body.kind == PRUNED_KIND
            && record.body.data.get("through_seq").and_then(Value::as_u64) == Some(through_seq)
            && record.body.data.get("through_hash").and_then(Value::as_str) == Some(through_hash)
    }
}

/// Die Grenzen eines Laufs: in der Zeit (`audit.retention_days`) und in Bytes
/// (`audit.max_bytes`). Ohne beide löscht ein Lauf nichts und schreibt nichts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PruneLimits {
    /// Gelöscht wird, was vor diesem Zeitpunkt geschrieben wurde; `None` heißt
    /// keine Grenze in der Zeit.
    pub before: Option<SystemTime>,
    /// Die Datei soll nach dem Lauf höchstens so viele Bytes haben; `0` heißt
    /// keine Grenze in Bytes.
    pub max_bytes: u64,
}

impl PruneLimits {
    /// Ob der Lauf überhaupt eine Grenze hat.
    #[must_use]
    pub const fn is_bounded(&self) -> bool {
        self.before.is_some() || self.max_bytes > 0
    }
}

/// Was ein Lauf gelöscht hat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PruneReport {
    /// Die Nummer des letzten gelöschten Records.
    pub through_seq: u64,
    /// Wie viele Records gelöscht wurden.
    pub records: u64,
    /// Wie viele Bytes die Datei danach kürzer ist, ohne den Record des Laufs
    /// und seinen Anker.
    pub bytes: u64,
}

/// Was ein Lauf mit Grenzen ergab: der Schnitt, falls es einen gab, und die
/// Größe der Datei danach (HUM-194).
///
/// Die Größe steht auch ohne Schnitt da: Ein Log über `audit.max_bytes`, von
/// dem sich nichts schneiden ließ, soll sich melden können, statt still über
/// der Grenze zu bleiben.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PruneOutcome {
    /// Der Schnitt; `None`, wenn keine Grenze etwas zu löschen fand.
    pub cut: Option<PruneReport>,
    /// Wie groß die Datei nach dem Lauf ist, mit dem Record des Laufs und
    /// seinem Anker, falls einer geschrieben wurde. Liegt nur dann über
    /// `audit.max_bytes`, wenn der neueste Record allein samt Beleg nicht
    /// darunter passt.
    pub size: u64,
}

impl PruneOutcome {
    /// Ob die Datei nach dem Lauf über `max_bytes` liegt; bei `0` (keine
    /// Grenze) nie.
    #[must_use]
    pub const fn over(&self, max_bytes: u64) -> bool {
        max_bytes > 0 && self.size > max_bytes
    }
}
