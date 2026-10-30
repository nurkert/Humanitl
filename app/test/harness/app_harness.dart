// Gemeinsamer Aufbau der Widget-Tests: die App über einem
// `FakeDaemonClient`, ohne Herzschlag (Tests schalten ihn gezielt ein),
// in einem Fenster von 1280×800.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/app.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/features/shell/providers/connection.dart';

import 'ui_state.dart';

/// Baut die App über [client] und pumpt, bis das Verbindungs-Gate steht.
Future<void> pumpApp(
  WidgetTester tester, {
  required DaemonClient client,
  Duration? heartbeat,
  Duration? reconnect,
  List<Override> overrides = const <Override>[],
  Override? uiState,
  Size size = const Size(1280, 800),
}) async {
  await tester.binding.setSurfaceSize(size);
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    ProviderScope(
      overrides: [
        daemonClientProvider.overrideWithValue(client),
        connectionHeartbeatProvider.overrideWithValue(heartbeat),
        // Ohne Frist versucht die Verbindung nichts von selbst; ein Test, der
        // den Zwei-Sekunden-Takt pruefen will, setzt ihn (HUM-044).
        connectionReconnectProvider.overrideWithValue(reconnect),
        uiState ?? uiStateOverride(),
        ...overrides,
      ],
      child: const HumanitlApp(),
    ),
  );
  // Ein Frame für `GetInfo`, einer für das Ergebnis.
  await tester.pump();
  await tester.pump();
}

/// Drückt [key] mit gehaltener Strg-Taste.
Future<void> pressCtrl(WidgetTester tester, LogicalKeyboardKey key) async {
  await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
  await tester.sendKeyEvent(key);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
  await tester.pump();
}

/// Klappt die Leiste mit ihrem Griff um: `Ctrl+Shift+0` (HUM-254; seit
/// HUM-356 mit `Shift`, weil `Ctrl+0` die Skalierung zurücksetzt).
Future<void> pressRailToggle(WidgetTester tester) async {
  await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
  await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
  await tester.sendKeyEvent(LogicalKeyboardKey.digit0);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
  await tester.pump();
}

/// Opens the setup step [kind] (`SetupCheckKind.name`) by its "Details"
/// button when it stands as one line (HUM-287).
///
/// A green step is folded to one line, and so is every step behind the one in
/// front; a test that looks at the controls of such a step opens it first,
/// the way a person would. A step that is already open is left as it is.
Future<void> openSetupStep(WidgetTester tester, String kind) async {
  if (find.byKey(Key('setup-open-$kind')).evaluate().isNotEmpty) {
    return;
  }
  final Finder toggle = find.byKey(Key('setup-step-toggle-$kind'));
  await tester.ensureVisible(toggle);
  await tester.pump();
  await tester.tap(toggle);
  await tester.pump();
}
