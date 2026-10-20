// Ein Terminal-Strom, der ohne Exit-Code endet, wird neu verbunden (HUM-058,
// Zeile 19).
//
// Das Fenster schreibt eine Zeile ins Terminal, hängt sich nach einer kurzen
// Wartezeit wieder an, schickt dabei die Geometrie, die es jetzt hat, und
// behält alles, was schon auf dem Schirm stand.
//
// Gegen welche Umkehrung jeder Test gerichtet ist:
//
// - `_scheduleReconnect` in `_onDone` weg: `reconnect_keeps_buffer` wird rot,
//   weil es keinen zweiten Anschluss gibt.
// - Der Horcher im Bildschirm, der die Zeile schreibt, weg: rot im selben
//   Test, weil die Zeile im Rücklauf fehlt.
// - Die Wache gegen `ended` oder `refused` im Zeitgeber weg: die beiden
//   Gegenproben werden rot, weil das Fenster sich nach einem Ende oder einer
//   Absage wieder anhängt.

import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/sandbox/providers/terminal_provider.dart';
import 'package:humanitl/features/sandbox/widgets/terminal_pane.dart';

import 'harness.dart';

/// Ein Client, dessen Terminal-Ströme dem Test gehören.
class ReconnectClient extends SandboxTestClient {
  /// Der Strom des jüngsten Anschlusses.
  StreamController<TerminalFrame> frames = StreamController<TerminalFrame>();

  /// Jedes `Open`, das ein Anschluss geschickt hat.
  final List<TerminalOpen> opens = <TerminalOpen>[];

  final List<StreamSubscription<TerminalCommand>> _commands =
      <StreamSubscription<TerminalCommand>>[];

  @override
  Stream<TerminalFrame> terminal(Stream<TerminalCommand> input) {
    _commands.add(
      input.listen((TerminalCommand command) {
        if (command is TerminalOpen) {
          opens.add(command);
        }
      }),
    );
    if (_commands.length > 1) {
      frames = StreamController<TerminalFrame>();
    }
    return frames.stream;
  }

  /// Lässt den jüngsten Strom enden, ohne Exit-Code.
  void endStream() => unawaited(frames.close());
}

ReconnectClient running() {
  final ReconnectClient client = ReconnectClient();
  client.sandbox = client.sandbox.copyWith(
    state: SandboxState.running,
    agentRunning: true,
    startedAt: sandboxTestNow,
    sandboxId: FakeDaemonClient.defaultSandbox,
  );
  return client;
}

TerminalSessionState sessionOf(WidgetTester tester, ReconnectClient client) =>
    ProviderScope.containerOf(tester.element(find.byType(TerminalPane)))
        .read(terminalSessionProvider(client.sandbox.sandboxId!.value));

Future<void> attached(WidgetTester tester, ReconnectClient client) async {
  await pumpSandbox(tester, client: client);
  await tester.pump();
  client.frames.add(const TerminalGeometry(cols: 80, rows: 24));
  await tester.pump();
}

Future<void> waitOut(WidgetTester tester) async {
  for (int i = 0; i < 4; i++) {
    await tester.pump(const Duration(milliseconds: 600));
  }
}

void main() {
  testWidgets('reconnect_keeps_buffer', (WidgetTester tester) async {
    final ReconnectClient client = running();
    await attached(tester, client);
    expect(client.opens, hasLength(1));
    client.frames.add(
      TerminalOutput(
        Uint8List.fromList(utf8.encode('hello from the agent\r\n')),
      ),
    );
    await tester.pump();

    client.endStream();
    await tester.pump();
    final TerminalSessionState broken = sessionOf(tester, client);
    expect(broken.phase, TerminalPhase.detached);
    expect(broken.reconnecting, isTrue);
    expect(
      broken.terminal.buffer.getText(),
      contains('[Humanitl] Connection to the terminal lost, reconnecting …'),
    );

    await waitOut(tester);
    expect(client.opens, hasLength(2), reason: 'attached again');
    final TerminalOpen again = client.opens.last;
    expect(again.cols, broken.terminal.viewWidth, reason: 'the geometry now');
    expect(again.rows, broken.terminal.viewHeight);

    client.frames.add(const TerminalGeometry(cols: 80, rows: 24));
    await tester.pump();
    final TerminalSessionState back = sessionOf(tester, client);
    expect(back.phase, TerminalPhase.attached);
    expect(back.reconnecting, isFalse);
    expect(identical(back.terminal, broken.terminal), isTrue);
    expect(back.terminal.buffer.getText(), contains('hello from the agent'));
  });

  testWidgets('a detach ends the attempt, and the next break gets its line', (
    WidgetTester tester,
  ) async {
    final ReconnectClient client = running();
    await attached(tester, client);
    client.endStream();
    await tester.pump();
    expect(sessionOf(tester, client).reconnecting, isTrue);

    final TerminalSession session = ProviderScope.containerOf(
      tester.element(find.byType(TerminalPane)),
    ).read(terminalSessionProvider(client.sandbox.sandboxId!.value).notifier);
    await session.detach();
    await tester.pump();

    // Ohne das bliebe `reconnecting` stehen, und der nächste Abriss bekäme
    // keine Zeile im Terminal (Review).
    expect(sessionOf(tester, client).reconnecting, isFalse);
  });

  testWidgets('an agent that ended is not attached again', (
    WidgetTester tester,
  ) async {
    final ReconnectClient client = running();
    await attached(tester, client);
    client.frames.add(const TerminalExit(0));
    await tester.pump();
    client.endStream();
    await waitOut(tester);

    expect(client.opens, hasLength(1));
    expect(sessionOf(tester, client).reconnecting, isFalse);
  });

  testWidgets('a refusal of the daemon is not tried again', (
    WidgetTester tester,
  ) async {
    final ReconnectClient client = running();
    await attached(tester, client);
    client.frames.add(
      const TerminalFinding(
        Diagnostic(
          code: 'TERM_002',
          severity: Severity.error,
          why: 'the sandbox has no terminal',
        ),
      ),
    );
    await tester.pump();
    client.endStream();
    await waitOut(tester);

    expect(client.opens, hasLength(1));
    expect(sessionOf(tester, client).phase, TerminalPhase.refused);
  });
}
