//! Die Ladeanimation von `humanitl go` (HUM-496): die Faust des Logos, gelb
//! und in die Tiefe gezogen, dreht sich, solange die Wegwerf-Sitzung startet.
//!
//! # Wann sie erscheint
//!
//! Nur wenn Ausgabe und Fehlerausgabe ein Terminal sind, ohne `--json` und
//! mit `terminal.splash = true` (Vorgabe); mit `NO_COLOR` ohne Farbe
//! ([`wanted`], [`palette`]). Gezeichnet wird auf `stderr`, im alternativen
//! Bildschirm, damit der Verlauf des Terminals danach unverändert dasteht.
//!
//! # Wie lange
//!
//! Mindestens [`MINIMUM`], und nicht länger, als der Start ohnehin dauert:
//! Sobald die Sitzung läuft, steht sie höchstens noch den Rest der
//! Mindestzeit ([`remaining`], [`Splash::rest`]) und gibt das Terminal dann
//! frei; Ereignisse und Signale liest der Befehl währenddessen weiter.
//! Scheitert der Start, endet sie sofort ([`Splash::stop`]), damit der Befund
//! auf einem gewöhnlichen Terminal steht.
//!
//! # Das Terminal zurück, auf jedem Weg
//!
//! Die Animation läuft in einem eigenen Thread, damit das Rechnen der Bilder
//! keinen Arbeiter der Laufzeit blockiert. Alles, was sie auf das Terminal
//! schreibt, schreibt dieser Thread: den Eintritt, die Bilder und zuletzt
//! [`RESTORE`]. [`Splash::stop`] setzt nur ein Zeichen und wartet höchstens
//! [`STOP_PATIENCE`]; ein Terminal mit angehaltener Ausgabe hält damit den
//! Thread auf, aber nie den Befehl, der die Sitzung stoppen muss. `Drop` ruft
//! `stop`: Ein Fehlerpfad, ein frühes `return` und das Abwickeln einer Panik
//! beenden die Animation damit. Den Rohmodus setzt sie nicht; `Ctrl+C` bleibt
//! ein Signal, und der Befehl, der sie zeigt, beendet sie, bevor er auf das
//! Signal antwortet.

mod fist;
pub mod render;

use std::fmt::Write as _;
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub use render::Palette;

/// So lange steht die Animation mindestens.
pub const MINIMUM: Duration = Duration::from_secs(1);

/// Abstand der Bilder: etwa 30 je Sekunde.
const FRAME: Duration = Duration::from_millis(33);

/// Eine volle Drehung dauert so lange.
const TURN_SECS: f32 = 3.0;

/// Größtes Bild in Zeichen; ein größeres kostet nur Bandbreite.
const MAX_COLS: usize = 64;
/// Größte Höhe in Zeilen, ohne die Zeile darunter.
const MAX_ROWS: usize = 30;

/// Was die Animation betritt: alternativer Bildschirm, Cursor aus, leer.
const ENTER: &str = "\x1b[?1049h\x1b[?25l\x1b[2J";
/// Was sie zurückgibt: Farben, Cursor, normaler Bildschirm.
pub const RESTORE: &str = "\x1b[0m\x1b[?25h\x1b[?1049l";

/// Die Zeile unter der Faust.
const CAPTION: &str = "humanitl · starting a disposable session";

/// Ob die Animation erscheinen soll.
///
/// `setting` ist `terminal.splash`, `json` der Schalter `--json`,
/// `terminals`, ob `stdout` und `stderr` beide ein Terminal sind.
#[must_use]
pub const fn wanted(setting: bool, json: bool, terminals: bool) -> bool {
    setting && !json && terminals
}

/// Die Palette nach `NO_COLOR` (no-color.org): gesetzt und nicht leer heißt
/// ohne Farbe.
#[must_use]
pub fn palette(no_color: Option<&std::ffi::OsStr>) -> Palette {
    match no_color {
        Some(value) if !value.is_empty() => Palette::Mono,
        _ => Palette::TrueColor,
    }
}

