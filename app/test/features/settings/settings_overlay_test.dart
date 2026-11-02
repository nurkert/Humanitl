// Die Einstellungen als Overlay von unten mit Kategorien und Suche (HUM-404).
//
// Geprüft wird an der ganzen App: Das Register kennt jede Einstellung des
// alten Blatts und die Suche findet sie auf Deutsch und Englisch; ein Treffer
// führt zur Zeile; die Tastatur geht Suche, Kategorien, Inhalt, die Pfeile
// wählen die Kategorie, `Esc` leert erst die Suche und schließt dann; das
// Zahnrad bekommt die Tastatur zurück; nichts läuft über, im Fenster von
// 960 x 600 und bei jeder Stufe der Skalierung.

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/connection.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui_scale.dart';
import 'package:humanitl/features/settings/registry/settings_registry.dart';
import 'package:humanitl/features/settings/registry/settings_search.dart';
import 'package:humanitl/features/settings/settings_overlay.dart';
import 'package:humanitl/features/shell/providers/language.dart';
import 'package:humanitl/l10n/l10n.dart';

import '../../harness/app_harness.dart';
import '../../harness/settings_harness.dart';
import '../../goldens/support/golden_variants.dart';
import '../shell/ui_scale_test.dart' show freshStatePath, scaleState;
import 'change_setting_fix_test.dart' show focusIsWithin;

/// Jede Zeile, die das Blatt vor HUM-404 zeigte, in seiner Reihenfolge:
/// die Schlüssel von `FormKeys.shownInSheet`, die Skalierung und
/// `config.toml`. Eingefroren, damit ein Eintrag, der aus dem Register
/// fällt, hier auffällt und nicht still mit ihm verschwindet.
const List<String> oldSheetIds = <String>[
  FormKeys.llmEndpoint,
  FormKeys.llmModels,
  FormKeys.llmApiKeyRef,
  FormKeys.agentAdapter,
  FormKeys.holdTimeoutSecs,
  FormKeys.holdAskMode,
  FormKeys.uiLanguage,
  FormKeys.uiTheme,
  SettingIds.uiScale,
  FormKeys.uiNotifications,
  FormKeys.sandboxWorkDir,
  FormKeys.sandboxMode,
  FormKeys.tlsTrustSystemStore,
  FormKeys.updateCheck,
  SettingIds.configFile,
];

/// Deutsch, unabhängig von Konfiguration und Rechner.
class German extends LanguageSetting {
  @override
  AppLanguage? build() => AppLanguage.de;
}

/// Ein Daemon, der jede Zeile anbietet, auch `sandbox.mode`.
FakeDaemonClient everything() =>
    FakeDaemonClient(info: FakeDaemonClient.disposableInfo)
      ..configValues = const ConfigValues(
        values: <String, Object?>{
          'hold': <String, Object?>{'timeout_secs': 300, 'ask_mode': 'ui'},
          'ui': <String, Object?>{
            'language': 'en',
            'theme': 'dark',
            'notifications': true,
          },
          'sandbox': <String, Object?>{'work_dir': '/home/u/src/acme'},
          'llm': <String, Object?>{
            'endpoint': 'http://192.168.1.20:11434/',
            'models': <Object?>['qwen2.5-coder:14b'],
          },
        },
        configPath: '/home/u/.config/humanitl/config.toml',
      );

/// Die App mit offenem Overlay.
Future<void> open(
  WidgetTester tester, {
  FakeDaemonClient? client,
  bool german = false,
  Size size = const Size(1280, 800),
  Override? uiState,
}) async {
  await pumpApp(
    tester,
    client: client ?? everything(),
    size: size,
    uiState: uiState,
    overrides: <Override>[
      if (german) languageProvider.overrideWith(German.new),
    ],
  );
  await pressCtrl(tester, LogicalKeyboardKey.comma);
  await tester.pumpAndSettle();
}

/// Was im Suchfeld steht.
String query(WidgetTester tester) => tester
    .widget<EditableText>(
      find.descendant(
        of: find.byKey(const Key('settings-search')),
        matching: find.byType(EditableText),
      ),
    )
    .controller
    .text;

Future<void> type(WidgetTester tester, String text) async {
  await tester.enterText(
    find.descendant(
      of: find.byKey(const Key('settings-search')),
      matching: find.byType(EditableText),
    ),
    text,
  );
  await tester.pumpAndSettle();
}

Future<void> press(WidgetTester tester, LogicalKeyboardKey key) async {
  await tester.sendKeyEvent(key);
  await tester.pumpAndSettle();
}

