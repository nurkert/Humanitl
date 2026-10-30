// Das Terminal als kleines Fenster über den Abschnitten, Bild im Bild
// (HUM-300): mit `Ctrl+Shift+P` oder dem Knopf über dem Terminal, dieselbe
// Sitzung, nie über der Warteschlange, Freigeben und Blockieren oder einem
// Hinweis, ziehbar, in der Größe änderbar, und es merkt sich beides.
//
// Linux als Plattform: Ohne Variante spielt `flutter test` Android, und die
// Tasten dieses Produkts sind die eines Linux-Desktops.

import 'dart:convert';
import 'dart:io';

import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/terminal_float.dart';
import 'package:humanitl/core/ui/terminal_placement.dart';
import 'package:humanitl/core/ui_state/ui_state_file.dart';
import 'package:humanitl/features/intercept/providers/coach_mark.dart'
    show coachMarkSeenKey;
import 'package:humanitl/features/intercept/widgets/action_bar.dart';
import 'package:humanitl/features/intercept/widgets/queue_pane.dart';
import 'package:humanitl/features/intercept/widgets/queue_row.dart';
import 'package:humanitl/features/sandbox/providers/sandbox_status_provider.dart';
import 'package:humanitl/features/sandbox/widgets/sandbox_terminal.dart'
    show lastFloatingSessionProvider;
import 'package:humanitl/features/settings/settings_sheet.dart';
import 'package:humanitl/features/shell/shell_screen.dart';
import 'package:humanitl/features/shell/widgets/floating_terminal.dart'
    show terminalFloatLinger;
import 'package:xterm2/ui.dart';

import '../../harness/app_harness.dart';
import '../sandbox/harness.dart';
import 'terminal_full_window_test.dart'
    show headerTitle, pressFullWindow, settle;
import 'terminal_keyboard_test.dart'
    show HeldAndRunningClient, pressFocusTerminal, terminalHasKeyboard;

/// Groß genug, dass das Fenster in seiner Vorgabe neben die drei Panes von
/// Intercept passt.
const Size window = Size(1600, 900);

const Key floatingKey = Key('shell-terminal-floating');

final Finder floating = find.byKey(floatingKey);

/// `Ctrl+Shift+P`.
Future<void> pressFloat(WidgetTester tester) async {
  await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
  await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
  await tester.sendKeyEvent(LogicalKeyboardKey.keyP);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
  await settle(tester);
}

/// Genau eine Ansicht der Sitzung, auch unter den verdeckten Abschnitten:
/// Zwei meldeten beide ihre Größe und nähmen beide Tasten.
void expectOneView() => expect(
  find.byType(TerminalView, skipOffstage: false),
  findsOneWidget,
  reason: 'exactly one view of the session',
);

/// Wo die Platzierung gerade steht.
TerminalPlacement placement(WidgetTester tester) =>
    ProviderScope.containerOf(tester.element(find.byType(ShellScreen)))
        .read(terminalPlacementProvider);

/// Ein Zustandsspeicher in einer eigenen Datei mit [content]; der Hinweis des
/// ersten gehaltenen Requests gilt als gesehen.
(Override, String) stateFile([Map<String, Object?> content = const {}]) {
  final Directory dir = Directory.systemTemp.createTempSync('humanitl-float');
  addTearDown(() => dir.deleteSync(recursive: true));
  final String path = '${dir.path}/${UiStateFile.fileName}';
  File(path).writeAsStringSync(
    jsonEncode(<String, Object?>{coachMarkSeenKey: true, ...content}),
  );
  return (uiStateFileProvider.overrideWithValue(UiStateFile(path)), path);
}

Map<String, Object?> readState(String path) =>
    jsonDecode(File(path).readAsStringSync()) as Map<String, Object?>;

/// Der Client einer laufenden Sitzung, der mitzählt, wie oft ein Terminal
/// die Sitzung öffnet.
class CountingClient extends SandboxTestClient {
  CountingClient() {
    sandbox = runningClient().sandbox;
  }

  /// Wie oft ein `Open` ankam.
  int opens = 0;

  @override
  Stream<TerminalFrame> terminal(Stream<TerminalCommand> input) =>
      super.terminal(
        input.map((TerminalCommand command) {
          if (command is TerminalOpen) {
            opens++;
          }
          return command;
        }),
      );
}