/// Wie lange die Animation noch stehen muss, wenn sie bei `started` begann
/// und es jetzt `now` ist.
#[must_use]
pub fn remaining(started: Instant, now: Instant, minimum: Duration) -> Duration {
    minimum.saturating_sub(now.saturating_duration_since(started))
}

/// Der Drehwinkel nach `elapsed`.
#[must_use]
pub fn angle(elapsed: Duration) -> f32 {
    elapsed.as_secs_f32() / TURN_SECS * std::f32::consts::TAU
}

/// Ein ganzes Bild für ein Terminal mit `cols` × `rows` Zeichen: die Faust
/// mittig, darunter die Zeile [`CAPTION`], jede Zeile an ihrem Platz.
#[must_use]
pub fn screen(
    model: &render::Model,
    elapsed: Duration,
    cols: usize,
    rows: usize,
    palette: Palette,
) -> String {
    let height = rows.saturating_sub(2).min(MAX_ROWS);
    let width = cols.min(MAX_COLS).min(height.saturating_mul(2).max(1));
    let top = (rows.saturating_sub(height + 2)) / 2 + 1;
    let left = cols.saturating_sub(width) / 2 + 1;
    let mut out = String::new();
    for (index, line) in render::frame(model, angle(elapsed), width, height, palette)
        .iter()
        .enumerate()
    {
        let _ = write!(out, "\x1b[{};{left}H{line}", top + index);
    }
    let caption: String = CAPTION.chars().take(cols).collect();
    let caption_left = cols.saturating_sub(caption.chars().count()) / 2 + 1;
    let _ = write!(out, "\x1b[{};{caption_left}H", top + height + 1);
    match palette {
        Palette::TrueColor => {
            let (r, g, b) = render::BRAND_YELLOW;
            let _ = write!(out, "\x1b[38;2;{r};{g};{b}m{caption}\x1b[0m");
        }
        Palette::Mono => out.push_str(&caption),
    }
    out
}

/// So lange wartet [`Splash::stop`] höchstens auf den Zeichen-Thread.
///
/// Ein Terminal, dessen Ausgabe angehalten ist (`Ctrl+S`, ein voller Puffer),
/// hält den Thread in seinem Schreiben fest. Dann geht `stop` nach dieser
/// Frist trotzdem zurück, damit der Befehl die Sitzung stoppen kann; der
/// Thread gibt das Terminal zurück, sobald es wieder liest.
pub const STOP_PATIENCE: Duration = Duration::from_millis(250);

/// Die laufende Animation. `Drop` beendet sie.
#[derive(Debug)]
pub struct Splash {
    /// Gesetzt, sobald die Animation enden soll; der Thread prüft es vor
    /// jedem Bild.
    cancel: Arc<AtomicBool>,
    /// Gesetzt, wenn der Thread danach nichts mehr schreiben soll, auch
    /// [`RESTORE`] nicht ([`Splash::abandon`]).
    mute: Arc<AtomicBool>,
    /// Meldet, dass der Thread das Terminal zurückgegeben hat.
    done: Option<std::sync::mpsc::Receiver<()>>,
    /// Der Zeichen-Thread, bis [`Splash::stop`] ihn einsammelt.
    thread: Option<JoinHandle<()>>,
    /// Seit wann sie steht.
    started: Instant,
}

impl Splash {
    /// Startet die Animation auf `stderr`.
    ///
    /// `None`, wenn sich kein Thread starten lässt; dann läuft der Start ohne
    /// Animation, und am Terminal hat sich nichts geändert.
    #[must_use]
    pub fn start(palette: Palette) -> Option<Self> {
        Self::start_on(palette, Box::new(std::io::stderr()))
    }

