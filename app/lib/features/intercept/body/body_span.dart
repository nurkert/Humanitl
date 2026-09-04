/// Wo ein Fund im Rumpf steht, und wie Text aus einem Rumpf angezeigt werden
/// darf.
///
/// Zwei Dinge, die zusammengehören, weil beide an derselben Zahl hängen: der
/// Zeichenversatz. Der Daemon meldet Funde in **Byte**-Versätzen, Flutter
/// zeichnet in **UTF-16-Code-Units**, und jede Bereinigung des Textes, die
/// dazwischen die Länge veränderte, verschöbe jede Markierung dahinter. Die
/// Bereinigung in [sanitizeBodyText] ersetzt deshalb Zeichen eins zu eins und
/// löscht nie eines.
///
/// Frei von Flutter, damit die Zuordnung in `Isolate.run` laufen kann
/// (`docs/UX.md` 7).
library;

import 'dart:convert';
import 'dart:typed_data';

import '../../../core/domain/domain.dart';

/// Der Kanal, in dem ein Fund markiert wird.
///
/// Zwei, nicht acht: in einer Rumpf-Ansicht sind Funde die einzige Chroma
/// (`docs/UX.md` 3.3, Regel 7), und zwei Töne sind das Meiste, was sich
/// nebeneinander noch unterscheiden lässt.
enum BodyFindingTone {
  /// Ein Geheimnis: Schlüssel, Token, Kontonummer, Kartennummer.
  secret,

  /// Ein persönliches Datum: Adresse, Nummer, ein Begriff des Nutzers.
  personal,
}

/// Welcher Ton zu einem Fund gehört.
///
/// Eine Art, die hier nicht steht, gilt als Geheimnis. Ein unbekannter Fund
/// wird lieber zu ernst genommen als zu leicht.
BodyFindingTone bodyFindingTone(String kind) {
  final int colon = kind.indexOf(':');
  final String base = colon < 0 ? kind : kind.substring(0, colon);
  return switch (base) {
    'email' || 'phone' || 'ipv4' || 'user_term' => BodyFindingTone.personal,
    _ => BodyFindingTone.secret,
  };
}

/// Ein Fund, umgerechnet auf die Zeichen des angezeigten Textes.
///
/// Ein einfacher Wert ohne Flutter-Typ und ohne `freezed`, damit er durch
/// `Isolate.run` passt.
class BodyFinding {
  /// Creates a mapped finding.
  const BodyFinding({
    required this.index,
    required this.kind,
    required this.tier,
    required this.tone,
    required this.byteStart,
    required this.byteEnd,
    required this.charStart,
    required this.charEnd,
    required this.needle,
  });

  /// Die Stelle in `FlowDetail.findings`; sie ist der Name dieses Fundes in
  /// jeder Ansicht.
  final int index;

  /// Die Art, wie der Daemon sie schreibt, zum Beispiel `api_key:github`.
  final String kind;

  /// Wie sicher der Fund ist.
  final FindingTier tier;

  /// Der Ton, in dem markiert wird.
  final BodyFindingTone tone;

  /// Erstes Byte des Treffers.
  final int byteStart;

  /// Erstes Byte hinter dem Treffer.
  final int byteEnd;

  /// Erste Code-Unit des Treffers im angezeigten Text.
  final int charStart;

  /// Erste Code-Unit hinter dem Treffer.
  final int charEnd;

  /// Der Treffertext selbst, bereinigt. Der Baum sucht damit den Wert, in dem
  /// der Fund steckt; die Bytes allein sagen dort nichts.
  final String needle;

  /// Wahr, solange der Treffer Länge hat.
  bool get hasRange => charEnd > charStart;
}

