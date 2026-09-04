// Die Aktionsleiste (HUM-028) am ganzen Screen: Tastatur, Armierung,
// Halten, Merken-Raster, Abstand, Semantik und Textskalierung.
//
// Die Regeln aus docs/UX.md 5.4 stehen hier als Tests, nicht als Absicht:
// Erlauben feuert erst nach `HMotion.rearm`, eine Tastenwiederholung
// entscheidet nie, gedrücktes Enter erzeugt genau ein `Decide`, und die
// Auswahl wartet, solange eine Entscheidungstaste unten ist.

import 'package:flutter/semantics.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/app.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/intents.dart';
import 'package:humanitl/features/intercept/intercept_screen.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/providers/now.dart';
import 'package:humanitl/features/intercept/widgets/action_bar.dart';
import 'package:humanitl/features/intercept/widgets/block_button.dart';
import 'package:humanitl/features/intercept/widgets/release_valve.dart';
import 'package:humanitl/features/shell/providers/connection.dart';
import 'package:humanitl/features/shell/widgets/header_bar.dart';

import 'fixtures.dart';

/// Ein Fake mit stehender Uhr.
FakeDaemonClient fake([List<ScriptedEvent>? script]) =>
    FakeDaemonClient(script: script, clock: () => testStart);

FlowDetail github({int n = 1, int findings = 0}) => detailFor(
  heldFlow(
    n: n,
    deadline: testStart.add(const Duration(minutes: 5)),
    method: Method.post,
    host: 'api.github.com',
    path: '/graphql?first=20',
    requestSize: 428,
  ).copyWith(findingCount: findings),
  bodyPreview: '{"query": "mutation { createIssue }"}',
  contentType: 'application/json',
  // Die registrierbare Domain kommt aus dem Katalog des Daemons; ohne sie
  // gibt es den Domain-Scope nicht (CONVENTIONS 4.13).
  apex: 'github.com',
  findings: <Finding>[for (int i = 0; i < findings; i++) testFinding()],
);

FlowDetail npm({int n = 2}) => detailFor(
  heldFlow(
    n: n,
    deadline: testStart.add(const Duration(minutes: 8)),
    host: 'registry.npmjs.org',
    path: '/react',
  ),
);

/// Baut die App über [client] und lässt das Verbindungs-Gate durch.
Future<void> pumpIntercept(
  WidgetTester tester, {
  required FakeDaemonClient client,
  List<Override> overrides = const <Override>[],
  Size size = const Size(1400, 900),
  TextScaler textScaler = TextScaler.noScaling,
}) async {
  await tester.binding.setSurfaceSize(size);
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    ProviderScope(
      overrides: <Override>[
        daemonClientProvider.overrideWithValue(client),
        connectionHeartbeatProvider.overrideWithValue(null),
        nowProvider.overrideWith(() => FixedNow(testStart)),
        ...overrides,
      ],
      child: MediaQuery(
        // `supportsAnnounce` kommt sonst von der Plattform und ist im Test
        // aus; die Ansagen aus docs/UX.md 6 wären damit nie prüfbar.
        data: MediaQueryData(textScaler: textScaler, supportsAnnounce: true),
        child: const HumanitlApp(),
      ),
    ),
  );
  await tester.pump();
  await tester.pump();
}

/// Lässt das Skript ablaufen. Die Vorgabe liegt über `HMotion.rearm`, also
/// ist die Auswahl danach scharf.
Future<void> playScript(
  WidgetTester tester, [
  Duration duration = const Duration(milliseconds: 400),
]) async {
  await tester.pump(duration);
  await tester.pump();
  await tester.pump();
}

/// Hält [finder] für [duration] gedrückt.
Future<void> hold(WidgetTester tester, Finder finder, Duration duration) async {
  final TestGesture gesture = await tester.startGesture(
    tester.getCenter(finder),
  );
  await tester.pump();
  await tester.pump(duration);
  await gesture.up();
  await tester.pump();
  await tester.pump();
}

/// Drückt [key] mit gehaltener Umschalttaste.
Future<void> pressShift(WidgetTester tester, LogicalKeyboardKey key) async {
  await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
  await tester.sendKeyEvent(key);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
  await tester.pump();
}

