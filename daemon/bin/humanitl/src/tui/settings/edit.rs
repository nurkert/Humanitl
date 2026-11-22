//! Die Bedienelemente über der Liste: Eingabe, Auswahl und Rückfrage
//! (GitHub #7).
//!
//! Sie halten nur, was getippt oder gewählt ist, und wohin es danach geht
//! ([`Target`]). Ob ein Wert gilt, prüfen sie nicht; das entscheidet der
//! Daemon, und seine Ablehnung steht danach an der Zeile.
//!
//! # Verdeckte Eingabe
//!
//! Ein Geheimnis (Zugangsschlüssel, Zugangsdaten des Proxys) wird in einer
//! [`Input`] mit `secret` getippt. Gezeigt wird davon nichts, auch nicht die
//! Zahl der Zeichen: kein Echo. Der Text verlässt die Eingabe genau einmal,
//! beim Bestätigen ([`Input::take`]).
//!
//! Was gewischt wird: der Puffer des Felds bei jedem Löschen, Leeren,
//! Abbrechen, Wachsen und Fallenlassen, und der Text eines Einfügens, nachdem
//! er übernommen ist (`tui::screen`). Bis zum Bau der Anfrage bleibt der Text
//! unter `Zeroizing`. Was nicht gewischt werden kann: das Feld der Anfrage
//! selbst, denn tonic nimmt die Nachricht und gibt sie nicht zurück, die
//! Puffer, in die prost und tonic sie kodieren, und die Lesepuffer von
//! crossterm für Tasten und Einfügen.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// Wohin eine Eingabe, eine Wahl oder eine Bestätigung geht.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// Den Wert eines Katalogeintrags schreiben: global über `SetConfig`,
    /// in einem Projekt über `Projects.save_choice`.
    Write(String),
    /// Den Zugangsschlüssel eines Modellservers ablegen (`LlmKey`); leer
    /// heißt der aktive.
    LlmKey(String),
    /// Den Zugangsschlüssel eines Modellservers entfernen.
    RemoveLlmKey(String),
    /// Die Zugangsdaten des Entscheidungs-Proxys ablegen (`Egress`).
    EgressAuth,
    /// Die Zugangsdaten des Entscheidungs-Proxys entfernen.
    RemoveEgressAuth,
    /// Einen Server anlegen, erster Schritt: der Name.
    AddName,
    /// Zweiter Schritt: die Adresse.
    AddEndpoint {
        /// Der Name aus dem ersten Schritt.
        name: String,
    },
    /// Dritter Schritt: die Modelle.
    AddModels {
        /// Der Name.
        name: String,
        /// Die Adresse.
        endpoint: String,
    },
    /// Einen Server umbenennen.
    Rename(String),
    /// Die Adresse eines gespeicherten Servers ändern (`LlmServers.Edit`),
    /// ohne ihn zum Standard zu machen.
    EditEndpoint(String),
    /// Die Modelle eines gespeicherten Servers ändern (`LlmServers.Edit`).
    EditModels(String),
    /// Einen Server entfernen, mit dem Server, der danach aktiv ist.
    RemoveServer {
        /// Der Server.
        name: String,
        /// Der Nachfolger; leer, wenn keiner nötig ist.
        replacement: String,
    },
    /// Einem Profil einen Server zuordnen (`LlmServers.Assign`).
    AssignProfile(String),
    /// Dem Projektordner einen Server zuordnen.
    AssignProject(String),
    /// Die Quelle eines Agenten setzen (`Agents.SetSource`).
    AgentSource(String),
    /// Den Daemon neu starten.
    Restart,
}

/// Was ein Bedienelement nach einer Taste sagt.
#[derive(Debug, PartialEq, Eq)]
pub enum Step<T> {
    /// Es bleibt offen.
    Open,
    /// Abgebrochen; nichts geht hinaus.
    Cancel,
    /// Fertig, mit dem Ergebnis.
    Done(T),
}

/// So viele Bytes hält ein Feld von Anfang an bereit: Bis dahin wächst der
/// Puffer nie um, und ein Umzug ließe sonst eine Kopie im alten Speicher.
const RESERVED: usize = 256;

/// Überschreibt einen Puffer samt ungenutzter Kapazität mit Nullen und
/// leert ihn (`zeroize`). Jeder Weg, auf dem Text ein Feld verlässt oder
/// verworfen wird, geht hierüber.
fn wipe(text: &mut String) {
    use zeroize::Zeroize as _;

    #[cfg(test)]
    WIPED.with(|wiped| wiped.set(wiped.get() + text.capacity()));
    text.zeroize();
}

