// Zustände der Request-Karte aus HUM-058: die abgelaufene Anfrage (Zeile 7)
// und die Anfrage, die eine WebSocket-Verbindung öffnet (Zeile 11). Große
// und binäre Rümpfe (Zeilen 8 und 10) prüfen schon die Tests von HUM-030 und
// HUM-047, siehe `backlog/sprint-5.md`, Abschnitt HUM-058.
//
// Gegen welche Umkehrung jeder Test gerichtet ist:
//
// - `_timedOut` liefert wieder den alten Satz ohne Wartezeit:
//   `timeout_banner_names_the_wait` wird rot.
// - `_WebSocketNotice` fehlt in der Karte, oder `requestOpensWebSocket`
//   vergleicht den Wert mit Groß- und Kleinschreibung:
//   `websocket_card_copy` wird rot.
// - Der Zweig `websocket` vor `allowLabel` in der Aktionsleiste weg: rot im
//   selben Test, weil der Knopf wieder „Allow" heißt.
// - `_TimeoutRule` fehlt unter dem Band, oder `TimeoutRule.draftFor` nimmt
//   die Dauer des offenen Rasters nicht: `timeout_banner_keeps_draft` wird
//   rot.

import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/providers/decision.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/providers/timeout_rule.dart';
import 'package:humanitl/features/intercept/providers/now.dart';
import 'package:humanitl/features/intercept/rule_sentence.dart';
import 'package:humanitl/core/domain/websocket.dart';
import 'package:humanitl/features/intercept/widgets/action_bar.dart';
import 'package:humanitl/features/intercept/widgets/request_card.dart';
import 'package:humanitl/features/shell/providers/connection.dart';

import '../../goldens/support/golden_variants.dart';
import '../../harness/ui_state.dart';
import 'fixtures.dart';

/// Karte und Aktionsleiste zu [detail], in [variant].
Widget cardAndBar(
  GoldenVariant variant,
  FlowDetail detail, {
  TestDaemonClient? daemon,
}) {
  final TestDaemonClient client = daemon ?? TestDaemonClient();
  client.details[detail.summary.id] = detail;
  return goldenFrame(
    variant: variant,
    overrides: <Override>[
      uiStateOverride(),
      daemonClientProvider.overrideWithValue(client),
      connectionHeartbeatProvider.overrideWithValue(null),
      nowProvider.overrideWith(() => FixedNow(testStart)),
      flowsProvider.overrideWith(
        () => FixedFlows(<FlowId, Flow>{detail.summary.id: detail.summary}),
      ),
    ],
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Expanded(child: RequestCard(flow: detail.summary)),
        const HHairline(),
        ActionBar(flow: detail.summary),
      ],
    ),
  );
}

Future<void> show(
  WidgetTester tester,
  FlowDetail detail, {
  GoldenVariant variant = GoldenVariant.darkEn,
  TestDaemonClient? daemon,
}) async {
  await tester.binding.setSurfaceSize(const Size(1000, 700));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(cardAndBar(variant, detail, daemon: daemon));
  await tester.pump();
  await tester.pump();
}

/// Ein Daemon, der jede Regel ablehnt.
class RefusingClient extends TestDaemonClient {
  @override
  Future<RuleSet> addRule(Rule rule) async => throw const DaemonException(
    Diagnostic(
      code: 'RULES_001',
      severity: Severity.error,
      why: 'rules.yaml is not writable',
    ),
  );
}

/// Eine Anfrage, deren Frist nach [budget] ablief.
FlowDetail timedOut({Duration? budget = const Duration(seconds: 300)}) {
  final Flow held = heldFlow(
    n: 1,
    deadline: testStart.add(budget ?? Duration.zero),
  );
  return detailFor(
    held.copyWith(
      state: FlowState.decided,
      decision: DecisionKind.timedOut,
      decisionSource: DecisionSource.timeout,
      deadline: budget == null ? null : held.deadline,
    ),
  );
}

/// Ein gehaltener Verbindungsaufbau an `ws.example.com` mit [upgrade] in der
/// Kopfzeile `Upgrade`, ohne die Kopfzeile bei null.
FlowDetail upgrade(String? upgrade) => detailFor(
  heldFlow(
    n: 2,
    deadline: testStart.add(const Duration(minutes: 4)),
    host: 'ws.example.com',
    path: '/socket',
  ),
  headers: <Header>[
    header('host', 'ws.example.com'),
    if (upgrade != null) ...<Header>[
      header('connection', 'Upgrade'),
      header('upgrade', upgrade),
    ],
  ],
);

