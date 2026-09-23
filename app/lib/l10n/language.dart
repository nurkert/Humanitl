/// The two languages of the interface (HUM-052).
///
/// Englisch ist die Quelle, Deutsch die Übersetzung. Ohne Wahl folgt das
/// Fenster dem Desktop: `WidgetsApp` nimmt die erste bevorzugte Sprache des
/// Desktops, die es kennt, und sonst den ersten Eintrag von
/// `supportedLocales`. Dass das Englisch ist und nicht Deutsch, steht in
/// `l10n.yaml` (`preferred-supported-locales`); `flutter gen-l10n` sortierte
/// die Liste sonst alphabetisch, und ein französischer Desktop — oder `C` und
/// `POSIX` in CI und Containern — bekäme eine deutsche Oberfläche.
library;

import 'dart:ui' show Locale;

import 'generated/app_localizations.dart';

/// A language of the interface. The names are the values of `ui.language`.
enum AppLanguage {
  /// English, the source language.
  en,

  /// German.
  de;

  /// The locale that loads this language.
  Locale get locale => Locale(name);

  /// The language [locale] speaks: German for `de` in any region, English
  /// for everything else.
  static AppLanguage fromLocale(Locale locale) =>
      locale.languageCode == 'de' ? AppLanguage.de : AppLanguage.en;

  /// The name of this language in itself (`English`, `Deutsch`).
  ///
  /// The same in every translation, so a person who cannot read the current
  /// language still finds their own.
  String endonym(AppLocalizations l10n) => switch (this) {
    AppLanguage.en => l10n.commonLanguageEnglish,
    AppLanguage.de => l10n.commonLanguageGerman,
  };
}
