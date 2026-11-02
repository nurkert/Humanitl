/// The hits of the search in the settings overlay, grouped by category, and
/// the one sentence when there are none (HUM-404).
library;

import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../registry/setting_entry.dart';
import '../registry/settings_search.dart';
import 'setting_row.dart';
import 'settings_list_item.dart';

/// The hits of [query], or the empty state.
class SettingsSearchResults extends StatelessWidget {
  /// Creates the view of [groups] for [query].
  const SettingsSearchResults({
    required this.query,
    required this.groups,
    required this.onChoose,
    required this.onClear,
    super.key,
  });

  /// What was typed, as typed.
  final String query;

  /// The hits, as [searchSettings] grouped them.
  final List<SettingsSearchGroup> groups;

  /// Invoked with the id of the chosen hit.
  final ValueChanged<String> onChoose;

  /// Empties the search.
  final VoidCallback onClear;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    if (groups.isEmpty) {
      return Padding(
        key: const Key('settings-search-empty'),
        padding: const EdgeInsets.symmetric(horizontal: SettingRowGrid.inset),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Text(
              l10n.settingsSearchNone(query.trim()),
              style: tokens.typography.ui13.tinted(tokens.colors.fg1),
            ),
            SizedBox(height: tokens.spacing.x3),
            HButton(
              key: const Key('settings-search-clear'),
              onPressed: onClear,
              child: Text(l10n.settingsSearchClear),
            ),
          ],
        ),
      );
    }
    return Column(
      key: const Key('settings-search-results'),
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        for (int g = 0; g < groups.length; g++) ...<Widget>[
          Padding(
            padding: EdgeInsets.fromLTRB(
              SettingRowGrid.inset,
              g == 0 ? 0 : tokens.spacing.x4,
              SettingRowGrid.inset,
              tokens.spacing.x1,
            ),
            child: Semantics(
              header: true,
              child: Text(
                groups[g].category.title(l10n),
                style: tokens.typography.ui12.semibold.tinted(
                  tokens.colors.fg1,
                ),
              ),
            ),
          ),
          for (final SettingEntry entry in groups[g].entries)
            SettingsListItem(
              key: ValueKey<String>('settings-hit-${entry.id}'),
              title: entry.title(l10n),
              subtitle: entry.help?.call(l10n),
              onTap: () => onChoose(entry.id),
            ),
        ],
      ],
    );
  }
}
