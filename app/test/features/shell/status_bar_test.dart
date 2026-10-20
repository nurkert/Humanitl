// Die Auslastung der Warteschlange in der Statusleiste (HUM-058, Zeile 16,
// übernommen von HUM-057): unter 80 % nichts, ab 80 % amber, ab 100 % rot mit
// `LIMIT_002`.

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
import 'package:humanitl/features/shell/queue_pressure.dart';
import 'package:humanitl/features/shell/widgets/status_bar.dart';
import 'package:humanitl/l10n/l10n.dart';

import '../intercept/fixtures.dart';

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

    // 8 von 10: ab 80 % amber, mit Zahl, Grenze und der Belegung des Daemons.
    await _pump(tester, _held(8), poolBytes: 8000);
    expect(find.text('Queue 8/10 · 8.0 kB'), findsOneWidget);
    expect(_colorOf(tester), tokens.stateTextColor(HFlowState.held));
    expect(find.byKey(const Key('status-queue-limit')), findsNothing);

    // 10 von 10: voll, rot, mit `LIMIT_002` und dem Titel aus ARB.
    await _pump(tester, _held(10));
    expect(_colorOf(tester), tokens.stateTextColor(HFlowState.blocked));
    expect(find.byKey(const Key('status-queue-limit')), findsOneWidget);
    expect(find.text('LIMIT_002'), findsOneWidget);
    expect(find.text('Queue full'), findsOneWidget);

    // Auch die Bytes zählen, und zwar die des Daemons: 850 kB von 1 MB im
    // Budget sind amber, auch wenn die gehaltenen Flüsse wenig ankündigen
    // (gerade gepufferte Rümpfe anderer Anfragen zählen mit).
    await _pump(tester, _held(1), poolBytes: 850000);
    expect(_colorOf(tester), tokens.stateTextColor(HFlowState.held));

    // Ein gehaltener Stream-Kopf kündigt viel an und belegt fast nichts: kein
    // Amber, kein Rot (HUM-057, Review 2).
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
}
