// Der Text im Terminal lässt sich vergrößern und verkleinern (HUM-301).
//
// `Ctrl` mit `+`, `=` oder dem Plus des Ziffernblocks macht ihn größer,
// `Ctrl` mit `-` kleiner, `Ctrl+Shift+0` und `Ctrl` mit der Null des
// Ziffernblocks bringen die Vorgabe zurück; `Ctrl` mit dem Mausrad über dem
// Emulator zoomt auch. Keiner dieser Griffe erreicht den Agenten, und
// keiner tut etwas, solange kein Terminal zu sehen ist. Die Größe übersteht
// einen Neustart, und nach jedem Zoom geht die neue Geometrie zum Daemon.
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
import 'package:humanitl/core/ui/terminal_zoom.dart';
import 'package:humanitl/core/ui_state/ui_state_file.dart';
import 'package:humanitl/features/sandbox/providers/terminal_provider.dart';
import 'package:humanitl/features/shell/providers/rail.dart';
import 'package:xterm2/ui.dart';

import '../../harness/app_harness.dart';
import '../sandbox/harness.dart';
import 'terminal_full_window_test.dart' show settle;
import 'terminal_keyboard_test.dart'
    show
        pressFocusTerminal,
        releaseKeyboard,
        terminalCenter,
        terminalHasKeyboard;

/// Die Schriftgröße, mit der der Emulator gerade zeichnet.
double fontSize(WidgetTester tester) =>
    tester.widget<TerminalView>(find.byType(TerminalView)).textStyle.fontSize;

/// Spalten und Zeilen, die der Emulator gerade zählt.
(int, int) geometry(WidgetTester tester) {
  final TerminalView view = tester.widget<TerminalView>(
    find.byType(TerminalView),
  );
  return (view.terminal.viewWidth, view.terminal.viewHeight);
}

/// Tasten, für die der Simulator keinen Linux-Code kennt; sie kommen als
/// Android-Ereignis. Die Tastatur der App sieht davon nur die logische Taste.
final Set<LogicalKeyboardKey> _androidOnly = <LogicalKeyboardKey>{
  LogicalKeyboardKey.add,
  LogicalKeyboardKey.numpadSubtract,
  LogicalKeyboardKey.numpad0,
};

/// [key] mit gehaltenem `Ctrl`, und mit `Shift`, wenn [shift] gilt.
Future<void> chord(
  WidgetTester tester,
  LogicalKeyboardKey key, {
  bool shift = false,
}) async {
  await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
  if (shift) {
    await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
  }
  // Das `+` einer deutschen Tastatur liegt auf der Taste rechts vom `Ü`;
  // der Simulator kennt für `add` keinen Linux-Code, also
  // kommt es als Android-Ereignis.
  await tester.sendKeyEvent(
    key,
    platform: _androidOnly.contains(key) ? 'android' : null,
    physicalKey: key == LogicalKeyboardKey.add
        ? PhysicalKeyboardKey.bracketRight
        : null,
  );
  if (shift) {
    await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
  }
  await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
  await tester.pump();
  await tester.pump();
}

/// Die App mit laufender Sitzung, in der Sandbox, mit der Tastatur im
/// Terminal -- geholt mit ``Ctrl+` `` (HUM-303), damit kein Zeiger über dem
/// Emulator stehen bleibt, der den Test mit dem Rad stört.
Future<SandboxTestClient> pumpSandboxWithTerminalFocused(
  WidgetTester tester, {
  Override? uiState,
}) async {
  final SandboxTestClient client = runningClient();
  await pumpApp(tester, client: client, uiState: uiState);
  await pressCtrl(tester, LogicalKeyboardKey.digit4);
  await settle(tester);
  expect(find.byType(TerminalView), findsOneWidget);
  await releaseKeyboard(tester);
  await pressFocusTerminal(tester);
  expect(terminalHasKeyboard(), isTrue);
  return client;
}

/// Eine eigene Zustandsdatei, die zwei Starts der App teilen.
Override sharedUiState(String path) =>
    uiStateFileProvider.overrideWithValue(UiStateFile(path));

