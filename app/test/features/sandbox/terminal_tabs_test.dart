// Weitere Shells als Reiter des Sandbox-Terminals (HUM-314).
//
// Jeder Test prüft eine Zusage aus `backlog/sprint-5.md`, Abschnitt HUM-314:
// „+ Shell" nur mit `sandbox.exec` und laufender Sitzung, ein Reiter je Shell
// an genau ihrer Kennung, `TERM_002` als Hinweis über dem Terminal, die
// Grenze von vier, der Exit-Code am Reiter, das Ende mit der Sitzung, die
// Tasten, der einmalige Hinweis und das schwebende Fenster, das den aktiven
// Reiter zeigt. Gegen den Fake-Client, nie gegen den Socket des Daemons.
//
// Linux als Plattform: Die Griffe sind die eines Linux-Desktops.

import 'dart:async';
import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/terminal_float.dart'
    show terminalFloatCornerKey, terminalFloatSizeKey;
import 'package:humanitl/core/ui_state/ui_state_file.dart';
import 'package:humanitl/features/sandbox/providers/sandbox_status_provider.dart';
import 'package:humanitl/features/sandbox/providers/terminal_provider.dart';
import 'package:humanitl/features/sandbox/providers/terminal_tabs_provider.dart';
import 'package:humanitl/features/sandbox/widgets/terminal_pane.dart';
import 'package:humanitl/features/shell/widgets/floating_terminal.dart'
    show FloatingTerminalLayer;
import 'package:humanitl_ui/humanitl_ui.dart' show HButton, HKeepClear;
import 'package:xterm2/ui.dart';

import '../../harness/app_harness.dart';
import '../shell/terminal_float_test.dart'
    show floating, pressFloat, stateFile, window;
import '../shell/terminal_full_window_test.dart' show settle;
import 'harness.dart';

final TargetPlatformVariant _linux = TargetPlatformVariant.only(
  TargetPlatform.linux,
);

Finder _key(Key key) => find.byKey(key);

final Finder _strip = _key(const Key('sandbox-terminal-tabs'));
final Finder _newShell = _key(const Key('sandbox-terminal-new-shell'));
final Finder _agentTab = _key(const Key('sandbox-terminal-tab-agent'));
final Finder _refusal = _key(const Key('sandbox-terminal-shell-refusal'));
final Finder _hint = _key(const Key('sandbox-terminal-shell-hint'));

Finder _tab(String id) => _key(ValueKey<String>('sandbox-terminal-tab-$id'));

Finder _close(String id) =>
    _key(ValueKey<String>('sandbox-terminal-tab-close-$id'));

/// Eine laufende Sitzung, deren Daemon weitere Shells anbietet, wenn
/// [offers] wahr ist, und die jedes `Open` mitschreibt.
class ShellClient extends SandboxTestClient {
  ShellClient({this.offers = true}) {
    sandbox = runningClient().sandbox;
  }

  /// Ob `Info.capabilities` `sandbox.exec` nennt.
  final bool offers;

  /// Die Shell-Kennung jedes `Open`, leer für das Terminal des Agenten.
  final List<String> opens = <String>[];

  /// What `Sandbox(Status)` fails with instead of answering, when set.
  Diagnostic? statusFailure;

  @override
  Stream<SandboxUpdate> sandboxStatus() {
    if (statusFailure case final Diagnostic failure) {
      return Stream<SandboxUpdate>.error(DaemonException(failure));
    }
    return super.sandboxStatus();
  }

  @override
  Stream<SandboxUpdate> checkIsolation() {
    if (statusFailure case final Diagnostic failure) {
      return Stream<SandboxUpdate>.error(DaemonException(failure));
    }
    return super.checkIsolation();
  }

  @override
  Future<DaemonInfo> getInfo() async {
    final DaemonInfo info = await super.getInfo();
    return offers
        ? info.copyWith(
            capabilities: <String>[
              ...info.capabilities,
              DaemonInfo.execCapability,
            ],
          )
        : info;
  }

  @override
  Stream<TerminalFrame> terminal(Stream<TerminalCommand> input) =>
      super.terminal(
        input.map((TerminalCommand command) {
          if (command is TerminalOpen) {
            opens.add(command.shellId);
          }
          return command;
        }),
      );
}

