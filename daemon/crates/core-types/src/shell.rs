//! Werte als Wörter der Shell, für die Behebungsvorschläge der Befunde.
//!
//! Ein `fix` ist ein Befehl, den ein Mensch kopiert und ausführt. Er muss
//! dieselbe Datei nennen, die der Befund meint, auch wenn ihr Name Leerraum,
//! ein `'` oder Bytes enthält, die kein UTF-8 sind. Und er muss den Block der
//! Kommandozeile unverändert überstehen, der Leerraum faltet und
//! Steuerzeichen wegwirft (HUM-215). Deshalb steht hier alles außer
//! druckbarem ASCII und einem einzelnen Leerzeichen als Byte in der Form
//! `$'…\xHH…'`.
//!
//! Die Vorschläge setzen `bash` oder `zsh` voraus: `dash` kennt `$'…'` nicht
//! in jeder Fassung und gäbe es wörtlich weiter.

use std::fmt::Write as _;
use std::os::unix::ffi::OsStrExt as _;
use std::path::Path;

/// Ein Wert als ein Wort der Shell, so dass der Befehl ihn unverändert
/// weitergibt.
///
/// `humanitl config set llm.passthrough_paths ["/v1/","/api/"]` käme bei `bash` als
/// `[/v1/,/api/]` an: Die Anführungszeichen gehören der Shell. Ein Wert aus
/// Zeichen, die sie nicht deutet, bleibt, wie er ist; ein Wert aus druckbarem
/// ASCII und einzelnen Leerzeichen steht in einfachen Anführungszeichen, und
/// ein einfaches darin wird `'\''`. `'/tmp/a b'` bleibt so lesbar.
///
/// Alles andere steht nie wörtlich darin, sondern als Byte in der Form
/// `$'…\xHH…'` der Shell: zwei Leerzeichen hintereinander, jeder andere
/// Leerraum, Steuerzeichen und jedes Byte über `0x7e`. Der Block eines
/// Befunds faltet eine Folge von Leerraum zu einem Leerzeichen und wirft
/// Steuerzeichen weg, und ein Dateiname mit Zeilenumbruch oder zwei
/// Leerzeichen käme sonst als ein anderer Name beim `mv` an. Ein einzelnes
/// Leerzeichen zwischen den Anführungszeichen faltet er nicht. `\xHH` ist ein
/// Byte und kein Zeichen: Es gilt unter jeder Locale, auch unter `LC_ALL=C`,
/// wo `\u…` nichts bedeutet.
#[must_use]
pub fn shell_word(value: &str) -> String {
    shell_bytes(value.as_bytes())
}

/// Ein Pfad als ein Wort der Shell, aus seinen Bytes und nicht aus seiner
/// Anzeige.
///
/// `Path::display` ersetzt ein Byte, das kein UTF-8 ist, durch `U+FFFD`; ein
/// Vorschlag daraus nennte eine Datei, die es nicht gibt.
#[must_use]
pub fn shell_path(path: &Path) -> String {
    shell_bytes(path.as_os_str().as_bytes())
}

/// Die Bytes als ein Wort der Shell (siehe [`shell_word`]).
fn shell_bytes(bytes: &[u8]) -> String {
    let bare = |byte: u8| byte.is_ascii_alphanumeric() || b"_-./:@%+=,".contains(&byte);
    let printable = |byte: u8| (0x21..=0x7e).contains(&byte);
    // Ein Leerzeichen, neben dem kein zweites steht, übersteht das Falten
    // des Blocks. Am Rand eines Worts steht in einfachen Anführungszeichen
    // das `'` daneben, nie ein Leerzeichen der Zeile.
    let lone_space = |index: usize| {
        bytes.get(index) == Some(&b' ')
            && (index == 0 || bytes.get(index - 1) != Some(&b' '))
            && bytes.get(index + 1) != Some(&b' ')
    };
    // Ein `=` am Anfang bleibt nicht nackt: zsh macht aus `=name` den Pfad
    // des Programms `name` (EQUALS).
    if bytes.first().is_some_and(|&first| first != b'=') && bytes.iter().all(|&byte| bare(byte)) {
        return String::from_utf8_lossy(bytes).into_owned();
    }
    if bytes
        .iter()
        .enumerate()
        .all(|(index, &byte)| printable(byte) || lone_space(index))
    {
        let text = String::from_utf8_lossy(bytes);
        return format!("'{}'", text.replace('\'', "'\\''"));
    }
    let mut out = String::from("$'");
    for &byte in bytes {
        match byte {
            b'\\' => out.push_str("\\\\"),
            b'\'' => out.push_str("\\'"),
            byte if printable(byte) => out.push(char::from(byte)),
            byte => {
                // Ein `String` nimmt jedes `write!` an.
                let _ = write!(out, "\\x{byte:02x}");
            }
        }
    }
    out.push('\'');
    out
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{shell_path, shell_word};

    #[test]
    fn plain_words_stay_bare_and_the_rest_is_quoted() {
        assert_eq!(shell_word("/home/x/.local/share"), "/home/x/.local/share");
        assert_eq!(shell_word("it's"), r"'it'\''s'");
        assert_eq!(shell_word(""), "''");
        assert_eq!(shell_word("=ls"), "'=ls'");
        assert_eq!(shell_word("~/x"), "'~/x'");
    }

    /// Ein einzelnes Leerzeichen bleibt lesbar, weil der Block es nicht
    /// faltet; zwei, ein Tabulator oder ein Zeilenumbruch werden zu Bytes.
    #[test]
    fn a_lone_space_stays_readable() {
        assert_eq!(shell_path(Path::new("/tmp/a b")), "'/tmp/a b'");
        assert_eq!(shell_word(" a b "), "' a b '");
        assert_eq!(shell_word("a\tb"), r"$'a\x09b'");
        assert_eq!(shell_word("a b  c"), r"$'a\x20b\x20\x20c'");
    }

    /// Leerraum steht als Byte darin, damit ihn kein Falten ändern kann.
    #[test]
    fn whitespace_is_written_as_bytes() {
        assert_eq!(shell_word("/tmp/a  b"), r"$'/tmp/a\x20\x20b'");
        assert_eq!(shell_word("a\nb's"), r"$'a\x0ab\'s'");
        assert_eq!(
            shell_path(Path::new("/home/u/Audit  2026/a.jsonl")),
            r"$'/home/u/Audit\x20\x202026/a.jsonl'"
        );
    }
}
