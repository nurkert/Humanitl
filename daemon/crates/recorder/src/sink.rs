//! Der Mitschnitt einer streamenden Antwort.
//!
//! Antworten werden nie gepuffert und dann weitergegeben, sondern durchgereicht
//! und dabei gespiegelt (ADR-005). Der [`ResponseSink`] ist diese Spiegelung:
//! er hasht jedes Stück, hält den Anfang im Speicher und wechselt in eine
//! Temp-Datei, sobald der Anfang über `recorder.inline_max_bytes` hinauswüchse.
//! Sein Speicherbedarf bleibt damit bei der Inline-Grenze stehen, auch wenn ein
//! einzelnes Stück größer ist als sie.
//!
//! Über `limits.recorder_max_body_bytes` hinaus wird nichts mehr gespeichert.
//! Die Zeile trägt dann `truncated = 1` und in `size` weiterhin die volle
//! Länge, wie sie über die Leitung ging: die Aufzeichnung lügt nicht über die
//! Größe, nur über die Vollständigkeit, und sagt das.
//!
//! # Ein Mitschnitt endet immer mit einer Zeile
//!
//! Es gibt drei Enden, und alle drei schreiben:
//!
//! - [`ResponseSink::finish`], wenn die Antwort vollständig durchlief.
//! - [`ResponseSink::abort`], wenn der Client aufgab: `truncated = 1`.
//! - Das Fallenlassen des Sinks, wenn der Handler abbricht — abgebrochener
//!   Task, Panic, ein `?` auf dem Weg. Auch dann wird geschrieben, was bis
//!   dahin durchlief, ebenfalls mit `truncated = 1`.
//!
//! Der dritte Weg ist der Grund für `Drop`. Ohne ihn verschwände ein
//! angefangener Antwortkörper spurlos: keine Zeile, kein `truncated`, kein
//! Befund — und die Zusage „alles wird aufgezeichnet" wäre still gebrochen
//! (`backlog/CONVENTIONS.md` 4.13). Weil `Drop` nichts zurückgeben kann, geht
//! ein Fehler dort als [`Diagnostic`] in denselben Strom wie die Fehler des
//! Schreib-Threads.

use std::fs::File;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Arc;

use bytes::Bytes;
use humanitl_core::{BodyRef, Diagnostic, FlowId, HeaderMap};
use sha2::{Digest as _, Sha256};
use tokio::sync::broadcast;

use crate::blob::BlobStore;
use crate::error::{RecorderError, blob_failed};
use crate::message::{content_encoding_of, content_type_of, encode_headers};
use crate::settings::RecorderSettings;
use crate::types::Dir;
use crate::writer::{MessageWrite, WriterCmd, WriterHandle};

/// Der Mitschnitt einer Antwort, Stück für Stück.
///
/// Erzeugt von [`Recorder::begin_response`](crate::Recorder::begin_response).
pub struct ResponseSink {
    flow: FlowId,
    writer: Arc<WriterHandle>,
    blobs: Arc<BlobStore>,
    diagnostics: broadcast::Sender<Diagnostic>,
    settings: RecorderSettings,
    headers_json: String,
    content_type: Option<String>,
    content_encoding: Option<String>,
    hasher: Sha256,
    buffer: Vec<u8>,
    spill: Option<(File, PathBuf)>,
    size: u64,
    stored: u64,
    truncated: bool,
    failed: Option<RecorderError>,
    done: bool,
}

impl core::fmt::Debug for ResponseSink {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ResponseSink")
            .field("flow", &self.flow)
            .field("size", &self.size)
            .field("stored", &self.stored)
            .field("truncated", &self.truncated)
            .field("spilled", &self.spill.is_some())
            .field("done", &self.done)
            .finish_non_exhaustive()
    }
}

