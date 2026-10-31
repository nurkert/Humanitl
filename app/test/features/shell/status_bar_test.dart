// Die Auslastung der Warteschlange in der Statusleiste (HUM-058, Zeile 16,
// übernommen von HUM-057): unter 80 % nichts, ab 80 % Warnfarbe, ab 100 % rot mit
// `LIMIT_002`. Dazu die Störung der Aufzeichnung (HUM-250).
// And the notice for a daemon of another version than the app (HUM-277).

import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/connection.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ipc/flow_events.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/shell/daemon_restart.dart';
import 'package:humanitl/features/shell/queue_pressure.dart';
import 'package:humanitl/features/sandbox/providers/sandbox_status_provider.dart';
import 'package:humanitl/features/shell/widgets/status_bar.dart';
import 'package:humanitl/l10n/l10n.dart';

import '../intercept/fixtures.dart';
import '../intercept/harness.dart' as harness;

/// Die Grenzen dieser Tests: zehn Flüsse, eine Million Bytes.
const ConfigValues _limits = ConfigValues(
  values: <String, Object?>{
    'limits': <String, Object?>{
      'hold_max_flows': 10,
      'hold_max_bytes': 1000000,
    },
  },
);

/// [count] gehaltene Flüsse mit je [size] Bytes.
List<Flow> _held(int count, {int size = 0}) => <Flow>[
  for (int n = 0; n < count; n++)
    heldFlow(
      n: n,
      deadline: testStart.add(const Duration(minutes: 5)),
      requestSize: size,
    ),
];

/// Ob die Verbindung lebt, im Test von Hand gestellt.
class _Link extends Notifier<bool> {
  @override
  bool build() => true;

  void set(bool live) => state = live;
}

final NotifierProvider<_Link, bool> _linkProvider =
    NotifierProvider<_Link, bool>(_Link.new);

/// Die Belegung des Byte-Budgets, wie der Daemon sie zuletzt meldete.
class _FixedPoolBytes extends DaemonQueueBytes {
  _FixedPoolBytes(this.bytes);

  final int bytes;

  @override
  int build() => bytes;
}

Future<void> _pump(
  WidgetTester tester,
  List<Flow> held, {
  int poolBytes = 0,
  Widget child = const Center(child: QueuePressureLabel()),
}) async {
  final FakeDaemonClient client = FakeDaemonClient.empty()
    ..configValues = _limits;
  await tester.pumpWidget(
    ProviderScope(
      // Ein frischer Container je Fall: Eine geänderte `overrideWith`-Fabrik
      // übernimmt ein bestehender nicht.
      key: UniqueKey(),
      overrides: [
        daemonClientProvider.overrideWithValue(client),
        heldFlowsProvider.overrideWithValue(held),
        daemonQueueBytesProvider.overrideWith(() => _FixedPoolBytes(poolBytes)),
      ],
      child: WidgetsApp(
        color: HColors.bg0,
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        builder: (BuildContext context, Widget? _) =>
            HTheme(tokens: HTokens.dark, child: child),
      ),
    ),
  );
  // `GetConfig` antwortet asynchron; danach steht die Grenze fest.
  await tester.pump();
  await tester.pump();
}

Color? _colorOf(WidgetTester tester) => tester
    .widget<Text>(find.byKey(const Key('status-queue-pressure')))
    .style
    ?.color;

