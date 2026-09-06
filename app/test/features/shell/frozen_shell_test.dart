// Was passiert, wenn die Verbindung mitten in der Arbeit abbricht (HUM-044,
// `docs/UX.md` 4.2, Fall 4).
//
// Der Setup-Bildschirm ersetzt die Shell nur beim Kaltstart. Bricht eine
// Verbindung, die stand, dann bleibt die Shell stehen, ein Banner nennt Grund,
// Folge und die eine Aktion, und alles darunter ist ein Schnappschuss: Die
// Countdowns stehen still, und keine Entscheidung geht mehr hinaus.
//
// Jeder Test hier ist gegen genau eine Umkehrung gerichtet:
//
// - `SetupHost()` zurück in den Zweig `ConnectionFrozen` des Gates: Die
//   Shell verschwindet, und `a_break_keeps_the_shell` wird rot.
// - `FrozenSections` weg, oder nur sein `nowProvider`-Override: Die Uhr, aus
//   der die Zeile rechnet, läuft weiter, und `the_countdown_stands_still` wird
//   rot. Der Test liest diese Uhr über `ProviderScope.containerOf` und nicht
//   nur die Beschriftung: Die steht ohnehin still, solange `TickerMode` den
//   Teilbaum anhält, und bewiese den Ersatz deshalb nicht.
// - `ExcludeFocus` weg: Die Warteschlange nimmt den Fokus wieder an, und
//   `the_frozen_queue_refuses_the_keyboard` wird rot. Der Beweis, dass die
//   Taste überhaupt etwas tut, steht im Test davor.
//   `nothing_is_decided_while_the_connection_is_down` bleibt davon grün, und
//   auch dann noch, wenn zusätzlich der Griff nach dem Fokus aus
//   `_linkChanged` fällt: Über der eingefrorenen Fläche stehen mehrere Riegel,
//   die dieselbe Taste abfangen. Der Test sagt deshalb, dass keine
//   Entscheidung hinausgeht, und beweist keinen einzelnen von ihnen.
// - Der `linkLiveProvider`-Horcher im Intercept-Screen weg, der nach dem
//   Wiederanschluss `_syncFocus` nachholt:
//   `the_queue_takes_the_keyboard_back_after_a_reconnect` wird rot, weil die
//   Tastatur nach dem Wiederanschluss bei der Shell hängen bleibt und keine
//   Entscheidung mehr in der Warteschlange ankommt. `initState` tut das nicht
//   mehr: Seit die Abschnitte einen `GlobalKey` tragen, überlebt ihr `State`
//   den Bruch.
// - Die `GlobalKey`s der fünf Abschnitte in `ShellScreen._sections` weg:
//   `a_break_keeps_the_state_of_the_sections` wird rot, weil `FrozenSections`
//   den Widget-Typ an jeder Stelle des Stapels wechselt und die Elemente
//   darunter samt ihrem `State` weggeworfen werden;
//   `a_break_does_not_re_admit_the_rows_the_pill_still_counts` wird ebenfalls
//   rot, weil das neue Pane in `initState` alles zulässt, was der Zähler der
//   Ankünfte noch als wartend führt.
// - `FrozenSections` wieder um den ganzen `IndexedStack` statt um die fünf
//   Abschnitte mit Daemon-Daten: `the_setup_section_stays_live` wird rot, weil
//   `Retry` kein zweites `GetInfo` mehr auslöst.
// - Der Riegel in `Flows._apply` weg: `a_held_that_arrives_after_the_break`
//   wird rot, weil die neue Zeile in den Schnappschuss läuft.
// - `visibleQueueFlows` liest wieder `nowProvider` statt `queueClockProvider`:
//   `a_decided_row_does_not_leave_the_frozen_queue` wird rot, weil die Zeile
//   drei Sekunden nach dem Bruch verschwindet.
// - Der `live`-Zweig in `status_bar.dart` weg: `the_status_bar_stops_saying`
//   wird rot, weil Punkt und Beschriftung „Connected" behalten.
// - Das `if (live)` vor `queue-allow-all` weg: `the_palette_drops_the_command`
//   wird rot; `_linkChanged` weg: `a_break_takes_back_the_batch_modal` wird
//   rot, weil das Modal stehen bleibt, und `the_setup_section_stays_live` wird
//   rot, weil ohne den Griff nach dem Fokus keine Taste der Shell mehr ankommt.
// - Der `linkLiveProvider`-Horcher in `Flows.build` weg: das Ende von
//   `a_held_that_arrives_after_the_break` wird rot, weil die während des
//   Bruchs verworfene Zeile nach dem Wiederanschluss fehlt.

