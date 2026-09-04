// Die fünf Sicherungen aus dem externen Review von HUM-028, als Tests.
//
// Jede von ihnen schützt dieselbe Zusage: eine unumkehrbare Handlung passiert
// nur an dem, was der Mensch gelesen hat, und nur, wenn er sie wollte
// (docs/UX.md 5.4, 4.7, CONVENTIONS 4.13).

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/widgets/batch_modal.dart';
import 'package:humanitl/features/intercept/widgets/note_field.dart';

import 'fixtures.dart';
import 'harness.dart';

/// Eine angehaltene Anfrage mit Apex, wie der Katalog des Daemons sie liefert.
FlowDetail known(
  int n, {
  String host = 'api.github.com',
  String apex = 'github.com',
}) => detailFor(
  heldFlow(
    n: n,
    deadline: testStart.add(Duration(minutes: 5, seconds: n)),
    method: Method.post,
    host: host,
    path: '/graphql',
    requestSize: 428,
  ),
  apex: apex,
);

/// Eine angehaltene Anfrage, in der ein Schlüssel gefunden wurde.
FlowDetail withFinding(int n) => detailFor(
  heldFlow(
    n: n,
    deadline: testStart.add(const Duration(minutes: 5)),
    method: Method.post,
    host: 'api.example.com',
    path: '/v1/upload',
    requestSize: 512,
  ).copyWith(findingCount: 1),
  apex: 'example.com',
  findings: <Finding>[testFinding()],
);

/// Ein Fake, der nach [failAfter] Entscheidungen scheitert.
class FailingClient extends FakeDaemonClient {
  /// Scheitert ab der [failAfter]-ten Entscheidung.
  FailingClient({required this.failAfter, required List<ScriptedEvent> script})
    : super(script: script, clock: () => testStart);

  /// Wie viele Entscheidungen durchgehen, bevor der Daemon abweist.
  final int failAfter;

  @override
  Future<Rule?> decide(FlowId id, Decision decision, {Rule? remember}) async {
    if (decisions.length >= failAfter) {
      throw DaemonException(
        const Diagnostic(
          code: DiagnosticCodes.flowNotHeld,
          severity: Severity.error,
          why: 'the flow is no longer held',
        ),
      );
    }
    return super.decide(id, decision, remember: remember);
  }
}

/// Drückt [key] mit gehaltener Strg-Taste, ohne sie loszulassen.
Future<void> holdChord(WidgetTester tester, LogicalKeyboardKey key) async {
  await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
  await tester.sendKeyDownEvent(key);
  await tester.pump();
}

