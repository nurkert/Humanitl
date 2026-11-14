//! Die Terminal-Tests aus `terminal.rs` mit dem Terminal im Strom-Modus
//! (HUM-443).
//!
//! Akzeptanzkriterium 1 des Issues: Beide Modi bestehen dieselben
//! Terminal-Tests. Statt die Fälle abzuschreiben, bindet diese Datei
//! `terminal.rs` als Modul ein; dort wählt `transport()` am Modulpfad den
//! Strom. Terminal, Größe, Lesen und Schreiben mehrerer Clients, Anhängen und
//! Lösen, Ctrl+C, weitere Shells und ihr Ende laufen damit über den
//! Steuerkanal statt über die Herrscherseite beim Daemon.

#[path = "terminal.rs"]
mod stream_cases;