/// Sammelt die Ansagen, die an die Barrierefreiheit gehen.
List<String> captureAnnouncements(WidgetTester tester) {
  final List<String> said = <String>[];
  tester.binding.defaultBinaryMessenger.setMockDecodedMessageHandler<Object?>(
    SystemChannels.accessibility,
    (Object? message) async {
      final Map<Object?, Object?> event = (message! as Map<Object?, Object?>);
      if (event['type'] == 'announce') {
        final Map<Object?, Object?> data =
            event['data']! as Map<Object?, Object?>;
        said.add(data['message']! as String);
      }
      return null;
    },
  );
  addTearDown(
    () => tester.binding.defaultBinaryMessenger
        .setMockDecodedMessageHandler<Object?>(
          SystemChannels.accessibility,
          null,
        ),
  );
  return said;
}

void main() {
  testWidgets('a refused key is shown and said out loud', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fake(holdScript(<FlowDetail>[github()]));
    await pumpIntercept(tester, client: client);
    final List<String> said = captureAnnouncements(tester);
    await tester.pump(const Duration(milliseconds: 100));
    await tester.pump();

    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();

    expect(client.decisions, isEmpty);
    expect(said, contains('The request has just changed · read it, then send'));
  });

  testWidgets('a decision is announced with host and size', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fake(holdScript(<FlowDetail>[github()]));
    await pumpIntercept(tester, client: client);
    final List<String> said = captureAnnouncements(tester);
    await playScript(tester);

    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    await tester.pump();

    expect(said, contains('Sent to api.github.com · 428 B'));
  });

  testWidgets('allow fires only after the URL has stood still', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fake(holdScript(<FlowDetail>[github()]));
    await pumpIntercept(tester, client: client);
    // Kürzer als `HMotion.rearm`: die URL stand noch nicht lange genug.
    await tester.pump(const Duration(milliseconds: 100));
    await tester.pump();

    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();

    expect(client.decisions, isEmpty);
    expect(
      find.text('The request has just changed · read it, then send'),
      findsOneWidget,
    );

    await tester.pump(HMotion.rearm);
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();

    expect(client.decisions, hasLength(1));
  });

  testWidgets('enter held for 500 ms produces exactly one decide', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fake(
      holdScript(<FlowDetail>[github(), npm(), github(n: 3)]),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    await tester.sendKeyDownEvent(LogicalKeyboardKey.enter);
    for (int i = 0; i < 10; i++) {
      await tester.pump(const Duration(milliseconds: 50));
      await tester.sendKeyRepeatEvent(LogicalKeyboardKey.enter);
    }
    await tester.sendKeyUpEvent(LogicalKeyboardKey.enter);
    await tester.pump();

    expect(client.decisions, hasLength(1));
  });

  testWidgets('a key repeat never decides, and a held key stays locked', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fake(
      holdScript(<FlowDetail>[github(), npm()]),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    await tester.sendKeyDownEvent(LogicalKeyboardKey.keyA);
    await tester.pump();
    expect(client.decisions, hasLength(1));

    // Ein zweiter Druck ohne Loslassen wird abgelehnt, nicht ausgeführt.
    await tester.sendKeyDownEvent(LogicalKeyboardKey.keyA);
    await tester.pump();
    expect(client.decisions, hasLength(1));
    expect(find.text('Release the key, then decide again'), findsOneWidget);

    await tester.sendKeyUpEvent(LogicalKeyboardKey.keyA);
    await tester.pump();
  });

  testWidgets('the selection waits while a decision key is down', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fake(
      holdScript(<FlowDetail>[github(), npm()]),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    final ProviderContainer container = ProviderScope.containerOf(
      tester.element(find.byType(InterceptScreen)),
    );
    expect(container.read(selectedFlowIdProvider), testFlowId(1));

    await tester.sendKeyDownEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    await tester.pump();
    // Entschieden, aber die Taste ist noch unten: die Auswahl bleibt stehen.
    expect(client.decisions, hasLength(1));
    expect(container.read(selectedFlowIdProvider), testFlowId(1));

    await tester.sendKeyUpEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(container.read(selectedFlowIdProvider), testFlowId(2));
  });

  testWidgets('enter belongs to the focused control, not to the queue', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fake(holdScript(<FlowDetail>[github()]));
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    // Bis der Fokus auf Blockieren steht.
    bool onBlock() {
      final BuildContext? context = FocusManager.instance.primaryFocus?.context;
      return context != null &&
          context.findAncestorWidgetOfExactType<BlockButton>() != null;
    }

    for (int i = 0; i < 12 && !onBlock(); i++) {
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
    }
    expect(onBlock(), isTrue, reason: 'focus never reached Block');

    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    await tester.pump();

    // Der Screen bindet Enter auf Erlauben; das fokussierte Control gewinnt
    // trotzdem (docs/UX.md 5.2).
    expect(client.decisions, hasLength(1));
    expect(client.decisions.single.decision, const Decision.block());
  });

  testWidgets('B blocks at once, without a hold', (WidgetTester tester) async {
    final FakeDaemonClient client = fake(holdScript(<FlowDetail>[github()]));
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    await tester.sendKeyEvent(LogicalKeyboardKey.keyB);
    await tester.pump();

    expect(client.decisions, hasLength(1));
    expect(client.decisions.single.decision, const Decision.block());
  });

  testWidgets('every bound key has an action in the screen', (
    WidgetTester tester,
  ) async {
    await pumpIntercept(
      tester,
      client: fake(holdScript(<FlowDetail>[github()])),
    );
    await playScript(tester);

    final Set<Type> handled = <Type>{
      for (final Actions actions in tester.widgetList<Actions>(
        find.descendant(
          of: find.byType(InterceptScreen),
          matching: find.byType(Actions),
        ),
      ))
        ...actions.actions.keys,
    };
    final Set<Type> bound = interceptShortcuts().values
        .map((Intent intent) => intent.runtimeType)
        .toSet();

    // Eine Bindung ohne Action wird gelöscht, nicht stillgelegt
    // (docs/UX.md 5.3).
    expect(bound.difference(handled), isEmpty);
  });

  testWidgets('no control on the bar is dead', (WidgetTester tester) async {
    await pumpIntercept(
      tester,
      client: fake(holdScript(<FlowDetail>[github()])),
    );
    await playScript(tester);

    // `Edit + Allow` konnte nie gedrückt werden; ein toter Zustand ohne Grund
    // ist schlimmer als ein fehlender (CONVENTIONS 4.13). Es kommt mit dem
    // Editor aus HUM-047 zurück.
    expect(find.byKey(const Key('intercept-edit-allow')), findsNothing);
    for (final HButton button in tester.widgetList<HButton>(
      find.descendant(
        of: find.byType(ActionBar),
        matching: find.byType(HButton),
      ),
    )) {
      expect(
        button.onPressed,
        isNotNull,
        reason: 'a control that does nothing',
      );
    }
  });

  testWidgets('allow and block keep their distance at 900 px', (
    WidgetTester tester,
  ) async {
    await pumpIntercept(
      tester,
      client: fake(holdScript(<FlowDetail>[github()])),
      // Der Inspector-Pane misst bei 2000 px Fenster rund 900 px.
      size: const Size(2000, 900),
    );
    await playScript(tester);

    final Rect allow = tester.getRect(find.byType(ReleaseValve));
    final Rect block = tester.getRect(find.byType(BlockButton));
    expect(block.left - allow.right, greaterThanOrEqualTo(decisionGap));
  });

  testWidgets('the bar wraps and puts block on its own line', (
    WidgetTester tester,
  ) async {
    await pumpIntercept(
      tester,
      client: fake(holdScript(<FlowDetail>[github()])),
      size: const Size(1100, 900),
    );
    await playScript(tester);

    final Rect allow = tester.getRect(find.byType(ReleaseValve));
    final Rect block = tester.getRect(find.byType(BlockButton));
    expect(block.top, greaterThanOrEqualTo(allow.bottom));
  });

  testWidgets('the remember keys build the rule and the sentence', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fake(holdScript(<FlowDetail>[github()]));
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    // Ohne Raster steht der Haltegrund da und keine Regel.
    expect(find.text('Held: no rule matches · default: ask'), findsOneWidget);

    // `2` ist die Session, `Shift+3` die registrierbare Domain.
    await tester.sendKeyEvent(LogicalKeyboardKey.digit2);
    await tester.pump();
    await pressShift(tester, LogicalKeyboardKey.digit3);

    expect(
      find.text('allow · ∗ · **.github.com · this session'),
      findsOneWidget,
    );

    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    await tester.pump();

    expect(client.decisions, hasLength(1));
    final Rule? remembered = client.decisions.single.remember;
    expect(remembered, isNotNull);
    expect(remembered!.matcher.host, '**.github.com');
    expect(remembered.expires, const RuleExpiry.session());
    expect(client.rules, hasLength(1));
  });

  testWidgets('once creates no rule and greys the scope out', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fake(holdScript(<FlowDetail>[github()]));
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    await tester.sendKeyEvent(LogicalKeyboardKey.digit1);
    await tester.pump();

    expect(find.byKey(const Key('intercept-rule-sentence')), findsNothing);
    expect(find.text('Allow'), findsOneWidget);

    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    await tester.pump();

    expect(client.decisions.single.remember, isNull);
  });

  testWidgets('the hold reason is built from the flow', (
    WidgetTester tester,
  ) async {
    await pumpIntercept(
      tester,
      client: fake(holdScript(<FlowDetail>[github(findings: 2)])),
    );
    await playScript(tester);

    // Sobald der Daemon den Fund beschreibt, nennt der Grund Art und Ort in
    // Klartext; der `kind`-Bezeichner steht nie auf dem Schirm (docs/UX.md 4.3).
    expect(
      find.text('Held: a github API key was found in the body'),
      findsOneWidget,
    );
  });

  testWidgets('near the end the bar names the consequence, not a bare clock', (
    WidgetTester tester,
  ) async {
    // Zehn Prozent des Budgets sind übrig: unter der Schwelle, ab der die
    // App einmal warnt (docs/UX.md 4.8).
    final Flow nearlyOver =
        heldFlow(
          n: 1,
          deadline: testStart.add(const Duration(seconds: 30)),
          host: 'registry.npmjs.org',
          path: '/react',
        ).copyWith(
          heldAt: testStart.subtract(const Duration(minutes: 4, seconds: 30)),
        );
    await pumpIntercept(
      tester,
      client: fake(const <ScriptedEvent>[]),
      overrides: <Override>[
        flowsProvider.overrideWith(
          () => FixedFlows(<FlowId, Flow>{nearlyOver.id: nearlyOver}),
        ),
      ],
    );
    await playScript(tester);

    expect(
      find.text('registry.npmjs.org auto-blocks in 00:30'),
      findsOneWidget,
    );
  });

  testWidgets('both decisions carry the remaining time as a semantics value', (
    WidgetTester tester,
  ) async {
    final SemanticsHandle handle = tester.ensureSemantics();
    await pumpIntercept(
      tester,
      client: fake(holdScript(<FlowDetail>[github()])),
    );
    await playScript(tester);

    // Die Frist steht im Semantics-Value, nie im Label: ein Label mit
    // `mm:ss` wird bei jedem Fokus vollständig neu vorgelesen (docs/UX.md 6).
    for (final Key key in <Key>[
      const Key('intercept-block-label'),
      const Key('intercept-valve-label'),
    ]) {
      final SemanticsNode node = tester.getSemantics(find.byKey(key));
      expect(node.value, '05:00 left', reason: '$key');
      expect(node.label, isNotEmpty, reason: '$key');
    }
    handle.dispose();
  });

  testWidgets('the bar survives twice the text scale', (
    WidgetTester tester,
  ) async {
    await pumpIntercept(
      tester,
      client: fake(holdScript(<FlowDetail>[github()])),
      textScaler: const TextScaler.linear(2),
    );
    await playScript(tester);

    expect(tester.takeException(), isNull);
    expect(find.byType(ActionBar), findsOneWidget);
    // Auch mit offenem Raster, das am meisten Text trägt.
    await tester.sendKeyEvent(LogicalKeyboardKey.digit2);
    await tester.pump();
    expect(tester.takeException(), isNull);
  });

  testWidgets('a decision does not rebuild the shell', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fake(
      holdScript(<FlowDetail>[github(), npm()]),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    final Element header = tester.element(find.byType(HeaderBar));
    final Element shell = tester.element(find.byType(InterceptScreen));

    await tester.sendKeyDownEvent(LogicalKeyboardKey.keyA);
    // Vor dem Frame gemessen: was hier nicht schmutzig ist, baut auch nicht
    // neu (docs/UX.md 7).
    expect(header.dirty, isFalse);
    expect(shell.dirty, isFalse);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.keyA);
    await tester.pump();

    expect(client.decisions, hasLength(1));
  });
}
