// Die gemerkte Aufteilung eines Bildschirms (HUM-305): was aus der Datei
// gelesen wird, was hineingeschrieben wird, und was ein kaputter Eintrag
// bedeutet.

import 'dart:convert';
import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ui/pane_split.dart';
import 'package:humanitl/core/ui_state/ui_state_file.dart';

const PaneSplitSpec _spec = PaneSplitSpec('paneSplit.test', <double>[0.6, 0.4]);

/// Ein Container über einer Datei, die [stored] unter dem Schlüssel hält;
/// ohne Angabe gibt es die Datei nicht.
(ProviderContainer, UiStateFile) _over(Object? stored, {bool write = true}) {
  final Directory dir = Directory.systemTemp.createTempSync('hum305-unit');
  addTearDown(() => dir.deleteSync(recursive: true));
  final UiStateFile store = UiStateFile('${dir.path}/${UiStateFile.fileName}');
  if (write) {
    File(store.path).writeAsStringSync(
      jsonEncode(<String, Object?>{_spec.key: stored, 'other': true}),
    );
  }
  final ProviderContainer container = ProviderContainer(
    overrides: <Override>[uiStateFileProvider.overrideWithValue(store)],
  );
  addTearDown(container.dispose);
  return (container, store);
}

void main() {
  test('without a file the default split stands', () {
    final (ProviderContainer container, _) = _over(null, write: false);
    expect(container.read(paneSplitProvider(_spec)), <double>[0.6, 0.4]);
  });

  test('a stored split comes back, normalised to a sum of one', () {
    final (ProviderContainer container, _) = _over(<double>[3, 1]);
    expect(container.read(paneSplitProvider(_spec)), <double>[0.75, 0.25]);
  });

  test('a malformed entry is the default split', () {
    for (final Object? stored in <Object?>[
      'wide',
      <double>[1],
      <double>[0.3, 0.3, 0.4],
      <Object?>[0.5, 'x'],
      <double>[0.5, 0],
      <double>[1.2, -0.2],
      <String, Object?>{'a': 1},
      // Die Summe läuft über, oder ein Pane bekäme nach dem Teilen nichts.
      <double>[1e308, 1e308],
      <double>[1e308, 1e-308],
    ]) {
      final (ProviderContainer container, _) = _over(stored);
      expect(container.read(paneSplitProvider(_spec)), <double>[
        0.6,
        0.4,
      ], reason: '$stored');
    }
  });

  test(
    'set changes memory only; commit and reset write, and keep the rest',
    () {
      final (ProviderContainer container, UiStateFile store) = _over(null);
      final PaneSplit split = container.read(paneSplitProvider(_spec).notifier);

      split.set(<double>[1, 1]);
      expect(container.read(paneSplitProvider(_spec)), <double>[0.5, 0.5]);
      expect(store.read()[_spec.key], isNull, reason: 'a drag is not over yet');

      split.commit();
      expect(store.read()[_spec.key], <double>[0.5, 0.5]);
      expect(store.read()['other'], true);

      split.reset();
      expect(container.read(paneSplitProvider(_spec)), <double>[0.6, 0.4]);
      expect(store.read()[_spec.key], <double>[0.6, 0.4]);
    },
  );

  test('set ignores ratios of another number of panes or without size', () {
    final (ProviderContainer container, _) = _over(null, write: false);
    final PaneSplit split = container.read(paneSplitProvider(_spec).notifier);
    split
      ..set(<double>[1, 1, 1])
      ..set(<double>[0, 0])
      ..set(<double>[1e308, 1e308])
      ..set(<double>[1e308, 1e-308]);
    expect(container.read(paneSplitProvider(_spec)), <double>[0.6, 0.4]);
  });
}