import 'dart:async';

import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl_ui/humanitl_ui.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/intercept/intercept_screen.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/providers/now.dart';
import 'package:humanitl/features/intercept/widgets/batch_modal.dart';
import 'package:humanitl/features/intercept/widgets/countdown_ring.dart';
import 'package:humanitl/features/intercept/widgets/queue_pane.dart';
import 'package:humanitl/features/intercept/widgets/queue_row.dart';
import 'package:humanitl/features/setup/setup_screen.dart';
import 'package:humanitl/features/shell/shell_screen.dart';
import 'package:humanitl/features/shell/widgets/frozen_banner.dart';
import 'package:humanitl/features/shell/widgets/frozen_sections.dart';
import 'package:humanitl/features/shell/widgets/setup_host.dart';
import 'package:humanitl/features/shell/widgets/splash.dart';

import '../../harness/app_harness.dart';

/// Der Augenblick, an dem alles in diesen Tests hängt.
final DateTime testStart = DateTime.utc(2026, 9, 6, 12);

/// Die Uhr, die der Test selbst stellt.
///
/// Sie steht an der Stelle von `nowProvider`, damit „die Uhr steht still"
/// messbar ist: Eine Uhr, die im Test ohnehin kaum weiterläuft, bewiese nichts.
class TestClock extends Now {
  /// Beginnt bei [_at].
  TestClock(this._at);

  DateTime _at;

  @override
  DateTime build() => _at;

  /// Stellt die Uhr auf [at].
  void moveTo(DateTime at) {
    _at = at;
    state = at;
  }
}

/// Ein Fake mit drei gehaltenen Anfragen, alle auf [testStart] gestempelt.
///
/// Drei und nicht das mitgelieferte Skript: Was hier geprüft wird, ist die
/// Warteschlange mit ihren Countdowns, und `burst` füllt sie in 60 ms.
FakeDaemonClient fake() => FakeDaemonClient.burst(
  count: 3,
  spacing: const Duration(milliseconds: 20),
  clock: () => testStart,
);

/// Der Text des ersten Countdowns der Warteschlange.
String countdown(WidgetTester tester) => tester
    .widget<Text>(
      find
          .descendant(
            of: find.byType(CountdownLabel),
            matching: find.byType(Text),
          )
          .first,
    )
    .data!;

/// Die Uhr, die der eingefrorene Teilbaum tatsächlich liest.
///
/// `ProviderScope.containerOf` auf dem Element eines [CountdownLabel] trifft
/// den Behälter **unterhalb** des Ersatzes aus `FrozenSections`, und genau aus
/// ihm holt sich die Zeile die Sekunde, mit der sie rechnet. Der Blick auf den
/// Text allein genügte nicht: Solange `TickerMode` den Teilbaum anhält, steht
/// die Beschriftung auch dann still, wenn die Uhr darunter weiterliefe.
DateTime frozenClock(WidgetTester tester) =>
    ProviderScope.containerOf(tester.element(find.byType(CountdownLabel).first))
        .read(nowProvider);

/// Die Laufzeit, die die Statuszeile des Sandbox-Abschnitts zeigt.
String uptimeLabel(WidgetTester tester) =>
    tester.widget<Text>(find.byKey(const Key('sandbox-uptime'))).data!;

/// Die Uhr, aus der die Laufzeit rechnet.
///
/// Derselbe Griff wie [frozenClock], nur an der anderen Anzeige: Der Behälter
/// am Element dieser Beschriftung liegt **unterhalb** des Ersatzes aus
/// `FrozenSections`. Die Beschriftung allein bewiese den Ersatz nicht, denn
/// `TickerMode` pausiert im eingefrorenen Teilbaum ohnehin jedes `ref.watch`.
DateTime uptimeClock(WidgetTester tester) => ProviderScope.containerOf(
  tester.element(find.byKey(const Key('sandbox-uptime'))),
).read(nowProvider);

