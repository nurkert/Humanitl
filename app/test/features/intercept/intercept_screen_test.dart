// Widget-Tests des Intercept-Screens (HUM-020): Tastatur, Maskierung,
// Ablauf und die Wirkung der Aktionsleiste, alles gegen den Fake.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/app.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/widgets/block_button.dart';
import 'package:humanitl/features/intercept/widgets/release_valve.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:humanitl/features/intercept/intercept_screen.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/providers/now.dart';
import 'package:humanitl/features/intercept/providers/pane_layout.dart';
import 'package:humanitl/features/intercept/widgets/queue_pane.dart';
import 'package:humanitl/features/intercept/widgets/queue_row.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/features/shell/providers/connection.dart';

import 'fixtures.dart';

/// Ein Fake mit stehender Uhr: seine Ereignisse tragen dieselbe Zeit wie
/// [FixedNow], damit Fristen und Ausstiegsfenster berechenbar sind.
FakeDaemonClient fake([List<ScriptedEvent>? script]) =>
    FakeDaemonClient(script: script, clock: () => testStart);

/// Baut die App über [client] und lässt das Verbindungs-Gate durch.
Future<void> pumpIntercept(
  WidgetTester tester, {
  required FakeDaemonClient client,
  List<Override> overrides = const <Override>[],
  Size size = const Size(1400, 900),
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
      child: const HumanitlApp(),
    ),
  );
  await tester.pump();
  await tester.pump();
}

/// Der Provider-Container des laufenden Baums.
ProviderContainer containerOf(WidgetTester tester) =>
    ProviderScope.containerOf(tester.element(find.byType(InterceptScreen)));

/// Lässt das Skript des Fakes ablaufen und die Antworten auf `getFlow`
/// eintreffen: ein Frame für die Ereignisse, zwei für die Provider, die
/// darauf erst im nächsten Frame neu bauen.
///
/// Die Vorgabedauer liegt über `HMotion.rearm`, damit die Auswahl scharf ist:
/// Erlauben feuert erst, wenn die URL lange genug stand (docs/UX.md 5.4).
Future<void> playScript(
  WidgetTester tester, [
  Duration duration = const Duration(milliseconds: 400),
]) async {
  await tester.pump(duration);
  await tester.pump();
  await tester.pump();
}

/// Wartet, bis Erlauben wieder feuert (`HMotion.rearm` plus ein Frame).
Future<void> rearm(WidgetTester tester) async {
  await tester.pump(HMotion.rearm + const Duration(milliseconds: 50));
  await tester.pump();
}

/// Hält [finder] für [duration] gedrückt: die Halte-Bestätigung des
/// Blockierens (250 ms) und der Release Valve (400 ms).
Future<void> hold(WidgetTester tester, Finder finder, Duration duration) async {
  final TestGesture gesture = await tester.startGesture(
    tester.getCenter(finder),
  );
  // Ein Frame startet den Ticker, erst der nächste bewegt ihn: ohne diesen
  // Frame misst der Controller die ganze Haltezeit als null.
  await tester.pump();
  await tester.pump(duration);
  await gesture.up();
  await tester.pump();
  await tester.pump();
}

/// Drückt [key] mit gehaltener Strg-Taste.
Future<void> pressCtrl(WidgetTester tester, LogicalKeyboardKey key) async {
  await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
  await tester.sendKeyEvent(key);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
  await tester.pump();
}

FlowDetail github({int n = 1, List<Header> headers = const <Header>[]}) =>
    detailFor(
      heldFlow(
        n: n,
        deadline: testStart.add(const Duration(minutes: 5)),
        method: Method.post,
        host: 'api.github.com',
        path: '/graphql?first=20',
        requestSize: 42,
      ),
      headers: headers,
      bodyPreview: '{"query": "mutation { createIssue }"}',
      contentType: 'application/json',
    );

FlowDetail pypi({int n = 2}) => detailFor(
  heldFlow(
    n: n,
    deadline: testStart.add(const Duration(minutes: 5)),
    host: 'pypi.org',
    path: '/simple/requests/',
  ),
);

