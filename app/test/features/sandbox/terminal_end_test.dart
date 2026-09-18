// Wie ein Terminal endet, und was danach im Fenster steht (HUM-136).
//
// Drei Enden, und bis zu diesem Issue sagten zwei davon nichts: Der Agent
// endet mit einem Code, der Daemon schließt den Strom, oder die Verbindung
// bricht. Die letzten beiden fielen stillschweigend auf `idle` zurück -- ein
// Fenster, das verstummt, und ein Mensch, der nichts zu melden hat.

import 'dart:async';

import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_diagnostics.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/sandbox/providers/terminal_provider.dart';
import 'package:humanitl/features/sandbox/widgets/terminal_pane.dart';

import 'harness.dart';

/// Ein Client, dessen Terminal-Strom dem Test gehört.
///
/// Der Fake des Repositories hält seinen Strom selbst offen; für dieses Issue
/// muss ein Test ihn enden lassen können, ohne dass ein Agent dabei endet.
class _ControlledClient extends SandboxTestClient {
  /// Der Strom des jüngsten Anschlusses.
  ///
  /// Jeder Anschluss bekommt einen eigenen, wie beim echten Client ein
  /// eigener Aufruf: Ein Test, der sich zweimal anhängt, soll den zweiten
  /// Strom steuern können und nicht an einem schon geschlossenen scheitern.
  StreamController<TerminalFrame> frames = StreamController<TerminalFrame>();

  /// Wie oft sich ein Terminal angehängt hat.
  int attachments = 0;

  StreamSubscription<TerminalCommand>? _commands;

  @override
  Stream<TerminalFrame> terminal(Stream<TerminalCommand> input) {
    _commands = input.listen((TerminalCommand _) {});
    if (attachments > 0) {
      frames = StreamController<TerminalFrame>();
    }
    attachments++;
    return frames.stream;
  }

  /// Die Geometrie ist die erste Antwort des Daemons; erst damit steht der
  /// Strom, und erst dann ist die Phase attached.
  Future<void> open(WidgetTester tester) async {
    frames.add(const TerminalGeometry(cols: 80, rows: 24));
    await tester.pump();
  }

  /// Lässt den Strom enden, ohne auf ihn zu warten.
  ///
  /// Nicht `close`: So heißt die Methode, mit der die Anwendung den ganzen
  /// Client abräumt. Sie zu überschreiben hieße, dem Bildschirm das Aufräumen
  /// wegzunehmen, und der Test hing daran, bis er gemessen wurde.
  ///
  /// Kein `await`: Die Futures eines `StreamController` laufen in der
  /// gefälschten Zeit eines Widget-Tests nicht von selbst weiter, und ein
  /// `await` darauf hängt den Test, statt ihn zu messen. Was zugestellt wird,
  /// stellt der nächste `pump` zu.
  void endStream() {
    unawaited(_commands?.cancel() ?? Future<void>.value());
    unawaited(frames.close());
  }
}

_ControlledClient _controlledClient() {
  final _ControlledClient client = _ControlledClient();
  client.sandbox = client.sandbox.copyWith(
    state: SandboxState.running,
    agentRunning: true,
    startedAt: sandboxTestNow,
    sandboxId: FakeDaemonClient.defaultSandbox,
  );
  return client;
}

TerminalSession _notifier(WidgetTester tester, SandboxTestClient client) {
  final ProviderContainer container = ProviderScope.containerOf(
    tester.element(find.byType(TerminalPane)),
  );
  return container.read(
    terminalSessionProvider(client.sandbox.sandboxId!.value).notifier,
  );
}