void main() {
  testWidgets('queue_pressure_colors', (WidgetTester tester) async {
    final HTokens tokens = HTokens.dark;

    // 7 von 10: unter 80 %, die Leiste schweigt.
    await _pump(tester, _held(7));
    expect(find.byKey(const Key('status-queue-pressure')), findsNothing);

    // 8 von 10: ab 80 % Warnfarbe, mit Zahl, Grenze und der Belegung des Daemons.
    await _pump(tester, _held(8), poolBytes: 8000);
    expect(find.text('Queue 8/10 · 8.0 kB'), findsOneWidget);
    expect(_colorOf(tester), tokens.colors.warningText);
    expect(find.byKey(const Key('status-queue-limit')), findsNothing);

    // 10 von 10: voll, rot, mit `LIMIT_002` und dem Titel aus ARB.
    await _pump(tester, _held(10));
    expect(_colorOf(tester), tokens.stateTextColor(HFlowState.blocked));
    expect(find.byKey(const Key('status-queue-limit')), findsOneWidget);
    expect(find.text('LIMIT_002'), findsOneWidget);
    expect(find.text('Queue full'), findsOneWidget);

    // Auch die Bytes zählen, und zwar die des Daemons: 850 kB von 1 MB im
    // Budget tragen die Warnfarbe, auch wenn die gehaltenen Flüsse wenig ankündigen
    // (gerade gepufferte Rümpfe anderer Anfragen zählen mit).
    await _pump(tester, _held(1), poolBytes: 850000);
    expect(_colorOf(tester), tokens.colors.warningText);

    // Ein gehaltener Stream-Kopf kündigt viel an und belegt fast nichts: kein
    // Warnfarbe, kein Rot (HUM-057, Review 2).
    await _pump(tester, _held(1, size: 900000), poolBytes: 1000);
    expect(find.byKey(const Key('status-queue-pressure')), findsNothing);
  });

  group('QueueLoad.pressure', () {
    QueueLoad load({int count = 0, int bytes = 0}) =>
        QueueLoad(count: count, bytes: bytes, maxFlows: 500, maxBytes: 1000);

    test('the fuller of the two limits decides, from 80 % on', () {
      expect(load(count: 399).pressure, QueuePressure.normal);
      expect(load(count: 400).pressure, QueuePressure.high);
      expect(load(count: 499).pressure, QueuePressure.high);
      expect(load(count: 500).pressure, QueuePressure.full);
      expect(load(bytes: 799).pressure, QueuePressure.normal);
      expect(load(bytes: 800).pressure, QueuePressure.high);
      expect(load(bytes: 1000).pressure, QueuePressure.full);
      expect(load(count: 10, bytes: 1000).pressure, QueuePressure.full);
    });

    test('without limits from the daemon the defaults of the daemon apply', () {
      final QueueLoad empty = QueueLoad.of(
        const <Flow>[],
        ConfigValues.empty,
        4096,
      );
      expect(empty.maxFlows, defaultHoldMaxFlows);
      expect(empty.maxBytes, defaultHoldMaxBytes);
      expect(empty.bytes, 0, reason: 'without held flows the value is stale');
      final QueueLoad configured = QueueLoad.of(
        _held(2, size: 5),
        _limits,
        4096,
      );
      expect(configured.maxFlows, 10);
      expect(configured.maxBytes, 1000000);
      expect(configured.count, 2);
      expect(configured.bytes, 4096, reason: 'the daemon counts, not the app');
    });
  });

  test('the occupancy follows the last Held event of the daemon', () async {
    final ProviderContainer container = ProviderContainer(
      overrides: [
        linkLiveProvider.overrideWith((Ref ref) => ref.watch(_linkProvider)),
        flowEventsProvider.overrideWith(
          (Ref ref) => Stream<FlowEvent>.fromIterable(<FlowEvent>[
            FlowEvent.held(
              at: testStart,
              flowId: testFlowId(1),
              deadline: testStart.add(const Duration(minutes: 5)),
              queueBytes: 123456,
              queueCount: 1,
            ),
          ]),
        ),
      ],
    );
    addTearDown(container.dispose);
    final ProviderSubscription<int> subscription = container.listen(
      daemonQueueBytesProvider,
      (int? previous, int next) {},
    );
    addTearDown(subscription.close);
    expect(container.read(daemonQueueBytesProvider), 0);
    await container.read(flowEventsProvider.future);
    await Future<void>.delayed(Duration.zero);
    expect(container.read(daemonQueueBytesProvider), 123456);

    // Bricht die Verbindung ab, gilt die alte Zahl nicht mehr (Review 3).
    container.read(_linkProvider.notifier).set(false);
    expect(container.read(daemonQueueBytesProvider), 0);
  });

  testWidgets('the status bar itself shows a full queue', (
    WidgetTester tester,
  ) async {
    // Die Leiste baut das Etikett ein; ohne diesen Fall stünde die Zahl nur im
    // Test des Etiketts und nirgends auf dem Schirm.
    await _pump(
      tester,
      _held(10),
      child: const Align(
        alignment: Alignment.bottomCenter,
        child: StatusBar(
          info: DaemonInfo(
            daemonVersion: '0.0.0',
            protoMajor: 1,
            protoMinor: 0,
          ),
        ),
      ),
    );
    expect(
      find.descendant(
        of: find.byType(StatusBar),
        matching: find.byKey(const Key('status-queue-limit')),
      ),
      findsOneWidget,
    );
    expect(find.text('Queue 10/10 · 0 B'), findsOneWidget);
  });

  // HUM-250: Meldet der Daemon, dass die Aufzeichnung nicht schreibt, sagt die
  // Statusleiste es, jede Karte trägt den Hinweis, und die erste Freigabe der
  // Sitzung hält an einer Pause. Mit `RECORDER_005` verschwinden die
  // Hinweise; die Bestätigung gilt für die Sitzung, nicht für die Störung.
  testWidgets('recorder_failure_marks_cards', (WidgetTester tester) async {
    const Diagnostic failure = Diagnostic(
      code: 'RECORDER_003',
      severity: Severity.error,
      title: 'Aufzeichnung konnte nicht schreiben',
      why: 'could not commit a recorder transaction (database or disk is full)',
    );
    const Diagnostic recovered = Diagnostic(
      code: 'RECORDER_005',
      severity: Severity.info,
      title: 'Aufzeichnung läuft wieder',
      why: 'the recording writes again',
    );
    final FakeDaemonClient client = harness.fakeDaemon(<ScriptedEvent>[
      ScriptedEvent(
        Duration.zero,
        (FakeSessionState state, DateTime now) =>
            FlowEvent.diagnostic(at: now, diagnostic: failure),
      ),
      ...holdScript(<FlowDetail>[
        harness.held(1),
        harness.held(2, host: 'api.github.com', apex: 'github.com'),
        harness.held(3, host: 'pypi.org', apex: 'pypi.org'),
      ]),
      ScriptedEvent(
        const Duration(seconds: 5),
        (FakeSessionState state, DateTime now) =>
            FlowEvent.diagnostic(at: now, diagnostic: recovered),
      ),
    ]);
    await harness.pumpIntercept(tester, client: client);
    await harness.playScript(tester);

    const Key notRecorded = Key('intercept-card-not-recorded');
    const Key pause = Key('intercept-recorder-pause');
    expect(find.byKey(const Key('status-recorder-disrupted')), findsOneWidget);
    expect(find.text('Recording disrupted'), findsOneWidget);
    expect(find.text('RECORDER_003'), findsWidgets);
    expect(find.byKey(notRecorded), findsOneWidget);

    // Die erste Freigabe geht nicht hinaus, sondern hält an der Pause.
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(find.byKey(pause), findsOneWidget);
    expect(client.decisions, isEmpty);

    // Ein `S` sofort danach überspringt sie nicht: Der Knopf nimmt erst nach
    // `HMotion.rearm` an.
    await tester.sendKeyEvent(LogicalKeyboardKey.keyS);
    await tester.pump();
    expect(client.decisions, isEmpty);
    expect(find.byKey(pause), findsOneWidget);

    await tester.pump(HMotion.rearm);
    await tester.pump();
    await tester.tap(find.byKey(const Key('intercept-recorder-pause-send')));
    await tester.pump();
    await tester.pump();
    expect(client.decisions, hasLength(1));
    expect(client.decisions.single.flowId, testFlowId(1));

    // Die nächste Anfrage trägt den Hinweis auch, und ihre Freigabe geht ohne
    // zweite Pause: einmal je Sitzung.
    await tester.pump(const Duration(milliseconds: 600));
    await tester.pump();
    expect(find.byKey(notRecorded), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    await tester.pump();
    expect(find.byKey(pause), findsNothing);
    expect(client.decisions, hasLength(2));

    // `RECORDER_005`: Statusleiste und Hinweis sind weg.
    await tester.pump(const Duration(seconds: 5));
    await tester.pump();
    await tester.pump();
    expect(find.byKey(const Key('status-recorder-disrupted')), findsNothing);
    expect(find.byKey(notRecorded), findsNothing);
  });

  // HUM-277: After a package upgrade the old user daemon keeps running, and
  // restarting the app does not restart it. The status bar says so next to the
  // daemon version, with the command that restarts it.
  testWidgets('daemon_version_mismatch_notice', (WidgetTester tester) async {
    Future<void> pumpBar({
      required String daemon,
      required String app,
      List<String> capabilities = const <String>[],
    }) async {
      await _pump(
        tester,
        const <Flow>[],
        child: Align(
          alignment: Alignment.bottomCenter,
          child: StatusBar(
            info: DaemonInfo(
              daemonVersion: daemon,
              protoMajor: 1,
              protoMinor: 17,
              capabilities: capabilities,
            ),
            appVersion: app,
          ),
        ),
      );
    }

    const Key notice = Key('status-daemon-mismatch');

    await pumpBar(daemon: '0.0.5', app: '0.0.6');
    expect(find.byKey(notice), findsOneWidget);
    expect(find.text('Not the version of this app (0.0.6)'), findsOneWidget);
    expect(
      tester.widget<Text>(find.byKey(notice)).style?.color,
      HTokens.dark.colors.warningText,
    );

    // The same build: nothing to say.
    await pumpBar(daemon: '0.0.6', app: '0.0.6');
    expect(find.byKey(notice), findsNothing);

    // A build without a version of its own cannot tell.
    await pumpBar(daemon: '0.0.5', app: '');
    expect(find.byKey(notice), findsNothing);

    // A development build (0.0.0) on either side is no upgrade to report.
    await pumpBar(daemon: '0.0.0', app: '0.0.6');
    expect(find.byKey(notice), findsNothing);
    await pumpBar(daemon: '0.0.5', app: '0.0.0');
    expect(find.byKey(notice), findsNothing);

    // A daemon that reports no version is unknown, not different.
    await pumpBar(daemon: '', app: '0.0.6');
    expect(find.byKey(notice), findsNothing);

    // The fake daemon is no build at all.
    await pumpBar(
      daemon: '0.0.0-fake',
      app: '0.0.6',
      capabilities: const <String>[DaemonInfo.fakeCapability],
    );
    expect(find.byKey(notice), findsNothing);
  });

  // HUM-284: next to the notice a button restarts the daemon for this user
  // only: `systemctl --user daemon-reload`, then the daemon decides itself
  // whether a session holds it (`humanitl daemon retire-if-idle`), and only
  // after `retired` does `systemctl --user restart humanitld.service` run. A
  // daemon older than contract 1.19 is asked `Sandbox(Status)` instead.
  group('daemon_restart_button', () {
    const Key button = Key('status-daemon-restart');
    const Key blocked = Key('status-daemon-restart-blocked');
    const Key failed = Key('status-daemon-restart-failed');
    const String cli = '/usr/lib/humanitl/bin/humanitl';
    const List<String> retire = <String>[
      cli,
      '--json',
      'daemon',
      'retire-if-idle',
    ];

    /// The status bar of an app 0.0.6 over a daemon 0.0.5 that speaks
    /// [minor], with [client] and a runner that writes down what it was asked
    /// to run. `retire-if-idle` answers [outcome]; `daemon-reload` ends with
    /// [reloadExit].
    Future<List<List<String>>> pumpRestart(
      WidgetTester tester,
      FakeDaemonClient client, {
      int minor = daemonRetireMinor,
      String outcome = 'retired',
      int reloadExit = 0,
      Locale locale = const Locale('en'),
      String? cliPath = cli,
    }) async {
      final List<List<String>> ran = <List<String>>[];
      Future<ProcessResult> run(String executable, List<String> arguments) {
        final List<String> command = <String>[executable, ...arguments];
        ran.add(command);
        if (executable == cli) {
          return Future<ProcessResult>.value(
            ProcessResult(1, 0, '{"outcome":"$outcome"}\n', ''),
          );
        }
        final int code = arguments.contains('daemon-reload') ? reloadExit : 0;
        return Future<ProcessResult>.value(
          ProcessResult(1, code, '', code == 0 ? '' : 'no bus'),
        );
      }

      await tester.pumpWidget(
        ProviderScope(
          key: UniqueKey(),
          overrides: [
            daemonClientProvider.overrideWithValue(client),
            heldFlowsProvider.overrideWithValue(const <Flow>[]),
            daemonRestartRunnerProvider.overrideWithValue(run),
            daemonCliProvider.overrideWithValue(cliPath),
          ],
          child: WidgetsApp(
            color: HColors.bg0,
            locale: locale,
            localizationsDelegates: AppLocalizations.localizationsDelegates,
            supportedLocales: AppLocalizations.supportedLocales,
            builder: (BuildContext context, Widget? _) => HTheme(
              tokens: HTokens.dark,
              child: Align(
                alignment: Alignment.bottomCenter,
                child: StatusBar(
                  info: DaemonInfo(
                    daemonVersion: '0.0.5',
                    protoMajor: 1,
                    protoMinor: minor,
                  ),
                  appVersion: '0.0.6',
                ),
              ),
            ),
          ),
        ),
      );
      await tester.pump();
      await tester.pump();
      return ran;
    }

    bool enabled(WidgetTester tester) =>
        tester.widget<HButton>(find.byKey(button)).enabled;

    Future<void> click(WidgetTester tester) async {
      await tester.tap(find.byKey(button));
      for (int n = 0; n < 4; n++) {
        await tester.pump();
      }
    }

    testWidgets('a retired daemon is reloaded, asked, then restarted', (
      WidgetTester tester,
    ) async {
      final List<List<String>> ran = await pumpRestart(
        tester,
        FakeDaemonClient.empty(),
      );
      expect(find.text('Restart daemon'), findsOneWidget);
      expect(enabled(tester), isTrue);
      expect(find.byKey(blocked), findsNothing);

      await click(tester);
      expect(ran, <List<String>>[
        <String>['systemctl', '--user', 'daemon-reload'],
        retire,
        <String>['systemctl', '--user', 'restart', 'humanitld.service'],
      ]);
      expect(find.byKey(failed), findsNothing);
      expect(enabled(tester), isTrue, reason: 'busy ends after the restart');
    });

    testWidgets('a busy daemon is not restarted and says why', (
      WidgetTester tester,
    ) async {
      // The window shows a stopped sandbox; the daemon knows better.
      final FakeDaemonClient client = FakeDaemonClient.empty();
      final List<List<String>> ran = await pumpRestart(
        tester,
        client,
        outcome: 'busy',
      );
      client.sandbox = defaultSandbox_().copyWith(state: SandboxState.running);
      await click(tester);
      expect(ran, <List<String>>[
        <String>['systemctl', '--user', 'daemon-reload'],
        retire,
      ]);
      // The fresh snapshot shows the session; ending it takes a second answer.
      expect(enabled(tester), isTrue);
      expect(client.sandbox.state, SandboxState.running);
    });

    testWidgets('a refresh without a snapshot keeps the button resting', (
      WidgetTester tester,
    ) async {
      final _SilentLater client = _SilentLater();
      final List<List<String>> ran = await pumpRestart(
        tester,
        client,
        outcome: 'busy',
      );
      expect(enabled(tester), isTrue);
      client.silent = true;
      await click(tester);
      expect(ran, hasLength(2));
      expect(enabled(tester), isFalse, reason: 'nobody knows, so it rests');
      expect(find.byKey(blocked), findsOneWidget);
    });

    testWidgets('none while connected restarts nothing and says so', (
      WidgetTester tester,
    ) async {
      final List<List<String>> ran = await pumpRestart(
        tester,
        FakeDaemonClient.empty(),
        outcome: 'none',
      );
      await click(tester);
      expect(ran.last, retire, reason: 'no restart after none');
      expect(find.byKey(failed), findsOneWidget);
    });

    testWidgets('without a command line a new daemon is not restarted', (
      WidgetTester tester,
    ) async {
      final List<List<String>> ran = await pumpRestart(
        tester,
        FakeDaemonClient.empty(),
        cliPath: null,
      );
      await click(tester);
      expect(ran, <List<String>>[
        <String>['systemctl', '--user', 'daemon-reload'],
      ], reason: 'no fallback to asking and restarting from 1.19 on');
      expect(find.byKey(failed), findsOneWidget);
    });

    testWidgets('an unreadable answer restarts nothing', (
      WidgetTester tester,
    ) async {
      final List<List<String>> ran = await pumpRestart(
        tester,
        FakeDaemonClient.empty(),
        outcome: 'unknown',
      );
      await click(tester);
      expect(ran.last, retire);
    });

    testWidgets('a daemon before 1.19 is asked Sandbox(Status) instead', (
      WidgetTester tester,
    ) async {
      final List<List<String>> ran = await pumpRestart(
        tester,
        FakeDaemonClient.empty(),
        minor: daemonRetireMinor - 1,
      );
      await click(tester);
      expect(ran, <List<String>>[
        <String>['systemctl', '--user', 'daemon-reload'],
        <String>['systemctl', '--user', 'restart', 'humanitld.service'],
      ]);
    });

    testWidgets('an old daemon with a session started meanwhile stays', (
      WidgetTester tester,
    ) async {
      final FakeDaemonClient client = FakeDaemonClient.empty();
      final List<List<String>> ran = await pumpRestart(
        tester,
        client,
        minor: daemonRetireMinor - 1,
      );
      expect(enabled(tester), isTrue);
      client.sandbox = defaultSandbox_().copyWith(state: SandboxState.running);
      await click(tester);
      expect(ran, <List<String>>[
        <String>['systemctl', '--user', 'daemon-reload'],
      ]);
      expect(client.sandbox.state, SandboxState.running);
    });

    testWidgets('a start or stop under way disables it, with the reason', (
      WidgetTester tester,
    ) async {
      for (final SandboxState held in <SandboxState>[
        SandboxState.starting,
        SandboxState.stopping,
      ]) {
        final FakeDaemonClient client = FakeDaemonClient.empty()
          ..sandbox = defaultSandbox_().copyWith(state: held);
        final List<List<String>> ran = await pumpRestart(tester, client);
        expect(enabled(tester), isFalse, reason: '$held');
        expect(
          find.text('Possible once the agent session has ended'),
          findsOneWidget,
          reason: '$held',
        );
        await tester.tap(find.byKey(button), warnIfMissed: false);
        await tester.pump();
        expect(ran, isEmpty, reason: '$held: nothing may run');
      }
    });

    testWidgets('a running session is only stopped after the second answer', (
      WidgetTester tester,
    ) async {
      final FakeDaemonClient client = FakeDaemonClient.empty()
        ..sandbox = defaultSandbox_().copyWith(state: SandboxState.running);
      final List<List<String>> ran = await pumpRestart(tester, client);
      await click(tester);
      expect(find.text('End session and restart'), findsOneWidget);
      expect(ran, isEmpty);
      expect(client.sandbox.state, SandboxState.running);

      await tester.tap(find.byKey(const Key('status-daemon-restart-cancel')));
      await tester.pump();
      expect(
        find.byKey(const Key('status-daemon-restart-confirm')),
        findsNothing,
      );
      expect(client.sandbox.state, SandboxState.running);
      expect(ran, isEmpty);

      await click(tester);
      await tester.tap(find.byKey(const Key('status-daemon-restart-confirm')));
      await tester.pumpAndSettle();
      expect(client.sandbox.state, SandboxState.stopped);
      expect(ran, <List<String>>[
        <String>['systemctl', '--user', 'daemon-reload'],
        retire,
        <String>['systemctl', '--user', 'restart', 'humanitld.service'],
      ]);
    });

    testWidgets('the question takes the focus on cancel and gives it back', (
      WidgetTester tester,
    ) async {
      final FakeDaemonClient client = FakeDaemonClient.empty()
        ..sandbox = defaultSandbox_().copyWith(state: SandboxState.running);
      await pumpRestart(tester, client);
      final FocusNode before = FocusNode(debugLabel: 'before');
      addTearDown(before.dispose);
      FocusManager.instance.rootScope.setFirstFocus(FocusScopeNode());
      before.attach(tester.element(find.byType(StatusBar)));
      FocusManager.instance.rootScope.requestFocus(before);
      await tester.pump();
      await click(tester);
      await tester.pump();
      expect(FocusManager.instance.primaryFocus?.debugLabel, 'restart-cancel');
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pump();
      expect(
        find.byKey(const Key('status-daemon-restart-confirm')),
        findsNothing,
      );
      expect(client.sandbox.state, SandboxState.running);
    });

    testWidgets('without a command line a running session rests', (
      WidgetTester tester,
    ) async {
      final FakeDaemonClient client = FakeDaemonClient.empty()
        ..sandbox = defaultSandbox_().copyWith(state: SandboxState.running);
      await pumpRestart(tester, client, cliPath: null);
      expect(enabled(tester), isFalse);
      expect(find.byKey(blocked), findsOneWidget);
    });

    testWidgets('an answer never carries over to another sandbox', (
      WidgetTester tester,
    ) async {
      final FakeDaemonClient client = FakeDaemonClient.empty()
        ..sandbox = defaultSandbox_().copyWith(
          state: SandboxState.running,
          sandboxId: const SandboxId('first'),
        );
      await pumpRestart(tester, client);
      await click(tester);
      expect(
        find.byKey(const Key('status-daemon-restart-confirm')),
        findsOneWidget,
      );
      final ProviderContainer container = ProviderScope.containerOf(
        tester.element(find.byType(StatusBar)),
      );
      client.sandbox = client.sandbox.copyWith(
        sandboxId: const SandboxId('second'),
      );
      await container.read(sandboxStatusProvider.notifier).refresh();
      await tester.pump();
      expect(
        find.byKey(const Key('status-daemon-restart-confirm')),
        findsNothing,
      );
      expect(client.sandbox.state, SandboxState.running);
    });

    testWidgets('the German sentence fits the smallest window', (
      WidgetTester tester,
    ) async {
      // `windowMinimumSize` is 1100 px wide; the test font is wider than any
      // real one, so what fits here fits in the app.
      tester.view.physicalSize = const Size(1100, 700);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final FakeDaemonClient client = FakeDaemonClient.empty()
        ..sandbox = defaultSandbox_().copyWith(state: SandboxState.running);
      await pumpRestart(tester, client, locale: const Locale('de'));
      await click(tester);
      expect(find.text('Sitzung beenden, neu starten'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('a failed command stops there and says so', (
      WidgetTester tester,
    ) async {
      final List<List<String>> ran = await pumpRestart(
        tester,
        FakeDaemonClient.empty(),
        reloadExit: 1,
      );
      await click(tester);
      expect(ran, hasLength(1), reason: 'nothing after a failed reload');
      expect(
        find.text(
          'Restart failed: systemctl --user daemon-reload exited with 1: no bus',
        ),
        findsOneWidget,
      );
      expect(enabled(tester), isTrue, reason: 'busy ends after a failure too');
    });
  });

  test('agentSessionHolds: unknown counts as a running session', () {
    expect(
      agentSessionHolds(const AsyncLoading<SandboxStatus>()),
      isTrue,
      reason: 'no answer yet',
    );
    expect(
      agentSessionHolds(
        AsyncError<SandboxStatus>(StateError('down'), StackTrace.empty),
      ),
      isTrue,
    );
    expect(
      agentSessionHolds(const AsyncData<SandboxStatus>(SandboxStatus())),
      isFalse,
    );
    expect(
      agentSessionHolds(
        const AsyncData<SandboxStatus>(
          SandboxStatus(state: SandboxState.failed),
        ),
      ),
      isFalse,
      reason: 'a plan that fails without a held sandbox',
    );
    expect(
      agentSessionHolds(
        AsyncData<SandboxStatus>(
          SandboxStatus(state: SandboxState.failed, sandboxId: SandboxId('s1')),
        ),
      ),
      isTrue,
      reason: 'a held sandbox whose plan fails',
    );
  });
}

/// A fake whose `Sandbox(Status)` answers with nothing once [silent] is set:
/// a refresh that brings no snapshot (HUM-284).
class _SilentLater extends FakeDaemonClient {
  _SilentLater() : super(script: const <ScriptedEvent>[]);

  bool silent = false;

  @override
  Stream<SandboxUpdate> sandboxStatus() =>
      silent ? const Stream<SandboxUpdate>.empty() : super.sandboxStatus();
}
