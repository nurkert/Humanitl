// Die Entscheidung über eine Gruppe (HUM-029).
//
// Sorgfalt wächst mit Reichweite: eine bis fünf Anfragen sind durch Zeit
// geschützt — die Armierung des Ventils, das Halten beim Blockieren —, mehr
// als fünf bekommen das eine Modal dieses Screens (docs/UX.md 5.4). Erlauben
// einer ganzen Gruppe geht immer über die Aktionsleiste, nie aus der Zeile
// (3.5).

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/providers/queue_freeze.dart';
import 'package:humanitl/features/intercept/providers/selection.dart';
import 'package:humanitl/features/intercept/widgets/batch_modal.dart';
import 'package:humanitl/features/intercept/widgets/group_header_row.dart';
import 'package:humanitl/features/intercept/widgets/queue_row.dart';
import 'package:humanitl/features/intercept/widgets/selection_card.dart';

import 'fixtures.dart';
import 'harness.dart';

/// Drückt [key] mit Strg und Umschalt zusammen.
Future<void> pressChord(WidgetTester tester, LogicalKeyboardKey key) async {
  await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
  await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
  await tester.sendKeyEvent(key);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
  await tester.pump();
}

/// Hält [finder] für [duration] gedrückt.
///
/// Der Gruppenkopf verschwindet, sobald seine Anfragen entschieden sind, also
/// erreicht das Loslassen ihn nicht mehr; die Meldung darüber wird hier
/// abgeholt, weil sie das erwartete Verhalten beschreibt und nicht einen
/// Fehler.
Future<void> hold(WidgetTester tester, Finder finder, Duration duration) async {
  final TestGesture gesture = await tester.startGesture(
    tester.getCenter(finder),
  );
  await tester.pump();
  await tester.pump(duration);
  await gesture.up();
  await tester.pump();
  await tester.pump();
  tester.takeException();
}

/// [count] angehaltene Anfragen an dieselbe registrierbare Domain.
List<ScriptedEvent> burst(int count) => holdScriptOf(<FlowDetail>[
  for (int i = 1; i <= count; i++)
    held(i, host: 'registry.npmjs.org', path: '/react/-/react-19.$i.tgz'),
]);

/// Dasselbe wie `holdScript`, aber mit den Details dieses Tests.
List<ScriptedEvent> holdScriptOf(List<FlowDetail> details) {
  final List<ScriptedEvent> script = <ScriptedEvent>[];
  for (int i = 0; i < details.length; i++) {
    script.addAll(arriveAt(details[i], Duration(milliseconds: 10 * i)));
  }
  return script;
}