/// Schreibt, was bis zum Abbruch durchlief, statt es wegzuwerfen.
///
/// Läuft nur, wenn weder [`ResponseSink::finish`] noch [`ResponseSink::abort`]
/// gelaufen sind; beide setzen `done`. Siehe Modulkommentar.
impl Drop for ResponseSink {
    fn drop(&mut self) {
        if self.done {
            return;
        }
        self.truncated = true;
        if let Err(err) = self.close(None) {
            let diagnostic = err.into_diagnostic();
            tracing::warn!(
                code = diagnostic.code.as_str(),
                why = %diagnostic.why,
                "recorder lost a response body"
            );
            let _ignored = self.diagnostics.send(diagnostic);
        }
    }
}

impl ResponseSink {
    /// Beginnt den Mitschnitt einer Antwort.
    pub(crate) fn new(
        flow: FlowId,
        writer: Arc<WriterHandle>,
        blobs: Arc<BlobStore>,
        diagnostics: broadcast::Sender<Diagnostic>,
        settings: RecorderSettings,
        headers: &HeaderMap,
    ) -> Self {
        Self {
            flow,
            writer,
            blobs,
            diagnostics,
            settings,
            headers_json: encode_headers(headers),
            content_type: content_type_of(headers),
            content_encoding: content_encoding_of(headers),
            hasher: Sha256::new(),
            buffer: Vec::new(),
            spill: None,
            size: 0,
            stored: 0,
            truncated: false,
            failed: None,
            done: false,
        }
    }

    /// Nimmt ein Stück des Antwortkörpers auf.
    ///
    /// Der Puffer wächst nie über `recorder.inline_max_bytes` hinaus: passt ein
    /// Stück nicht mehr hinein, wandert erst der Puffer in die Temp-Datei und
    /// dann das Stück hinterher. Ein einzelnes Stück von vier Mebibyte kostet
    /// deshalb keine vier Mebibyte Speicher.
    ///
    /// Fehler beim Schreiben der Temp-Datei bleiben hier stehen und kommen bei
    /// [`ResponseSink::finish`] heraus: eine Antwort wird nicht abgebrochen,
    /// weil die Aufzeichnung stolpert.
    pub fn chunk(&mut self, chunk: &[u8]) {
        self.size = self
            .size
            .saturating_add(u64::try_from(chunk.len()).unwrap_or(u64::MAX));
        if self.failed.is_some() {
            return;
        }

        let room = self.settings.max_body_bytes.saturating_sub(self.stored);
        if room == 0 {
            if !chunk.is_empty() {
                self.truncated = true;
            }
            return;
        }
        let take = usize::try_from(room).unwrap_or(usize::MAX).min(chunk.len());
        if take < chunk.len() {
            self.truncated = true;
        }
        let Some(data) = chunk.get(..take) else {
            return;
        };
        if data.is_empty() {
            return;
        }

        self.hasher.update(data);
        self.stored = self
            .stored
            .saturating_add(u64::try_from(take).unwrap_or(u64::MAX));

        // Erst prüfen, dann puffern: sonst läge das Stück im Speicher, bevor
        // jemand merkt, dass es dort nicht hingehört.
        if self.spill.is_none() {
            let would_be =
                u64::try_from(self.buffer.len().saturating_add(data.len())).unwrap_or(u64::MAX);
            if would_be > self.settings.inline_max_bytes {
                self.start_spill();
            }
        }

        if let Some((file, path)) = self.spill.as_mut() {
            if let Err(err) = file.write_all(data) {
                self.failed = Some(blob_failed(format!(
                    "could not append to the response blob {} ({err})",
                    path.display()
                )));
            }
            return;
        }

        self.buffer.extend_from_slice(data);
    }

    /// Wie viele Bytes der Mitschnitt gerade im Speicher hält.
    ///
    /// Bleibt bei `recorder.inline_max_bytes` stehen; danach liegt alles in
    /// einer Temp-Datei. Der Test `response_sink_streaming` prüft damit, dass
    /// zehn Mebibyte nicht zehn Mebibyte Speicher kosten.
    #[must_use]
    pub fn buffered_bytes(&self) -> usize {
        self.buffer.capacity()
    }

