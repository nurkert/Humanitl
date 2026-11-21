//! `Projects.open_in_app`: die Bitte von `humanitl app [PATH]` an die
//! verbundene Desktop-App (HUM-539, ADR-0024 Punkt 5, Frage 4).
//!
//! Läuft eine App, holt sie ihr Startfenster nach vorne und wählt den Ordner
//! vor; läuft keine, startet das CLI sie selbst. Der Weg zur laufenden App
//! ist der Strom `Subscribe`, den sie ohnehin hält: kein neuer Socket und
//! kein Mechanismus des Schreibtischs je Plattform. Eine App meldet sich mit
//! `SubscribeRequest.desktop_app`; nur ein solcher Strom bekommt einen
//! Empfänger dieses Rundfunks, und die Zahl der Empfänger ist damit die Zahl
//! der verbundenen Apps. Endet ihr Strom, fällt ihr Empfänger mit ihm.
//!
//! Die Bitte ist nur eine Vorwahl. Der Daemon startet nichts und speichert
//! nichts; ob der Pfad ein Ordner ist, sagt er der App erst auf
//! `Projects.proposal`, und gestartet wird erst nach „Start" im Fenster.

use std::path::Path;
use std::time::SystemTime;

use humanitl_core::Diagnostic;
use tokio::sync::broadcast;
use tokio_stream::StreamExt as _;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

use crate::{convert, v1};

/// So viele Bitten hält der Rundfunk für eine App, die gerade nicht liest.
const CAPACITY: usize = 8;

/// Länger ist kein Pfad, den der Daemon weitergibt.
const MAX_PATH_BYTES: usize = 4096;

/// Der Sender der Bitten; günstig zu klonen.
#[derive(Debug, Clone)]
pub struct AppRequests {
    sender: broadcast::Sender<v1::flow_event::OpenInApp>,
}

impl Default for AppRequests {
    fn default() -> Self {
        Self::new()
    }
}

impl AppRequests {
    /// Ein Sender ohne verbundene App.
    #[must_use]
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(CAPACITY);
        Self { sender }
    }

    /// Beantwortet `Projects.open_in_app`: gibt den Ordner an jede verbundene
    /// App und sagt in `app_reached`, ob eine da war.
    ///
    /// # Errors
    ///
    /// `CONFIG_003` für einen relativen oder zu langen Pfad und einen mit
    /// Steuerzeichen.
    pub fn open_in_app(
        &self,
        asked: &v1::projects_request::OpenInApp,
    ) -> Result<v1::ProjectsResponse, Diagnostic> {
        check_path(&asked.path)?;
        let reached = self
            .sender
            .send(v1::flow_event::OpenInApp {
                path: asked.path.clone(),
            })
            .is_ok();
        Ok(v1::ProjectsResponse {
            app_reached: reached,
            ..v1::ProjectsResponse::default()
        })
    }

    /// Die Bitten als Ereignisse des Stroms `Subscribe` einer App. Fällt die
    /// App zurück, gehen die verpassten verloren; der Mensch ruft
    /// `humanitl app` dann noch einmal.
    #[must_use]
    pub fn stream(&self) -> crate::server_stub::BoxStream<Result<v1::FlowEvent, tonic::Status>> {
        Box::pin(
            BroadcastStream::new(self.sender.subscribe()).filter_map(|item| match item {
                Ok(open) => Some(Ok(event(open))),
                Err(BroadcastStreamRecvError::Lagged(_)) => None,
            }),
        )
    }
}

/// Das Ereignis des Stroms zu einer Bitte.
#[must_use]
pub fn event(open: v1::flow_event::OpenInApp) -> v1::FlowEvent {
    v1::FlowEvent {
        at: Some(convert::timestamp(SystemTime::now())),
        event: Some(v1::flow_event::Event::OpenInApp(open)),
    }
}

/// Leer oder absolut, ohne Steuerzeichen und nicht länger als
/// [`MAX_PATH_BYTES`]. Ob es den Ordner gibt, prüft hier niemand: Das sagt
/// `Projects.proposal` der App, und die zeigt es als Befund.
fn check_path(path: &str) -> Result<(), Diagnostic> {
    if path.is_empty() {
        return Ok(());
    }
    // Steuerzeichen nie: Die App zeigt den Pfad in einer Benachrichtigung,
    // und ein Zeilenumbruch oder eine Escape-Folge darin malte dort etwas
    // anderes als den Ordner (HUM-539, Review).
    if path.len() > MAX_PATH_BYTES
        || path.chars().any(char::is_control)
        || !Path::new(path).is_absolute()
    {
        return Err(crate::projects::refused(format!(
            "open_in_app needs an absolute folder or nothing, not {}",
            humanitl_core::block::sanitize_note(path)
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn asked(path: &str) -> v1::projects_request::OpenInApp {
        v1::projects_request::OpenInApp {
            path: path.to_owned(),
        }
    }

    #[test]
    fn without_an_app_nobody_is_reached() {
        let requests = AppRequests::new();
        let answer = requests.open_in_app(&asked("/home/dev/api")).unwrap();
        assert!(!answer.app_reached);
    }

    #[tokio::test]
    async fn a_connected_app_gets_the_folder_and_is_reached() {
        let requests = AppRequests::new();
        let mut app = requests.stream();
        let answer = requests.open_in_app(&asked("/home/dev/api")).unwrap();
        assert!(answer.app_reached);
        let event = app.next().await.unwrap().unwrap();
        assert_eq!(
            event.event,
            Some(v1::flow_event::Event::OpenInApp(
                v1::flow_event::OpenInApp {
                    path: "/home/dev/api".to_owned()
                }
            ))
        );
    }

    #[test]
    fn an_app_that_left_is_not_reached() {
        let requests = AppRequests::new();
        drop(requests.stream());
        assert!(!requests.open_in_app(&asked("")).unwrap().app_reached);
    }

    #[test]
    fn a_relative_path_is_refused() {
        let requests = AppRequests::new();
        let refused = requests.open_in_app(&asked("api")).unwrap_err();
        assert_eq!(refused.code.as_str(), "CONFIG_003");
        assert!(requests.open_in_app(&asked("/a\0b")).is_err());
        for control in ["/a\nb", "/a\u{1b}[31mb", "/a\u{85}b"] {
            let refused = requests.open_in_app(&asked(control)).unwrap_err();
            assert_eq!(refused.code.as_str(), "CONFIG_003", "{control:?}");
        }
        // Ein Leerzeichen am Ende ist kein Steuerzeichen und gehört zum Ordner.
        assert!(requests.open_in_app(&asked("/home/dev/repo ")).is_ok());
    }
}