ProviderContainer _container(WidgetTester tester) =>
    ProviderScope.containerOf(tester.element(find.byType(TerminalPane)));

TerminalTabsState _tabs(WidgetTester tester) =>
    _container(tester).read(terminalTabsProvider);

/// Der Pane, der gerade zu sehen ist.
TerminalPane _pane(WidgetTester tester) =>
    tester.widget<TerminalPane>(find.byType(TerminalPane));

Future<ShellClient> _pumpRunning(
  WidgetTester tester, {
  bool offers = true,
  Override? uiState,
}) async {
  final ShellClient client = ShellClient(offers: offers);
  await pumpSandbox(tester, client: client, uiState: uiState);
  await settle(tester);
  return client;
}

Future<void> _openShell(WidgetTester tester) async {
  await tester.tap(_newShell);
  await settle(tester);
}

/// Ein Griff mit `Ctrl` und wahlweise `Shift`, als Linux-Ereignisse.
Future<void> _chord(
  WidgetTester tester,
  LogicalKeyboardKey key, {
  bool shift = false,
}) async {
  await tester.sendKeyDownEvent(
    LogicalKeyboardKey.controlLeft,
    platform: 'linux',
  );
  if (shift) {
    await tester.sendKeyDownEvent(
      LogicalKeyboardKey.shiftLeft,
      platform: 'linux',
    );
  }
  await tester.sendKeyEvent(key, platform: 'linux');
  if (shift) {
    await tester.sendKeyUpEvent(
      LogicalKeyboardKey.shiftLeft,
      platform: 'linux',
    );
  }
  await tester.sendKeyUpEvent(
    LogicalKeyboardKey.controlLeft,
    platform: 'linux',
  );
  await settle(tester);
}

/// Gibt dem Terminal, das gerade zu sehen ist, die Tastatur.
Future<void> _focusTerminal(WidgetTester tester) async {
  await tester.tap(find.byType(TerminalView));
  await settle(tester);
}

/// Ein Zustandsspeicher in einer eigenen Datei, deren Pfad der Test kennt.
(Override, String) _stateFile() {
  final Directory dir = Directory.systemTemp.createTempSync('humanitl-tabs');
  addTearDown(() => dir.deleteSync(recursive: true));
  final String path = '${dir.path}/${UiStateFile.fileName}';
  return (uiStateFileProvider.overrideWithValue(UiStateFile(path)), path);
}

/// Sechzig Bilder lang steht das schwebende Fenster still, und genau ein
/// Terminal ist gebaut (HUM-352). Dazu hält nichts im Fenster selbst einen
/// Bereich frei: Der Hinweis zur ersten Shell tat es, und das Fenster wich
/// ihm aus, nahm ihn mit und kam im nächsten Bild zurück.
Future<void> expectStill(WidgetTester tester) async {
  final Rect still = tester.getRect(floating);
  expect(find.descendant(of: floating, matching: _hint), findsOneWidget);
  final List<Rect> keepClear = HKeepClear.rectsIn(
    tester.renderObject<RenderBox>(find.byType(FloatingTerminalLayer)),
  );
  expect(
    keepClear.where((Rect zone) => zone.overlaps(still)),
    isEmpty,
    reason: 'the window keeps nothing clear from itself',
  );
  for (int frame = 0; frame < 60; frame++) {
    await tester.pump(const Duration(milliseconds: 16));
    expect(floating, findsOneWidget, reason: 'frame $frame');
    expect(tester.getRect(floating), still, reason: 'frame $frame');
    expect(
      find.byType(TerminalView, skipOffstage: false),
      findsOneWidget,
      reason: 'one host only, frame $frame',
    );
  }
  expect(tester.takeException(), isNull);
}