    /// Wie viele Bytes bisher durchliefen.
    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }

    /// Schließt den Mitschnitt ab und schreibt die Zeile.
    ///
    /// `status` ist der Status der Antwort. Er wird in `flows.status`
    /// geschrieben und überschreibt dabei, was `FlowEvent::ResponseHeaders`
    /// dort schon eingetragen hat — beide nennen denselben Wert. Nur
    /// [`ResponseSink::abort`] und der Abbruch lassen die Spalte unberührt,
    /// weil sie keinen Status kennen.
    ///
    /// # Errors
    ///
    /// [`RecorderError::Blob`], wenn die Temp-Datei nicht geschrieben oder
    /// nicht umbenannt werden konnte, [`RecorderError::Storage`], wenn der
    /// Schreib-Thread die Zeile nicht mehr annimmt.
    pub fn finish(mut self, status: u16) -> Result<BodyRef, RecorderError> {
        self.close(Some(status))
    }

    /// Bricht den Mitschnitt ab: der Client hat aufgegeben.
    ///
    /// Was bis dahin durchlief, wird als gekürzte Antwort aufgezeichnet
    /// (`truncated = 1`).
    ///
    /// # Errors
    ///
    /// Wie [`ResponseSink::finish`].
    pub fn abort(mut self) -> Result<BodyRef, RecorderError> {
        self.truncated = true;
        self.close(None)
    }

    /// Der gemeinsame Abschluss aller drei Enden, siehe Modulkommentar.
    fn close(&mut self, status: Option<u16>) -> Result<BodyRef, RecorderError> {
        if self.done {
            return Err(crate::error::storage_failed(format!(
                "the response of the flow {} was already closed",
                self.flow
            )));
        }
        self.done = true;

        if let Some(err) = self.failed.take() {
            if let Some((file, path)) = self.spill.take() {
                drop(file);
                let _ignored = std::fs::remove_file(path);
            }
            return Err(err);
        }

        let sha256: [u8; 32] = core::mem::take(&mut self.hasher).finalize().into();
        let (inline, blob) = match self.spill.take() {
            Some((file, path)) => {
                // Der Blob wird angemeldet, bevor er entsteht: sonst könnte ein
                // Aufräumlauf zwischen Datei und Zeile die Datei für verwaist
                // halten (siehe `Writer::drop_unreferenced`).
                self.writer.send(WriterCmd::ReserveBlob(sha256))?;
                self.blobs.put_temp(&sha256, file, &path)?;
                (None, Some(sha256))
            }
            None => (Some(Bytes::from(core::mem::take(&mut self.buffer))), None),
        };

        let write = MessageWrite {
            flow: self.flow,
            dir: Dir::Response,
            headers_json: core::mem::take(&mut self.headers_json),
            content_type: self.content_type.clone(),
            content_encoding: self.content_encoding.take(),
            inline: inline.clone(),
            blob,
            size: self.size,
            truncated: self.truncated,
            status,
        };
        self.writer.send(WriterCmd::Message(Box::new(write)))?;

        Ok(BodyRef {
            sha256,
            size: self.size,
            inline,
            content_type: self.content_type.take(),
            truncated: self.truncated,
        })
    }

    /// Wechselt vom Speicher in eine Temp-Datei.
    fn start_spill(&mut self) {
        match self.blobs.temp_staging() {
            Ok((mut file, path)) => {
                if let Err(err) = file.write_all(&self.buffer) {
                    self.failed = Some(blob_failed(format!(
                        "could not spill the response body to {} ({err})",
                        path.display()
                    )));
                    drop(file);
                    let _ignored = std::fs::remove_file(&path);
                    return;
                }
                self.buffer = Vec::new();
                self.spill = Some((file, path));
            }
            Err(err) => self.failed = Some(err),
        }
    }
}
