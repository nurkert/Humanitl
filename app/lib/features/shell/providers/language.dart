/// The language the person chose for this window (HUM-052).
///
/// `null` heißt: niemand hat gewählt, und die Sprache folgt dem Desktop,
/// auch wenn der sie im laufenden Betrieb wechselt (`l10n/language.dart`).
/// Eine Wahl gilt sofort für das ganze Fenster, ohne Neustart: `app.dart`
/// reicht sie als `WidgetsApp.locale` weiter, und jedes Widget, das
/// `context.l10n` liest, baut sich neu.
///
/// Die Wahl startet aus `ui.language`, sobald jemand den Schlüssel gesetzt
/// hat (HUM-069: das Einstellungsformular schreibt ihn); an der Herkunft
/// `default` erkennt sie, dass niemand gewählt hat, und folgt dann dem Desktop.
/// Eine Wahl in der Palette gilt für dieses Fenster und schreibt nichts; sie
/// in die Konfiguration zu schreiben und den ersten Start mit der Sprache des
/// Desktops vorzubelegen, bleibt HUM-172.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/config_providers.dart';
import '../../../l10n/l10n.dart';

part 'language.g.dart';

/// The chosen language of the interface, or `null` to follow the desktop.
@Riverpod(keepAlive: true, name: 'languageProvider')
class LanguageSetting extends _$LanguageSetting {
  @override
  AppLanguage? build() => ref.watch(
    configProvider.select((AsyncValue<ConfigValues> config) {
      final ConfigValues? values = config.value;
      if (values == null || !values.isSet(FormKeys.uiLanguage)) {
        return null;
      }
      return values.uiLanguage == 'de' ? AppLanguage.de : AppLanguage.en;
    }),
  );

  /// Uses [language] from now on, in the whole window at once.
  void set(AppLanguage language) => state = language;
}
