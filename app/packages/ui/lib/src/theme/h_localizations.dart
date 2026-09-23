import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart' as shad;

/// The localizations the widgets of this package need, for every language
/// the application speaks (HUM-052).
///
/// Die Komponentenbibliothek holt ihre eigenen Wörter — Ausschneiden,
/// Kopieren, Einfügen im Kontextmenü eines Eingabefelds — über
/// `ShadcnLocalizations.of`, und das bricht mit einem Null-Check ab, wenn kein
/// Delegate die Sprache des Fensters lädt. Die Bibliothek bringt in 0.0.54 nur
/// Englisch mit. Ohne diesen Eintrag bricht deshalb unter `de` jede ihrer
/// Komponenten ab, die ihre Wörter braucht (Kontextmenü, Datumsauswahl,
/// Formularprüfung); der eigene Delegate der Bibliothek hilft nicht, denn er
/// erklärt `de` für nicht unterstützt und lädt dann nichts. Das Kontextmenü
/// eines `HTextField` braucht außerdem den `KeyboardShortcutDisplayMapper`
/// der Bibliothek und stürzt ohne ihn in jeder Sprache ab (HUM-173).
///
/// Dieser Delegate lädt für jede Sprache, die die Bibliothek nicht kennt, ihre
/// englischen Wörter. Das ist ein bewusster Rückfall: vier Menüwörter auf
/// Englisch in einer deutschen Oberfläche statt eines Absturzes. Bringt eine
/// spätere Fassung Deutsch mit, nimmt er es ohne Änderung.
///
/// Die Liste führt keinen Typ der Bibliothek in ihrer Signatur und darf
/// deshalb aus diesem Paket hinaus (siehe `humanitl_ui.dart`).
const List<LocalizationsDelegate<Object?>> hLocalizationsDelegates =
    <LocalizationsDelegate<Object?>>[_ComponentLocalizationsDelegate()];

class _ComponentLocalizationsDelegate
    extends LocalizationsDelegate<shad.ShadcnLocalizations> {
  const _ComponentLocalizationsDelegate();

  @override
  bool isSupported(Locale locale) => true;

  @override
  Future<shad.ShadcnLocalizations> load(Locale locale) {
    final bool known = shad.ShadcnLocalizations.supportedLocales.any(
      (Locale supported) => supported.languageCode == locale.languageCode,
    );
    return SynchronousFuture<shad.ShadcnLocalizations>(
      shad.lookupShadcnLocalizations(known ? locale : const Locale('en')),
    );
  }

  @override
  bool shouldReload(_ComponentLocalizationsDelegate old) => false;
}