    /// Wie [`Splash::start`], mit einem eigenen Ziel der Bilder.
    ///
    /// Alles, was auf das Terminal geht, schreibt der Thread selbst: zuerst
    /// [`ENTER`], dann die Bilder, zuletzt [`RESTORE`]. So steht hinter
    /// `RESTORE` nie ein Bild, und kein Schreiben hält den Besitzer auf.
    #[must_use]
    pub fn start_on(palette: Palette, out: Box<dyn Write + Send>) -> Option<Self> {
        let cancel = Arc::new(AtomicBool::new(false));
        let mute = Arc::new(AtomicBool::new(false));
        let (tell, done) = std::sync::mpsc::channel();
        let started = Instant::now();
        let thread = std::thread::Builder::new()
            .name("humanitl-splash".to_owned())
            .spawn({
                let cancel = Arc::clone(&cancel);
                let mute = Arc::clone(&mute);
                move || {
                    draw(out, &cancel, &mute, started, palette);
                    let _ = tell.send(());
                }
            })
            .ok()?;
        Some(Self {
            cancel,
            mute,
            done: Some(done),
            thread: Some(thread),
            started,
        })
    }

    /// Wie lange die Animation von jetzt an noch stehen muss.
    #[must_use]
    pub fn rest(&self) -> Duration {
        remaining(self.started, Instant::now(), MINIMUM)
    }

    /// Beendet die Animation und wartet ohne Frist, bis der Thread das
    /// Terminal zurückgegeben hat: der Weg vor der Übergabe an den Agenten.
    /// Danach schreibt die Animation nie mehr etwas.
    pub async fn finish(mut self) {
        self.cancel.store(true, Ordering::SeqCst);
        let (Some(done), Some(thread)) = (self.done.take(), self.thread.take()) else {
            return;
        };
        let _ = tokio::task::spawn_blocking(move || {
            let _ = done.recv();
            let _ = thread.join();
        })
        .await;
    }

    /// Beendet die Animation und wartet höchstens [`STOP_PATIENCE`] darauf,
    /// dass das Terminal zurück ist. Idempotent.
    pub fn stop(&mut self) {
        self.cancel.store(true, Ordering::SeqCst);
        let (Some(done), Some(thread)) = (self.done.take(), self.thread.take()) else {
            return;
        };
        if done.recv_timeout(STOP_PATIENCE).is_ok() {
            let _ = thread.join();
        }
        // Sonst hängt der Thread in einem Schreiben, das das Terminal nicht
        // annimmt. Er bleibt sich selbst überlassen und schreibt `RESTORE`,
        // sobald es weitergeht; der Befehl wartet nicht auf ihn.
    }
}

impl Splash {
    /// Beendet die Animation ohne [`RESTORE`] und ohne zu warten: nach
    /// `SIGHUP`, wenn es das Terminal nicht mehr gibt und jedes Schreiben
    /// `EIO` bekäme (GitHub #13). Der Thread endet nach seinem laufenden
    /// Bild von selbst; der Befehl wartet nicht auf ihn. Idempotent.
    pub fn abandon(&mut self) {
        self.mute.store(true, Ordering::SeqCst);
        self.cancel.store(true, Ordering::SeqCst);
        self.done = None;
        self.thread = None;
    }
}

