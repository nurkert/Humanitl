// Der Ring im Header (HUM-041): drei Bögen, drei Garantien, und der Weg zu
// dem Beleg dahinter. Er ist immer sichtbar, also muss er immer die Wahrheit
// zeigen -- grau, solange nichts gemessen wurde.

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/sandbox/providers/sandbox_status_provider.dart';
import 'package:humanitl/features/sandbox/sandbox_screen.dart';
import 'package:humanitl/features/sandbox/widgets/isolation_panel.dart';
import 'package:humanitl/features/shell/widgets/header_bar.dart';

import '../../harness/app_harness.dart';
import '../sandbox/harness.dart';

/// Der Schlüssel des Rings im Header.
const Key ringKey = Key('header-isolation-ring');

/// Die Beschriftung, die der Ring gerade trägt.
String ringLabel(WidgetTester tester) =>
    tester.widget<HButton>(find.byKey(ringKey)).semanticsLabel!;

/// Ein Fake, dessen Sandbox mit [checks] laeuft.
///
/// Die Momentaufnahme traegt keine Ergebnisse; sie kommen als Ereignisse aus
/// `Sandbox(IsolationCheck)`, wie beim echten Daemon.
FakeDaemonClient clientWithChecks(List<IsolationCheckResult> checks) {
  final FakeDaemonClient client = FakeDaemonClient(
    script: const <ScriptedEvent>[],
  );
  client
    ..isolationChecks = checks
    ..sandbox = client.sandbox.copyWith(
      state: SandboxState.running,
      agentRunning: true,
      sandboxId: FakeDaemonClient.defaultSandbox,
    );
  return client;
}

void main() {
  testWidgets('a_ring_that_measured_nothing_says_so_and_never_counts_zero', (
    WidgetTester tester,
  ) async {
    await pumpApp(tester, client: FakeDaemonClient());

    expect(find.byType(IsolationRing), findsOneWidget);
    expect(find.byKey(ringKey), findsOneWidget);
    // Nicht „0/3 bestanden": das wäre eine Zahl über etwas, das niemand
    // gemessen hat (CONVENTIONS 4.13).
    expect(ringLabel(tester), 'Isolation: not checked yet');
  });

  testWidgets('a_ring_over_three_measured_guarantees_counts_them', (
    WidgetTester tester,
  ) async {
    await pumpApp(tester, client: clientWithChecks(isolationGreenChecks));
    // Der Sandbox-Bildschirm fragt beim Sichtbarwerden nach; der Ring liest
    // dieselbe Momentaufnahme.
    await tester.pump();
    await tester.pump();

    expect(ringLabel(tester), '3/3 isolation checks passed');
  });

  testWidgets('a_ring_over_a_guarantee_that_does_not_hold_names_it', (
    WidgetTester tester,
  ) async {
    await pumpApp(tester, client: clientWithChecks(isolationOneRedCheck));
    await tester.pump();
    await tester.pump();

    expect(
      ringLabel(tester),
      'Isolation check failed: Isolation check 2: more than one door',
    );
  });

  testWidgets('the_ring_leads_to_the_evidence', (WidgetTester tester) async {
    await pumpApp(tester, client: clientWithChecks(isolationGreenChecks));
    await tester.pump();
    await tester.pump();

    await tester.tap(find.byKey(ringKey));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 600));

    // Der Sandbox-Bildschirm, und darin der Reiter mit den drei Garantien.
    expect(find.byType(SandboxScreen), findsOneWidget);
    expect(find.byType(IsolationPanel), findsOneWidget);
    expect(
      tester.state<State<SandboxScreen>>(find.byType(SandboxScreen)).mounted,
      isTrue,
    );
  });

  testWidgets('the_tab_choice_the_ring_makes_is_the_isolation_tab', (
    WidgetTester tester,
  ) async {
    await pumpApp(tester, client: clientWithChecks(isolationGreenChecks));
    await tester.pump();
    await tester.pump();

    await tester.tap(find.byKey(ringKey));
    await tester.pump();

    expect(
      ProviderScope.containerOf(tester.element(find.byKey(ringKey)))
          .read(sandboxTabChoiceProvider),
      SandboxTab.isolation,
    );
  });
}
