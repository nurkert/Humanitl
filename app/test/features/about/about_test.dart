// Der Dialog „Über Humanitl“ (HUM-031): Die Palette öffnet ihn, er nennt die
// mitgelieferte Rangliste mit ihrer Lizenz, und sein Knopf führt auf Flutters
// Lizenzseite.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/about/about_dialog.dart';
import 'package:humanitl/features/shell/widgets/command_palette.dart';
import 'package:humanitl/l10n/l10n.dart';

import '../../harness/app_harness.dart';

/// Öffnet die Palette, sucht den Eintrag unter seiner Beschriftung in
/// [l10n] und wählt ihn.
Future<void> openAbout(WidgetTester tester, AppLocalizations l10n) async {
  await pressCtrl(tester, LogicalKeyboardKey.keyK);
  await tester.enterText(
    find.byKey(const Key('palette-input')),
    l10n.shellPaletteAbout,
  );
  await tester.pump();
  await tester.tap(find.byKey(const Key('palette-about')));
  await tester.pump();
  // Der Dialog blendet ein; danach steht er.
  await tester.pump(const Duration(milliseconds: 300));
}

/// [matching] innerhalb des About-Dialogs.
Finder inAbout(Finder matching) =>
    find.descendant(of: find.byType(AboutDialog), matching: matching);

void main() {
  testWidgets('the palette opens About, and About names the ranking', (
    WidgetTester tester,
  ) async {
    final AppLocalizations l10n = lookupAppLocalizations(const Locale('en'));
    await pumpApp(tester, client: FakeDaemonClient());

    await openAbout(tester, l10n);

    expect(find.byType(CommandPalette), findsNothing);
    expect(find.byType(AboutDialog), findsOneWidget);
    expect(inAbout(find.text(l10n.appTitle)), findsOneWidget);
    expect(appVersion, isNotEmpty);
    expect(inAbout(find.text(appVersion)), findsOneWidget);
    expect(inAbout(find.text(l10n.aboutLegalese)), findsOneWidget);
    expect(inAbout(find.text(l10n.aboutRanks)), findsOneWidget);
    // Die Namensnennung nach CC BY 3.0: Quelle, Lizenz mit Adresse, und dass
    // es ein Ausschnitt ist. Hier gegen die Quelle der Übersetzung geprüft,
    // damit ein gekürzter Satz nicht still durchgeht.
    for (final String part in <String>[
      'Majestic Million',
      'Majestic-12 Ltd.',
      'https://majestic.com/reports/majestic-million',
      'CC BY 3.0',
      'https://creativecommons.org/licenses/by/3.0/',
      'excerpt',
    ]) {
      expect(l10n.aboutRanks, contains(part));
    }
  });

  testWidgets('About speaks German when the desktop does', (
    WidgetTester tester,
  ) async {
    tester.platformDispatcher.localesTestValue = const <Locale>[Locale('de')];
    addTearDown(tester.platformDispatcher.clearLocalesTestValue);
    final AppLocalizations de = lookupAppLocalizations(const Locale('de'));
    final AppLocalizations en = lookupAppLocalizations(const Locale('en'));
    await pumpApp(tester, client: FakeDaemonClient());

    await openAbout(tester, de);

    expect(de.aboutRanks, isNot(en.aboutRanks));
    expect(inAbout(find.text(de.aboutRanks)), findsOneWidget);
    expect(inAbout(find.text(de.aboutLegalese)), findsOneWidget);
    for (final String part in <String>[
      'Majestic Million',
      'https://majestic.com/reports/majestic-million',
      'CC BY 3.0',
      'https://creativecommons.org/licenses/by/3.0/',
      'Ausschnitt',
    ]) {
      expect(de.aboutRanks, contains(part));
    }
  });

  testWidgets('About leads to the licence page', (WidgetTester tester) async {
    final AppLocalizations l10n = lookupAppLocalizations(const Locale('en'));
    await pumpApp(tester, client: FakeDaemonClient());
    await openAbout(tester, l10n);

    final BuildContext dialog = tester.element(find.byType(AboutDialog));
    await tester.tap(
      inAbout(
        find.text(MaterialLocalizations.of(dialog).viewLicensesButtonLabel),
      ),
    );
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 500));

    expect(find.byType(LicensePage), findsOneWidget);
  });

  testWidgets('Escape closes About and the shell keys work again', (
    WidgetTester tester,
  ) async {
    final AppLocalizations l10n = lookupAppLocalizations(const Locale('en'));
    await pumpApp(tester, client: FakeDaemonClient());
    await openAbout(tester, l10n);

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 300));
    expect(find.byType(AboutDialog), findsNothing);

    await pressCtrl(tester, LogicalKeyboardKey.keyK);
    expect(find.byType(CommandPalette), findsOneWidget);
  });
}
