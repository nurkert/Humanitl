// Das Terminal des Sandbox-Bildschirms (HUM-042). Jeder Test prüft eine
// Zusage, die der Bildschirm über fremde Bytes macht: dass der Hinweis über
// dem Terminal steht, dass Tastendrücke den Weg hinauf nehmen, dass ein
// gehaltener Fluss außerhalb des Emulators sichtbar wird und dass ein zweiter
// Schreiber den Befund des Daemons zu sehen bekommt.

import 'dart:async';
import 'dart:typed_data';

import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/flow_events.dart';
import 'package:humanitl/features/sandbox/providers/terminal_provider.dart';
import 'package:humanitl/features/sandbox/widgets/terminal_pane.dart';
import 'package:xterm2/ui.dart';

import 'harness.dart';

/// Der Fake antwortet wie der Daemon: erst die Geometrie, dann eine Zeile,
/// danach das Echo der Eingabe.
const String fakeGreeting = 'humanitl fake terminal';

void main() {
  testWidgets('the_untrusted_banner_stands_above_the_terminal', (
    WidgetTester tester,
  ) async {
    await pumpSandbox(tester, client: runningClient());
    expect(
      find.textContaining('Agent output is untrusted'),
      findsOneWidget,
      reason: 'the sentence stands for as long as the terminal does',
    );
    // Und zwar über dem Terminal, nicht darunter oder daneben.
    final double banner = tester
        .getTopLeft(find.textContaining('Agent output is untrusted'))
        .dy;
    final double terminal = tester.getTopLeft(find.byType(TerminalPane)).dy;
    expect(banner, greaterThanOrEqualTo(terminal));
  });

  testWidgets('a_key_reaches_the_agent_as_bytes', (WidgetTester tester) async {
    final SandboxTestClient client = runningClient();
    await pumpSandbox(tester, client: client);
    // Der Strom steht erst nach dem ersten Bild; der Fake grüßt danach.
    await tester.pump();
    await tester.pump();

    final TerminalSessionState session = _session(tester, client);
    expect(session.phase, TerminalPhase.attached);

    // `onOutput` ist die Tastatur des Emulators: Was ein Mensch tippt, geht
    // als Bytes hinauf, und der Fake spiegelt es zurück. Die Marke steht in
    // nichts, was der Fake von sich aus schreibt — sonst wäre die Zusicherung
    // schon wahr, bevor eine Taste gefallen ist.
    const String typed = 'zzq7';
    expect(session.terminal.buffer.getText(), isNot(contains(typed)));
    session.terminal.onOutput?.call(typed);
    await tester.pump();
    await tester.pump();
    expect(
      session.terminal.buffer.getText(),
      contains(typed),
      reason: 'the bytes went up and came back',
    );
  });

  testWidgets('the_emulator_takes_no_keys_when_the_daemon_would_drop_them', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = runningClient();
    await pumpSandbox(tester, client: client);
    await tester.pump();
    await tester.pump();

    TerminalView view() =>
        tester.widget<TerminalView>(find.byType(TerminalView));
    expect(view().readOnly, isFalse, reason: 'the writer types');

    final ProviderContainer container = ProviderScope.containerOf(
      tester.element(find.byType(TerminalPane)),
    );
    final TerminalSession session = container.read(
      terminalSessionProvider(client.sandbox.sandboxId!.value).notifier,
    );
    // Ein Leser: Der Daemon verwirft seine Tastendrücke, und der Emulator
    // hört auf, welche anzunehmen. Ein Cursor, der auf Eingabe zu warten
    // scheint, verspricht sonst etwas, das niemand hält.
    session.state = session.state.copyWith(readOnly: true);
    await tester.pump();
    expect(view().readOnly, isTrue, reason: 'a reader only watches');

    // Und nach dem Ende des Agenten hört ohnehin niemand mehr zu.
    session.state = session.state.copyWith(
      readOnly: false,
      phase: TerminalPhase.ended,
    );
    await tester.pump();
    expect(view().readOnly, isTrue, reason: 'the agent is gone');
  });

  testWidgets('a_held_flow_shows_a_strip_above_the_terminal', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = runningClient();
    await pumpSandbox(tester, client: client);
    expect(find.byKey(const Key('sandbox-terminal-held')), findsNothing);

    final ProviderContainer container = ProviderScope.containerOf(
      tester.element(find.byType(TerminalPane)),
    );
    final HeldNotice notice = container.read(heldNoticeProvider.notifier);
    // Der Streifen liest den Ereignisstrom, nicht den Bytestrom: Ein
    // Vollbild-Agent zeichnet die Zeile im Strom mit dem nächsten Bild weg.
    notice.state = const TerminalNotice(
      flowId: FlowId('f-1'),
      method: 'POST',
      host: 'api.github.com',
      path: '/repos/x/y/issues',
    );
    await tester.pump();

    expect(find.byKey(const Key('sandbox-terminal-held')), findsOneWidget);
    expect(find.textContaining('api.github.com'), findsWidgets);
  });

  testWidgets('the_strip_comes_from_the_event_stream', (
    WidgetTester tester,
  ) async {
    final StreamController<FlowEvent> events =
        StreamController<FlowEvent>.broadcast();
    addTearDown(events.close);
    await pumpSandbox(
      tester,
      client: runningClient(),
      overrides: <Override>[
        flowEventsProvider.overrideWith((Ref ref) => events.stream),
      ],
    );

    final DateTime at = DateTime.utc(2026, 9, 5, 12);
    events
      ..add(
        FlowEvent.received(
          at: at,
          flow: Flow(
            id: const FlowId('f-2'),
            sessionId: const SessionId('s-1'),
            receivedAt: at,
            method: Method.post,
            scheme: Scheme.https,
            authority: const Authority(host: 'pypi.org', port: 443),
            path: '/simple/requests/',
            state: FlowState.received,
          ),
        ),
      )
      ..add(
        FlowEvent.held(
          at: at,
          flowId: const FlowId('f-2'),
          deadline: at.add(const Duration(minutes: 5)),
        ),
      );
    await tester.pump();
    await tester.pump();

    expect(find.byKey(const Key('sandbox-terminal-held')), findsOneWidget);
    expect(find.textContaining('pypi.org'), findsWidgets);

    // Eine Entscheidung beendet das Warten, und der Streifen geht wieder weg:
    // Was entschieden wurde, steht in der Historie.
    events.add(
      FlowEvent.decided(
        at: at,
        flowId: const FlowId('f-2'),
        kind: DecisionKind.allow,
      ),
    );
    await tester.pump();
    await tester.pump();
    expect(find.byKey(const Key('sandbox-terminal-held')), findsNothing);
  });

  testWidgets('a_second_writer_sees_the_finding_of_the_daemon', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = runningClient();
    await pumpSandbox(tester, client: client);
    await tester.pump();
    await tester.pump();

    final ProviderContainer container = ProviderScope.containerOf(
      tester.element(find.byType(TerminalPane)),
    );
    final String sandboxId = client.sandbox.sandboxId!.value;
    expect(
      container.read(terminalSessionProvider(sandboxId)).phase,
      TerminalPhase.attached,
    );

    // Ein zweiter schreibender Client an derselben Sitzung: Der Fake führt
    // denselben Platz wie der Daemon und lehnt ihn mit `TERM_001` ab.
    final List<TerminalFrame> frames = <TerminalFrame>[];
    final StreamController<TerminalCommand> input =
        StreamController<TerminalCommand>();
    addTearDown(input.close);
    final StreamSubscription<TerminalFrame> second = client
        .terminal(input.stream)
        .listen(frames.add);
    addTearDown(second.cancel);
    input.add(TerminalOpen(sandboxId: sandboxId, cols: 80, rows: 24));
    await tester.pump();
    await tester.pump();

    expect(frames, hasLength(1));
    final TerminalFrame first = frames.single;
    expect(first, isA<TerminalFinding>());
    expect(
      (first as TerminalFinding).diagnostic.code,
      DiagnosticCodes.terminalSecondWriter,
    );
  });

  testWidgets('what_a_reader_types_never_leaves_this_process', (
    WidgetTester tester,
  ) async {
    // Die Grenze steht im Daemon; der Fake führt sie mit, damit die
    // Oberfläche nicht gegen ein Verhalten übt, das der Daemon ablehnt.
    final SandboxTestClient client = runningClient();
    await pumpSandbox(tester, client: client);
    final List<TerminalFrame> frames = <TerminalFrame>[];
    final StreamController<TerminalCommand> input =
        StreamController<TerminalCommand>();
    addTearDown(input.close);
    final StreamSubscription<TerminalFrame> reader = client
        .terminal(input.stream)
        .listen(frames.add);
    addTearDown(reader.cancel);
    input.add(
      TerminalOpen(
        sandboxId: client.sandbox.sandboxId!.value,
        cols: 80,
        rows: 24,
        readOnly: true,
      ),
    );
    await tester.pump();
    input.add(TerminalKeys(Uint8List.fromList(<int>[0x61])));
    await tester.pump();
    await tester.pump();

    final String seen = frames
        .whereType<TerminalOutput>()
        .map((TerminalOutput frame) => String.fromCharCodes(frame.bytes))
        .join();
    expect(seen, contains(fakeGreeting));
    expect(
      seen.endsWith('a'),
      isFalse,
      reason: 'the keys of a reader are dropped, not echoed',
    );
  });
}

/// Der Zustand der Terminal-Sitzung dieses Bildschirms.
TerminalSessionState _session(WidgetTester tester, SandboxTestClient client) {
  final ProviderContainer container = ProviderScope.containerOf(
    tester.element(find.byType(TerminalPane)),
  );
  return container.read(
    terminalSessionProvider(client.sandbox.sandboxId!.value),
  );
}