/// Der Fokusknoten, unter dem die Warteschlange liegt.
FocusNode queueFocus(WidgetTester tester) =>
    Focus.of(tester.element(find.byType(QueuePane)));

/// Baut die App, lässt das Skript bis zum ersten gehaltenen Flow laufen und
/// gibt die Uhr zurück, die der Test stellt.
Future<TestClock> pumpQueue(
  WidgetTester tester, {
  required FakeDaemonClient client,
  Duration heartbeat = const Duration(seconds: 1),
  Duration? reconnect,
}) async {
  final TestClock clock = TestClock(testStart);
  await pumpApp(
    tester,
    client: client,
    heartbeat: heartbeat,
    reconnect: reconnect,
    overrides: <Override>[nowProvider.overrideWith(() => clock)],
  );
  // Die drei Anfragen laufen in 60 ms ein.
  await tester.pump(const Duration(milliseconds: 100));
  await tester.pump();
  // Der Setup-Bildschirm bietet sich beim Start einmal an, weil ohne Sandbox
  // nicht alles grün ist. Hier geht es um die Warteschlange, also zurück zu
  // ihr: `Ctrl+1`, wie ein Mensch es täte. Die 400 ms danach liegen über
  // `HMotion.rearm`, also ist die Auswahl scharf.
  await pressCtrl(tester, LogicalKeyboardKey.digit1);
  await tester.pump(const Duration(milliseconds: 400));
  await tester.pump();
  return clock;
}

/// Nimmt dem Daemon die Antwort und lässt den Herzschlag es merken.
Future<void> breakConnection(
  WidgetTester tester,
  FakeDaemonClient client,
) async {
  client.goOffline();
  await tester.pump(const Duration(seconds: 1));
  await tester.pump();
}

/// Der Punkt der Statuszeile.
Finder get statusDot => find.byKey(const Key('status-connection-dot'));

/// Der Punkt, sofern er gerade [label] heißt.
Finder dotLabelled(String label) => find.byWidgetPredicate(
  (Widget widget) => widget is Semantics && widget.properties.label == label,
);

/// Die Farbe des Punktes.
Color dotColor(WidgetTester tester) =>
    (tester
                .widget<DecoratedBox>(
                  find.descendant(
                    of: statusDot,
                    matching: find.byType(DecoratedBox),
                  ),
                )
                .decoration
            as BoxDecoration)
        .color!;

/// Ein Fake, dessen Ereignisstrom den Daemon überlebt.
///
/// [FakeDaemonClient.goOffline] nimmt jedem Aufruf die Antwort, auch dem
/// Abspielen des Skripts: Der Strom endet dann mit einem Fehler, und ein
/// `Held` nach dem Bruch wäre gar nicht erst zu schicken. Genau der Fall muss
/// aber geprüft werden, denn die Warteschlange darf sich nicht darauf
/// verlassen, dass der Transport für sie aufräumt. Beim echten Daemon hängen
/// `GetInfo` und `Subscribe` an derselben Leitung, aber nicht aneinander: Der
/// Strom bringt seine eigene Wiederverbindungsleiter mit, und `GetInfo` kann
/// scheitern, während der Strom noch liefert.
class LingeringEvents extends FakeDaemonClient {
  /// Erzeugt einen Fake ohne Skript; die Ereignisse schickt der Test.
  LingeringEvents()
    : super(script: const <ScriptedEvent>[], clock: (() => testStart));

  final StreamController<FlowEvent> _events =
      StreamController<FlowEvent>.broadcast();

  @override
  Stream<FlowEvent> subscribe({
    FlowId? since,
    bool includePassthrough = false,
  }) => _events.stream;

