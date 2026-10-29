// Proportionale Schrift für Layouttests (HUM-307).
//
// Im Test zeichnet Flutter jede Glyphe als Quadrat von einem Geviert. Das ist
// überall gleich und deshalb für Goldens richtig, aber rund doppelt so breit
// wie Inter: „Blockieren" misst 130 px statt etwa 65. Ein Test, der prüfen
// will, was in einem 480 px schmalen Pane nebeneinander passt, misst mit den
// Quadraten eine Leiste, die es auf keinem Bildschirm gibt.
//
// Die App liefert ihre Schriften nicht mit (`HType`), und die Schriften des
// Rechners unterscheiden sich zwischen CI und Entwicklerrechner. Roboto liegt
// dagegen in jedem Flutter-SDK unter `bin/cache/artifacts/material_fonts`,
// in derselben Version wie das SDK selbst; sie ist ähnlich breit wie Inter.
// Geladen wird sie unter dem Namen der UI-Familie, nur in der Testdatei, die
// das hier aufruft. Die Monospace-Familie bleibt bei den Quadraten: Die
// Tastenkürzel neben den Beschriftungen fallen damit etwas breiter aus als
// mit JetBrains Mono, und ein Test, der mit ihnen passt, passt erst recht.

import 'dart:io';

import 'package:flutter/services.dart';
import 'package:humanitl_ui/humanitl_ui.dart';

/// Where the fonts of the Flutter SDK lie.
///
/// `FLUTTER_ROOT` when the tool passed it on, otherwise derived from the
/// test runner, which lies under `bin/cache/artifacts/engine/<platform>/`.
Directory sdkFontDirectory() {
  final String? root = Platform.environment['FLUTTER_ROOT'];
  final List<Directory> candidates = <Directory>[
    if (root != null && root.isNotEmpty)
      Directory('$root/bin/cache/artifacts/material_fonts'),
    Directory(
      '${File(Platform.resolvedExecutable).parent.parent.parent.path}'
      '/material_fonts',
    ),
  ];
  for (final Directory candidate in candidates) {
    if (File('${candidate.path}/Roboto-Regular.ttf').existsSync()) {
      return candidate;
    }
  }
  throw StateError(
    'Roboto not found in the Flutter SDK; looked in '
    '${candidates.map((Directory d) => d.path).join(', ')}. '
    'Run `flutter precache` once.',
  );
}

/// Loads Roboto in its three weights under the name of the UI family.
///
/// Call it once per test file, in `setUpAll`.
Future<void> loadLayoutFonts() async {
  final Directory fonts = sdkFontDirectory();
  final FontLoader loader = FontLoader(HType.uiFamily);
  for (final String weight in <String>['Regular', 'Medium', 'Bold']) {
    final File file = File('${fonts.path}/Roboto-$weight.ttf');
    loader.addFont(
      Future<ByteData>.value(ByteData.sublistView(file.readAsBytesSync())),
    );
  }
  await loader.load();
}
