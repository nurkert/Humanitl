// Drückt die Taste eines Aktivators aus dem Katalog (HUM-420).
//
// Die Tests der Bindungen lesen die Taste aus dem Katalog, nicht aus einer
// eigenen Tabelle: Ein Eintrag, dessen Taste niemand bindet, fällt so auf.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/shortcuts/intents.dart';

/// Hält die Modifikatoren von [activator], drückt seine Taste unter Linux und
/// lässt alles wieder los. Wahr, wenn jemand das Drücken der Taste behandelt
/// hat.
Future<bool> pressActivator(
  WidgetTester tester,
  ShortcutActivator activator,
) async {
  final (
    LogicalKeyboardKey key,
    List<LogicalKeyboardKey> held,
  ) = switch (activator) {
    final SingleActivator a => (
      a.trigger,
      <LogicalKeyboardKey>[
        if (a.control) LogicalKeyboardKey.controlLeft,
        if (a.shift) LogicalKeyboardKey.shiftLeft,
        if (a.alt) LogicalKeyboardKey.altLeft,
        if (a.meta) LogicalKeyboardKey.metaLeft,
      ],
    ),
    // Unter Linux: `Ctrl+Shift` mit der Taste.
    TerminalClipboardActivator() => (
      activator.triggers.first,
      <LogicalKeyboardKey>[
        LogicalKeyboardKey.controlLeft,
        LogicalKeyboardKey.shiftLeft,
      ],
    ),
    _ => throw UnsupportedError('${activator.runtimeType}'),
  };
  for (final LogicalKeyboardKey modifier in held) {
    await tester.sendKeyDownEvent(modifier, platform: 'linux');
  }
  final bool handled = await tester.sendKeyDownEvent(key, platform: 'linux');
  await tester.sendKeyUpEvent(key, platform: 'linux');
  for (final LogicalKeyboardKey modifier in held.reversed) {
    await tester.sendKeyUpEvent(modifier, platform: 'linux');
  }
  await tester.pump();
  return handled;
}
