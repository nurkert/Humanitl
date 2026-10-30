// Die Befunde des Sandbox-Bildschirms schweben über dem Terminal (HUM-302).
//
// Gemeldet am 2026-10-29: `LLM_006` („LLM endpoint is not on a private
// network") stand als eigenes Band zwischen Kopf und Terminal, kam mit jedem
// Status und ging mit ihm, und jedes Mal schrumpfte oder wuchs das Terminal,
// und die Oberfläche des Agenten darin brach neu um. Schließen ließ es sich
// nicht. Diese Tests halten fest: Das Terminal behält seine Größe und schickt
// kein `TerminalResize`, wenn ein Befund kommt oder geht; das Schließen gilt
// für den Befund samt Gegenstand, auch über einen neuen Status und einen
// neuen Bildschirm hinweg, und ein anderer Endpunkt bringt ihn zurück.

import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:humanitl/core/ipc/client_diagnostics.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui_state/dismissed_findings.dart';
import 'package:humanitl/core/ui_state/ui_state_file.dart';
import 'package:humanitl/features/sandbox/providers/closed_failures.dart';
import 'package:humanitl/features/sandbox/providers/sandbox_status_provider.dart';
import 'package:humanitl/features/sandbox/sandbox_screen.dart';
import 'package:humanitl/features/sandbox/widgets/sandbox_header.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:xterm2/ui.dart';

import 'harness.dart';

/// `LLM_006`, wie der Start ihn für [endpoint] meldet.
Diagnostic llm006(String endpoint) => Diagnostic(
  code: 'LLM_006',
  severity: Severity.info,
  title: 'LLM-Endpunkt liegt nicht in einem privaten Netz',
  why:
      '$endpoint is not on a private network. Traffic to this address '
      'bypasses the queue, so only put a machine you control here.',
);

/// Ein Fake, dessen Start abgelehnt wird, bevor die Sandbox je `starting`
/// meldet: kein Zwischenstand, nur der Befund und der Fehlschlag.
class RefusingClient extends SandboxTestClient {
  /// Der Grund, mit dem jeder Start scheitert.
  static const Diagnostic refusal = Diagnostic(
    code: 'SANDBOX_006',
    severity: Severity.error,
    title: 'project folder refused',
    why: '/home/a is not a project folder the sandbox may mount',
  );

  @override
  Stream<SandboxUpdate> startSandbox({
    String? profile,
    WorkMode? workMode,
    SandboxMode? sandboxMode,
  }) async* {
    starts++;
    sandbox = sandbox.copyWith(
      state: SandboxState.failed,
      diagnostics: const <Diagnostic>[refusal],
    );
    yield const SandboxUpdate.diagnostic(refusal);
    yield SandboxUpdate.status(sandbox);
  }
}

/// Ein zweiter Befund derselben Antwort, der offen bleibt.
const Diagnostic other = Diagnostic(
  code: 'SANDBOX_021',
  severity: Severity.info,
  title: 'a second finding',
  why: 'the second finding stays while the first is closed',
);

/// Der Schlüssel des Schließen-Knopfs der ersten Karte mit [code].
Key dismissOf(String code, {int place = 0}) =>
    ValueKey<String>('sandbox-diagnostic-dismiss-$place-$code');

/// Fragt den Status neu ab, wie es der Bildschirm beim Sichtbarwerden tut.
Future<void> refresh(WidgetTester tester) async {
  final ProviderContainer container = ProviderScope.containerOf(
    tester.element(find.byType(SandboxScreen)),
  );
  await container.read(sandboxStatusProvider.notifier).refresh();
  await tester.pump();
  await tester.pump();
}

/// Ein Zustandsspeicher in einer Datei, die der Test selbst kennt.
UiStateFile stateFile() {
  final Directory dir = Directory.systemTemp.createTempSync('hum302-state');
  addTearDown(() => dir.deleteSync(recursive: true));
  return UiStateFile('${dir.path}/${UiStateFile.fileName}');
}

