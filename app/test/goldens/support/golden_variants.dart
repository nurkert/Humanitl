// Die vier Varianten der Goldens aus HUM-054: dunkel und hell, Englisch und
// Deutsch.
//
// Jedes Golden dieses Satzes entsteht als `<name>.<variante>.png` unter
// `test/goldens/goldens/ci/`, mit den Varianten `dark-en`, `dark-de`,
// `light-en` und `light-de`. Gerendert wird im CI-Modus von alchemist
// (`test/flutter_test_config.dart`): Text als Blöcke, keine Schrift des
// Rechners. Deutsch und Englisch unterscheiden sich darin nur in der Breite
// der Blöcke; genau das soll das Golden sehen, denn deutsche Wörter sind
// länger, und eine Zeile, die im Deutschen umbricht, sieht man nur so.
//
// Erneuern mit `make goldens-update`.
library;

import 'package:alchemist/alchemist.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
// `Override` lebt in riverpod 3 im Nebeneingang `misc.dart`.
import 'package:flutter_riverpod/misc.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/shell/providers/language.dart';
import 'package:humanitl/features/shell/providers/theme.dart';
import 'package:humanitl/l10n/l10n.dart';

import 'golden_names.dart';

/// Eine der vier Varianten eines Goldens.
enum GoldenVariant {
  /// Dunkel, Englisch.
  darkEn(HThemeMode.dark, AppLanguage.en),

  /// Dunkel, Deutsch.
  darkDe(HThemeMode.dark, AppLanguage.de),

  /// Hell, Englisch.
  lightEn(HThemeMode.light, AppLanguage.en),

  /// Hell, Deutsch.
  lightDe(HThemeMode.light, AppLanguage.de);

  const GoldenVariant(this.mode, this.language);

  /// Das Theme der Variante.
  final HThemeMode mode;

  /// Die Sprache der Variante.
  final AppLanguage language;

  /// Der Teil des Dateinamens, etwa `dark-de`.
  String get slug => '${mode.name}-${language.name}';

  /// Die Locale der Variante.
  Locale get locale => language.locale;

  /// Die Tokens der Variante.
  HTokens get tokens => mode.resolve(Brightness.dark);

  /// Theme und Sprache für die ganze App (`HumanitlApp`), die beides aus
  /// Providern liest.
  List<Override> get appOverrides => <Override>[
    themeModeProvider.overrideWith(() => VariantTheme(mode)),
    languageProvider.overrideWith(() => VariantLanguage(language)),
  ];
}

/// Die vier Dateiendungen, in der Reihenfolge der Varianten.
List<String> get goldenSlugs => <String>[
  for (final GoldenVariant variant in GoldenVariant.values) variant.slug,
];

/// Der Dateiname ohne Endung, wie alchemist ihn erwartet.
String goldenFileName(String name, GoldenVariant variant) =>
    '$name.${variant.slug}';

/// Legt das Golden [name] in allen vier Varianten an.
///
/// [name] muss in [goldenNames] stehen; ein Name, der dort fehlt, ist ein
/// Fehler beim Anlegen der Tests und nicht erst beim Vergleich, damit die
/// Liste und die Tests nicht auseinanderlaufen. Der Tag `golden` kommt von
/// `goldenTest` selbst.
void goldenVariants(
  String name, {
  required BoxConstraints constraints,
  required Widget Function(GoldenVariant variant) builder,
  PumpAction pumpBeforeTest = onlyPumpAndSettle,
}) {
  if (!goldenNames.contains(name)) {
    throw ArgumentError.value(
      name,
      'name',
      'not in goldenNames (test/goldens/support/golden_names.dart)',
    );
  }
  for (final GoldenVariant variant in GoldenVariant.values) {
    goldenTest(
      '$name ${variant.slug}',
      fileName: goldenFileName(name, variant),
      constraints: constraints,
      pumpBeforeTest: pumpBeforeTest,
      builder: () => builder(variant),
    );
  }
}

/// Ein Stück Oberfläche in der Variante [variant]: Locale, Tokens, ein
/// Overlay für alles, was schwebt, und die Provider aus [overrides].
///
/// Wie `app.dart`, aber ohne Navigator und ohne die Shell. Theme und Sprache
/// stehen hier nicht als Provider, sondern direkt: Ein Golden einer
/// Komponente liest sie nicht aus der Konfiguration.
Widget goldenFrame({
  required GoldenVariant variant,
  required Widget child,
  List<Override> overrides = const <Override>[],
  EdgeInsets padding = EdgeInsets.zero,
  Color? background,
}) {
  final HTokens tokens = variant.tokens;
  return ProviderScope(
    overrides: overrides,
    child: WidgetsApp(
      color: tokens.colors.bg0,
      debugShowCheckedModeBanner: false,
      locale: variant.locale,
      localizationsDelegates: <LocalizationsDelegate<Object?>>[
        ...AppLocalizations.localizationsDelegates,
        ...hLocalizationsDelegates,
      ],
      supportedLocales: AppLocalizations.supportedLocales,
      builder: (BuildContext context, Widget? _) => HTheme(
        tokens: tokens,
        child: ColoredBox(
          color: background ?? tokens.colors.bg1,
          child: Overlay(
            initialEntries: <OverlayEntry>[
              OverlayEntry(
                builder: (BuildContext context) =>
                    Padding(padding: padding, child: child),
              ),
            ],
          ),
        ),
      ),
    ),
  );
}

/// Ein Theme, das nicht umschaltet.
class VariantTheme extends ThemeModeSetting {
  /// Bleibt bei [mode].
  VariantTheme(this.mode);

  /// Das feste Theme.
  final HThemeMode mode;

  @override
  HThemeMode build() => mode;
}

/// Eine Sprache, die nicht aus der Konfiguration kommt.
class VariantLanguage extends LanguageSetting {
  /// Bleibt bei [language].
  VariantLanguage(this.language);

  /// Die feste Sprache.
  final AppLanguage language;

  @override
  AppLanguage? build() => language;
}
