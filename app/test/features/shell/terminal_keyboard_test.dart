// Das Terminal nimmt die Tastatur, sobald der Zeiger darüber steht, und mit
// `Ctrl+Backquote` (HUM-303).
//
// Zwei Regeln halten den Fokus, wo er ist: Eine Ebene über der Arbeit
// (Dialog, Blatt, Menü, Palette) gibt ihn nie an den Zeiger ab, und ein
// Textfeld nicht an einen Zeiger, der nur über das Terminal fährt. Und die
// Entscheidungstasten der Warteschlange feuern nie, weil der Fokus durch
// Darüberfahren wanderte.
//
// Linux als Plattform: Ohne Variante spielt `flutter test` Android, und die
// Tasten dieses Produkts sind die eines Linux-Desktops.

import 'dart:async';

import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/shortcuts/intents.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/intercept_screen.dart';
import 'package:humanitl/features/intercept/widgets/queue_pane.dart';
import 'package:humanitl/features/sandbox/widgets/sandbox_terminal.dart';
import 'package:humanitl/features/settings/settings_sheet.dart';
import 'package:humanitl/features/shell/widgets/command_palette.dart';
import 'package:humanitl/l10n/l10n.dart';
import 'package:xterm2/ui.dart';

import '../../harness/app_harness.dart';
import '../../harness/ui_state.dart';
import '../sandbox/harness.dart';
import 'terminal_full_window_test.dart'
    show expectFullWindow, expectLayout, headerTitle, pressFullWindow, settle;

/// Ob der Knoten des Emulators die Tastatur hat.
bool terminalHasKeyboard() =>
    FocusManager.instance.primaryFocus?.debugLabel == 'terminal';

/// Die Mitte des Emulators.
Offset terminalCenter(WidgetTester tester) =>
    tester.getCenter(find.byType(TerminalView));

/// Eine Maus, die noch nirgends steht: links oben in der Ecke des Fensters,
/// über der Kopfleiste und nicht über dem Terminal.
Future<TestGesture> mouse(WidgetTester tester) async {
  final TestGesture gesture = await tester.createGesture(
    kind: PointerDeviceKind.mouse,
  );
  await gesture.addPointer(location: const Offset(1, 1));
  addTearDown(gesture.removePointer);
  await tester.pump();
  return gesture;
}

/// Nimmt dem Emulator die Tastatur und gibt sie dem Bereich um ihn.
///
/// Das Terminal holt sie sich beim Erscheinen selbst (`autofocus`); wer
/// prüfen will, dass Zeiger oder Kürzel sie bringen, nimmt sie ihm vorher.
Future<void> releaseKeyboard(WidgetTester tester) async {
  FocusManager.instance.primaryFocus?.unfocus();
  await tester.pump();
  expect(terminalHasKeyboard(), isFalse);
}

/// `Ctrl+Backquote`.
Future<void> pressFocusTerminal(WidgetTester tester) async {
  await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
  await tester.sendKeyEvent(LogicalKeyboardKey.backquote);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
  await tester.pump();
  await tester.pump();
}

/// Ein Fake mit gehaltenen Anfragen **und** laufender Sitzung, der jedes
/// Byte mitschreibt, das ein Terminal schickt.
class HeldAndRunningClient extends FakeDaemonClient {
  /// [count] gehaltene Anfragen, eine alle 20 ms.
  HeldAndRunningClient({int count = 2})
    : super(
        script: FakeDaemonClient.burstScript(
          count: count,
          spacing: const Duration(milliseconds: 20),
          budget: const Duration(minutes: 5),
        ),
      ) {
    sandbox = runningClient().sandbox;
  }

  /// Jedes Byte, das beim Agenten ankam.
  final List<int> typed = <int>[];

  @override
  Stream<TerminalFrame> terminal(Stream<TerminalCommand> input) {
    final StreamController<TerminalCommand> seen =
        StreamController<TerminalCommand>();
    final StreamSubscription<TerminalCommand> source = input.listen((
      TerminalCommand command,
    ) {
      if (command is TerminalKeys) {
        typed.addAll(command.bytes);
      }
      seen.add(command);
    }, onDone: seen.close);
    seen.onCancel = source.cancel;
    return super.terminal(seen.stream);
  }
}

