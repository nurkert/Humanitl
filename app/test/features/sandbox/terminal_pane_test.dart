// Das Terminal des Sandbox-Bildschirms (HUM-042). Jeder Test prüft eine
// Zusage, die der Bildschirm über fremde Bytes macht: dass der Hinweis über
// dem Terminal steht, dass Tastendrücke den Weg hinauf nehmen, dass ein
// gehaltener Fluss außerhalb des Emulators sichtbar wird und dass ein zweiter
// Schreiber den Befund des Daemons zu sehen bekommt.

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/flow_events.dart';
import 'package:humanitl/core/shortcuts/intents.dart';
import 'package:humanitl/features/sandbox/providers/terminal_provider.dart';
import 'package:humanitl/features/sandbox/widgets/terminal_pane.dart';
import 'package:humanitl_ui/humanitl_ui.dart';
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

  /// Die Tasten, die kein Buchstabe sind, kommen beim Agenten an (HUM-042).
  ///
  /// **Warum dieser Test neben dem darüber steht.** Der andere ruft
  /// `terminal.onOutput` selbst auf und misst damit die Leitung vom Emulator
  /// zum Dienst. Ein Mensch drückt aber Tasten, und Rücktaste, Pfeil, Enter
  /// und jedes `Ctrl`-Kürzel nehmen im Emulator einen anderen Weg als ein
  /// Buchstabe: nicht die Texteingabe, sondern die Tastenbehandlung. Am
  /// 2026-09-07 hat ein Mensch am Bildschirm gemeldet, dass er in OpenCode
  /// nichts löschen kann und `Ctrl+P` nicht ankommt -- und kein Test hat es
  /// gesehen.
  testWidgets('every_key_that_is_not_a_letter_reaches_the_agent', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = runningClient();
    await pumpSandbox(tester, client: client);
    await tester.pump();
    await tester.pump();
    expect(_session(tester, client).phase, TerminalPhase.attached);

    await tester.tap(find.byType(TerminalView));
    await tester.pump();

    for (final (String what, LogicalKeyboardKey key, List<int> bytes)
        in <(String, LogicalKeyboardKey, List<int>)>[
          // Rücktaste ist `DEL`, wie im Terminal seit je.
          ('backspace', LogicalKeyboardKey.backspace, <int>[0x7f]),
          ('enter', LogicalKeyboardKey.enter, <int>[0x0d]),
          ('arrow up', LogicalKeyboardKey.arrowUp, <int>[0x1b, 0x5b, 0x41]),
        ]) {
      client.typed.clear();
      await tester.sendKeyEvent(key);
      await tester.pump();
      expect(client.typed, bytes, reason: '$what reaches the agent');
    }

    // Und die Kürzel, die dem Agenten gehören und nicht der Anwendung:
    // `Ctrl+C` als Byte 0x03, `Ctrl+P` als 0x10, `Ctrl+A` als 0x01.
    for (final (String what, LogicalKeyboardKey key, int byte)
        in <(String, LogicalKeyboardKey, int)>[
          ('ctrl+c', LogicalKeyboardKey.keyC, 0x03),
          ('ctrl+p', LogicalKeyboardKey.keyP, 0x10),
          // `Ctrl+A` ist der Zeilenanfang in readline und in bubbletea, also
          // auch in OpenCode. Der Emulator bindet die Taste in seiner Vorgabe
          // auf „alles auswählen" und prüft seine Kürzel vor der Übersetzung
          // in Bytes; ohne den beschnittenen Vorrat käme sie nirgends an.
          ('ctrl+a', LogicalKeyboardKey.keyA, 0x01),
        ]) {
      client.typed.clear();
      await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
      await tester.sendKeyEvent(key);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
      await tester.pump();
      expect(client.typed, <int>[byte], reason: '$what reaches the agent');
    }
  });

  /// Die Kürzel der Anwendung überleben den Emulator (HUM-042, HUM-019).
  ///
  /// Die Gegenprobe zum Test darüber: Weil das Terminal jetzt jede Taste
  /// selbst behandeln darf, müssen die Kürzel der Anwendung trotzdem oben
  /// ankommen und nicht beim Agenten landen. Von allein täte das nur `Ctrl+1`
  /// bis `Ctrl+5`; `Ctrl+6` ist für den Emulator `0x1e` und `Ctrl+K` ist
  /// `0x0b`, und was er kennt, meldet er als behandelt. Ohne diese Kürzel käme
  /// man aus einem Vollbild-TUI nur noch mit der Maus heraus.
  testWidgets('a_shortcut_of_the_application_survives_the_terminal', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = runningClient();
    final List<int> navigated = <int>[];
    int palettes = 0;
    await pumpSandbox(
      tester,
      client: client,
      wrap: (Widget child) => Shortcuts(
        shortcuts: shellShortcuts(),
        child: Actions(
          actions: <Type, Action<Intent>>{
            NavIntent: CallbackAction<NavIntent>(
              onInvoke: (NavIntent intent) {
                navigated.add(intent.index);
                return null;
              },
            ),
            PaletteIntent: CallbackAction<PaletteIntent>(
              onInvoke: (PaletteIntent intent) {
                palettes++;
                return null;
              },
            ),
          },
          child: child,
        ),
      ),
    );
    await tester.pump();
    await tester.pump();
    expect(_session(tester, client).phase, TerminalPhase.attached);

    await tester.tap(find.byType(TerminalView));
    await tester.pump();

    // Drei Tasten, drei Wege durch den Emulator: `Ctrl+1` kennt er nicht,
    // `Ctrl+6` ist für ihn `0x1e` und `Ctrl+K` ist `0x0b`. Nur die erste käme
    // von allein oben an; die anderen beiden sind der Grund, warum `_onKey`
    // die Absicht selbst auslöst.
    for (final (String what, LogicalKeyboardKey key)
        in <(String, LogicalKeyboardKey)>[
          ('ctrl+1', LogicalKeyboardKey.digit1),
          ('ctrl+6', LogicalKeyboardKey.digit6),
          ('ctrl+k', LogicalKeyboardKey.keyK),
        ]) {
      client.typed.clear();
      await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
      await tester.sendKeyEvent(key);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
      await tester.pump();
      expect(client.typed, isEmpty, reason: '$what never reaches the agent');
    }

    expect(navigated, <int>[
      0,
      5,
    ], reason: 'both section shortcuts reached the application');
    expect(palettes, 1, reason: 'and the palette opened');
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

  /// Die Hälfte dieser Zusage, die im UI liegt (HUM-042).
  ///
  /// Der Weg vom Fenster zum Agenten hat zwei Hälften. Die untere ist im
  /// Daemon gemessen (`a_resize_reaches_the_agent` in
  /// `daemon/crates/ipc/tests/terminal.rs`: der Agent meldet nach einem Resize
  /// `SIZE 43 132`). Die obere steht hier: Was der Emulator über seine neue
  /// Größe meldet, geht als `TerminalResize` hinauf — und nicht nur in einen
  /// Rückruf, den niemand liest. Der Fake antwortet auf ein `TerminalResize`
  /// mit der Geometrie, die er verstanden hat, und erst die trägt die Zahlen
  /// in den Zustand.
  testWidgets('a_resize_of_the_window_goes_up_as_a_command', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = runningClient();
    await pumpSandbox(tester, client: client);
    await tester.pump();
    await tester.pump();

    final TerminalSessionState before = _session(tester, client);
    expect(before.phase, TerminalPhase.attached);
    // Die Zahlen, die gleich kommen, stehen vorher nirgends.
    expect(before.cols, isNot(132));
    expect(before.rows, isNot(43));

    before.terminal.resize(132, 43);
    await tester.pump();
    await tester.pump();

    final TerminalSessionState after = _session(tester, client);
    expect(
      (after.cols, after.rows),
      (132, 43),
      reason: 'the daemon answered the resize with the geometry it understood',
    );
  });

  /// Läuft die Kommandozeile, sieht das Fenster zu (HUM-067, Kriterium 4).
  ///
  /// `humanitl run` hält den Schreibplatz der Sitzung. Ein Fenster, das sich
  /// daneben öffnet, bekam bisher `TERM_001` und zeigte einen toten Streifen —
  /// dabei ist das der Normalfall und kein Fehlschlag. Jetzt hängt es sich
  /// lesend an: dieselbe Ausgabe, keine Tastatur, und der Grund steht dabei.
  testWidgets('a_window_next_to_a_writing_command_line_watches_read_only', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = runningClient();

    // Der Platz des Schreibers ist vergeben, bevor das Fenster aufgeht: Das
    // ist die Kommandozeile, die die Sitzung gestartet hat.
    final StreamController<TerminalCommand> cli =
        StreamController<TerminalCommand>();
    addTearDown(cli.close);
    final StreamSubscription<TerminalFrame> held = client
        .terminal(cli.stream)
        .listen((TerminalFrame _) {});
    addTearDown(held.cancel);
    cli.add(
      TerminalOpen(
        sandboxId: client.sandbox.sandboxId!.value,
        cols: 80,
        rows: 24,
      ),
    );
    await tester.pump();

    await pumpSandbox(tester, client: client);
    await tester.pump();
    await tester.pump();
    await tester.pump();

    final TerminalSessionState session = _session(tester, client);
    expect(
      session.phase,
      TerminalPhase.attached,
      reason: 'the window watches instead of showing a dead strip',
    );
    expect(session.readOnly, isTrue, reason: 'and it gave up the keyboard');
    expect(
      session.diagnostic?.code,
      DiagnosticCodes.terminalSecondWriter,
      reason: 'the reason stays visible: somebody else is typing',
    );
    // Und es sieht, was der Agent schreibt.
    expect(session.terminal.buffer.getText(), contains(fakeGreeting));

    // Und der Streifen darüber erklärt, statt einen Befund zu melden: Dass
    // jemand anders tippt, ist kein Zustand einer Anfrage, also trägt die
    // Zeile das ruhige Chrome und keine Zustandsfarbe (`docs/UX.md` 5 und 6).
    expect(find.byKey(const Key('sandbox-terminal-watching')), findsOneWidget);
    expect(find.byKey(const Key('sandbox-terminal-finding')), findsNothing);
    final HTokens tokens = HTheme.of(
      tester.element(find.byKey(const Key('sandbox-terminal-watching'))),
    );
    final Container strip = tester.widget<Container>(
      find.byKey(const Key('sandbox-terminal-watching')),
    );
    expect(strip.color, tokens.colors.bg2);
    expect(strip.color, isNot(HColorDerivation.tint(tokens.state.error)));
    expect(strip.color, isNot(HColorDerivation.tint(tokens.state.held)));
    expect(find.textContaining('watching'), findsOneWidget);

    // Und einfügen geht nicht, solange dieses Fenster zusieht: `Terminal.paste`
    // ruft `onOutput` auch in einer nur lesenden Ansicht, die Bytes gingen
    // hinauf, und der Daemon verwürfe sie -- ein Menüpunkt, der nichts tut.
    // Die Stelle ist wichtig: hier ist die Phase `attached`, also misst die
    // Zusicherung `readOnly` und nicht die Phase.
    final HContextMenu menu = tester.widget<HContextMenu>(
      find.byType(HContextMenu),
    );
    final HMenuItem paste = menu.itemsBuilder().firstWhere(
      (HMenuItem item) => item.label == 'Paste',
    );
    expect(paste.enabled, isFalse);

    // Und wenn die Sitzung endet, kommt der Befund nicht zurück: `TERM_001`
    // bleibt im Zustand stehen, aber er gehört einer Lage, die vorbei ist —
    // orange neben der Exit-Zeile wäre eine Meldung über etwas, das gerade
    // gutgegangen ist.
    client.endTerminals();
    await tester.pump();
    await tester.pump();
    expect(
      _session(tester, client).phase,
      TerminalPhase.ended,
      reason: 'the agent ended while this window was watching',
    );
    expect(
      find.byKey(const Key('sandbox-terminal-finding')),
      findsNothing,
      reason: 'and TERM_001 belongs to a situation that is over',
    );
    expect(find.byKey(const Key('sandbox-terminal-watching')), findsNothing);
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
