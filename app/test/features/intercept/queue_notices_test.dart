// Die Karten über der Warteschlange schweben (HUM-302).
//
// Gemeldet am 2026-10-29: Ein Befund des Daemons über der Queue schob die
// Liste nach unten, sein Schließen stand neben der Karte statt auf ihr, und
// rechts blieb eine leere Spalte. Diese Tests halten fest: Die erste Zeile der
// Liste steht mit und ohne Karte an derselben Stelle, die Karte steht
// rechtsbündig oben in der Spalte der Queue und nicht über den Nachbarn, das
// Schließen liegt in der Karte, und die Ankunft nimmt niemandem die Tastatur.

import 'package:flutter/rendering.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/h_diagnostic_card.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/providers/diagnostics.dart';
import 'package:humanitl/features/intercept/widgets/agent_ask_card.dart';
import 'package:humanitl/features/intercept/widgets/queue_pane.dart';
import 'package:humanitl/features/intercept/widgets/queue_row.dart';

import 'fixtures.dart';
import 'harness.dart';

/// Zwei gehaltene Anfragen, dann nach 3 s ein Befund des Daemons: Bis
/// dahin ist das Einblenden der Zeilen vorbei, und sie stehen still.
List<ScriptedEvent> script() => <ScriptedEvent>[
  ...holdScript(<FlowDetail>[held(1), held(2, path: '/vue')]),
  ScriptedEvent(
    const Duration(seconds: 3),
    (FakeSessionState state, DateTime now) => FlowEvent.diagnostic(
      at: now,
      diagnostic: const Diagnostic(
        code: 'TLS_001',
        severity: Severity.warning,
        why: 'curl in the sandbox does not trust the Humanitl CA yet',
      ),
    ),
  ),
];

/// Prüft, dass ein Zeiger auf der Mitte und dem rechten Ende von [row] die
/// Zeile trifft und keine Karte darüber.
void expectRowReachable(WidgetTester tester, Finder row) {
  final Rect rect = tester.getRect(row);
  final RenderObject target = tester.renderObject(row);
  for (final Offset point in <Offset>[
    rect.center,
    rect.centerRight - const Offset(12, 0),
  ]) {
    final HitTestResult result = tester.hitTestOnBinding(point);
    final List<RenderObject> path = <RenderObject>[
      for (final HitTestEntry entry in result.path)
        if (entry.target is RenderObject) entry.target as RenderObject,
    ];
    bool inRow(RenderObject object) {
      RenderObject? node = object;
      while (node != null) {
        if (identical(node, target)) {
          return true;
        }
        node = node.parent;
      }
      return false;
    }

    expect(
      path.isNotEmpty && inRow(path.first),
      isTrue,
      reason: 'the pointer at $point reaches the row, not a card above it',
    );
  }
}

