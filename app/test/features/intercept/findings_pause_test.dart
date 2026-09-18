// Die Pause beim Senden mit offenen Funden (HUM-049).
//
// Eine gehaltene Anfrage mit einer E-Mail-Adresse im Rumpf: Die Freigabe ist
// amber und heißt „Send with 1 finding", und weder ein Klick noch `Enter`
// schickt sie hinaus. Beide öffnen die Pause in der Karte; erst dort
// entscheidet der Mensch zwischen Senden, Pseudonymisieren und Blockieren.
// Gezählt wird am Fake-Daemon: Was er nicht als Entscheidung gesehen hat, ist
// nicht hinausgegangen.
//
// Alle Tests laufen als Linux-Desktop: `flutter test` spielt sonst Android,
// und Tastatur und Zeiger verhalten sich dort anders als in der App, die es
// nur für Linux gibt.

import 'dart:convert';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/editor/model/draft.dart';
import 'package:humanitl/features/editor/providers/draft_provider.dart';
import 'package:humanitl/features/intercept/providers/decision.dart';
import 'package:humanitl/features/intercept/providers/findings_pause.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/widgets/findings_pause.dart';
import 'package:humanitl/features/intercept/widgets/note_field.dart';

import '../../harness/ui_state.dart';
import 'fixtures.dart';
import 'harness.dart';

/// Nur Linux: die einzige Plattform, für die es die App gibt.
final TargetPlatformVariant linux = TargetPlatformVariant.only(
  TargetPlatform.linux,
);

/// Der Rumpf der Anfrage; die Adresse steht an den Stellen 5 bis 11.
const String mailBody = 'mail a@x.de';

/// Der Fund der E-Mail-Adresse, wie der Daemon ihn meldet.
///
/// `displayPrefix` in der Form, die `daemon/crates/findings/src/display.rs`
/// einer Adresse gibt: erstes Zeichen, drei Sterne, die Domain.
Finding mailFinding({int start = 5, int end = 11, bool resolved = false}) =>
    Finding(
      kind: 'email',
      location: FindingLocation.body,
      spanStart: start,
      spanEnd: end,
      tier: FindingTier.regex,
      displayPrefix: 'a***@x.de',
      resolved: resolved,
    );

/// Eine angehaltene Anfrage mit einer E-Mail-Adresse im Rumpf.
///
/// [host] und [apex] trennen zwei solche Anfragen in zwei Gruppen;
/// [findings] ersetzt den einen Fund.
FlowDetail withMail(
  int n, {
  String host = 'api.example.com',
  String apex = 'example.com',
  List<Finding>? findings,
}) {
  final List<Finding> found = findings ?? <Finding>[mailFinding()];
  return detailFor(
    heldFlow(
      n: n,
      deadline: testStart.add(const Duration(minutes: 5)),
      method: Method.post,
      host: host,
      apex: apex,
      path: '/v1/contact',
      requestSize: mailBody.length,
    ).copyWith(findingCount: found.length),
    apex: apex,
    bodyPreview: mailBody,
    contentType: 'text/plain',
    findings: found,
  );
}

/// Ein Fake, der die Anfrage anhält und ihren Rumpf kennt.
FakeDaemonClient mailClient([List<ScriptedEvent>? script]) {
  final FakeDaemonClient client = fakeDaemon(
    script ?? holdScript(<FlowDetail>[withMail(1)]),
  );
  // Der Rumpf liegt unter seinem Digest, wie ihn `detailFor` vergibt; der
  // Editor holt ihn von dort, genau wie vom Daemon.
  client.state.bodies[List<String>.filled(32, '07').join()] =
      Uint8List.fromList(utf8.encode(mailBody));
  return client;
}

/// Wartet, bis die Freigabe scharf ist.
///
/// Die Freigabe nimmt erst Eingaben an, wenn die URL lange genug lesbar war
/// (`docs/UX.md` 5.4); vorher wird jede Eingabe abgewiesen und nichts
/// passiert, auch keine Pause.
Future<void> armed(WidgetTester tester) async {
  for (
    int i = 0;
    i < 30 && !containerOf(tester).read(allowArmedProvider);
    i++
  ) {
    await tester.pump(const Duration(milliseconds: 50));
  }
  expect(containerOf(tester).read(allowArmedProvider), isTrue);
}

