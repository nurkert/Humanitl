// Nichts bewegt sich unter dem lesenden Auge (docs/UX.md 2.8, HUM-029).
//
// Die eingefrorene Sicht lebt im `State` des Panes; nur der Zähler der
// ausstehenden Ankünfte ist ein Provider, weil ihn die Pille und die Ansage
// teilen (docs/UX.md 8). Geprüft wird deshalb, was man sieht: wie viele Zeilen
// stehen, was die Pille sagt, und wann zusammengeführt wird.

import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/intercept/providers/queue_freeze.dart';
import 'package:humanitl/features/intercept/widgets/queue_pane.dart';
import 'package:humanitl/features/intercept/widgets/queue_row.dart';

import 'harness.dart';

/// Zwei Anfragen an zwei Domains: die erste sofort, die zweite nach 1 s.
List<ScriptedEvent> twoArrivals() => <ScriptedEvent>[
  ...arriveAt(held(1, host: 'registry.npmjs.org'), Duration.zero),
  ...arriveAt(
    held(2, host: 'api.github.com', path: '/graphql'),
    const Duration(seconds: 1),
  ),
];

/// Wie viele Zeilen die Queue gerade zeigt.
int rows() => find.byType(QueueRow).evaluate().length;

/// Fährt mit dem Zeiger in das Queue-Pane und bleibt dort.
Future<TestGesture> enterQueue(WidgetTester tester) async {
  final TestGesture gesture = await tester.createGesture(
    kind: PointerDeviceKind.mouse,
  );
  await gesture.addPointer(location: Offset.zero);
  addTearDown(gesture.removePointer);
  await gesture.moveTo(tester.getCenter(find.byType(QueuePane)));
  await tester.pump();
  return gesture;
}

void main() {
  testWidgets('freeze_while_hover', (WidgetTester tester) async {
    await pumpIntercept(tester, client: fakeDaemon(twoArrivals()));
    await playScript(tester, const Duration(milliseconds: 100));
    expect(rows(), 1);

    final TestGesture pointer = await enterQueue(tester);
    // Die zweite Anfrage trifft ein, während der Zeiger im Pane steht.
    await tester.pump(const Duration(seconds: 1));
    await tester.pump();

    expect(rows(), 1, reason: 'no row may appear under the pointer');
    expect(containerOf(tester).read(pendingArrivalsProvider), hasLength(1));
    expect(find.text('+1 new'), findsOneWidget);

    // Zeiger raus, dann 600 ms: zusammengeführt.
    await pointer.moveTo(const Offset(5, 5));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 600));
    await tester.pump();
    await tester.pumpAndSettle();

    expect(rows(), 2);
    expect(containerOf(tester).read(pendingArrivalsProvider), isEmpty);
    expect(find.text('+1 new'), findsNothing);
  });

  testWidgets('freeze_after_keyboard_nav', (WidgetTester tester) async {
    await pumpIntercept(tester, client: fakeDaemon(twoArrivals()));
    await playScript(tester, const Duration(milliseconds: 100));

    await tester.sendKeyEvent(LogicalKeyboardKey.keyJ);
    await tester.pump();

    // 1,3 s nach der letzten Tastaturnavigation ist die Anfrage da, die
    // Sperre aber noch nicht abgelaufen: nichts wird eingefügt.
    await tester.pump(const Duration(milliseconds: 1200));
    await tester.pump();
    expect(rows(), 1);
    expect(containerOf(tester).read(pendingArrivalsProvider), hasLength(1));

    // Nach `HMotion.freezeAfterKey` (2 s) läuft die Sperre ab.
    await tester.pump(const Duration(milliseconds: 1200));
    await tester.pump();
    await tester.pumpAndSettle();
    expect(rows(), 2);
  });

  testWidgets('shift+J takes the arrivals in, without a pointer', (
    WidgetTester tester,
  ) async {
    await pumpIntercept(tester, client: fakeDaemon(twoArrivals()));
    await playScript(tester, const Duration(milliseconds: 100));

    await tester.sendKeyEvent(LogicalKeyboardKey.keyJ);
    await tester.pump(const Duration(milliseconds: 1200));
    await tester.pump();
    expect(rows(), 1);

    await pressShiftKey(tester, LogicalKeyboardKey.keyJ);
    await tester.pumpAndSettle();

    expect(rows(), 2);
    expect(containerOf(tester).read(pendingArrivalsProvider), isEmpty);
  });

  testWidgets('the pill is a control, and clicking it merges', (
    WidgetTester tester,
  ) async {
    await pumpIntercept(tester, client: fakeDaemon(twoArrivals()));
    await playScript(tester, const Duration(milliseconds: 100));
    final TestGesture pointer = await enterQueue(tester);
    await tester.pump(const Duration(milliseconds: 1200));
    await tester.pump();

    expect(find.byKey(const Key('intercept-new-pill')), findsOneWidget);
    await tester.tap(find.byKey(const Key('intercept-new-pill')));
    await tester.pumpAndSettle();
    await pointer.moveTo(const Offset(5, 5));

    expect(rows(), 2);
  });

  testWidgets('arrivals are announced in one bundle, politely', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fakeDaemon(<ScriptedEvent>[
      ...arriveAt(held(1, host: 'registry.npmjs.org'), Duration.zero),
      ...arriveAt(
        held(2, host: 'api.github.com'),
        const Duration(milliseconds: 20),
      ),
      ...arriveAt(held(3, host: 'pypi.org'), const Duration(milliseconds: 40)),
    ]);
    await pumpIntercept(tester, client: client);
    final List<String> said = captureAnnouncements(tester);
    await playScript(tester, const Duration(milliseconds: 100));

    // Höchstens eine Ansage je zwei Sekunden, und sie nennt die älteste
    // (docs/UX.md 6).
    expect(said.length, lessThanOrEqualTo(2));
    expect(
      said.where((String line) => line.contains('new, oldest')),
      isNotEmpty,
    );
  });
}
