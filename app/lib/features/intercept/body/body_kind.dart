/// Was in einem Rumpf steht — beantwortet aus den Bytes, nicht aus der
/// Behauptung des Absenders.
///
/// Ein Rumpf kommt aus dem Netz, durch einen Agenten, den niemand kontrolliert,
/// und ein Mensch entscheidet auf dieser Grundlage über die Freigabe. Die
/// Erkennung entscheidet mit, was er dabei zu sehen bekommt, und sie folgt
/// deshalb einer Regel, die über die Reihenfolge der Spezifikation
/// hinausgeht: **eine Deklaration darf einschränken, nie behaupten.** Ein
/// `Content-Type: application/json` über einem Bild macht das Bild nicht zu
/// JSON; es macht die Kopfzeile zu einer Lüge, und die Anzeige folgt in
/// diesem Fall den Bytes (siehe [detectBodyKind]).
///
/// Nichts hier zeichnet, und nichts hier kennt Flutter: die Erkennung läuft
/// über 64 KiB in `Isolate.run` (`docs/UX.md` 7).
library;

import 'dart:typed_data';

/// Wie ein Rumpf angezeigt wird.
enum BodyKind {
  /// JSON, als Baum lesbar.
  json,

  /// `application/x-www-form-urlencoded`, als Tabelle lesbar.
  form,

  /// XML oder HTML; als Text lesbar, ohne eigenen Baum.
  xml,

  /// Text ohne erkannte Struktur.
  text,

  /// Bytes, die kein Text sind; nur als Hex lesbar.
  binary,

  /// Kein Rumpf. Ausdrücklich nicht dasselbe wie „nicht lesbar".
  empty,

  /// Größer als [bodyMaxBytes]; nichts davon wird zerlegt.
  tooLarge,
}

/// Ab hier wird ein Rumpf nicht mehr zerlegt: 8 MiB.
///
/// Die Grenze ist strikt größer — genau 8 MiB ist noch keine Überschreitung.
const int bodyMaxBytes = 8 * 1024 * 1024;

/// Wie viele Bytes die Hex-Ansicht zeigt und wie viel von einem zu großen
/// Rumpf lesbar bleibt: 64 KiB.
const int bodyHexLimit = 64 * 1024;

/// Wie viele Bytes das Schnüffeln ansieht.
const int bodySniffWindow = 512;

/// Ab welchem Anteil untypbarer Bytes ein Rumpf als Binärdaten gilt.
const double bodyBinaryRatio = 0.30;

/// Das Muster eines Formularrumpfs: `a=1&b=2`.
final RegExp bodyFormPattern = RegExp(r'^[^=&\s]+=[^&]*(&[^=&\s]+=[^&]*)*$');

/// Wie [contentType] einen Rumpf nennt, oder null für einen Typ, den diese
/// Anzeige nicht kennt.
///
/// Parameter werden abgeschnitten, der Rest wird kleingeschrieben verglichen.
BodyKind? kindOfContentType(String? contentType) {
  if (contentType == null) {
    return null;
  }
  final int semicolon = contentType.indexOf(';');
  final String mime =
      (semicolon < 0 ? contentType : contentType.substring(0, semicolon))
          .trim()
          .toLowerCase();
  if (mime.isEmpty) {
    return null;
  }
  if (mime == 'application/json' || mime.endsWith('+json')) {
    return BodyKind.json;
  }
  if (mime == 'application/x-www-form-urlencoded') {
    return BodyKind.form;
  }
  if (mime == 'application/xml' ||
      mime == 'text/xml' ||
      mime == 'text/html' ||
      mime.endsWith('+xml')) {
    return BodyKind.xml;
  }
  if (mime.startsWith('text/')) {
    return BodyKind.text;
  }
  return null;
}

