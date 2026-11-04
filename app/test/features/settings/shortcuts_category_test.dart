// Die Kategorie „Tastenkürzel“ der Einstellungen (HUM-420).
//
// Die Übersicht zeigt jeden Eintrag des Katalogs in seiner Gruppe, die
// Gruppen in der Reihenfolge der Bereiche, und die Suche findet jedes Kürzel
// an seinem Namen, auf Deutsch und Englisch. Auf macOS steht `⌘`, wo `Ctrl`
// dort auch als `Cmd` gilt.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/shortcuts/shortcut_catalog.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/settings/registry/settings_registry.dart';
import 'package:humanitl/features/settings/registry/settings_search.dart';
import 'package:humanitl/l10n/l10n.dart';

import '../../harness/settings_harness.dart';
import 'settings_overlay_test.dart' show open, page, query, type;

Finder row(AppShortcut shortcut) =>
    find.byKey(ValueKey<String>('shortcut-row-${shortcut.name}'));

void main() {
  final TargetPlatformVariant linux = TargetPlatformVariant.only(
    TargetPlatform.linux,
  );

  group('the register', () {
    test('every shortcut has an entry in its category, once', () {
      for (final AppShortcut shortcut in AppShortcut.values) {
        final SettingEntry entry = settingEntry(SettingIds.shortcut(shortcut));
        expect(entry.category, SettingsCategory.shortcuts);
      }
      for (final ShortcutScope scope in ShortcutScope.values) {
        expect(
          settingEntry(SettingIds.shortcutGroup(scope)).build,
          isNotNull,
          reason: 'the group of $scope draws its rows',
        );
      }
    });

    test('search_finds_shortcut_by_label in English and German', () {
      for (final Locale locale in const <Locale>[Locale('en'), Locale('de')]) {
        final AppLocalizations l10n = lookupAppLocalizations(locale);
        for (final AppShortcut shortcut in AppShortcut.values) {
          if (!shortcut.appliesOn()) {
            continue;
          }
          final List<String> hits = <String>[
            for (final SettingsSearchGroup group in searchSettings(
              shortcut.label(l10n),
              l10n,
              FakeDaemonClient.defaultInfo,
            ))
              for (final SettingEntry entry in group.entries) entry.id,
          ];
          expect(
            hits,
            contains(SettingIds.shortcut(shortcut)),
            reason:
                '"${shortcut.label(l10n)}" finds $shortcut in '
                '${locale.languageCode}',
          );
        }
      }
    });

    test('the keys and the word for shortcuts find them too', () {
      final AppLocalizations de = lookupAppLocalizations(const Locale('de'));
      List<String> ids(String query) => <String>[
        for (final SettingsSearchGroup group in searchSettings(
          query,
          de,
          FakeDaemonClient.defaultInfo,
        ))
          for (final SettingEntry entry in group.entries) entry.id,
      ];
      expect(ids('strg+k'), contains(SettingIds.shortcut(AppShortcut.palette)));
      expect(
        ids('tastenkürzel'),
        contains(SettingIds.shortcutGroup(ShortcutScope.global)),
      );
    });
  });

  testWidgets(
    'overview_lists_every_entry, grouped by scope in catalogue order',
    (WidgetTester tester) async {
      await open(tester);
      await showSettingsCategory(tester, SettingsCategory.shortcuts);
      expect(page(SettingsCategory.shortcuts), findsOneWidget);
      double last = double.negativeInfinity;
      for (final ShortcutScope scope in ShortcutScope.values) {
        final Finder group = find.byKey(
          ValueKey<String>('shortcut-group-${scope.name}'),
        );
        expect(group, findsOneWidget, reason: '$scope has a group');
        for (final AppShortcut shortcut in AppShortcut.of(scope)) {
          expect(
            find.descendant(of: group, matching: row(shortcut)),
            findsOneWidget,
            reason: '$shortcut stands in the group of $scope',
          );
          final double top = tester.getTopLeft(row(shortcut)).dy;
          expect(top, greaterThan(last), reason: '$shortcut in order');
          last = top;
        }
      }
      // Nur lesen: keine Schaltfläche, kein Feld auf der Seite.
      expect(
        find.descendant(
          of: page(SettingsCategory.shortcuts),
          matching: find.byType(HButton),
        ),
        findsNothing,
      );
      expect(
        find.descendant(
          of: row(AppShortcut.toggleRail),
          matching: find.text('Shift'),
        ),
        findsOneWidget,
      );
      expect(tester.takeException(), isNull);
    },
    variant: linux,
  );

  testWidgets('a hit leads to the row of the shortcut and lights it (de)', (
    WidgetTester tester,
  ) async {
    await open(tester, german: true);
    final AppLocalizations de = lookupAppLocalizations(const Locale('de'));
    await type(tester, AppShortcut.ruleDown.label(de));
    final Finder hit = find.byKey(
      ValueKey<String>(
        'settings-hit-${SettingIds.shortcut(AppShortcut.ruleDown)}',
      ),
    );
    expect(hit, findsOneWidget);
    // Unter dem Namen stehen Bereich und Tasten.
    expect(
      find.descendant(of: hit, matching: find.text('Regeln · Alt+↓')),
      findsOneWidget,
    );
    await tester.tap(hit);
    await tester.pump();
    await tester.pump();
    final AnimatedContainer lit = tester.widget<AnimatedContainer>(
      find
          .ancestor(
            of: row(AppShortcut.ruleDown),
            matching: find.byType(AnimatedContainer),
          )
          .first,
    );
    expect(
      (lit.decoration! as BoxDecoration).color!.a,
      greaterThan(0),
      reason: 'lit after the jump',
    );
    await tester.pumpAndSettle(const Duration(seconds: 2));
    expect(query(tester), isEmpty);
    expect(page(SettingsCategory.shortcuts), findsOneWidget);
    final Rect view = tester.getRect(
      find.byKey(const ValueKey<String>('settings-scroll-shortcuts')),
    );
    final Rect at = tester.getRect(row(AppShortcut.ruleDown));
    expect(at.top >= view.top && at.bottom <= view.bottom, isTrue);
    expect(find.text('Strg'), findsWidgets, reason: 'German keycaps');
  }, variant: linux);

  testWidgets('macOS shows ⌘ where Ctrl answers to Cmd as well', (
    WidgetTester tester,
  ) async {
    await open(tester);
    await showSettingsCategory(tester, SettingsCategory.shortcuts);
    expect(
      find.descendant(of: row(AppShortcut.palette), matching: find.text('⌘')),
      findsOneWidget,
    );
    expect(
      find.descendant(
        of: row(AppShortcut.settingsSearch),
        matching: find.text('⌃'),
      ),
      findsOneWidget,
    );
    expect(row(AppShortcut.terminalCopySelection), findsNothing);
    expect(find.text('Ctrl'), findsNothing);
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));
}
