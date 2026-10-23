// Die Reste einer Zwischenablage an Endpunkt und Zugangsschlüssel (HUM-283):
// dieselbe Regel wie im Daemon, und nur der eingefügte Teil wird bereinigt.

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ui/pasted_text.dart';

TextEditingValue _value(String text, [int? cursor]) => TextEditingValue(
  text: text,
  selection: TextSelection.collapsed(offset: cursor ?? text.length),
);

void main() {
  test('only a matching pair goes, otherwise one quote at one edge', () {
    expect(stripPasteDebris('"abc"'), 'abc');
    expect(stripPasteDebris("'abc'"), 'abc');
    expect(stripPasteDebris('" abc "\n'), 'abc');
    expect(stripPasteDebris('abc"'), 'abc');
    expect(stripPasteDebris('"abc'), 'abc');
    expect(stripPasteDebris('"abc\''), "abc'");
    expect(stripPasteDebris('\u{feff}abc'), 'abc');
    expect(stripPasteDebris('""a b""'), '"a b"');
  });

  const PasteDebrisFormatter formatter = PasteDebrisFormatter();

  test('a paste at the cursor is cleaned, the text around it is not', () {
    // Ein Leerzeichen, das schon im Feld stand, bleibt; nur das Eingefügte
    // verliert sein Anführungszeichen.
    final TextEditingValue old = _value('a b', 3);
    final TextEditingValue out = formatter.formatEditUpdate(
      old,
      _value('a b/v1/"'),
    );
    expect(out.text, 'a b/v1/');
    expect(out.selection, const TextSelection.collapsed(offset: 7));
  });

  test('a paste that replaces a selection counts as a paste', () {
    const String endpoint = 'https://h/v1/';
    const TextEditingValue old = TextEditingValue(
      text: endpoint,
      selection: TextSelection(baseOffset: 0, extentOffset: endpoint.length),
    );
    final TextEditingValue out = formatter.formatEditUpdate(
      old,
      _value('$endpoint"'),
    );
    expect(out.text, endpoint);
  });

  test('a typed character stays, also a quote', () {
    final TextEditingValue old = _value('https://h/v1/');
    final TextEditingValue typed = _value('https://h/v1/"');
    expect(formatter.formatEditUpdate(old, typed), typed);
  });
}
