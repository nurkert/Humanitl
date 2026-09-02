//! Werttypen des Kerns: typisierte IDs, Flow-Zustandsautomat, Regeln, Findings,
//! Diagnostics. Kein IO, kein async, kein Protobuf.
//!
//! Siehe `docs/ARCHITECTURE.md` für die Schichtung und `backlog/CONVENTIONS.md`
//! Abschnitt 3.1 für die erlaubten Abhängigkeiten dieser Crate.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

#[cfg(test)]
mod tests {
    #[test]
    fn workspace_builds() {}
}
