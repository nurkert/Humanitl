// Das Terminal über dem ganzen Fenster (HUM-298): aus jedem Abschnitt mit
// `Ctrl+Shift+Enter` oder dem Knopf über dem Terminal, und mit demselben Griff
// zurück an die Stelle, von der man kam.
//
// Linux als Plattform: Ohne Variante spielt `flutter test` Android, und die
// Tasten dieses Produkts sind die eines Linux-Desktops.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/widgets/queue_pane.dart';
import 'package:humanitl/features/sandbox/providers/sandbox_status_provider.dart';
import 'package:humanitl/features/shell/shell_screen.dart';
import 'package:humanitl/features/shell/widgets/command_palette.dart';
import 'package:humanitl/features/shell/widgets/icon_rail.dart';
import 'package:humanitl/features/shell/widgets/status_bar.dart';
import 'package:xterm2/ui.dart';

import '../../harness/app_harness.dart';
import '../sandbox/harness.dart';

/// Der Titel des Abschnitts im Kopf der Shell.
String headerTitle(WidgetTester tester) =>
    tester.widget<Text>(find.byKey(const Key('header-section-title'))).data!;

/// Die Beschriftung des Halte-Badges im Kopf.
String heldBadge(WidgetTester tester) =>
    tester.widget<HBadge>(find.byKey(const Key('header-held-badge'))).text;

/// Das Terminal steht über dem ganzen Fenster: sein Wirt in der Shell ist
/// da, Leiste, Abschnitte und Statusleiste sind es nicht.
void expectFullWindow(WidgetTester tester) {
  expect(
    find.byKey(const Key('shell-terminal-full-window')),
    findsOneWidget,
    reason: 'the shell hosts the terminal',
  );
  expect(find.byType(RailEntry), findsNothing, reason: 'no rail');
  expect(find.byType(StatusBar), findsNothing, reason: 'no status bar');
  // Auch unter den verdeckten Abschnitten nicht: Zwei Ansichten derselben
  // Sitzung meldeten beide ihre Größe und nähmen beide Tasten.
  expect(
    find.byKey(const Key('sandbox-terminal'), skipOffstage: false),
    findsOneWidget,
    reason: 'exactly one view of the terminal, the one over the window',
  );
}

/// Leiste und Abschnitte sind zurück, der Wirt ist weg.
void expectLayout(WidgetTester tester) {
  expect(find.byKey(const Key('shell-terminal-full-window')), findsNothing);
  expect(find.byType(RailEntry), findsWidgets);
  expect(find.byType(StatusBar), findsOneWidget);
}

/// `Ctrl+Shift+Enter`, oder mit [key] das `Enter` des Ziffernblocks.
Future<void> pressFullWindow(
  WidgetTester tester, {
  LogicalKeyboardKey key = LogicalKeyboardKey.enter,
}) async {
  await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
  await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
  await tester.sendKeyEvent(key);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
  await tester.pump();
  await tester.pump();
}

/// Ein paar Frames: Status, Anschluss des Stroms, Gruß des Fakes, Fokus.
Future<void> settle(WidgetTester tester) async {
  for (int i = 0; i < 4; i++) {
    await tester.pump();
  }
}