/// Die Funde von [findings], die im Rumpf stehen, auf [bytes] umgerechnet.
///
/// Ein Durchgang für alle: die Byte-Versätze werden sortiert und der Text
/// einmal gestückelt dekodiert. Das ist derselbe Dekodierer, der den Text
/// später zeichnet, also stimmen die Zahlen auch bei kaputtem UTF-8 überein.
///
/// [place] ist falsch, wenn diese Bytes **nicht** die sind, auf denen der
/// Daemon gesucht hat — ein `Content-Encoding`, das diese Ansicht nicht
/// auspacken kann, ist der Fall. Dann behält jeder Fund seinen Namen, bekommt
/// aber keine Stelle: eine Markierung im falschen Byteraum zeigt auf einen
/// Wert, der dort nie stand, und das ist schlimmer als gar keine Markierung.
List<BodyFinding> mapBodyFindings(
  Uint8List bytes,
  List<Finding> findings, {
  bool place = true,
}) {
  final List<int> wanted = <int>[];
  if (place) {
    for (int i = 0; i < findings.length; i++) {
      final Finding finding = findings[i];
      if (finding.location != FindingLocation.body) {
        continue;
      }
      if (finding.spanStart >= bytes.length) {
        continue;
      }
      wanted
        ..add(finding.spanStart.clamp(0, bytes.length))
        ..add(finding.spanEnd.clamp(0, bytes.length));
    }
  }
  final Map<int, int> chars = byteToCharOffsets(bytes, wanted);
  final List<BodyFinding> mapped = <BodyFinding>[];
  for (int i = 0; i < findings.length; i++) {
    final Finding finding = findings[i];
    if (finding.location != FindingLocation.body) {
      continue;
    }
    if (!place) {
      mapped.add(
        BodyFinding(
          index: i,
          kind: finding.kind,
          tier: finding.tier,
          tone: bodyFindingTone(finding.kind),
          byteStart: 0,
          byteEnd: 0,
          charStart: 0,
          charEnd: 0,
          needle: '',
        ),
      );
      continue;
    }
    if (finding.spanStart >= bytes.length) {
      // Der Fund liegt hinter dem, was geladen wurde -- bei einem zu großen
      // Rumpf hinter den ersten 64 KiB. Ihn ans Ende zu klemmen erfände eine
      // Stelle und einen Treffertext, den es hier nicht gibt; er behält den
      // Namen und verliert die Stelle.
      mapped.add(
        BodyFinding(
          index: i,
          kind: finding.kind,
          tier: finding.tier,
          tone: bodyFindingTone(finding.kind),
          byteStart: finding.spanStart,
          byteEnd: finding.spanStart,
          charStart: 0,
          charEnd: 0,
          needle: '',
        ),
      );
      continue;
    }
    final int start = finding.spanStart.clamp(0, bytes.length);
    final int end = finding.spanEnd.clamp(start, bytes.length);
    mapped.add(
      BodyFinding(
        index: i,
        kind: finding.kind,
        tier: finding.tier,
        tone: bodyFindingTone(finding.kind),
        byteStart: start,
        byteEnd: end,
        charStart: chars[start] ?? 0,
        charEnd: chars[end] ?? chars[start] ?? 0,
        needle: sanitizeBodyText(
          const Utf8Decoder(allowMalformed: true).convert(bytes, start, end),
        ),
      ),
    );
  }
  return mapped;
}

/// Die Zeichenversätze zu [byteOffsets], in einem Durchgang.
Map<int, int> byteToCharOffsets(Uint8List bytes, List<int> byteOffsets) {
  final List<int> sorted = byteOffsets.toSet().toList()..sort();
  final Map<int, int> result = <int, int>{};
  if (sorted.isEmpty) {
    return result;
  }
  int chars = 0;
  final ByteConversionSink sink = const Utf8Decoder(allowMalformed: true)
      .startChunkedConversion(
        _CountingSink((String chunk) => chars += chunk.length),
      );
  int consumed = 0;
  for (final int offset in sorted) {
    final int end = offset.clamp(0, bytes.length);
    if (end > consumed) {
      sink.addSlice(bytes, consumed, end, false);
      consumed = end;
    }
    result[offset] = chars;
  }
  sink.close();
  return result;
}

/// Der Zeichenversatz zu einem einzelnen [byteOffset].
///
/// Der Umweg über [byteToCharOffsets] kostet einen Durchgang; wer mehrere
/// braucht, ruft die Mehrzahl.
int byteToCharOffset(Uint8List bytes, int byteOffset) =>
    byteToCharOffsets(bytes, <int>[byteOffset])[byteOffset] ?? 0;

/// Was statt eines unsichtbaren oder richtungsdrehenden Zeichens steht.
const String bodyReplacementChar = '�';

