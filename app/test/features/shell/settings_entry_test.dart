// Die Einstellungen haben einen sichtbaren Eingang (HUM-275): ein Zahnrad in
// der Kopfleiste, in jedem Abschnitt und bei ein- wie ausgeklappter
// Seitenleiste (HUM-254) mit einem Klick erreichbar, ohne `Ctrl+,` zu kennen.
// Es öffnet dasselbe Blatt wie die Taste.
//
// Rot, sobald der Knopf fehlt, verdeckt ist oder etwas anderes öffnet.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/shortcuts/intents.dart';
import 'package:humanitl/features/shell/providers/language.dart';
import 'package:humanitl/features/shell/section.dart';
import 'package:humanitl/l10n/l10n.dart';

import '../../harness/app_harness.dart';

const Key gear = Key('header-settings-button');
const Key sheet = Key('settings-sheet');

/// Deutsch, unabhängig von Konfiguration und Rechner.
class German extends LanguageSetting {
  @override
  AppLanguage? build() => AppLanguage.de;
}

/// Klickt das Zahnrad und verlangt das Blatt; schließt es danach mit der
/// Taste, damit der nächste Schritt wieder bei geschlossenem Blatt beginnt.
Future<void> openWithGear(WidgetTester tester, String at) async {
  expect(find.byKey(sheet), findsNothing, reason: at);
  expect(find.byKey(gear).hitTestable(), findsOneWidget, reason: at);
  await tester.tap(find.byKey(gear));
  await tester.pumpAndSettle();
  expect(find.byKey(sheet), findsOneWidget, reason: at);
  await pressCtrl(tester, LogicalKeyboardKey.comma);
  await tester.pumpAndSettle();
  expect(find.byKey(sheet), findsNothing, reason: at);
}

void main() {
  testWidgets('settings_entry: the gear opens the sheet from every section, '
      'with the rail collapsed and expanded', (WidgetTester tester) async {
    await pumpApp(tester, client: FakeDaemonClient.empty());

    for (final bool expanded in <bool>[false, true]) {
      if (expanded) {
        await pressCtrl(tester, LogicalKeyboardKey.digit0);
        await tester.pumpAndSettle();
      }
      for (final Section section in Section.values) {
        await pressCtrl(tester, navigationKeys[section.index]);
        await tester.pumpAndSettle();
        await openWithGear(
          tester,
          '${section.name}, rail ${expanded ? 'expanded' : 'collapsed'}',
        );
      }
    }
    expect(tester.takeException(), isNull);
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('the gear names the sheet and its shortcut', (
    WidgetTester tester,
  ) async {
    await pumpApp(tester, client: FakeDaemonClient.empty());
    expect(find.bySemanticsLabel('Settings (Ctrl+,)'), findsOneWidget);
  });

  testWidgets('in German the shortcut says Strg', (WidgetTester tester) async {
    await pumpApp(
      tester,
      client: FakeDaemonClient.empty(),
      overrides: <Override>[languageProvider.overrideWith(German.new)],
    );
    expect(
      lookupAppLocalizations(const Locale('de')).shellSettingsButton,
      'Einstellungen (Strg+,)',
    );
    expect(find.bySemanticsLabel('Einstellungen (Strg+,)'), findsOneWidget);
    await openWithGear(tester, 'German');
  });
}
