// Folgen des schwebenden Terminals (HUM-315): Auf der Sandbox-Seite steht es
// im Pane und schwebt beim Verlassen wieder, „Go to terminal" der leeren
// Warteschlange lässt es schweben und gibt ihm die Tastatur, und ohne freie
// Ecke schrumpft es zu einem schmalen Streifen statt zu verschwinden.
//
// Linux als Plattform: Ohne Variante spielt `flutter test` Android, und die
// Tasten dieses Produkts sind die eines Linux-Desktops.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ui/terminal_placement.dart';
import 'package:humanitl/features/sandbox/providers/sandbox_status_provider.dart';
import 'package:humanitl/features/settings/settings_sheet.dart';
import 'package:humanitl/features/shell/shell_screen.dart';
import 'package:humanitl/features/shell/widgets/floating_terminal.dart';
import 'package:xterm2/ui.dart';

import '../../harness/app_harness.dart';
import '../sandbox/harness.dart';
import 'terminal_float_test.dart'
    show expectOneView, floating, placement, pressFloat, window;
import 'terminal_full_window_test.dart' show headerTitle, settle;
import 'terminal_keyboard_test.dart'
    show pressFocusTerminal, terminalHasKeyboard;

final Finder strip = find.byKey(const Key('shell-terminal-strip'));

/// Die Ansicht im Pane der Sandbox-Seite.
final Finder paneView = find.descendant(
  of: find.byKey(const Key('sandbox-terminal')),
  matching: find.byType(TerminalView),
);

ProviderContainer containerOf(WidgetTester tester) =>
    ProviderScope.containerOf(tester.element(find.byType(ShellScreen)));

