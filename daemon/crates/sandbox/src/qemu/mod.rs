//! Die Kernel-Stufe als eigenes Backend: eine QEMU-micro-VM mit eigenem
//! Gastkernel aus Debian-Paketen (HUM-464, ADR-0021).
//!
//! Bis HUM-444 gibt es hier keinen Adapter, nur die Prüfung der
//! Voraussetzungen ([`missing_prerequisite`]): Die Liste der Backends nennt
//! damit, was auf diesem Rechner fehlt, und ein Start mit
//! [`crate::Backend::detect_unadmitted`] scheitert mit genau diesem Befund,
//! bevor er sagt, dass der Adapter fehlt. Gestartet wird nichts, weder eine
//! VM noch ein Hilfsprozess; gefragt wird nur die Hilfe der Programme.
//!
//! Die Namen `qemu`, `virtiofsd`, `vhost-device-vsock` und `microvm` stehen
//! nur in diesem Verzeichnis (`[qemu]` in `tools/coupling-baseline.toml`).
//! Außerhalb heißt die Stufe Kernel-Stufe oder VM-Backend.

mod prerequisites;

pub(crate) use prerequisites::missing_prerequisite;
