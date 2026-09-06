// Was eine Schaltfläche der Notification entscheidet, wenn der Dienst nicht
// mehr antwortet (HUM-044, `docs/UX.md` 4.2, Fall 4).
//
// Die Meldung überlebt die Verbindung, über die sie hinausging: Sie wird
// zurückgenommen, indem `withdraw` gerufen wird, und dieser Ruf ist unbeachtet
// und läuft über einen Bus, den es womöglich auch nicht mehr gibt. Ein Druck
// im selben Augenblick kommt trotzdem an. Er darf dann nichts senden, und er
// darf auch nicht still bleiben.
//
// Umkehrungen, gegen die diese beiden Tests stehen:
//
// - Der Riegel `if (!ref.read(linkLiveProvider))` in `_TrayHostState._decide`
//   weg: `a_notification_decides_nothing_into_a_frozen_shell` wird rot, weil
//   der Befund im Benachrichtigungsschlitz ausbleibt -- der Fehlschlag landete
//   dann in der Fehlerkarte der Aktionsleiste, also innerhalb der
//   eingefrorenen Fläche.
// - Dass der Druck überhaupt etwas tut, steht im zweiten Test.

import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/app.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/connection.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/intercept/providers/now.dart';
import 'package:humanitl/features/tray/desktop_ports.dart';
import 'package:humanitl/features/tray/providers/attention.dart';
import 'package:humanitl/features/tray/providers/notice.dart';

import '../tray/fixtures.dart';

/// Baut die App über [client] und [desktop], ohne Fokus, damit eine Meldung
/// hinausgeht.
///
/// [heartbeat] bleibt null, solange die Verbindung stehen soll: Ein laufender
/// Herzschlag ist am Ende des Tests ein offener Timer, und darauf sieht
/// `flutter_test` nach. Wer den Bruch braucht, setzt ihn -- der Fehlschlag
/// hält ihn selbst wieder an.
Future<ProviderContainer> pumpNotified(
  WidgetTester tester, {
  required FakeDaemonClient client,
  required FakeDesktop desktop,
  Duration? heartbeat,
}) async {
  await tester.binding.setSurfaceSize(const Size(1400, 900));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  final ProviderContainer container = ProviderContainer(
    overrides: <Override>[
      daemonClientProvider.overrideWithValue(client),
      connectionHeartbeatProvider.overrideWithValue(heartbeat),
      connectionReconnectProvider.overrideWithValue(null),
      desktopPortsProvider.overrideWithValue(desktop.ports),
      // Eine stehende Uhr: der 250-ms-Ticker der Queue liesse den Test mit
      // einem laufenden Timer enden.
      nowProvider.overrideWith(() => TrayFixedNow(DateTime.now())),
    ],
  );
  addTearDown(container.dispose);
  await tester.pumpWidget(
    UncontrolledProviderScope(container: container, child: const HumanitlApp()),
  );
  await tester.pump();
  await tester.pump();
  // Der Fokus geht, bevor die erste Anfrage eintrifft: nur dann geht eine
  // Meldung hinaus.
  desktop.window.emit(focused: false);
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 400));
  await tester.pump();
  return container;
}

void main() {
  testWidgets('a_press_on_the_message_decides_while_the_connection_stands', (
    WidgetTester tester,
  ) async {
    final FakeDesktop desktop = FakeDesktop();
    final FakeDaemonClient client = FakeDaemonClient(script: trayHoldScript(1));
    final ProviderContainer container = await pumpNotified(
      tester,
      client: client,
      desktop: desktop,
    );
    expect(desktop.notifications.posts, isNotEmpty);

    desktop.notifications.press(NotificationActionKind.allow);
    await tester.pump();
    await tester.pump();

    // Ohne diese Zusage bewiese der Test danach nichts.
    expect(client.decisions, hasLength(1));
    expect(container.read(attentionNoticeProvider), isNull);
    await tester.pump(const Duration(seconds: 11));
    await tester.pump();
  });

  testWidgets('a_notification_decides_nothing_into_a_frozen_shell', (
    WidgetTester tester,
  ) async {
    final FakeDesktop desktop = FakeDesktop();
    final FakeDaemonClient client = FakeDaemonClient(script: trayHoldScript(1));
    final ProviderContainer container = await pumpNotified(
      tester,
      client: client,
      desktop: desktop,
      heartbeat: const Duration(seconds: 1),
    );
    expect(desktop.notifications.posts, isNotEmpty);

    // Der Dienst fällt weg; der Herzschlag merkt es.
    client.goOffline();
    await tester.pump(const Duration(seconds: 1));
    await tester.pump();
    expect(container.read(connectionStateProvider), isA<ConnectionFrozen>());

    // Die Meldung steht noch, und jemand drückt darauf.
    desktop.notifications.press(NotificationActionKind.allow);
    await tester.pump();
    await tester.pump();

    expect(client.decisions, isEmpty);
    // Und die Absage steht dort, wo sie zu lesen ist: im
    // Benachrichtigungsschlitz über dem Schnappschuss, nicht in der
    // Fehlerkarte der Aktionsleiste innerhalb der eingefrorenen Fläche.
    expect(
      container.read(attentionNoticeProvider)?.code,
      DiagnosticCodes.daemonUnreachable,
    );
    expect(desktop.window.reveals, 1);
  });
}