void main() {
  final TargetPlatformVariant linux = TargetPlatformVariant.only(
    TargetPlatform.linux,
  );

  testWidgets(
    'the_shortcut_shows_the_terminal_over_the_window_from_another_section_and_returns',
    (WidgetTester tester) async {
      await pumpApp(tester, client: runningClient());
      await pressCtrl(tester, LogicalKeyboardKey.digit2);
      expect(headerTitle(tester), 'History');
      expectLayout(tester);

      await pressFullWindow(tester);
      await settle(tester);
      expectFullWindow(tester);
      expect(find.byType(TerminalView), findsOneWidget);

      // Zurück mit dem `Enter` des Ziffernblocks: dieselbe Taste.
      await pressFullWindow(tester, key: LogicalKeyboardKey.numpadEnter);
      await settle(tester);
      expectLayout(tester);
      expect(
        headerTitle(tester),
        'History',
        reason: 'back on the section it came from',
      );
      expect(tester.takeException(), isNull);
    },
    variant: linux,
  );

  testWidgets(
    'the_shortcut_works_with_the_terminal_focused_and_esc_reaches_the_agent',
    (WidgetTester tester) async {
      final SandboxTestClient client = runningClient();
      await pumpApp(tester, client: client);
      await pressCtrl(tester, LogicalKeyboardKey.digit3);

      await pressFullWindow(tester);
      await settle(tester);
      expectFullWindow(tester);

      // Ohne Klick: Wer das Terminal über das Fenster holt, tippt sofort
      // beim Agenten. `Esc` gehört ihm, OpenCode und pi brechen damit ab.
      client.typed.clear();
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pump();
      expect(client.typed, <int>[0x1b], reason: 'Esc reaches the agent');
      expectFullWindow(tester);

      // `Ctrl+Shift+Enter` ist für den Emulator eine Taste wie jede andere;
      // es darf ihn nicht erreichen und muss die Ansicht zurückstellen.
      client.typed.clear();
      await pressFullWindow(tester);
      await settle(tester);
      expect(
        client.typed,
        isEmpty,
        reason: 'the chord never reaches the agent',
      );
      expectLayout(tester);
      expect(headerTitle(tester), 'Rules');
    },
    variant: linux,
  );

  testWidgets('the_button_over_the_terminal_toggles_the_whole_window', (
    WidgetTester tester,
  ) async {
    await pumpApp(tester, client: runningClient());
    await pressCtrl(tester, LogicalKeyboardKey.digit4);
    await settle(tester);
    expect(headerTitle(tester), 'Sandbox');

    await tester.tap(find.byKey(const Key('sandbox-terminal-full-window')));
    await settle(tester);
    expectFullWindow(tester);

    await tester.tap(find.byKey(const Key('sandbox-terminal-full-window')));
    await settle(tester);
    expectLayout(tester);
    expect(headerTitle(tester), 'Sandbox');
    expect(
      find.byKey(const Key('sandbox-terminal')),
      findsOneWidget,
      reason: 'the terminal is back in its pane',
    );

    // `Ctrl+4` aus dem ganzen Fenster zeigt den Abschnitt, auf dem man schon
    // steht: Die Navigation ändert sich nicht, die Ansicht muss es trotzdem.
    await tester.tap(find.byKey(const Key('sandbox-terminal-full-window')));
    await settle(tester);
    expectFullWindow(tester);
    await pressCtrl(tester, LogicalKeyboardKey.digit4);
    await settle(tester);
    expectLayout(tester);
    expect(headerTitle(tester), 'Sandbox');
  }, variant: linux);

  testWidgets('without_a_session_the_whole_window_shows_the_empty_terminal', (
    WidgetTester tester,
  ) async {
    await pumpApp(tester, client: FakeDaemonClient());
    await pressFullWindow(tester);
    await settle(tester);
    expectFullWindow(tester);
    expect(find.byType(TerminalView), findsNothing);
    expect(
      find.textContaining('Nothing runs'),
      findsOneWidget,
      reason: 'the usual empty state of the terminal',
    );
    // Und der Weg zurück mit der Maus steht auch ohne Sitzung da.
    await tester.tap(find.byKey(const Key('sandbox-terminal-full-window')));
    await settle(tester);
    expectLayout(tester);
    expect(headerTitle(tester), 'Intercept');
  }, variant: linux);

  testWidgets('the_palette_lists_the_whole_window_and_a_section_ends_it', (
    WidgetTester tester,
  ) async {
    await pumpApp(tester, client: FakeDaemonClient());
    await pressCtrl(tester, LogicalKeyboardKey.keyK);
    await tester.enterText(find.byKey(const Key('palette-input')), 'terminal');
    await tester.pump();
    await tester.tap(
      find.byKey(const Key('palette-toggle-terminal-full-window')),
    );
    await settle(tester);
    expect(find.byType(CommandPalette), findsNothing);
    expectFullWindow(tester);

    // „Go to History" will History sehen und beendet das ganze Fenster.
    await pressCtrl(tester, LogicalKeyboardKey.keyK);
    await tester.tap(find.byKey(const Key('palette-go-history')));
    await settle(tester);
    expectLayout(tester);
    expect(headerTitle(tester), 'History');
  }, variant: linux);

  /// Die Taste entscheidet nichts (HUM-298, Entscheidung des Eigentümers).
  ///
  /// `Enter` gibt eine gehaltene Anfrage frei, `Ctrl+Enter` sperrt sie. Mit
  /// dem Fokus in der Warteschlange und scharfer Auswahl darf
  /// `Ctrl+Shift+Enter` keine der beiden auslösen, sondern nur das ganze
  /// Fenster zeigen. Die Gegenprobe am Ende zeigt, dass derselbe Aufbau mit
  /// `Enter` wirklich entscheidet.
  testWidgets('the_chord_decides_nothing_in_the_focused_queue', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.burst(
      count: 1,
      spacing: const Duration(milliseconds: 20),
    );
    await pumpApp(tester, client: client);
    await tester.pump(const Duration(milliseconds: 100));
    await tester.pump();
    expect(heldBadge(tester), '1 held');

    // Fokus in die Warteschlange, Auswahl scharf (`HMotion.rearm`).
    await tester.tap(find.byType(QueuePane));
    await tester.pump(const Duration(milliseconds: 500));
    await tester.pump();

    for (final LogicalKeyboardKey key in <LogicalKeyboardKey>[
      LogicalKeyboardKey.enter,
      LogicalKeyboardKey.numpadEnter,
    ]) {
      await pressFullWindow(tester, key: key);
      await settle(tester);
      expect(client.decisions, isEmpty, reason: '${key.keyLabel} decided');
      expectFullWindow(tester);
      expect(heldBadge(tester), '1 held');

      await pressFullWindow(tester, key: key);
      await settle(tester);
      expectLayout(tester);
      expect(headerTitle(tester), 'Intercept');
      await tester.tap(find.byType(QueuePane));
      await tester.pump(const Duration(milliseconds: 500));
      await tester.pump();
    }

    // Gegenprobe: `Enter` an derselben Stelle gibt die Anfrage frei.
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    await tester.pump();
    expect(client.decisions, hasLength(1));
  }, variant: linux);

  testWidgets('going_to_the_section_underneath_ends_the_whole_window', (
    WidgetTester tester,
  ) async {
    await pumpApp(tester, client: runningClient());
    await pressCtrl(tester, LogicalKeyboardKey.digit4);
    await pressFullWindow(tester);
    await settle(tester);
    expectFullWindow(tester);

    // „Go to Sandbox" ändert die Navigation nicht, die Ansicht aber doch.
    await pressCtrl(tester, LogicalKeyboardKey.keyK);
    await tester.tap(find.byKey(const Key('palette-go-sandbox')));
    await settle(tester);
    expectLayout(tester);
    expect(headerTitle(tester), 'Sandbox');
  }, variant: linux);

  testWidgets('after_the_palette_the_keys_go_back_to_the_terminal', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = runningClient();
    await pumpApp(tester, client: client);
    await pressCtrl(tester, LogicalKeyboardKey.digit2);
    await pressFullWindow(tester);
    await settle(tester);
    expectFullWindow(tester);

    await pressCtrl(tester, LogicalKeyboardKey.keyK);
    expect(find.byType(CommandPalette), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await settle(tester);
    expect(find.byType(CommandPalette), findsNothing);
    expectFullWindow(tester);

    client.typed.clear();
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(client.typed, <int>[0x1b], reason: 'Esc reaches the agent again');
  }, variant: linux);

  testWidgets('a_session_that_starts_later_gets_the_keys', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = SandboxTestClient();
    await pumpApp(tester, client: client);
    await pressCtrl(tester, LogicalKeyboardKey.digit2);
    await pressFullWindow(tester);
    await settle(tester);
    expectFullWindow(tester);
    expect(find.byType(TerminalView), findsNothing);

    // Die Sitzung startet nebenan, etwa über `humanitl run`.
    final SandboxTestClient running = runningClient();
    client.sandbox = running.sandbox;
    await ProviderScope.containerOf(tester.element(find.byType(ShellScreen)))
        .read(sandboxStatusProvider.notifier)
        .refresh();
    await settle(tester);
    expect(find.byType(TerminalView), findsOneWidget);

    client.typed.clear();
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(client.typed, <int>[0x1b], reason: 'the late terminal has the keys');
  }, variant: linux);
}
