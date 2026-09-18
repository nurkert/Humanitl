/// Was aus einem Fehlertext verschwindet, bevor er im Protokoll steht
/// (HUM-136).
///
/// # Warum überhaupt
///
/// `docs/SECURITY.md` 8 sagt für das Audit-Log: Bodies, Header, Klartext-Werte
/// von Funden und die Notiz einer Blockierung stehen nie darin, der Pfad einer
/// Anfrage nur als Streuwert, der Host im Klartext. `app.log` liegt außerhalb
/// des Recorders und außerhalb der Hash-Kette, also fängt es niemand sonst
/// ab -- und der Text einer Ausnahme ist genau die Stelle, an der so etwas
/// hineinkommt: Eine `FormatException` beim Lesen von JSON hängt das Stück
/// Quelltext an, an dem sie scheiterte, und das ist der Rumpf einer Anfrage.
///
/// # Was [redactForLog] entfernt
///
/// * **Pfad und Abfrage jeder URL.** Der Host bleibt stehen, alles dahinter
///   wird [removedPathMarker], und eine Anmeldung davor
///   (`https://benutzer:wort@host/...`) wird [redactedMarker]. Das Audit-Log
///   setzt an die Stelle des Pfades einen SHA-256, damit zwei Einträge sich
///   derselben Anfrage zuordnen lassen; hier ordnet nichts zu, also ist
///   Wegwerfen die engere Antwort und kostet nichts.
/// * **Bearer- und Basic-Anmeldungen** hinter `Authorization`.
/// * **Zuweisungen an Schlüssel, deren Name ein Geheimnis ankündigt**
///   (`token`, `api_key`, `secret`, `password`, `cookie`, `session_id` und
///   Verwandte), samt Wert.
/// * **Lange zusammenhängende Läufe aus Base64- oder Hex-Zeichen** (ab 32),
///   wie sie ein Schlüssel, ein Token oder ein Stück Rumpf bildet. Der
///   Schrägstrich gehört nicht dazu, damit ein Stapelabzug seine Pfade behält.
///
/// Dazu kommt [describeErrorForLog]: Für eine `FormatException` steht nur
/// Meldung und Versatz in der Zeile, nie die Quelle, die `toString()` sonst
/// anhängt.
///
/// # Was es nicht kann
///
/// Elf Grenzen, gemessen und hier genannt, damit niemand mehr erwartet, als
/// hier steht:
///
/// 1. **Fließtext.** Ein Widget-Fehler beim Zeichnen einer gehaltenen Anfrage
///    kann deren Rumpf als gewöhnliche Prosa enthalten. Dafür gibt es kein
///    Muster.
/// 2. **Ein kurzes Geheimnis ohne Namen davor.** `ghp_abc123` allein steht
///    unter [secretRunLength] Zeichen und trägt keinen Schlüsselnamen.
/// 3. **Base64 mit dem Standard-Alphabet.** `/` gehört nicht zu
///    [_secretRun] -- sonst verlöre ein Stapelabzug seine Pfade --, also
///    zerfällt solcher Text an jedem Schrägstrich in kurze Stücke.
/// 4. **Umbrochenes Base64.** Wer unter [secretRunLength] Spalten umbricht,
///    hat keine Läufe mehr, die lang genug sind.
/// 5. **Das zweite und jedes weitere Paar eines Kopfzeilen-Werts ohne
///    Anführungszeichen.** Aus `cookie: sid=x; theme=dark` geht das erste
///    Paar; was hinter dem Semikolon steht, bleibt. Steht der Wert in
///    Anführungszeichen (`'cookie': 'sid=x; theme=dark'`), geht er ganz.
/// 6. **Ein Schlüsselname außerhalb der Liste.** Wer seine Kopfzeile
///    `x-firmen-geheimnis` nennt, bekommt keine Entfernung. Ein neuer Name
///    gehört in den Ausdruck, und wer ihn hinzufügt, achtet auf den
///    Unterstrich: `\b` beginnt kein Wort zwischen `access` und `token`.
/// 7. **Eine URL ohne Schema.** `_url` verlangt `://`; ein Pfad wie
///    `/v1/messages?access_token=…` ist für dieses Modul gewöhnlicher Text,
///    und nur die Zuweisung darin wird gefunden.
/// 8. **Ein zweiter Schlüsselname innerhalb eines eingefassten Werts.** Aus
///    `{"token": "my password": hunter2}` geht der Wert in
///    Anführungszeichen, und mit ihm der Anker `password`, den die nackte
///    Form sonst gefunden hätte; `hunter2` bleibt. Das ist der Preis dafür,
///    dass ein eingefasster Wert ganz geht, und der ist es wert: Die
///    umgekehrte Wahl ließe ganze Rümpfe stehen.
/// 9. **Ein Wert aus mehreren Wörtern ohne Anführungszeichen.** Die nackte
///    Form endet am ersten Leerzeichen, und `Map.toString()` von Dart setzt
///    keine: Aus `{password: two words here, token: abc}` wird
///    `{password=[redacted] words here, token=[redacted]}`. Die nackte Form
///    bis zum nächsten Trennzeichen laufen zu lassen, fräße gewöhnliche Sätze
///    und mit ihnen den Stapelabzug, den `AppLog` hinter die Meldung hängt;
///    ein Leerzeichen ist das einzige Ende, das eine Meldung verlässlich hat.
/// 10. **Ein offener Wert, der das Trennzeichen des Stapels enthält.** Ein
///    Wert in Anführungszeichen -- einfachen, doppelten oder maskierten --,
///    die auf dieser Zeile nie schließen, endet vor ` | #` und einer Ziffer,
///    damit der Stapelabzug stehen bleibt, den `AppLog` so anhängt. Enthält
///    der Wert selbst diese Folge, bleibt, was dahinter steht. Ein
///    geschlossener Wert ist davon nicht betroffen, auch kein maskierter:
///    Die geschlossenen Formen werden zuerst versucht.
/// 11. **Alles hinter [maxRedactedInput] Zeichen.** Es wird nicht angesehen,
///    sondern abgeschnitten, und `AppLog` kürzt die Zeile danach ohnehin
///    weit darunter. Wer [redactForLog] anderswo benutzt, bekommt einen
///    gekürzten Text zurück.
///
/// Die Redaktion ist deshalb die zweite Verteidigungslinie und nicht die
/// erste. Die erste ist der Ort: `0600` in einem Verzeichnis `0700`
/// (`docs/SECURITY.md` 4 und 8), damit ein anderes Konto auf derselben
/// Maschine die Datei nicht liest, und nichts davon verlässt den Rechner.
library;

