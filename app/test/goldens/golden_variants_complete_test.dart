// Der Meta-Test aus HUM-054: Zu jedem Golden-Namen der Liste gibt es genau
// die vier Varianten-Dateien, und keine Varianten-Datei steht ohne Namen da.
//
// Ein Golden, dessen Datei fehlt, schlägt in alchemist nicht fehl, solange
// niemand den Test ausführt, der sie erzeugt; ein gelöschter Test hinterlässt
// Bilder, die nichts mehr bewachen. Beides findet dieser Test, ohne ein
// einziges Pixel zu rendern.
@Tags(<String>['golden'])
library;

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import 'support/golden_names.dart';
import 'support/golden_variants.dart';

/// Wo alchemist die Bilder des CI-Modus ablegt, vom Paket aus gesehen.
final Directory ciGoldens = Directory('test/goldens/goldens/ci');

/// `<name>.<variante>.png`, mit einer der vier Varianten.
final RegExp variantFile = RegExp(
  r'^(.+)\.(dark-en|dark-de|light-en|light-de)\.png$',
);

/// Die Dateinamen unter [ciGoldens].
Set<String> filesOnDisk() => <String>{
  for (final FileSystemEntity entity in ciGoldens.listSync())
    if (entity is File) entity.uri.pathSegments.last,
};

/// `goldenVariants(` mit dem Namen als erstem Argument, über Zeilen hinweg.
final RegExp variantsCall = RegExp(r"goldenVariants\(\s*'([a-z0-9_]+)'");

/// Die Namen, die die Golden-Tests dieses Verzeichnisses wirklich anlegen.
///
/// Gelesen aus dem Quelltext, nicht aus einem Lauf: Der Meta-Test läuft in
/// seinem eigenen Prozess und sieht die Tests der anderen Dateien nicht.
Set<String> namesInTests() => <String>{
  for (final FileSystemEntity entity in Directory('test/goldens').listSync())
    if (entity is File && entity.path.endsWith('_golden_test.dart'))
      for (final RegExpMatch match in variantsCall.allMatches(
        entity.readAsStringSync(),
      ))
        match.group(1)!,
};

void main() {
  test('every listed golden is rendered by a test', () {
    // Ein gelöschter `goldenVariants`-Aufruf ließe vier Bilder und einen
    // Namen in der Liste zurück, die niemand mehr vergleicht.
    expect(namesInTests(), goldenNames.toSet());
  });

  test('golden_variants_complete', () {
    expect(ciGoldens.existsSync(), isTrue, reason: '${ciGoldens.path} exists');
    expect(goldenSlugs, <String>['dark-en', 'dark-de', 'light-en', 'light-de']);
    expect(
      goldenNames.toSet(),
      hasLength(goldenNames.length),
      reason: 'every golden name is listed once',
    );

    final Set<String> files = filesOnDisk();
    final List<String> missing = <String>[
      for (final String name in goldenNames)
        for (final String slug in goldenSlugs)
          if (!files.contains('$name.$slug.png')) '$name.$slug.png',
    ];
    expect(
      missing,
      isEmpty,
      reason: 'regenerate with `make goldens-update` and check the images in',
    );

    final List<String> orphans = <String>[
      for (final String file in files)
        if (variantFile.firstMatch(file) case final RegExpMatch match)
          if (!goldenNames.contains(match.group(1))) file,
    ]..sort();
    expect(
      orphans,
      isEmpty,
      reason:
          'a variant image without a name in goldenNames guards nothing; '
          'delete it or list its name',
    );
  });
}