  /// Hält die [n]-te Anfrage: erst empfangen, dann angehalten.
  void hold(int n) {
    final FlowId id = FlowId(
      '018f0044-0000-7000-8000-${n.toString().padLeft(12, '0')}',
    );
    final DateTime deadline = testStart.add(const Duration(minutes: 5));
    final Flow flow = Flow(
      id: id,
      sessionId: const SessionId('018f0044-0000-7000-8000-00000000000f'),
      receivedAt: testStart,
      method: Method.get,
      scheme: Scheme.https,
      authority: const Authority(host: 'api.github.com', port: 443),
      path: '/graphql',
      state: FlowState.held,
      deadline: deadline,
      heldAt: testStart,
    );
    state.flows[id] = flow;
    state.details[id] = FlowDetail(summary: flow);
    _events
      ..add(FlowEvent.received(at: testStart, flow: flow))
      ..add(FlowEvent.held(at: testStart, flowId: id, deadline: deadline));
  }

  @override
  Future<void> close() async {
    await _events.close();
    await super.close();
  }
}

void main() {
  testWidgets(
    'a_cold_start_without_a_daemon_shows_setup_instead_of_the_shell',
    (WidgetTester tester) async {
      await pumpApp(tester, client: FakeDaemonClient.unavailable());

      // Fall 2: Es gibt keine Shell, in die etwas passte.
      expect(find.byType(SetupScreen), findsOneWidget);
      expect(find.byType(ShellScreen), findsNothing);
      expect(find.byType(FrozenBanner), findsNothing);
    },
  );

  testWidgets('a_break_keeps_the_shell', (WidgetTester tester) async {
    final FakeDaemonClient client = fake();
    await pumpQueue(tester, client: client);
    expect(find.byType(ShellScreen), findsOneWidget);
    final int rows = tester.widgetList(find.byType(CountdownLabel)).length;
    expect(rows, greaterThan(0));

    await breakConnection(tester, client);

    // Die Shell steht, die Warteschlange steht darin, und das Banner sagt
    // Grund, Folge und die eine Aktion.
    expect(find.byType(ShellScreen), findsOneWidget);
    expect(find.byType(CountdownLabel), findsNWidgets(rows));
    expect(find.byType(FrozenBanner), findsOneWidget);
    expect(find.byKey(const Key('shell-frozen-title')), findsOneWidget);
    expect(find.byKey(const Key('shell-frozen-consequence')), findsOneWidget);
    expect(find.text('Reconnect'), findsOneWidget);
    // Und der Grund steht dort, nicht nur ein Zustand.
    expect(find.byKey(const Key('shell-frozen-why')), findsOneWidget);
  });

  /// Ein Bruch nimmt der Shell nichts weg, was in ihr steht.
  ///
  /// `FrozenSections` kommt bei einem Bruch über jeden der fünf Abschnitte
  /// und wechselt damit an jeder Stelle des `IndexedStack` den Widget-Typ.
  /// Ohne den `GlobalKey` je Abschnitt würfe Flutter das Element darunter weg
  /// und baute es neu, und mit ihm stürbe jeder `State` -- die eingefrorene
  /// Reihenfolge der Warteschlange, ihre Scrollposition, die Zeigeranwesenheit
  /// (`docs/UX.md` 7).
  testWidgets('a_break_keeps_the_state_of_the_sections', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fake();
    await pumpQueue(tester, client: client);
    final State<QueuePane> pane = tester.state<State<QueuePane>>(
      find.byType(QueuePane),
    );
    final State<InterceptScreen> screen = tester.state<State<InterceptScreen>>(
      find.byType(InterceptScreen),
    );

    await breakConnection(tester, client);

    expect(
      identical(tester.state<State<QueuePane>>(find.byType(QueuePane)), pane),
      isTrue,
      reason: 'the queue pane keeps its state across a break',
    );
    expect(
      identical(
        tester.state<State<InterceptScreen>>(find.byType(InterceptScreen)),
        screen,
      ),
      isTrue,
      reason: 'the intercept screen keeps its state across a break',
    );
  });

  /// Und weil der `State` steht, bleibt auch die Pille wahr.
  ///
  /// Der Zähler der ausstehenden Ankünfte ist ein Provider und überlebt einen
  /// Bruch ohnehin; die Warteliste, auf die er sich bezieht, liegt im `State`
  /// des Panes. Stirbt der `State`, dann lässt `QueuePane.initState` jede
  /// Zeile zu, die es gibt -- und über den bereits gezeichneten Zeilen stünde
  /// „+1 new" mit einem Klick, der nichts mehr zusammenzuführen hat
  /// (`docs/UX.md` 2.8 und 5.3).
  testWidgets('a_break_does_not_re_admit_the_rows_the_pill_still_counts', (
    WidgetTester tester,
  ) async {
    final LingeringEvents client = LingeringEvents();
    await pumpQueue(tester, client: client);
    client.hold(1);
    await tester.pump();
    await tester.pump();
    expect(find.byType(QueueRow), findsOneWidget);

    // Der Zeiger steht im Pane, also friert die Reihenfolge ein und die
    // zweite Anfrage wartet draußen.
    final TestGesture pointer = await tester.createGesture(
      kind: PointerDeviceKind.mouse,
    );
    await pointer.addPointer(location: Offset.zero);
    addTearDown(pointer.removePointer);
    await pointer.moveTo(tester.getCenter(find.byType(QueuePane)));
    await tester.pump();
    client.hold(2);
    await tester.pump();
    await tester.pump();
    expect(find.byType(QueueRow), findsOneWidget);
    expect(find.text('+1 new'), findsOneWidget);

    await breakConnection(tester, client);

    // Der Bruch lässt beides, wie es war: eine Zeile und eine wartende.
    expect(
      find.byType(QueueRow),
      findsOneWidget,
      reason: 'the frozen order survives the break',
    );
    expect(find.text('+1 new'), findsOneWidget);
  });

  testWidgets('the_countdown_stands_still_while_the_connection_is_down', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fake();
    final TestClock clock = await pumpQueue(tester, client: client);

    // Solange die Verbindung steht, folgt der Countdown der Uhr.
    final String atStart = countdown(tester);
    clock.moveTo(testStart.add(const Duration(seconds: 30)));
    await tester.pump();
    final String after30s = countdown(tester);
    expect(after30s, isNot(atStart));

    await breakConnection(tester, client);
    expect(countdown(tester), after30s);
    // Die Uhr, aus der die Zeile rechnet, steht seit dem Bruch auf dessen
    // Sekunde. Ohne diesen Blick prüfte der Test nur die Beschriftung, und die
    // steht ohnehin still, solange `TickerMode` den Teilbaum anhält.
    final DateTime stopped = frozenClock(tester);
    expect(stopped, testStart.add(const Duration(seconds: 30)));

    // Danach bewegt die Uhr des Programms den Countdown nicht mehr: Der
    // Daemon, der um 05:00 blockieren würde, ist weg.
    clock.moveTo(testStart.add(const Duration(seconds: 90)));
    await tester.pump();
    expect(frozenClock(tester), stopped);
    expect(countdown(tester), after30s);
  });

  testWidgets('enter_decides_while_the_connection_stands', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fake();
    await pumpQueue(tester, client: client);

    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();

    // Ohne diese Zusage bewiese der Test danach nichts.
    expect(client.decisions, hasLength(1));
  });

  testWidgets('nothing_is_decided_while_the_connection_is_down', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fake();
    await pumpQueue(tester, client: client);
    await breakConnection(tester, client);

    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    await tester.pump();

    expect(client.decisions, isEmpty);
    // Und die Taste ist nicht bloß erfolglos, sie kommt gar nicht erst an:
    // Ein Versuch gegen den weggefallenen Daemon endete in der Fehlerkarte
    // der Aktionsleiste. Sie fehlt, weil über der eingefrorenen Fläche
    // mehrere Riegel dieselbe Taste abfangen; welcher davon hält, sagt dieser
    // Test nicht, sondern der darunter.
    expect(find.byKey(const Key('intercept-decision-error')), findsNothing);
  });

  /// Der zweite Riegel, und der Test dafür, dass er einer ist: Der Fokus wird
  /// beim Bruch weggezogen, aber nichts hindert einen Klick, eine Tabulatur
  /// oder ein Widget daran, ihn wieder anzufordern. `ExcludeFocus` nimmt der
  /// eingefrorenen Fläche die Tastatur, also läuft diese Anforderung ins Leere
  /// und die Taste bleibt bei der Shell.
  testWidgets('the_frozen_queue_refuses_the_keyboard', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fake();
    await pumpQueue(tester, client: client);
    // Vorher nimmt sie ihn an; ohne diese Zusage bewiese der Test nichts.
    expect(queueFocus(tester).hasFocus, isTrue);

    await breakConnection(tester, client);

    final FocusNode frozen = queueFocus(tester);
    frozen.requestFocus();
    await tester.pump();
    expect(frozen.hasFocus, isFalse);

    // Und deshalb entscheidet die Eingabetaste auch danach nichts, und die
    // Aktionsleiste zeigt keinen gescheiterten Versuch.
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    await tester.pump();
    expect(client.decisions, isEmpty);
    expect(find.byKey(const Key('intercept-decision-error')), findsNothing);
  });

  testWidgets('the_queue_takes_the_keyboard_back_after_a_reconnect', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fake();
    await pumpQueue(tester, client: client);
    await breakConnection(tester, client);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(client.decisions, isEmpty);

    client.goOnline();
    await tester.tap(find.byKey(const Key('shell-frozen-reconnect')));
    await tester.pump();
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));
    await tester.pump();

    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(client.decisions, hasLength(1));
  });

  /// Auch die Laufzeit der Sandbox steht still. Sie ist eine Uhr wie jede
  /// andere, und ein Bild, das als Schnappschuss beschriftet ist, darf keine
  /// Ziffer weiterzaehlen (`docs/UX.md` 4.2, Fall 4).
  ///
  /// Der Test misst den **gezeichneten Text** und in beide Richtungen. Die
  /// erste Haelfte -- die Beschriftung folgt der einen Uhr des Programms,
  /// solange die Verbindung lebt -- ist die, die ein privater
  /// `Timer.periodic` mit `DateTime.now()` nicht bestehen kann: Der laeuft in
  /// einem Widget-Test an der gestellten Uhr vorbei. Die zweite Haelfte faellt
  /// mit dem `nowProvider`-Ersatz aus `FrozenSections`. Seit die fuenf
  /// Abschnitte einen `GlobalKey` tragen, wird der Abschnitt beim Bruch
  /// umgehaengt statt zerstoert, also ueberlebte ein eigener Timer den Bruch.
  testWidgets('the_sandbox_uptime_stands_still', (WidgetTester tester) async {
    final FakeDaemonClient client = fake();
    client.sandbox = client.sandbox.copyWith(
      state: SandboxState.running,
      agentRunning: true,
      sandboxId: FakeDaemonClient.defaultSandbox,
      startedAt: testStart.subtract(const Duration(seconds: 30)),
    );
    final TestClock clock = await pumpQueue(tester, client: client);
    await pressCtrl(tester, LogicalKeyboardKey.digit4);
    await tester.pumpAndSettle();

    final String atStart = uptimeLabel(tester);
    clock.moveTo(testStart.add(const Duration(seconds: 20)));
    await tester.pump();
    final String live = uptimeLabel(tester);
    expect(
      live,
      isNot(atStart),
      reason: 'the label counts on the one clock of the program',
    );

    await breakConnection(tester, client);
    final String frozen = uptimeLabel(tester);
    clock.moveTo(testStart.add(const Duration(minutes: 5)));
    await tester.pump();
    await tester.pump();

    expect(
      uptimeLabel(tester),
      frozen,
      reason: 'nothing counts on in a picture the banner calls a snapshot',
    );
    expect(uptimeLabel(tester), live);
    // Und die Beschriftung steht still, weil die Uhr unter ihr steht, nicht
    // nur weil `TickerMode` den Teilbaum angehalten hat: Der Behälter an
    // dieser Stelle hält den Augenblick des Bruchs fest, während die Uhr
    // darüber schon fünf Minuten weiter ist.
    expect(uptimeClock(tester), testStart.add(const Duration(seconds: 20)));
  });

  testWidgets('a_retry_of_a_broken_connection_shows_no_splash', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fake();
    await pumpQueue(
      tester,
      client: client,
      reconnect: const Duration(seconds: 2),
    );
    await breakConnection(tester, client);
    expect(find.byType(ShellScreen), findsOneWidget);

    // Der Zwei-Sekunden-Takt versucht es wieder. Der Splash gehört dem ersten
    // Start; hier nähme er alle zwei Sekunden den Bildschirm weg.
    await tester.pump(const Duration(seconds: 2));
    await tester.pump();
    expect(find.byType(Splash), findsNothing);
    expect(find.byType(ShellScreen), findsOneWidget);
    expect(find.byType(FrozenBanner), findsOneWidget);
  });

  testWidgets('reconnect_from_the_banner_thaws_the_queue', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fake();
    final TestClock clock = await pumpQueue(tester, client: client);
    await breakConnection(tester, client);
    final String frozen = countdown(tester);

    client.goOnline();
    await tester.tap(find.byKey(const Key('shell-frozen-reconnect')));
    await tester.pump();
    await tester.pump();

    expect(find.byType(FrozenBanner), findsNothing);
    expect(find.byType(ShellScreen), findsOneWidget);
    // Die Uhr läuft wieder mit dem Programm.
    clock.moveTo(testStart.add(const Duration(seconds: 120)));
    await tester.pump();
    expect(countdown(tester), isNot(frozen));
  });

  testWidgets('the_setup_section_stays_live_while_the_connection_is_down', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fake();
    await pumpQueue(tester, client: client);
    await breakConnection(tester, client);

    // Der Schnappschuss liegt über dem Abschnitt mit den Daten des Daemons.
    expect(
      find.ancestor(
        of: find.byType(InterceptScreen),
        matching: find.byType(FrozenSections),
      ),
      findsOneWidget,
    );

    // Zum Setup-Abschnitt, wie ein Mensch es täte. Dass `Ctrl+6` das noch tut,
    // ist selbst eine Aussage: Der Fokus sass in der Warteschlange, und die
    // liegt jetzt unter `ExcludeFocus`.
    await pressCtrl(tester, LogicalKeyboardKey.digit6);
    await tester.pump(const Duration(milliseconds: 400));
    await tester.pump();
    expect(find.byType(SetupScreen), findsOneWidget);

    // Und über diesem Abschnitt liegt er nicht.
    expect(
      find.ancestor(
        of: find.byType(SetupHost),
        matching: find.byType(FrozenSections),
      ),
      findsNothing,
    );

    // Und der Knopf tut etwas: `Retry` fragt noch einmal nach `GetInfo`.
    final int before = client.infoCalls;
    await tester.tap(find.byKey(const Key('setup-daemon-retry')));
    await tester.pump();
    await tester.pump();
    expect(client.infoCalls, greaterThan(before));
  });

  testWidgets('a_held_that_arrives_after_the_break_never_enters_the_queue', (
    WidgetTester tester,
  ) async {
    final LingeringEvents client = LingeringEvents();
    await pumpApp(
      tester,
      client: client,
      heartbeat: const Duration(seconds: 1),
      overrides: <Override>[
        nowProvider.overrideWith(() => TestClock(testStart)),
      ],
    );
    // Die Neusynchronisation der ersten Verbindung ist durch, bevor die erste
    // Anfrage kommt; sonst räumte ihre Antwort die Zeile wieder weg.
    await tester.pump(const Duration(milliseconds: 50));
    await tester.pump();

    client.hold(1);
    await tester.pump();
    await tester.pump();

    final ProviderContainer container = ProviderScope.containerOf(
      tester.element(find.byType(ShellScreen)),
    );
    List<FlowId> rows() => <FlowId>[
      for (final Flow flow in container.read(visibleQueueFlowsProvider).flows)
        flow.id,
    ];
    expect(rows(), hasLength(1));

    await breakConnection(tester, client);
    final List<FlowId> frozenRows = rows();

    // Der Ereignisstrom lebt weiter, obwohl `GetInfo` nicht mehr antwortet.
    // Genau darauf darf sich die Warteschlange nicht verlassen.
    client.hold(2);
    await tester.pump();
    await tester.pump();

    expect(rows(), frozenRows);
    expect(container.read(flowsProvider), hasLength(1));

    // Und was der Schnappschuss verworfen hat, ist nicht verloren: Sobald die
    // Verbindung steht, holt `ListFlows` die gehaltenen Anfragen -- hier auch
    // die, die während des Bruchs kam. Ohne diesen Weg bliebe sie weg, denn
    // der Strom brach nie und schickte deshalb nie ein `Lagged`.
    client.goOnline();
    await tester.tap(find.byKey(const Key('shell-frozen-reconnect')));
    await tester.pump();
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 50));
    await tester.pump();

    expect(rows(), hasLength(2));
  });

  testWidgets('a_decided_row_does_not_leave_the_frozen_queue', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fake();
    final TestClock clock = await pumpQueue(tester, client: client);

    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    await tester.pump();
    expect(client.decisions, hasLength(1));

    await breakConnection(tester, client);
    final ProviderContainer container = ProviderScope.containerOf(
      tester.element(find.byType(ShellScreen)),
    );
    final QueueSnapshot frozen = container.read(visibleQueueFlowsProvider);
    expect(
      frozen.flows.any((Flow flow) => flow.decidedAt != null),
      isTrue,
      reason: 'ohne die entschiedene Zeile bewiese der Test nichts',
    );

    // Die Uhr des Programms läuft weiter; die der Warteschlange nicht. Ohne
    // das räumte `queueExitWindow` die entschiedene Zeile aus dem
    // Schnappschuss, während ihr Countdown daneben stillsteht.
    clock.moveTo(testStart.add(queueExitWindow * 3));
    await tester.pump();
    await tester.pump();

    expect(container.read(visibleQueueFlowsProvider), frozen);
  });

  testWidgets('the_status_bar_stops_saying_connected', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fake();
    await pumpQueue(tester, client: client);
    expect(dotLabelled('Connected'), findsOneWidget);
    expect(dotLabelled('Not connected'), findsNothing);
    final Color connected = dotColor(tester);

    await breakConnection(tester, client);

    expect(dotLabelled('Connected'), findsNothing);
    expect(dotLabelled('Not connected'), findsOneWidget);
    expect(dotColor(tester), isNot(connected));
  });

  /// Dieselbe Luege wie beim Punkt und beim Ring, eine Zeile daneben: Ohne
  /// Daemon faengt niemand etwas ab, und eine gruene Pille „Intercept ON"
  /// behauptet dann das Gegenteil.
  testWidgets('the_header_pill_stops_claiming_it_intercepts', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fake();
    await pumpQueue(tester, client: client);
    expect(find.text('Intercept ON'), findsOneWidget);
    final Color? on = tester
        .widget<HBadge>(find.byKey(const Key('header-intercept-badge')))
        .color;

    await breakConnection(tester, client);

    expect(find.text('Intercept ON'), findsNothing);
    expect(find.text('Intercept: no connection'), findsOneWidget);
    expect(
      tester
          .widget<HBadge>(find.byKey(const Key('header-intercept-badge')))
          .color,
      isNot(on),
    );
  });

  testWidgets('the_palette_drops_the_command_that_touches_the_queue', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fake();
    await pumpQueue(tester, client: client);

    await pressCtrl(tester, LogicalKeyboardKey.keyK);
    await tester.pump(const Duration(milliseconds: 300));
    expect(find.byKey(const Key('palette-queue-allow-all')), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump(const Duration(milliseconds: 300));

    await breakConnection(tester, client);

    await pressCtrl(tester, LogicalKeyboardKey.keyK);
    await tester.pump(const Duration(milliseconds: 300));
    // Der Befehl, der hilft, bleibt.
    expect(find.byKey(const Key('palette-reconnect')), findsOneWidget);
    // Der Befehl, dessen Modal im eingefrorenen Teil des Baums stünde, nicht.
    expect(find.byKey(const Key('palette-queue-allow-all')), findsNothing);
  });

  testWidgets('a_break_takes_back_the_batch_modal_it_would_strand', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fake();
    await pumpQueue(tester, client: client);

    await pressCtrl(tester, LogicalKeyboardKey.keyK);
    await tester.pump(const Duration(milliseconds: 300));
    await tester.tap(find.byKey(const Key('palette-queue-allow-all')));
    await tester.pump(const Duration(milliseconds: 300));
    await tester.pump();
    expect(find.byType(BatchModal), findsOneWidget);

    await breakConnection(tester, client);

    // Bestätigen, Schirm und `Escape` wären still; das Modal bliebe stehen,
    // bis die Verbindung zurückkommt.
    expect(find.byType(BatchModal), findsNothing);
    expect(client.decisions, isEmpty);
  });
}