/// Ob die Kategorie [category] als Seite steht.
Finder page(SettingsCategory category) =>
    find.byKey(ValueKey<String>('settings-page-${category.name}'));

void main() {
  final TargetPlatformVariant linux = TargetPlatformVariant.only(
    TargetPlatform.linux,
  );

  group('the register', () {
    test(
      'registry_covers_old_sheet: every row of the old sheet has an entry',
      () {
        final Set<String> ids = <String>{
          for (final SettingEntry entry in settingsRegistry) entry.id,
        };
        expect(
          ids,
          hasLength(settingsRegistry.length),
          reason: 'ids are unique',
        );
        for (final String id in oldSheetIds) {
          expect(ids, contains(id), reason: '$id has an entry');
        }
        for (final String key in FormKeys.shownInSheet) {
          expect(ids, contains(key), reason: 'a fix on $key has a row');
        }
      },
    );

    test('registry_search_finds_every_entry in English and German', () {
      for (final Locale locale in const <Locale>[Locale('en'), Locale('de')]) {
        final AppLocalizations l10n = lookupAppLocalizations(locale);
        for (final String id in oldSheetIds) {
          final SettingEntry entry = settingEntry(id);
          final List<String> keywords = entry
              .keywords(l10n)
              .split(',')
              .map((String word) => word.trim())
              .toList();
          for (final String term in <String>[
            entry.title(l10n),
            keywords.first,
          ]) {
            final List<SettingEntry> hits = <SettingEntry>[
              for (final SettingsSearchGroup group in searchSettings(
                term,
                l10n,
                FakeDaemonClient.disposableInfo,
              ))
                ...group.entries,
            ];
            expect(
              hits,
              contains(entry),
              reason: '"$term" finds $id in ${locale.languageCode}',
            );
          }
        }
      }
    });

    test('search folds case and umlauts and needs every word', () {
      final AppLocalizations de = lookupAppLocalizations(const Locale('de'));
      List<String> ids(String query) => <String>[
        for (final SettingsSearchGroup group in searchSettings(
          query,
          de,
          FakeDaemonClient.disposableInfo,
        ))
          for (final SettingEntry entry in group.entries) entry.id,
      ];
      expect(ids('SCHLUSSEL'), contains(FormKeys.llmApiKeyRef));
      expect(ids('zertifikat system'), <String>[FormKeys.tlsTrustSystemStore]);
      expect(ids('zertifikat banane'), isEmpty);
      expect(ids('   '), isEmpty);
    });

    test('a setting the daemon does not offer is not found', () {
      final AppLocalizations en = lookupAppLocalizations(const Locale('en'));
      final Iterable<String> found = <String>[
        for (final SettingsSearchGroup group in searchSettings(
          'disposable',
          en,
          FakeDaemonClient.defaultInfo,
        ))
          for (final SettingEntry entry in group.entries) entry.id,
      ];
      expect(found, isNot(contains(FormKeys.sandboxMode)));
    });

    test(
      'the next-session note stands only where a session keeps the value',
      () {
        final Set<String> next = <String>{
          for (final SettingEntry entry in settingsRegistry)
            if (entry.nextSession) entry.id,
        };
        expect(next, <String>{
          FormKeys.llmEndpoint,
          FormKeys.llmModels,
          FormKeys.llmApiKeyRef,
          FormKeys.agentAdapter,
          FormKeys.sandboxWorkDir,
          FormKeys.sandboxMode,
        });
      },
    );
  });

  // Durch die App: jede Einstellung über die Suche, in beiden Sprachen, und
  // der Treffer führt zu ihrer Zeile.
  for (final bool german in <bool>[false, true]) {
    testWidgets(
      'every_setting_is_found_and_reached (${german ? 'de' : 'en'})',
      (WidgetTester tester) async {
        await open(tester, german: german);
        final AppLocalizations l10n = lookupAppLocalizations(
          Locale(german ? 'de' : 'en'),
        );
        for (final String id in oldSheetIds) {
          final SettingEntry entry = settingEntry(id);
          await type(tester, entry.title(l10n));
          final Finder hit = find.byKey(ValueKey<String>('settings-hit-$id'));
          expect(hit, findsOneWidget, reason: '$id is a hit');
          await tester.tap(hit);
          await tester.pumpAndSettle();
          expect(query(tester), isEmpty, reason: 'choosing empties the search');
          expect(
            page(entry.category),
            findsOneWidget,
            reason: '$id opens its page',
          );
          final Finder row = find.byKey(ValueKey<String>('setting-row-$id'));
          // Die Zeile steht im sichtbaren Teil der Seite, ganz oben oder
          // darunter, nicht unter dem Rand.
          final Rect view = tester.getRect(
            find.byKey(
              ValueKey<String>('settings-scroll-${entry.category.name}'),
            ),
          );
          final Rect at = tester.getRect(row);
          expect(
            at.top >= view.top && at.top < view.bottom,
            isTrue,
            reason: '$id in view ($at in $view)',
          );
          await tester.pumpAndSettle();
        }
        expect(tester.takeException(), isNull);
      },
      variant: linux,
    );
  }

  testWidgets(
    'hits are grouped by category, and none says so with a way back',
    (WidgetTester tester) async {
      await open(tester);
      // „model" steht in den Zeilen der Modelle und in der Erklärung des
      // Agenten: zwei Gruppen.
      await type(tester, 'model');
      final List<String> headers = <String>[
        for (final SettingsSearchGroup group in searchSettings(
          'model',
          lookupAppLocalizations(const Locale('en')),
          FakeDaemonClient.disposableInfo,
        ))
          group.category.name,
      ];
      expect(
        headers.length,
        greaterThan(1),
        reason: 'the test needs two groups',
      );
      expect(find.byKey(const Key('settings-search-results')), findsOneWidget);
      final double first = tester.getTopLeft(find.text('Models').last).dy;
      final double second = tester.getTopLeft(find.text('Agent').last).dy;
      expect(first, lessThan(second), reason: 'in the order of the column');

      await type(tester, 'zzqx');
      expect(find.byKey(const Key('settings-search-empty')), findsOneWidget);
      expect(find.text('No setting matches “zzqx”.'), findsOneWidget);
      await tester.tap(find.byKey(const Key('settings-search-clear')));
      await tester.pumpAndSettle();
      expect(query(tester), isEmpty);
      expect(page(SettingsCategory.models), findsOneWidget);
      expect(focusIsWithin(const Key('settings-search')), isTrue);
    },
    variant: linux,
  );

  testWidgets('keyboard: search, categories, content; arrows choose; Esc '
      'clears the search before it closes', (WidgetTester tester) async {
    await open(tester);
    expect(
      focusIsWithin(const Key('settings-search')),
      isTrue,
      reason: 'the search has the cursor on opening',
    );

    await press(tester, LogicalKeyboardKey.tab);
    expect(
      focusIsWithin(const ValueKey<String>('settings-nav-models')),
      isTrue,
      reason: 'Tab goes on to the chosen category',
    );
    await press(tester, LogicalKeyboardKey.arrowDown);
    expect(page(SettingsCategory.agent), findsOneWidget);
    expect(focusIsWithin(const ValueKey<String>('settings-nav-agent')), isTrue);
    await press(tester, LogicalKeyboardKey.end);
    expect(page(SettingsCategory.daemon), findsOneWidget);
    await press(tester, LogicalKeyboardKey.arrowUp);
    expect(page(SettingsCategory.network), findsOneWidget);

    await press(tester, LogicalKeyboardKey.tab);
    expect(
      focusIsWithin(
        const ValueKey<String>('setting-info-${FormKeys.tlsTrustSystemStore}'),
      ),
      isTrue,
      reason: 'Tab from the column goes into the page, not to the next entry',
    );

    // Tippen außerhalb eines Feldes beginnt die Suche.
    await press(tester, LogicalKeyboardKey.keyC);
    expect(query(tester), 'c');
    expect(focusIsWithin(const Key('settings-search')), isTrue);

    await press(tester, LogicalKeyboardKey.escape);
    expect(query(tester), isEmpty, reason: 'the first Esc empties the search');
    expect(find.byType(SettingsOverlay), findsOneWidget);
    await press(tester, LogicalKeyboardKey.escape);
    expect(
      find.byType(SettingsOverlay),
      findsNothing,
      reason: 'then it closes',
    );
  }, variant: linux);

  testWidgets('Ctrl+F puts the cursor into the search from anywhere inside', (
    WidgetTester tester,
  ) async {
    await open(tester);
    await showSettingsCategory(tester, SettingsCategory.interface);
    expect(focusIsWithin(const Key('settings-search')), isFalse);
    await pressCtrl(tester, LogicalKeyboardKey.keyF);
    await tester.pumpAndSettle();
    expect(focusIsWithin(const Key('settings-search')), isTrue);
  }, variant: linux);

  testWidgets('the gear opens it, a click on the dim and the close button '
      'close it, and the gear gets the keyboard back', (
    WidgetTester tester,
  ) async {
    await pumpApp(tester, client: everything());
    final Finder gear = find.byKey(const Key('header-settings-button'));
    await tester.tap(gear);
    await tester.pumpAndSettle();
    expect(find.byType(SettingsOverlay), findsOneWidget);
    final Rect panel = tester.getRect(
      find.byKey(const Key('settings-overlay')),
    );
    // Über der Tafel, in der abgedunkelten Fläche.
    await tester.tapAt(Offset(panel.center.dx, panel.top / 2));
    await tester.pumpAndSettle();
    expect(find.byType(SettingsOverlay), findsNothing);
    expect(focusIsWithin(const Key('header-settings-button')), isTrue);

    await tester.tap(gear);
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('settings-close')));
    await tester.pumpAndSettle();
    expect(find.byType(SettingsOverlay), findsNothing);
    expect(focusIsWithin(const Key('header-settings-button')), isTrue);
  }, variant: linux);

  testWidgets('a switch says Saved at its row for a moment', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = everything();
    await open(tester, client: client);
    await showSettingsCategory(tester, SettingsCategory.interface);
    final Finder saved = find.byKey(
      const ValueKey<String>('setting-saved-ui.theme'),
    );
    expect(saved, findsNothing);
    await tapSegment(tester, const Key('settings-theme'), 'Light');
    expect(client.configWrites.last, ('ui.theme', 'light'));
    expect(saved, findsOneWidget);
    await tester.pump(const Duration(seconds: 3));
    await tester.pumpAndSettle();
    expect(saved, findsNothing, reason: 'it fades after a moment');
  }, variant: linux);

  testWidgets('the longer explanation stands behind the info button, and the '
      'next-session note only where it is true', (WidgetTester tester) async {
    await open(tester);
    await showSettingsCategory(tester, SettingsCategory.network);
    const Key details = ValueKey<String>(
      'setting-details-${FormKeys.tlsTrustSystemStore}',
    );
    expect(find.byKey(details), findsNothing);
    await tester.tap(
      find.byKey(
        const ValueKey<String>('setting-info-${FormKeys.tlsTrustSystemStore}'),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.byKey(details), findsOneWidget);
    expect(
      find.byKey(
        const ValueKey<String>('setting-next-${FormKeys.tlsTrustSystemStore}'),
      ),
      findsNothing,
    );

    await showSettingsCategory(tester, SettingsCategory.agent);
    expect(
      find.byKey(
        const ValueKey<String>('setting-next-${FormKeys.agentAdapter}'),
      ),
      findsOneWidget,
    );
  }, variant: linux);

  testWidgets('a fix opens the category of its key and lights the row', (
    WidgetTester tester,
  ) async {
    await open(tester);
    final SettingsOverlay overlay = tester.widget<SettingsOverlay>(
      find.byType(SettingsOverlay),
    );
    expect(overlay.focusKey, isNull);
    // Der Weg eines `ChangeSetting` (HUM-275) ist in
    // `change_setting_fix_test.dart` geprüft; hier das Licht an der Zeile.
    await type(tester, 'certificates');
    await tester.tap(
      find.byKey(
        const ValueKey<String>('settings-hit-${FormKeys.tlsTrustSystemStore}'),
      ),
    );
    await tester.pump();
    await tester.pump();
    final AnimatedContainer row = tester.widget<AnimatedContainer>(
      find
          .ancestor(
            of: find.byKey(
              const ValueKey<String>(
                'setting-row-${FormKeys.tlsTrustSystemStore}',
              ),
            ),
            matching: find.byType(AnimatedContainer),
          )
          .first,
    );
    expect(
      (row.decoration! as BoxDecoration).color!.a,
      greaterThan(0),
      reason: 'lit after the jump',
    );
    expect(focusIsWithin(const Key('settings-trust-system-store')), isTrue);
    await tester.pumpAndSettle(const Duration(seconds: 2));
  }, variant: linux);

  // Kein Überlauf: im kleinsten Fenster, das die Aufgabe nennt, und bei jeder
  // Stufe in einem Fenster, das jede Stufe trägt (2200 x 1400 ist bei 200 %
  // logisch 1100 x 700, die Mindestgröße der Shell).
  for (final (Size window, List<int> percents) in <(Size, List<int>)>[
    (const Size(960, 600), <int>[90, 100]),
    (const Size(2200, 1400), uiScalePresets),
  ]) {
    for (final int percent in percents) {
      testWidgets('no_overflow at $percent % in ${window.width.round()}x'
          '${window.height.round()}, German', (WidgetTester tester) async {
        await open(
          tester,
          german: true,
          size: window,
          uiState: scaleState(freshStatePath(), percent: percent),
        );
        for (final SettingsCategory category in SettingsCategory.values) {
          await showSettingsCategory(tester, category);
          expect(
            tester.takeException(),
            isNull,
            reason: '${category.name} at $percent %',
          );
        }
        await type(tester, 'a');
        expect(tester.takeException(), isNull, reason: 'search at $percent %');
        await type(tester, 'zzqx');
        expect(tester.takeException(), isNull, reason: 'empty at $percent %');
      }, variant: linux);
    }
  }

  // Ein Sprung, bevor der Daemon geantwortet hat (Review HUM-404): Die Zeile
  // kommt später, und der Sprung wartet auf sie, statt nach zwei Bildern
  // aufzugeben.
  group('a jump before the daemon answered', () {
    /// Das Overlay allein, mit Sprungziel [key].
    Widget alone(FakeDaemonClient client, String key, {Override? info}) =>
        goldenFrame(
          variant: GoldenVariant.darkEn,
          background: GoldenVariant.darkEn.tokens.colors.bg0,
          overrides: <Override>[
            daemonClientProvider.overrideWithValue(client),
            ?info,
          ],
          child: SettingsOverlay(onClose: () {}, focusKey: key),
        );

    Future<void> frames(WidgetTester tester, int count) async {
      for (int i = 0; i < count; i++) {
        await tester.pump(const Duration(milliseconds: 50));
      }
    }

    bool lit(WidgetTester tester, String id) {
      final AnimatedContainer row = tester.widget<AnimatedContainer>(
        find
            .ancestor(
              of: find.byKey(ValueKey<String>('setting-row-$id')),
              matching: find.byType(AnimatedContainer),
            )
            .first,
      );
      return (row.decoration! as BoxDecoration).color!.a > 0;
    }

    testWidgets('waits for GetConfig, then scrolls, focuses and lights', (
      WidgetTester tester,
    ) async {
      final GatedConfigClient client = GatedConfigClient();
      await tester.binding.setSurfaceSize(const Size(1100, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(alone(client, FormKeys.tlsTrustSystemStore));
      await frames(tester, 10);
      expect(find.byKey(const Key('settings-loading')), findsOneWidget);

      client.release();
      await frames(tester, 4);
      expect(page(SettingsCategory.network), findsOneWidget);
      expect(focusIsWithin(const Key('settings-trust-system-store')), isTrue);
      expect(lit(tester, FormKeys.tlsTrustSystemStore), isTrue);
      await tester.pumpAndSettle(const Duration(seconds: 2));
    }, variant: linux);

    testWidgets('waits for GetInfo when the row depends on it', (
      WidgetTester tester,
    ) async {
      final Completer<DaemonInfo> info = Completer<DaemonInfo>();
      await tester.binding.setSurfaceSize(const Size(1100, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(
        alone(
          everything(),
          FormKeys.sandboxMode,
          info: daemonInfoProvider.overrideWith((Ref ref) => info.future),
        ),
      );
      await frames(tester, 10);
      expect(find.byKey(const Key('settings-sandbox-mode')), findsNothing);

      info.complete(FakeDaemonClient.disposableInfo);
      await frames(tester, 4);
      expect(focusIsWithin(const Key('settings-sandbox-mode')), isTrue);
      expect(lit(tester, FormKeys.sandboxMode), isTrue);
      await tester.pumpAndSettle(const Duration(seconds: 2));
    }, variant: linux);

    testWidgets('a search in between drops the waiting target', (
      WidgetTester tester,
    ) async {
      final Completer<DaemonInfo> info = Completer<DaemonInfo>();
      await tester.binding.setSurfaceSize(const Size(1100, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(
        alone(
          everything(),
          FormKeys.sandboxMode,
          info: daemonInfoProvider.overrideWith((Ref ref) => info.future),
        ),
      );
      await frames(tester, 10);
      await tester.enterText(find.byKey(const Key('settings-search')), 'x');
      await tester.enterText(find.byKey(const Key('settings-search')), '');
      await frames(tester, 2);

      info.complete(FakeDaemonClient.disposableInfo);
      await frames(tester, 4);
      expect(focusIsWithin(const Key('settings-sandbox-mode')), isFalse);
      expect(focusIsWithin(const Key('settings-search')), isTrue);
      await tester.pumpAndSettle(const Duration(seconds: 2));
    }, variant: linux);

    testWidgets('drops a target the daemon does not offer', (
      WidgetTester tester,
    ) async {
      await tester.binding.setSurfaceSize(const Size(1100, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(
        alone(FakeDaemonClient.empty(), FormKeys.sandboxMode),
      );
      await tester.pumpAndSettle();
      expect(page(SettingsCategory.sandbox), findsOneWidget);
      expect(find.byKey(const Key('settings-sandbox-mode')), findsNothing);
      expect(tester.takeException(), isNull);
    }, variant: linux);
  });

  testWidgets('Ctrl+, over the palette or the pause question opens nothing', (
    WidgetTester tester,
  ) async {
    await pumpApp(
      tester,
      client: FakeDaemonClient(info: FakeIntercept.interceptInfo),
    );
    await pressCtrl(tester, LogicalKeyboardKey.keyK);
    await tester.pumpAndSettle();
    await pressCtrl(tester, LogicalKeyboardKey.comma);
    await tester.pumpAndSettle();
    expect(find.byType(SettingsOverlay), findsNothing, reason: 'palette');
    await press(tester, LogicalKeyboardKey.escape);

    await tester.tap(find.byKey(const Key('header-intercept-badge')));
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('intercept-pause-modal')), findsOneWidget);
    await pressCtrl(tester, LogicalKeyboardKey.comma);
    await tester.pumpAndSettle();
    expect(find.byType(SettingsOverlay), findsNothing, reason: 'pause');
  }, variant: linux);

  testWidgets('the daemon page offers the restart of the status bar, and the '
      'search finds it in both languages', (WidgetTester tester) async {
    await open(tester);
    await showSettingsCategory(tester, SettingsCategory.daemon);
    expect(
      find.descendant(
        of: find.byKey(
          const ValueKey<String>('setting-row-${SettingIds.daemonRestart}'),
        ),
        matching: find.byKey(const Key('status-daemon-restart')),
      ),
      findsOneWidget,
    );
    expect(find.text('Restart daemon'), findsOneWidget);
    for (final Locale locale in const <Locale>[Locale('en'), Locale('de')]) {
      final AppLocalizations l10n = lookupAppLocalizations(locale);
      final Iterable<String> found = <String>[
        for (final SettingsSearchGroup group in searchSettings(
          l10n.settingsRestartLabel,
          l10n,
          FakeDaemonClient.defaultInfo,
        ))
          for (final SettingEntry entry in group.entries) entry.id,
      ];
      expect(found, contains(SettingIds.daemonRestart));
    }
  }, variant: linux);

  testWidgets('a scale the window cannot carry says so at the row', (
    WidgetTester tester,
  ) async {
    await open(tester, uiState: scaleState(freshStatePath(), percent: 200));
    await showSettingsCategory(tester, SettingsCategory.interface);
    expect(find.byKey(const Key('settings-scale-limited')), findsOneWidget);
  }, variant: linux);

  testWidgets('a hidden Saved takes no width from the name', (
    WidgetTester tester,
  ) async {
    await open(tester);
    await showSettingsCategory(tester, SettingsCategory.interface);
    final Finder mark = find.descendant(
      of: find.byKey(const ValueKey<String>('setting-row-${FormKeys.uiTheme}')),
      matching: find.byType(AnimatedSwitcher),
    );
    expect(tester.getSize(mark).width, 0);
  }, variant: linux);

  testWidgets('typing while it closes starts no search', (
    WidgetTester tester,
  ) async {
    await open(tester);
    await press(tester, LogicalKeyboardKey.tab);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.keyC);
    await tester.pump();
    expect(find.byType(SettingsOverlay), findsOneWidget, reason: 'closing');
    expect(query(tester), isEmpty);
    await tester.pumpAndSettle();
  }, variant: linux);
}

/// Ein Daemon, der `GetConfig` erst nach [release] beantwortet.
class GatedConfigClient extends FakeDaemonClient {
  GatedConfigClient() : super(info: FakeDaemonClient.disposableInfo) {
    configValues = everything().configValues;
  }

  final Completer<void> _gate = Completer<void>();

  /// Lässt die Antwort durch.
  void release() => _gate.complete();

  @override
  Future<ConfigValues> getConfig() async {
    await _gate.future;
    return super.getConfig();
  }
}
