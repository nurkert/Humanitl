// Gemeinsamer Aufbau der Widget-Tests: die App über einem
// `FakeDaemonClient`, ohne Herzschlag (Tests schalten ihn gezielt ein),
// in einem Fenster von 1280×800.

import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/app.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/features/shell/providers/connection.dart';

/// Baut die App über [client] und pumpt, bis das Verbindungs-Gate steht.
Future<void> pumpApp(
  WidgetTester tester, {
  required DaemonClient client,
  Duration? heartbeat,
  Size size = const Size(1280, 800),
}) async {
  await tester.binding.setSurfaceSize(size);
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    ProviderScope(
      overrides: [
        daemonClientProvider.overrideWithValue(client),
        connectionHeartbeatProvider.overrideWithValue(heartbeat),
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
