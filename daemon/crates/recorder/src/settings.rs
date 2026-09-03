//! Die Grenzen, nach denen die Aufzeichnung arbeitet.
//!
//! Die Werte stammen aus der Konfiguration (`recorder.inline_max_bytes`,
//! `limits.recorder_max_body_bytes`, `recorder.retention_days`), aber diese
//! Crate darf `humanitl-config` nicht kennen: `tools/deps-allow.toml` erlaubt
//! ihr nur `humanitl-core`. Der Daemon rechnet die Konfiguration deshalb beim
//! Start in diesen Typ um; die Vorgaben hier sind dieselben wie dort, damit ein
//! Test ohne Konfiguration dasselbe Verhalten sieht wie der Daemon.

/// Ein Kibibyte.
const KIB: u64 = 1024;

/// Ein Mebibyte.
const MIB: u64 = 1024 * KIB;

/// Grenzen der Aufzeichnung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecorderSettings {
    /// Bodies bis zu dieser Größe stehen in der Datenbank, größere als Datei
    /// im Blob-Speicher (`recorder.inline_max_bytes`).
    pub inline_max_bytes: u64,
    /// Größter Body, den die Aufzeichnung überhaupt ablegt
    /// (`limits.recorder_max_body_bytes`). Alles darüber wird gekürzt
    /// gespeichert und in `messages.truncated` vermerkt.
    pub max_body_bytes: u64,
    /// Tage, die eine Aufzeichnung aufgehoben wird (`recorder.retention_days`).
    pub retention_days: u32,
}

impl Default for RecorderSettings {
    fn default() -> Self {
        Self {
            inline_max_bytes: 256 * KIB,
            max_body_bytes: 32 * MIB,
            retention_days: 90,
        }
    }
}

impl RecorderSettings {
    /// Grenzen mit ausdrücklichen Werten.
    #[must_use]
    pub const fn new(inline_max_bytes: u64, max_body_bytes: u64, retention_days: u32) -> Self {
        Self {
            inline_max_bytes,
            max_body_bytes,
            retention_days,
        }
    }

    /// Die Werte, so wie sie tatsächlich gelten.
    ///
    /// `inline_max_bytes` liegt nie über `max_body_bytes` und nie bei null:
    /// beides wäre eine Konfiguration, die die Aufzeichnung stillegte, und die
    /// Aufzeichnung fällt nicht still aus, sie rückt auf den nächsten
    /// sinnvollen Wert. Die Konfiguration prüft dieselbe Bedingung schon beim
    /// Laden (`recorder_is_well_formed`); dies hier ist der Boden darunter.
    #[must_use]
    pub const fn normalized(self) -> Self {
        let max_body_bytes = if self.max_body_bytes == 0 {
            1
        } else {
            self.max_body_bytes
        };
        let inline_max_bytes = if self.inline_max_bytes == 0 {
            1
        } else if self.inline_max_bytes > max_body_bytes {
            max_body_bytes
        } else {
            self.inline_max_bytes
        };
        Self {
            inline_max_bytes,
            max_body_bytes,
            retention_days: self.retention_days,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::RecorderSettings;

    #[test]
    fn defaults_match_the_configuration() {
        let settings = RecorderSettings::default();
        assert_eq!(settings.inline_max_bytes, 256 * 1024);
        assert_eq!(settings.max_body_bytes, 32 * 1024 * 1024);
        assert_eq!(settings.retention_days, 90);
    }

    #[test]
    fn inline_never_exceeds_the_body_cap_and_never_reaches_zero() {
        let settings = RecorderSettings::new(1_000, 100, 7).normalized();
        assert_eq!(settings.inline_max_bytes, 100);
        let settings = RecorderSettings::new(0, 0, 7).normalized();
        assert_eq!(settings.inline_max_bytes, 1);
        assert_eq!(settings.max_body_bytes, 1);
    }
}