void main() {
  testWidgets('a burst reads as one line, and the line only blocks', (
    WidgetTester tester,
  ) async {
    await pumpIntercept(tester, client: fakeDaemon(burst(12)));
    await playScript(tester);

    // Zwölf Anfragen an eine Domain: ein Kopf, keine zwölf Zeilen.
    expect(find.byType(GroupHeaderRow), findsOneWidget);
    expect(find.byType(QueueRow), findsNothing);
    expect(find.text('12× GET'), findsOneWidget);

    // Der Aktionsslot ist bei Ruhe leer und deckt bei Hover genau eine Aktion
    // auf: aus der Zeile heraus wird nur blockiert (docs/UX.md 3.4, 3.5).
    expect(find.byKey(const Key('queue-group-block-npmjs.org')), findsNothing);
    final Rect before = tester.getRect(find.byType(GroupHeaderRow));
    await hoverOver(tester, find.byType(GroupHeaderRow));
    await tester.pump();

    expect(
      find.byKey(const Key('queue-group-block-npmjs.org')),
      findsOneWidget,
    );
    expect(
      tester.getRect(find.byType(GroupHeaderRow)),
      before,
      reason: 'the slot moves nothing when it fills',
    );
    expect(find.text('Send 12'), findsNothing);

    // Die Pfeiltaste klappt auf, ohne einen zweiten Fokusstopp zu bauen.
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
    await tester.pumpAndSettle();
    expect(find.byType(QueueRow), findsWidgets);
  });

  testWidgets('ctrl+shift+F selects the group first, then sends it', (
    WidgetTester tester,
  ) async {
    // Befund 2: Senden ist unumkehrbar, also muss die Karte zeigen, worüber
    // entschieden wird, bevor entschieden wird (docs/UX.md 3.5, 5.4).
    final FakeDaemonClient client = fakeDaemon(burst(4));
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    await pressChord(tester, LogicalKeyboardKey.keyF);
    await tester.pump();

    expect(client.decisions, isEmpty);
    expect(containerOf(tester).read(selectionProvider), hasLength(4));
    expect(find.byType(SelectionCard), findsOneWidget);

    await pressChord(tester, LogicalKeyboardKey.keyF);
    await tester.pump();
    await tester.pump();

    expect(client.decisions, hasLength(4));
  });

  testWidgets('a request that waits outside the list is never sent along', (
    WidgetTester tester,
  ) async {
    // Befund 2, zweite Hälfte: eine Ankunft, die das Einfrieren zurückhält,
    // steht nicht auf dem Schirm und gehört in keine Reichweite (docs/UX.md
    // 2.8).
    final FakeDaemonClient client = fakeDaemon(<ScriptedEvent>[
      ...arriveAt(
        held(1, host: 'registry.npmjs.org', path: '/a'),
        Duration.zero,
      ),
      ...arriveAt(
        held(2, host: 'registry.npmjs.org', path: '/b'),
        const Duration(milliseconds: 10),
      ),
      ...arriveAt(
        held(3, host: 'registry.npmjs.org', path: '/c'),
        const Duration(seconds: 1),
      ),
    ]);
    await pumpIntercept(tester, client: client);
    await playScript(tester, const Duration(milliseconds: 100));

    // `J` friert die Reihenfolge für zwei Sekunden ein; die dritte Anfrage
    // kommt in dieser Zeit an und wartet.
    await tester.sendKeyEvent(LogicalKeyboardKey.keyJ);
    await tester.pump(const Duration(milliseconds: 1200));
    await tester.pump();
    expect(containerOf(tester).read(pendingArrivalsProvider), <FlowId>{
      testFlowId(3),
    });

    await pressControl(tester, LogicalKeyboardKey.keyA);
    await tester.pump();

    expect(containerOf(tester).read(selectionProvider), hasLength(2));
    expect(
      containerOf(tester).read(selectionProvider).contains(testFlowId(3)),
      isFalse,
      reason: 'a request nobody has seen is in no reach',
    );
  });

  testWidgets('a group of several hosts is named by hosts, not by a guess', (
    WidgetTester tester,
  ) async {
    // Befund 4: `psl.dart` rät; außerhalb seiner Tabelle nennt es ein Public
    // Suffix als Domain. Ein Satz, der eine unumkehrbare Sendung bewacht,
    // nennt Hosts (CONVENTIONS 4.13).
    final FakeDaemonClient client = fakeDaemon(<ScriptedEvent>[
      for (int i = 1; i <= 6; i++)
        ...arriveAt(
          held(i, host: i.isEven ? 'a.foo.com.pl' : 'b.foo.com.pl', path: '/x'),
          Duration(milliseconds: 10 * i),
        ),
    ]);
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    // `psl.dart` würde `com.pl` raten — ein Public Suffix, keine Domain.
    expect(find.text('com.pl'), findsNothing);
    expect(find.text('b.foo.com.pl and 1 more host'), findsOneWidget);

    await pressControl(tester, LogicalKeyboardKey.keyA);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyB);
    await tester.pumpAndSettle();

    expect(find.text('Block 6 requests to 2 hosts?'), findsOneWidget);
  });

  testWidgets('the palette offers allow all, and it opens the modal', (
    WidgetTester tester,
  ) async {
    // Befund 3: das einzige „allow all" des Programms, und es sendet nie
    // still (HUM-029).
    final FakeDaemonClient client = fakeDaemon(burst(4));
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    await pressControl(tester, LogicalKeyboardKey.keyK);
    await tester.pumpAndSettle();
    expect(find.text('Queue: allow all…'), findsOneWidget);

    await tester.tap(find.text('Queue: allow all…'));
    await tester.pumpAndSettle();

    expect(find.byType(BatchModal), findsOneWidget);
    expect(client.decisions, isEmpty);
  });

  testWidgets('a block out of the group header carries no note', (
    WidgetTester tester,
  ) async {
    // Befund 7: die Notiz gehört der ausgewählten Anfrage, nicht der Gruppe
    // unter dem Zeiger (HUM-072).
    final FakeDaemonClient client = fakeDaemon(burst(3));
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    await tester.sendKeyEvent(LogicalKeyboardKey.keyN);
    await tester.pump();
    await tester.pump();
    await tester.enterText(
      find.byKey(const Key('intercept-note-input')),
      'use PyPI',
    );
    await tester.pump();

    await hoverOver(tester, find.byType(GroupHeaderRow));
    await tester.pump();
    await hold(
      tester,
      find.byKey(const Key('queue-group-block-npmjs.org')),
      HMotion.holdToBlock + const Duration(milliseconds: 50),
    );

    expect(client.decisions, hasLength(3));
    for (final RecordedDecision decided in client.decisions) {
      expect(decided.decision, const Decision.block());
    }
  });

  testWidgets('a note travels with a whole selection, and the label says so', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fakeDaemon(burst(3));
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    await pressControl(tester, LogicalKeyboardKey.keyA);
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.keyN);
    await tester.pump();
    await tester.pump();
    await tester.enterText(
      find.byKey(const Key('intercept-note-input')),
      'use PyPI',
    );
    await tester.pump();

    expect(find.text('Block 3 selected with note'), findsOneWidget);

    await pressChord(tester, LogicalKeyboardKey.keyL);
    await tester.pump();
    await tester.pump();

    expect(client.decisions, hasLength(3));
    for (final RecordedDecision decided in client.decisions) {
      expect(decided.decision, const Decision.block(note: 'use PyPI'));
    }
  });

  testWidgets('a decided row leaves the count of its group', (
    WidgetTester tester,
  ) async {
    // Befund 4: eine entschiedene Zeile ruht drei Sekunden an ihrem Platz,
    // aber sie ist nicht mehr gehalten. Zählte der Kopf sie mit, wiese
    // `Block {n}` die ganze Gruppe ab, weil eine davon nicht mehr entscheidbar
    // ist (docs/UX.md 2.8).
    final FakeDaemonClient client = fakeDaemon(burst(3));
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    expect(find.text('3× GET'), findsOneWidget);

    await tester.sendKeyEvent(LogicalKeyboardKey.keyB);
    await tester.pump();
    await tester.pump();

    expect(client.decisions, hasLength(1));
    expect(find.text('2× GET'), findsOneWidget);

    await hoverOver(tester, find.byType(GroupHeaderRow));
    await tester.pump();
    await hold(
      tester,
      find.byKey(const Key('queue-group-block-npmjs.org')),
      HMotion.holdToBlock + const Duration(milliseconds: 50),
    );

    expect(client.decisions, hasLength(3), reason: 'the two that are left');
    expect(find.text('Pick a request first'), findsNothing);
  });

  testWidgets('a group of several hosts always asks first', (
    WidgetTester tester,
  ) async {
    // Befund 7: `psl.dart` rät die Domain, also können zwei fremde
    // Registranten in einer Gruppe landen. Unter sechs Anfragen schützt sonst
    // nur das Halten; über mehrere Hosts fragt das Modal immer.
    final FakeDaemonClient client = fakeDaemon(<ScriptedEvent>[
      ...arriveAt(held(1, host: 'a.foo.com.pl', path: '/x'), Duration.zero),
      ...arriveAt(
        held(2, host: 'b.evil.com.pl', path: '/y'),
        const Duration(milliseconds: 10),
      ),
    ]);
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    await pressControl(tester, LogicalKeyboardKey.keyA);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyB);
    await tester.pumpAndSettle();

    expect(find.byType(BatchModal), findsOneWidget);
    expect(find.text('Block 2 requests to 2 hosts?'), findsOneWidget);
    expect(client.decisions, isEmpty);
  });

  testWidgets('the greyed out scope says why instead of swallowing the click', (
    WidgetTester tester,
  ) async {
    // Befund 6: ohne Apex ist der Domain-Scope ausgegraut, aber er bleibt
    // anfassbar und nennt den Grund (docs/UX.md 5.3).
    await pumpIntercept(
      tester,
      client: fakeDaemon(
        holdScript(<FlowDetail>[
          detailFor(
            heldFlow(n: 1, deadline: testStart.add(const Duration(minutes: 5))),
          ),
        ]),
      ),
    );
    await playScript(tester);

    await tester.sendKeyEvent(LogicalKeyboardKey.digit2);
    await tester.pump();
    await tester.tap(
      find.descendant(
        of: find.byKey(const Key('intercept-remember')),
        matching: find.text('Domain'),
      ),
    );
    await tester.pump();

    expect(
      find.text(
        'The registrable domain is not known yet · the rule can cover the host',
      ),
      findsOneWidget,
    );
  });

  testWidgets('the queue survives twice the text scale', (
    WidgetTester tester,
  ) async {
    // docs/UX.md 6: Zeile und Gruppenkopf bei TextScaler 2.0 ohne Overflow.
    await pumpIntercept(
      tester,
      client: fakeDaemon(burst(4)),
      textScaler: const TextScaler.linear(2),
    );
    await playScript(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
    await tester.pumpAndSettle();

    expect(tester.takeException(), isNull);
    expect(find.byType(GroupHeaderRow), findsOneWidget);
    expect(find.byType(QueueRow), findsWidgets);

    // Und die Karte der Mehrfachauswahl, deren Zeilen keine feste Höhe haben:
    // eine Textzeile mit `itemExtent` schnitte bei großer Skalierung ab.
    await pressControl(tester, LogicalKeyboardKey.keyA);
    await tester.pumpAndSettle();
    expect(find.byType(SelectionCard), findsOneWidget);
    expect(tester.takeException(), isNull);
    final Finder path = find
        .descendant(
          of: find.byType(SelectionCard),
          matching: find.textContaining('registry.npmjs.org'),
        )
        .first;
    expect(
      tester.getSize(path).height,
      greaterThan(HSize.hitMin),
      reason: 'a line of text grows with the type, it is not 28 px tall',
    );
  });

  testWidgets('ctrl_a_selects_group', (WidgetTester tester) async {
    await pumpIntercept(tester, client: fakeDaemon(burst(4)));
    await playScript(tester);

    await pressControl(tester, LogicalKeyboardKey.keyA);
    await tester.pump();

    expect(containerOf(tester).read(selectionProvider), hasLength(4));
    // Die Aktionsleiste beschriftet sich um und bleibt das eine gefüllte
    // Control des Screens (docs/UX.md 3.5).
    expect(find.text('Allow 4 selected'), findsOneWidget);
    expect(find.text('Block 4 selected'), findsOneWidget);
    // Und die Karte zeigt, worüber entschieden wird.
    expect(find.byType(SelectionCard), findsOneWidget);
    expect(find.text('4 requests selected'), findsOneWidget);
  });

  testWidgets('batch_allow_one_rule', (WidgetTester tester) async {
    final FakeDaemonClient client = fakeDaemon(burst(4));
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    // `2` ist die Sitzung: die Regel, die einmal entsteht.
    await tester.sendKeyEvent(LogicalKeyboardKey.digit2);
    await tester.pump();
    await pressControl(tester, LogicalKeyboardKey.keyA);
    await tester.pump();

    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    await tester.pump();

    expect(client.decisions, hasLength(4));
    expect(
      client.decisions.where((RecordedDecision d) => d.remember != null).length,
      1,
      reason: 'the rule is created once, with the first flow',
    );
    expect(client.rules, hasLength(1));
    for (final RecordedDecision decided in client.decisions) {
      expect(decided.decision, const Decision.allow());
    }
  });

  testWidgets('block_gt5_needs_modal', (WidgetTester tester) async {
    final FakeDaemonClient client = fakeDaemon(burst(6));
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    await pressControl(tester, LogicalKeyboardKey.keyA);
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.keyB);
    await tester.pumpAndSettle();

    expect(find.byType(BatchModal), findsOneWidget);
    expect(
      find.text('Block 6 requests to registry.npmjs.org?'),
      findsOneWidget,
    );
    expect(client.decisions, isEmpty);

    // Solange das Modal steht, sind die Entscheidungstasten des Screens
    // still; `Enter` gehört dem fokussierten Abbrechen (docs/UX.md 5.4).
    await tester.sendKeyEvent(LogicalKeyboardKey.keyA);
    await tester.pump();
    expect(find.byType(BatchModal), findsOneWidget);
    expect(client.decisions, isEmpty);

    await tester.tap(find.byKey(const Key('intercept-batch-confirm')));
    await tester.pumpAndSettle();

    expect(find.byType(BatchModal), findsNothing);
    expect(client.decisions, hasLength(6));
    for (final RecordedDecision decided in client.decisions) {
      expect(decided.decision, const Decision.block());
    }
  });

  testWidgets('escape closes the modal and decides nothing', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fakeDaemon(burst(6));
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    await pressControl(tester, LogicalKeyboardKey.keyA);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();

    // Auch Erlauben fragt oberhalb von fünf Anfragen zuerst.
    expect(find.byType(BatchModal), findsOneWidget);
    expect(find.text('Send 6 requests to registry.npmjs.org?'), findsOneWidget);

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();

    expect(find.byType(BatchModal), findsNothing);
    expect(client.decisions, isEmpty);
  });

  testWidgets('the rail carries membership, the fill carries the cursor', (
    WidgetTester tester,
  ) async {
    await pumpIntercept(tester, client: fakeDaemon(burst(4)));
    await playScript(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
    await tester.pumpAndSettle();
    await pressControl(tester, LogicalKeyboardKey.keyA);
    await tester.pumpAndSettle();

    final List<QueueRow> rows = tester
        .widgetList<QueueRow>(find.byType(QueueRow))
        .toList();
    expect(rows.where((QueueRow row) => row.member).length, 4);
    // Genau eine Zeile im ganzen Pane trägt die Füllung (docs/UX.md 3.5).
    expect(rows.where((QueueRow row) => row.selected).length, 1);
  });
}
