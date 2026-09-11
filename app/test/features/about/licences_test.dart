// Die Lizenz der Rangliste auf Flutters Lizenzseite (HUM-031): Die gebündelte
// Kopie ist `catalog/RANKS-LICENSE`, und die Seite zeigt deren Wortlaut.

import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/features/about/licences.dart';

/// Das Original; `flutter test` läuft in `app/`.
final File original = File('../catalog/RANKS-LICENSE');

/// [text] als Folge seiner Wörter.
///
/// Die Lizenzseite bricht Zeilen neu um und nimmt Einrückung als Absatzmaß;
/// verglichen wird deshalb, was gelesen wird, nicht der Leerraum.
List<String> words(String text) => text
    .split(RegExp(r'\s+'))
    .where((String word) => word.isNotEmpty)
    .toList(growable: false);

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  // Die Bindung trägt beim Start die Lizenzen der Pakete ein. Jeder Test hier
  // sieht nur, was er selbst einträgt.
  setUp(LicenseRegistry.reset);
  tearDown(LicenseRegistry.reset);

  test(
    'the bundled ranking licence is catalog/RANKS-LICENSE, byte for byte',
    () async {
      final ByteData bundled = await rootBundle.load(ranksLicenceAsset);
      expect(
        Uint8List.sublistView(bundled),
        original.readAsBytesSync(),
        reason:
            'app/assets/catalog/RANKS-LICENSE is a copy of '
            'catalog/RANKS-LICENSE and has drifted. Refresh it from the '
            'repository root with '
            '`cp catalog/RANKS-LICENSE app/assets/catalog/RANKS-LICENSE`.',
      );
    },
  );

  test('the licence page carries the ranking in the words of '
      'catalog/RANKS-LICENSE', () async {
    registerBundledLicences();

    final List<LicenseEntry> entries = await LicenseRegistry.licenses.toList();
    final LicenseEntry ranks = entries.singleWhere(
      (LicenseEntry entry) => entry.packages.contains(ranksLicencePackage),
    );
    final String shown = ranks.paragraphs
        .map((LicenseParagraph paragraph) => paragraph.text)
        .join(' ');

    expect(words(shown), words(original.readAsStringSync()));
  });
}