/// Ein leeres Verzeichnis für die Zustandsdatei dieses Tests.
String freshStatePath() {
  final Directory dir = Directory.systemTemp.createTempSync('humanitl-zoom');
  addTearDown(() => dir.deleteSync(recursive: true));
  return '${dir.path}/${UiStateFile.fileName}';
}

void main() {
  final TargetPlatformVariant linux = TargetPlatformVariant.only(
    TargetPlatform.linux,
  );

  testWidgets('the_zoom_chords_change_the_size_and_never_reach_the_agent', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = await pumpSandboxWithTerminalFocused(
      tester,
    );
    expect(fontSize(tester), terminalFontSizeDefault);
    client.typed.clear();

    // `Ctrl+=`, das `+` einer US-Tastatur (`Shift+=`), das `+` einer
    // deutschen und das des Ziffernblocks.
    await chord(tester, LogicalKeyboardKey.equal);
    expect(fontSize(tester), 14);
    await chord(tester, LogicalKeyboardKey.equal, shift: true);
    expect(fontSize(tester), 15);
    await chord(tester, LogicalKeyboardKey.add);
    expect(fontSize(tester), 16);
    await chord(tester, LogicalKeyboardKey.numpadAdd);
    expect(fontSize(tester), 17);

    await chord(tester, LogicalKeyboardKey.minus);
    expect(fontSize(tester), 16);
    await chord(tester, LogicalKeyboardKey.numpadSubtract);
    expect(fontSize(tester), 15);

    await chord(tester, LogicalKeyboardKey.digit0, shift: true);
    expect(fontSize(tester), terminalFontSizeDefault, reason: 'reset');
    await chord(tester, LogicalKeyboardKey.equal);
    await chord(tester, LogicalKeyboardKey.numpad0);
    expect(fontSize(tester), terminalFontSizeDefault, reason: 'reset');

    expect(client.typed, isEmpty, reason: 'no chord reached the agent');
    expect(terminalHasKeyboard(), isTrue);

    // Gegenprobe: Eine andere Taste geht weiter an den Agenten.
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(client.typed, <int>[0x0d]);
  }, variant: linux);

  /// `Ctrl+0` bleibt die Leiste (HUM-254), auch aus dem Terminal, und
  /// `Ctrl+Shift+-` -- `Ctrl+_`, Rückgängig in readline -- zoomt nicht.
  testWidgets('ctrl_0_stays_the_rail_and_ctrl_shift_minus_does_not_zoom', (
    WidgetTester tester,
  ) async {
    await pumpSandboxWithTerminalFocused(tester);
    await chord(tester, LogicalKeyboardKey.equal);
    expect(fontSize(tester), 14);
    final ProviderContainer container = ProviderScope.containerOf(
      tester.element(find.byType(TerminalView)),
    );
    final bool railBefore = container.read(railExpandedProvider);

    await chord(tester, LogicalKeyboardKey.digit0);
    expect(fontSize(tester), 14, reason: 'Ctrl+0 is not the reset');
    expect(container.read(railExpandedProvider), !railBefore);

    await chord(tester, LogicalKeyboardKey.minus, shift: true);
    expect(fontSize(tester), 14, reason: 'Ctrl+Shift+- is not zoom out');
  }, variant: linux);

  testWidgets('without_a_terminal_on_the_screen_the_chords_do_nothing', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = runningClient();
    await pumpApp(tester, client: client);
    await pressCtrl(tester, LogicalKeyboardKey.digit2);
    await settle(tester);
    expect(find.byType(TerminalView), findsNothing);
    final ProviderContainer container = ProviderScope.containerOf(
      tester.element(find.byKey(const Key('header-section-title'))),
    );

    await chord(tester, LogicalKeyboardKey.equal);
    await chord(tester, LogicalKeyboardKey.minus);
    await chord(tester, LogicalKeyboardKey.minus);
    expect(container.read(terminalFontSizeProvider), terminalFontSizeDefault);
  }, variant: linux);

  testWidgets('the_palette_zooms_and_lists_zoom_only_with_a_terminal', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = runningClient();
    await pumpApp(tester, client: client);
    await pressCtrl(tester, LogicalKeyboardKey.keyK);
    await tester.enterText(find.byKey(const Key('palette-input')), 'terminal');
    await tester.pump();
    for (final String id in <String>['zoomIn', 'zoomOut', 'reset']) {
      expect(
        find.byKey(Key('palette-terminal-$id')),
        findsNothing,
        reason: 'no terminal on the screen in Intercept, no command',
      );
    }
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await settle(tester);

    await pressCtrl(tester, LogicalKeyboardKey.digit4);
    await settle(tester);
    client.typed.clear();

    Future<void> run(String query, String id) async {
      await pressCtrl(tester, LogicalKeyboardKey.keyK);
      await tester.enterText(find.byKey(const Key('palette-input')), query);
      await tester.pump();
      expect(find.byKey(Key('palette-terminal-$id')), findsOneWidget);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await settle(tester);
    }

    await run('terminal larger', 'zoomIn');
    expect(fontSize(tester), 14);
    await run('terminal larger', 'zoomIn');
    expect(fontSize(tester), 15);
    await run('terminal smaller', 'zoomOut');
    expect(fontSize(tester), 14);
    await run('terminal default', 'reset');
    expect(fontSize(tester), terminalFontSizeDefault);
    expect(client.typed, isEmpty);
  }, variant: linux);

  /// Mit Rücklauf im Emulator: Ohne ihn meldet sich dessen `Scrollable` gar
  /// nicht beim Rad, und der Wettlauf um das Ereignis fände nicht statt.
  testWidgets('ctrl_wheel_over_the_terminal_zooms_and_the_wheel_alone_not', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = await pumpSandboxWithTerminalFocused(
      tester,
    );
    tester
        .widget<TerminalView>(find.byType(TerminalView))
        .terminal
        .write(List<String>.generate(200, (int i) => 'line $i').join('\r\n'));
    await tester.pump();
    client.typed.clear();
    final TestPointer wheel = TestPointer(7, PointerDeviceKind.mouse);
    final Offset center = terminalCenter(tester);
    await tester.sendEventToBinding(wheel.hover(center));

    // Das Rad allein ändert die Größe nicht.
    await tester.sendEventToBinding(wheel.scroll(const Offset(0, -60)));
    await tester.pump();
    expect(fontSize(tester), terminalFontSizeDefault);

    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    await tester.sendEventToBinding(wheel.scroll(const Offset(0, -60)));
    await tester.pump();
    expect(fontSize(tester), 14, reason: 'Ctrl + wheel up is larger');
    await tester.sendEventToBinding(wheel.scroll(const Offset(0, -60)));
    await tester.pump();
    expect(fontSize(tester), 15);
    await tester.sendEventToBinding(wheel.scroll(const Offset(0, 60)));
    await tester.pump();
    expect(fontSize(tester), 14, reason: 'Ctrl + wheel down is smaller');
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    await tester.pump();
    expect(client.typed, isEmpty);
  }, variant: linux);

  /// Gehaltene Griffe: Drücken, Wiederholungen, Loslassen. Keines dieser
  /// Ereignisse fällt zum Emulator durch -- ein Agent mit dem Tastatur-
  /// protokoll von kitty bekäme sonst Wiederholung und Loslassen eines
  /// Griffs, dessen Anfang er nie sah --, und die Vorgabe gilt nur einmal.
  testWidgets('a_held_zoom_chord_never_falls_through_to_the_agent', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = await pumpSandboxWithTerminalFocused(
      tester,
    );
    client.typed.clear();

    // `Ctrl+=` gehalten: jede Wiederholung ein Schritt.
    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    expect(await tester.sendKeyDownEvent(LogicalKeyboardKey.equal), isTrue);
    for (int i = 0; i < 3; i++) {
      expect(await tester.sendKeyRepeatEvent(LogicalKeyboardKey.equal), isTrue);
    }
    expect(
      await tester.sendKeyUpEvent(LogicalKeyboardKey.equal),
      isTrue,
      reason: 'the release stays with the application',
    );
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    await tester.pump();
    expect(fontSize(tester), 17);

    // `Ctrl+Shift+0` gehalten: einmal die Vorgabe, die Wiederholungen und
    // das Loslassen nimmt die Anwendung, ohne sie noch einmal anzuwenden.
    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
    expect(await tester.sendKeyDownEvent(LogicalKeyboardKey.digit0), isTrue);
    await tester.pump();
    expect(fontSize(tester), terminalFontSizeDefault);
    // Zwischen Drücken und Wiederholung zoomt das Rad; die Wiederholung
    // der Vorgabe darf das nicht zurücknehmen.
    final ProviderContainer container = ProviderScope.containerOf(
      tester.element(find.byType(TerminalView)),
    );
    container.read(terminalFontSizeProvider.notifier).zoomIn();
    for (int i = 0; i < 3; i++) {
      expect(
        await tester.sendKeyRepeatEvent(LogicalKeyboardKey.digit0),
        isTrue,
        reason: 'the repeat stays with the application',
      );
    }
    expect(await tester.sendKeyUpEvent(LogicalKeyboardKey.digit0), isTrue);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    await tester.pump();
    expect(fontSize(tester), 14, reason: 'the reset applied once');
    expect(client.typed, isEmpty);

    // Gegenprobe: Danach gehören die Tasten wieder dem Agenten.
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(client.typed, contains(0x0d));
  }, variant: linux);

  testWidgets('the_size_stops_at_10_and_28', (WidgetTester tester) async {
    await pumpSandboxWithTerminalFocused(tester);
    for (int i = 0; i < 25; i++) {
      await chord(tester, LogicalKeyboardKey.equal);
    }
    expect(fontSize(tester), terminalFontSizeMax);
    expect(terminalFontSizeMax, 28);
    for (int i = 0; i < 25; i++) {
      await chord(tester, LogicalKeyboardKey.minus);
    }
    expect(fontSize(tester), terminalFontSizeMin);
    expect(terminalFontSizeMin, 10);
  }, variant: linux);

  testWidgets('a_zoomed_size_is_there_again_after_a_restart', (
    WidgetTester tester,
  ) async {
    final String path = freshStatePath();
    await pumpSandboxWithTerminalFocused(tester, uiState: sharedUiState(path));
    await chord(tester, LogicalKeyboardKey.equal);
    await chord(tester, LogicalKeyboardKey.equal);
    expect(fontSize(tester), 15);
    expect(
      (jsonDecode(File(path).readAsStringSync())
          as Map<String, Object?>)[terminalFontSizeKey],
      15,
    );

    // Neustart: alles abbauen, dann eine frische App über derselben Datei.
    await tester.pumpWidget(const SizedBox());
    await tester.pump();
    await pumpSandboxWithTerminalFocused(tester, uiState: sharedUiState(path));
    expect(fontSize(tester), 15, reason: 'the size survived the restart');

    // Und die Vorgabe gilt wieder als keine Wahl.
    await chord(tester, LogicalKeyboardKey.digit0, shift: true);
    expect(fontSize(tester), terminalFontSizeDefault);
    expect(
      (jsonDecode(File(path).readAsStringSync())
          as Map<String, Object?>)[terminalFontSizeKey],
      isNull,
    );
  }, variant: linux);

  testWidgets('a_zoom_sends_the_new_geometry_to_the_daemon', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = await pumpSandboxWithTerminalFocused(
      tester,
    );
    await tester.pump(terminalResizeInterval * 2);
    final (int, int) before = geometry(tester);
    client.resizes.clear();

    for (int i = 0; i < 3; i++) {
      await chord(tester, LogicalKeyboardKey.equal);
    }
    await tester.pump(terminalResizeInterval * 2);
    await tester.pump();
    final (int, int) after = geometry(tester);
    expect(after.$1, lessThan(before.$1), reason: 'fewer columns');
    expect(after.$2, lessThan(before.$2), reason: 'fewer rows');
    expect(client.resizes, isNotEmpty, reason: 'the daemon heard of it');
    expect(client.resizes.last, after);
  }, variant: linux);
}