void main() {
  testWidgets('a single key that may not act gives the key back', (
    WidgetTester tester,
  ) async {
    // Befund 1: `Shortcuts` meldet jede aufgerufene Action als `handled`, und
    // eine behandelte Taste erreicht das Text-Input-System nie. Solange das
    // Notizfeld die Tastatur hat, muss jede Ein-Tasten-Bindung des Screens
    // `ignored` liefern, sonst fehlen `a`, `b` und `n` in jeder Notiz.
    final FakeDaemonClient client = fakeDaemon(
      holdScript(<FlowDetail>[known(1)]),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    await tester.sendKeyEvent(LogicalKeyboardKey.keyN);
    await tester.pump();
    await tester.pump();
    expect(find.byType(NoteField), findsOneWidget);

    for (final LogicalKeyboardKey key in <LogicalKeyboardKey>[
      LogicalKeyboardKey.keyA,
      LogicalKeyboardKey.keyB,
      LogicalKeyboardKey.keyN,
      LogicalKeyboardKey.keyJ,
      LogicalKeyboardKey.keyK,
      LogicalKeyboardKey.digit1,
      LogicalKeyboardKey.digit4,
    ]) {
      expect(
        await tester.sendKeyEvent(key),
        isFalse,
        reason: '\$key belongs to the field while it has the keyboard',
      );
      await tester.pump();
    }

    // Und nichts davon hat nebenbei entschieden oder das Feld geschlossen.
    expect(client.decisions, isEmpty);
    expect(find.byType(NoteField), findsOneWidget);
  });

  testWidgets('the release of a key is read while the modal stands', (
    WidgetTester tester,
  ) async {
    // Befund 5: das Loslassen darf nicht am offenen Modal hängen bleiben,
    // sonst weist der nächste Druck mit `keyHeld` ab, obwohl keine Taste
    // unten ist.
    final FakeDaemonClient client = fakeDaemon(
      holdScript(<FlowDetail>[
        for (int i = 1; i <= 6; i++)
          known(i, host: 'registry.npmjs.org', apex: 'npmjs.org'),
      ]),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    await pressControl(tester, LogicalKeyboardKey.keyA);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.keyB);
    await tester.pumpAndSettle();
    expect(find.byType(BatchModal), findsOneWidget);

    // Taste kommt hoch, während das Modal steht.
    await tester.sendKeyUpEvent(LogicalKeyboardKey.keyB);
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();

    await tester.sendKeyDownEvent(LogicalKeyboardKey.keyB);
    await tester.pumpAndSettle();

    expect(find.text('Release the key, then decide again'), findsNothing);
    expect(find.byType(BatchModal), findsOneWidget);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.keyB);
    await tester.pump();
  });

  group('every key that decides is locked', () {
    testWidgets('Ctrl+F held down allows exactly one request', (
      WidgetTester tester,
    ) async {
      final FakeDaemonClient client = fakeDaemon(
        holdScript(<FlowDetail>[
          known(1),
          known(2, host: 'pypi.org', apex: 'pypi.org'),
        ]),
      );
      await pumpIntercept(tester, client: client);
      await playScript(tester);

      await holdChord(tester, LogicalKeyboardKey.keyF);
      for (int i = 0; i < 5; i++) {
        await tester.pump(const Duration(milliseconds: 100));
        await tester.sendKeyRepeatEvent(LogicalKeyboardKey.keyF);
      }
      await tester.pump();
      // Auch ein zweiter Druck, ohne dass die Taste hochkam, entscheidet
      // nichts mehr; die Auswahl ist inzwischen weitergewandert.
      await tester.sendKeyDownEvent(LogicalKeyboardKey.keyF);
      await tester.pump();

      // Ein Finger, der nicht hochkam, entscheidet einmal (docs/UX.md 5.4).
      expect(client.decisions, hasLength(1));
      expect(find.text('Release the key, then decide again'), findsOneWidget);

      await tester.sendKeyUpEvent(LogicalKeyboardKey.keyF);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
      await tester.pump();
    });

    testWidgets('the selection waits while Ctrl+L is down', (
      WidgetTester tester,
    ) async {
      final FakeDaemonClient client = fakeDaemon(
        holdScript(<FlowDetail>[
          known(1),
          known(2, host: 'pypi.org', apex: 'pypi.org'),
        ]),
      );
      await pumpIntercept(tester, client: client);
      await playScript(tester);

      await holdChord(tester, LogicalKeyboardKey.keyL);
      await tester.pump();
      expect(client.decisions, hasLength(1));
      expect(containerOf(tester).read(selectedFlowIdProvider), testFlowId(1));

      await tester.sendKeyUpEvent(LogicalKeyboardKey.keyL);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
      await tester.pump();
      expect(containerOf(tester).read(selectedFlowIdProvider), testFlowId(2));
    });
  });

  testWidgets('a hold that loses its request decides nothing', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fakeDaemon(
      holdScript(<FlowDetail>[
        known(1),
        known(2, host: 'pypi.org', apex: 'pypi.org'),
      ]),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    final TestGesture gesture = await tester.startGesture(
      tester.getCenter(find.byKey(const Key('intercept-valve-hold'))),
    );
    await tester.pump(const Duration(milliseconds: 100));
    // Die Auswahl wandert unter dem Finger weiter.
    await tester.sendKeyEvent(LogicalKeyboardKey.keyJ);
    await tester.pump();
    await tester.pump(HMotion.holdToConfirm + const Duration(milliseconds: 50));
    await gesture.up();
    await tester.pump();
    await tester.pump();

    // Gehalten wurde für die eine Anfrage, losgelassen über einer anderen:
    // entschieden wird keine (docs/UX.md 5.4).
    expect(client.decisions, isEmpty);
  });

  group('the registrable domain comes from the daemon', () {
    testWidgets('without an apex the scope refuses and says why', (
      WidgetTester tester,
    ) async {
      // Kein `DomainInfo`: der Katalog hat noch nichts gesagt.
      final FakeDaemonClient client = fakeDaemon(
        holdScript(<FlowDetail>[
          detailFor(
            heldFlow(n: 1, deadline: testStart.add(const Duration(minutes: 5))),
          ),
        ]),
      );
      await pumpIntercept(tester, client: client);
      await playScript(tester);

      await tester.sendKeyEvent(LogicalKeyboardKey.digit2);
      await tester.pump();
      await pressShiftKey(tester, LogicalKeyboardKey.digit3);

      expect(
        find.text(
          'The registrable domain is not known yet · the rule can cover the host',
        ),
        findsOneWidget,
      );
      // Und die Regel bleibt die des Hosts, nicht die einer geratenen Domain.
      expect(
        find.text('allow · ∗ · api.github.com · this session'),
        findsOneWidget,
      );
    });

    testWidgets('with an apex the scope writes the rule of the daemon', (
      WidgetTester tester,
    ) async {
      final FakeDaemonClient client = fakeDaemon(
        holdScript(<FlowDetail>[known(1)]),
      );
      await pumpIntercept(tester, client: client);
      await playScript(tester);

      await tester.sendKeyEvent(LogicalKeyboardKey.digit2);
      await tester.pump();
      await pressShiftKey(tester, LogicalKeyboardKey.digit3);

      expect(
        find.text('allow · ∗ · **.github.com · this session'),
        findsOneWidget,
      );
    });
  });

  testWidgets('a forever rule asks first', (WidgetTester tester) async {
    final FakeDaemonClient client = fakeDaemon(
      holdScript(<FlowDetail>[known(1)]),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    // `4` ist Für immer: die Regel überlebt die Sitzung (docs/UX.md 5.4).
    await tester.sendKeyEvent(LogicalKeyboardKey.digit4);
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();

    expect(find.byType(BatchModal), findsOneWidget);
    expect(
      find.text('Save a permanent rule for api.github.com?'),
      findsOneWidget,
    );
    expect(find.byKey(const Key('intercept-batch-rule')), findsOneWidget);
    expect(client.decisions, isEmpty);

    await tester.tap(find.byKey(const Key('intercept-batch-confirm')));
    await tester.pumpAndSettle();

    expect(client.decisions, hasLength(1));
    expect(client.decisions.single.remember?.expires, const RuleExpiry.never());
  });

  testWidgets('the sentence names the host the rule will name', (
    WidgetTester tester,
  ) async {
    // Befund 2: die Regel entsteht aus der ersten Zeile der Reichweite, der
    // Cursor kann eine andere sein. Der Satz bewacht eine unumkehrbare
    // Handlung, also muss er dieselbe nennen (CONVENTIONS 4.13).
    final FakeDaemonClient client = fakeDaemon(
      holdScript(<FlowDetail>[known(1), known(2, host: 'codeload.github.com')]),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    await pressControl(tester, LogicalKeyboardKey.keyA);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyJ);
    await tester.sendKeyEvent(LogicalKeyboardKey.digit2);
    await tester.pump();

    expect(
      find.text('allow · ∗ · api.github.com · this session'),
      findsOneWidget,
    );
    expect(find.textContaining('codeload.github.com · this'), findsNothing);
  });

  testWidgets('a batch that breaks says what already left', (
    WidgetTester tester,
  ) async {
    // Befund 3: drei Anfragen sind raus, die vierte scheitert. Was raus ist,
    // bleibt gesagt, der Fehler hängt an der ersten, die nicht ging, und der
    // Cursorwechsel löscht ihn nicht (docs/UX.md 4.4).
    final FailingClient client = FailingClient(
      failAfter: 3,
      script: holdScript(<FlowDetail>[
        for (int i = 1; i <= 5; i++)
          known(i, host: 'registry.npmjs.org', apex: 'npmjs.org'),
      ]),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    await pressControl(tester, LogicalKeyboardKey.keyA);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();

    expect(client.decisions, hasLength(3));
    expect(
      find.byKey(const Key('intercept-decision-error')),
      findsOneWidget,
      reason: 'the request that did not go says why',
    );
    expect(find.text('3 sent to registry.npmjs.org'), findsNothing);
  });

  group('a request with a finding', () {
    testWidgets('names the finding, the place and the consequence', (
      WidgetTester tester,
    ) async {
      await pumpIntercept(
        tester,
        client: fakeDaemon(holdScript(<FlowDetail>[withFinding(1)])),
      );
      await playScript(tester);

      expect(
        find.text('Held: a github API key was found in the body'),
        findsOneWidget,
      );
      expect(
        find.text('Sending sends a github API key to api.example.com.'),
        findsOneWidget,
      );
      expect(find.text('Send with 1 finding'), findsOneWidget);
    });

    testWidgets('a click on the valve does not send it', (
      WidgetTester tester,
    ) async {
      final FakeDaemonClient client = fakeDaemon(
        holdScript(<FlowDetail>[withFinding(1)]),
      );
      await pumpIntercept(tester, client: client);
      await playScript(tester);

      await tester.tap(find.byKey(const Key('intercept-valve-hold')));
      await tester.pump();
      await tester.pump();

      expect(client.decisions, isEmpty);
      expect(
        find.text('Hold to send: a finding is unresolved'),
        findsOneWidget,
      );
    });

    testWidgets('holding the valve sends it', (WidgetTester tester) async {
      final FakeDaemonClient client = fakeDaemon(
        holdScript(<FlowDetail>[withFinding(1)]),
      );
      await pumpIntercept(tester, client: client);
      await playScript(tester);

      final TestGesture gesture = await tester.startGesture(
        tester.getCenter(find.byKey(const Key('intercept-valve-hold'))),
      );
      await tester.pump();
      await tester.pump(
        HMotion.holdToConfirm + const Duration(milliseconds: 50),
      );
      await gesture.up();
      await tester.pump();
      await tester.pump();

      expect(client.decisions, hasLength(1));
      expect(client.decisions.single.decision, const Decision.allow());
    });
  });
}