/// Wie [bytes] aussehen, wenn niemand etwas behauptet.
///
/// Die Prüfung auf Binärdaten steht **vor** den Strukturmarken und nicht
/// dahinter, wie es die Reihenfolge der Spezifikation nahelegt. Ein Rumpf, der
/// mit `{` beginnt und dahinter zu 40 Prozent aus untypbaren Bytes besteht,
/// ist kein JSON, das gleich scheitern wird, sondern eine Datei mit einer
/// geschickt gewählten ersten Zeile; der Mensch soll sie als das sehen, was
/// sie ist.
BodyKind sniffBodyKind(Uint8List bytes) {
  final int from = _bomLength(bytes);
  final int end = bytes.length < from + bodySniffWindow
      ? bytes.length
      : from + bodySniffWindow;
  if (from >= end) {
    return BodyKind.text;
  }
  if (_isBinary(bytes, from, end)) {
    return BodyKind.binary;
  }
  for (int i = from; i < end; i++) {
    final int byte = bytes[i];
    if (byte == 0x20 || byte == 0x09 || byte == 0x0A || byte == 0x0D) {
      continue;
    }
    if (byte == 0x7B || byte == 0x5B) {
      return BodyKind.json;
    }
    if (byte == 0x3C) {
      return BodyKind.xml;
    }
    break;
  }
  if (_looksLikeForm(bytes, from, end)) {
    return BodyKind.form;
  }
  return BodyKind.text;
}

/// Wie ein Rumpf angezeigt wird.
///
/// Die Reihenfolge:
///
/// 1. Nichts da (`size == 0`) ist [BodyKind.empty].
/// 2. Mehr als [bodyMaxBytes] ist [BodyKind.tooLarge]; [totalSize] ist die
///    Größe des ganzen Rumpfs, auch wenn nur ein Präfix geladen wurde.
/// 3. Der `Content-Type` nennt einen Typ, den diese Anzeige kennt.
/// 4. Sonst entscheiden die Bytes ([sniffBodyKind]).
///
/// Zwischen 3 und 4 steht die Regel dieser Datei: sagt die Kopfzeile einen
/// Texttyp und sind die Bytes keine Textbytes, gewinnen die Bytes. Umgekehrt
/// gilt das nicht — eine Kopfzeile, die weniger behauptet, als dasteht, ist
/// keine Täuschung, und ein als `application/octet-stream` deklariertes JSON
/// wird über Schritt 4 trotzdem als Baum lesbar.
BodyKind detectBodyKind(
  Uint8List bytes,
  String? contentType, {
  int? totalSize,
}) {
  final int size = totalSize ?? bytes.length;
  if (size == 0) {
    return BodyKind.empty;
  }
  if (size > bodyMaxBytes) {
    return BodyKind.tooLarge;
  }
  final BodyKind sniffed = sniffBodyKind(bytes);
  final BodyKind? declared = kindOfContentType(contentType);
  if (declared == null) {
    return sniffed;
  }
  return sniffed == BodyKind.binary ? BodyKind.binary : declared;
}

/// Wahr, wenn der `Content-Type` etwas anderes sagt als die Bytes zeigen.
///
/// Nur für den Satz in der Ansicht: was gezeigt wird, steht schon fest.
bool bodyTypeIsDisputed(Uint8List bytes, String? contentType) {
  final BodyKind? declared = kindOfContentType(contentType);
  return declared != null && sniffBodyKind(bytes) == BodyKind.binary;
}

/// Die vier Ansichten auf denselben Rumpf.
///
/// Nicht `BodyViewMode`: so heißt der Notifier, der sich die gewählte Ansicht
/// je Flow merkt, und aus seinem Namen macht der Generator
/// `bodyViewModeProvider(FlowId)` (CONVENTIONS 3.9).
enum BodyPane {
  /// Der JSON-Baum.
  tree,

  /// Die Tabelle eines Formularrumpfs.
  form,

  /// Der Rohtext mit Zeilennummern.
  raw,

  /// Der Hex-Auszug.
  hex,
}

