// Der Isolations-Reiter (HUM-041): drei Garantien, drei Belege, und die drei
// Zustände, die niemals gleich aussehen dürfen -- belegt, gilt nicht, nicht
// gemessen.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/sandbox/providers/sandbox_status_provider.dart';
import 'package:humanitl/features/sandbox/sandbox_screen.dart';
import 'package:humanitl/features/sandbox/sandbox_text.dart';

import 'harness.dart';

/// Die Evidenzzeile mit einem Suchlauf, der sein Budget erreicht hat.
///
/// `limit=entries` heisst: die Suche ist nicht fertig geworden. Das ist kein
/// Fehler und auch kein Beweis, und das Panel muss den Unterschied zeigen.
const String truncatedSocketEvidence =
    'single_socket ok: sockets=/run/humanitl/proxy.sock;unexpected=none;'
    'entries=2000;limit=entries; '
    'bridge_listening ok: proxy=127.0.0.1:3128->/run/humanitl/proxy.sock';

/// Die drei Garantien mit einem Suchlauf, der frueh abgebrochen hat.
const List<IsolationCheckResult> truncatedWalkChecks = <IsolationCheckResult>[
  IsolationCheckResult(
    check: IsolationCheck.noNetworkInterface,
    passed: true,
    evidence: 'no_interfaces ok: lo',
  ),
  IsolationCheckResult(
    check: IsolationCheck.singleSocket,
    passed: true,
    evidence: truncatedSocketEvidence,
  ),
  IsolationCheckResult(
    check: IsolationCheck.seccompActive,
    passed: true,
    evidence: 'seccomp_applied ok: Seccomp:2;NoNewPrivs:1',
  ),
];

/// Ein Client, dessen zweiter Start bei `starting` stehen bleibt.
///
/// So laesst sich der eine Frame anschauen, in dem der alte Pruefstand noch
/// im Speicher liegt und die neue Sandbox noch nichts gemessen hat.
class StallingSecondStartClient extends SandboxTestClient {
  /// Wie oft `Sandbox(Start)` schon gerufen wurde.
  int startCount = 0;

  @override
  Stream<SandboxUpdate> startSandbox({
    String? profile,
    String? workDir,
    WorkMode? workMode,
  }) async* {
    startCount++;
    if (startCount > 1) {
      sandbox = sandbox.copyWith(
        state: SandboxState.starting,
        agentRunning: false,
      );
      yield SandboxUpdate.status(sandbox);
      return;
    }
    yield* super.startSandbox(
      profile: profile,
      workDir: workDir,
      workMode: workMode,
    );
  }
}

/// Der Provider dieses Bildschirms.
SandboxStatusNotifier notifierOf(WidgetTester tester) =>
    ProviderScope.containerOf(tester.element(find.byType(SandboxScreen)))
        .read(sandboxStatusProvider.notifier);

