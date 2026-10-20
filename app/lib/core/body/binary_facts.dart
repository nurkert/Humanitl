/// Zwei Sätze über einen binären Rumpf (HUM-058, Zeile 10): was die ersten
/// Bytes vermuten lassen, und dass der Daemon in ihm nur druckbare Läufe
/// durchsucht hat.
///
/// Beides ist eine Anzeige, keine Entscheidung. Die Vermutung sagt, wonach
/// die Bytes aussehen, und behauptet nicht, was sie sind: Die ersten Bytes
/// wählt der Absender, und ein Mensch soll sie als Hinweis lesen. Der zweite
/// Satz spiegelt die Regel des Detektors, damit die Zahl der Funde richtig
/// gelesen wird: null Funde in einem Rumpf, der nur über seine druckbaren
/// Läufe durchsucht wurde, sind weniger als null Funde in einem Text.
library;

import 'dart:convert';
import 'dart:typed_data';

/// Die bekannten Anfänge, längster Präfix zuerst wo nötig.
///
/// Eine kleine Tabelle der Formate, die ein Agent typischerweise hochlädt;
/// was hier fehlt, bekommt keine Vermutung, und der Satz entfällt.
const List<(List<int?>, String)> _magic = <(List<int?>, String)>[
  (<int>[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A], 'image/png'),
  (<int>[0xFF, 0xD8, 0xFF], 'image/jpeg'),
  (<int>[0x47, 0x49, 0x46, 0x38], 'image/gif'),
  (
    <int?>[
      0x52,
      0x49,
      0x46,
      0x46,
      null,
      null,
      null,
      null,
      0x57,
      0x45,
      0x42,
      0x50,
    ],
    'image/webp',
  ),
  (<int>[0x25, 0x50, 0x44, 0x46, 0x2D], 'application/pdf'),
  (<int>[0x50, 0x4B, 0x03, 0x04], 'application/zip'),
  (<int>[0x1F, 0x8B], 'application/gzip'),
  (<int>[0x42, 0x5A, 0x68], 'application/x-bzip2'),
  (<int>[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C], 'application/x-7z-compressed'),
  (<int>[0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00], 'application/x-xz'),
  (<int>[0x28, 0xB5, 0x2F, 0xFD], 'application/zstd'),
  (<int>[0x00, 0x61, 0x73, 0x6D], 'application/wasm'),
  (<int>[0x7F, 0x45, 0x4C, 0x46], 'application/x-elf'),
  (
    <int>[0x53, 0x51, 0x4C, 0x69, 0x74, 0x65, 0x20, 0x66],
    'application/vnd.sqlite3',
  ),
];

/// Der MIME-Typ, nach dem die ersten Bytes von [bytes] aussehen, oder null.
String? guessMimeFromMagic(Uint8List bytes) {
  for (final (List<int?> prefix, String mime) in _magic) {
    if (bytes.length < prefix.length) {
      continue;
    }
    bool match = true;
    for (int i = 0; i < prefix.length; i++) {
      final int? want = prefix[i];
      if (want != null && bytes[i] != want) {
        match = false;
        break;
      }
    }
    if (match) {
      return mime;
    }
  }
  // `ustar` an Stelle 257 ist das Kennzeichen eines tar-Archivs.
  if (bytes.length >= 262 &&
      ascii.decode(bytes.sublist(257, 262), allowInvalid: true) == 'ustar') {
    return 'application/x-tar';
  }
  return null;
}

/// Wahr, wenn der Daemon einen Rumpf mit [contentType] und [bytes] nur über
/// seine druckbaren Läufe durchsucht.
///
/// Spiegel von `is_textual` in `daemon/crates/findings/src/input.rs` und
/// `ContentType::is_textual` in `daemon/crates/findings/src/content_type.rs`:
/// Mit einem `Content-Type` entscheidet allein der Typ (`text/*`, JSON,
/// Formular, XML, JavaScript, GraphQL, `+json`, `+xml`), ohne ihn, ob die
/// Bytes gültiges UTF-8 sind. Alles andere geht durch `printable_only`.
/// [bytes] sind die ausgepackten Bytes, die der Detektor gesehen hat, oder ihr
/// geladener Anfang; bei einem gekürzten Anfang kann die UTF-8-Probe an
/// einem abgeschnittenen Zeichen scheitern, und dann sagt der Satz zu viel
/// Vorsicht, nie zu wenig.
bool scannedAsStrings(String? contentType, Uint8List bytes) {
  final String essence = _essence(contentType);
  if (essence.isEmpty) {
    try {
      utf8.decode(bytes);
      return false;
    } on FormatException {
      return true;
    }
  }
  if (essence.startsWith('text/')) {
    return false;
  }
  const Set<String> textual = <String>{
    'application/json',
    'application/x-www-form-urlencoded',
    'application/xml',
    'application/javascript',
    'application/graphql',
  };
  if (textual.contains(essence)) {
    return false;
  }
  final int slash = essence.indexOf('/');
  final String subtype = slash < 0 ? '' : essence.substring(slash + 1);
  return !(subtype.endsWith('+json') || subtype.endsWith('+xml'));
}

String _essence(String? contentType) {
  if (contentType == null) {
    return '';
  }
  final int semicolon = contentType.indexOf(';');
  return (semicolon < 0 ? contentType : contentType.substring(0, semicolon))
      .trim()
      .toLowerCase();
}