/// Lässt den Aufbau der Pause und den Weg zum Daemon zu Ende laufen.
Future<void> settle(WidgetTester tester) async {
  for (int i = 0; i < 6; i++) {
    await tester.pump(const Duration(milliseconds: 50));
  }
}

final Finder pause = find.byKey(const Key('intercept-findings-pause'));

void main() {
  testWidgets('button_label_with_findings', (WidgetTester tester) async {
    await pumpIntercept(tester, client: mailClient());
    await playScript(tester);

    expect(find.text('Send with 1 finding'), findsOneWidget);
    expect(pause, findsNothing);
  }, variant: linux);

  testWidgets('enter_opens_pause_not_send', (WidgetTester tester) async {
    final FakeDaemonClient client = mailClient();
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await armed(tester);

    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await settle(tester);

    expect(client.decisions, isEmpty, reason: 'nothing left');
    expect(pause, findsOneWidget);
    expect(find.text('1 finding in this request'), findsOneWidget);
    // Art, gekürzter Wert, Ort: was hinausginge, ohne den ganzen Wert. Der
    // Wert steht genau so da, wie der Daemon ihn maskiert hat, ohne ein
    // zweites Auslassungszeichen.
    expect(
      tester
          .widget<Text>(
            find.byKey(const Key('intercept-findings-pause-prefix-0')),
          )
          .data,
      'a***@x.de',
    );
    expect(
      find.descendant(of: pause, matching: find.text('in the body')),
      findsOneWidget,
    );
  }, variant: linux);

  testWidgets('a click on the valve opens the pause and sends nothing', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = mailClient();
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await armed(tester);

    await tester.tap(find.byKey(const Key('intercept-valve-hold')));
    await settle(tester);

    expect(client.decisions, isEmpty);
    expect(pause, findsOneWidget);
  }, variant: linux);

  testWidgets('send_anyway_sends_the_request_unchanged', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = mailClient();
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await armed(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await settle(tester);

    await tester.tap(find.byKey(const Key('intercept-findings-pause-send')));
    await settle(tester);

    expect(client.decisions, hasLength(1));
    expect(client.decisions.single.decision, const Decision.allow());
  }, variant: linux);

  testWidgets('S in the pause sends', (WidgetTester tester) async {
    final FakeDaemonClient client = mailClient();
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await armed(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await settle(tester);

    await tester.sendKeyEvent(LogicalKeyboardKey.keyS);
    await settle(tester);

    expect(client.decisions, hasLength(1));
    expect(client.decisions.single.decision, const Decision.allow());
  }, variant: linux);

  testWidgets('S without an open pause sends nothing', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = mailClient();
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await armed(tester);

    await tester.sendKeyEvent(LogicalKeyboardKey.keyS);
    await settle(tester);

    expect(client.decisions, isEmpty);
    expect(pause, findsNothing);
  }, variant: linux);

  testWidgets('block in the pause blocks', (WidgetTester tester) async {
    final FakeDaemonClient client = mailClient();
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await armed(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await settle(tester);

    await tester.tap(find.byKey(const Key('intercept-findings-pause-block')));
    await settle(tester);

    expect(client.decisions, hasLength(1));
    expect(client.decisions.single.decision, isA<DecisionBlock>());
  }, variant: linux);

  testWidgets('Esc closes the pause and decides nothing', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = mailClient();
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await armed(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await settle(tester);
    expect(pause, findsOneWidget);

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await settle(tester);

    expect(pause, findsNothing);
    expect(client.decisions, isEmpty);
    expect(find.byKey(const Key('intercept-allow')), findsOneWidget);
  }, variant: linux);

  testWidgets('pseudonymize opens the editor with every finding replaced', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = mailClient();
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await armed(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await settle(tester);

    await tester.tap(
      find.byKey(const Key('intercept-findings-pause-pseudonymize')),
    );
    await settle(tester);

    expect(client.decisions, isEmpty, reason: 'the editor decides nothing');
    expect(find.byKey(const Key('editor-send')), findsOneWidget);
    final FlowId id = client.state.flows.keys.first;
    final Draft? draft = containerOf(tester).read(draftProvider(id));
    expect(draft?.body, 'mail <EMAIL_1>');
  }, variant: linux);

  testWidgets('P in the pause pseudonymizes', (WidgetTester tester) async {
    final FakeDaemonClient client = mailClient();
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await armed(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await settle(tester);

    await tester.sendKeyEvent(LogicalKeyboardKey.keyP);
    await settle(tester);

    final FlowId id = client.state.flows.keys.first;
    expect(containerOf(tester).read(draftProvider(id))?.body, 'mail <EMAIL_1>');
  }, variant: linux);

  testWidgets('the valve and the pause speak German', (
    WidgetTester tester,
  ) async {
    // Das Kriterium nennt die deutsche Beschriftung: „Senden mit 1 Finding".
    tester.platformDispatcher.localesTestValue = const <Locale>[Locale('de')];
    addTearDown(tester.platformDispatcher.clearLocalesTestValue);
    final FakeDaemonClient client = mailClient();
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    expect(find.text('Senden mit 1 Finding'), findsOneWidget);
    await armed(tester);

    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await settle(tester);

    expect(client.decisions, isEmpty);
    expect(find.text('1 Fund in dieser Anfrage'), findsOneWidget);
    for (final String label in <String>[
      'Trotzdem senden',
      'Pseudonymisieren',
      'Blockieren',
    ]) {
      expect(
        find.descendant(of: pause, matching: find.text(label)),
        findsOneWidget,
      );
    }
  }, variant: linux);

  testWidgets('E opens the editor without replacing anything', (
    WidgetTester tester,
  ) async {
    // Die Gegenprobe zu „Pseudonymisieren": Wer den Editor selbst öffnet,
    // findet den Entwurf, wie der Agent ihn geschickt hat.
    final FakeDaemonClient client = mailClient();
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    await tester.sendKeyEvent(LogicalKeyboardKey.keyE);
    await settle(tester);

    final FlowId id = client.state.flows.keys.first;
    expect(containerOf(tester).read(draftProvider(id))?.body, mailBody);
  }, variant: linux);

  testWidgets('B in the pause blocks and the pause goes with the flow', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = mailClient();
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await armed(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await settle(tester);

    await tester.sendKeyEvent(LogicalKeyboardKey.keyB);
    await settle(tester);

    expect(client.decisions, hasLength(1));
    expect(client.decisions.single.decision, isA<DecisionBlock>());
    // Entschieden ist entschieden: keine Pause über einer Anfrage, die nicht
    // mehr wartet.
    expect(pause, findsNothing);
  }, variant: linux);

  testWidgets('Back closes the pause and decides nothing', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = mailClient();
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await armed(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await settle(tester);

    await tester.tap(find.byKey(const Key('intercept-findings-pause-back')));
    await settle(tester);

    expect(pause, findsNothing);
    expect(client.decisions, isEmpty);
    expect(find.byKey(const Key('intercept-allow')), findsOneWidget);
  }, variant: linux);

  testWidgets('over two requests the click refuses and opens no pause', (
    WidgetTester tester,
  ) async {
    // Über eine Gruppe gibt es keine Liste der Funde und deshalb keine Pause;
    // der Klick verlangt weiter das Halten (`docs/UX.md` 4.7), und `S` bleibt
    // stumm.
    final FakeDaemonClient client = mailClient(
      holdScript(<FlowDetail>[withMail(1), withMail(2)]),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await pressControl(tester, LogicalKeyboardKey.keyA);
    await armed(tester);

    await tester.tap(find.byKey(const Key('intercept-valve-hold')));
    await settle(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyS);
    await settle(tester);

    expect(pause, findsNothing);
    expect(client.decisions, isEmpty);
    expect(find.text('Hold to send: a finding is unresolved'), findsOneWidget);
  }, variant: linux);

  testWidgets('the pause closes when the selection moves', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = mailClient(
      holdScript(<FlowDetail>[
        withMail(1),
        withMail(2, host: 'mail.other.org', apex: 'other.org'),
      ]),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await armed(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await settle(tester);
    expect(pause, findsOneWidget);

    await tester.sendKeyEvent(LogicalKeyboardKey.keyJ);
    await settle(tester);
    expect(pause, findsNothing);

    // Auch zurück steht sie nicht wieder da: Sie gehörte zu dem Moment, in
    // dem sie aufging.
    await tester.sendKeyEvent(LogicalKeyboardKey.keyK);
    await settle(tester);
    expect(pause, findsNothing);
    expect(client.decisions, isEmpty);
  }, variant: linux);

  testWidgets('the pause closes when the flow times out', (
    WidgetTester tester,
  ) async {
    final FlowDetail detail = withMail(1);
    final FakeDaemonClient client = mailClient(<ScriptedEvent>[
      ...holdScript(<FlowDetail>[detail]),
      ScriptedEvent(
        const Duration(seconds: 2),
        (FakeSessionState state, DateTime now) =>
            FlowEvent.timedOut(at: now, flowId: detail.summary.id),
      ),
    ]);
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await armed(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await settle(tester);
    expect(pause, findsOneWidget);

    await tester.pump(const Duration(seconds: 2));
    await settle(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyS);
    await settle(tester);

    expect(pause, findsNothing);
    expect(client.decisions, isEmpty);
  }, variant: linux);

  testWidgets('once every finding is resolved, S and Esc are free again', (
    WidgetTester tester,
  ) async {
    // Was die Leiste zeichnet und worauf die Tasten hören, ist dasselbe
    // Prädikat: Verschwindet die Pause, weil kein Fund mehr offen ist, dann
    // sendet `S` nichts mehr, und `Esc` wird nicht verschluckt.
    final FakeDaemonClient client = mailClient();
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await armed(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await settle(tester);
    expect(pause, findsOneWidget);

    final FlowId id = client.state.flows.keys.first;
    client.state.details[id] = withMail(
      1,
      findings: <Finding>[mailFinding(resolved: true)],
    );
    containerOf(tester).invalidate(flowDetailProvider(id));
    await settle(tester);

    expect(pause, findsNothing);
    expect(containerOf(tester).read(openFindingsPauseProvider), id);
    expect(await tester.sendKeyEvent(LogicalKeyboardKey.escape), isFalse);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyS);
    await settle(tester);
    expect(client.decisions, isEmpty);
  }, variant: linux);

  testWidgets('S and P belong to the note field while it has the keyboard', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = mailClient();
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await armed(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await settle(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyN);
    await settle(tester);
    expect(find.byType(NoteField), findsOneWidget);

    expect(await tester.sendKeyEvent(LogicalKeyboardKey.keyS), isFalse);
    expect(await tester.sendKeyEvent(LogicalKeyboardKey.keyP), isFalse);
    await settle(tester);

    expect(client.decisions, isEmpty);
    expect(find.byKey(const Key('editor-send')), findsNothing);
    expect(pause, findsOneWidget);
  }, variant: linux);

  testWidgets('many findings scroll and the buttons stay reachable', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = mailClient(
      holdScript(<FlowDetail>[
        withMail(
          1,
          findings: <Finding>[
            for (int i = 0; i < 30; i++) mailFinding(start: 5, end: 11),
          ],
        ),
      ]),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await armed(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await settle(tester);

    expect(tester.takeException(), isNull, reason: 'no overflow');
    expect(
      tester
          .getSize(find.byKey(const Key('intercept-findings-pause-list')))
          .height,
      lessThanOrEqualTo(findingsPauseListMaxHeight),
    );
    await tester.tap(find.byKey(const Key('intercept-findings-pause-send')));
    await settle(tester);
    expect(client.decisions, hasLength(1));
  }, variant: linux);

  testWidgets('a held Esc closes the pause and leaves the coach mark', (
    WidgetTester tester,
  ) async {
    // Der erste Druck schließt die Pause. Die Wiederholungen derselben Taste
    // sind ein Finger, der noch unten ist, und kein zweiter Wunsch: Sie dürfen
    // nicht an den einmaligen Hinweis darunter durchfallen.
    final FakeDaemonClient client = mailClient();
    await pumpIntercept(
      tester,
      client: client,
      uiState: uiStateOverride(seen: false),
    );
    await playScript(tester);
    final Finder mark = find.byKey(const Key('intercept-coach-mark'));
    expect(mark, findsOneWidget);
    await armed(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await settle(tester);
    expect(pause, findsOneWidget);

    await tester.sendKeyDownEvent(LogicalKeyboardKey.escape);
    await settle(tester);
    for (int i = 0; i < 3; i++) {
      await tester.sendKeyRepeatEvent(LogicalKeyboardKey.escape);
      await settle(tester);
    }
    await tester.sendKeyUpEvent(LogicalKeyboardKey.escape);
    await settle(tester);

    expect(pause, findsNothing);
    expect(mark, findsOneWidget);
    expect(client.decisions, isEmpty);
  }, variant: linux);
}