/// Was anstelle eines Geheimnisses steht.
const String redactedMarker = '[redacted]';

/// Was anstelle von Pfad und Abfrage einer URL steht.
const String removedPathMarker = '[path removed]';

/// Ab wie vielen Zeichen ein Lauf als Geheimnis gilt.
const int secretRunLength = 32;

/// Schema und Host bleiben, die Anmeldung davor und alles dahinter geht.
///
/// Vier Gruppen: Schema, Anmeldung (`benutzer:wort@`, optional), Host, Rest.
/// Die Anmeldung braucht eine eigene Gruppe, weil sie sonst im Host steckt --
/// `https://alice:s3cr3t@api.example.com/v1` ließe sonst das Wort stehen.
///
/// **Das Schema ist auf 32 Zeichen begrenzt, und das ist keine Stilfrage.**
/// Ein unbegrenztes `[A-Za-z0-9+.\-]*` vor `://` probiert in einem langen
/// Lauf solcher Zeichen ohne `://` jede Startstelle bis zum Ende durch -- in
/// der Summe quadratisch. Gemessen am 2026-09-18: 20 000 Zeichen 5 s, ein
/// Hex-Token von 20 KB 2,6 s, und das alles synchron im Fehler-Handler, also
/// im Fenster, das dabei steht. Kein registriertes Schema ist länger als 32.
final RegExp _url = RegExp(
  r'([A-Za-z][A-Za-z0-9+.\-]{0,31})://(?:([^\s/?#@]*)@)?([^\s/?#]*)([^\s]*)',
);

/// Wie viel Text [redactForLog] überhaupt ansieht.
///
/// Die zweite Grenze nach der des Schemas, und die, die für jedes künftige
/// Muster gilt: Kein Ausdruck in diesem Modul darf auf einer unbegrenzten
/// Eingabe laufen, weil ein einziger, der zurückspringt, das Fenster anhält,
/// während es einen Fehler aufschreibt. `AppLog` kürzt danach ohnehin auf
/// `maxLineBytes`; was hinter dieser Grenze stand, hätte die Datei nie
/// erreicht.
const int maxRedactedInput = 64 * 1024;