String banner(WidgetTester tester) =>
    tester.widget<Text>(find.byKey(const Key('intercept-card-banner'))).data!;

void main() {
  group('timeout_banner_names_the_wait', () {
    testWidgets('en', (WidgetTester tester) async {
      await show(tester, timedOut());
      expect(
        banner(tester),
        'Blocked (timed out after 300 s). The agent may retry.',
      );
    });

    testWidgets('de', (WidgetTester tester) async {
      await show(tester, timedOut(), variant: GoldenVariant.lightDe);
      expect(
        banner(tester),
        'Blockiert (Zeit nach 300 s abgelaufen). Der Agent kann es erneut '
        'versuchen.',
      );
    });

    testWidgets('another budget, another number', (WidgetTester tester) async {
      await show(tester, timedOut(budget: const Duration(seconds: 45)));
      expect(banner(tester), contains('after 45 s'));
    });

    testWidgets('without a deadline it invents no number', (
      WidgetTester tester,
    ) async {
      await show(tester, timedOut(budget: null));
      expect(banner(tester), 'Held, then blocked (timeout)');
    });
  });

  group('timeout_banner_keeps_draft', () {
    testWidgets('the draft of the grid becomes the rule', (
      WidgetTester tester,
    ) async {
      final TestDaemonClient client = TestDaemonClient();
      await show(tester, timedOut(), daemon: client);
      final ProviderContainer container = ProviderScope.containerOf(
        tester.element(find.byType(RequestCard)),
      );
      // Was jemand vor dem Ablauf im Raster gewählt hatte, steht noch da.
      container
          .read(rememberDraftProvider.notifier)
          .setDuration(RememberDuration.forever);
      await tester.pump();

      expect(find.byKey(const Key('intercept-timeout-rule')), findsOneWidget);
      await tester.tap(find.byKey(const Key('intercept-timeout-rule-save')));
      await tester.pump();

      // „Für immer" fragt erst im Modal der Aktionsleiste, und ohne
      // Bestätigung entsteht keine Regel (`docs/UX.md` 5.4).
      expect(client.remembered, isEmpty);
      final BatchRequest? asked = container.read(batchConfirmProvider);
      expect(asked?.ruleOnly, isTrue);
      expect(asked?.reason, ConfirmReason.forever);
      await container.read(interceptDecisionProvider.notifier).confirmBatch();
      await tester.pump();
      await tester.pump();

      expect(client.remembered, hasLength(1));
      expect(client.decisions, isEmpty, reason: 'nothing is decided');
      final Rule rule = client.remembered.single;
      expect(rule.action, RuleAction.allow);
      expect(rule.matcher.host, 'api.github.com');
      expect(rule.expires, const RuleExpiry.never());
      expect(rule.createdFrom, testFlowId(1));
      expect(
        find.text('Rule saved. The agent\'s next attempt is decided by it.'),
        findsOneWidget,
      );
      expect(
        find.byKey(const Key('intercept-timeout-rule-save')),
        findsNothing,
      );
    });

    testWidgets('a closed grid saves a rule for the session', (
      WidgetTester tester,
    ) async {
      final TestDaemonClient client = TestDaemonClient();
      await show(
        tester,
        timedOut(),
        daemon: client,
        variant: GoldenVariant.darkDe,
      );
      expect(find.text('Als Regel übernehmen'), findsOneWidget);

      await tester.tap(find.byKey(const Key('intercept-timeout-rule-save')));
      await tester.pump();
      await tester.pump();

      expect(client.remembered.single.expires, const RuleExpiry.session());
    });

    testWidgets('a second save of the same request writes no second rule', (
      WidgetTester tester,
    ) async {
      final TestDaemonClient client = TestDaemonClient();
      final FlowDetail detail = timedOut();
      await show(tester, detail, daemon: client);
      final ProviderContainer container = ProviderScope.containerOf(
        tester.element(find.byType(RequestCard)),
      );
      final TimeoutRule notifier = container.read(timeoutRuleProvider.notifier);

      await notifier.save(detail.summary);
      await notifier.save(detail.summary);
      await tester.pump();

      expect(client.remembered, hasLength(1));
    });

    testWidgets('each request keeps its own saved rule', (
      WidgetTester tester,
    ) async {
      // A, dann B, dann noch einmal A: Gemerkt wird je Anfrage, nicht nur die
      // zuletzt gespeicherte (Review).
      final TestDaemonClient client = TestDaemonClient();
      final FlowDetail a = timedOut();
      await show(tester, a, daemon: client);
      final ProviderContainer container = ProviderScope.containerOf(
        tester.element(find.byType(RequestCard)),
      );
      final Flow b = a.summary.copyWith(id: testFlowId(9));
      final TimeoutRule notifier = container.read(timeoutRuleProvider.notifier);

      await notifier.save(a.summary);
      await notifier.save(b);
      await notifier.save(a.summary);

      expect(client.remembered, hasLength(2));
      expect(client.remembered.map((Rule rule) => rule.createdFrom), <FlowId>[
        testFlowId(1),
        testFlowId(9),
      ]);
    });

    testWidgets('a held request offers no such rule', (
      WidgetTester tester,
    ) async {
      await show(tester, upgrade(null));
      expect(find.byKey(const Key('intercept-timeout-rule')), findsNothing);
    });

    testWidgets('a refusal of the daemon stands under the button', (
      WidgetTester tester,
    ) async {
      final RefusingClient client = RefusingClient();
      await show(tester, timedOut(), daemon: client);
      await tester.tap(find.byKey(const Key('intercept-timeout-rule-save')));
      await tester.pump();
      await tester.pump();

      expect(
        find.text('Rule not saved: rules.yaml is not writable'),
        findsOneWidget,
      );
      expect(
        find.byKey(const Key('intercept-timeout-rule-save')),
        findsOneWidget,
      );
    });
  });

  group('websocket_card_copy', () {
    testWidgets('en', (WidgetTester tester) async {
      await show(tester, upgrade('websocket'));

      expect(find.byKey(const Key('intercept-card-websocket')), findsOneWidget);
      expect(find.text('WebSocket'), findsOneWidget);
      expect(
        find.text(
          'Opens a lasting connection to ws.example.com. Messages after '
          'that are recorded, not held.',
        ),
        findsOneWidget,
      );
      expect(find.text('Allow connection'), findsOneWidget);
    });

    testWidgets('de', (WidgetTester tester) async {
      await show(tester, upgrade('WebSocket'), variant: GoldenVariant.darkDe);

      expect(
        find.textContaining('Öffnet eine dauerhafte Verbindung zu '),
        findsOneWidget,
      );
      expect(find.text('Verbindung öffnen'), findsOneWidget);
    });

    testWidgets('an ordinary request keeps its card and its button', (
      WidgetTester tester,
    ) async {
      await show(tester, upgrade(null));

      expect(find.byKey(const Key('intercept-card-websocket')), findsNothing);
      expect(find.text('Allow connection'), findsNothing);
      expect(find.text('Allow'), findsOneWidget);
    });

    testWidgets('another upgrade is no websocket', (WidgetTester tester) async {
      await show(tester, upgrade('h2c'));

      expect(find.byKey(const Key('intercept-card-websocket')), findsNothing);
      expect(find.text('Allow connection'), findsNothing);
    });
  });

  group('requestOpensWebSocket mirrors the daemon', () {
    test('header, case-insensitive, first one counts', () {
      expect(
        requestOpensWebSocket(<Header>[header('Upgrade', 'WEBSOCKET')]),
        isTrue,
      );
      expect(
        requestOpensWebSocket(<Header>[
          header('upgrade', 'h2c'),
          header('upgrade', 'websocket'),
        ]),
        isFalse,
      );
      expect(
        requestOpensWebSocket(<Header>[header('upgrade', 'websocket, h2c')]),
        isFalse,
      );
      expect(requestOpensWebSocket(const <Header>[]), isFalse);
    });

    test('the header decides, not a scheme', () {
      // Wie `requested_upgrade` in `daemon/crates/proxy/src/connect.rs`: Der
      // Proxy vergibt `ws`/`wss` nie, und ohne Kopfzeile kein Wechsel.
      expect(requestOpensWebSocket(const <Header>[]), isFalse);
      expect(
        requestOpensWebSocket(<Header>[header('connection', 'Upgrade')]),
        isFalse,
      );
    });

    test('a value that is no visible ASCII is no upgrade', () {
      expect(
        requestOpensWebSocket(const <Header>[
          Header(
            name: 'upgrade',
            value: <int>[0x77, 0x65, 0x62, 0xFF, 0x6F, 0x63, 0x6B, 0x65, 0x74],
          ),
        ]),
        isFalse,
      );
    });
  });
}