#[cfg(test)]
thread_local! {
    /// Wie viele Bytes [`wipe`] in diesem Thread überschrieben hat; nur für
    /// den Beweis, dass jeder Weg hinaus wischt.
    pub static WIPED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Ein Feld, in das getippt wird.
pub struct Input {
    /// Was über dem Feld steht.
    pub label: String,
    /// Der Text; bei `secret` nie gezeigt. Beim Fallenlassen überschrieben.
    text: zeroize::Zeroizing<String>,
    /// Verdeckt, ohne Echo.
    pub secret: bool,
    /// Wohin der Text geht.
    pub target: Target,
}

impl Input {
    /// Ein Feld, vorbelegt mit `text`.
    #[must_use]
    pub fn plain(label: String, text: String, target: Target) -> Self {
        let mut input = Self::secret(label, target);
        input.secret = false;
        input.push_str(&text);
        // Auch ein gewöhnlicher Wert bleibt nicht doppelt im Speicher.
        let mut text = text;
        wipe(&mut text);
        input
    }

    /// Ein verdecktes Feld, leer.
    #[must_use]
    pub fn secret(label: String, target: Target) -> Self {
        Self {
            label,
            text: zeroize::Zeroizing::new(String::with_capacity(RESERVED)),
            secret: true,
            target,
        }
    }

    /// Was im Feld zu sehen ist: der Text, oder bei einem Geheimnis nichts.
    #[must_use]
    pub fn shown(&self) -> &str {
        if self.secret { "" } else { &self.text }
    }

    /// Nimmt den Text heraus, weiter unter `Zeroizing`; das Feld ist danach
    /// leer, mit frischem Puffer. Der Text zieht um, ohne kopiert zu werden,
    /// und erst die Anfrage an den Daemon nimmt ihn aus dem `Zeroizing`
    /// ([`super::actions::commit`]).
    pub fn take(&mut self) -> zeroize::Zeroizing<String> {
        zeroize::Zeroizing::new(std::mem::replace(
            &mut *self.text,
            String::with_capacity(RESERVED),
        ))
    }

    /// Hängt Text an. Reicht die Kapazität nicht, zieht er in einen größeren
    /// Puffer, und der alte wird gewischt.
    fn push_str(&mut self, more: &str) {
        let needed = self.text.len() + more.len();
        if needed > self.text.capacity() {
            let mut bigger = String::with_capacity(needed.max(self.text.capacity() * 2));
            bigger.push_str(&self.text);
            wipe(&mut self.text);
            *self.text = bigger;
        }
        self.text.push_str(more);
    }

    /// Nimmt das letzte Zeichen weg. Der Rest zieht in einen frischen
    /// Puffer, der alte samt dem Zeichen wird gewischt.
    fn pop(&mut self) {
        let Some(last) = self.text.chars().next_back() else {
            return;
        };
        let keep = self.text.len() - last.len_utf8();
        let mut rest = String::with_capacity(self.text.capacity());
        rest.push_str(&self.text[..keep]);
        wipe(&mut self.text);
        *self.text = rest;
    }

    /// Nimmt eine Taste: Zeichen, `Backspace`, `Ctrl+U` leert, `Enter`
    /// bestätigt, `Esc` bricht ab.
    pub fn key(&mut self, key: &KeyEvent) -> Step<()> {
        if key.kind != KeyEventKind::Press {
            return Step::Open;
        }
        match key.code {
            KeyCode::Esc => {
                // Ein abgebrochenes Geheimnis bleibt nicht im Speicher stehen.
                wipe(&mut self.text);
                return Step::Cancel;
            }
            KeyCode::Enter => return Step::Done(()),
            KeyCode::Backspace => self.pop(),
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                wipe(&mut self.text);
            }
            KeyCode::Char(c)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                let mut bytes = [0u8; 4];
                self.push_str(c.encode_utf8(&mut bytes));
            }
            _ => {}
        }
        Step::Open
    }

    /// Nimmt eingefügten Text, ohne Zeilenumbrüche.
    pub fn paste(&mut self, text: &str) {
        for c in text.chars().filter(|c| *c != '\n' && *c != '\r') {
            let mut bytes = [0u8; 4];
            self.push_str(c.encode_utf8(&mut bytes));
        }
    }
}

/// Eine Möglichkeit einer Auswahl.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Opt {
    /// Der Wert, der geschrieben wird.
    pub value: String,
    /// Der Name.
    pub title: String,
    /// Ob sie gewählt werden kann; sonst steht `reason` daneben.
    pub enabled: bool,
    /// Der Grund des Daemons, warum nicht, oder ein Zusatz; leer ohne.
    pub reason: String,
}

impl Opt {
    /// Eine wählbare Möglichkeit ohne Zusatz.
    #[must_use]
    pub fn new(value: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            title: title.into(),
            enabled: true,
            reason: String::new(),
        }
    }
}

/// Eine Auswahl aus festen Möglichkeiten.
pub struct Picker {
    /// Was über der Liste steht.
    pub label: String,
    /// Die Möglichkeiten in der Reihenfolge der Antwort.
    pub options: Vec<Opt>,
    /// Die markierte.
    pub cursor: usize,
    /// Wohin der gewählte Wert geht.
    pub target: Target,
}