/// Welche Ansichten für [kind] etwas zeigen können, in der Reihenfolge des
/// Umschalters. Die erste ist die Vorauswahl.
List<BodyPane> bodyViewsFor(BodyKind kind) => switch (kind) {
  BodyKind.json => const <BodyPane>[BodyPane.tree, BodyPane.raw, BodyPane.hex],
  BodyKind.form => const <BodyPane>[BodyPane.form, BodyPane.raw, BodyPane.hex],
  BodyKind.xml ||
  BodyKind.text ||
  BodyKind.tooLarge => const <BodyPane>[BodyPane.raw, BodyPane.hex],
  BodyKind.binary => const <BodyPane>[BodyPane.hex, BodyPane.raw],
  BodyKind.empty => const <BodyPane>[],
};

/// Die Ansicht, die für [kind] gilt, wenn [chosen] gewählt wurde.
///
/// Eine Wahl, die diese Art nicht anbietet, verfällt still auf die Vorauswahl:
/// der Mensch hat Hex für einen Rumpf gewählt, nicht für jeden.
BodyPane effectiveBodyPane(BodyKind kind, BodyPane? chosen) {
  final List<BodyPane> panes = bodyViewsFor(kind);
  if (panes.isEmpty) {
    return BodyPane.raw;
  }
  return chosen != null && panes.contains(chosen) ? chosen : panes.first;
}

/// Wie viele Bytes die Byte-Reihenfolge-Marke am Anfang belegt.
int _bomLength(Uint8List bytes) =>
    bytes.length >= 3 &&
        bytes[0] == 0xEF &&
        bytes[1] == 0xBB &&
        bytes[2] == 0xBF
    ? 3
    : 0;

/// Der Anteil der Bytes, die weder druckbares ASCII noch Teil einer gültigen
/// UTF-8-Folge sind.
///
/// Eine an der Fenstergrenze abgeschnittene Folge zählt für keine der beiden
/// Seiten: sie sagt nichts darüber, was der Rumpf ist.
bool _isBinary(Uint8List bytes, int from, int end) {
  int opaque = 0;
  int seen = 0;
  int i = from;
  while (i < end) {
    final int byte = bytes[i];
    if (byte == 0x09 ||
        byte == 0x0A ||
        byte == 0x0D ||
        (byte >= 0x20 && byte <= 0x7E)) {
      i++;
      seen++;
      continue;
    }
    final int length = _sequenceLength(byte);
    if (length > 1) {
      if (i + length > end) {
        break;
      }
      if (_isSequence(bytes, i, length)) {
        i += length;
        seen++;
        continue;
      }
    }
    i++;
    seen++;
    opaque++;
  }
  return seen > 0 && opaque / seen > bodyBinaryRatio;
}

/// Die Länge der UTF-8-Folge, die mit [lead] beginnt, oder 1 für ein Byte,
/// das keine beginnt.
int _sequenceLength(int lead) {
  if (lead >= 0xC2 && lead <= 0xDF) {
    return 2;
  }
  if (lead >= 0xE0 && lead <= 0xEF) {
    return 3;
  }
  if (lead >= 0xF0 && lead <= 0xF4) {
    return 4;
  }
  return 1;
}

/// Wahr, wenn auf [start] eine gültige Folge der Länge [length] steht.
bool _isSequence(Uint8List bytes, int start, int length) {
  for (int i = 1; i < length; i++) {
    final int byte = bytes[start + i];
    if (byte < 0x80 || byte > 0xBF) {
      return false;
    }
  }
  return true;
}

/// Wahr, wenn das Fenster wie ein Formularrumpf aussieht.
///
/// Reicht der Rumpf über das Fenster hinaus, wird beim letzten `&` geschnitten:
/// ein an einer beliebigen Stelle abgeschnittenes Paar würde das verankerte
/// Muster sonst immer scheitern lassen.
bool _looksLikeForm(Uint8List bytes, int from, int end) {
  final StringBuffer buffer = StringBuffer();
  for (int i = from; i < end; i++) {
    buffer.writeCharCode(bytes[i]);
  }
  String probe = buffer.toString();
  if (bytes.length > end) {
    final int cut = probe.lastIndexOf('&');
    if (cut <= 0) {
      return false;
    }
    probe = probe.substring(0, cut);
  }
  return probe.isNotEmpty && bodyFormPattern.hasMatch(probe);
}
