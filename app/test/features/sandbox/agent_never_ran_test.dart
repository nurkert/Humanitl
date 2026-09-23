// Ein Agent, den es nicht gibt, fällt nicht mehr lautlos aus (HUM-137).
//
// Der Daemon schickt `AGENT_005`, wenn der Agent mit `127` oder `126` endet,
// bevor er ein Byte geschrieben hat, und zwar nach `running` und vor dem
// Exit-Code: Die Sandbox stand, ihre drei Garantien waren belegt, nur das
// Kommando darin ließ sich nicht starten. Diese Tests halten fest, was der
// Bildschirm daraus macht: Der Grund steht unter dem Kopf, und der Ring bleibt
// bei der Wahrheit über die Sandbox. Ein Befund über den Agenten färbt keine
// Garantie rot, und er beendet keine Sitzung, die steht.

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/h_diagnostic_card.dart';

import 'harness.dart';

/// Der Befund, wie ihn `humanitl_sandbox::did_not_start` baut, für ein
/// Kommando, das es in der Sandbox nicht gibt.
const Diagnostic agentNeverRan = Diagnostic(
  code: 'AGENT_005',
  severity: Severity.error,
  title: 'Agent startete nicht',
  why:
      'the agent gibt-es-nicht ended with exit code 127 before it wrote '
      'anything: the sandbox could not find or execute it. Inside the sandbox '
      'commands are looked up on PATH=/usr/local/bin:/usr/bin:/bin, and only '
      'what the sandbox profile mounts exists there.',
  fix: FixAction.copyCommand(
    command:
        "humanitl sandbox run -- /bin/sh -c 'command -v gibt-es-nicht || "
        'echo "not found on PATH=\$PATH"\'',
  ),
);

/// Ein Fake, dessen Start wie beim echten Daemon verläuft, wenn das
/// Kommando fehlt: drei belegte Garantien, `running`, und danach der Befund.
///
/// Nach dem Befund kommt keine weitere Momentaufnahme; der Daemon schickt
/// nur noch Exit-Code und Zusammenfassung, und beides faltet der Provider
/// nicht in den Zustand. Der Befund muss also an der Momentaufnahme von
/// `running` hängen bleiben.
class NeverRanClient extends SandboxTestClient {
  NeverRanClient() {
    isolationChecks = isolationGreenChecks;
  }

  @override
  Stream<SandboxUpdate> startSandbox({
    String? profile,
    String? workDir,
    WorkMode? workMode,
  }) async* {
    // Der Daemon nimmt die Momentaufnahme `running` erst nach der
    // Isolationsprüfung; ein `exec`, das nach Millisekunden scheitert, ist
    // dann schon vorbei, und `agent_running` ist falsch.
    await for (final SandboxUpdate update in super.startSandbox(
      profile: profile,
      workDir: workDir,
      workMode: workMode,
    )) {
      if (update is SandboxUpdateStatus &&
          update.status.state == SandboxState.running) {
        sandbox = update.status.copyWith(agentRunning: false);
        yield SandboxUpdate.status(sandbox);
      } else {
        yield update;
      }
    }
    yield const SandboxUpdate.diagnostic(agentNeverRan);
  }
}

/// Die Plattform, auf der Humanitl läuft; `flutter test` wäre sonst Android.
final TargetPlatformVariant linux = TargetPlatformVariant.only(
  TargetPlatform.linux,
);

void main() {
  testWidgets('an_agent_that_never_ran_is_said_under_the_header', (
    WidgetTester tester,
  ) async {
    final NeverRanClient client = NeverRanClient();
    await pumpSandbox(tester, client: client);

    await tester.tap(find.byKey(const Key('sandbox-start')));
    await tester.pump();
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 600));

    final Iterable<HDiagnosticCard> cards = tester
        .widgetList<HDiagnosticCard>(find.byType(HDiagnosticCard))
        .where((HDiagnosticCard card) => card.code == 'AGENT_005');
    expect(cards, hasLength(1), reason: 'the reason nothing runs is said');
    expect(
      cards.single.why,
      contains('gibt-es-nicht'),
      reason: 'the sentence of the daemon, not one the screen made up',
    );
    expect(cards.single.why, contains('PATH='));
    expect(find.textContaining('gibt-es-nicht'), findsWidgets);
  }, variant: linux);

  testWidgets('the_ring_stays_true_to_the_sandbox_when_the_agent_never_ran', (
    WidgetTester tester,
  ) async {
    final NeverRanClient client = NeverRanClient();
    await pumpSandbox(tester, client: client);

    await tester.tap(find.byKey(const Key('sandbox-start')));
    await tester.pump();
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 600));

    final SandboxStatus status = statusOf(tester);
    expect(
      status.diagnostics.map((Diagnostic finding) => finding.code),
      contains('AGENT_005'),
      reason: 'the snapshot carries the finding',
    );
    // Die Sandbox steht: Sie wird weder als gescheitert gemeldet, noch
    // verliert eine der drei Garantien ihr Grün an einen Befund, der über den
    // Agenten spricht und nicht über die Isolation.
    expect(status.state, SandboxState.running);
    expect(status.agentExited, isTrue, reason: 'and nothing runs inside it');
    for (final IsolationCheck check in IsolationCheck.values) {
      expect(
        status.segmentFor(check),
        IsolationSegment.passed,
        reason: '$check was measured and holds',
      );
    }
    // Ein Fehler und kein blockierender Befund: Er verbietet nichts, er
    // erklärt ein Ende. Der Stopp bleibt erreichbar.
    expect(status.blocking, isNull);
    expect(find.byKey(const Key('sandbox-stop')), findsOneWidget);
  }, variant: linux);
}
