/// The right side of the settings overlay: the rows of one category
/// (HUM-404).
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/connection.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../registry/settings_registry.dart';
import 'setting_row.dart';

/// The page of [category]: its name, then its rows in register order, one
/// hairline between two rows.
class SettingsCategoryPage extends ConsumerWidget {
  /// Creates the page of [category] over [data].
  const SettingsCategoryPage({
    required this.category,
    required this.data,
    super.key,
  });

  /// The category shown.
  final SettingsCategory category;

  /// The configuration and the state of the key.
  final SettingsPageData data;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final DaemonInfo? info = ref.watch(daemonInfoProvider).value;
    final List<Widget> rows = <Widget>[
      for (final SettingEntry entry in settingsOf(category, info))
        if (entry.build case final SettingBuilder build
            when entry.drawnWhen?.call(data) ?? true)
          build(data),
    ];
    return Column(
      key: ValueKey<String>('settings-page-${category.name}'),
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Padding(
          padding: EdgeInsets.fromLTRB(
            SettingRowGrid.inset,
            0,
            SettingRowGrid.inset,
            tokens.spacing.x2,
          ),
          child: Semantics(
            header: true,
            child: Text(
              category.title(context.l10n),
              style: tokens.typography.ui16.semibold.tinted(tokens.colors.fg0),
            ),
          ),
        ),
        for (int i = 0; i < rows.length; i++) ...<Widget>[
          if (i > 0) const HHairline(),
          rows[i],
        ],
      ],
    );
  }
}
