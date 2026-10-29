// Kein Test liest den Zustandsspeicher des Menschen, der ihn laufen lässt
// (HUM-307).
//
// `UiStateFile.resolve()` zeigt auf `~/.local/state/humanitl/ui-state.json`.
// Sieben Testdateien bauten die ganze App ohne eigenen Speicher und lasen
// damit diese Datei; wer die Leiste der Shell aufgeklappt hatte (HUM-254),
// bekam schmalere Panes als die CI, und `action_bar_de_test` war lokal rot
// und in der CI grün. Schreiben hätten sie auch können: Der Hinweis der
// Aktionsleiste setzt seine Marke beim ersten Zeigen.
//
// Geprüft wird am Quelltext, für jeden Bau einzeln: Zwischen dem vorigen
// `HumanitlApp` der Datei (oder ihrem Anfang) und diesem steht ein
// `uiStateOverride(` oder ein eigenes `uiStateFileProvider.overrideWith`.
// Eine Datei mit einem geschützten und einem ungeschützten Bau fällt so
// ebenfalls auf.

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// The builds of the app in [source] without their own UI state file, as
/// line numbers.
List<int> unguardedBuilds(String source) {
  const String build = 'HumanitlApp()';
  final List<int> lines = <int>[];
  int from = 0;
  for (
    int at = source.indexOf(build);
    at >= 0;
    at = source.indexOf(build, at + 1)
  ) {
    final String before = source.substring(from, at);
    if (!before.contains('uiStateOverride(') &&
        !before.contains('uiStateFileProvider.overrideWith')) {
      lines.add('\n'.allMatches(source.substring(0, at)).length + 1);
    }
    from = at + build.length;
  }
  return lines;
}

void main() {
  test('the guard sees each build, not the file', () {
    // Der erste Bau ist geschützt, der zweite nicht.
    const String source =
        'overrides: [uiStateOverride()], child: const HumanitlApp(),\n'
        'overrides: [], child: const HumanitlApp(),\n';
    expect(unguardedBuilds(source), <int>[2]);
  });

  test('every test that builds the app brings its own UI state file', () {
    final List<String> leaking = <String>[
      for (final FileSystemEntity entity in Directory(
        'test',
      ).listSync(recursive: true))
        if (entity is File &&
            entity.path.endsWith('.dart') &&
            !entity.path.endsWith('ui_state_guard_test.dart'))
          for (final int line in unguardedBuilds(entity.readAsStringSync()))
            '${entity.path}:$line',
    ];
    expect(
      leaking,
      isEmpty,
      reason:
          'these files read ~/.local/state/humanitl/ui-state.json; add '
          "uiStateOverride() from 'test/harness/ui_state.dart' to their "
          'ProviderScope',
    );
  });
}
