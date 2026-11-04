/// The category Keyboard shortcuts: every shortcut the application binds,
/// grouped by where it applies, read only (HUM-420).
///
/// Everything here comes from the catalogue (`core/shortcuts`), the same
/// entries the bindings read their keys from; nothing is written down twice.
/// Changing a shortcut comes with HUM-421 and will start from the same rows.
library;

import 'package:flutter/widgets.dart';

import '../../../core/shortcuts/shortcut_catalog.dart';
import '../../../core/shortcuts/shortcut_labels.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../registry/setting_entry.dart';
import '../settings_scope.dart';
import '../widgets/setting_row.dart';

/// The register entries of the category: per scope one entry that draws the
/// group, and per shortcut one that the group draws along with it.
///
/// A shortcut entry is found by its name, by its keys as text (`Ctrl+K`) and
/// by its scope, and the hit names the scope and the keys under the name.
List<SettingEntry> shortcutEntries() => <SettingEntry>[
  for (final ShortcutScope scope in ShortcutScope.values) ...<SettingEntry>[
    SettingEntry(
      id: SettingIds.shortcutGroup(scope),
      category: SettingsCategory.shortcuts,
      title: scope.title,
      keywords: (AppLocalizations l10n) => l10n.settingsKeywordsShortcuts,
      build: (SettingsPageData data) => ShortcutGroup(scope: scope),
    ),
    for (final AppShortcut shortcut in AppShortcut.of(scope))
      SettingEntry(
        id: SettingIds.shortcut(shortcut),
        category: SettingsCategory.shortcuts,
        title: shortcut.label,
        help: (AppLocalizations l10n) =>
            '${scope.title(l10n)} · ${shortcutText(shortcut, l10n)}',
        keywords: (AppLocalizations l10n) => shortcutText(shortcut, l10n),
        // Ein Kürzel, das es auf dieser Plattform nicht gibt, steht weder in
        // der Übersicht noch in der Suche.
        shownWhen: (_) => shortcut.appliesOn(),
      ),
  ],
];

/// The shortcuts of [scope]: its name, then one row per shortcut.
class ShortcutGroup extends StatelessWidget {
  /// Creates the group of [scope].
  const ShortcutGroup({required this.scope, super.key});

  /// The scope shown.
  final ShortcutScope scope;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final SettingsController? settings = SettingsScope.maybeOf(context);
    final String id = SettingIds.shortcutGroup(scope);
    return Padding(
      key: settings?.rowKey(id),
      padding: EdgeInsets.symmetric(vertical: tokens.spacing.x2),
      child: Column(
        key: ValueKey<String>('shortcut-group-${scope.name}'),
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Padding(
            padding: EdgeInsets.fromLTRB(
              SettingRowGrid.inset,
              0,
              SettingRowGrid.inset,
              tokens.spacing.x1,
            ),
            child: Semantics(
              header: true,
              child: Text(
                scope.title(context.l10n),
                style: tokens.typography.ui12.semibold.tinted(
                  tokens.colors.fg1,
                ),
              ),
            ),
          ),
          for (final AppShortcut shortcut in AppShortcut.of(scope))
            if (shortcut.appliesOn()) ShortcutRow(shortcut: shortcut),
        ],
      ),
    );
  }
}

/// One shortcut: its name on the left, its keys as caps on the right.
///
/// No control and no focus stop: the page only reads. A jump from the search
/// lights the row like any other row of the overlay.
class ShortcutRow extends StatelessWidget {
  /// Creates the row of [shortcut].
  const ShortcutRow({required this.shortcut, super.key});

  /// The shortcut shown.
  final AppShortcut shortcut;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final SettingsController? settings = SettingsScope.maybeOf(context);
    final String id = SettingIds.shortcut(shortcut);
    final bool lit = settings?.highlighted == id;
    final String label = shortcut.label(l10n);
    return Semantics(
      label: '$label, ${shortcutText(shortcut, l10n)}',
      excludeSemantics: true,
      child: AnimatedContainer(
        key: settings?.rowKey(id),
        // Wie `SettingRow`: Das Aufleuchten nach einem Sprung ist eine Farbe,
        // keine Bewegung.
        duration: HMotion.sweep,
        curve: HMotion.exit,
        decoration: BoxDecoration(
          color: HColorDerivation.fade(tokens.colors.accent, lit ? 0.12 : 0),
          borderRadius: HRadius.controlRadius,
        ),
        padding: EdgeInsets.symmetric(
          horizontal: SettingRowGrid.inset,
          vertical: tokens.spacing.x1,
        ),
        child: Row(
          key: ValueKey<String>('shortcut-row-${shortcut.name}'),
          children: <Widget>[
            Expanded(
              child: Text(
                label,
                style: tokens.typography.ui13.tinted(tokens.colors.fg0),
              ),
            ),
            SizedBox(width: SettingRowGrid.gutter),
            // Die Tasten in ihrer eigenen Breite an der rechten Kante; der
            // Name nimmt, was bleibt, und bricht um, statt sie zu drücken.
            HKeyCaps(shortcutChords(shortcut, l10n)),
          ],
        ),
      ),
    );
  }
}