/// `Bearer <Token>` und `Basic <Base64>`.
final RegExp _credential = RegExp(
  r'\b(bearer|basic)\s+[A-Za-z0-9._~+/=\-]+',
  caseSensitive: false,
);

/// Ein Schlüssel, dessen Name das Geheimnis ankündigt, und sein Wert.
///
/// Die Anführungszeichen vor dem Doppelpunkt sind der Grund, warum dieser
/// Ausdruck so aussieht: In JSON steht `{"password": "hunter2"}`, und ein
/// Muster, das den Doppelpunkt unmittelbar hinter dem Namen erwartet, findet
/// genau den Fall nicht, wegen dem es dieses Modul gibt. Einfache und
/// doppelte, weil Meldungen beide schreiben -- die einfachen etwa, wenn ein
/// Paket einen Wert in seiner Meldung zitiert. **`Map.toString()` von Dart
/// schreibt gar keine**: `{password: two words here}`; was das kostet, steht
/// als Punkt 9 oben.
///
/// Hinter dem Namen darf das Anführungszeichen auch **maskiert** stehen
/// (`\\?["']`): JSON in einem JSON-String schreibt `{\"password\": ...}`, und
/// dort steht zwischen Name und Doppelpunkt `\"`.
///
/// Der Wert hat neun Formen, in dieser Reihenfolge -- erst alle
/// geschlossenen, dann die offenen, zuletzt die nackte:
///
/// 1. und 2. **In Anführungszeichen und geschlossen**, doppelt oder einfach.
///    Er reicht bis zum schließenden Zeichen, darf Leerzeichen enthalten,
///    und ein maskiertes Anführungszeichen darin gehört zum Wert
///    (`(?:[^"\\\n]|\\.)*`) -- JSON in JSON schreibt
///    `{"secret": "{\"nested\": \"value\"}"}`, und eine Form, die am ersten
///    `\"` abbräche, ließe den Rest im Klartext stehen.
/// 3. und 4. **In maskierten Anführungszeichen und geschlossen**
///    (`\"...\"`, `\'...\'`): ein geheimer Name **innerhalb** eines
///    JSON-Strings, wie in `{"body":"{\"password\":\"hunter2\"}"}`. Er reicht
///    bis zum ersten schließenden Paar, ganz gleich, was dazwischen steht --
///    auch über ` | #` und eine Ziffer hinweg.
/// 5. und 6. **In maskierten Anführungszeichen, die nie schließen**, und
/// 7. und 8. **in Anführungszeichen, die auf dieser Zeile nie schließen** --
///    abgeschnittener Text, eine Meldung, die nach dem Wert endet. Dann geht
///    der Rest der Zeile mit, **bis vor den Stapelabzug**: `AppLog` hängt ihn
///    mit ` | #0 …` an, und dieses Trennzeichen ist Text des Protokolls,
///    nicht Text der Ausnahme. Ohne den Halt davor ginge jeder Rahmen
///    verloren, sobald eine Meldung einen Wert offen lässt; mit ihm bleibt
///    der Stapel stehen und nur der Wert geht. **Der Preis, ausdrücklich:**
///    Ein offener Wert, der selbst ` | #` und eine Ziffer enthält, endet
///    dort, und was dahinter steht, bleibt im Klartext. Ein Geheimnis müsste
///    dafür das Trennzeichen dieses Protokolls wörtlich enthalten; das ist
///    Punkt 10 oben.
/// 9. **Nackt**, bis zum ersten Leerzeichen oder Trennzeichen. Sie trägt ein
///    öffnendes, auch maskiertes Anführungszeichen selbst mit
///    (`(?:\\?["'])?`); der Backslash gehört nicht zum Wert, damit ein
///    `\"` dahinter den Wert beendet statt verlängert, und die öffnende
///    Klammer auch nicht: Ein Setter im Stapelabzug heißt
///    `AuthStore.token= (package:…/store.dart:41:12)`, und ohne `(` im
///    Ausschluss nähme die nackte Form Datei und Zeile mit.
final RegExp _assignment = RegExp(
  // `access[_-]?token` steht neben `token`, obwohl `\btoken\b` wie eine
  // Abdeckung aussieht: Der Unterstrich ist ein Wortzeichen, also beginnt in
  // `access_token` kein Wort bei `token`, und der Name rutschte durch.
  // Dieselbe Falle für jeden künftigen Namen mit Unterstrich.
  r'''\b(authorization|auth|api[_-]?key|apikey|access[_-]?key'''
  r'''|access[_-]?token|refresh[_-]?token|token|secret'''
  r'''|password|passwd|pwd|cookie|session[_-]?id|credential)\b'''
  r'''(?:\\?["'])?\s*[:=]\s*(?:'''
  // 1 und 2: geschlossen, eingefasst.
  r'''"(?:[^"\\\n]|\\.)*"|'(?:[^'\\\n]|\\.)*'|'''
  // 3 und 4: geschlossen, maskiert eingefasst. Vor den offenen Formen, damit
  // ein geschlossener Wert nie am Trennzeichen des Stapels endet.
  r'''\\"(?:[^"\\\n]|\\.)*?\\"|\\'(?:[^'\\\n]|\\.)*?\\'|'''
  // 5 und 6: maskiert eingefasst und nie geschlossen, Halt vor dem Stapel.
  r'''\\"(?:(?!\\")(?! \| #\d).)*|'''
  r'''\\'(?:(?!\\')(?! \| #\d).)*|'''
  // 7 und 8: eingefasst und nie geschlossen, Halt vor dem Stapel.
  r'''"(?:(?! \| #\d)(?:[^"\\\n]|\\.))*|'''
  r'''['](?:(?! \| #\d)(?:[^'\\\n]|\\.))*|'''
  // 9: nackt.
  r'''(?:\\?["'])?[^\s"'\\&,;()\]}]+)''',
  caseSensitive: false,
);

