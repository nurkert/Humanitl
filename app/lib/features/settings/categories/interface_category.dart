/// The category Interface: language, appearance, scale and desktop
/// notifications of this window (HUM-404).
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/ui.dart';
import '../../../core/ui/ui_scale.dart';
import '../../../l10n/l10n.dart';
import '../registry/setting_entry.dart';
import '../settings_scope.dart';
import '../widgets/setting_controls.dart';
import '../widgets/setting_row.dart';

/// `ui.language`, each language in its own name.
class LanguageRow extends StatelessWidget {
  /// Creates the row for [config].
  const LanguageRow({required this.config, super.key});

  /// The configuration as `GetConfig` answered.
  final ConfigValues config;

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = context.l10n;
    return SettingRow(
      settingId: FormKeys.uiLanguage,
      child: HSegmented<String>(
        key: const Key('settings-language'),
        options: <HSegmentOption<String>>[
          for (final AppLanguage language in AppLanguage.values)
            HSegmentOption<String>(
              value: language.name,
              label: language.endonym(l10n),
            ),
        ],
        selected: config.uiLanguage,
        onSelect: (String language) =>
            writeSetting(context, FormKeys.uiLanguage, language),
      ),
    );
  }
}

/// `ui.theme`: dark, light, or as the system says.
class ThemeRow extends StatelessWidget {
  /// Creates the row for [config].
  const ThemeRow({required this.config, super.key});

  /// The configuration as `GetConfig` answered.
  final ConfigValues config;

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = context.l10n;
    return SettingRow(
      settingId: FormKeys.uiTheme,
      child: HSegmented<String>(
        key: const Key('settings-theme'),
        options: <HSegmentOption<String>>[
          HSegmentOption<String>(value: 'dark', label: l10n.settingsThemeDark),
          HSegmentOption<String>(
            value: 'light',
            label: l10n.settingsThemeLight,
          ),
          HSegmentOption<String>(
            value: 'system',
            label: l10n.settingsThemeSystem,
          ),
        ],
        selected: config.uiTheme,
        onSelect: (String theme) =>
            writeSetting(context, FormKeys.uiTheme, theme),
      ),
    );
  }
}

/// Die Stufen der Skalierung (HUM-356), mit dem Wert, der gerade gilt.
///
/// Sie gehört diesem Rechner und seinem Schirm, nicht der Konfiguration des
/// Daemons, und steht deshalb in der Zustandsdatei der Oberfläche; sie gilt
/// sofort. Eine Stufe über der Grenze des Fensters bleibt wählbar: Sie gilt,
/// sobald das Fenster groß genug ist, und bis dahin nennt die Erklärung den
/// Wert, der gilt. Die Tasten erreichen auch die Werte zwischen den Stufen;
/// dann ist keine Stufe gewählt.
class ScaleRow extends ConsumerWidget {
  /// Creates the row.
  const ScaleRow({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final int preferred = ref.watch(uiScaleProvider);
    final int effective = UiScaleScope.maybeOf(context)?.effective ?? preferred;
    return SettingRow(
      settingId: SettingIds.uiScale,
      details: l10n.settingsScaleHelp(l10n.shellScalePercent(effective)),
      // Ein Zustand, keine Erklärung: Trägt das Fenster die Wahl nicht, steht
      // der Wert, der gilt, sichtbar an der Zeile und nicht hinter dem Info.
      status: <Widget>[
        if (effective != preferred)
          Text(
            l10n.settingsScaleLimited(l10n.shellScalePercent(effective)),
            key: const Key('settings-scale-limited'),
            style: tokens.typography.ui12.tinted(tokens.colors.fg1),
          ),
      ],
      child: HChoiceChips<int>(
        key: const Key('settings-scale'),
        alignment: WrapAlignment.end,
        options: <HSegmentOption<int>>[
          for (final int percent in uiScalePresets)
            HSegmentOption<int>(
              value: percent,
              label: l10n.shellScalePercent(percent),
            ),
        ],
        selected: <int>{preferred},
        onToggle: (int percent) {
          ref.read(uiScaleProvider.notifier).choose(percent);
          SettingsScope.maybeOf(context)?.markSaved(SettingIds.uiScale);
        },
      ),
    );
  }
}

/// `ui.notifications`: whether the desktop announces a waiting request.
class NotificationsRow extends StatelessWidget {
  /// Creates the row for [config].
  const NotificationsRow({required this.config, super.key});

  /// The configuration as `GetConfig` answered.
  final ConfigValues config;

  @override
  Widget build(BuildContext context) => SettingRow(
    settingId: FormKeys.uiNotifications,
    child: OnOffControl(
      key: const Key('settings-notifications'),
      value: config.uiNotifications,
      onChanged: (bool on) =>
          writeSetting(context, FormKeys.uiNotifications, '$on'),
    ),
  );
}
