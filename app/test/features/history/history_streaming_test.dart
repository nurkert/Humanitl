// Eine Antwort, die noch einläuft, im History-Detail (HUM-058, Zeile 12;
// HUM-032).
//
// Die Größe der Antwort ist eine laufende Summe und sagt das, solange der
// Strom läuft; nach dem Ende steht die Gesamtgröße ohne den Zusatz.
//
// Gegen welche Umkehrung der Test gerichtet ist: `ResponseChunk` in
// `HistoryPageNotifier._apply` übergangen, und die Zahl im Detail bleibt
// stehen; `historyResponseStreaming` immer falsch, und der Zusatz fehlt.

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/history/history_view.dart';
import 'package:humanitl/features/history/providers/history_page.dart';
import 'package:humanitl/features/history/providers/history_detail.dart';

import 'harness.dart';

Future<void> settle(WidgetTester tester) async {
  for (int i = 0; i < 4; i++) {
    await tester.pump(const Duration(milliseconds: 200));
  }
}

void main() {
  testWidgets('streaming_counter_updates', (WidgetTester tester) async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final FlowId id = client.state.flows.keys.first;
    client.state.update(
      id,
      (Flow flow) => flow.copyWith(state: FlowState.responded, responseSize: 0),
    );
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    container.read(historySelectionProvider.notifier).select(id);
    await settle(tester);
    final HistoryPageNotifier page = container.read(
      historyPageProvider.notifier,
    );
    final Flow row = container
        .read(historyPageProvider)
        .rows
        .firstWhere((Flow flow) => flow.id == id);

    String sizeOf(int bytes, {required bool final_}) {
      final String pair = formatHistorySizePair(
        row.copyWith(
          responseSize: bytes,
          state: final_ ? FlowState.recorded : FlowState.responded,
        ),
        unknown: '—',
      );
      return final_ ? pair : '$pair · still arriving';
    }

    for (final int bytes in <int>[1200, 640000, 1200000]) {
      page.applyEventForTest(
        FlowEvent.responseChunk(
          at: row.receivedAt,
          flowId: id,
          bytesSoFar: bytes,
        ),
      );
      await settle(tester);
      expect(
        find.text(sizeOf(bytes, final_: false)),
        findsOneWidget,
        reason: 'after $bytes bytes',
      );
    }

    page.applyEventForTest(FlowEvent.recorded(at: row.receivedAt, flowId: id));
    await settle(tester);
    expect(find.textContaining('still arriving'), findsNothing);
    expect(find.text(sizeOf(1200000, final_: true)), findsOneWidget);
  });
}