void main() {
  testWidgets('a_finding_floats_over_the_queue_and_leaves_its_rows_in_place', (
    WidgetTester tester,
  ) async {
    await pumpIntercept(tester, client: fakeDaemon(script()));
    // Ankunft und Einblenden der Zeilen sind nach einer Sekunde in Frames
    // vorbei; ein einzelnes `pump` mit langer Dauer wäre nur ein Frame.
    for (int frame = 0; frame < 10; frame++) {
      await tester.pump(const Duration(milliseconds: 100));
    }
    final Finder rows = find.byType(QueueRow);
    expect(rows, findsNWidgets(2));
    expect(find.byType(HDiagnosticCard), findsNothing);
    final Rect first = tester.getRect(rows.first);
    // Die Tastatur steht auf der Queue: ein Klick auf die erste Zeile.
    await tester.tap(rows.first);
    await tester.pump();
    final FocusNode? focus = FocusManager.instance.primaryFocus;
    expect(focus, isNotNull);

    // Der Befund kommt.
    for (int frame = 0; frame < 25; frame++) {
      await tester.pump(const Duration(milliseconds: 100));
    }
    final Finder card = find.byType(HDiagnosticCard);
    expect(card, findsOneWidget);

    // Die erste Zeile steht, wo sie stand: Die Karte schiebt nichts.
    expect(tester.getRect(rows.first), first);
    // Und die Tastatur bleibt, wo sie war.
    expect(FocusManager.instance.primaryFocus, same(focus));

    // Rechtsbündig unten in der Spalte der Queue, über der Liste, nicht über
    // dem Inspector daneben.
    final Rect queue = tester.getRect(find.byType(QueuePane));
    final Rect header = tester.getRect(
      find.byKey(const Key('intercept-queue-header')),
    );
    final Rect drawn = tester.getRect(card);
    expect(drawn.right, queue.right - HSpace.x2);
    expect(drawn.left, greaterThanOrEqualTo(queue.left + HSpace.x2));
    expect(drawn.bottom, queue.bottom - HSpace.x2);
    expect(drawn.top, greaterThan(header.bottom));
    // Die oberste Zeile mit ihrem Blockieren bleibt frei und greifbar
    // (Review): Ein Zeiger auf ihr trifft die Zeile und nicht die Karte.
    expectRowReachable(tester, rows.first);

    // Das Schließen liegt in der Karte, oben rechts, mit 28 px Trefferfläche.
    final Finder close = find.descendant(
      of: card,
      matching: find.byWidgetPredicate(
        (Widget widget) =>
            widget is HIconButton &&
            widget.key is ValueKey<String> &&
            (widget.key! as ValueKey<String>).value.startsWith(
              'intercept-diagnostic-dismiss-',
            ),
      ),
    );
    expect(close, findsOneWidget);
    final Rect button = tester.getRect(close);
    expect(button.right, drawn.right - HSpace.x1);
    expect(button.top, drawn.top + HSpace.x1);
    expect(button.width, greaterThanOrEqualTo(HSize.hitMin));

    // Schließen nimmt die Karte, und die Zeilen bleiben stehen.
    await tester.tap(close);
    await tester.pump();
    expect(card, findsNothing);
    expect(
      containerOf(tester).read(diagnosticsProvider),
      isEmpty,
      reason: 'the card went with its entry',
    );
    expect(tester.getRect(rows.first), first);
  });

  testWidgets('a_request_of_the_agent_floats_the_same_way', (
    WidgetTester tester,
  ) async {
    await pumpIntercept(
      tester,
      client: fakeDaemon(<ScriptedEvent>[
        ...holdScript(<FlowDetail>[held(1), held(2, path: '/vue')]),
        ScriptedEvent(
          const Duration(seconds: 3),
          (FakeSessionState state, DateTime now) => FlowEvent.agentAsk(
            at: now,
            askId: 'ask-1',
            text: 'please allow pypi.org so I can install requests',
            suggestedHost: 'pypi.org',
            suggestedPath: '/simple/requests/',
          ),
        ),
      ]),
    );
    for (int frame = 0; frame < 10; frame++) {
      await tester.pump(const Duration(milliseconds: 100));
    }
    final Finder rows = find.byType(QueueRow);
    expect(rows, findsNWidgets(2));
    final Rect first = tester.getRect(rows.first);
    await tester.tap(rows.first);
    await tester.pump();
    final FocusNode? focus = FocusManager.instance.primaryFocus;
    expect(focus, isNotNull);

    for (int frame = 0; frame < 25; frame++) {
      await tester.pump(const Duration(milliseconds: 100));
    }
    final Finder card = find.byType(AgentAskCard);
    expect(card, findsOneWidget);
    expect(tester.getRect(rows.first), first);
    expect(FocusManager.instance.primaryFocus, same(focus));

    final Rect queue = tester.getRect(find.byType(QueuePane));
    final Rect header = tester.getRect(
      find.byKey(const Key('intercept-queue-header')),
    );
    final Rect drawn = tester.getRect(card);
    expect(drawn.right, queue.right - HSpace.x2);
    expect(drawn.bottom, queue.bottom - HSpace.x2);
    expect(drawn.top, greaterThan(header.bottom));
    expectRowReachable(tester, rows.first);

    // Das Schließen steht in der Karte, oben rechts.
    final Finder close = find.descendant(
      of: card,
      matching: find.byType(HIconButton),
    );
    expect(close, findsOneWidget);
    final Rect button = tester.getRect(close);
    expect(button.right, drawn.right - HSpace.x1);
    expect(button.top, drawn.top + HSpace.x1);

    await tester.tap(close);
    await tester.pump();
    expect(card, findsNothing);
    expect(tester.getRect(rows.first), first);
  });
}
