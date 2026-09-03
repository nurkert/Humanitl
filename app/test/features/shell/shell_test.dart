// Widget-Tests der Shell (HUM-019): Rail, Tastatur, Palette, Theme.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/shortcuts/intents.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/setup/setup_screen.dart';
import 'package:humanitl/features/shell/providers/navigation.dart';
import 'package:humanitl/features/shell/providers/theme.dart';
import 'package:humanitl/features/shell/section.dart';
import 'package:humanitl/features/shell/shell_screen.dart';
import 'package:humanitl/features/shell/widgets/command_palette.dart';
import 'package:humanitl/features/shell/widgets/icon_rail.dart';

import '../../harness/app_harness.dart';

String headerTitle(WidgetTester tester) =>
    tester.widget<Text>(find.byKey(const Key('header-section-title'))).data!;

void main() {
  testWidgets('shell_renders_rail_and_sections', (WidgetTester tester) async {
    await pumpApp(tester, client: FakeDaemonClient());

    expect(find.byType(ShellScreen), findsOneWidget);
    expect(find.byType(SetupScreen), findsNothing);
    expect(find.byType(RailEntry), findsNWidgets(5));
    expect(headerTitle(tester), 'Intercept');

    await pressCtrl(tester, LogicalKeyboardKey.digit2);
    expect(headerTitle(tester), 'History');

    await pressCtrl(tester, LogicalKeyboardKey.digit5);
    expect(headerTitle(tester), 'Audit');

    // Ctrl+9 hat keinen Abschnitt und tut nichts.
    await pressCtrl(tester, LogicalKeyboardKey.digit9);
    expect(headerTitle(tester), 'Audit');
    expect(tester.takeException(), isNull);
  });

  testWidgets('rail click selects the section and marks it active', (
    WidgetTester tester,
  ) async {
    await pumpApp(tester, client: FakeDaemonClient());

    await tester.tap(find.byType(RailEntry).at(Section.rules.index));
    await tester.pump();

    expect(headerTitle(tester), 'Rules');
    final RailEntry active = tester.widget(
      find.byType(RailEntry).at(Section.rules.index),
    );
    expect(active.active, isTrue);
    final RailEntry inactive = tester.widget(find.byType(RailEntry).first);
    expect(inactive.active, isFalse);
  });

  testWidgets('palette_opens_and_navigates', (WidgetTester tester) async {
    await pumpApp(tester, client: FakeDaemonClient());
    expect(find.byType(CommandPalette), findsNothing);

    await pressCtrl(tester, LogicalKeyboardKey.keyK);
    expect(find.byType(CommandPalette), findsOneWidget);

    await tester.enterText(find.byKey(const Key('palette-input')), 'hist');
    await tester.pump();
    expect(find.byKey(const Key('palette-go-history')), findsOneWidget);
    expect(find.byKey(const Key('palette-go-rules')), findsNothing);

    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();

    expect(find.byType(CommandPalette), findsNothing);
    expect(headerTitle(tester), 'History');

    // Nach dem Schließen hat die Shell den Fokus wieder: Ctrl+1 wirkt.
    await pressCtrl(tester, LogicalKeyboardKey.digit1);
    expect(headerTitle(tester), 'Intercept');
  });

  testWidgets('palette arrow keys move the selection and Escape closes', (
    WidgetTester tester,
  ) async {
    await pumpApp(tester, client: FakeDaemonClient());
    await pressCtrl(tester, LogicalKeyboardKey.keyK);

    // Ohne Filter: erste Zeile ist "Go to Intercept"; Pfeil runter wählt
    // History, Enter führt aus.
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(headerTitle(tester), 'History');

    await pressCtrl(tester, LogicalKeyboardKey.keyK);
    expect(find.byType(CommandPalette), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(find.byType(CommandPalette), findsNothing);

    // Ctrl+K schließt eine offene Palette wieder.
    await pressCtrl(tester, LogicalKeyboardKey.keyK);
    await pressCtrl(tester, LogicalKeyboardKey.keyK);
    expect(find.byType(CommandPalette), findsNothing);
  });

  testWidgets('the palette takes the focus and the text-field guard sees it', (
    WidgetTester tester,
  ) async {
    // Fallstricke: Einzeltasten-Shortcuts (HUM-020) dürfen nicht feuern,
    // solange jemand tippt. Der Wächter erkennt das Palettenfeld, und die
    // Palette holt sich den Fokus selbst, obwohl die Shell ihn hatte.
    await pumpApp(tester, client: FakeDaemonClient());
    expect(isTextInputFocused(), isFalse);

    await pressCtrl(tester, LogicalKeyboardKey.keyK);
    expect(isTextInputFocused(), isTrue);

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(find.byType(CommandPalette), findsNothing);
    expect(isTextInputFocused(), isFalse);
  });

  testWidgets('palette without a match says so', (WidgetTester tester) async {
    await pumpApp(tester, client: FakeDaemonClient());
    await pressCtrl(tester, LogicalKeyboardKey.keyK);
    await tester.enterText(find.byKey(const Key('palette-input')), 'zzz');
    await tester.pump();
    expect(find.text('No matching command'), findsOneWidget);
    // Enter ohne Treffer tut nichts und lässt die Palette offen.
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(find.byType(CommandPalette), findsOneWidget);
  });

  testWidgets('header button opens the palette', (WidgetTester tester) async {
    await pumpApp(tester, client: FakeDaemonClient());
    await tester.tap(find.byKey(const Key('header-palette-button')));
    await tester.pump();
    expect(find.byType(CommandPalette), findsOneWidget);
  });

  testWidgets('theme_toggle_changes_tokens', (WidgetTester tester) async {
    await pumpApp(tester, client: FakeDaemonClient());
    HTokens tokens() => HTheme.of(tester.element(find.byType(IconRail)));
    expect(tokens().brightness, Brightness.dark);
    expect(tokens().colors.bg0, HColors.bg0);

    await pressCtrl(tester, LogicalKeyboardKey.keyK);
    await tester.enterText(find.byKey(const Key('palette-input')), 'theme');
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();

    expect(tokens().brightness, Brightness.light);
    expect(tokens().colors.bg0, HColors.lBg0);

    final ProviderContainer container = ProviderScope.containerOf(
      tester.element(find.byType(ShellScreen)),
    );
    expect(container.read(themeModeProvider), HThemeMode.light);
    container.read(themeModeProvider.notifier).toggle(Brightness.light);
    await tester.pump();
    expect(tokens().brightness, Brightness.dark);
  });

  testWidgets('status bar shows daemon version and fake tag', (
    WidgetTester tester,
  ) async {
    await pumpApp(tester, client: FakeDaemonClient());
    expect(find.text('Daemon 0.0.0-fake'), findsOneWidget);
    expect(find.text('fake'), findsOneWidget);
    expect(find.text('Session 018f0001'), findsOneWidget);
  });

  test('navigation ignores indices outside the rail', () {
    final ProviderContainer container = ProviderContainer();
    addTearDown(container.dispose);
    final Navigation navigation = container.read(navigationProvider.notifier);
    navigation.goIndex(4);
    expect(container.read(navigationProvider), Section.audit);
    navigation.goIndex(5);
    expect(container.read(navigationProvider), Section.audit);
    navigation.goIndex(-1);
    expect(container.read(navigationProvider), Section.audit);
  });
}
