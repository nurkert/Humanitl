//! Ein Record der Kette: Felder, Hash, MAC und die Zeile in der Datei.
//!
//! Der Hash deckt sechs Felder, der MAC den Hash:
//!
//! ```text
//! hash = SHA-256( canonical_json({ data, kind, prev, seq, session, ts }) )
//! mac  = HMAC-SHA256( key, hash als 32 Bytes )
//! ```
//!
//! Beide stehen als Hex in Kleinbuchstaben im Record. Die Zeile selbst ist die
//! kanonische Form aller acht Felder, also nach Schlüsseln sortiert
//! (`data`, `hash`, `kind`, `mac`, `prev`, `seq`, `session`, `ts`). Damit ist
//! auch die Datei kanonisch, und die Prüfung kann jede Zeile Byte für Byte
//! nachbauen.

use chrono::{DateTime, Utc};
use hmac::{Hmac, KeyInit as _, Mac as _};
use serde_json::{Map, Value};
use sha2::{Digest as _, Sha256};

use crate::canonical::{CanonicalError, canonical_json};

/// Länge eines Hashes oder MACs in Hex-Zeichen.
pub const HASH_HEX_LEN: usize = 64;

/// `prev` des ersten Records: 64 Nullen.
pub const GENESIS_PREV: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// `session` eines Records, der zu keiner Sitzung gehört.
pub const NO_SESSION: &str = "-";

/// Das Format jedes Zeitstempels im Log: UTC, Mikrosekunden, immer `Z`.
///
/// Ausdrücklich und nicht über `serde`: `chrono` schreibt je nach Feature mal
/// mit, mal ohne Nanosekunden, und ein Zeitstempel, der sich mit einer
/// Abhängigkeit ändert, änderte den Hash.
pub const TS_FORMAT: &str = "%Y-%m-%dT%H:%M:%S%.6fZ";

/// Ein Zeitpunkt in der Form, die im Log steht.
#[must_use]
pub fn format_ts(at: DateTime<Utc>) -> String {
    at.format(TS_FORMAT).to_string()
}

/// SHA-256 über `bytes` als Hex in Kleinbuchstaben.
///
/// Dafür da, dass ein Wert, der ein Geheimnis tragen kann (ein Pfad mit
/// Query, ein Arbeitsverzeichnis, eine Kommandozeile), nur als Prüfsumme ins
/// Log kommt.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Die sechs Felder, über die der Hash läuft; noch ohne Hash und MAC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordBody {
    /// Laufende Nummer, beginnt bei 1, lückenlos.
    pub seq: u64,
    /// Zeitpunkt nach [`TS_FORMAT`].
    pub ts: String,
    /// Die Sitzung als UUID oder [`NO_SESSION`].
    pub session: String,
    /// Die Art, zum Beispiel `flow.decided` (siehe [`crate::kinds`]).
    pub kind: String,
    /// Die Daten der Art; ein Objekt aus Ganzzahlen, Strings, Booleans,
    /// Listen und `null`.
    pub data: Value,
    /// Der Hash des Vorgängers, für `seq == 1` [`GENESIS_PREV`].
    pub prev: String,
}

impl RecordBody {
    /// Der Wert, über den der Hash läuft.
    fn hashed_value(&self) -> Value {
        let mut map = Map::with_capacity(6);
        map.insert("data".to_owned(), self.data.clone());
        map.insert("kind".to_owned(), Value::String(self.kind.clone()));
        map.insert("prev".to_owned(), Value::String(self.prev.clone()));
        map.insert("seq".to_owned(), Value::from(self.seq));
        map.insert("session".to_owned(), Value::String(self.session.clone()));
        map.insert("ts".to_owned(), Value::String(self.ts.clone()));
        Value::Object(map)
    }