void main() {
  Future<void> openIsolation(WidgetTester tester) =>
      openTab(tester, 'sandbox-tab-isolation');

  testWidgets('three_measured_guarantees_stand_with_their_evidence', (
    WidgetTester tester,
  ) async {
    await pumpSandbox(tester, client: checkedClient(isolationGreenChecks));
    await openIsolation(tester);

    // Die drei Sätze, wortgleich mit docs/SECURITY.md Abschnitt 1.
    expect(
      find.text('No network interface. There is nowhere for traffic to go.'),
      findsOneWidget,
    );
    expect(
      find.text('Exactly one door: a socket that leads to Humanitl.'),
      findsOneWidget,
    );
    expect(
      find.text('The kernel opens no new door (seccomp).'),
      findsOneWidget,
    );

    // Und daneben, je Zeile, der Beleg. Ohne ihn ist der Punkt Dekoration.
    for (final IsolationCheck check in IsolationCheck.values) {
      expect(
        find.byKey(Key('isolation-evidence-${check.name}')),
        findsOneWidget,
        reason: '${check.name} must show what was measured',
      );
      expect(
        tester
            .widget<Text>(find.byKey(Key('isolation-state-${check.name}')))
            .data,
        'proven',
      );
    }
    expect(
      find.textContaining('sockets=/run/humanitl/proxy.sock'),
      findsOneWidget,
    );
    expect(find.textContaining('Seccomp:2;NoNewPrivs:1'), findsOneWidget);
  });

  testWidgets('a_guarantee_that_does_not_hold_says_what_it_means', (
    WidgetTester tester,
  ) async {
    await pumpSandbox(tester, client: checkedClient(isolationOneRedCheck));
    await openIsolation(tester);

    expect(find.text('does not hold'), findsOneWidget);
    // Der Befund des Daemons steht unter der Zeile, mit `why` und `fix`, und
    // nennt die Garantie -- nicht „check failed".
    expect(
      find.byKey(const Key('isolation-diagnostic-singleSocket')),
      findsOneWidget,
    );
    expect(
      find.textContaining('unexpected=/work/agent.sock'),
      findsNWidgets(2),
      reason: 'the evidence and the finding both name the file',
    );
    expect(find.text('SANDBOX_015'), findsOneWidget);
    // Die beiden anderen bleiben, was sie sind: gemessen und belegt.
    expect(find.text('proven'), findsNWidgets(2));
  });

  testWidgets('a_missing_result_is_its_own_state', (WidgetTester tester) async {
    await pumpSandbox(
      tester,
      client: checkedClient(<IsolationCheckResult>[isolationGreenChecks.first]),
    );
    await openIsolation(tester);

    // Genau eine Zeile ist belegt; die beiden anderen sind es nicht, und sie
    // tragen weder das Wort noch die Farbe einer belegten.
    expect(find.text('proven'), findsOneWidget);
    expect(find.text('not measured'), findsNWidgets(2));
    expect(
      find.byKey(const Key('isolation-missing-singleSocket')),
      findsOneWidget,
    );
    expect(
      find.byKey(const Key('isolation-missing-seccompActive')),
      findsOneWidget,
    );
    expect(
      find.textContaining('nothing about this guarantee is proven'),
      findsNWidgets(2),
    );
  });

  testWidgets('a_stopped_sandbox_says_that_nothing_was_measured_once', (
    WidgetTester tester,
  ) async {
    await pumpSandbox(tester, client: SandboxTestClient());
    await openIsolation(tester);

    expect(
      find.textContaining('Nothing runs, so nothing is measured'),
      findsOneWidget,
    );
    expect(find.text('not measured'), findsNWidgets(3));
    // Nichts läuft, also fehlt auch kein Bericht: der Satz über den
    // ausgebliebenen Befund gehört hier nicht hin.
    expect(
      find.textContaining('nothing about this guarantee is proven'),
      findsNothing,
    );
  });

  testWidgets('a_walk_that_ran_out_of_budget_is_not_smooth_green', (
    WidgetTester tester,
  ) async {
    await pumpSandbox(tester, client: checkedClient(truncatedWalkChecks));
    await openIsolation(tester);

    expect(
      find.byKey(const Key('isolation-limit-singleSocket')),
      findsOneWidget,
    );
    expect(
      find.textContaining('stopped at its budget'),
      findsOneWidget,
      reason: 'limit=entries must never read as a finished search',
    );
    // Die Mutationsprobe: derselbe Bildschirm mit `limit=none` sagt es nicht.
    await tester.pumpWidget(
      sandboxUnderTest(client: checkedClient(isolationGreenChecks)),
    );
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 600));
    await openIsolation(tester);
    expect(find.textContaining('stopped at its budget'), findsNothing);
  });

  testWidgets('the_fourth_line_names_the_llm_exception', (
    WidgetTester tester,
  ) async {
    await pumpSandbox(tester, client: checkedClient(isolationGreenChecks));
    await openIsolation(tester);

    expect(
      tester
          .widget<Text>(find.byKey(const Key('sandbox-isolation-exception')))
          .data,
      'Exception: LLM at http://192.168.1.50:11434 -- passthrough, logged, '
      'never held.',
    );
  });

  testWidgets('without_an_endpoint_the_fourth_line_says_there_is_none', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = checkedClient(isolationGreenChecks);
    client.sandbox = client.sandbox.copyWith(llmEndpoint: '');
    await pumpSandbox(tester, client: client);
    await openIsolation(tester);

    expect(
      tester
          .widget<Text>(find.byKey(const Key('sandbox-isolation-exception')))
          .data,
      'No LLM exception configured.',
    );
  });

  testWidgets('the_panel_leads_to_the_exact_command', (
    WidgetTester tester,
  ) async {
    await pumpSandbox(tester, client: checkedClient(isolationGreenChecks));
    await openIsolation(tester);

    // Der Reiter ist höher als sein Pane; der Weg zum Beleg wird sichtbar
    // gemacht, bevor er geklickt wird.
    await tester.ensureVisible(find.byKey(const Key('sandbox-isolation-argv')));
    await tester.pump();
    await tester.tap(find.byKey(const Key('sandbox-isolation-argv')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 300));
    expect(find.byKey(const Key('sandbox-argv-text')), findsOneWidget);
  });

  testWidgets('a_start_that_fails_a_guarantee_never_reports_running', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = SandboxTestClient()
      ..isolationChecks = isolationOneRedCheck;
    await pumpSandbox(tester, client: client);
    await tester.tap(find.byKey(const Key('sandbox-start')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 600));

    final SandboxStatus status = statusOf(tester);
    expect(status.state, SandboxState.failed);
    expect(status.isolationProven, isFalse);
    expect(
      status.diagnostics.map((Diagnostic d) => d.code),
      contains(DiagnosticCodes.isolationSingleSocket),
    );
    // Und es gibt kein „trotzdem starten": der blockierende Befund lässt die
    // Schaltfläche aus (BACKLOG.md 4.1).
    expect(
      tester.widget<HButton>(find.byKey(const Key('sandbox-start'))).onPressed,
      isNull,
    );
  });

  testWidgets('a_report_that_never_arrived_is_never_a_start', (
    WidgetTester tester,
  ) async {
    // Die Form, die der echte Daemon schickt: drei rote Ergebnisse mit
    // `SANDBOX_013`, nicht null Ereignisse.
    final SandboxTestClient client = SandboxTestClient()
      ..isolationChecks = isolationNoReportChecks;
    await pumpSandbox(tester, client: client);
    await tester.tap(find.byKey(const Key('sandbox-start')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 600));

    final SandboxStatus status = statusOf(tester);
    expect(status.state, SandboxState.failed);
    expect(
      status.diagnostics.map((Diagnostic d) => d.code),
      contains(DiagnosticCodes.isolationNoReport),
    );
    // Drei rote Zeilen, keine graue: „kein Bericht" ist selbst ein Ergebnis.
    expect(status.checks, hasLength(3));
    expect(status.checksPassed, 0);
    await openIsolation(tester);
    expect(find.text('does not hold'), findsNWidgets(3));
    expect(find.text('not measured'), findsNothing);
  });

  testWidgets('a_second_start_never_shows_a_result_of_the_first', (
    WidgetTester tester,
  ) async {
    final StallingSecondStartClient client = StallingSecondStartClient()
      ..isolationChecks = isolationOneRedCheck;
    await pumpSandbox(tester, client: client);
    await openIsolation(tester);

    // Lauf 1: Pruefung 1 gruen, Pruefung 2 rot, Sandbox stirbt.
    await notifierOf(tester).start();
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 600));
    SandboxStatus status = statusOf(tester);
    expect(status.state, SandboxState.failed);
    expect(status.checks, hasLength(3));
    expect(
      status.segmentFor(IsolationCheck.noNetworkInterface),
      IsolationSegment.passed,
    );
    expect(find.text('proven'), findsNWidgets(2));

    // Der Befund verschwindet erst, wenn jemand neu fragt; danach darf wieder
    // gestartet werden.
    await notifierOf(tester).refresh();
    await tester.pump();
    // Ein Ergebnis desselben Laufs ueberlebt die Nachfrage -- es gehoert noch
    // zu ihm.
    expect(statusOf(tester).checks, hasLength(3));

    // Lauf 2: er steht bei `starting`, und dort ist nichts gemessen.
    await notifierOf(tester).start();
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 600));

    status = statusOf(tester);
    expect(status.state, SandboxState.starting);
    expect(
      status.checks,
      isEmpty,
      reason: 'a result of the first run says nothing about the second',
    );
    for (final IsolationCheck check in IsolationCheck.values) {
      expect(status.segmentFor(check), IsolationSegment.running);
      expect(
        isolationSegmentFilled(status.segmentFor(check)),
        isFalse,
        reason: 'no dot is filled while the new sandbox has measured nothing',
      );
    }
    expect(find.text('proven'), findsNothing);
    expect(find.text('does not hold'), findsNothing);
    expect(find.text('measuring'), findsNWidgets(3));
  });

  testWidgets('a_result_of_another_sandbox_is_dropped_on_sight', (
    WidgetTester tester,
  ) async {
    // Der Bildschirm kennt Lauf A. Der Daemon antwortet ueber Lauf B -- von
    // der Kommandozeile gestartet -- und bringt keine Ergebnisse mit.
    await pumpSandbox(tester, client: checkedClient(isolationGreenChecks));
    await openIsolation(tester);
    expect(statusOf(tester).checks, hasLength(3));
    expect(
      statusOf(tester).checksSandboxId,
      FakeDaemonClient.defaultSandbox,
      reason: 'the results were adopted by the run they were measured in',
    );

    final SandboxStatus otherRun = statusOf(tester).copyWith(
      sandboxId: const SandboxId('018f0002-0000-7000-8000-0000000000b2'),
      checks: const <IsolationCheckResult>[],
      checksSandboxId: null,
    );
    expect(
      statusOf(tester).carryChecksInto(otherRun).checks,
      isEmpty,
      reason: 'a result from run A says nothing about run B',
    );
  });
}
