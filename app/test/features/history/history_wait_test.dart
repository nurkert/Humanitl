// Warten nach `docs/UX.md` 2.11: unter 150 ms passiert nichts, danach stehen
// Skelette in der Zieldichte, und beim Eintreffen wird nichts verschoben.

import 'dart:async';

import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/history/history_metrics.dart';
import 'package:humanitl/features/history/history_screen.dart';
import 'package:humanitl/features/history/history_table.dart';
import 'package:humanitl/l10n/l10n.dart';

/// A fake whose `ListFlows` answers only when the test says so.
///
/// A subclass rather than a hand-written `DaemonClient`: the port grows with
/// every screen, and a stand-in that has to list every method breaks whenever
/// somebody adds one.
class SlowClient extends FakeDaemonClient {
  /// Creates a fake with [count] recorded flows and a gate in front of the
  /// list.
  SlowClient({int count = 24}) : super(script: const <ScriptedEvent>[]) {
    seedHistory(state, count: count, start: historyEpoch);
  }

  /// Completes the pending `ListFlows`.
  Completer<void>? pending;

  @override
  Future<FlowPage> listFlows(
    FlowFilter filter, {
    int limit = 200,
    String? cursor,
  }) async {
    final Completer<void> gate = Completer<void>();
    pending = gate;
    await gate.future;
    return super.listFlows(filter, limit: limit, cursor: cursor);
  }
}

Widget _app(ProviderContainer container) => UncontrolledProviderScope(
  container: container,
  child: WidgetsApp(
    color: HColors.bg0,
    debugShowCheckedModeBanner: false,
    localizationsDelegates: AppLocalizations.localizationsDelegates,
    supportedLocales: AppLocalizations.supportedLocales,
    builder: (BuildContext context, Widget? _) => HTheme(
      tokens: HTokens.dark,
      child: ColoredBox(
        color: HColors.bg0,
        child: Overlay(
          initialEntries: <OverlayEntry>[
            OverlayEntry(
              builder: (BuildContext context) => const HistoryScreen(),
            ),
          ],
        ),
      ),
    ),
  ),
);

void main() {
  testWidgets('nothing is shown for the first 150 ms of a wait', (
    WidgetTester tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final SlowClient client = SlowClient();
    final ProviderContainer container = ProviderContainer(
      overrides: [daemonClientProvider.overrideWithValue(client)],
    );
    addTearDown(container.dispose);
    await tester.pumpWidget(_app(container));
    await tester.pump();

    // Below the threshold the pane keeps its layout and says nothing: no
    // rows, and no sketch of rows either.
    await tester.pump(const Duration(milliseconds: 100));
    expect(find.byType(HistoryRow), findsNothing);
    expect(
      find.byType(HSkeleton),
      findsNothing,
      reason: 'no skeleton before HMotion.waitVisible',
    );

    // Past it the skeleton of the expected rows stands where they will
    // stand, in the target density.
    await tester.pump(const Duration(milliseconds: 100));
    expect(find.byType(HSkeleton), findsOneWidget);
    final Rect tableBefore = tester.getRect(find.byType(HistoryTable));

    client.pending!.complete();
    for (int i = 0; i < 60; i++) {
      await tester.pump(const Duration(milliseconds: 16));
      if (find.byType(HistoryRow).evaluate().isNotEmpty) {
        break;
      }
    }
    expect(find.byType(HistoryRow), findsWidgets);
    expect(find.byType(HSkeleton), findsNothing);
    // The rows replaced the skeleton where the skeleton stood; nothing was
    // pushed anywhere (`docs/UX.md` 2.11).
    expect(tester.getRect(find.byType(HistoryTable)), tableBefore);
    expect(
      tester.getSize(find.byType(HistoryRow).first).height,
      historyRowHeight,
    );
    await tester.pump(const Duration(seconds: 1));
  });

  testWidgets('a skeleton that appeared stands its minimum time', (
    WidgetTester tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final SlowClient client = SlowClient();
    final ProviderContainer container = ProviderContainer(
      overrides: [daemonClientProvider.overrideWithValue(client)],
    );
    addTearDown(container.dispose);
    await tester.pumpWidget(_app(container));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));
    expect(find.byType(HSkeleton), findsOneWidget);

    // The answer arrives right after the skeleton appeared. It stays for
    // `HMotion.waitMinVisible`, or the two frames would read as a flicker.
    client.pending!.complete();
    await tester.pump(const Duration(milliseconds: 50));
    expect(find.byType(HSkeleton), findsOneWidget);

    await tester.pump(HMotion.waitMinVisible);
    await tester.pump(const Duration(milliseconds: 32));
    expect(find.byType(HSkeleton), findsNothing);
    expect(find.byType(HistoryRow), findsWidgets);
    await tester.pump(const Duration(seconds: 1));
  });
}