/// Die Rechtecke der Bereiche, die frei bleiben müssen und gerade etwas
/// zeigen.
List<Rect> decisionRects(WidgetTester tester) => <Rect>[
  tester.getRect(find.byType(QueuePane)),
  tester.getRect(find.byType(ActionBar)),
  for (final Element notices
      in find.byKey(const Key('notice-layer-notices')).evaluate())
    if (notices.renderObject case final RenderBox box
        when box.hasSize && !box.size.isEmpty)
      box.localToGlobal(Offset.zero) & box.size,
];

void main() {
  final TargetPlatformVariant linux = TargetPlatformVariant.only(
    TargetPlatform.linux,
  );

  testWidgets('the_chord_floats_the_terminal_over_a_section_and_back', (
    WidgetTester tester,
  ) async {
    await pumpApp(tester, client: runningClient(), size: window);
    await settle(tester);
    expect(floating, findsNothing);

    await pressFloat(tester);
    expect(floating, findsOneWidget);
    expect(headerTitle(tester), 'Intercept', reason: 'the section stays');
    expectOneView();
    expect(
      find.descendant(of: floating, matching: find.byType(TerminalView)),
      findsOneWidget,
      reason: 'the view is the floating one, the pane stays empty',
    );

    await pressFloat(tester);
    expect(floating, findsNothing);
    expect(placement(tester), TerminalPlacement.pane);
    expectOneView();
    expect(tester.takeException(), isNull);
  }, variant: linux);

  testWidgets('the_same_session_moves_without_a_second_attach', (
    WidgetTester tester,
  ) async {
    final CountingClient client = CountingClient();
    await pumpApp(tester, client: client, size: window);
    await pressCtrl(tester, LogicalKeyboardKey.digit4);
    await settle(tester);
    final Object before = tester
        .widget<TerminalView>(find.byType(TerminalView))
        .terminal;
    expect(client.opens, 1);

    // Auf der Sandbox-Seite bleibt es im Pane (HUM-315); es schwebt, sobald
    // ein anderer Abschnitt zu sehen ist.
    await tester.tap(find.byKey(const Key('sandbox-terminal-float')));
    await settle(tester);
    expect(floating, findsNothing);
    expect(placement(tester), TerminalPlacement.floating);
    await pressCtrl(tester, LogicalKeyboardKey.digit2);
    await settle(tester);
    expect(floating, findsOneWidget);
    final Object after = tester
        .widget<TerminalView>(
          find.descendant(of: floating, matching: find.byType(TerminalView)),
        )
        .terminal;
    expect(
      identical(before, after),
      isTrue,
      reason: 'the same emulator with its scrollback',
    );
    expect(client.opens, 1, reason: 'no second Open for the same session');

    // Zurück auf der Sandbox-Seite steht dieselbe Sitzung wieder im Pane.
    await pressCtrl(tester, LogicalKeyboardKey.digit4);
    await settle(tester);
    expect(floating, findsNothing);
    expect(find.byKey(const Key('sandbox-terminal')), findsOneWidget);
    expect(
      identical(
        tester.widget<TerminalView>(find.byType(TerminalView)).terminal,
        before,
      ),
      isTrue,
    );
    expect(client.opens, 1);
    expectOneView();
  }, variant: linux);

  testWidgets('without_an_agent_nothing_floats', (WidgetTester tester) async {
    await pumpApp(tester, client: SandboxTestClient(), size: window);
    await pressFloat(tester);
    expect(floating, findsNothing);
    expect(placement(tester), TerminalPlacement.pane);

    await pressCtrl(tester, LogicalKeyboardKey.digit4);
    await settle(tester);
    await tester.tap(
      find.byKey(const Key('sandbox-terminal-float')),
      warnIfMissed: false,
    );
    await settle(tester);
    expect(floating, findsNothing, reason: 'the button is disabled');
  }, variant: linux);

  testWidgets('the_window_never_covers_the_queue_the_decision_or_a_notice', (
    WidgetTester tester,
  ) async {
    final HeldAndRunningClient client = HeldAndRunningClient();
    await pumpApp(tester, client: client, size: window);
    await tester.pump(const Duration(milliseconds: 100));
    await settle(tester);
    await tester.tap(find.byType(QueuePane));
    await tester.pump(const Duration(milliseconds: 500));
    await tester.pump();

    await pressFloat(tester);
    expect(floating, findsOneWidget, reason: 'there is room beside them');
    final Rect pip = tester.getRect(floating);
    for (final Rect zone in decisionRects(tester)) {
      expect(pip.overlaps(zone), isFalse, reason: '$pip covers $zone');
    }
    // Und was dort steht, nimmt den Zeiger, nicht das Fenster.
    expect(
      find.byKey(const Key('intercept-allow')).hitTestable(),
      findsOneWidget,
    );
    expect(
      find.byKey(const Key('intercept-block')).hitTestable(),
      findsOneWidget,
    );
    expect(
      find.byType(QueueRow).hitTestable().evaluate().length,
      find.byType(QueueRow).evaluate().length,
    );
  }, variant: linux);

  // Die Breite des Laptops des Owners: Auch dort bleiben Freigeben und
  // Blockieren mit offenem Fenster treffbar.
  testWidgets('at_1280_the_decision_stays_reachable_with_the_window_open', (
    WidgetTester tester,
  ) async {
    final HeldAndRunningClient client = HeldAndRunningClient();
    await pumpApp(tester, client: client);
    await tester.pump(const Duration(milliseconds: 100));
    await settle(tester);
    await tester.tap(find.byType(QueuePane));
    await tester.pump(const Duration(milliseconds: 500));
    await tester.pump();

    await pressFloat(tester);
    expect(floating, findsOneWidget, reason: 'it still finds a corner');
    final Rect pip = tester.getRect(floating);
    for (final Rect zone in decisionRects(tester)) {
      expect(pip.overlaps(zone), isFalse, reason: '$pip covers $zone');
    }
    expect(
      find.byKey(const Key('intercept-allow')).hitTestable(),
      findsOneWidget,
    );
    expect(
      find.byKey(const Key('intercept-block')).hitTestable(),
      findsOneWidget,
    );
    expect(
      find.byType(QueueRow).hitTestable().evaluate().length,
      find.byType(QueueRow).evaluate().length,
    );
  }, variant: linux);

  // Ein schmales Kontext-Pane: Unten rechts reicht dann auch die kleinste
  // Größe bis über die Aktionsleiste, also weicht das Fenster nach oben aus.
  testWidgets('a_narrow_context_pane_sends_the_window_away_from_the_bar', (
    WidgetTester tester,
  ) async {
    final (Override state, String _) = stateFile(<String, Object?>{
      'paneSplit.intercept': <double>[0.3, 0.55, 0.15],
    });
    await pumpApp(tester, client: HeldAndRunningClient(), uiState: state);
    await tester.pump(const Duration(milliseconds: 100));
    await settle(tester);

    await pressFloat(tester);
    expect(floating, findsOneWidget);
    final Rect pip = tester.getRect(floating);
    final Rect bar = tester.getRect(find.byType(ActionBar));
    expect(pip.overlaps(bar), isFalse, reason: '$pip covers the bar $bar');
    expect(
      find.byKey(const Key('intercept-allow')).hitTestable(),
      findsOneWidget,
    );
    expect(
      find.byKey(const Key('intercept-block')).hitTestable(),
      findsOneWidget,
    );
  }, variant: linux);

  testWidgets('a_corner_over_the_queue_is_refused_for_the_next_free_one', (
    WidgetTester tester,
  ) async {
    final (Override state, String _) = stateFile(<String, Object?>{
      terminalFloatCornerKey: FloatCorner.bottomLeft.name,
    });
    await pumpApp(
      tester,
      client: HeldAndRunningClient(),
      size: window,
      uiState: state,
    );
    await tester.pump(const Duration(milliseconds: 100));
    await settle(tester);

    await pressFloat(tester);
    expect(floating, findsOneWidget);
    final Rect pip = tester.getRect(floating);
    final Rect queue = tester.getRect(find.byType(QueuePane));
    expect(pip.overlaps(queue), isFalse);
    expect(pip.left, greaterThanOrEqualTo(queue.right));
    for (final Rect zone in decisionRects(tester)) {
      expect(pip.overlaps(zone), isFalse, reason: '$pip covers $zone');
    }
  }, variant: linux);

  testWidgets('the_queue_keeps_the_keyboard_until_the_terminal_takes_it', (
    WidgetTester tester,
  ) async {
    final HeldAndRunningClient client = HeldAndRunningClient();
    await pumpApp(tester, client: client, size: window);
    await tester.pump(const Duration(milliseconds: 100));
    await settle(tester);
    await tester.tap(find.byType(QueuePane));
    await tester.pump(const Duration(milliseconds: 500));
    await tester.pump();

    // Aus der Warteschlange geöffnet, bleibt die Tastatur dort: `Enter`
    // gibt frei, der Agent hört nichts.
    await pressFloat(tester);
    expect(floating, findsOneWidget);
    expect(terminalHasKeyboard(), isFalse);
    client.typed.clear();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    await tester.pump();
    expect(client.decisions, hasLength(1), reason: 'Enter allows');
    expect(client.typed, isEmpty, reason: 'and the agent hears nothing');

    // Der Zeiger über dem Fenster gibt dem Terminal die Tastatur (HUM-303),
    // und dann geht `Enter` an den Agenten und entscheidet nichts.
    final TestGesture mouse = await tester.createGesture(
      kind: PointerDeviceKind.mouse,
    );
    await mouse.addPointer(location: const Offset(1, 1));
    addTearDown(mouse.removePointer);
    await mouse.moveTo(
      tester.getCenter(
        find.descendant(of: floating, matching: find.byType(TerminalView)),
      ),
    );
    await tester.pump();
    expect(terminalHasKeyboard(), isTrue);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(client.typed, contains(0x0d));
    expect(client.decisions, hasLength(1), reason: 'no second decision');
  }, variant: linux);

  testWidgets('shell_chords_work_from_the_focused_floating_terminal', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = runningClient();
    await pumpApp(tester, client: client, size: window);
    await pressCtrl(tester, LogicalKeyboardKey.digit2);
    await settle(tester);

    // `Ctrl+Backquote` ohne Terminal auf dem Schirm lässt es schweben und
    // gibt ihm die Tastatur (HUM-303 hatte das offen gelassen).
    await pressFocusTerminal(tester);
    await settle(tester);
    expect(floating, findsOneWidget);
    expect(terminalHasKeyboard(), isTrue);
    expect(headerTitle(tester), 'History');

    client.typed.clear();
    await pressCtrl(tester, LogicalKeyboardKey.digit3);
    await settle(tester);
    expect(headerTitle(tester), 'Rules', reason: 'Ctrl+3 reached the shell');
    expect(floating, findsOneWidget, reason: 'and the window stays');

    await pressFloat(tester);
    expect(floating, findsNothing);
    expect(client.typed, isEmpty, reason: 'no chord reached the agent');
    expect(
      FocusManager.instance.primaryFocus,
      isNot(isA<FocusScopeNode>()),
      reason: 'somebody who hears the shell keys holds the keyboard',
    );
    await pressCtrl(tester, LogicalKeyboardKey.digit2);
    await settle(tester);
    expect(headerTitle(tester), 'History');
  }, variant: linux);

  testWidgets('back_to_the_sandbox_and_close', (WidgetTester tester) async {
    await pumpApp(tester, client: runningClient(), size: window);
    await pressCtrl(tester, LogicalKeyboardKey.digit2);
    await pressFloat(tester);
    expect(floating, findsOneWidget);

    await tester.tap(find.byKey(const Key('shell-terminal-floating-close')));
    await settle(tester);
    expect(floating, findsNothing);
    expect(headerTitle(tester), 'History', reason: 'close stays where it is');

    await pressFloat(tester);
    await tester.tap(find.byKey(const Key('shell-terminal-floating-back')));
    await settle(tester);
    expect(floating, findsNothing);
    expect(headerTitle(tester), 'Sandbox');
    expect(find.byKey(const Key('sandbox-terminal')), findsOneWidget);
    expectOneView();
  }, variant: linux);

  testWidgets('a_drag_snaps_to_a_corner_and_the_corner_is_remembered', (
    WidgetTester tester,
  ) async {
    final (Override state, String path) = stateFile();
    await pumpApp(
      tester,
      client: runningClient(),
      size: window,
      uiState: state,
    );
    await pressCtrl(tester, LogicalKeyboardKey.digit2);
    await pressFloat(tester);
    final Rect start = tester.getRect(floating);
    expect(start.center.dx, greaterThan(window.width / 2));
    expect(start.center.dy, greaterThan(window.height / 2));

    await tester.drag(
      find.byKey(const Key('shell-terminal-floating-title')),
      Offset(-start.left + 100, -start.top + 150),
      kind: PointerDeviceKind.mouse,
    );
    await settle(tester);
    final Rect snapped = tester.getRect(floating);
    expect(snapped.center.dx, lessThan(window.width / 2));
    expect(snapped.center.dy, lessThan(window.height / 2));
    expect(readState(path)[terminalFloatCornerKey], 'topLeft');

    // Ein neuer Start liest die Ecke wieder.
    await tester.pumpWidget(const SizedBox.shrink());
    await pumpApp(
      tester,
      client: runningClient(),
      size: window,
      uiState: uiStateFileProvider.overrideWithValue(UiStateFile(path)),
    );
    await pressCtrl(tester, LogicalKeyboardKey.digit2);
    await pressFloat(tester);
    expect(tester.getRect(floating), snapped);
  }, variant: linux);

  testWidgets('the_grip_resizes_within_the_limits_and_is_remembered', (
    WidgetTester tester,
  ) async {
    final (Override state, String path) = stateFile();
    await pumpApp(
      tester,
      client: runningClient(),
      size: window,
      uiState: state,
    );
    await pressCtrl(tester, LogicalKeyboardKey.digit2);
    await pressFloat(tester);
    expect(tester.getSize(floating), terminalFloatDefaultSize);

    // Unten rechts verankert: Der Griff sitzt oben links, nach links oben
    // gezogen wird das Fenster größer.
    await tester.drag(
      find.byKey(const Key('shell-terminal-floating-grip')),
      const Offset(-100, -50),
      kind: PointerDeviceKind.mouse,
    );
    await settle(tester);
    expect(tester.getSize(floating), const Size(580, 350));
    expect(readState(path)[terminalFloatSizeKey], <double>[580, 350]);

    // Nie kleiner als das Minimum.
    await tester.drag(
      find.byKey(const Key('shell-terminal-floating-grip')),
      const Offset(1000, 1000),
      kind: PointerDeviceKind.mouse,
    );
    await settle(tester);
    expect(tester.getSize(floating), terminalFloatMinSize);
  }, variant: linux);

  testWidgets('a_sheet_pushes_the_window_aside', (WidgetTester tester) async {
    await pumpApp(tester, client: runningClient(), size: window);
    await pressCtrl(tester, LogicalKeyboardKey.digit2);
    await pressFloat(tester);
    expect(floating, findsOneWidget);

    await pressCtrl(tester, LogicalKeyboardKey.comma);
    await settle(tester);
    expect(find.byType(SettingsSheet), findsOneWidget);
    await settle(tester);
    expect(floating, findsOneWidget, reason: 'there is room on the left');
    expect(
      tester
          .getRect(floating)
          .overlaps(tester.getRect(find.byType(SettingsSheet))),
      isFalse,
    );
  }, variant: linux);

  testWidgets('when_the_agent_ends_the_window_shows_it_then_closes', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = runningClient();
    await pumpApp(tester, client: client, size: window);
    await pressCtrl(tester, LogicalKeyboardKey.digit2);
    await pressFloat(tester);
    expect(floating, findsOneWidget);

    client.sandbox = client.sandbox.copyWith(agentRunning: false);
    await ProviderScope.containerOf(tester.element(find.byType(ShellScreen)))
        .read(sandboxStatusProvider.notifier)
        .refresh();
    await settle(tester);
    expect(floating, findsOneWidget, reason: 'the end stays visible a moment');

    await tester.pump(terminalFloatLinger - const Duration(milliseconds: 100));
    expect(floating, findsOneWidget);
    await tester.pump(const Duration(milliseconds: 200));
    await settle(tester);
    expect(floating, findsNothing);
    expect(placement(tester), TerminalPlacement.pane);
    expect(headerTitle(tester), 'History');
  }, variant: linux);

  testWidgets('the_palette_floats_the_terminal', (WidgetTester tester) async {
    await pumpApp(tester, client: runningClient(), size: window);
    await settle(tester);
    await pressCtrl(tester, LogicalKeyboardKey.keyK);
    await tester.enterText(find.byKey(const Key('palette-input')), 'float');
    await tester.pump();
    await tester.tap(find.byKey(const Key('palette-toggle-terminal-float')));
    await settle(tester);
    expect(floating, findsOneWidget);
    expect(headerTitle(tester), 'Intercept');
  }, variant: linux);

  // --- Review-Runde 1 ---------------------------------------------------------

  // Codex: Ein Kürzel der Shell nahm das Drücken, Wiederholungen und Loslassen
  // fielen zum Emulator durch. Mit dem Tastaturprotokoll von kitty (Ereignisse
  // und alle Tasten als Folge) hätte der Agent das `P` gesehen, und das
  // Loslassen kommt im anderen Wirt an.
  testWidgets('a_held_float_chord_never_reaches_the_agent', (
    WidgetTester tester,
  ) async {
    // Seit HUM-315 bleibt das Terminal auf der Sandbox-Seite im Pane; der
    // Wechsel des Wirts kommt deshalb vom schwebenden Fenster über History,
    // das das Kürzel zurücklegt. Das Loslassen kommt dann woanders an.
    final SandboxTestClient client = runningClient();
    await pumpApp(tester, client: client, size: window);
    await pressCtrl(tester, LogicalKeyboardKey.digit2);
    await settle(tester);
    await pressFocusTerminal(tester);
    await settle(tester);
    expect(floating, findsOneWidget);
    expect(terminalHasKeyboard(), isTrue);
    tester
        .widget<TerminalView>(
          find.descendant(of: floating, matching: find.byType(TerminalView)),
        )
        .terminal
        .write('\x1b[>11u');
    await tester.pump();
    client.typed.clear();

    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.keyP);
    await settle(tester);
    expect(floating, findsNothing, reason: 'the chord put it back');
    for (int i = 0; i < 3; i++) {
      await tester.sendKeyRepeatEvent(LogicalKeyboardKey.keyP);
    }
    await tester.sendKeyUpEvent(LogicalKeyboardKey.keyP);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    await settle(tester);

    final String sent = utf8.decode(client.typed, allowMalformed: true);
    expect(sent, isNot(contains('[112;')), reason: 'no report of the P key');
    expect(client.typed, isNot(contains(0x10)));
    expect(floating, findsNothing, reason: 'the chord toggled once');
    expect(placement(tester), TerminalPlacement.pane);
  }, variant: linux);

  // Codex: Nach dem Ende zeigte das Fenster vier Sekunden den Leerzustand
  // statt des Endes.
  testWidgets('while_it_lingers_the_window_shows_the_ended_session', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = runningClient();
    await pumpApp(tester, client: client, size: window);
    await pressCtrl(tester, LogicalKeyboardKey.digit2);
    await pressFloat(tester);

    client.sandbox = client.sandbox.copyWith(agentRunning: false);
    await ProviderScope.containerOf(tester.element(find.byType(ShellScreen)))
        .read(sandboxStatusProvider.notifier)
        .refresh();
    await settle(tester);
    expect(floating, findsOneWidget);
    expect(
      find.descendant(of: floating, matching: find.byType(TerminalView)),
      findsOneWidget,
      reason: 'the session that ended, not the empty state',
    );
    expect(find.textContaining('Nothing runs'), findsNothing);
    await tester.pump(terminalFloatLinger + const Duration(milliseconds: 100));
    await settle(tester);
  }, variant: linux);

  // Codex: Schwebend, dann das ganze Fenster, dann endet der Agent, dann
  // zurück: Das Fenster blieb für immer stehen.
  testWidgets('an_end_seen_over_the_whole_window_closes_the_float_on_return', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = runningClient();
    await pumpApp(tester, client: client, size: window);
    await pressCtrl(tester, LogicalKeyboardKey.digit2);
    await pressFloat(tester);
    await pressFullWindow(tester);
    await settle(tester);
    expect(placement(tester), TerminalPlacement.fullWindow);

    client.sandbox = client.sandbox.copyWith(agentRunning: false);
    await ProviderScope.containerOf(tester.element(find.byType(ShellScreen)))
        .read(sandboxStatusProvider.notifier)
        .refresh();
    await settle(tester);
    await tester.pump(terminalFloatLinger + const Duration(milliseconds: 100));
    await settle(tester);
    expect(placement(tester), TerminalPlacement.fullWindow);

    await pressFullWindow(tester);
    await settle(tester);
    expect(floating, findsNothing);
    expect(placement(tester), TerminalPlacement.pane);
  }, variant: linux);

  // Codex: Ein Zug am Griff zog das Fenster über die Aktionsleiste.
  testWidgets('the_grip_never_pulls_the_window_over_the_bar', (
    WidgetTester tester,
  ) async {
    await pumpApp(tester, client: HeldAndRunningClient(), size: window);
    await tester.pump(const Duration(milliseconds: 100));
    await settle(tester);
    await pressFloat(tester);
    final Rect before = tester.getRect(floating);

    final TestGesture hand = await tester.startGesture(
      tester.getCenter(find.byKey(const Key('shell-terminal-floating-grip'))),
      kind: PointerDeviceKind.mouse,
    );
    for (int i = 0; i < 8; i++) {
      await hand.moveBy(const Offset(-60, -20));
      await tester.pump();
      for (final Rect zone in decisionRects(tester)) {
        final Rect pip = tester.getRect(floating);
        expect(pip.overlaps(zone), isFalse, reason: 'mid-drag $pip on $zone');
      }
    }
    await hand.up();
    await settle(tester);
    final Rect after = tester.getRect(floating);
    expect(after.height, greaterThan(before.height), reason: 'it still grew');
    for (final Rect zone in decisionRects(tester)) {
      expect(after.overlaps(zone), isFalse, reason: '$after on $zone');
    }
  }, variant: linux);

  // Codex: Beim Ziehen darf es kurz über der Warteschlange stehen; losgelassen
  // weicht es aus, und die gewählte Ecke bleibt die gewählte.
  testWidgets('a_window_let_go_over_the_queue_gives_way', (
    WidgetTester tester,
  ) async {
    final (Override state, String path) = stateFile();
    await pumpApp(
      tester,
      client: HeldAndRunningClient(),
      size: window,
      uiState: state,
    );
    await tester.pump(const Duration(milliseconds: 100));
    await settle(tester);
    await pressFloat(tester);
    final Rect queue = tester.getRect(find.byType(QueuePane));
    final Rect start = tester.getRect(floating);

    await tester.drag(
      find.byKey(const Key('shell-terminal-floating-title')),
      Offset(queue.left + 20 - start.left, 0),
      kind: PointerDeviceKind.mouse,
    );
    await settle(tester);
    expect(readState(path)[terminalFloatCornerKey], 'bottomLeft');
    final Rect after = tester.getRect(floating);
    for (final Rect zone in decisionRects(tester)) {
      expect(after.overlaps(zone), isFalse, reason: '$after on $zone');
    }
  }, variant: linux);

  // Sonnet: Ein Klick auf die Titelleiste endet als abgebrochener Zug und
  // speicherte die Ausweichecke als gewählte.
  testWidgets('a_click_on_the_title_keeps_the_chosen_corner', (
    WidgetTester tester,
  ) async {
    final (Override state, String path) = stateFile(<String, Object?>{
      terminalFloatCornerKey: FloatCorner.bottomLeft.name,
    });
    await pumpApp(
      tester,
      client: HeldAndRunningClient(),
      size: window,
      uiState: state,
    );
    await tester.pump(const Duration(milliseconds: 100));
    await settle(tester);
    await pressFloat(tester);
    expect(
      tester.getRect(floating).left,
      greaterThan(tester.getRect(find.byType(QueuePane)).right),
      reason: 'it gives way to the queue',
    );

    final Rect title = tester.getRect(
      find.byKey(const Key('shell-terminal-floating-title')),
    );
    await tester.tapAt(title.centerLeft + const Offset(40, 0));
    await settle(tester);
    expect(readState(path)[terminalFloatCornerKey], 'bottomLeft');
  }, variant: linux);

  // Sonnet: Unten verankert lag der Griff auf dem Schließen-Knopf.
  testWidgets('the_grip_keeps_off_the_title_buttons', (
    WidgetTester tester,
  ) async {
    for (final FloatCorner corner in FloatCorner.values) {
      final (Override state, String _) = stateFile(<String, Object?>{
        terminalFloatCornerKey: corner.name,
      });
      await pumpApp(
        tester,
        client: runningClient(),
        size: window,
        uiState: state,
      );
      await pressCtrl(tester, LogicalKeyboardKey.digit2);
      await pressFloat(tester);
      final Rect grip = tester.getRect(
        find.byKey(const Key('shell-terminal-floating-grip')),
      );
      final Rect title = tester.getRect(
        find.byKey(const Key('shell-terminal-floating-title')),
      );
      expect(grip.overlaps(title), isFalse, reason: '$corner');
      await tester.pumpWidget(const SizedBox.shrink());
    }
  }, variant: linux);

  // --- Review-Runde 2 ---------------------------------------------------------

  // Codex: Schwebend, ganzes Fenster, Ende, zurück innerhalb der vier
  // Sekunden: Das neu gebaute Fenster kannte die Sitzung nicht mehr.
  testWidgets('back_from_the_whole_window_in_time_the_float_shows_the_end', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = runningClient();
    await pumpApp(tester, client: client, size: window);
    await pressCtrl(tester, LogicalKeyboardKey.digit2);
    await pressFloat(tester);
    await pressFullWindow(tester);
    await settle(tester);

    client.sandbox = client.sandbox.copyWith(agentRunning: false);
    await ProviderScope.containerOf(tester.element(find.byType(ShellScreen)))
        .read(sandboxStatusProvider.notifier)
        .refresh();
    await settle(tester);
    await pressFullWindow(tester);
    await settle(tester);
    expect(floating, findsOneWidget, reason: 'the time is not over yet');
    expect(
      find.descendant(of: floating, matching: find.byType(TerminalView)),
      findsOneWidget,
      reason: 'the session that ended, not the empty state',
    );
    expect(find.textContaining('Nothing runs'), findsNothing);

    await tester.pump(terminalFloatLinger + const Duration(milliseconds: 100));
    await settle(tester);
    expect(floating, findsNothing);
  }, variant: linux);

  // --- Review-Runde 3 ---------------------------------------------------------

  // Codex: Endet A und startet im Nachlauf eine Sandbox B, deren Agent noch
  // nicht läuft, baute das Fenster weiter A.
  testWidgets('a_new_sandbox_during_the_linger_drops_the_ended_session', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = runningClient();
    await pumpApp(tester, client: client, size: window);
    await pressCtrl(tester, LogicalKeyboardKey.digit2);
    await pressFloat(tester);
    final ProviderContainer container = ProviderScope.containerOf(
      tester.element(find.byType(ShellScreen)),
    );

    client.sandbox = client.sandbox.copyWith(agentRunning: false);
    await container.read(sandboxStatusProvider.notifier).refresh();
    await settle(tester);
    expect(
      find.descendant(of: floating, matching: find.byType(TerminalView)),
      findsOneWidget,
      reason: 'A ended, its end is shown',
    );

    client.sandbox = client.sandbox.copyWith(
      sandboxId: const SandboxId('sandbox-b'),
    );
    await container.read(sandboxStatusProvider.notifier).refresh();
    await settle(tester);
    expect(floating, findsOneWidget, reason: 'still within the linger');
    expect(
      find.descendant(of: floating, matching: find.byType(TerminalView)),
      findsNothing,
      reason: 'B replaced A: no emulator of A any more',
    );
    expect(container.read(lastFloatingSessionProvider), isEmpty);
    await tester.pump(terminalFloatLinger + const Duration(milliseconds: 100));
    await settle(tester);
  }, variant: linux);

  // Codex: Das Loslassen einer Kürzeltaste außerhalb jedes Terminals ließ sie
  // in der Menge; die Wiederholungen eines späteren Drückens, das anderswo
  // begann, schluckte das Terminal.
  testWidgets('a_chord_key_released_elsewhere_is_no_longer_held', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = runningClient();
    await pumpApp(tester, client: client, size: window);
    await pressCtrl(tester, LogicalKeyboardKey.digit4);
    await settle(tester);
    await pressFocusTerminal(tester);
    expect(terminalHasKeyboard(), isTrue);
    tester
        .widget<TerminalView>(find.byType(TerminalView))
        .terminal
        .write('\x1b[>11u');
    await tester.pump();

    // Gegenprobe: Ein gewöhnliches `=` meldet das Protokoll mit Drücken,
    // Wiederholung und Loslassen.
    client.typed.clear();
    await tester.sendKeyDownEvent(LogicalKeyboardKey.equal);
    await tester.sendKeyRepeatEvent(LogicalKeyboardKey.equal);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.equal);
    await settle(tester);
    expect(
      utf8.decode(client.typed, allowMalformed: true),
      contains('[61;1:2u'),
    );
    // `Ctrl+=` im Terminal: Die Anwendung nimmt das `=`.
    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.equal);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    await tester.pump();
    // Die Tastatur geht weg, das `=` kommt anderswo hoch und wird anderswo
    // wieder gedrückt.
    FocusManager.instance.primaryFocus?.unfocus();
    await tester.pump();
    expect(terminalHasKeyboard(), isFalse);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.equal);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.equal);
    await tester.pump();
    // Das Terminal holt sich die Tastatur, während das `=` gehalten ist.
    await pressFocusTerminal(tester);
    expect(terminalHasKeyboard(), isTrue);
    client.typed.clear();

    await tester.sendKeyRepeatEvent(LogicalKeyboardKey.equal);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.equal);
    await settle(tester);
    final String sent = utf8.decode(client.typed, allowMalformed: true);
    expect(
      sent,
      contains('[61;'),
      reason: 'the repeat of a press that began elsewhere is the agent\'s',
    );
  }, variant: linux);
}