impl Picker {
    /// Eine Auswahl, die Markierung auf `current`, sonst auf der ersten.
    #[must_use]
    pub fn new(label: String, options: Vec<Opt>, current: &str, target: Target) -> Self {
        let cursor = options
            .iter()
            .position(|option| option.value == current)
            .unwrap_or(0);
        Self {
            label,
            options,
            cursor,
            target,
        }
    }

    /// Nimmt eine Taste: hoch, runter, `Enter` wählt eine wählbare
    /// Möglichkeit, `Esc` bricht ab.
    pub fn key(&mut self, key: &KeyEvent) -> Step<String> {
        if key.kind != KeyEventKind::Press {
            return Step::Open;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => Step::Cancel,
            KeyCode::Up | KeyCode::Char('k') => {
                self.cursor = self.cursor.saturating_sub(1);
                Step::Open
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.cursor + 1 < self.options.len() {
                    self.cursor += 1;
                }
                Step::Open
            }
            KeyCode::Enter | KeyCode::Char(' ') => match self.options.get(self.cursor) {
                Some(option) if option.enabled => Step::Done(option.value.clone()),
                _ => Step::Open,
            },
            _ => Step::Open,
        }
    }
}

/// Eine Rückfrage mit Ja und Nein.
pub struct Confirm {
    /// Die Frage, ausformuliert.
    pub question: String,
    /// Was bei Ja geschieht.
    pub target: Target,
}

impl Confirm {
    /// Nimmt eine Taste: `y` oder `Enter` ja, `n`, `q` oder `Esc` nein.
    #[must_use]
    pub fn key(key: &KeyEvent) -> Step<()> {
        if key.kind != KeyEventKind::Press {
            return Step::Open;
        }
        match key.code {
            KeyCode::Char('y' | 'Y') | KeyCode::Enter => Step::Done(()),
            KeyCode::Char('n' | 'N' | 'q') | KeyCode::Esc => Step::Cancel,
            _ => Step::Open,
        }
    }
}

/// Was gerade über der Liste offen ist.
pub enum Overlay {
    /// Ein Feld.
    Input(Input),
    /// Eine Auswahl.
    Picker(Picker),
    /// Eine Rückfrage.
    Confirm(Confirm),
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::{Input, Opt, Picker, Step, Target};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// Ein verdecktes Feld zeigt nichts von dem, was getippt ist, und gibt
    /// es genau einmal heraus.
    #[test]
    fn a_secret_input_shows_nothing_and_gives_its_text_once() {
        let mut input = Input::secret("Key".to_owned(), Target::EgressAuth);
        for c in "u:pw".chars() {
            assert_eq!(input.key(&key(KeyCode::Char(c))), Step::Open);
        }
        assert_eq!(input.shown(), "");
        assert_eq!(input.key(&key(KeyCode::Enter)), Step::Done(()));
        assert_eq!(input.take().as_str(), "u:pw");
        assert_eq!(input.take().as_str(), "", "taken once");
    }

    /// Löschen, Leeren, Abbrechen und Wachsen wischen den alten Puffer;
    /// nach `take` ist das Feld leer.
    #[test]
    fn every_way_out_of_an_input_wipes_the_buffer() {
        let wiped = || super::WIPED.with(std::cell::Cell::get);
        let mut input = Input::secret("Key".to_owned(), Target::EgressAuth);
        for c in "abc".chars() {
            input.key(&key(KeyCode::Char(c)));
        }
        let before = wiped();
        input.key(&key(KeyCode::Backspace));
        assert!(wiped() > before, "Backspace wipes");
        let before = wiped();
        input.key(&KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
        assert!(wiped() > before, "Ctrl+U wipes");
        input.paste(&"x".repeat(300));
        let before = wiped();
        input.paste(&"y".repeat(300));
        assert!(wiped() > before, "growing wipes the old buffer");
        let before = wiped();
        assert_eq!(input.key(&key(KeyCode::Esc)), Step::Cancel);
        assert!(wiped() > before, "Esc wipes");
        input.paste("pw");
        assert_eq!(input.take().as_str(), "pw");
        assert_eq!(input.take().as_str(), "");
    }

    /// Eine Möglichkeit, die nicht wählbar ist, lässt sich nicht wählen.
    #[test]
    fn a_disabled_option_cannot_be_picked() {
        let mut off = Opt::new("other", "Other backend");
        off.enabled = false;
        let mut picker = Picker::new(
            "Backend".to_owned(),
            vec![Opt::new("auto", "Automatic"), off],
            "other",
            Target::Write("sandbox.backend".to_owned()),
        );
        assert_eq!(picker.cursor, 1);
        assert_eq!(picker.key(&key(KeyCode::Enter)), Step::Open);
        picker.key(&key(KeyCode::Up));
        assert_eq!(
            picker.key(&key(KeyCode::Enter)),
            Step::Done("auto".to_owned())
        );
    }
}
