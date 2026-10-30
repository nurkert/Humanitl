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
    show FloatingTerminalLayer, terminalFloatLinger;
import 'package:humanitl_ui/humanitl_ui.dart' show HMotion, HSpace;
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

/// Die Fläche, über der das Fenster schwebt.
Rect layerRect(WidgetTester tester) =>
    tester.getRect(find.byType(FloatingTerminalLayer));

/// Lässt das Fenster nach dem Loslassen an seinen Anker gleiten (HUM-352).
Future<void> glideHome(WidgetTester tester) async {
  await settle(tester);
  await tester.pump(HMotion.arrive);
  await settle(tester);
}

/// [actual] ist [expected], bis auf Rundung im Layout.
void expectSize(Size actual, Size expected, String reason) {
  expect(actual.width, closeTo(expected.width, 0.01), reason: reason);
  expect(actual.height, closeTo(expected.height, 0.01), reason: reason);
}

/// Zieht den Griff an [anchor] weit nach außen, in kleinen Schritten.
Future<void> pullGripFar(WidgetTester tester, FloatAnchor anchor) async {
  final TestGesture hand = await tester.startGesture(
    tester.getCenter(find.byKey(const Key('shell-terminal-floating-grip'))),
    kind: PointerDeviceKind.mouse,
  );
  for (int i = 0; i < 40; i++) {
    await hand.moveBy(Offset(anchor.gripX * 40.0, anchor.gripY * 40.0));
    await tester.pump();
  }
  await hand.up();
  await settle(tester);
}

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
      terminalFloatCornerKey: FloatAnchor.bottomLeft.name,
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
    await glideHome(tester);
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
    await glideHome(tester);
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
      terminalFloatCornerKey: FloatAnchor.bottomLeft.name,
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

  // Sonnet (HUM-300): Unten verankert lag der Griff auf dem Schließen-Knopf.
  // Owner (HUM-352): Unten rechts lag er auf Titel und Hinweis. An jedem
  // Anker sitzt er jetzt in der Ecke, und der Kopf rückt auf seiner Seite ein.
  testWidgets('the_grip_keeps_off_title_hint_and_buttons_at_every_anchor', (
    WidgetTester tester,
  ) async {
    for (final FloatAnchor anchor in FloatAnchor.values) {
      final (Override state, String _) = stateFile(<String, Object?>{
        terminalFloatCornerKey: anchor.name,
      });
      await pumpApp(
        tester,
        client: runningClient(),
        size: window,
        uiState: state,
      );
      await pressCtrl(tester, LogicalKeyboardKey.digit2);
      await pressFloat(tester);
      final Rect pip = tester.getRect(floating);
      final Rect grip = tester.getRect(
        find.byKey(const Key('shell-terminal-floating-grip')),
      );
      // In der Ecke gegenüber dem Anker, bündig mit dem Fenster.
      expect(
        anchor.gripX == -1 ? grip.left : grip.right,
        anchor.gripX == -1 ? pip.left : pip.right,
        reason: '$anchor',
      );
      expect(
        anchor.gripY == -1 ? grip.top : grip.bottom,
        anchor.gripY == -1 ? pip.top : pip.bottom,
        reason: '$anchor',
      );
      Rect within(Finder finder) =>
          tester.getRect(find.descendant(of: floating, matching: finder));
      final Map<String, Rect> header = <String, Rect>{
        'title': within(find.text('Agent terminal')),
        'hint': within(
          find.text(
            'Agent output is untrusted. Clipboard and link sequences are '
            'filtered.',
          ),
        ),
        'back': within(find.byKey(const Key('shell-terminal-floating-back'))),
        'close': within(find.byKey(const Key('shell-terminal-floating-close'))),
        'full window': within(
          find.byKey(const Key('sandbox-terminal-full-window')),
        ),
      };
      for (final MapEntry<String, Rect> part in header.entries) {
        expect(
          grip.overlaps(part.value),
          isFalse,
          reason: '$anchor: the grip $grip on the ${part.key} ${part.value}',
        );
      }
      await tester.pumpWidget(const SizedBox.shrink());
    }
  }, variant: linux);

  // HUM-352: Jeder der neun Anker ist mit der Hand erreichbar, und ein
  // ruhiges Loslassen rastet am nächsten ein.
  testWidgets('every_anchor_is_reachable_by_a_drag', (
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
    final Rect region = layerRect(tester);
    for (final FloatAnchor anchor in <FloatAnchor>[
      // Vom Start unten rechts aus alle anderen, dann zurück.
      ...FloatAnchor.values.reversed.skip(1),
      FloatAnchor.bottomRight,
    ]) {
      final Rect from = tester.getRect(floating);
      final Rect target = terminalFloatRectAt(
        region,
        anchor,
        from.size,
        HSpace.x3,
      );
      // Ruhig gezogen, ein Stück neben das Ziel: Es rastet trotzdem dort ein.
      await tester.timedDrag(
        find.byKey(const Key('shell-terminal-floating-title')),
        target.center - from.center + const Offset(25, -15),
        const Duration(seconds: 2),
      );
      await glideHome(tester);
      expect(tester.getRect(floating), target, reason: '$anchor');
      expect(readState(path)[terminalFloatCornerKey], anchor.name);
    }
  }, variant: linux);

  // HUM-352: Ein kurzer Wurf zählt mehr als der Abstand. Aus jeder Ecke
  // landet er beim Nachbarn in seiner Richtung, nicht zurück am Start; ein
  // langsames Loslassen nach demselben Weg bleibt am Start.
  testWidgets('a_throw_from_each_corner_lands_on_its_neighbour', (
    WidgetTester tester,
  ) async {
    for (final FloatAnchor corner in <FloatAnchor>[
      FloatAnchor.topLeft,
      FloatAnchor.topRight,
      FloatAnchor.bottomLeft,
      FloatAnchor.bottomRight,
    ]) {
      for (final (Offset way, FloatAnchor neighbour) in <(Offset, FloatAnchor)>[
        (Offset(-corner.x * 60.0, 0), FloatAnchor.at(0, corner.y)),
        (Offset(0, -corner.y * 60.0), FloatAnchor.at(corner.x, 0)),
      ]) {
        for (final bool fast in <bool>[true, false]) {
          final (Override state, String path) = stateFile(<String, Object?>{
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
          final Finder title = find.byKey(
            const Key('shell-terminal-floating-title'),
          );
          if (fast) {
            await tester.fling(
              title,
              way,
              1500,
              deviceKind: PointerDeviceKind.mouse,
            );
          } else {
            await tester.timedDrag(title, way, const Duration(seconds: 2));
          }
          await glideHome(tester);
          final FloatAnchor landed = fast ? neighbour : corner;
          expect(
            readState(path)[terminalFloatCornerKey],
            landed.name,
            reason: '$corner by $way, fast: $fast',
          );
          final Rect pip = tester.getRect(floating);
          expect(
            pip,
            terminalFloatRectAt(layerRect(tester), landed, pip.size, HSpace.x3),
          );
          await tester.pumpWidget(const SizedBox.shrink());
        }
      }
    }
  }, variant: linux);

  // HUM-352: Dieselbe Hand bekommt an jedem Anker dasselbe größte Fenster.
  testWidgets('the_largest_window_is_the_same_at_every_anchor', (
    WidgetTester tester,
  ) async {
    for (final FloatAnchor anchor in FloatAnchor.values) {
      final (Override state, String _) = stateFile(<String, Object?>{
        terminalFloatCornerKey: anchor.name,
      });
      await pumpApp(
        tester,
        client: runningClient(),
        size: window,
        uiState: state,
      );
      await pressCtrl(tester, LogicalKeyboardKey.digit2);
      await pressFloat(tester);
      await pullGripFar(tester, anchor);
      expectSize(
        tester.getSize(floating),
        terminalFloatMaxIn(layerRect(tester).size),
        '$anchor',
      );
      await tester.pumpWidget(const SizedBox.shrink());
    }
  }, variant: linux);

  // Owner (HUM-352): Auf Intercept wuchs das Fenster oben rechts bis zur
  // Grenze, unten rechts nur über das Kontext-Pane: Die leere Leiste mit
  // Freigeben und Blockieren hielt frei, obwohl nichts zu entscheiden war.
  testWidgets('on_an_empty_queue_bottom_right_grows_as_far_as_top_right', (
    WidgetTester tester,
  ) async {
    final Map<FloatAnchor, Size> grown = <FloatAnchor, Size>{};
    for (final FloatAnchor anchor in <FloatAnchor>[
      FloatAnchor.topRight,
      FloatAnchor.bottomRight,
    ]) {
      final (Override state, String _) = stateFile(<String, Object?>{
        terminalFloatCornerKey: anchor.name,
      });
      await pumpApp(
        tester,
        client: runningClient(),
        size: window,
        uiState: state,
      );
      await pressFloat(tester);
      expect(headerTitle(tester), 'Intercept');
      await pullGripFar(tester, anchor);
      grown[anchor] = tester.getSize(floating);
      await tester.pumpWidget(const SizedBox.shrink());
    }
    expectSize(
      grown[FloatAnchor.bottomRight]!,
      grown[FloatAnchor.topRight]!,
      'bottom right against top right',
    );
    expect(
      grown[FloatAnchor.bottomRight]!.width,
      greaterThan(terminalFloatDefaultSize.width),
    );
  }, variant: linux);

  // --- HUM-352, Review-Runde 1 -----------------------------------------------

  // Codex: Der Editor einer gehaltenen Anfrage ersetzt die Aktionsleiste und
  // hielt nichts frei; unten in der Mitte lag das Fenster auf Senden und
  // Verwerfen.
  testWidgets('the_window_keeps_off_the_controls_of_the_editor', (
    WidgetTester tester,
  ) async {
    for (final FloatAnchor anchor in <FloatAnchor>[
      FloatAnchor.bottomCenter,
      FloatAnchor.bottomRight,
    ]) {
      final (Override state, String _) = stateFile(<String, Object?>{
        terminalFloatCornerKey: anchor.name,
      });
      await pumpApp(
        tester,
        client: HeldAndRunningClient(),
        size: window,
        uiState: state,
      );
      await tester.pump(const Duration(milliseconds: 100));
      await settle(tester);
      await tester.tap(find.byType(QueueRow).first);
      await settle(tester);
      await pressFloat(tester);
      // Der Editor der gehaltenen Anfrage.
      await tester.sendKeyEvent(LogicalKeyboardKey.keyE);
      for (int i = 0; i < 6; i++) {
        await tester.pump(const Duration(milliseconds: 16));
      }
      expect(find.byKey(const Key('editor-send')), findsOneWidget);
      final Rect pip = tester.getRect(floating);
      for (final Key key in const <Key>[
        Key('editor-send'),
        Key('editor-discard'),
      ]) {
        final Rect control = tester.getRect(find.byKey(key));
        expect(
          pip.overlaps(control),
          isFalse,
          reason: '$anchor: $pip covers $key at $control',
        );
        expect(find.byKey(key).hitTestable(), findsOneWidget, reason: '$key');
      }
      await tester.pumpWidget(const SizedBox.shrink());
    }
  }, variant: linux);

  // Codex: Das Ziel des Einrastens war frei, der Weg dorthin nicht. Über
  // Freigeben und Blockieren losgelassen, glitt das Fenster 180 ms über sie.
  testWidgets('a_window_let_go_over_the_bar_never_glides_across_it', (
    WidgetTester tester,
  ) async {
    await pumpApp(tester, client: HeldAndRunningClient(), size: window);
    await tester.pump(const Duration(milliseconds: 100));
    await settle(tester);
    await tester.tap(find.byType(QueueRow).first);
    await settle(tester);
    await pressFloat(tester);
    final Rect bar = tester.getRect(find.byType(ActionBar));
    final Rect start = tester.getRect(floating);

    await tester.drag(
      find.byKey(const Key('shell-terminal-floating-title')),
      bar.center - start.center,
      kind: PointerDeviceKind.mouse,
    );
    for (int frame = 0; frame < 20; frame++) {
      await tester.pump(const Duration(milliseconds: 16));
      final Rect pip = tester.getRect(floating);
      expect(pip.overlaps(bar), isFalse, reason: 'frame $frame: $pip on $bar');
    }
  }, variant: linux);

  // Codex und Sonnet: Ein Zug, der mitten im Gleiten beginnt, begann am Ziel,
  // und das Fenster sprang dorthin.
  testWidgets('a_drag_during_the_glide_starts_where_the_window_is', (
    WidgetTester tester,
  ) async {
    await pumpApp(tester, client: runningClient(), size: window);
    await pressCtrl(tester, LogicalKeyboardKey.digit2);
    await pressFloat(tester);
    final Rect start = tester.getRect(floating);
    final Rect region = layerRect(tester);
    final Rect goal = terminalFloatRectAt(
      region,
      FloatAnchor.topLeft,
      start.size,
      HSpace.x3,
    );
    await tester.drag(
      find.byKey(const Key('shell-terminal-floating-title')),
      goal.center - start.center + const Offset(150, 100),
      kind: PointerDeviceKind.mouse,
    );
    await tester.pump();
    await tester.pump(HMotion.arrive ~/ 3);
    final Rect midway = tester.getRect(floating);
    expect(midway, isNot(goal), reason: 'still on the way');

    final TestGesture hand = await tester.startGesture(
      tester
              .getRect(find.byKey(const Key('shell-terminal-floating-title')))
              .centerLeft +
          const Offset(60, 0),
      kind: PointerDeviceKind.mouse,
    );
    await hand.moveBy(const Offset(30, 0));
    await tester.pump();
    final Rect held = tester.getRect(floating);
    expect(held.top, closeTo(midway.top, 1), reason: '$held from $midway');
    expect(held.left, closeTo(midway.left + 30, 1), reason: '$held');
    await hand.up();
    await glideHome(tester);
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