void main() {
  testWidgets('a_stream_that_ends_without_an_exit_leaves_the_phase_detached', (
    WidgetTester tester,
  ) async {
    final _ControlledClient client = _controlledClient();
    await pumpSandbox(tester, client: client);
    await tester.pump();
    await client.open(tester);
    expect(_notifier(tester, client).state.phase, TerminalPhase.attached);

    client.endStream();
    await tester.pump();

    expect(
      _notifier(tester, client).state.phase,
      TerminalPhase.detached,
      reason: 'nicht idle: idle heißt, es lief nie etwas',
    );
  });

  testWidgets('a_broken_connection_leaves_the_phase_detached', (
    WidgetTester tester,
  ) async {
    final _ControlledClient client = _controlledClient();
    await pumpSandbox(tester, client: client);
    await tester.pump();
    await client.open(tester);

    // Genau das, was `GrpcDaemonClient.terminal` wirft, wenn die Leitung
    // bricht: eine `DaemonException` mit `DAEMON_001`. Ein Test, der hier
    // irgendeine andere Ausnahme schickt, misst eine Form, die kein Client
    // erzeugt -- und verfehlt damit den Fall, für den dieses Issue existiert.
    client.frames.addError(
      DaemonException(
        ClientDiagnostics.daemonUnreachable(
          socketPath: '/run/user/1000/humanitl/daemon.sock',
          detail: 'UNAVAILABLE: broken pipe',
        ),
      ),
    );
    await tester.pump();

    expect(_notifier(tester, client).state.phase, TerminalPhase.detached);
    expect(find.byKey(const Key('sandbox-terminal-detached')), findsOneWidget);
    expect(find.textContaining('humanitl sandbox attach'), findsOneWidget);

    // `DAEMON_001` heißt: Es antwortet kein Daemon mehr -- und die Sandbox
    // lebt in diesem Daemon. Wer hier schriebe, die Sitzung laufe weiter,
    // behauptete etwas über den Rechner des Nutzers, das diese Anwendung
    // nicht wissen kann (`backlog/CONVENTIONS.md` 4.13).
    expect(
      find.textContaining('The session keeps running'),
      findsNothing,
      reason: 'ohne Daemon weiß niemand, was aus der Sitzung wurde',
    );
    expect(find.textContaining('is unknown'), findsOneWidget);
  });

  testWidgets('the_first_geometry_clears_a_stale_daemon_unreachable', (
    WidgetTester tester,
  ) async {
    // `DAEMON_001` heißt „es antwortet kein Daemon". Wer sich danach wieder
    // anhängt und eine Geometrie bekommt, hat die Antwort des Daemons in der
    // Hand -- stünde der Befund dann weiter da, behauptete das Fenster über
    // einem lebenden Terminal, es gebe keinen Daemon, und nach einem
    // gewöhnlichen Ende später, die Verbindung sei fort.
    final _ControlledClient client = _controlledClient();
    await pumpSandbox(tester, client: client);
    await tester.pump();
    await client.open(tester);

    // Der erste Anschluss bricht mit `DAEMON_001`, und wie beim echten
    // Client endet der Strom mit dem Fehler.
    client.frames.addError(
      DaemonException(
        ClientDiagnostics.daemonUnreachable(
          socketPath: '/run/user/1000/humanitl/daemon.sock',
        ),
      ),
    );
    client.endStream();
    await tester.pump();
    await tester.pump();
    expect(find.textContaining('is unknown'), findsOneWidget);

    // Ein zweiter Anschluss. Der Versuch allein ist noch keine Antwort: Bis
    // der Daemon etwas schickt, bleibt „keine Verbindung" die letzte wahre
    // Aussage. Räumte das Anhängen den Befund, stünde ein Fenster, dessen
    // Daemon die Verbindung annimmt und dann schweigt, ohne ein Wort da.
    await _notifier(tester, client).attach();
    await tester.pump();
    expect(client.attachments, 2, reason: 'ein zweiter Anschluss');
    expect(
      _notifier(tester, client).state.diagnostic?.code,
      DiagnosticCodes.daemonUnreachable,
      reason: 'vor der ersten Antwort gilt der alte Befund',
    );
    expect(find.textContaining('DAEMON_001'), findsOneWidget);

    // Der Daemon antwortet: die erste Geometrie. Zwei Bilder -- das erste
    // stellt sie zu, das zweite baut die Kachel mit dem geräumten Zustand.
    await client.open(tester);
    await tester.pump();

    final TerminalSessionState live = _notifier(tester, client).state;
    expect(live.phase, TerminalPhase.attached);
    expect(
      live.diagnostic,
      isNull,
      reason: 'ein neuer Anschluss widerlegt „es antwortet kein Daemon"',
    );
    expect(find.textContaining('DAEMON_001'), findsNothing);

    // Und endet der zweite Strom gewöhnlich, sagt das Fenster nicht, die
    // Verbindung zum Daemon sei fort.
    client.endStream();
    await tester.pump();
    await tester.pump();
    expect(find.textContaining('is unknown'), findsNothing);
    expect(
      find.textContaining('if the session is still running'),
      findsOneWidget,
    );
  });

  testWidgets('a_refusal_of_the_daemon_is_not_a_broken_connection', (
    WidgetTester tester,
  ) async {
    final _ControlledClient client = _controlledClient();
    await pumpSandbox(tester, client: client);
    await tester.pump();
    await client.open(tester);

    // `IPC_001` ist der einzige andere Code, der auf diesem Weg als Status
    // ankommt: Der Token-Interceptor lehnt den Client ab, bevor der Strom
    // steht. Das ist eine Aussage über diesen Client und keine über die
    // Leitung, also bleibt es ein Befund -- ein Weg zurück wäre dort eine
    // Lüge, denn derselbe Token wird auch beim nächsten Versuch abgewiesen.
    //
    // `TERM_001` steht hier ausdrücklich **nicht**: Der Daemon schickt den
    // als gewöhnliches Frame, nicht als Fehler, und `_onFrame` behandelt ihn.
    // Ein Test, der ihn hier einspeiste, misst eine Form, die es nicht gibt.
    client.frames.addError(
      DaemonException(
        ClientDiagnostics.tokenRejected(
          tokenPath: '/run/user/1000/humanitl/token',
        ),
      ),
    );
    await tester.pump();

    expect(_notifier(tester, client).state.phase, TerminalPhase.refused);
    expect(find.byKey(const Key('sandbox-terminal-detached')), findsNothing);
  });

  testWidgets('a_stream_that_never_opened_is_not_silence', (
    WidgetTester tester,
  ) async {
    // Der früheste Fehlschlag von allen: Der Strom endet, bevor die erste
    // Geometrie ankommt. Hinge der Übergang an `attached`, bliebe genau
    // dieser Fall stumm -- ein Fenster ohne Terminal und ohne ein Wort dazu.
    final _ControlledClient client = _controlledClient();
    await pumpSandbox(tester, client: client);
    await tester.pump();
    expect(_notifier(tester, client).state.phase, TerminalPhase.idle);

    client.endStream();
    // Zwei Bilder: Das erste stellt das Ende des Stroms zu, das zweite baut
    // die Kachel damit neu.
    await tester.pump();
    await tester.pump();

    expect(_notifier(tester, client).state.phase, TerminalPhase.detached);
    expect(find.byKey(const Key('sandbox-terminal-detached')), findsOneWidget);
  });

  testWidgets('an_error_without_a_finding_still_ends_as_detached', (
    WidgetTester tester,
  ) async {
    // Der Rückfall. Kein Client von heute wirft so etwas; die Verzweigung
    // steht trotzdem da, weil ein Strom, der ohne Befund abbricht, dieselbe
    // Frage stellt wie einer mit.
    final _ControlledClient client = _controlledClient();
    await pumpSandbox(tester, client: client);
    await tester.pump();
    await client.open(tester);

    client.frames.addError(const _NoFinding());
    await tester.pump();

    expect(_notifier(tester, client).state.phase, TerminalPhase.detached);
  });

  testWidgets('the_window_explains_the_end_and_names_the_way_back', (
    WidgetTester tester,
  ) async {
    final _ControlledClient client = _controlledClient();
    await pumpSandbox(tester, client: client);
    await tester.pump();
    await client.open(tester);
    expect(
      find.byKey(const Key('sandbox-terminal-detached')),
      findsNothing,
      reason: 'solange der Strom steht, gibt es nichts zu erklären',
    );

    client.endStream();
    await tester.pump();

    expect(find.byKey(const Key('sandbox-terminal-detached')), findsOneWidget);
    expect(
      find.textContaining('The stream ended without an exit code'),
      findsOneWidget,
    );
    expect(
      find.textContaining('humanitl sandbox attach'),
      findsOneWidget,
      reason: 'der Weg zurück wird genannt, nicht nur der Verlust',
    );
  });

  testWidgets('an_exit_code_wins_over_the_explanation', (
    WidgetTester tester,
  ) async {
    final _ControlledClient client = _controlledClient();
    await pumpSandbox(tester, client: client);
    await tester.pump();
    await client.open(tester);

    final TerminalSession session = _notifier(tester, client);
    session.state = session.state.copyWith(
      phase: TerminalPhase.detached,
      exitCode: 3,
    );
    await tester.pump();

    expect(
      find.byKey(const Key('sandbox-terminal-exit')),
      findsOneWidget,
      reason: 'der Code ist die genauere Auskunft',
    );
    expect(find.byKey(const Key('sandbox-terminal-detached')), findsNothing);
  });

  testWidgets('the_finding_of_the_daemon_stays_above_the_terminal', (
    WidgetTester tester,
  ) async {
    final _ControlledClient client = _controlledClient();
    await pumpSandbox(tester, client: client);
    await tester.pump();
    await client.open(tester);

    final TerminalSession session = _notifier(tester, client);
    session.state = session.state.copyWith(diagnostic: blockingFinding);
    client.endStream();
    await tester.pump();

    expect(_notifier(tester, client).state.phase, TerminalPhase.detached);
    expect(find.textContaining('SANDBOX_001'), findsOneWidget);
    expect(find.byKey(const Key('sandbox-terminal-detached')), findsOneWidget);
  });
}

/// Eine Ausnahme ohne Befund, wie sie heute kein Client wirft.
class _NoFinding implements Exception {
  const _NoFinding();

  @override
  String toString() => 'connection closed';
}
