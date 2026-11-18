//! `FlowEvent.config_changed`: die Auflösung der Konfiguration kann sich
//! geändert haben (HUM-534, Spezifikation 2c).
//!
//! Das Ereignis reist im bestehenden Strom `Subscribe` und nicht in einem
//! eigenen, aus demselben Grund wie `RulesChanged`: Ein Client braucht genau
//! ein Abonnement, und der Strom läuft auch ohne Sitzung (Startfenster, TUI).
//! Der Inhalt sind nur die betroffenen Schlüssel und, wenn eine Projektwahl
//! betroffen ist, die Kennung des Projekts; wer es sieht, lädt mit
//! `GetConfig` neu.
//!
//! Gesendet wird nach jedem erfolgreichen Schreiben, das die Auflösung ändern
//! kann: `SetConfig`, `Projects.save_choice`, ein `Projects.cleanup`, das eine
//! Wahl gelöscht hat, eine Änderung über `LlmServers`, `LlmKey`, `Egress` oder
//! `Agents`, und eine `config.toml`, die sich auf der Platte geändert hat
//! ([`watch_file`]; etwa `humanitl config set`, das die Datei selbst
//! schreibt, oder ein Editor).

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use tokio::sync::broadcast;
use tokio_stream::StreamExt as _;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tonic::Status;

use crate::{convert, v1};

/// So viele Änderungen hält der Rundfunk für einen langsamen Zuhörer.
const CAPACITY: usize = 64;

/// So oft schaut [`watch_file`] nach `config.toml`.
pub const WATCH_EVERY: Duration = Duration::from_secs(2);

/// Der Sender der Änderungen; günstig zu klonen.
#[derive(Debug, Clone)]
pub struct ConfigEvents {
    sender: broadcast::Sender<v1::flow_event::ConfigChanged>,
}

impl Default for ConfigEvents {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigEvents {
    /// Ein Sender ohne Zuhörer.
    #[must_use]
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(CAPACITY);
        Self { sender }
    }

    /// Meldet eine Änderung an jeden Zuhörer. Ohne Zuhörer geht sie verloren;
    /// wer später abonniert, liest den Stand ohnehin mit `GetConfig`.
    pub fn publish<I, K>(&self, keys: I, project_id: &str)
    where
        I: IntoIterator<Item = K>,
        K: Into<String>,
    {
        let _receivers = self.sender.send(v1::flow_event::ConfigChanged {
            keys: keys.into_iter().map(Into::into).collect(),
            project_id: project_id.to_owned(),
        });
    }

    /// Ein neuer Zuhörer.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<v1::flow_event::ConfigChanged> {
        self.sender.subscribe()
    }

    /// Die Änderungen als Ereignisse des Stroms `Subscribe`. Fällt ein
    /// Zuhörer zurück, bekommt er statt der verpassten eine Änderung ohne
    /// Schlüssel: Er lädt ohnehin alles neu.
    #[must_use]
    pub fn stream(&self) -> crate::server_stub::BoxStream<Result<v1::FlowEvent, Status>> {
        Box::pin(BroadcastStream::new(self.subscribe()).map(|item| {
            let changed = match item {
                Ok(changed) => changed,
                Err(BroadcastStreamRecvError::Lagged(_)) => {
                    v1::flow_event::ConfigChanged::default()
                }
            };
            Ok(event(changed))
        }))
    }
}

/// Das Ereignis des Stroms zu einer Änderung.
#[must_use]
pub fn event(changed: v1::flow_event::ConfigChanged) -> v1::FlowEvent {
    v1::FlowEvent {
        at: Some(convert::timestamp(SystemTime::now())),
        event: Some(v1::flow_event::Event::ConfigChanged(changed)),
    }
}

/// Was von einer Datei verglichen wird: Zeitpunkt der Änderung und Größe;
/// `None`, solange es sie nicht gibt.
fn stamp(path: &std::path::Path) -> Option<(SystemTime, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

/// Schaut alle [`WATCH_EVERY`] nach `path` und meldet eine Änderung ohne
/// Schlüssel, wenn sich Zeitpunkt oder Größe geändert haben, die Datei
/// entstanden oder verschwunden ist. Nur Metadaten, nie der Inhalt; der
/// laufende Daemon lädt dadurch nichts neu, nur die Clients.
///
/// Läuft, bis der Rückgabewert abgebrochen wird oder die Laufzeit endet.
#[must_use]
pub fn watch_file(events: ConfigEvents, path: PathBuf) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut last = stamp(&path);
        let mut ticks = tokio::time::interval(WATCH_EVERY);
        ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticks.tick().await;
            let now = stamp(&path);
            if now != last {
                last = now;
                events.publish(std::iter::empty::<String>(), "");
            }
        }
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[tokio::test]
    async fn a_listener_gets_the_keys_and_the_project() {
        let events = ConfigEvents::new();
        let mut first = events.subscribe();
        let mut second = events.subscribe();
        events.publish(["sandbox.mode"], "p1");
        for listener in [&mut first, &mut second] {
            let changed = listener.recv().await.unwrap();
            assert_eq!(changed.keys, vec!["sandbox.mode".to_owned()]);
            assert_eq!(changed.project_id, "p1");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_changed_file_is_reported_without_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let events = ConfigEvents::new();
        let mut listener = events.subscribe();
        let watcher = watch_file(events.clone(), path.clone());
        tokio::time::sleep(WATCH_EVERY * 2).await;
        assert!(listener.try_recv().is_err(), "nothing changed yet");
        std::fs::write(&path, "[hold]\ntimeout_secs = 30\n").unwrap();
        tokio::time::sleep(WATCH_EVERY * 2).await;
        let changed = listener.try_recv().expect("the new file is reported");
        assert!(changed.keys.is_empty());
        watcher.abort();
    }
}
