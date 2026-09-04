// Die Seite der History: Blättern ohne Lücke und ohne Dublette, die ehrliche
// Trefferzahl, der abgelehnte Filter und die Aktualisierung aus dem
// Ereignisstrom.

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/history/history_metrics.dart';
import 'package:humanitl/features/history/history_view.dart';
import 'package:humanitl/features/history/providers/history_page.dart';
import 'package:humanitl/features/history/providers/history_query.dart';

ProviderContainer _container(FakeDaemonClient client) {
  final ProviderContainer container = ProviderContainer(
    overrides: [daemonClientProvider.overrideWithValue(client)],
  );
  addTearDown(container.dispose);
  // Riverpod 3 pauses the listeners of a provider nobody listens to. On
  // screen the table watches the page; in a test this stands in for it, and
  // without it the live update would never reach the rows.
  container.listen(
    historyPageProvider,
    (HistoryPageState? previous, HistoryPageState next) {},
  );
  return container;
}

/// Waits until the page provider has finished loading.
///
/// The first load is scheduled by `build`, so a test cannot await it; it
/// waits for the state instead.
Future<void> settle(ProviderContainer container) async {
  for (int i = 0; i < 200; i++) {
    final HistoryPageState page = container.read(historyPageProvider);
    if (!page.loading && !page.loadingMore) {
      return;
    }
    await Future<void>.delayed(Duration.zero);
  }
  fail('the history page never finished loading');
}