void main() {
  final TargetPlatformVariant linux = TargetPlatformVariant.only(
    TargetPlatform.linux,
  );

  group('the Sandbox page', () {
    testWidgets('a_floating_terminal_docks_on_the_page_and_floats_on_leaving', (
      WidgetTester tester,
    ) async {
      await pumpApp(tester, client: runningClient(), size: window);
      await pressCtrl(tester, LogicalKeyboardKey.digit2);
      await pressFloat(tester);
      expect(floating, findsOneWidget);
      final Object emulator = tester
          .widget<TerminalView>(
            find.descendant(of: floating, matching: find.byType(TerminalView)),
          )
          .terminal;

      await pressCtrl(tester, LogicalKeyboardKey.digit4);
      await settle(tester);
      expect(headerTitle(tester), 'Sandbox');
      expect(floating, findsNothing, reason: 'no window over its own page');
      expect(paneView, findsOneWidget, reason: 'it stands in its pane');
      expect(
        identical(tester.widget<TerminalView>(paneView).terminal, emulator),
        isTrue,
        reason: 'the same session',
      );
      expectOneView();
      expect(
        placement(tester),
        TerminalPlacement.floating,
        reason: 'docking for the page is no choice of the person',
      );

      await pressCtrl(tester, LogicalKeyboardKey.digit3);
      await settle(tester);
      expect(headerTitle(tester), 'Rules');
      expect(floating, findsOneWidget, reason: 'it floats again');
      expectOneView();
      expect(tester.takeException(), isNull);
    }, variant: linux);

    testWidgets('a_terminal_never_floated_stays_in_its_pane_on_leaving', (
      WidgetTester tester,
    ) async {
      await pumpApp(tester, client: runningClient(), size: window);
      await pressCtrl(tester, LogicalKeyboardKey.digit4);
      await settle(tester);
      await pressCtrl(tester, LogicalKeyboardKey.digit2);
      await settle(tester);
      expect(floating, findsNothing);
      expect(placement(tester), TerminalPlacement.pane);
    }, variant: linux);

    testWidgets('an_end_on_the_page_docks_for_good', (
      WidgetTester tester,
    ) async {
      final SandboxTestClient client = runningClient();
      await pumpApp(tester, client: client, size: window);
      await pressCtrl(tester, LogicalKeyboardKey.digit2);
      await pressFloat(tester);
      await pressCtrl(tester, LogicalKeyboardKey.digit4);
      await settle(tester);
      expect(placement(tester), TerminalPlacement.floating);

      client.sandbox = client.sandbox.copyWith(agentRunning: false);
      await containerOf(tester).read(sandboxStatusProvider.notifier).refresh();
      await settle(tester);
      expect(
        placement(tester),
        TerminalPlacement.pane,
        reason: 'no session left to float',
      );

      await pressCtrl(tester, LogicalKeyboardKey.digit2);
      await settle(tester);
      await tester.pump(terminalFloatLinger);
      expect(floating, findsNothing, reason: 'leaving floats nothing');
    }, variant: linux);

    testWidgets('the_keyboard_follows_onto_the_page_and_not_off_it', (
      WidgetTester tester,
    ) async {
      await pumpApp(tester, client: runningClient(), size: window);
      await pressCtrl(tester, LogicalKeyboardKey.digit2);
      await settle(tester);
      await pressFocusTerminal(tester);
      await settle(tester);
      expect(floating, findsOneWidget);
      expect(terminalHasKeyboard(), isTrue);

      await pressCtrl(tester, LogicalKeyboardKey.digit4);
      await settle(tester);
      expect(paneView, findsOneWidget);
      expect(terminalHasKeyboard(), isTrue, reason: 'typing goes on');

      // Weg von der Seite fragt nach einem anderen Abschnitt; die
      // Warteschlange nimmt die Tastatur wie immer.
      await pressCtrl(tester, LogicalKeyboardKey.digit1);
      await settle(tester);
      expect(floating, findsOneWidget);
      expect(terminalHasKeyboard(), isFalse);
      expect(
        FocusManager.instance.primaryFocus,
        isNot(isA<FocusScopeNode>()),
        reason: 'somebody who hears the shell keys holds the keyboard',
      );
    }, variant: linux);
  });

  group('leaving the page with the keyboard', () {
    testWidgets('the_pane_keeps_no_keyboard_after_leaving_and_ctrl_1_works', (
      WidgetTester tester,
    ) async {
      await pumpApp(tester, client: runningClient(), size: window);
      await pressCtrl(tester, LogicalKeyboardKey.digit4);
      await settle(tester);
      // Schweben gewählt, auf der Seite angedockt, und das Pane hat die
      // Tastatur.
      await pressFloat(tester);
      await pressFocusTerminal(tester);
      await settle(tester);
      expect(paneView, findsOneWidget);
      expect(terminalHasKeyboard(), isTrue);

      await pressCtrl(tester, LogicalKeyboardKey.digit2);
      await settle(tester);
      expect(headerTitle(tester), 'History');
      expect(floating, findsOneWidget, reason: 'it floats again');
      expect(
        terminalHasKeyboard(),
        isFalse,
        reason: 'the terminal does not take the keyboard off the page',
      );

      await pressCtrl(tester, LogicalKeyboardKey.digit1);
      await settle(tester);
      expect(headerTitle(tester), 'Intercept', reason: 'Ctrl+1 still works');
    }, variant: linux);
  });

  group('Go to terminal', () {
    testWidgets('the_empty_queue_floats_the_terminal_with_the_keyboard', (
      WidgetTester tester,
    ) async {
      await pumpApp(tester, client: runningClient(), size: window);
      await settle(tester);
      final Finder go = find.byKey(const Key('queue-go-to-terminal'));
      expect(go, findsOneWidget);

      await tester.tap(go);
      await settle(tester);
      expect(headerTitle(tester), 'Intercept', reason: 'the section stays');
      expect(floating, findsOneWidget);
      expect(placement(tester), TerminalPlacement.floating);
      expect(
        terminalHasKeyboard(),
        isTrue,
        reason: 'an explicit ask for the terminal hands it the keyboard',
      );
      expectOneView();
    }, variant: linux);

    testWidgets('a_floating_terminal_only_takes_the_keyboard', (
      WidgetTester tester,
    ) async {
      await pumpApp(tester, client: runningClient(), size: window);
      await settle(tester);
      await pressFloat(tester);
      expect(floating, findsOneWidget);
      expect(terminalHasKeyboard(), isFalse);

      await tester.tap(find.byKey(const Key('queue-go-to-terminal')));
      await settle(tester);
      expect(floating, findsOneWidget);
      expect(placement(tester), TerminalPlacement.floating);
      expect(terminalHasKeyboard(), isTrue);
    }, variant: linux);
  });

  group('the title strip', () {
    /// History mit offenem Einstellungsblatt in einem Fenster, dessen linker
    /// Rest zu schmal für das kleinste Fenster ist, aber breit genug für den
    /// Streifen.
    Future<void> leaveNoCornerFree(WidgetTester tester) async {
      await pumpApp(
        tester,
        client: runningClient(),
        size: const Size(870, 700),
      );
      await pressCtrl(tester, LogicalKeyboardKey.digit2);
      await pressFloat(tester);
      await pressCtrl(tester, LogicalKeyboardKey.comma);
      await settle(tester);
      await settle(tester);
      expect(find.byType(SettingsSheet), findsOneWidget);
      final Rect region = tester.getRect(find.byType(FloatingTerminalLayer));
      final double left = region.width - SettingsSheet.sheetWidth;
      expect(
        left,
        allOf(greaterThanOrEqualTo(240 + 24), lessThan(320 + 24)),
        reason: 'the setup of the test: room for the strip, not the window',
      );
    }

    testWidgets('no_free_corner_shrinks_the_window_to_a_strip', (
      WidgetTester tester,
    ) async {
      await leaveNoCornerFree(tester);
      expect(floating, findsNothing);
      expect(strip, findsOneWidget, reason: 'it says it is still there');
      expect(placement(tester), TerminalPlacement.floating);
      final Rect at = tester.getRect(strip);
      expect(at.size, const Size(240, 28), reason: 'HSize.terminalStrip');
      expect(
        at.overlaps(tester.getRect(find.byType(SettingsSheet))),
        isFalse,
        reason: 'never over the sheet',
      );
      expect(
        find.text('default · Agent'),
        findsOneWidget,
        reason: 'the profile of the sandbox and the active tab',
      );
    }, variant: linux);

    testWidgets('the_strip_opens_the_window_once_there_is_room', (
      WidgetTester tester,
    ) async {
      await leaveNoCornerFree(tester);
      expect(strip, findsOneWidget);

      // Das Blatt geht, die Ecke ist frei: der Streifen wird wieder Fenster.
      await pressCtrl(tester, LogicalKeyboardKey.comma);
      await settle(tester);
      await settle(tester);
      expect(strip, findsNothing);
      expect(floating, findsOneWidget);
    }, variant: linux);

    testWidgets('asking_for_the_terminal_with_only_the_strip_docks_it', (
      WidgetTester tester,
    ) async {
      // Zu niedrig für das kleinste Fenster (200 hoch und der Rand), hoch
      // genug für den Streifen; ohne Blatt, damit keine Ebene die Tastatur
      // hält.
      await pumpApp(
        tester,
        client: runningClient(),
        size: const Size(900, 280),
      );
      await pressCtrl(tester, LogicalKeyboardKey.digit3);
      await pressFloat(tester);
      expect(floating, findsNothing);
      expect(strip, findsOneWidget);

      await pressFocusTerminal(tester);
      await settle(tester);
      expect(strip, findsNothing);
      expect(placement(tester), TerminalPlacement.pane);
      expect(headerTitle(tester), 'Sandbox');
      expect(paneView, findsOneWidget);
      expect(
        terminalHasKeyboard(),
        isTrue,
        reason: 'the request reaches the pane',
      );
    }, variant: linux);

    testWidgets('a_click_on_the_strip_without_room_docks_it_on_the_page', (
      WidgetTester tester,
    ) async {
      await leaveNoCornerFree(tester);
      await tester.tap(find.byKey(const Key('shell-terminal-strip-open')));
      await settle(tester);
      expect(strip, findsNothing);
      expect(placement(tester), TerminalPlacement.pane);
      expect(headerTitle(tester), 'Sandbox');
      expect(paneView, findsOneWidget);
      expectOneView();
    }, variant: linux);
  });
}
