// Notification und Tray über der ganzen App (HUM-034, HUM-054): die Meldung
// beim Übergang von null auf eine wartende Anfrage, und die Zahl im Tray, die
// mit der Warteschlange steigt und fällt.
//
// `attention_test.dart` prüft die Maschine dahinter ohne Widgets; hier läuft
// der Weg vom Ereignisstrom des Fake-Daemons durch die Queue und den
// `TrayHost` bis an die gefälschten Ports.

import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/app.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/intercept/providers/now.dart';
import 'package:humanitl/features/shell/providers/connection.dart';
import 'package:humanitl/features/tray/desktop_ports.dart';
import 'package:humanitl/features/tray/providers/attention.dart';

import 'fixtures.dart';

/// Die App über [client] und [desktop]; [focused] falsch nimmt dem Fenster
/// den Fokus, bevor die erste Anfrage eintrifft.
Future<void> pumpDesktop(
  WidgetTester tester, {
  required FakeDaemonClient client,
  required FakeDesktop desktop,
  required bool focused,
}) async {
  await tester.binding.setSurfaceSize(const Size(1400, 900));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  final ProviderContainer container = ProviderContainer(
    overrides: <Override>[
      daemonClientProvider.overrideWithValue(client),
      connectionHeartbeatProvider.overrideWithValue(null),
      desktopPortsProvider.overrideWithValue(desktop.ports),
      // Eine stehende Uhr: Der 250-ms-Ticker der Queue liesse sonst einen
      // Timer zurück.
      nowProvider.overrideWith(() => TrayFixedNow(DateTime.now())),
    ],
  );
  addTearDown(container.dispose);
  await tester.pumpWidget(
    UncontrolledProviderScope(container: container, child: const HumanitlApp()),
  );
  await tester.pump();
  await tester.pump();
  if (!focused) {
    desktop.window.emit(focused: false);
    await tester.pump();
  }
  // 100 ms Versatz: Das Skript hält jede Anfrage 1 ms nach ihrer Ankunft an,
  // und ein Schritt von ganzen Sekunden landete sonst genau dazwischen.
  await tester.pump(const Duration(milliseconds: 100));
}

/// Lässt die Fenster ablaufen, die eine Meldung oder eine Entscheidung
/// hinterlässt (Bündelung, Rückgängig, Bestätigung der Zeile); ein Test, der
/// sie stehen lässt, endet mit einem laufenden Timer.
Future<void> drain(WidgetTester tester) async {
  await tester.pump(const Duration(seconds: 11));
  await tester.pump();
}

/// Lässt [duration] Skriptzeit ablaufen und die Ereignisse ankommen.
Future<void> advance(WidgetTester tester, Duration duration) async {
  await tester.pump(duration);
  await tester.pump();
  await tester.pump();
}

void main() {
  // Linux: Fenster, Tray und Notification sind die eines Linux-Desktops, und
  // `flutter test` spielt ohne Variante Android.
  final TargetPlatformVariant linux = TargetPlatformVariant.only(
    TargetPlatform.linux,
  );

  testWidgets('notification_sent_on_zero_to_one', (WidgetTester tester) async {
    final FakeDesktop desktop = FakeDesktop();
    await pumpDesktop(
      tester,
      client: FakeDaemonClient(
        script: trayHoldScript(2, spacing: const Duration(seconds: 2)),
      ),
      desktop: desktop,
      focused: false,
    );
    expect(desktop.notifications.posts, isEmpty, reason: 'nothing waits yet');

    // Von null auf eins: genau eine Meldung, mit dem Host als Überschrift.
    await advance(tester, const Duration(seconds: 2));
    expect(desktop.notifications.posts, hasLength(1));
    expect(desktop.notifications.posts.single.summary, 'api.github.com');
    expect(desktop.notifications.posts.single.flowId, trayFlowId(1));

    // Von eins auf zwei ist kein neuer Übergang von null: Die stehende Meldung
    // wird höchstens ergänzt, nie durch eine zweite für dieselbe Lage
    // ersetzt, die wieder bei der ersten Anfrage anfinge.
    await advance(tester, const Duration(seconds: 2));
    expect(
      desktop.notifications.posts.where(
        (DesktopNotification post) => post.flowId == trayFlowId(2),
      ),
      isEmpty,
      reason: 'the message names the longest waiting request, not the newest',
    );
    await drain(tester);
  }, variant: linux);

  testWidgets('no notification on zero to one under the eye', (
    WidgetTester tester,
  ) async {
    // Die Gegenprobe: Dieselbe Ankunft bei fokussiertem Fenster meldet sich
    // nicht auf dem Schreibtisch. Ohne sie bewiese der Test oben nur, dass
    // jede Ankunft eine Meldung macht.
    final FakeDesktop desktop = FakeDesktop();
    await pumpDesktop(
      tester,
      client: FakeDaemonClient(
        script: trayHoldScript(1, spacing: const Duration(seconds: 2)),
      ),
      desktop: desktop,
      focused: true,
    );
    await advance(tester, const Duration(seconds: 2));
    expect(desktop.notifications.posts, isEmpty);
    expect(desktop.tray.face.count, 1, reason: 'the request did arrive');
    await drain(tester);
  }, variant: linux);

  testWidgets('tray_badge_count', (WidgetTester tester) async {
    final FakeDesktop desktop = FakeDesktop();
    final FakeDaemonClient client = FakeDaemonClient(
      script: trayHoldScript(3, spacing: const Duration(seconds: 1)),
    );
    await pumpDesktop(tester, client: client, desktop: desktop, focused: true);
    expect(desktop.tray.face.state, TrayIconState.idle);
    expect(desktop.tray.face.count, 0);

    for (int held = 1; held <= 3; held++) {
      await advance(tester, const Duration(seconds: 1));
      expect(desktop.tray.face.state, TrayIconState.held);
      expect(desktop.tray.face.count, held, reason: 'after arrival $held');
      expect(desktop.window.titles.last, '($held) Humanitl');
    }

    // Eine Entscheidung nimmt eine Anfrage aus der Warteschlange und eine aus
    // der Zahl.
    await client.decide(trayFlowId(2), const Decision.block());
    await advance(tester, const Duration(milliseconds: 10));
    expect(desktop.tray.face.count, 2);
    expect(desktop.window.titles.last, '(2) Humanitl');
    await drain(tester);
  }, variant: linux);
}