impl Drop for Splash {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Der Zeichen-Thread: [`ENTER`], ein Bild je [`FRAME`], bis `cancel`
/// gesetzt ist, dann [`RESTORE`].
fn draw(
    mut out: Box<dyn Write + Send>,
    cancel: &AtomicBool,
    mute: &AtomicBool,
    started: Instant,
    palette: Palette,
) {
    let model = render::Model::fist();
    let mut size = (0, 0);
    let mut pending = String::from(ENTER);
    while !cancel.load(Ordering::SeqCst) {
        let begun = Instant::now();
        let (cols, rows) = crate::tty::window_size();
        let now = (
            usize::try_from(cols).unwrap_or(MAX_COLS),
            usize::try_from(rows).unwrap_or(MAX_ROWS),
        );
        if now != size {
            // Nach einer Größenänderung stünden Reste des alten Bilds da.
            pending.push_str("\x1b[2J");
            size = now;
        }
        pending.push_str(&screen(&model, started.elapsed(), now.0, now.1, palette));
        // Ein Bild ist ein Schreiben; ein Fehler ist kein Grund, den Start
        // aufzuhalten. Nach `SIGHUP` (`Splash::abandon`) geht kein Bild mehr
        // hinaus, auch keines, das schon gebaut war (GitHub #13).
        if mute.load(Ordering::SeqCst) {
            return;
        }
        let _ = out.write_all(pending.as_bytes());
        let _ = out.flush();
        pending.clear();
        let rest = FRAME.saturating_sub(begun.elapsed());
        // In kleinen Schritten schlafen, damit ein Ende nicht auf das ganze
        // Bild wartet.
        let until = Instant::now() + rest;
        while !cancel.load(Ordering::SeqCst) && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(5).min(until - Instant::now()));
        }
    }
    // Wer nie ein Bild schrieb, hat auch nichts zurückzugeben; ein Terminal,
    // das es nicht mehr gibt, auch nicht.
    if pending.is_empty() && !mute.load(Ordering::SeqCst) {
        let _ = out.write_all(RESTORE.as_bytes());
        let _ = out.flush();
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::ffi::OsStr;
    use std::time::{Duration, Instant};

    use super::{MINIMUM, Palette, palette, remaining, screen, wanted};

    /// Eine Sekunde mindestens: Wer nach 300 ms läuft, wartet 700 ms, wer
    /// nach 1,5 s läuft, wartet nicht. Die Uhr ist gestellt, nicht gemessen.
    #[test]
    fn the_splash_stands_one_second_and_never_longer_than_the_start() {
        let started = Instant::now();
        assert_eq!(MINIMUM, Duration::from_secs(1));
        assert_eq!(
            remaining(started, started + Duration::from_millis(300), MINIMUM),
            Duration::from_millis(700)
        );
        assert_eq!(remaining(started, started, MINIMUM), MINIMUM);
        assert_eq!(
            remaining(started, started + Duration::from_millis(1500), MINIMUM),
            Duration::ZERO
        );
        // Eine Uhr, die rückwärts geht, verlängert nichts.
        assert_eq!(
            remaining(started + Duration::from_secs(5), started, MINIMUM),
            MINIMUM
        );
    }

    /// Nur auf einem Terminal, nie mit `--json`, und die Einstellung schaltet
    /// sie ab.
    #[test]
    fn the_splash_needs_both_terminals_no_json_and_the_setting() {
        assert!(wanted(true, false, true));
        assert!(!wanted(false, false, true), "terminal.splash = false");
        assert!(!wanted(true, true, true), "--json");
        assert!(!wanted(true, false, false), "no terminal");
    }

    /// `NO_COLOR` mit Wert heißt ohne Farbe; leer oder fehlend heißt Farbe.
    #[test]
    fn no_color_turns_the_colour_off() {
        assert_eq!(palette(Some(OsStr::new("1"))), Palette::Mono);
        assert_eq!(palette(Some(OsStr::new(""))), Palette::TrueColor);
        assert_eq!(palette(None), Palette::TrueColor);
    }

    /// Ein ganzes Bild ohne Farbe trägt nur Cursor-Positionen, keine Farbe.
    #[test]
    fn a_mono_screen_carries_no_colour() {
        let model = super::render::Model::fist();
        let out = screen(&model, Duration::ZERO, 80, 24, Palette::Mono);
        assert!(!out.contains("\x1b[38;"), "{out:?}");
        assert!(!out.contains("\x1b[48;"), "{out:?}");
        assert!(out.contains("starting a disposable session"));
        let coloured = screen(&model, Duration::ZERO, 80, 24, Palette::TrueColor);
        assert!(coloured.contains("\x1b[38;2;"), "colour without NO_COLOR");
    }

    /// Auch ein winziges Terminal bringt kein Bild außerhalb des Schirms.
    #[test]
    fn a_tiny_terminal_gets_no_panic() {
        let model = super::render::Model::fist();
        let _ = screen(&model, Duration::from_millis(500), 3, 2, Palette::TrueColor);
        let _ = screen(&model, Duration::from_millis(500), 0, 0, Palette::Mono);
    }

    /// Ein Ziel, das alles mitschreibt.
    #[derive(Clone, Default)]
    struct Recorded(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

    impl std::io::Write for Recorded {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Ein Terminal mit angehaltener Ausgabe: Jedes Schreiben wartet, bis
    /// das Tor offen ist, und wird dann mitgeschrieben.
    #[derive(Clone, Default)]
    struct Paused {
        open: std::sync::Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>,
        seen: Recorded,
    }

    impl std::io::Write for Paused {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let (lock, wake) = &*self.open;
            let mut open = lock.lock().unwrap();
            while !*open {
                open = wake.wait(open).unwrap();
            }
            drop(open);
            self.seen.write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Eintritt zuerst, `RESTORE` zuletzt, dazwischen Bilder.
    #[test]
    fn the_thread_enters_draws_and_gives_the_terminal_back_last() {
        let seen = Recorded::default();
        let mut shown =
            super::Splash::start_on(Palette::Mono, Box::new(seen.clone())).expect("a thread");
        std::thread::sleep(Duration::from_millis(80));
        shown.stop();
        let text = String::from_utf8(seen.0.lock().unwrap().clone()).unwrap();
        assert!(text.starts_with(super::ENTER), "{text:?}");
        assert!(text.ends_with(super::RESTORE), "{text:?}");
        assert!(text.contains("starting a disposable session"));
    }

    /// Ein Terminal, das nichts annimmt, hält `stop` höchstens
    /// `STOP_PATIENCE` auf, und sobald es wieder liest, gibt der Thread es
    /// zurück.
    #[test]
    fn a_paused_terminal_does_not_hold_the_stop() {
        let paused = Paused::default();
        let mut shown =
            super::Splash::start_on(Palette::Mono, Box::new(paused.clone())).expect("a thread");
        std::thread::sleep(Duration::from_millis(50));
        let (tell, stopped) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let begun = Instant::now();
            shown.stop();
            let _ = tell.send(begun.elapsed());
        });
        let took = stopped
            .recv_timeout(Duration::from_secs(3))
            .expect("stop returned although the terminal takes nothing");
        assert!(took < super::STOP_PATIENCE * 3, "{took:?}");

        let (lock, wake) = &*paused.open;
        *lock.lock().unwrap() = true;
        wake.notify_all();
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let text = String::from_utf8_lossy(&paused.seen.0.lock().unwrap()).into_owned();
            if text.ends_with(super::RESTORE) {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the terminal never came back: {text:?}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Before the hand-over to the agent `finish` waits without a bound: as
    /// long as the terminal takes nothing it does not return, and when it
    /// returns, `RESTORE` is the last thing written.
    #[tokio::test]
    async fn finish_waits_until_the_terminal_is_given_back() {
        let paused = Paused::default();
        let shown =
            super::Splash::start_on(Palette::Mono, Box::new(paused.clone())).expect("a thread");
        tokio::time::sleep(Duration::from_millis(50)).await;
        let mut finishing = tokio::spawn(shown.finish());
        let early = tokio::time::timeout(super::STOP_PATIENCE * 3, &mut finishing).await;
        assert!(
            early.is_err(),
            "finish returned while the terminal took nothing"
        );

        let (lock, wake) = &*paused.open;
        *lock.lock().unwrap() = true;
        wake.notify_all();
        tokio::time::timeout(Duration::from_secs(3), finishing)
            .await
            .expect("finish returned once the terminal reads again")
            .unwrap();
        let text = String::from_utf8_lossy(&paused.seen.0.lock().unwrap()).into_owned();
        assert!(text.ends_with(super::RESTORE), "{text:?}");
    }
}
