// Die andere Hälfte der Übergabe: der History-Screen bittet, die Shell führt
// aus. Ein Feature greift nicht in ein anderes (ARCHITECTURE 5), also steht
// die Zusage in zwei Tests — hier der über die ganze App.

import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/app.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ipc/flow_handoff.dart';
import 'package:humanitl/core/ipc/flow_reveal.dart';
import 'package:humanitl/core/ui/h_diagnostic_card.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/history/history_screen.dart';
import 'package:humanitl/features/history/providers/history_detail.dart';
import 'package:humanitl/features/history/providers/history_page.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/shell/providers/connection.dart';
import 'package:humanitl/features/shell/providers/navigation.dart';
import 'package:humanitl/features/shell/section.dart';

void main() {
  testWidgets('the history does not take the focus back out of the rail', (
    WidgetTester tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final FakeDaemonClient client = FakeDaemonClient.history(count: 12);
    final ProviderContainer container = ProviderContainer(
      overrides: [
        daemonClientProvider.overrideWithValue(client),
        connectionHeartbeatProvider.overrideWithValue(null),
      ],
    );
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: const HumanitlApp(),
      ),
    );
    await tester.pump();
    await tester.pump();
    container.read(navigationProvider.notifier).go(Section.history);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 600));

    // Put the focus on a control of the shell, outside the history. This is
    // the case the screen cannot see from inside: its own focus node counts
    // a focused row as focused, so only a node in a sibling subtree tells
    // the two behaviours apart.
    final FocusNode rail = FocusNode(debugLabel: 'outside-the-history');
    addTearDown(rail.dispose);
    tester.binding.focusManager.rootScope.attach(
      rail.context ?? tester.element(find.byType(HumanitlApp)),
    );
    rail.requestFocus();
    await tester.pump();
    final int claims = debugHistoryFocusClaims;

    // Rebuild the history a few times.
    for (int i = 0; i < 3; i++) {
      container
          .read(historySelectionProvider.notifier)
          .select(client.state.flows.values.elementAt(i).id);
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 400));
    }

    expect(
      debugHistoryFocusClaims,
      claims,
      reason: 'the keyboard is claimed once per becoming visible',
    );

    container.dispose();
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('the shell carries out a handover the history asked for', (
    WidgetTester tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final ProviderContainer container = ProviderContainer(
      overrides: [
        daemonClientProvider.overrideWithValue(client),
        connectionHeartbeatProvider.overrideWithValue(null),
      ],
    );
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: const HumanitlApp(),
      ),
    );
    await tester.pump();
    await tester.pump();

    container.read(navigationProvider.notifier).go(Section.history);
    await tester.pump();

    final Flow held = client.state.flows.values.firstWhere(
      (Flow flow) => flow.isHeld,
    );
    container.read(flowHandoffProvider.notifier).request(held.id);
    await tester.pump();
    await tester.pump();

    expect(container.read(navigationProvider), Section.intercept);
    expect(container.read(selectedFlowIdProvider), held.id);
    // The note is cleared, so the handover happens once and not on every
    // rebuild of the shell.
    expect(container.read(flowHandoffProvider), isNull);

    // The queue's clock is a periodic timer of the intercept feature; it is
    // disposed inside the test, because the framework checks for pending
    // timers before tear-downs run.
    container.dispose();
    await tester.pumpWidget(const SizedBox.shrink());
  });

  // HUM-039: a finding over the queue names a flow the queue does not hold,
  // the passthrough behind `LLM_005`. The shell shows the history, and the
  // history opens the flow from `GetFlow` -- red as soon as the sheet is built
  // from the table rows, because this flow is on no page the table has loaded.
  testWidgets('the history opens a flow another section asked for', (
    WidgetTester tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final FakeDaemonClient client = FakeDaemonClient.history(count: 250);
    final ProviderContainer container = ProviderContainer(
      overrides: [
        daemonClientProvider.overrideWithValue(client),
        connectionHeartbeatProvider.overrideWithValue(null),
      ],
    );
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: const HumanitlApp(),
      ),
    );
    await tester.pump();
    await tester.pump();
    expect(container.read(navigationProvider), Section.intercept);

    final Flow oldest = client.state.flows.values
        .where((Flow flow) => !flow.isHeld)
        .reduce(
          (Flow a, Flow b) => a.receivedAt.isBefore(b.receivedAt) ? a : b,
        );
    expect(
      container
          .read(historyPageProvider)
          .rows
          .any((Flow row) => row.id == oldest.id),
      isFalse,
      reason: 'the flow must be outside the loaded page for this to prove it',
    );

    container.read(flowRevealProvider.notifier).request(oldest.id);
    await tester.pump();
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));

    expect(container.read(navigationProvider), Section.history);
    expect(container.read(historySelectionProvider), oldest.id);
    expect(find.byType(HSheet), findsOneWidget);
    // Carried out once: the note is cleared by the history, not left for the
    // next rebuild to open the sheet again.
    expect(container.read(flowRevealProvider), isNull);

    container.dispose();
    await tester.pumpWidget(const SizedBox.shrink());
  });

  // HUM-039, review: a request the daemon cannot give back is said on the
  // screen, with the daemon's sentence, and not swallowed -- red as soon as
  // the failure goes silent again (`docs/UX.md` 4.4).
  testWidgets('a request the history cannot open is said, not swallowed', (
    WidgetTester tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final FakeDaemonClient client = FakeDaemonClient.history(count: 12);
    final ProviderContainer container = ProviderContainer(
      overrides: [
        daemonClientProvider.overrideWithValue(client),
        connectionHeartbeatProvider.overrideWithValue(null),
      ],
    );
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: const HumanitlApp(),
      ),
    );
    await tester.pump();
    await tester.pump();

    const FlowId unknown = FlowId('01920000-0000-7000-8000-0000000000aa');
    container.read(flowRevealProvider.notifier).request(unknown);
    await tester.pump();
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));

    expect(container.read(navigationProvider), Section.history);
    expect(find.text('Could not open the request'), findsOneWidget);
    // The daemon's own sentence, not a generic one of the application.
    final HDiagnosticCard card = tester.widget<HDiagnosticCard>(
      find.byKey(const Key('history-reveal-failure')),
    );
    expect(card.why, contains('unknown flow'));
    expect(find.byType(HSheet), findsNothing);
    expect(container.read(flowRevealProvider), isNull);

    // And the next request that works replaces the complaint: red as soon as
    // the card outlives the failure it describes (review, second round).
    final Flow known = client.state.flows.values.firstWhere(
      (Flow flow) => !flow.isHeld,
    );
    container.read(flowRevealProvider.notifier).request(known.id);
    await tester.pump();
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));
    expect(find.byKey(const Key('history-reveal-failure')), findsNothing);
    expect(find.byType(HSheet), findsOneWidget);

    container.dispose();
    await tester.pumpWidget(const SizedBox.shrink());
  });
}
