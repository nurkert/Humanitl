// Die Sprache des Fensters (HUM-052): Wechsel im laufenden Programm, ohne
// Neustart und ohne die gehaltene Anfrage zu verlieren, und der Rückfall auf
// Englisch, wenn der Desktop eine andere Sprache spricht.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/widgets/queue_row.dart';
import 'package:humanitl/features/shell/providers/language.dart';
import 'package:humanitl/features/shell/shell_screen.dart';
import 'package:humanitl/l10n/l10n.dart';

import '../../harness/app_harness.dart';
import '../intercept/fixtures.dart';

final AppLocalizations en = lookupAppLocalizations(const Locale('en'));
final AppLocalizations de = lookupAppLocalizations(const Locale('de'));

String headerTitle(WidgetTester tester) =>
    tester.widget<Text>(find.byKey(const Key('header-section-title'))).data!;

FlowDetail held() => detailFor(
  heldFlow(
    n: 1,
    deadline: testStart.add(const Duration(minutes: 5)),
    method: Method.post,
    host: 'api.github.com',
    path: '/graphql',
  ),
);

final TargetPlatformVariant linux = TargetPlatformVariant.only(
  TargetPlatform.linux,
);

void main() {
  testWidgets('language_switch_updates_texts', (WidgetTester tester) async {
    final FakeDaemonClient client = FakeDaemonClient(
      script: holdScript(<FlowDetail>[held()]),
      clock: () => testStart,
    );
    await pumpApp(tester, client: client);
    await tester.pump(const Duration(milliseconds: 400));
    await tester.pump();
    await tester.pump();

    expect(find.byType(QueueRow), findsOneWidget);
    expect(find.text(en.interceptAllowButton), findsWidgets);
    expect(find.text(de.interceptAllowButton), findsNothing);
    expect(headerTitle(tester), en.shellNavIntercept);

    ProviderScope.containerOf(tester.element(find.byType(ShellScreen)))
        .read(languageProvider.notifier)
        .set(AppLanguage.de);
    await tester.pump();

    // Der Knopf heißt „Senden“ (docs/GLOSSARY.md), der Abschnitt „Anhalten“.
    expect(find.text('Senden'), findsWidgets);
    expect(find.text(en.interceptAllowButton), findsNothing);
    expect(headerTitle(tester), 'Anhalten');
    // Ohne Neustart: dieselbe Anfrage steht noch da, niemand hat entschieden.
    expect(find.byType(QueueRow), findsOneWidget);
    expect(client.decisions, isEmpty);
  }, variant: linux);

  testWidgets('the palette switches the language and back', (
    WidgetTester tester,
  ) async {
    await pumpApp(tester, client: FakeDaemonClient());
    expect(headerTitle(tester), 'Intercept');

    await pressCtrl(tester, LogicalKeyboardKey.keyK);
    await tester.enterText(find.byKey(const Key('palette-input')), 'Deutsch');
    await tester.pump();
    expect(find.text(en.shellPaletteSwitchLanguage('Deutsch')), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();

    expect(headerTitle(tester), 'Anhalten');

    await pressCtrl(tester, LogicalKeyboardKey.keyK);
    await tester.enterText(find.byKey(const Key('palette-input')), 'English');
    await tester.pump();
    expect(find.text(de.shellPaletteSwitchLanguage('English')), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();

    expect(headerTitle(tester), 'Intercept');
  }, variant: linux);

  testWidgets('a German desktop gets German without a choice', (
    WidgetTester tester,
  ) async {
    tester.platformDispatcher.localesTestValue = const <Locale>[
      Locale('de', 'AT'),
    ];
    addTearDown(tester.platformDispatcher.clearLocalesTestValue);
    await pumpApp(tester, client: FakeDaemonClient());

    expect(headerTitle(tester), 'Anhalten');
  }, variant: linux);

  testWidgets('any other desktop language falls back to English', (
    WidgetTester tester,
  ) async {
    // `flutter gen-l10n` sortiert `supportedLocales` alphabetisch; ohne
    // `preferred-supported-locales: [en]` in l10n.yaml fiele ein
    // französischer Desktop auf Deutsch zurück.
    tester.platformDispatcher.localesTestValue = const <Locale>[
      Locale('fr', 'FR'),
    ];
    addTearDown(tester.platformDispatcher.clearLocalesTestValue);
    await pumpApp(tester, client: FakeDaemonClient());

    expect(headerTitle(tester), 'Intercept');
  }, variant: linux);

  testWidgets('the app registers the words of the component library', (
    WidgetTester tester,
  ) async {
    // Ohne sie findet unter `de` keine Komponente der Bibliothek ihre Wörter
    // (`packages/ui/test/localizations_test.dart`).
    await pumpApp(tester, client: FakeDaemonClient());
    final WidgetsApp app = tester.widget<WidgetsApp>(find.byType(WidgetsApp));
    expect(app.localizationsDelegates, containsAll(hLocalizationsDelegates));
  });

  test('AppLanguage.fromLocale: de in any region, English otherwise', () {
    expect(AppLanguage.fromLocale(const Locale('de')), AppLanguage.de);
    expect(AppLanguage.fromLocale(const Locale('de', 'CH')), AppLanguage.de);
    expect(AppLanguage.fromLocale(const Locale('en', 'US')), AppLanguage.en);
    expect(AppLanguage.fromLocale(const Locale('fr')), AppLanguage.en);
    // Was der Desktop unter C oder POSIX meldet.
    expect(AppLanguage.fromLocale(const Locale('C')), AppLanguage.en);
    expect(AppLanguage.fromLocale(const Locale('und')), AppLanguage.en);
  });
}