/// Warteschlange und Terminal nebeneinander, ohne Shell.
///
/// So stehen beide heute nirgends auf einmal da: Die Warteschlange gehört zu
/// Intercept, das Terminal zur Sandbox oder über das ganze Fenster. Das
/// schwebende Terminal (HUM-300) bringt genau diese Lage, als Geschwister
/// über dem Layout; dieser Aufbau nimmt sie vorweg und misst die Regel daran,
/// bevor es sie gibt.
///
/// Ohne [queue] steht das Terminal allein da, ohne Shell und ohne einen
/// Knoten, der den Fokus zurückholt.
Future<void> pumpQueueBesideTerminal(
  WidgetTester tester,
  HeldAndRunningClient client, {
  bool queue = true,
}) async {
  await tester.binding.setSurfaceSize(const Size(1600, 900));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  final HTokens tokens = HThemeMode.dark.resolve(Brightness.dark);
  await tester.pumpWidget(
    ProviderScope(
      overrides: [
        daemonClientProvider.overrideWithValue(client),
        uiStateOverride(),
      ],
      child: WidgetsApp(
        color: tokens.colors.bg0,
        debugShowCheckedModeBanner: false,
        locale: const Locale('en'),
        localizationsDelegates: <LocalizationsDelegate<Object?>>[
          ...AppLocalizations.localizationsDelegates,
          ...hLocalizationsDelegates,
        ],
        supportedLocales: AppLocalizations.supportedLocales,
        builder: (BuildContext context, Widget? _) => HTheme(
          tokens: tokens,
          child: ColoredBox(
            color: tokens.colors.bg0,
            child: Overlay(
              initialEntries: <OverlayEntry>[
                OverlayEntry(
                  builder: (BuildContext context) => Row(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: <Widget>[
                      Expanded(
                        child: queue
                            ? const InterceptScreen()
                            : const SizedBox.shrink(),
                      ),
                      const SizedBox(
                        width: 500,
                        child: SandboxTerminal(catchUp: true),
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    ),
  );
  // Status, Anschluss des Stroms, die beiden Ankünfte bei 0 und 20 ms.
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 100));
  await settle(tester);
}

void main() {
  final TargetPlatformVariant linux = TargetPlatformVariant.only(
    TargetPlatform.linux,
  );

  testWidgets(
    'hovering_the_terminal_hands_it_the_keyboard_and_enter_reaches_the_agent',
    (WidgetTester tester) async {
      final SandboxTestClient client = runningClient();
      await pumpApp(tester, client: client);
      await pressCtrl(tester, LogicalKeyboardKey.digit4);
      await settle(tester);
      expect(find.byType(TerminalView), findsOneWidget);
      await releaseKeyboard(tester);

      final TestGesture pointer = await mouse(tester);
      await pointer.moveTo(terminalCenter(tester));
      await tester.pump();
      expect(terminalHasKeyboard(), isTrue, reason: 'no click needed');

      client.typed.clear();
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pump();
      expect(client.typed, <int>[0x0d], reason: 'Enter reaches the agent');
    },
    variant: linux,
  );

  /// Die Entscheidungstasten feuern nie, weil der Fokus durch Darüberfahren
  /// wanderte -- und nie beides.
  ///
  /// Gegenprobe zuerst: Mit dem Fokus in der Warteschlange gibt `Enter` die
  /// ausgewählte Anfrage frei und schickt dem Agenten nichts. Dann fährt der
  /// Zeiger über das Terminal; erst nachdem der Fokus dort wirklich steht,
  /// geht das nächste `Enter` an den Agenten, und die Warteschlange
  /// entscheidet nichts mehr.
  testWidgets('enter_goes_to_the_terminal_only_once_the_focus_moved', (
    WidgetTester tester,
  ) async {
    final HeldAndRunningClient client = HeldAndRunningClient();
    await pumpQueueBesideTerminal(tester, client);
    expect(find.byType(TerminalView), findsOneWidget);

    // Fokus in die Warteschlange, Auswahl scharf (`HMotion.rearm`).
    await tester.tap(find.byType(QueuePane));
    await tester.pump(const Duration(milliseconds: 500));
    await tester.pump();
    expect(terminalHasKeyboard(), isFalse);

    client.typed.clear();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    await tester.pump();
    expect(client.decisions, hasLength(1), reason: 'the queue allows');
    expect(client.typed, isEmpty, reason: 'and the agent hears nothing');

    // Die zweite Anfrage scharf werden lassen, dann den Zeiger hinüber.
    await tester.pump(const Duration(milliseconds: 500));
    await tester.pump();
    final TestGesture pointer = await mouse(tester);
    await pointer.moveTo(terminalCenter(tester));
    await tester.pump();
    expect(
      terminalHasKeyboard(),
      isTrue,
      reason: 'the focus moved before the next key',
    );

    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    await tester.pump();
    expect(client.typed, <int>[0x0d], reason: 'Enter reaches the agent');
    expect(
      client.decisions,
      hasLength(1),
      reason: 'and the queue decides nothing more',
    );
  }, variant: linux);

  testWidgets('a_text_field_keeps_the_keyboard_while_the_pointer_crosses', (
    WidgetTester tester,
  ) async {
    final HeldAndRunningClient client = HeldAndRunningClient();
    await pumpQueueBesideTerminal(tester, client);
    await tester.tap(find.byType(QueuePane));
    await tester.pump(const Duration(milliseconds: 500));
    await tester.pump();

    // `N` setzt den Fokus in das Notizfeld der ausgewählten Anfrage.
    await tester.sendKeyEvent(LogicalKeyboardKey.keyN);
    await tester.pump();
    await tester.pump();
    expect(isTextInputFocused(), isTrue, reason: 'the note field has it');

    final TestGesture pointer = await mouse(tester);
    await pointer.moveTo(terminalCenter(tester));
    await tester.pump();
    await pointer.moveBy(const Offset(12, 8));
    await tester.pump();
    expect(isTextInputFocused(), isTrue, reason: 'crossing does not take it');
    expect(terminalHasKeyboard(), isFalse);

    // Wer das Terminal dann will, fragt ausdrücklich: Ohne Shell hört hier
    // niemand auf das Kürzel, also mit einem Klick.
    await tester.tapAt(terminalCenter(tester));
    await tester.pump();
    expect(terminalHasKeyboard(), isTrue);
    expect(client.decisions, isEmpty);
  }, variant: linux);

  testWidgets('the_menu_of_the_terminal_keeps_the_keyboard_under_the_pointer', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = runningClient();
    await pumpApp(tester, client: client);
    await pressCtrl(tester, LogicalKeyboardKey.digit4);
    await settle(tester);

    final TestGesture pointer = await mouse(tester);
    final Offset center = terminalCenter(tester);
    await pointer.moveTo(center);
    await tester.pump();
    expect(terminalHasKeyboard(), isTrue);

    // Rechtsklick öffnet das Menü unter dem Zeiger, über dem Terminal.
    await tester.tapAt(
      center,
      buttons: kSecondaryMouseButton,
      kind: PointerDeviceKind.mouse,
    );
    await tester.pump();
    expect(find.text('Copy'), findsOneWidget, reason: 'the menu is open');
    await tester.pump();
    expect(
      terminalHasKeyboard(),
      isFalse,
      reason: 'the menu took the keyboard from the terminal it opened over',
    );
    expect(HLayerScope.holdsFocus(), isTrue);

    // Der Zeiger bewegt sich weiter über dem Terminal, das das Menü
    // durchscheinen lässt. `Esc` schließt dann das Menü und bricht nicht den
    // Agenten ab.
    await pointer.moveBy(const Offset(20, 10));
    await tester.pump();
    expect(terminalHasKeyboard(), isFalse);
    client.typed.clear();
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(find.text('Copy'), findsNothing, reason: 'Esc closed the menu');
    expect(client.typed, isEmpty, reason: 'Esc did not reach the agent');
    await tester.pump();
    expect(
      terminalHasKeyboard(),
      isTrue,
      reason: 'the menu gave the keyboard back to the terminal',
    );
  }, variant: linux);

  testWidgets('an_open_sheet_keeps_the_keyboard_under_the_pointer', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = runningClient();
    await pumpApp(tester, client: client);
    await pressCtrl(tester, LogicalKeyboardKey.digit4);
    await settle(tester);
    expect(terminalHasKeyboard(), isTrue, reason: 'the terminal has the keys');

    // `Ctrl+,` aus dem Terminal: Das Blatt öffnet sich und nimmt die
    // Tastatur, obwohl der Emulator sie hielt.
    await pressCtrl(tester, LogicalKeyboardKey.comma);
    await settle(tester);
    expect(find.byType(SettingsSheet), findsOneWidget);
    expect(terminalHasKeyboard(), isFalse, reason: 'the sheet took the keys');
    expect(HLayerScope.holdsFocus(), isTrue);

    client.typed.clear();
    await tester.sendKeyEvent(LogicalKeyboardKey.keyA);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(client.typed, isEmpty, reason: 'typing in the sheet stays there');

    final FocusNode? before = FocusManager.instance.primaryFocus;
    final TestGesture pointer = await mouse(tester);
    // Links im Terminal, das Blatt hängt rechts.
    final Rect view = tester.getRect(find.byType(TerminalView));
    await pointer.moveTo(view.centerLeft + const Offset(40, 0));
    await tester.pump();
    await pointer.moveBy(const Offset(10, 0));
    await tester.pump();
    expect(terminalHasKeyboard(), isFalse);
    expect(FocusManager.instance.primaryFocus, before);
  }, variant: linux);

  /// Die Palette über dem Blatt (Befund Codex): Schließt sie, geht die
  /// Tastatur an das Blatt zurück und nicht an Shell oder Terminal dahinter,
  /// und „Focus the terminal" steht dann nicht in der Liste.
  testWidgets('the_palette_over_a_sheet_gives_the_keyboard_back_to_the_sheet', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = runningClient();
    await pumpApp(tester, client: client);
    await pressCtrl(tester, LogicalKeyboardKey.digit4);
    await settle(tester);
    await pressCtrl(tester, LogicalKeyboardKey.comma);
    await settle(tester);
    expect(HLayerScope.holdsFocus(), isTrue);
    final FocusNode? sheet = FocusManager.instance.primaryFocus;

    await pressCtrl(tester, LogicalKeyboardKey.keyK);
    await tester.enterText(find.byKey(const Key('palette-input')), 'focus');
    await tester.pump();
    expect(
      find.byKey(const Key('palette-focus-terminal')),
      findsNothing,
      reason: 'not offered over a sheet',
    );
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await settle(tester);
    expect(find.byType(CommandPalette), findsNothing);
    expect(find.byType(SettingsSheet), findsOneWidget);
    expect(FocusManager.instance.primaryFocus, sheet, reason: 'back to it');
    expect(terminalHasKeyboard(), isFalse);

    // Und das Kürzel holt das Terminal nicht hinter das offene Blatt.
    await pressFocusTerminal(tester);
    expect(terminalHasKeyboard(), isFalse);
    client.typed.clear();
    await tester.sendKeyEvent(LogicalKeyboardKey.keyA);
    await tester.pump();
    expect(client.typed, isEmpty);
  }, variant: linux);

  /// Hält niemand die Tastatur -- nur der Bereich um alles --, nimmt der
  /// Zeiger sie (Befund Sonnet). Ein `primaryFocus` von `null` gibt es nach
  /// dem ersten Frame nicht mehr: Fällt der Fokus, landet er beim
  /// Wurzelbereich.
  testWidgets('hover_takes_the_keyboard_when_nobody_holds_it', (
    WidgetTester tester,
  ) async {
    final HeldAndRunningClient client = HeldAndRunningClient(count: 0);
    await pumpQueueBesideTerminal(tester, client, queue: false);
    expect(find.byType(TerminalView), findsOneWidget);
    // Jeder Schritt gibt den Fokus an den zuvor fokussierten Knoten des
    // Bereichs ab, bis keiner mehr übrig ist und der Bereich selbst ihn hält.
    for (
      int i = 0;
      i < 8 && FocusManager.instance.primaryFocus is! FocusScopeNode;
      i++
    ) {
      FocusManager.instance.primaryFocus?.unfocus(
        disposition: UnfocusDisposition.scope,
      );
      await tester.pump();
    }
    expect(
      FocusManager.instance.primaryFocus,
      isA<FocusScopeNode>(),
      reason: 'no control holds it, only the scope around them',
    );
    expect(terminalHasKeyboard(), isFalse);

    final TestGesture pointer = await mouse(tester);
    await pointer.moveTo(terminalCenter(tester));
    await tester.pump();
    expect(terminalHasKeyboard(), isTrue);
    client.typed.clear();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(client.typed, <int>[0x0d]);
  }, variant: linux);

  testWidgets('the_palette_keeps_the_keyboard_and_lists_the_terminal', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = runningClient();
    await pumpApp(tester, client: client);
    await pressCtrl(tester, LogicalKeyboardKey.keyK);
    await tester.enterText(find.byKey(const Key('palette-input')), 'terminal');
    await tester.pump();
    expect(
      find.byKey(const Key('palette-focus-terminal')),
      findsNothing,
      reason: 'no terminal on the screen in Intercept, no command',
    );
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await settle(tester);

    await pressCtrl(tester, LogicalKeyboardKey.digit4);
    await settle(tester);
    final TestGesture pointer = await mouse(tester);
    final Offset center = terminalCenter(tester);

    await pressCtrl(tester, LogicalKeyboardKey.keyK);
    await pointer.moveTo(center);
    await tester.pump();
    expect(terminalHasKeyboard(), isFalse, reason: 'the palette keeps it');
    // Auch das Kürzel holt es nicht unter der offenen Palette weg.
    await pressFocusTerminal(tester);
    expect(terminalHasKeyboard(), isFalse);
    expect(find.byType(CommandPalette), findsOneWidget);

    // Der Zeiger geht vom Terminal weg und der Befehl läuft mit `Enter`:
    // Sonst brächte das Darüberstehen die Tastatur, sobald die Palette
    // verschwindet, und der Test mäße den Zeiger statt des Befehls.
    await pointer.moveTo(const Offset(1, 1));
    await tester.pump();
    await tester.enterText(find.byKey(const Key('palette-input')), 'focus');
    await tester.pump();
    expect(find.byKey(const Key('palette-focus-terminal')), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await settle(tester);
    expect(find.byType(CommandPalette), findsNothing);
    expect(terminalHasKeyboard(), isTrue, reason: 'the command gave it');
    expectLayout(tester);
    client.typed.clear();
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(client.typed, <int>[0x1b]);
  }, variant: linux);

  testWidgets('the_shortcut_hands_the_keyboard_to_the_terminal_in_place', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = runningClient();
    await pumpApp(tester, client: client);
    await pressCtrl(tester, LogicalKeyboardKey.digit4);
    await settle(tester);
    await releaseKeyboard(tester);

    await pressFocusTerminal(tester);
    expect(terminalHasKeyboard(), isTrue);
    expectLayout(tester);
    expect(headerTitle(tester), 'Sandbox', reason: 'the layout stays');

    // Mit dem Fokus im Terminal erreicht das Kürzel den Agenten nicht
    // und `Esc` geht weiter an ihn.
    client.typed.clear();
    await pressFocusTerminal(tester);
    expect(client.typed, isEmpty, reason: 'the chord never reaches the agent');
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(client.typed, <int>[0x1b]);
  }, variant: linux);

  /// Der Weg durch `TerminalPane._onKey` für sich: Das Terminal hat die
  /// Tastatur schon (über den Zeiger), und das Kürzel darf trotzdem kein
  /// Byte beim Agenten abliefern.
  testWidgets('the_chord_never_reaches_the_agent', (WidgetTester tester) async {
    final SandboxTestClient client = runningClient();
    await pumpApp(tester, client: client);
    await pressCtrl(tester, LogicalKeyboardKey.digit4);
    await settle(tester);
    await releaseKeyboard(tester);
    final TestGesture pointer = await mouse(tester);
    await pointer.moveTo(terminalCenter(tester));
    await tester.pump();
    expect(terminalHasKeyboard(), isTrue);

    client.typed.clear();
    await pressFocusTerminal(tester);
    expect(client.typed, isEmpty, reason: 'the chord never reaches the agent');
    expect(terminalHasKeyboard(), isTrue);
  }, variant: linux);

  testWidgets('the_shortcut_also_reaches_the_terminal_over_the_whole_window', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = runningClient();
    await pumpApp(tester, client: client);
    await pressCtrl(tester, LogicalKeyboardKey.digit2);
    await pressFullWindow(tester);
    await settle(tester);
    expectFullWindow(tester);

    // Weg vom Emulator, auf den Bereich um ihn.
    FocusManager.instance.primaryFocus?.unfocus();
    await tester.pump();
    expect(terminalHasKeyboard(), isFalse);

    await pressFocusTerminal(tester);
    expect(terminalHasKeyboard(), isTrue);
    expectFullWindow(tester);
  }, variant: linux);

  // Mit laufendem Agenten lässt dasselbe Kürzel das Terminal schweben
  // (HUM-300, `terminal_float_test.dart`); ohne Agenten gibt es nichts zu
  // zeigen, und es tut nichts.
  testWidgets('without_an_agent_the_shortcut_does_nothing', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = SandboxTestClient();
    await pumpApp(tester, client: client);
    await pressCtrl(tester, LogicalKeyboardKey.digit2);
    await settle(tester);
    final FocusNode? before = FocusManager.instance.primaryFocus;

    await pressFocusTerminal(tester);
    await settle(tester);
    expect(headerTitle(tester), 'History');
    expectLayout(tester);
    expect(
      find.byKey(const Key('shell-terminal-floating')),
      findsNothing,
      reason: 'nothing to float without an agent',
    );
    expect(terminalHasKeyboard(), isFalse);
    expect(FocusManager.instance.primaryFocus, before);
    expect(client.typed, isEmpty);
  }, variant: linux);

  /// `Esc` im selben Frame wie der Rechtsklick (Befund Codex): Das Menü ist
  /// noch nicht gebaut, und trotzdem darf die Taste den Agenten nicht mehr
  /// erreichen.
  testWidgets('esc_in_the_frame_the_menu_opens_does_not_reach_the_agent', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = runningClient();
    await pumpApp(tester, client: client);
    await pressCtrl(tester, LogicalKeyboardKey.digit4);
    await settle(tester);
    final TestGesture pointer = await mouse(tester);
    final Offset center = terminalCenter(tester);
    await pointer.moveTo(center);
    await tester.pump();
    expect(terminalHasKeyboard(), isTrue);

    client.typed.clear();
    await tester.tapAt(
      center,
      buttons: kSecondaryMouseButton,
      kind: PointerDeviceKind.mouse,
    );
    expect(find.text('Copy'), findsNothing, reason: 'not built yet');
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    expect(client.typed, isEmpty, reason: 'Esc did not reach the agent');
    await tester.pump();
    await tester.pump();
    expect(find.text('Copy'), findsOneWidget);
    expect(HLayerScope.holdsFocus(), isTrue);
  }, variant: linux);

  /// Das Blatt aus dem Terminal geöffnet gibt die Tastatur dem Terminal
  /// zurück, nicht der Shell (Befund Codex).
  testWidgets('settings_opened_from_the_terminal_give_the_keys_back_to_it', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = runningClient();
    await pumpApp(tester, client: client);
    await pressCtrl(tester, LogicalKeyboardKey.digit4);
    await settle(tester);
    expect(terminalHasKeyboard(), isTrue);

    await pressCtrl(tester, LogicalKeyboardKey.comma);
    await settle(tester);
    expect(find.byType(SettingsSheet), findsOneWidget);
    expect(terminalHasKeyboard(), isFalse);

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await settle(tester);
    expect(find.byType(SettingsSheet), findsNothing);
    expect(terminalHasKeyboard(), isTrue, reason: 'back to the terminal');
    client.typed.clear();
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(client.typed, <int>[0x1b]);
  }, variant: linux);

  /// Der frühere Halter ist weg: Das Blatt beendet das ganze Fenster, und
  /// mit ihm verschwindet das Terminal, das die Tastatur hatte. Dann nimmt
  /// die Shell sie, sonst hörte `Ctrl+2` niemand.
  testWidgets('when_the_holder_is_gone_the_shell_takes_the_keys', (
    WidgetTester tester,
  ) async {
    await pumpApp(tester, client: runningClient());
    await pressFullWindow(tester);
    await settle(tester);
    expectFullWindow(tester);
    expect(terminalHasKeyboard(), isTrue);

    await pressCtrl(tester, LogicalKeyboardKey.comma);
    await settle(tester);
    expect(find.byType(SettingsSheet), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await settle(tester);
    expect(find.byType(SettingsSheet), findsNothing);

    await pressCtrl(tester, LogicalKeyboardKey.digit2);
    await settle(tester);
    expect(headerTitle(tester), 'History', reason: 'the shell hears Ctrl+2');
  }, variant: linux);

  /// Einstellungen aus dem ganzen Fenster über der Sandbox: Das Terminal
  /// kehrt in sein Pane zurück und darf dem Blatt die Tastatur nicht wieder
  /// wegnehmen, sobald die Sitzung bekannt ist. Nach dem Schließen ist der
  /// frühere Halter weg, und weil die Sandbox den Fokus nicht von selbst
  /// nimmt, muss die Shell ihn übernehmen.
  testWidgets('settings_over_the_returning_pane_keep_the_keys', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = runningClient();
    await pumpApp(tester, client: client);
    await pressCtrl(tester, LogicalKeyboardKey.digit4);
    await settle(tester);
    await pressFullWindow(tester);
    await settle(tester);
    expectFullWindow(tester);

    await pressCtrl(tester, LogicalKeyboardKey.comma);
    await settle(tester);
    await tester.pump(const Duration(milliseconds: 600));
    await settle(tester);
    expect(find.byType(SettingsSheet), findsOneWidget);
    expect(terminalHasKeyboard(), isFalse, reason: 'the sheet keeps it');
    client.typed.clear();
    await tester.sendKeyEvent(LogicalKeyboardKey.keyA);
    await tester.pump();
    expect(client.typed, isEmpty, reason: 'typing stays in the sheet');

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await settle(tester);
    expect(find.byType(SettingsSheet), findsNothing);
    await pressCtrl(tester, LogicalKeyboardKey.digit2);
    await settle(tester);
    expect(headerTitle(tester), 'History', reason: 'the shell hears Ctrl+2');
  }, variant: linux);

  group('TerminalFocusActivator', () {
    /// Ein Druck mit [logical] auf der physischen Taste [physical].
    Future<List<Intent>> pressOn(
      WidgetTester tester, {
      required LogicalKeyboardKey logical,
      PhysicalKeyboardKey physical = PhysicalKeyboardKey.backquote,
      bool control = true,
      bool shift = false,
      bool repeat = false,
    }) async {
      final List<Intent> fired = <Intent>[];
      await tester.pumpWidget(
        Shortcuts(
          shortcuts: shellShortcuts(),
          child: Actions(
            actions: <Type, Action<Intent>>{
              FocusTerminalIntent: CallbackAction<FocusTerminalIntent>(
                onInvoke: (FocusTerminalIntent intent) {
                  fired.add(intent);
                  return null;
                },
              ),
            },
            child: const Focus(autofocus: true, child: SizedBox.shrink()),
          ),
        ),
      );
      await tester.pump();
      if (control) {
        await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
      }
      if (shift) {
        await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
      }
      await tester.sendKeyDownEvent(logical, physicalKey: physical);
      if (repeat) {
        await tester.sendKeyRepeatEvent(logical, physicalKey: physical);
      }
      await tester.sendKeyUpEvent(logical, physicalKey: physical);
      if (shift) {
        await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
      }
      if (control) {
        await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
      }
      return fired;
    }

    testWidgets('a_dead_key_left_of_one_still_counts', (
      WidgetTester tester,
    ) async {
      // Auf deutscher Belegung liefert die Taste links der 1 ein totes `^`,
      // auf französischer ein `²`. Der Simulator kennt beide nicht; ein
      // anderes Zeichen auf derselben physischen Taste steht für sie.
      expect(
        await pressOn(tester, logical: LogicalKeyboardKey.backslash),
        hasLength(1),
      );
    }, variant: linux);

    testWidgets('exactly_ctrl_and_no_repeats', (WidgetTester tester) async {
      expect(
        await pressOn(tester, logical: LogicalKeyboardKey.backquote),
        hasLength(1),
      );
      expect(
        await pressOn(
          tester,
          logical: LogicalKeyboardKey.backquote,
          control: false,
        ),
        isEmpty,
        reason: 'a plain backquote is a character',
      );
      expect(
        await pressOn(
          tester,
          logical: LogicalKeyboardKey.backquote,
          shift: true,
        ),
        isEmpty,
        reason: 'Ctrl+Shift is another chord',
      );
      expect(
        await pressOn(
          tester,
          logical: LogicalKeyboardKey.backquote,
          repeat: true,
        ),
        hasLength(1),
        reason: 'a held chord asks once',
      );
    }, variant: linux);
  });
}
