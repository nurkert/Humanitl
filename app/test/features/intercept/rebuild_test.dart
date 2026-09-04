// Das Gatter aus docs/UX.md 7: eine Entscheidung baut die Shell nicht neu
// und kostet zwei Builds je sichtbarer Zeile, nicht zweihundert.
//
// Rebuild-Umfang ist für Compiler und Goldens unsichtbar und regressiert
// sonst still; deshalb ein Zähler und keine Absichtserklärung.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/app.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/intercept/intercept_screen.dart';
import 'package:humanitl/features/intercept/providers/now.dart';
import 'package:humanitl/features/intercept/widgets/queue_pane.dart';
import 'package:humanitl/features/intercept/widgets/queue_row.dart';
import 'package:humanitl/features/intercept/widgets/request_card.dart';
import 'package:humanitl/features/shell/providers/connection.dart';
import 'package:humanitl/features/shell/shell_screen.dart';
import 'package:humanitl/features/shell/widgets/header_bar.dart';
import 'package:humanitl/features/shell/widgets/status_bar.dart';

import 'fixtures.dart';

// Jeder Flow bekommt eine eigene registrierbare Domain, damit die Queue
// flache Zeilen zeigt und nicht eine Gruppe (HUM-029): gemessen wird hier der
// Bau der Zeile, nicht die Gruppierung.
FlowDetail detail(int n) => detailFor(
  heldFlow(
    n: n,
    deadline: testStart.add(Duration(minutes: 5, seconds: n)),
    host: 'host$n.example$n.org',
    path: '/thing/$n',
  ),
);

Future<FakeDaemonClient> pumpQueue(WidgetTester tester, int count) async {
  final FakeDaemonClient client = FakeDaemonClient(
    script: holdScript(<FlowDetail>[
      for (int i = 1; i <= count; i++) detail(i),
    ]),
    clock: () => testStart,
  );
  await tester.binding.setSurfaceSize(const Size(1400, 900));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    ProviderScope(
      overrides: <Override>[
        daemonClientProvider.overrideWithValue(client),
        connectionHeartbeatProvider.overrideWithValue(null),
        nowProvider.overrideWith(() => FixedNow(testStart)),
      ],
      child: const HumanitlApp(),
    ),
  );
  await tester.pump();
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 400));
  await tester.pump();
  return client;
}

void main() {
  testWidgets('a decision leaves the shell untouched', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = await pumpQueue(tester, 6);

    final Element header = tester.element(find.byType(HeaderBar));
    final Element status = tester.element(find.byType(StatusBar));
    final Element shell = tester.element(find.byType(ShellScreen));
    final Element screen = tester.element(find.byType(InterceptScreen));
    final Element queue = tester.element(find.byType(QueuePane));

    await tester.sendKeyDownEvent(LogicalKeyboardKey.keyA);
    // Was hier nicht schmutzig ist, baut in diesem Frame auch nicht neu.
    expect(header.dirty, isFalse, reason: 'header bar');
    expect(status.dirty, isFalse, reason: 'status bar');
    expect(shell.dirty, isFalse, reason: 'shell');
    expect(screen.dirty, isFalse, reason: 'intercept screen');
    // Das Queue-Pane hört auf den Schnappschuss und darf neu bauen; die
    // Zeilen darin zählt der nächste Test.
    expect(queue.dirty, isFalse, reason: 'queue pane before the frame');

    await tester.sendKeyUpEvent(LogicalKeyboardKey.keyA);
    await tester.pump();
    expect(client.decisions, hasLength(1));
  });

  testWidgets('a decision costs a bounded number of row builds', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = await pumpQueue(tester, 6);
    final int rows = find.byType(QueueRow).evaluate().length;
    expect(rows, 6);

    debugQueueRowBuilds = 0;
    await tester.sendKeyEvent(LogicalKeyboardKey.keyA);
    await tester.pump();
    await tester.pump();

    expect(client.decisions, hasLength(1));
    // Zwei Builds je sichtbarer Zeile: einer für den neuen Schnappschuss,
    // einer für die Auswahl, die weiterwandert (docs/UX.md 7).
    // ignore: avoid_print
    print('DECISION BUILDS: $debugQueueRowBuilds for $rows rows');
    expect(debugQueueRowBuilds, lessThanOrEqualTo(rows * 2));
  });

  testWidgets('a splitter frame builds neither the card nor a row', (
    WidgetTester tester,
  ) async {
    await pumpQueue(tester, 6);
    final Element card = tester.element(find.byType(RequestCard));

    debugQueueRowBuilds = 0;
    final Rect queue = tester.getRect(find.byType(QueuePane));
    final TestGesture gesture = await tester.startGesture(
      Offset(queue.right + 3, 400),
    );
    await tester.pump();
    for (int i = 0; i < 5; i++) {
      await gesture.moveBy(const Offset(8, 0));
      await tester.pump();
      // Der Ziehzustand liegt in einem `ValueNotifier` des Layouts; nichts
      // darüber baut je Zeigerbewegung neu (docs/UX.md 7).
      expect(card.dirty, isFalse, reason: 'card at step $i');
    }
    await gesture.up();
    await tester.pump();

    expect(debugQueueRowBuilds, 0);
  });

  testWidgets('a second of the clock costs at most one build per row', (
    WidgetTester tester,
  ) async {
    await pumpQueue(tester, 6);
    final int rows = find.byType(QueueRow).evaluate().length;
    final ProviderContainer container = ProviderScope.containerOf(
      tester.element(find.byType(InterceptScreen)),
    );

    debugQueueRowBuilds = 0;
    (container.read(nowProvider.notifier) as FixedNow).moveTo(
      testStart.add(const Duration(seconds: 1)),
    );
    await tester.pump();

    // ignore: avoid_print
    print('CLOCK BUILDS: $debugQueueRowBuilds for $rows rows');
    expect(debugQueueRowBuilds, lessThanOrEqualTo(rows));
  });
}