void main() {
  testWidgets('without_the_capability_there_is_no_strip_and_the_chord_reaches_'
      'the_agent', (WidgetTester tester) async {
    final ShellClient client = await _pumpRunning(tester, offers: false);
    expect(_strip, findsNothing);
    expect(_newShell, findsNothing);

    await _focusTerminal(tester);
    await _chord(tester, LogicalKeyboardKey.keyT, shift: true);
    expect(client.execCalls, isEmpty);
    // Ohne zweiten Reiter gehört `Ctrl+PageDown` dem Agenten.
    client.typed.clear();
    await _chord(tester, LogicalKeyboardKey.pageDown);
    expect(
      client.typed,
      isNotEmpty,
      reason: 'without the capability the chords stay with the agent',
    );
    expect(tester.takeException(), isNull);
  }, variant: _linux);

  testWidgets('without_a_session_there_is_no_new_shell', (
    WidgetTester tester,
  ) async {
    final ShellClient client = ShellClient();
    client.sandbox = SandboxTestClient().sandbox;
    await pumpSandbox(tester, client: client);
    await settle(tester);
    expect(_newShell, findsNothing);
    expect(_strip, findsNothing);
  }, variant: _linux);

  testWidgets('new_shell_opens_a_tab_bound_to_the_shell_the_daemon_named', (
    WidgetTester tester,
  ) async {
    final ShellClient client = await _pumpRunning(tester);
    expect(_strip, findsOneWidget);
    expect(_agentTab, findsOneWidget);
    final TerminalSessionState agent = _container(tester)
        .read(terminalSessionProvider(client.sandbox.sandboxId!.value));

    await _openShell(tester);
    expect(client.execCalls, hasLength(1));
    final (int cols, int rows, List<String> command) = client.execCalls.single;
    expect(command, isEmpty, reason: 'an interactive shell');
    expect(cols, agent.terminal.viewWidth, reason: 'the size of the view');
    expect(rows, agent.terminal.viewHeight);
    expect(cols, greaterThan(0));

    final String id = client.liveShells.single;
    expect(_tab(id), findsOneWidget);
    expect(_pane(tester).shellId, id, reason: 'the new tab is shown');
    expect(client.opens.last, id, reason: 'its Open carries the shell id');
    final TerminalSessionState shell = _container(tester).read(
      terminalSessionProvider(client.sandbox.sandboxId!.value, shellId: id),
    );
    expect(shell.phase, TerminalPhase.attached);
    expect(
      identical(shell.terminal, agent.terminal),
      isFalse,
      reason: 'a shell has an emulator of its own',
    );

    // Zurück zum Agenten: dessen Emulator, kein zweites Öffnen der Shell.
    final int opensBefore = client.opens.length;
    await tester.tap(_agentTab);
    await settle(tester);
    expect(_pane(tester).shellId, isEmpty);
    expect(
      identical(
        tester.widget<TerminalView>(find.byType(TerminalView)).terminal,
        agent.terminal,
      ),
      isTrue,
    );
    await tester.tap(_tab(id));
    await settle(tester);
    expect(_pane(tester).shellId, id);
    expect(
      client.opens.skip(opensBefore).where((String s) => s == id),
      isEmpty,
      reason: 'the shell stays attached while another tab is shown',
    );
    expect(tester.takeException(), isNull);
  }, variant: _linux);

  testWidgets('keys_of_a_shell_tab_go_to_that_shell_only', (
    WidgetTester tester,
  ) async {
    final ShellClient client = await _pumpRunning(tester);
    await _openShell(tester);
    final String id = client.liveShells.single;
    await _focusTerminal(tester);
    final TerminalSessionState agent = _container(tester)
        .read(terminalSessionProvider(client.sandbox.sandboxId!.value));
    final TerminalSessionState shell = _container(tester).read(
      terminalSessionProvider(client.sandbox.sandboxId!.value, shellId: id),
    );
    final String agentBefore = agent.terminal.buffer.getText();
    shell.terminal.textInput('zq');
    await settle(tester);
    expect(shell.terminal.buffer.getText(), contains('zq'));
    expect(agent.terminal.buffer.getText(), agentBefore);
  }, variant: _linux);

  testWidgets('term_002_stands_as_a_notice_and_four_tabs_disable_the_button', (
    WidgetTester tester,
  ) async {
    final ShellClient client = await _pumpRunning(tester);
    // Eine Shell, die dieses Fenster nicht zeigt (etwa aus
    // `humanitl sandbox exec`), zählt im Daemon mit.
    await client.exec(sandboxId: client.sandbox.sandboxId!.value);
    for (int i = 0; i < 3; i++) {
      await _openShell(tester);
    }
    expect(_tabs(tester).shells, hasLength(3));
    expect(_refusal, findsNothing);

    await _openShell(tester);
    expect(_tabs(tester).shells, hasLength(3), reason: 'no fourth tab');
    expect(_refusal, findsOneWidget);
    expect(
      find.descendant(of: _refusal, matching: find.textContaining('TERM_002')),
      findsOneWidget,
    );
    await tester.tap(_key(const Key('sandbox-terminal-shell-refusal-dismiss')));
    await settle(tester);
    expect(_refusal, findsNothing);

    // Die fremde Shell endet; die vierte geht auf, und mit vier Reitern
    // sperrt der Knopf.
    client.endShell(client.liveShells.first);
    await _openShell(tester);
    expect(_tabs(tester).shells, hasLength(4));
    expect(tester.widget<HButton>(_newShell).onPressed, isNull);
    final int calls = client.execCalls.length;
    await _focusTerminal(tester);
    await _chord(tester, LogicalKeyboardKey.keyT, shift: true);
    expect(client.execCalls, hasLength(calls), reason: 'the chord stops too');
    expect(tester.takeException(), isNull);
  }, variant: _linux);

  testWidgets('an_ended_shell_shows_its_exit_code_and_closes_on_click', (
    WidgetTester tester,
  ) async {
    final ShellClient client = await _pumpRunning(tester);
    await _openShell(tester);
    final String id = client.liveShells.single;

    final TerminalSessionProvider session = terminalSessionProvider(
      client.sandbox.sandboxId!.value,
      shellId: id,
    );
    final Object emulator = _container(tester).read(session).terminal;
    client.endShell(id, code: 3);
    await settle(tester);
    expect(
      find.descendant(of: _tab(id), matching: find.text('Shell 1 · exit 3')),
      findsOneWidget,
    );
    expect(find.text('The shell ended with exit code 3.'), findsOneWidget);

    await tester.tap(_tab(id));
    await settle(tester);
    expect(_tab(id), findsNothing);
    expect(_pane(tester).shellId, isEmpty, reason: 'back to the agent');
    // Wer die Sitzung danach liest, bekommt eine neue, nie angeschlossene:
    // Emulator und Rücklauf des geschlossenen Reiters sind fort.
    final TerminalSessionState after = _container(tester).read(session);
    expect(identical(after.terminal, emulator), isFalse);
    expect(after.phase, TerminalPhase.idle);
  }, variant: _linux);

  testWidgets('closing_a_running_shell_tab_detaches_only_this_view', (
    WidgetTester tester,
  ) async {
    final ShellClient client = await _pumpRunning(tester);
    await _openShell(tester);
    await _openShell(tester);
    final List<String> ids = client.liveShells;
    expect(_pane(tester).shellId, ids[1]);
    final int streams = client.openTerminals;

    await tester.tap(_close(ids[1]));
    await settle(tester);
    expect(_tab(ids[1]), findsNothing);
    expect(_pane(tester).shellId, ids[0], reason: 'the tab to the left');
    expect(client.openTerminals, streams - 1, reason: 'its stream ended');
    expect(
      client.liveShells,
      contains(ids[1]),
      reason: 'the daemon offers no call that ends a shell',
    );
  }, variant: _linux);

  testWidgets('a_failed_status_question_keeps_the_shell_tabs', (
    WidgetTester tester,
  ) async {
    final ShellClient client = await _pumpRunning(tester);
    await _openShell(tester);
    await _openShell(tester);
    final List<String> ids = client.liveShells;
    final ProviderContainer container = _container(tester);

    // Die Abfrage scheitert: Das sagt nichts über die Sitzung.
    client.statusFailure = const Diagnostic(
      code: DiagnosticCodes.daemonUnreachable,
      severity: Severity.error,
      why: 'the daemon did not answer this once',
    );
    await container.read(sandboxStatusProvider.notifier).refresh();
    await settle(tester);
    final AsyncValue<SandboxStatus> failed = container.read(
      sandboxStatusProvider,
    );
    expect(failed.hasError, isTrue, reason: 'the question really failed');
    expect(container.read(terminalTabsProvider).shells, hasLength(2));
    expect(container.read(terminalTabsProvider).active, ids[1]);

    // Die nächste Antwort nennt dieselbe Sitzung: Die Reiter sind wieder da.
    client.statusFailure = null;
    await container.read(sandboxStatusProvider.notifier).refresh();
    await settle(tester);
    expect(_tab(ids[0]), findsOneWidget);
    expect(_pane(tester).shellId, ids[1]);
    expect(tester.takeException(), isNull);
  }, variant: _linux);

  testWidgets('a_shell_the_daemon_no_longer_knows_shows_ended_and_closes', (
    WidgetTester tester,
  ) async {
    final ShellClient client = await _pumpRunning(tester);
    await _openShell(tester);
    final String id = client.liveShells.single;
    await tester.tap(_agentTab);
    await settle(tester);
    // Die Shell endete, ohne dass ihr Exit-Code dieses Fenster erreichte,
    // und der Daemon kennt sie nach seiner Frist nicht mehr.
    _container(tester).invalidate(
      terminalSessionProvider(client.sandbox.sandboxId!.value, shellId: id),
    );
    await settle(tester);
    client.endShell(id);
    await tester.tap(_tab(id));
    await settle(tester);
    await settle(tester);
    expect(
      find.descendant(of: _tab(id), matching: find.text('Shell 1 · ended')),
      findsOneWidget,
    );

    await tester.tap(_tab(id));
    await settle(tester);
    expect(_tab(id), findsNothing);
    expect(_pane(tester).shellId, isEmpty);
    expect(tester.takeException(), isNull);
  }, variant: _linux);

  test('the_fake_refuses_a_live_shell_of_another_sandbox', () async {
    final ShellClient client = ShellClient();
    final String id = (await client.exec(
      sandboxId: client.sandbox.sandboxId!.value,
    )).shellId;
    final StreamController<TerminalCommand> input =
        StreamController<TerminalCommand>();
    final Completer<Object> failed = Completer<Object>();
    client
        .terminal(input.stream)
        .listen((TerminalFrame _) {}, onError: failed.complete);
    input.add(
      TerminalOpen(sandboxId: 'other', cols: 80, rows: 24, shellId: id),
    );
    final Object error = await failed.future;
    expect(
      (error as DaemonException).diagnostic.code,
      DiagnosticCodes.capabilityUnavailable,
    );
    await input.close();
  });

  test('the_fake_closes_a_shell_stream_after_its_exit', () async {
    final ShellClient client = ShellClient();
    final String sandbox = client.sandbox.sandboxId!.value;
    final String id = (await client.exec(sandboxId: sandbox)).shellId;
    final StreamController<TerminalCommand> input =
        StreamController<TerminalCommand>();
    final List<TerminalFrame> frames = <TerminalFrame>[];
    final Completer<void> done = Completer<void>();
    client.terminal(input.stream).listen(frames.add, onDone: done.complete);
    input.add(
      TerminalOpen(sandboxId: sandbox, cols: 80, rows: 24, shellId: id),
    );
    await Future<void>.delayed(Duration.zero);
    client.endShell(id, code: 7);
    await done.future;
    expect(frames.last, isA<TerminalExit>());
    // Tasten danach fallen weg, statt in einen geschlossenen Strom zu gehen.
    input.add(TerminalKeys(Uint8List.fromList(<int>[0x61])));
    await Future<void>.delayed(Duration.zero);
    await input.close();
  });

  test('the_end_of_the_session_closes_every_shell_stream_once', () async {
    final ShellClient client = ShellClient();
    final String sandbox = client.sandbox.sandboxId!.value;
    final String id = (await client.exec(sandboxId: sandbox)).shellId;
    final StreamController<TerminalCommand> input =
        StreamController<TerminalCommand>();
    final List<TerminalFrame> frames = <TerminalFrame>[];
    final Completer<void> done = Completer<void>();
    client.terminal(input.stream).listen(frames.add, onDone: done.complete);
    input.add(
      TerminalOpen(sandboxId: sandbox, cols: 80, rows: 24, shellId: id),
    );
    await Future<void>.delayed(Duration.zero);
    client.endTerminals(code: 0);
    await done.future;
    expect(frames.whereType<TerminalExit>(), hasLength(1));
    await input.close();
  });

  testWidgets('the_end_of_the_session_ends_every_shell_tab', (
    WidgetTester tester,
  ) async {
    final ShellClient client = await _pumpRunning(tester);
    await _openShell(tester);
    await _openShell(tester);
    expect(_tabs(tester).shells, hasLength(2));

    client
      ..endTerminals(code: 0)
      ..sandbox = client.sandbox.copyWith(agentRunning: false);
    await _container(tester).read(sandboxStatusProvider.notifier).refresh();
    await settle(tester);
    expect(_tabs(tester).shells, isEmpty);
    expect(_strip, findsNothing);
    expect(_pane(tester).shellId, isEmpty);
    expect(tester.takeException(), isNull);
  }, variant: _linux);

  testWidgets('the_chords_open_a_shell_and_walk_the_tabs', (
    WidgetTester tester,
  ) async {
    final ShellClient client = await _pumpRunning(tester);
    await _focusTerminal(tester);
    client.typed.clear();
    // Mit dem Terminal des Agenten allein gehört `Ctrl+PageDown` ihm.
    await _chord(tester, LogicalKeyboardKey.pageDown);
    expect(client.typed, isNotEmpty);
    expect(_pane(tester).shellId, isEmpty);

    await _chord(tester, LogicalKeyboardKey.keyT, shift: true);
    expect(client.execCalls, hasLength(1));
    final String first = client.liveShells.single;
    expect(_pane(tester).shellId, first);
    expect(
      tester
          .widget<TerminalView>(find.byType(TerminalView))
          .focusNode!
          .hasPrimaryFocus,
      isTrue,
      reason: 'the new tab takes the keyboard',
    );

    await _chord(tester, LogicalKeyboardKey.keyT, shift: true);
    final String second = client.liveShells.last;
    expect(_pane(tester).shellId, second);

    client.typed.clear();
    await _chord(tester, LogicalKeyboardKey.pageDown);
    expect(_pane(tester).shellId, isEmpty, reason: 'around the end');
    await _chord(tester, LogicalKeyboardKey.pageUp);
    expect(_pane(tester).shellId, second);
    await _chord(tester, LogicalKeyboardKey.pageUp);
    expect(_pane(tester).shellId, first);
    expect(client.typed, isEmpty, reason: 'no byte of a chord reached a shell');
    expect(tester.takeException(), isNull);
  }, variant: _linux);

  testWidgets('the_hint_comes_once_with_the_first_shell', (
    WidgetTester tester,
  ) async {
    final (Override uiState, String path) = _stateFile();
    final ShellClient client = await _pumpRunning(tester, uiState: uiState);
    expect(_hint, findsNothing);

    await _openShell(tester);
    expect(_hint, findsOneWidget);
    expect(
      find.text('Do not type secrets here; the agent can read this shell.'),
      findsOneWidget,
    );
    await tester.tap(_key(const Key('sandbox-terminal-shell-hint-dismiss')));
    await settle(tester);
    expect(_hint, findsNothing);
    expect(File(path).readAsStringSync(), contains(shellHintSeenKey));

    await _openShell(tester);
    expect(client.liveShells, hasLength(2));
    expect(_hint, findsNothing, reason: 'once, not per shell');
  }, variant: _linux);

  testWidgets('the_floating_terminal_shows_the_active_tab', (
    WidgetTester tester,
  ) async {
    final ShellClient client = ShellClient();
    await pumpApp(tester, client: client, size: window);
    await pressCtrl(tester, LogicalKeyboardKey.digit4);
    await settle(tester);
    await _openShell(tester);
    final String id = client.liveShells.single;

    // Auf der Sandbox-Seite dockt es an (HUM-315); es schwebt über History.
    await pressFloat(tester);
    await pressCtrl(tester, LogicalKeyboardKey.digit2);
    await settle(tester);
    expect(floating, findsOneWidget);
    final TerminalPane pane = tester.widget<TerminalPane>(
      find.descendant(of: floating, matching: find.byType(TerminalPane)),
    );
    expect(pane.shellId, id);
    expect(
      find.descendant(of: floating, matching: _strip),
      findsOneWidget,
      reason: 'the tabs travel with the terminal',
    );
    expect(tester.takeException(), isNull);
  }, variant: _linux);

  // Owner (HUM-352): Über History, schwebend, groß links oben, „+ Shell" und
  // zurück zum Agenten, danach sprang das Fenster mehrmals je Sekunde
  // zwischen einem großen links oben und einem kleinen rechts oben. Der
  // Hinweis zur ersten Shell steht im schwebenden Fenster selbst und hielt
  // sich als Bereich frei, dem das Fenster ausweichen muss: Es wich sich
  // selbst aus, nahm den Hinweis mit, wurde dabei kleiner, und im nächsten
  // Bild war die gewählte Ecke wieder frei. Dazu blieb die verborgene
  // Sandbox-Seite nach Schließen und erneutem Schweben ein zweiter Wirt.
  testWidgets('a_new_shell_and_a_tab_switch_keep_the_float_still', (
    WidgetTester tester,
  ) async {
    final (Override uiState, String _) = stateFile(<String, Object?>{
      terminalFloatCornerKey: 'topLeft',
      terminalFloatSizeKey: <double>[900, 500],
    });
    await pumpApp(
      tester,
      client: ShellClient(),
      size: window,
      uiState: uiState,
    );
    await pressCtrl(tester, LogicalKeyboardKey.digit2);
    await settle(tester);
    await pressFloat(tester);
    expect(floating, findsOneWidget);

    await tester.tap(find.descendant(of: floating, matching: _newShell));
    await settle(tester);
    expect(find.descendant(of: floating, matching: _hint), findsOneWidget);
    await tester.tap(find.descendant(of: floating, matching: _agentTab));
    await settle(tester);

    await expectStill(tester);

    // Zweiter Weg des Owners: Fenster mit dem X schließen, mit
    // `Ctrl+Shift+P` wieder öffnen und dann den Reiter der Shell wählen.
    await tester.tap(find.byKey(const Key('shell-terminal-floating-close')));
    await settle(tester);
    expect(floating, findsNothing);
    await pressFloat(tester);
    expect(floating, findsOneWidget);
    await tester.tap(
      find.descendant(
        of: floating,
        matching: find.byWidgetPredicate(
          (Widget w) =>
              w.key is ValueKey<String> &&
              (w.key! as ValueKey<String>).value.startsWith(
                'sandbox-terminal-tab-',
              ) &&
              !(w.key! as ValueKey<String>).value.contains('close') &&
              (w.key! as ValueKey<String>).value !=
                  'sandbox-terminal-tab-agent',
        ),
      ),
    );
    await settle(tester);
    await expectStill(tester);
  }, variant: _linux);

  test('the_fake_refuses_an_unknown_shell_with_ipc_006', () async {
    final ShellClient client = ShellClient();
    final StreamController<TerminalCommand> input =
        StreamController<TerminalCommand>();
    final Completer<Object> failed = Completer<Object>();
    client
        .terminal(input.stream)
        .listen((TerminalFrame _) {}, onError: failed.complete);
    input.add(
      const TerminalOpen(sandboxId: '', cols: 80, rows: 24, shellId: 'x'),
    );
    final Object error = await failed.future;
    expect(error, isA<DaemonException>());
    expect(
      (error as DaemonException).diagnostic.code,
      DiagnosticCodes.capabilityUnavailable,
    );
    await input.close();
  });

  test('the_fake_counts_to_four_and_frees_a_place_when_one_ends', () async {
    final FakeDaemonClient client = ShellClient();
    final String sandbox = client.sandbox.sandboxId!.value;
    final List<String> ids = <String>[
      for (int i = 0; i < 4; i++)
        (await client.exec(sandboxId: sandbox)).shellId,
    ];
    await expectLater(
      client.exec(sandboxId: sandbox),
      throwsA(
        isA<DaemonException>().having(
          (DaemonException e) => e.diagnostic.code,
          'code',
          DiagnosticCodes.terminalUnavailable,
        ),
      ),
    );
    client.endShell(ids.first);
    expect((await client.exec(sandboxId: sandbox)).shellId, isNotEmpty);
  });
}