/// Ein langer Lauf ohne Trenner: ein Schlüssel, ein Token, ein Stück Rumpf.
final RegExp _secretRun = RegExp('[A-Za-z0-9_+=-]{$secretRunLength,}');

/// Der Text einer Ausnahme, so wie er ins Protokoll darf.
///
/// Höchstens [maxRedactedInput] Zeichen davon; der Rest fällt vorher weg.
String redactForLog(String text) {
  String bounded = text;
  if (text.length > maxRedactedInput) {
    // Nie zwischen den beiden Hälften eines Zeichens außerhalb der
    // Grundebene schneiden: Ein einzelnes hohes Surrogat am Ende ist kein
    // Text mehr, sondern ein kaputtes Zeichen.
    final int last = text.codeUnitAt(maxRedactedInput - 1);
    final bool splitsPair = last >= 0xD800 && last <= 0xDBFF;
    bounded = text.substring(
      0,
      splitsPair ? maxRedactedInput - 1 : maxRedactedInput,
    );
  }
  String out = bounded.replaceAllMapped(_url, (Match match) {
    final String scheme = match.group(1) ?? '';
    final String userinfo = match.group(2) == null ? '' : '$redactedMarker@';
    final String host = match.group(3) ?? '';
    final String rest = match.group(4) ?? '';
    if (rest.isEmpty || rest == '/') {
      return '$scheme://$userinfo$host$rest';
    }
    return '$scheme://$userinfo$host/$removedPathMarker';
  });
  out = out.replaceAllMapped(
    _credential,
    (Match match) => '${match.group(1)} $redactedMarker',
  );
  out = out.replaceAllMapped(
    _assignment,
    (Match match) => '${match.group(1)}=$redactedMarker',
  );
  return out.replaceAll(_secretRun, redactedMarker);
}

/// Wie eine Ausnahme im Protokoll heißt.
///
/// Für die meisten ist das ihr `toString()`. Für eine `FormatException` nicht:
/// Deren `toString()` hängt die Quelle an, an der sie scheiterte, und beim
/// Lesen einer Antwort ist das der Rumpf. Übrig bleiben Typ, Meldung und
/// Versatz -- genug, um den Fehler zu finden, ohne das Gelesene mitzunehmen.
String describeErrorForLog(Object error) {
  if (error is FormatException) {
    final int? offset = error.offset;
    final String at = offset == null ? '' : ' (offset $offset)';
    return '${error.runtimeType}: ${error.message}$at';
  }
  return '$error';
}