void main() {
  testWidgets('enter_allows_selected', (WidgetTester tester) async {
    final FakeDaemonClient client = fake(
      holdScript(<FlowDetail>[github(), pypi()]),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    expect(find.byType(QueueRow), findsNWidgets(2));

    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();

    expect(client.decisions, hasLength(1));
    expect(client.decisions.single.flowId, testFlowId(1));
    expect(client.decisions.single.decision, const Decision.allow());
  });

  testWidgets('enter without a selection decides nothing', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fake(const <ScriptedEvent>[]);
    await pumpIntercept(tester, client: client);
    await playScript(tester, const Duration(milliseconds: 200));

    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();

    expect(client.decisions, isEmpty);
  });

  testWidgets('single_keys_ignored_in_textfield', (WidgetTester tester) async {
    final FakeDaemonClient client = fake(holdScript(<FlowDetail>[github()]));
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    await pressCtrl(tester, LogicalKeyboardKey.keyK);
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.keyA);
    await tester.pump();

    expect(client.decisions, isEmpty);
  });

  testWidgets('keys do nothing while another section shows', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fake(holdScript(<FlowDetail>[github()]));
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    await pressCtrl(tester, LogicalKeyboardKey.digit2);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyA);
    await tester.pump();

    expect(client.decisions, isEmpty);

    // Zurück im Abschnitt wirkt dieselbe Taste.
    await pressCtrl(tester, LogicalKeyboardKey.digit1);
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.keyA);
    await tester.pump();

    expect(client.decisions, hasLength(1));
  });

  testWidgets('block from the action bar needs the hold', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fake(holdScript(<FlowDetail>[github()]));
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    final Finder block = find.byKey(const Key('intercept-block'));
    // Ein kurzer Klick entscheidet nicht, er nennt den Grund (docs/UX.md 5.3).
    await hold(tester, block, const Duration(milliseconds: 100));
    expect(client.decisions, isEmpty);
    expect(find.text('Hold to block'), findsOneWidget);

    await hold(
      tester,
      block,
      HMotion.holdToBlock + const Duration(milliseconds: 50),
    );
    expect(client.decisions, hasLength(1));
    expect(client.decisions.single.decision, const Decision.block());
    // Ohne Raster wird keine Regel angelegt.
    expect(client.decisions.single.remember, isNull);
  });

  testWidgets('j and k move the selection', (WidgetTester tester) async {
    final FakeDaemonClient client = fake(
      holdScript(<FlowDetail>[github(), pypi()]),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    QueueRow rowAt(int index) =>
        tester.widgetList<QueueRow>(find.byType(QueueRow)).elementAt(index);
    expect(rowAt(0).selected, isTrue);

    await tester.sendKeyEvent(LogicalKeyboardKey.keyJ);
    await tester.pump();
    expect(rowAt(1).selected, isTrue);

    await tester.sendKeyEvent(LogicalKeyboardKey.keyK);
    await tester.pump();
    expect(rowAt(0).selected, isTrue);
  });

  testWidgets('headers_masked_by_default', (WidgetTester tester) async {
    final FakeDaemonClient client = fake(
      holdScript(<FlowDetail>[
        github(
          headers: <Header>[
            header('authorization', 'Bearer secret-token'),
            header('content-type', 'application/json'),
          ],
        ),
      ]),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    expect(find.text('••••'), findsOneWidget);
    expect(find.text('Bearer secret-token'), findsNothing);

    await tester.tap(find.byKey(const Key('header-eye-authorization')));
    await tester.pump();

    expect(find.text('Bearer secret-token'), findsOneWidget);
    expect(find.text('••••'), findsNothing);
    // Der unverfängliche Header war nie maskiert.
    expect(find.text('application/json'), findsOneWidget);
  });

  testWidgets('timeout_marks_card', (WidgetTester tester) async {
    final FakeDaemonClient client = fake(<ScriptedEvent>[
      ...holdScript(<FlowDetail>[github()]),
      timeoutAfter(testFlowId(1), const Duration(milliseconds: 50)),
    ]);
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    final Finder banner = find.byKey(const Key('intercept-card-banner'));
    expect(banner, findsOneWidget);
    expect(tester.widget<Text>(banner).data, 'Held, then blocked (timeout)');

    final ReleaseValve allow = tester.widget<ReleaseValve>(
      find.byKey(const Key('intercept-allow')),
    );
    final BlockButton block = tester.widget<BlockButton>(
      find.byKey(const Key('intercept-block')),
    );
    expect(allow.enabled, isFalse);
    expect(block.enabled, isFalse);
  });

  testWidgets('the empty queue says so without a spinner', (
    WidgetTester tester,
  ) async {
    await pumpIntercept(tester, client: fake(const <ScriptedEvent>[]));
    await playScript(tester, const Duration(milliseconds: 200));

    expect(find.text('The queue is open'), findsOneWidget);
    expect(find.byType(QueueRow), findsNothing);
  });

  testWidgets('a decided row leaves the queue after three seconds', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fake(
      holdScript(<FlowDetail>[github(), pypi()]),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    expect(find.byType(QueueRow), findsNWidgets(2));

    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    // Die Zeile bleibt zunächst stehen, damit der Ausgang zu sehen ist.
    expect(find.byType(QueueRow), findsNWidgets(2));

    final FixedNow clock =
        containerOf(tester).read(nowProvider.notifier) as FixedNow;
    clock.moveTo(testStart.add(const Duration(seconds: 4)));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));

    expect(find.byType(QueueRow), findsOneWidget);
  });

  testWidgets('two hundred held flows build only what is on screen', (
    WidgetTester tester,
  ) async {
    final Map<FlowId, Flow> many = <FlowId, Flow>{
      for (int i = 1; i <= 200; i++)
        testFlowId(i): heldFlow(
          n: i,
          deadline: testStart.add(Duration(seconds: 60 + i)),
          // Eigene Domain je Flow: 200 flache Zeilen statt einer Gruppe.
          host: 'host$i.example$i.org',
        ),
    };
    await pumpIntercept(
      tester,
      client: fake(const <ScriptedEvent>[]),
      overrides: <Override>[flowsProvider.overrideWith(() => FixedFlows(many))],
    );
    await playScript(tester, const Duration(milliseconds: 100));

    // `AnimatedList` baut faul: sichtbare Zeilen plus Puffer, nie 200.
    final int built = find.byType(QueueRow).evaluate().length;
    expect(built, greaterThan(5));
    expect(built, lessThan(60));
    expect(tester.takeException(), isNull);
  });

  testWidgets('the splitter drags and the panes keep their minimum', (
    WidgetTester tester,
  ) async {
    await pumpIntercept(
      tester,
      client: fake(holdScript(<FlowDetail>[github()])),
    );
    await playScript(tester);

    final double before = tester.getRect(find.byType(QueuePane)).width;
    final List<double> ratiosBefore = containerOf(
      tester,
    ).read(paneRatiosProvider);
    await tester.dragFrom(
      Offset(tester.getRect(find.byType(QueuePane)).right + 3, 400),
      const Offset(60, 0),
    );
    await tester.pump();

    expect(tester.getRect(find.byType(QueuePane)).width, greaterThan(before));
    expect(containerOf(tester).read(paneRatiosProvider), isNot(ratiosBefore));
  });
}