/// [text], so wie eine Rumpf-Ansicht ihn zeigen darf.
///
/// Ersetzt wird alles, was die Anzeige eines Wertes verändern kann, ohne
/// selbst sichtbar zu sein: die Steuerzeichen C0 und C1 außer Tabulator,
/// Zeilenvorschub und Wagenrücklauf, die Zeilen- und Absatztrenner, und jedes
/// Formatzeichen der Kategorie `Cf` — die Richtungssteuerzeichen ebenso wie
/// die Tag-Zeichen ab `U+E0020`, mit denen sich ganze Sätze unsichtbar in
/// einen Wert legen lassen. Ein Rumpf, der `123\u{202E}456` schreibt, zeigt in
/// einem gewöhnlichen Textfeld `123654`; ein Mensch, der über die Freigabe
/// genau dieses Wertes entscheidet, liest dann etwas, das so nirgends gesendet
/// wird.
///
/// Jede Ersetzung ist genau eine Code-Unit lang — bei einem Zeichen außerhalb
/// der Basisebene also zwei für zwei —, also bleibt jeder Versatz gültig, den
/// [mapBodyFindings] berechnet hat.
String sanitizeBodyText(String text) {
  List<int>? units;
  for (int i = 0; i < text.length; i++) {
    final int unit = text.codeUnitAt(i);
    if (unit >= 0xD800 && unit <= 0xDBFF && i + 1 < text.length) {
      final int low = text.codeUnitAt(i + 1);
      if (low >= 0xDC00 && low <= 0xDFFF) {
        final int rune = 0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00);
        if (!_isDisplayableRune(rune)) {
          units ??= List<int>.of(text.codeUnits);
          units[i] = 0xFFFD;
          units[i + 1] = 0xFFFD;
        }
        i++;
        continue;
      }
    }
    if (!_isDisplayableRune(unit)) {
      units ??= List<int>.of(text.codeUnits);
      units[i] = 0xFFFD;
    }
  }
  return units == null ? text : String.fromCharCodes(units);
}

/// Wahr, wenn dieses Zeichen gezeigt werden darf.
bool _isDisplayableRune(int rune) {
  if (rune == 0x09 || rune == 0x0A || rune == 0x0D) {
    return true;
  }
  // C0, Delete und C1.
  if (rune < 0x20 || (rune >= 0x7F && rune <= 0x9F)) {
    return false;
  }
  if (rune < 0x00AD) {
    return true;
  }
  return switch (rune) {
    // Weiches Trennzeichen und die arabischen Formatzeichen.
    0x00AD || 0x061C || 0x06DD || 0x070F || 0x08E2 || 0x180E => false,
    // Mongolisch, Khmer und die Zahlzeichenpräfixe.
    >= 0x0600 && <= 0x0605 => false,
    // Nullbreite Zeichen und die Richtungsmarken.
    >= 0x200B && <= 0x200F => false,
    // Zeilen- und Absatztrenner.
    0x2028 || 0x2029 => false,
    // Einbettung, Aufhebung, Überschreibung, Isolate.
    >= 0x202A && <= 0x202E => false,
    >= 0x2060 && <= 0x206F => false,
    // Byte-Reihenfolge-Marke mitten im Text und die Annotationszeichen.
    0xFEFF => false,
    >= 0xFFF9 && <= 0xFFFB => false,
    // Formatzeichen außerhalb der Basisebene.
    0x110BD || 0x110CD => false,
    >= 0x13430 && <= 0x1343F => false,
    >= 0x1BCA0 && <= 0x1BCA3 => false,
    >= 0x1D173 && <= 0x1D17A => false,
    // Tag-Zeichen: ein ganzer unsichtbarer Satz in einem Wert.
    0xE0001 => false,
    >= 0xE0020 && <= 0xE007F => false,
    // Variantenselektoren der Ergänzung.
    >= 0xE0100 && <= 0xE01EF => false,
    _ => true,
  };
}

/// Zählt, was der gestückelte Dekodierer ausgibt.
class _CountingSink implements Sink<String> {
  _CountingSink(this._count);

  final void Function(String chunk) _count;

  @override
  void add(String data) => _count(data);

  @override
  void close() {}
}
