// „Rückgängig" macht immer die Regel rückgängig, nie die Anfrage
// (docs/UX.md 4.5): der Streifen sagt beides, der Knopf ruft `Rules(remove)`,
// und nach `HMotion.undoWindow` verschwindet nur der Streifen.

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
import 'package:humanitl/features/intercept/providers/now.dart';
import 'package:humanitl/features/shell/providers/connection.dart';

import 'fixtures.dart';

FlowDetail github() => detailFor(
  heldFlow(
    n: 1,
    deadline: testStart.add(const Duration(minutes: 5)),
    method: Method.post,
    host: 'api.github.com',
    path: '/graphql',
    requestSize: 428,
  ),
);

Future<FakeDaemonClient> pumpAndRemember(WidgetTester tester) async {
  final FakeDaemonClient client = FakeDaemonClient(
    script: holdScript(<FlowDetail>[github()]),
    clock: () => testStart,
  );
  await tester.binding.setSurfaceSize(const Size(1400, 900));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    ProviderScope(
      overrides: <Override>[
        daemonClientProvider.overrideWithValue(client),
        connectionHeartbeatProvider.overrideWithValue(null),
        nowProvider.overrideWith(() => FixedNow(testStart)),
      ],
      child: const HumanitlApp(),
    ),
  );
  await tester.pump();
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 400));
  await tester.pump();

  // Das Raster öffnen (Session, Host) und senden.
  await tester.sendKeyEvent(LogicalKeyboardKey.digit2);
  await tester.pump();
  await tester.sendKeyEvent(LogicalKeyboardKey.enter);
  await tester.pump();
  await tester.pump();
  return client;
}

void main() {
  testWidgets('a decision that created a rule offers to take it back', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = await pumpAndRemember(tester);

    expect(client.rules, hasLength(1));
    expect(
      find.text('Rule saved · the request is already out'),
      findsOneWidget,
    );

    await tester.tap(find.byKey(const Key('intercept-undo')));
    await tester.pump();
    await tester.pump();

    // Die Regel ist weg, die Anfrage bleibt draußen.
    expect(client.rules, isEmpty);
    expect(client.decisions, hasLength(1));
    expect(find.text('Rule removed'), findsOneWidget);
  });

  testWidgets('after the window only the strip goes, never the rule', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = await pumpAndRemember(tester);
    expect(find.byKey(const Key('intercept-undo')), findsOneWidget);

    await tester.pump(HMotion.undoWindow + const Duration(seconds: 1));
    await tester.pump();

    expect(find.byKey(const Key('intercept-undo')), findsNothing);
    expect(client.rules, hasLength(1));
  });

  testWidgets('a decision without a rule shows no undo', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient(
      script: holdScript(<FlowDetail>[github()]),
      clock: () => testStart,
    );
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(
      ProviderScope(
        overrides: <Override>[
          daemonClientProvider.overrideWithValue(client),
          connectionHeartbeatProvider.overrideWithValue(null),
          nowProvider.overrideWith(() => FixedNow(testStart)),
        ],
        child: const HumanitlApp(),
      ),
    );
    await tester.pump();
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));
    await tester.pump();

    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    await tester.pump();

    expect(client.decisions, hasLength(1));
    expect(client.rules, isEmpty);
    expect(find.byKey(const Key('intercept-undo')), findsNothing);
  });
}