    /// Der Hash dieser Felder als 32 Bytes.
    ///
    /// # Errors
    ///
    /// [`CanonicalError::Float`], wenn in `data` ein Float steht.
    pub fn hash(&self) -> Result<[u8; 32], CanonicalError> {
        let bytes = canonical_json(&self.hashed_value())?;
        Ok(Sha256::digest(&bytes).into())
    }

    /// Hash und MAC dazu: der fertige Record.
    ///
    /// # Errors
    ///
    /// [`CanonicalError::Float`], wenn in `data` ein Float steht.
    pub fn seal(self, key: &[u8; 32]) -> Result<AuditRecord, CanonicalError> {
        let hash = self.hash()?;
        Ok(AuditRecord {
            hash: hex::encode(hash),
            mac: mac_hex(key, &hash),
            body: self,
        })
    }
}

/// Ein fertiger Record: die sechs Felder, ihr Hash und der MAC darüber.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRecord {
    /// Die sechs Felder, über die der Hash läuft.
    pub body: RecordBody,
    /// Der Hash als Hex; stimmt nur, wenn niemand etwas geändert hat.
    pub hash: String,
    /// Der MAC als Hex.
    pub mac: String,
}

/// Warum eine Zeile kein Record ist.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LineError {
    /// Die Zeile ist kein JSON.
    #[error("the line is not JSON: {0}")]
    NotJson(String),
    /// Die Zeile ist JSON, hat aber nicht die acht Felder eines Records.
    #[error("the line is not an audit record: {0}")]
    Shape(&'static str),
}

impl AuditRecord {
    /// Die Zeile ohne abschließendes `\n`, in kanonischer Form.
    ///
    /// # Errors
    ///
    /// [`CanonicalError::Float`], wenn in `data` ein Float steht.
    pub fn to_line(&self) -> Result<Vec<u8>, CanonicalError> {
        let Value::Object(mut map) = self.body.hashed_value() else {
            return Ok(Vec::new());
        };
        map.insert("hash".to_owned(), Value::String(self.hash.clone()));
        map.insert("mac".to_owned(), Value::String(self.mac.clone()));
        canonical_json(&Value::Object(map))
    }

    /// Liest eine Zeile (ohne `\n`) als Record.
    ///
    /// Geprüft wird hier nur die Form: genau die acht Felder mit ihren Typen.
    /// Ob Hash, MAC und Kette stimmen, entscheidet die Prüfung, die dafür die
    /// Gründe kennt.
    ///
    /// # Errors
    ///
    /// [`LineError`], wenn die Zeile kein JSON oder kein Record ist.
    pub fn from_line(line: &[u8]) -> Result<Self, LineError> {
        let value: Value =
            serde_json::from_slice(line).map_err(|err| LineError::NotJson(err.to_string()))?;
        let Value::Object(mut map) = value else {
            return Err(LineError::Shape("not an object"));
        };
        if map.len() != 8 {
            return Err(LineError::Shape("a record has exactly eight fields"));
        }
        let mut text = |name: &'static str| match map.remove(name) {
            Some(Value::String(text)) => Ok(text),
            _ => Err(LineError::Shape(name)),
        };
        let ts = text("ts")?;
        let session = text("session")?;
        let kind = text("kind")?;
        let prev = text("prev")?;
        let hash = text("hash")?;
        let mac = text("mac")?;
        let seq = map
            .remove("seq")
            .and_then(|seq| seq.as_u64())
            .ok_or(LineError::Shape("seq"))?;
        let Some(data @ Value::Object(_)) = map.remove("data") else {
            return Err(LineError::Shape("data"));
        };
        Ok(Self {
            body: RecordBody {
                seq,
                ts,
                session,
                kind,
                data,
                prev,
            },
            hash,
            mac,
        })
    }

    /// Der Hash als 32 Bytes, falls das Feld Hex der richtigen Länge ist.
    #[must_use]
    pub fn hash_bytes(&self) -> Option<[u8; 32]> {
        decode32(&self.hash)
    }
}

