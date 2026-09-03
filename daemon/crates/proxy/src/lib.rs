//! Moderierender MITM-Proxy: CA, Hold-Queue, Egress-Port, Flow-Ablauf.
//!
//! In diesem Commit steht davon die CA (HUM-014): sie legt die lokale
//! Zertifizierungsstelle an, stellt Leaf-Zertifikate je Host aus und schreibt
//! das Bundle, das der Launcher in die Sandbox einhängt.
//!
//! Siehe `docs/ARCHITECTURE.md` für die Schichtung und `backlog/CONVENTIONS.md`
//! Abschnitt 3.1 für die erlaubten Abhängigkeiten dieser Crate.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod ca;