void main() {
  test('the first page arrives newest first, without passthrough', () async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final ProviderContainer container = _container(client);
    await settle(container);
    final HistoryPageState page = container.read(historyPageProvider);
    expect(page.loading, isFalse);
    expect(page.failure, isNull);
    expect(page.rows, isNotEmpty);
    expect(page.rows.every((Flow flow) => !flow.passthrough), isTrue);
    for (int i = 1; i < page.rows.length; i++) {
      expect(
        page.rows[i].receivedAt.isAfter(page.rows[i - 1].receivedAt),
        isFalse,
        reason: 'newest first',
      );
    }
  });

  test('paging never duplicates and never skips a row', () async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 450);
    final ProviderContainer container = _container(client);
    final HistoryPageNotifier notifier = container.read(
      historyPageProvider.notifier,
    );
    await settle(container);
    expect(
      container.read(historyPageProvider).rows,
      hasLength(historyPageSize),
    );
    while (container.read(historyPageProvider).hasMore) {
      await notifier.loadMore();
    }
    final List<Flow> rows = container.read(historyPageProvider).rows;
    // Everything the default query can see: the recorded flows minus the
    // passthrough ones it leaves out. Counted from the fake rather than
    // written down, so the assertion says "no row was lost" and not "the
    // scenario has this shape".
    final int visible = client.state.flows.values
        .where((Flow flow) => !flow.passthrough)
        .length;
    expect(rows, hasLength(visible));
    expect(
      rows.map((Flow flow) => flow.id.value).toSet(),
      hasLength(rows.length),
      reason: 'no duplicates',
    );
    expect(container.read(historyPageProvider).cursor, isEmpty);
  });

  test('the window stops at historyMaxRows and says so', () async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 2400);
    final ProviderContainer container = _container(client);
    final HistoryPageNotifier notifier = container.read(
      historyPageProvider.notifier,
    );
    await settle(container);
    for (
      int i = 0;
      i < 20 && container.read(historyPageProvider).hasMore;
      i++
    ) {
      await notifier.loadMore();
    }
    final HistoryPageState page = container.read(historyPageProvider);
    expect(page.rows.length, lessThanOrEqualTo(historyMaxRows));
    expect(page.hasMore, isFalse);
    expect(page.windowFull, isTrue);
  });

  test(
    'a total at the ceiling is a lower bound, not a count',
    () async {
      // Every twelfth flow is passthrough and the default query leaves it
      // out, so the set has to be larger than the ceiling by more than that.
      final FakeDaemonClient client = FakeDaemonClient.history(
        count: fakeCountCeiling + 2000,
      );
      final ProviderContainer container = _container(client);
      await settle(container);
      final HistoryPageState page = container.read(historyPageProvider);
      expect(page.total, fakeCountCeiling);
      // Read from the wire, never inferred from the value.
      expect(page.capped, isTrue);
    },
    timeout: const Timeout(Duration(minutes: 2)),
  );

  test(
    'a filter narrows the set and keeps the grammar of the daemon',
    () async {
      final FakeDaemonClient client = FakeDaemonClient.history(count: 120);
      final ProviderContainer container = _container(client);
      container.read(historyQueryProvider.notifier).submit('state:held');
      await settle(container);
      final HistoryPageState page = container.read(historyPageProvider);
      expect(page.rows, isNotEmpty);
      expect(page.rows.every((Flow flow) => flow.isHeld), isTrue);
    },
  );

  test('findings:>0 and host: read as the recorder reads them', () async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 120);
    final ProviderContainer container = _container(client);
    container
        .read(historyQueryProvider.notifier)
        .submit('findings:>0 host:github.com');
    await settle(container);
    final List<Flow> rows = container.read(historyPageProvider).rows;
    expect(rows, isNotEmpty);
    expect(
      rows.every(
        (Flow flow) =>
            flow.findingCount > 0 && flow.host.endsWith('github.com'),
      ),
      isTrue,
    );
  });

  test(
    'an unknown key comes back as RECORDER_002 naming every valid key',
    () async {
      final FakeDaemonClient client = FakeDaemonClient.history(count: 12);
      final ProviderContainer container = _container(client);
      container.read(historyQueryProvider.notifier).submit('hosst:github.com');
      await settle(container);
      final Diagnostic? failure = container.read(historyPageProvider).failure;
      expect(failure, isNotNull);
      expect(failure!.code, historyFilterInvalidCode);
      for (final String key in fakeFilterKeys) {
        expect(failure.why, contains(key));
      }
    },
  );

  test('the passthrough chip is the include flag, not a filter term', () async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 60);
    final ProviderContainer container = _container(client);
    container
        .read(historyQueryProvider.notifier)
        .toggle(HistoryChip.passthrough);
    await settle(container);
    final List<Flow> rows = container.read(historyPageProvider).rows;
    expect(rows.any((Flow flow) => flow.passthrough), isTrue);
    expect(container.read(historyQueryProvider).filter, isEmpty);
  });

  test(
    'sorting by host asks the daemon, it does not re-sort the page',
    () async {
      final FakeDaemonClient client = FakeDaemonClient.history(count: 60);
      final ProviderContainer container = _container(client);
      container.read(historyQueryProvider.notifier).orderBy(HistorySort.host);
      await settle(container);
      final List<Flow> rows = container.read(historyPageProvider).rows;
      expect(rows, isNotEmpty);
      for (int i = 1; i < rows.length; i++) {
        expect(rows[i].host.compareTo(rows[i - 1].host) <= 0, isTrue);
      }
      expect(container.read(historyQueryProvider).orderBy, 'host desc');
      container.read(historyQueryProvider.notifier).orderBy(HistorySort.host);
      expect(container.read(historyQueryProvider).orderBy, 'host asc');
    },
  );

  test('live_update_changes_row_state', () async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 12);
    final ProviderContainer container = _container(client);
    await settle(container);
    final Flow held = container
        .read(historyPageProvider)
        .rows
        .firstWhere((Flow flow) => flow.isHeld);
    expect(historyVisualState(held), isNot(HFlowState.allowed));

    final int index = container
        .read(historyPageProvider)
        .rows
        .indexWhere((Flow flow) => flow.id == held.id);
    await client.decide(held.id, const Decision.allow());
    for (int i = 0; i < 50; i++) {
      final Flow current = container
          .read(historyPageProvider)
          .rows
          .firstWhere((Flow flow) => flow.id == held.id);
      if (current.decision != null) {
        break;
      }
      await Future<void>.delayed(Duration.zero);
    }

    final Flow after = container
        .read(historyPageProvider)
        .rows
        .firstWhere((Flow flow) => flow.id == held.id);
    expect(after.decision, DecisionKind.allow);
    expect(historyVisualState(after), HFlowState.allowed);
    // The row kept its place: nothing moved under the reading eye.
    expect(
      container
          .read(historyPageProvider)
          .rows
          .indexWhere((Flow flow) => flow.id == held.id),
      index,
    );
  });

  test(
    'an arrival joins the rows at once while the list is at its head',
    () async {
      final FakeDaemonClient client = FakeDaemonClient.history(count: 12);
      final ProviderContainer container = _container(client);
      final HistoryPageNotifier notifier = container.read(
        historyPageProvider.notifier,
      );
      await settle(container);
      final int before = container.read(historyPageProvider).rows.length;

      // The table stands at its head, which is where it starts.
      final Flow arrival = _arrival();
      client.state.flows[arrival.id] = arrival;
      notifier.applyEventForTest(
        FlowEvent.received(at: arrival.receivedAt, flow: arrival),
      );

      final HistoryPageState page = container.read(historyPageProvider);
      expect(page.rows.first.id, arrival.id, reason: 'newest first');
      expect(page.rows.length, before + 1);
      expect(page.pending, isEmpty, reason: 'nothing is waiting to be shown');
      expect(page.total, greaterThan(before));
    },
  );

  test(
    'an arrival waits in the pill while the list is read further down',
    () async {
      final FakeDaemonClient client = FakeDaemonClient.history(count: 12);
      final ProviderContainer container = _container(client);
      final HistoryPageNotifier notifier = container.read(
        historyPageProvider.notifier,
      );
      await settle(container);
      final int before = container.read(historyPageProvider).rows.length;

      notifier.setAtHead(false);
      final Flow arrival = _arrival();
      client.state.flows[arrival.id] = arrival;
      notifier.applyEventForTest(
        FlowEvent.received(at: arrival.receivedAt, flow: arrival),
      );

      expect(container.read(historyPageProvider).rows.length, before);
      expect(container.read(historyPageProvider).pending, hasLength(1));

      // Back at the head, what waited joins the list.
      notifier.setAtHead(true);
      expect(container.read(historyPageProvider).rows.first.id, arrival.id);
      expect(container.read(historyPageProvider).pending, isEmpty);
    },
  );

  test(
    'a gap in the stream reloads the page instead of being ignored',
    () async {
      final FakeDaemonClient client = FakeDaemonClient.history(count: 12);
      final ProviderContainer container = _container(client);
      final HistoryPageNotifier notifier = container.read(
        historyPageProvider.notifier,
      );
      await settle(container);
      final Flow held = container
          .read(historyPageProvider)
          .rows
          .firstWhere((Flow flow) => flow.isHeld);

      // While nobody listened: the held flow was decided and a new one was
      // recorded. That is what `Lagged` means, and the stream raises one on
      // every reconnect.
      client.state.flows[held.id] = held.copyWith(
        state: FlowState.recorded,
        decision: DecisionKind.block,
        decisionSource: DecisionSource.user,
        blockReason: BlockReason.user,
        status: 403,
        deadline: null,
      );
      final Flow duringTheGap = _arrival();
      client.state.flows[duringTheGap.id] = duringTheGap;

      notifier.applyEventForTest(
        FlowEvent.lagged(at: DateTime.utc(2027), dropped: 0),
      );
      await settle(container);

      final HistoryPageState page = container.read(historyPageProvider);
      expect(
        page.rows.firstWhere((Flow flow) => flow.id == held.id).isHeld,
        isFalse,
        reason: 'the row no longer offers a decision the queue cannot take',
      );
      expect(
        page.rows.any((Flow flow) => flow.id == duringTheGap.id),
        isTrue,
        reason: 'what was recorded during the gap is here',
      );
    },
  );

  test('an arrival under a filter is counted, never dropped', () async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final ProviderContainer container = _container(client);
    final HistoryPageNotifier notifier = container.read(
      historyPageProvider.notifier,
    );
    container.read(historyQueryProvider.notifier).submit('host:github.com');
    await settle(container);
    final int before = container.read(historyPageProvider).rows.length;
    expect(before, greaterThan(0));

    // A hit of the very filter that is set.
    final Flow arrival = _arrival().copyWith(
      authority: const Authority(host: 'api.github.com', port: 443),
    );
    client.state.flows[arrival.id] = arrival;
    notifier.applyEventForTest(
      FlowEvent.received(at: arrival.receivedAt, flow: arrival),
    );

    final HistoryPageState page = container.read(historyPageProvider);
    expect(page.waiting, 1, reason: 'the pill offers the reload');
    expect(page.missed, 1);
    expect(page.rows.length, before, reason: 'the daemon places it, not us');

    // And the reload really brings it.
    await notifier.refresh();
    await settle(container);
    expect(
      container
          .read(historyPageProvider)
          .rows
          .any((Flow flow) => flow.id == arrival.id),
      isTrue,
    );
    expect(container.read(historyPageProvider).waiting, 0);
  });

  test('a session of nothing but model calls is not an empty record', () async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    // Every recorded flow went to the model endpoint.
    for (final FlowId id in client.state.flows.keys.toList()) {
      client.state.flows[id] = client.state.flows[id]!.copyWith(
        passthrough: true,
      );
    }
    final ProviderContainer container = _container(client);
    await settle(container);

    final HistoryPageState page = container.read(historyPageProvider);
    expect(page.rows, isEmpty);
    expect(
      page.hiddenPassthrough,
      isTrue,
      reason: 'the record is not empty, the chip is hiding it',
    );
  });

  test('a truly empty record is not mistaken for a hidden one', () async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 0);
    final ProviderContainer container = _container(client);
    await settle(container);
    final HistoryPageState page = container.read(historyPageProvider);
    expect(page.rows, isEmpty);
    expect(page.hiddenPassthrough, isFalse);
  });

  test('a waiting flow that comes back in a later page stands once', () async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 450);
    final ProviderContainer container = _container(client);
    final HistoryPageNotifier notifier = container.read(
      historyPageProvider.notifier,
    );
    await settle(container);

    // A flow that is not on the first page arrives while the list is read
    // further down; it has to be one the default query can see.
    notifier.setAtHead(false);
    final Set<String> onPageOne = <String>{
      for (final Flow flow in container.read(historyPageProvider).rows)
        flow.id.value,
    };
    final Flow later = client.state.flows.values.firstWhere(
      (Flow flow) => !flow.passthrough && !onPageOne.contains(flow.id.value),
    );
    notifier.applyEventForTest(
      FlowEvent.received(at: later.receivedAt, flow: later),
    );
    expect(container.read(historyPageProvider).pending, hasLength(1));

    while (container.read(historyPageProvider).hasMore) {
      await notifier.loadMore();
    }
    notifier.setAtHead(true);
    final List<Flow> rows = container.read(historyPageProvider).rows;
    expect(
      rows.where((Flow flow) => flow.id == later.id),
      hasLength(1),
      reason: 'the buffer counts as known while a page is appended',
    );
  });

  test('the cursor is not moved by a row that joins at the top', () async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 450);
    final ProviderContainer container = _container(client);
    final HistoryPageNotifier notifier = container.read(
      historyPageProvider.notifier,
    );
    await settle(container);
    final String cursor = container.read(historyPageProvider).cursor;
    expect(cursor, isNotEmpty);

    final Flow arrival = _arrival();
    client.state.flows[arrival.id] = arrival;
    notifier.applyEventForTest(
      FlowEvent.received(at: arrival.receivedAt, flow: arrival),
    );
    // The cursor belongs to the bottom of the list; a row above it moves no
    // page boundary.
    expect(container.read(historyPageProvider).cursor, cursor);

    await notifier.loadMore();
    final List<Flow> rows = container.read(historyPageProvider).rows;
    expect(
      rows.map((Flow flow) => flow.id.value).toSet(),
      hasLength(rows.length),
      reason: 'no duplicate after an insert at the top',
    );
  });
}

/// A flow that arrives now, newer than anything the scenario seeded.
Flow _arrival() => Flow(
  id: const FlowId('018f0009-0000-7000-8000-000000000001'),
  sessionId: historySession,
  receivedAt: DateTime.utc(2027),
  method: Method.get,
  scheme: Scheme.https,
  authority: const Authority(host: 'fresh.example', port: 443),
  path: '/just-arrived',
  state: FlowState.received,
);
