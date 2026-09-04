// Gemeinsames Gerüst der Intercept-Tests von HUM-029 und HUM-072: die App über
// einen Fake-Daemon, eine stehende Uhr und die Handgriffe, die jeder dieser
// Tests braucht.

import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/app.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/intercept/intercept_screen.dart';
import 'package:humanitl/features/intercept/providers/now.dart';
import 'package:humanitl/features/shell/providers/connection.dart';

import 'fixtures.dart';

/// Ein Fake mit stehender Uhr.
FakeDaemonClient fakeDaemon([List<ScriptedEvent>? script]) =>
    FakeDaemonClient(script: script, clock: () => testStart);

/// Ein angehaltener Flow mit Host [host] und Pfad [path].
FlowDetail held(
  int n, {
  String host = 'registry.npmjs.org',
  String path = '/react',
  Method method = Method.get,
  Duration deadline = const Duration(minutes: 5),
  int findings = 0,
  int requestSize = 128,
}) => detailFor(
  heldFlow(
    n: n,
    deadline: testStart.add(deadline + Duration(seconds: n)),
    method: method,
    host: host,
    path: path,
    requestSize: requestSize,
  ).copyWith(findingCount: findings),
);

/// Ein Skript, das genau [detail] nach [at] empfängt und anhält.
///
/// `holdScript` verteilt eine Liste gleichmäßig; hier steht der Zeitpunkt je
/// Flow, weil die Einfriertests genau zwischen zwei Ankünften prüfen.
List<ScriptedEvent> arriveAt(
  FlowDetail detail,
  Duration at, {
  Duration budget = const Duration(minutes: 5),
}) => <ScriptedEvent>[
  ScriptedEvent(at, (FakeSessionState state, DateTime now) {
    state.details[detail.summary.id] = detail;
    return FlowEvent.received(
      at: now,
      flow: detail.summary.copyWith(
        state: FlowState.received,
        deadline: null,
        heldAt: null,
        receivedAt: now,
      ),
    );
  }),
  ScriptedEvent(
    at + const Duration(milliseconds: 1),
    (FakeSessionState state, DateTime now) => FlowEvent.held(
      at: now,
      flowId: detail.summary.id,
      deadline: now.add(budget),
    ),
  ),
];

/// Baut die App über [client] und lässt das Verbindungs-Gate durch.
Future<void> pumpIntercept(
  WidgetTester tester, {
  required FakeDaemonClient client,
  List<Override> overrides = const <Override>[],
  Size size = const Size(1400, 900),
  TextScaler textScaler = TextScaler.noScaling,
  bool disableAnimations = false,
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
        data: MediaQueryData(
          textScaler: textScaler,
          supportsAnnounce: true,
          disableAnimations: disableAnimations,
        ),
        child: const HumanitlApp(),
      ),
    ),
  );
  await tester.pump();
  await tester.pump();
}

/// Lässt das Skript ablaufen. Die Vorgabe liegt über `HMotion.rearm`, also ist
/// die Auswahl danach scharf.
Future<void> playScript(
  WidgetTester tester, [
  Duration duration = const Duration(milliseconds: 400),
]) async {
  await tester.pump(duration);
  await tester.pump();
  await tester.pump();
}

/// Der Container des laufenden Screens.
ProviderContainer containerOf(WidgetTester tester) =>
    ProviderScope.containerOf(tester.element(find.byType(InterceptScreen)));

/// Drückt [key] mit gehaltener Strg-Taste.
Future<void> pressControl(WidgetTester tester, LogicalKeyboardKey key) async {
  await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
  await tester.sendKeyEvent(key);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
  await tester.pump();
}

/// Drückt [key] mit gehaltener Umschalttaste.
Future<void> pressShiftKey(WidgetTester tester, LogicalKeyboardKey key) async {
  await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
  await tester.sendKeyEvent(key);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
  await tester.pump();
}

/// Klickt [finder] mit gehaltener [modifier].
Future<void> tapWith(
  WidgetTester tester,
  Finder finder,
  LogicalKeyboardKey modifier,
) async {
  await tester.sendKeyDownEvent(modifier);
  await tester.tap(finder, warnIfMissed: false);
  await tester.pump();
  await tester.sendKeyUpEvent(modifier);
  await tester.pump();
}

/// Fährt mit dem Zeiger auf [finder] und bleibt dort.
Future<TestGesture> hoverOver(WidgetTester tester, Finder finder) async {
  final TestGesture gesture = await tester.createGesture(
    kind: PointerDeviceKind.mouse,
  );
  await gesture.addPointer(location: Offset.zero);
  addTearDown(gesture.removePointer);
  await gesture.moveTo(tester.getCenter(finder));
  await tester.pump();
  return gesture;
}

/// Sammelt die Ansagen, die an die Barrierefreiheit gehen.
List<String> captureAnnouncements(WidgetTester tester) {
  final List<String> said = <String>[];
  tester.binding.defaultBinaryMessenger.setMockDecodedMessageHandler<Object?>(
    SystemChannels.accessibility,
    (Object? message) async {
      final Map<Object?, Object?> event = message! as Map<Object?, Object?>;
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
