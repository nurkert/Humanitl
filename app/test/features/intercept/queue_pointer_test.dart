// Zwei Zusagen der Warteschlange gegenüber dem, der gerade liest oder tippt
// (HUM-054, `docs/UX.md` 2.8):
//
//  * `queue_not_reordered_under_pointer`: Steht der Zeiger auf Zeile 3 und
//    trifft eine Anfrage ein, die nach ihrer Frist ganz oben stünde, steht
//    unter dem Zeiger weiter dieselbe Anfrage.
//  * `focus_not_stolen_on_new_flow`: Wer im Editor tippt, behält den Fokus,
//    wenn eine neue Anfrage eintrifft.
//
// `queue_freeze_test.dart` zählt die Zeilen; hier geht es darum, welche
// Anfrage an welcher Stelle steht und wo der Fokus ist.

import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/intercept/widgets/queue_pane.dart';
import 'package:humanitl/features/intercept/widgets/queue_row.dart';

import 'fixtures.dart';
import 'harness.dart';

/// Die Anfragen der Zeilen, von oben nach unten.
List<FlowId> rowOrder(WidgetTester tester) {
  final List<Element> rows = find.byType(QueueRow).evaluate().toList()
    ..sort(
      (Element a, Element b) => tester
          .getTopLeft(find.byElementPredicate((Element e) => e == a))
          .dy
          .compareTo(
            tester
                .getTopLeft(find.byElementPredicate((Element e) => e == b))
                .dy,
          ),
    );
  return <FlowId>[
    for (final Element row in rows) (row.widget as QueueRow).flow.id,
  ];
}

/// Die Anfrage der Zeile, deren Fläche [point] enthält.
FlowId? rowAt(WidgetTester tester, Offset point) {
  for (final Element element in find.byType(QueueRow).evaluate()) {
    final Rect rect = tester.getRect(
      find.byElementPredicate((Element e) => e == element),
    );
    if (rect.contains(point)) {
      return (element.widget as QueueRow).flow.id;
    }
  }
  return null;
}

/// Drei Anfragen an drei Domains, dann nach 2 s eine vierte, deren Frist vor
/// allen anderen abläuft: Nach Frist sortiert gehörte sie ganz nach oben.
List<ScriptedEvent> threeThenUrgent() => <ScriptedEvent>[
  ...arriveAt(held(1, host: 'registry.npmjs.org'), Duration.zero),
  ...arriveAt(
    held(2, host: 'api.github.com', apex: 'github.com', path: '/graphql'),
    const Duration(milliseconds: 10),
  ),
  ...arriveAt(
    held(3, host: 'pypi.org', apex: 'pypi.org', path: '/simple/requests/'),
    const Duration(milliseconds: 20),
  ),
  ...arriveAt(
    held(4, host: 'crates.io', apex: 'crates.io', path: '/api/v1/crates'),
    const Duration(seconds: 2),
    budget: const Duration(seconds: 30),
  ),
];

void main() {
  // Linux: Zeiger, Fokus und Textfeld verhalten sich je Plattform anders, und
  // `flutter test` spielt ohne Variante Android.
  final TargetPlatformVariant linux = TargetPlatformVariant.only(
    TargetPlatform.linux,
  );

  testWidgets('queue_not_reordered_under_pointer', (WidgetTester tester) async {
    final FakeDaemonClient client = fakeDaemon(threeThenUrgent());
    await pumpIntercept(tester, client: client);
    await playScript(tester, const Duration(milliseconds: 100));

    final List<FlowId> before = rowOrder(tester);
    expect(before, hasLength(3));
    final FlowId third = before[2];

    // Der Zeiger steht auf Zeile 3 und bleibt dort.
    final Offset point = tester.getCenter(
      find.byWidgetPredicate((Widget w) => w is QueueRow && w.flow.id == third),
    );
    final TestGesture pointer = await tester.createGesture(
      kind: PointerDeviceKind.mouse,
    );
    await pointer.addPointer(location: Offset.zero);
    addTearDown(pointer.removePointer);
    await pointer.moveTo(point);
    await tester.pump();

    // Die dringende Anfrage trifft ein.
    await tester.pump(const Duration(seconds: 2));
    await tester.pump();
    await tester.pump();

    expect(rowAt(tester, point), third, reason: 'the row under the pointer');
    expect(rowOrder(tester), before, reason: 'no row moved under the pointer');

    // Zeiger weg: Erst jetzt wird zusammengeführt, und die neue Anfrage steht
    // oben. Ohne diesen Teil bewiese der Test nichts, denn eine Anfrage, die
    // auch so unten einsortiert würde, verschöbe Zeile 3 nie.
    await pointer.moveTo(tester.getCenter(find.byType(QueuePane)) * 4);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 600));
    await tester.pumpAndSettle();
    final List<FlowId> after = rowOrder(tester);
    expect(after, hasLength(4));
    expect(after.first, testFlowId(4), reason: 'the urgent one sorts first');
    expect(after.indexOf(third), 3, reason: 'row 3 moved down once merged');
  }, variant: linux);

  testWidgets('focus_not_stolen_on_new_flow', (WidgetTester tester) async {
    final FakeDaemonClient client = fakeDaemon(threeThenUrgent());
    await pumpIntercept(tester, client: client);
    await playScript(tester, const Duration(milliseconds: 100));

    // Der Editor der ausgewählten Anfrage, und der Cursor im Rumpf.
    await tester.sendKeyEvent(LogicalKeyboardKey.keyE);
    for (int i = 0; i < 6; i++) {
      await tester.pump(const Duration(milliseconds: 16));
    }
    final Finder body = find.descendant(
      of: find.byKey(const Key('editor-draft-body')),
      matching: find.byType(EditableText),
    );
    await tester.tap(body);
    await tester.pump();
    final FocusNode typing = tester.widget<EditableText>(body).focusNode;
    expect(typing.hasPrimaryFocus, isTrue, reason: 'the body field has focus');

    // Die vierte Anfrage trifft ein, während getippt wird.
    await tester.pump(const Duration(seconds: 2));
    await tester.pump();
    await tester.pump();
    expect(
      find.byWidgetPredicate(
        (Widget w) => w is QueueRow && w.flow.id == testFlowId(4),
      ),
      findsOne,
      reason: 'the new request did arrive in the queue',
    );

    expect(find.byKey(const Key('editor-send')), findsOne);
    expect(
      FocusManager.instance.primaryFocus,
      same(typing),
      reason: 'the focus stays in the editor',
    );

    // Und die nächste Taste landet weiter im Feld, nicht in der Queue.
    tester.testTextInput.enterText('still typing');
    await tester.pump();
    expect(tester.widget<EditableText>(body).controller.text, 'still typing');
  }, variant: linux);
}
