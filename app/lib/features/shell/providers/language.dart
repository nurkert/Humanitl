/// The language the person chose for this window (HUM-052).
///
/// `null` heißt: niemand hat gewählt, und die Sprache folgt dem Desktop,
/// auch wenn der sie im laufenden Betrieb wechselt (`l10n/language.dart`).
/// Eine Wahl gilt sofort für das ganze Fenster, ohne Neustart: `app.dart`
/// reicht sie als `WidgetsApp.locale` weiter, und jedes Widget, das
/// `context.l10n` liest, baut sich neu.
///
/// Gespeichert wird die Wahl noch nicht. `ui.language` steht im Schema des
/// Daemons, aber dem Client fehlt `GetConfig`, und `SetConfig` nimmt bis
/// HUM-069 nur eine Variable unter `sandbox.env` an (`CONFIG_014`). Lesen und
/// Schreiben des Schlüssels ist HUM-172.
library;

import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../l10n/l10n.dart';

part 'language.g.dart';

/// The chosen language of the interface, or `null` to follow the desktop.
@Riverpod(keepAlive: true, name: 'languageProvider')
class LanguageSetting extends _$LanguageSetting {
  @override
  AppLanguage? build() => null;

  /// Uses [language] from now on, in the whole window at once.
  void set(AppLanguage language) => state = language;
}
