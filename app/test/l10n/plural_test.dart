// ICU-Plural in beiden Sprachen (HUM-052): `=1` und `other` getrennt, und
// die Wörter aus dem Glossar (Fund, nicht Finding).

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/l10n/l10n.dart';

void main() {
  final AppLocalizations en = lookupAppLocalizations(const Locale('en'));
  final AppLocalizations de = lookupAppLocalizations(const Locale('de'));

  test('plural_de_one_vs_other', () {
    expect(de.interceptSendWithFindings(1), 'Senden mit 1 Fund');
    expect(de.interceptSendWithFindings(3), 'Senden mit 3 Funden');
    expect(de.interceptGroupFindings(0), '0 Funde');
    expect(de.interceptGroupFindings(1), '1 Fund');
    expect(de.interceptGroupFindings(2), '2 Funde');
  });

  test('plural_en_one_vs_other', () {
    expect(en.interceptSendWithFindings(1), 'Send with 1 finding');
    expect(en.interceptSendWithFindings(3), 'Send with 3 findings');
  });

  test('an apostrophe survives use-escaping', () {
    expect(en.sandboxStopBody, contains("agent's terminal"));
    expect(en.setupIntro, contains("daemon's answer"));
    expect(en.editorHeaderInvalidName('x'), contains(r"!#$%&'*+-.^_`|~"));
  });
}
