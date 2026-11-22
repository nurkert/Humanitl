//! Der Weg der Bilder auf das Terminal: das Backend von ratatui mit crossterm,
//! nur mit eigener Größe und eigener Ausgabe (GitHub #6).
//!
//! # Warum nicht `CrosstermBackend` allein
//!
//! Zwei Stellen von crossterm gehen an `/dev/tty` vorbei an dem, was dieser
//! Prozess als Terminal bekommen hat:
//!
//! - `crossterm::terminal::size` öffnet immer erst `/dev/tty`. Hat der Prozess
//!   ein anderes Terminal auf `stdin` als sein steuerndes (ein Test mit eigenem
//!   Pseudoterminal, ein `script`, ein Aufruf aus einem anderen Fenster), wäre
//!   jedes Bild für die falsche Größe gerechnet. [`TtyBackend`] liest die
//!   Größe deshalb wie der Rest von `humanitl` an `stdin` (`tty::window_size`).
//! - Der Leser der Tasten nimmt `/dev/tty`, wenn `stdin` kein Terminal ist.
//!   Das verhindert [`super::guard::TerminalGuard::enter`]: Ohne Terminal auf
//!   `stdin` und `stdout` entsteht gar kein Bildschirm.
//!
//! # Warum eine eigene Ausgabe
//!
//! [`TermOut`] sammelt ein Bild und schreibt es erst bei `flush`, und es
//! schreibt beim Fallenlassen **nichts**. Ein `BufWriter` täte das: Nach
//! einer Panik hat der Hook das Terminal schon zurückgegeben, und ein halbes
//! Bild, das danach noch aus einem Puffer fiele, stünde im gewöhnlichen
//! Bildschirm des Menschen.

use std::io::{self, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ratatui::backend::{Backend, ClearType, CrosstermBackend, WindowSize};
use ratatui::buffer::Cell;
use ratatui::layout::{Position, Size};

use crate::tty;

/// Die Ausgabe eines Bildschirms: eine eigene Kopie von `stdout`.
pub struct TermOut {
    /// Das Terminal.
    fd: OwnedFd,
    /// Was seit dem letzten `flush` geschrieben wurde.
    pending: Vec<u8>,
    /// Wahr, solange kein Bildschirm steht: Dann wird nichts geschrieben.
    /// Geteilt mit dem Wächter, der es setzt ([`TermOut::mute_switch`]).
    muted: Arc<AtomicBool>,
}

impl TermOut {
    /// Eine Ausgabe auf `fd`.
    #[must_use]
    pub fn new(fd: OwnedFd) -> Self {
        Self {
            fd,
            pending: Vec::new(),
            muted: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Der Schalter, der diese Ausgabe stumm macht.
    ///
    /// Gesetzt in dem Augenblick, in dem das Terminal zurückgegeben wird:
    /// ratatui zeigt beim Fallenlassen den Cursor wieder und schreibt dabei
    /// aus, was im Puffer liegt. Nach einer Panik mitten im Zeichnen wäre das
    /// ein halbes Bild im gewöhnlichen Bildschirm. Was stumm geschrieben
    /// wird, ist verworfen; nach dem Weiterlaufen zeichnet ratatui ohnehin
    /// alles neu.
    #[must_use]
    pub fn mute_switch(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.muted)
    }

    /// Ob die Ausgabe gerade stumm ist.
    fn muted(&self) -> bool {
        self.muted.load(Ordering::SeqCst)
    }
}

impl Write for TermOut {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if !self.muted() {
            self.pending.extend_from_slice(bytes);
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.muted() {
            self.pending.clear();
            return Ok(());
        }
        let result = write_all(self.fd.as_fd(), &self.pending);
        self.pending.clear();
        result
    }
}

/// Schreibt alles, auch wenn das Terminal es in Häppchen nimmt.
///
/// Ohne Zuteilung, damit der Panik-Hook denselben Weg nehmen kann.
pub fn write_all(fd: std::os::fd::BorrowedFd<'_>, mut bytes: &[u8]) -> io::Result<()> {
    while !bytes.is_empty() {
        match rustix::io::write(fd, bytes) {
            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
            Ok(count) => bytes = &bytes[count..],
            Err(rustix::io::Errno::INTR) => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

/// `CrosstermBackend` mit der Größe von `stdin` statt von `/dev/tty`.
pub struct TtyBackend {
    /// Alles außer der Größe.
    inner: CrosstermBackend<TermOut>,
}

impl TtyBackend {
    /// Ein Backend, das nach `out` zeichnet.
    #[must_use]
    pub const fn new(out: TermOut) -> Self {
        Self {
            inner: CrosstermBackend::new(out),
        }
    }
}

/// Die Größe des Terminals auf `stdin` in Zellen.
fn cells() -> Size {
    let size = super::size::Cells::from_window(tty::window_size());
    Size::new(size.cols, size.rows)
}

impl Backend for TtyBackend {
    type Error = io::Error;

    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        self.inner.draw(content)
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.inner.hide_cursor()
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.inner.show_cursor()
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        // Nur ein Viewport `Inline` fragt danach; die Bildschirme hier sind
        // ganzseitig. Die Antwort von crossterm käme über `/dev/tty`.
        Ok(Position::ORIGIN)
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        self.inner.set_cursor_position(position)
    }

    fn clear(&mut self) -> io::Result<()> {
        self.inner.clear()
    }

    fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
        self.inner.clear_region(clear_type)
    }

    fn size(&self) -> io::Result<Size> {
        Ok(cells())
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        Ok(WindowSize {
            columns_rows: cells(),
            pixels: Size::new(0, 0),
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        Backend::flush(&mut self.inner)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::io::{Read as _, Write as _};
    use std::os::unix::net::UnixStream;

    use super::TermOut;

    /// Erst `flush` schreibt, Fallenlassen schreibt nichts, und nach `mute`
    /// schreibt auch `flush` nichts mehr: Ein halbes Bild nach einer Panik
    /// bleibt im Puffer.
    #[test]
    fn nothing_leaves_without_flush() {
        let (mut reader, writer) = UnixStream::pair().expect("a socket pair");
        reader.set_nonblocking(true).expect("non-blocking");
        let mut out = TermOut::new(writer.into());
        out.write_all(b"half a frame").expect("buffered");
        let mut buffer = [0u8; 64];
        assert!(
            reader.read(&mut buffer).is_err(),
            "nothing is written before flush"
        );
        out.flush().expect("flushed");
        let read = reader.read(&mut buffer).expect("the frame");
        assert_eq!(&buffer[..read], b"half a frame");
        out.write_all(b"lost").expect("buffered");
        out.mute_switch()
            .store(true, std::sync::atomic::Ordering::SeqCst);
        out.flush().expect("a muted flush is no error");
        drop(out);
        let read = reader.read(&mut buffer).expect("end of the stream");
        assert_eq!(read, 0, "dropping wrote nothing");
    }
}