void main() {
  testWidgets('a_finding_leaves_the_terminal_its_size_and_sends_no_resize', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = runningClient();
    await pumpSandbox(tester, client: client);
    await tester.pump();
    await tester.pump();
    final Rect terminal = tester.getRect(find.byType(TerminalView));
    final int resizes = client.resizes.length;

    // Der Befund kommt mit dem nächsten Status.
    client.sandbox = client.sandbox.copyWith(
      diagnostics: <Diagnostic>[llm006('https://api.example.com')],
    );
    await refresh(tester);
    await tester.pump(const Duration(milliseconds: 300));
    expect(find.text('LLM_006'), findsOneWidget);
    expect(tester.getRect(find.byType(TerminalView)), terminal);

    // Er schwebt rechts oben über dem Terminal, unter dem Kopf.
    final Rect card = tester.getRect(
      find
          .ancestor(
            of: find.text('LLM_006'),
            matching: find.byWidgetPredicate(
              (Widget widget) => widget is Semantics && widget.container,
            ),
          )
          .first,
    );
    expect(
      card.top,
      greaterThanOrEqualTo(tester.getRect(find.byType(SandboxHeader)).bottom),
    );
    expect(card.right, lessThan(terminal.right));
    expect(card.left, greaterThan(terminal.left));

    // Und er geht wieder, mit dem Status, der ihn nicht mehr trägt.
    client.sandbox = client.sandbox.copyWith(diagnostics: const <Diagnostic>[]);
    await refresh(tester);
    await tester.pump(const Duration(milliseconds: 300));
    expect(find.text('LLM_006'), findsNothing);
    expect(tester.getRect(find.byType(TerminalView)), terminal);

    // Kein einziges `TerminalResize` in der ganzen Zeit.
    expect(client.resizes.length, resizes);
  });

  testWidgets('closing_a_finding_keeps_it_closed_until_its_subject_changes', (
    WidgetTester tester,
  ) async {
    final UiStateFile file = stateFile();
    final SandboxTestClient client = runningClient();
    // Neben `LLM_006` ein zweiter Befund: Geschlossen wird nur der eine.
    client.sandbox = client.sandbox.copyWith(
      diagnostics: <Diagnostic>[llm006('https://api.example.com'), other],
    );
    await pumpSandbox(
      tester,
      client: client,
      uiState: uiStateFileProvider.overrideWithValue(file),
    );
    await tester.pump();
    expect(find.text('SANDBOX_021'), findsOneWidget);
    final Rect terminal = tester.getRect(find.byType(TerminalView));
    expect(find.text('LLM_006'), findsOneWidget);

    await tester.tap(find.byKey(dismissOf('LLM_006')));
    await tester.pump();
    expect(find.text('LLM_006'), findsNothing);
    expect(find.text('SANDBOX_021'), findsOneWidget);
    expect(tester.getRect(find.byType(TerminalView)), terminal);

    // Derselbe Befund im nächsten Status: bleibt zu.
    await refresh(tester);
    expect(find.text('LLM_006'), findsNothing);

    // Geschrieben ist der Fingerabdruck, nicht der Satz.
    final Object? stored = file.read()[dismissedFindingKey('LLM_006')];
    expect(stored, <String>[
      findingFingerprint(llm006('https://api.example.com')),
    ]);
    expect(file.read().toString(), isNot(contains('api.example.com')));

    // Ein neuer Bildschirm über derselben Datei: bleibt zu.
    await tester.pumpWidget(const SizedBox.shrink());
    await pumpSandbox(
      tester,
      client: client,
      uiState: uiStateFileProvider.overrideWithValue(file),
    );
    expect(find.text('LLM_006'), findsNothing);

    // Ein anderer Endpunkt ist ein anderer Gegenstand: Der Befund kommt
    // wieder.
    client.sandbox = client.sandbox.copyWith(
      diagnostics: <Diagnostic>[llm006('https://other.example.net'), other],
    );
    await refresh(tester);
    await tester.pump(const Duration(milliseconds: 300));
    expect(find.text('LLM_006'), findsOneWidget);
  });

  testWidgets('the_arrival_of_a_finding_leaves_the_keyboard_where_it_was', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = runningClient();
    await pumpSandbox(tester, client: client);
    await tester.pump();
    await tester.pump();
    await tester.tap(find.byType(TerminalView));
    await tester.pump();
    final FocusNode? before = FocusManager.instance.primaryFocus;
    expect(before, isNotNull);

    client.sandbox = client.sandbox.copyWith(
      diagnostics: <Diagnostic>[llm006('https://api.example.com')],
    );
    await refresh(tester);
    await tester.pump(const Duration(milliseconds: 300));
    expect(find.text('LLM_006'), findsOneWidget);
    expect(FocusManager.instance.primaryFocus, same(before));
  });

  testWidgets('a_closed_start_failure_comes_back_when_the_next_start_fails', (
    WidgetTester tester,
  ) async {
    // Ein Fehlschlag ist ein Ereignis, kein Befund, der steht (Review): Ein
    // zweiter Start, der wieder scheitert, muss das sagen, auch mit
    // wörtlich demselben Grund. Und geschrieben wird dafür nichts.
    final UiStateFile file = stateFile();
    final SandboxTestClient client = SandboxTestClient()
      ..sandboxStartFailure = const Diagnostic(
        code: 'SANDBOX_006',
        severity: Severity.error,
        title: 'project folder refused',
        why: '/home/a is not a project folder the sandbox may mount',
      );
    await pumpSandbox(
      tester,
      client: client,
      uiState: uiStateFileProvider.overrideWithValue(file),
    );
    final ProviderContainer container = ProviderScope.containerOf(
      tester.element(find.byType(SandboxScreen)),
    );
    Future<void> start() async {
      await container.read(sandboxStatusProvider.notifier).start();
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 300));
    }

    await start();
    expect(find.text('SANDBOX_006'), findsOneWidget);
    await tester.tap(find.byKey(dismissOf('SANDBOX_006')));
    await tester.pump();
    expect(find.text('SANDBOX_006'), findsNothing);
    // Derselbe Status noch einmal abgefragt: bleibt zu, es ist dieselbe
    // Operation.
    await refresh(tester);
    expect(find.text('SANDBOX_006'), findsNothing);
    expect(
      file.read().keys,
      isNot(contains(dismissedFindingKey('SANDBOX_006'))),
    );

    // Der nächste Start scheitert genauso: Die Karte ist wieder da.
    await start();
    expect(find.text('SANDBOX_006'), findsWidgets);
  });

  test('a_closed_folder_dialog_failure_is_that_one_failure_only', () {
    final ProviderContainer container = ProviderContainer(
      overrides: <Override>[
        daemonClientProvider.overrideWithValue(SandboxTestClient()),
        uiStateFileProvider.overrideWithValue(stateFile()),
      ],
    );
    addTearDown(container.dispose);
    final ClosedFailures closed = container.read(
      closedFailuresProvider.notifier,
    );
    final Diagnostic first = ClientDiagnostics.folderDialogFailed('no portal');
    final Diagnostic again = ClientDiagnostics.folderDialogFailed('no portal');
    expect(first, again, reason: 'equal in value');
    closed.closeDialog(first);
    expect(closed.coversDialog(first), isTrue);
    // Der nächste fehlgeschlagene Dialog ist ein neues Ereignis.
    expect(closed.coversDialog(again), isFalse);
  });

  testWidgets('a_start_refused_before_starting_shows_its_failure_again', (
    WidgetTester tester,
  ) async {
    // Der Start erreicht `starting` nie (Review). Geschlossen bleibt der
    // Fehlschlag über eine Abfrage des Status hinweg, und der nächste Start
    // bringt ihn zurück: Er ist eine neue Operation.
    final RefusingClient client = RefusingClient();
    await pumpSandbox(tester, client: client);
    final ProviderContainer container = ProviderScope.containerOf(
      tester.element(find.byType(SandboxScreen)),
    );
    Future<void> start() async {
      await container.read(sandboxStatusProvider.notifier).start();
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 300));
    }

    await start();
    expect(find.text('SANDBOX_006'), findsOneWidget);
    await tester.tap(find.byKey(dismissOf('SANDBOX_006')));
    await tester.pump();
    expect(find.text('SANDBOX_006'), findsNothing);

    await refresh(tester);
    expect(find.text('SANDBOX_006'), findsNothing);

    await start();
    expect(client.starts, 2);
    expect(find.text('SANDBOX_006'), findsOneWidget);
  });

  testWidgets('findings_leave_the_bottom_rows_of_a_small_terminal_free', (
    WidgetTester tester,
  ) async {
    // Das Terminal auf seiner kleinsten Höhe und zwei Befunde darüber: Die
    // Eingabezeile und die Zeile darüber bleiben frei (Review).
    final UiStateFile file = stateFile();
    file.setValue(sandboxPaneSplit.key, <double>[0.05, 0.95]);
    final SandboxTestClient client = runningClient();
    client.sandbox = client.sandbox.copyWith(
      diagnostics: <Diagnostic>[llm006('https://api.example.com'), other],
    );
    await pumpSandbox(
      tester,
      client: client,
      uiState: uiStateFileProvider.overrideWithValue(file),
    );
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 300));
    expect(tester.takeException(), isNull);

    final Rect terminal = tester.getRect(find.byType(TerminalView));
    expect(
      terminal.height,
      lessThanOrEqualTo(sandboxTerminalMinHeight),
      reason: 'the terminal stands at its minimum',
    );
    final Rect notices = tester.getRect(
      find.byKey(const Key('notice-layer-notices')),
    );
    expect(notices.height, greaterThan(0), reason: 'the findings still show');
    final HTokens tokens = HTheme.of(tester.element(find.byType(TerminalView)));
    final double row = (tokens.typography.mono13.fontSize ?? 13) * 1.2;
    // Unter den Befunden: zwei Zeilen und der Innenabstand des Emulators.
    expect(
      notices.bottom,
      lessThanOrEqualTo(
        terminal.bottom - sandboxTerminalFreeRows * row - HSpace.x2 + 0.5,
      ),
    );
  });
}
