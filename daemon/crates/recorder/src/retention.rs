//! Die Aufbewahrung: wie lange eine Aufzeichnung bleibt (HUM-051).
//!
//! Die Mechanik des Löschens steht im Schreib-Thread ([`crate::writer`]): eine
//! Transaktion über `flows`, `messages`, `findings`, danach die Blobs, auf die
//! niemand mehr zeigt. Hier steht die **Politik** darüber, und sie besteht aus
//! genau einer Entscheidung: Was bedeutet die Zahl in
//! `recorder.retention_days`?
//!
//! Die Zahl ist eine Anzahl Tage, und `0` heißt **nie löschen**. Das ist keine
//! Geschmacksfrage: Ohne diesen Fall rechnet ein Aufräumlauf `jetzt − 0 Tage`
//! aus, bekommt `jetzt` heraus und löscht die ganze Aufzeichnung. Ein
//! Konfigurationswert, der beim Wert null das Gegenteil seiner Absicht tut,
//! ist die Art Falle, gegen die [`Retention`] steht. `humanitl-config` nimmt
//! `recorder.retention_days` von 0 bis 3650 an, Vorgabe 180 (HUM-051); diese
//! Crate kennt die Konfiguration nicht und bekommt die Zahl als Wert, also
//! steht die Bedeutung der Null hier und nicht dort.
//!
//! Den Record `recorder.retention_applied` schreibt nicht diese Crate, sondern
//! der Daemon (`humanitld`, `purge_once`): Die Aufzeichnung darf die
//! Audit-Crate nicht kennen (`backlog/CONVENTIONS.md` 3.1). Die Grenze, die er
//! vermerkt, ist [`Retention::horizon`].
//!
//! Die Audit-Kette fasst die Aufbewahrung der Aufzeichnung nicht an. Weder
//! `audit.jsonl` noch die Tabelle `audit_anchors` stehen in einer der
//! Anweisungen des Aufräumlaufs, und ein gelöschter Anker wäre ein Loch in
//! genau der Aussage, für die es die Tabelle gibt (`V7__audit_anchors.sql`).
//! `audit.retention_days` ist eine zweite, eigene Zahl; solange sie `0` ist,
//! wird an der Kette nichts gelöscht. Ihren Lauf macht der Daemon über den
//! Schreiber des Audit-Logs (HUM-157, `humanitl_audit::retention`), und auch
//! der lässt `audit_anchors` stehen.

use std::num::NonZeroU32;
use std::time::{Duration, SystemTime};

/// Ein Tag in Sekunden.
pub const DAY_SECS: u64 = 24 * 60 * 60;

/// Der Abstand zwischen zwei Aufräumläufen: einmal beim Start des Daemons,
/// danach täglich.
pub const RETENTION_INTERVAL: Duration = Duration::from_secs(DAY_SECS);

/// Wie lange eine Aufzeichnung aufgehoben wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retention {
    /// Es wird nichts gelöscht (`recorder.retention_days == 0`).
    Forever,
    /// Alles, was älter ist als so viele Tage, wird gelöscht.
    Days(NonZeroU32),
}

impl Retention {
    /// Liest die Zahl aus der Konfiguration; `0` heißt [`Retention::Forever`].
    #[must_use]
    pub const fn from_days(days: u32) -> Self {
        match NonZeroU32::new(days) {
            Some(days) => Self::Days(days),
            None => Self::Forever,
        }
    }

    /// Die Zahl, so wie sie in der Konfiguration steht; `0` für
    /// [`Retention::Forever`].
    #[must_use]
    pub const fn days(self) -> u32 {
        match self {
            Self::Forever => 0,
            Self::Days(days) => days.get(),
        }
    }

    /// Wahr, wenn nichts gelöscht wird.
    #[must_use]
    pub const fn is_forever(self) -> bool {
        matches!(self, Self::Forever)
    }

    /// Der Zeitpunkt, vor dem alles gelöscht wird, oder `None`, wenn nichts
    /// gelöscht wird.
    ///
    /// Liegt der Zeitpunkt vor der Epoche — eine lange Frist auf einer Uhr, die
    /// falsch geht —, ist es die Epoche selbst. Vor der Epoche liegt keine
    /// Aufzeichnung, der Lauf löscht dann also ohnehin nichts; die Grenze
    /// bleibt aber eine Zahl, die sich protokollieren und anzeigen lässt, statt
    /// einer negativen.
    #[must_use]
    pub fn horizon(self, now: SystemTime) -> Option<SystemTime> {
        let Self::Days(days) = self else {
            return None;
        };
        let back = Duration::from_secs(u64::from(days.get()) * DAY_SECS);
        let horizon = now.checked_sub(back).unwrap_or(SystemTime::UNIX_EPOCH);
        Some(horizon.max(SystemTime::UNIX_EPOCH))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use super::{DAY_SECS, RETENTION_INTERVAL, Retention};

    #[test]
    fn zero_days_means_forever_and_names_no_horizon() {
        let retention = Retention::from_days(0);
        assert_eq!(retention, Retention::Forever);
        assert!(retention.is_forever());
        assert_eq!(retention.days(), 0);
        assert_eq!(
            retention.horizon(SystemTime::now()),
            None,
            "a horizon of `now` would delete the whole recording"
        );
    }

    #[test]
    fn a_day_count_names_the_horizon_that_many_days_back() {
        let now = UNIX_EPOCH + Duration::from_secs(1_000 * DAY_SECS);
        let retention = Retention::from_days(180);
        assert!(!retention.is_forever());
        assert_eq!(retention.days(), 180);
        assert_eq!(
            retention.horizon(now),
            Some(UNIX_EPOCH + Duration::from_secs(820 * DAY_SECS))
        );
    }

    #[test]
    fn a_horizon_before_the_epoch_becomes_the_epoch() {
        let now = UNIX_EPOCH + Duration::from_secs(DAY_SECS);
        assert_eq!(Retention::from_days(3_650).horizon(now), Some(UNIX_EPOCH));
    }

    #[test]
    fn the_interval_is_one_day() {
        assert_eq!(RETENTION_INTERVAL, Duration::from_secs(24 * 60 * 60));
    }
}