/// HMAC-SHA256 mit `key` über die 32 Bytes eines Hashes, als Hex.
#[must_use]
pub fn mac_hex(key: &[u8; 32], hash: &[u8; 32]) -> String {
    let mut mac = hmac_with(key);
    mac.update(hash);
    hex::encode(mac.finalize().into_bytes())
}

/// Prüft einen MAC in konstanter Zeit.
///
/// Ein Feld, das kein Hex der richtigen Länge ist, ist ein falscher MAC.
#[must_use]
pub fn mac_matches(key: &[u8; 32], hash: &[u8; 32], mac: &str) -> bool {
    let Some(expected) = decode32(mac) else {
        return false;
    };
    let mut check = hmac_with(key);
    check.update(hash);
    check.verify_slice(&expected).is_ok()
}

fn hmac_with(key: &[u8; 32]) -> Hmac<Sha256> {
    match Hmac::<Sha256>::new_from_slice(key) {
        Ok(mac) => mac,
        // HMAC nimmt Schlüssel jeder Länge (RFC 2104, 2); `new_from_slice`
        // kann für ihn nicht scheitern, der Typ muss es nur allgemein sagen.
        Err(_) => unreachable!("HMAC accepts a key of any length"),
    }
}

/// Liest 64 Hex-Zeichen in Kleinbuchstaben als 32 Bytes.
///
/// Großbuchstaben gelten nicht: Die Datei ist kanonisch, und ein `A` statt
/// eines `a` wäre dieselbe Zahl in einer anderen Zeile.
fn decode32(text: &str) -> Option<[u8; 32]> {
    if text.len() != HASH_HEX_LEN || text.bytes().any(|byte| byte.is_ascii_uppercase()) {
        return None;
    }
    let mut out = [0_u8; 32];
    hex::decode_to_slice(text, &mut out).ok()?;
    Some(out)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use serde_json::json;

    use super::{AuditRecord, GENESIS_PREV, NO_SESSION, RecordBody, mac_hex, mac_matches};

    fn body() -> RecordBody {
        RecordBody {
            seq: 1,
            ts: "2026-09-02T10:00:00.123456Z".to_owned(),
            session: NO_SESSION.to_owned(),
            kind: "daemon.started".to_owned(),
            data: json!({"version": "0.0.0"}),
            prev: GENESIS_PREV.to_owned(),
        }
    }

    #[test]
    fn a_line_round_trips_and_is_sorted() {
        let record = body().seal(&[7; 32]).unwrap();
        let line = record.to_line().unwrap();
        let text = String::from_utf8(line.clone()).unwrap();
        let order: Vec<usize> = [
            "\"data\"",
            "\"hash\"",
            "\"kind\"",
            "\"mac\"",
            "\"prev\"",
            "\"seq\"",
            "\"session\"",
            "\"ts\"",
        ]
        .iter()
        .map(|key| text.find(key).unwrap())
        .collect();
        assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{text}");
        assert_eq!(AuditRecord::from_line(&line).unwrap(), record);
    }

    #[test]
    fn a_line_with_a_ninth_field_is_no_record() {
        let record = body().seal(&[7; 32]).unwrap();
        let mut text = String::from_utf8(record.to_line().unwrap()).unwrap();
        text.insert_str(1, "\"extra\":1,");
        assert!(AuditRecord::from_line(text.as_bytes()).is_err());
    }

    #[test]
    fn the_mac_depends_on_the_key_and_is_checked_in_full() {
        let hash = body().hash().unwrap();
        let mac = mac_hex(&[1; 32], &hash);
        assert!(mac_matches(&[1; 32], &hash, &mac));
        assert!(!mac_matches(&[2; 32], &hash, &mac));
        assert!(!mac_matches(&[1; 32], &hash, &mac[..62]));
        assert!(!mac_matches(&[1; 32], &hash, &mac.to_uppercase()));
    }
}
