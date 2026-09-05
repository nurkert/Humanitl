/// Wie aus fremden Werten ein Befehl wird, den ein Mensch kopieren darf — und
/// wann daraus keiner wird.
///
/// Ein Knopf, der etwas in die Zwischenablage legt, ist der kürzeste Weg von
/// einem Wert aus dem Netz in eine Shell auf dem Rechner des Nutzers. Der
/// Daemon baut den Vorschlag `SetEnv(key, value)` aus Material, das er nicht
/// selbst erfunden hat, und die Anwendung interpolierte ihn bis hierher
/// ungequotet zu `export $key=$value`. Ein `value` mit `;`, `&&`, `|`,
/// Backtick oder `$(…)` wäre damit ein zweiter Befehl gewesen, ein `value` mit
/// Zeilenumbruch eine zweite Zeile, die manches Terminal beim Einfügen sofort
/// ausführt.
///
/// Die Bereinigung der Anzeige hilft hier nicht: `sanitizeBodyText` lässt
/// Tabulator, Zeilenvorschub und Wagenrücklauf ausdrücklich durch, weil eine
/// Rumpf-Ansicht sie zeigen können muss. Für eine Zeile, die in eine Shell
/// wandert, ist genau das falsch.
///
/// Die Regel ist deshalb dieselbe, die der Daemon für seine eigenen
/// Kommandozeilen anwendet (`humanitl_sandbox::shell_quote`,
/// `daemon/crates/sandbox/src/bwrap_args.rs`), plus zwei Weigerungen: **Ein
/// Befehl entsteht nur, wenn er beweisbar genau das ist, was er zu sein
/// vorgibt.** Lässt sich das nicht beweisen, entsteht kein Befehl — kein
/// Ersatzschlüssel, kein Platzhalter. Ein Knopf, der etwas anderes kopiert,
/// als er anzeigt, ist schlimmer als kein Knopf
/// (`backlog/CONVENTIONS.md` 4.13).
library;

/// Ein Name, den eine Shell als Variable zuweisen kann.
///
/// Dieselbe Menge, die POSIX für einen Namen zulässt. Ein Schlüssel, der sie
/// verletzt, ergibt keine Zuweisung, sondern irgendetwas anderes — und was
/// genau, hängt an der Shell des Nutzers.
final RegExp _environmentName = RegExp(r'^[A-Za-z_][A-Za-z0-9_]*$');

/// Warum aus einem `FixAction.setEnv` kein Befehl wird.
enum ExportRefusal {
  /// Der Schlüssel ist kein Name, den eine Shell zuweisen kann.
  key,

  /// Der Wert geht über mehr als eine Zeile.
  ///
  /// Ein Wagenrücklauf oder Zeilenvorschub im Wert erzeugte in der
  /// Zwischenablage zwei Zeilen. Quotieren allein genügt hier nicht: Wer den
  /// Text in ein Terminal ohne Klammer-Einfügen wirft, hat die erste Zeile
  /// abgeschickt, bevor er die zweite gelesen hat.
  value,
}

/// Der Befehl `export KEY=VALUE`, oder null, wenn keiner entstehen darf.
///
/// [value] wird in einfache Anführungszeichen gesetzt, ein inneres `'` als
/// `'\''` — danach liest jede POSIX-Shell die Zuweisung als genau ein Wort,
/// gleich was darin steht. Nur wenn schon der Rahmen nicht stimmt, entsteht
/// gar nichts; [exportRefusal] sagt dann, welcher.
String? exportCommand({required String key, required String value}) =>
    exportRefusal(key: key, value: value) == null
    ? 'export $key=${shellQuote(value)}'
    : null;

/// Warum [exportCommand] null liefert, oder null, wenn es einen Befehl gibt.
ExportRefusal? exportRefusal({required String key, required String value}) {
  if (!_environmentName.hasMatch(key)) {
    return ExportRefusal.key;
  }
  if (value.contains('\n') || value.contains('\r')) {
    return ExportRefusal.value;
  }
  return null;
}

/// [word], so dass `sh` genau ein Wort daraus liest.
///
/// Ein Wort aus unbedenklichen Zeichen bleibt, wie es ist; alles andere kommt
/// in einfache Anführungszeichen, in denen die Shell kein Zeichen mehr deutet.
/// Das einzige, was dort nicht stehen kann, ist das einfache Anführungszeichen
/// selbst; es wird zu `'\''` — schließen, ein maskiertes Zeichen, wieder
/// öffnen.
///
/// Die Tilde fehlt in der unbedenklichen Menge, anders als in der Fassung des
/// Daemons für Pfade: Nach einem `=` expandiert die Shell sie zum
/// Heimatverzeichnis, und ein Wert, der beim Einfügen etwas anderes bedeutet
/// als auf dem Schirm, ist genau der Fehler, den diese Datei verhindert.
String shellQuote(String word) {
  if (word.isNotEmpty && word.runes.every(_isSafe)) {
    return word;
  }
  return "'${word.replaceAll("'", r"'\''")}'";
}

/// Wahr, wenn dieses Zeichen für keine Shell eine Bedeutung hat.
bool _isSafe(int rune) {
  if (rune >= 0x30 && rune <= 0x39) {
    return true;
  }
  if (rune >= 0x41 && rune <= 0x5A) {
    return true;
  }
  if (rune >= 0x61 && rune <= 0x7A) {
    return true;
  }
  return switch (rune) {
    0x2F || 0x2E || 0x5F || 0x2D || 0x2B || 0x3A || 0x40 => true,
    _ => false,
  };
}
