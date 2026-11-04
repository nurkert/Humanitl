/// The left column of the settings overlay: the categories (HUM-404).
library;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

import '../../../core/shortcuts/shortcut_catalog.dart';
import '../../../l10n/l10n.dart';
import '../registry/setting_entry.dart';
import 'settings_list_item.dart';

/// The categories as one stop of the `Tab` order.
///
/// `Tab` reaches the chosen category, the arrow keys move between them and
/// choose as they go, `Home` and `End` jump to the first and the last. The
/// others are left out of the `Tab` order, so `Tab` from the column goes on
/// to the page instead of through seven entries first.
class SettingsNav extends StatefulWidget {
  /// Creates the column; [selected] is the category the page shows, or null
  /// while the search shows its hits.
  const SettingsNav({
    required this.selected,
    required this.onSelect,
    super.key,
  });

  /// The category of the page, or null.
  final SettingsCategory? selected;

  /// Invoked with the chosen category.
  final ValueChanged<SettingsCategory> onSelect;

  @override
  State<SettingsNav> createState() => _SettingsNavState();
}

class _SettingsNavState extends State<SettingsNav> {
  late final Map<SettingsCategory, FocusNode> _nodes =
      <SettingsCategory, FocusNode>{
        for (final SettingsCategory category in SettingsCategory.values)
          category: FocusNode(debugLabel: 'settings-nav-${category.name}'),
      };

  @override
  void dispose() {
    for (final FocusNode node in _nodes.values) {
      node.dispose();
    }
    super.dispose();
  }

  /// The category the arrow keys start from: the focused one, else the
  /// chosen one, else the first.
  SettingsCategory get _current {
    for (final MapEntry<SettingsCategory, FocusNode> entry in _nodes.entries) {
      if (entry.value.hasFocus) {
        return entry.key;
      }
    }
    return widget.selected ?? SettingsCategory.values.first;
  }

  KeyEventResult _onKey(FocusNode _, KeyEvent event) {
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) {
      return KeyEventResult.ignored;
    }
    const List<SettingsCategory> all = SettingsCategory.values;
    final int at = _current.index;
    // Die Tasten stehen im Katalog (HUM-420).
    final int? next = switch (event) {
      _ when AppShortcut.listNext.matches(event) => (at + 1).clamp(
        0,
        all.length - 1,
      ),
      _ when AppShortcut.listPrevious.matches(event) => (at - 1).clamp(
        0,
        all.length - 1,
      ),
      _ when AppShortcut.listFirst.matches(event) => 0,
      _ when AppShortcut.listLast.matches(event) => all.length - 1,
      _ => null,
    };
    if (next == null) {
      return KeyEventResult.ignored;
    }
    _choose(all[next]);
    return KeyEventResult.handled;
  }

  void _choose(SettingsCategory category) {
    widget.onSelect(category);
    _nodes[category]!.requestFocus();
  }

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = context.l10n;
    // Ohne Wahl (während der Suche) steht der erste Eintrag im `Tab`.
    final SettingsCategory tabStop =
        widget.selected ?? SettingsCategory.values.first;
    for (final MapEntry<SettingsCategory, FocusNode> entry in _nodes.entries) {
      entry.value.skipTraversal = entry.key != tabStop;
    }
    return Semantics(
      container: true,
      label: l10n.settingsNavLabel,
      child: Focus(
        canRequestFocus: false,
        skipTraversal: true,
        onKeyEvent: _onKey,
        child: ListView(
          key: const Key('settings-nav'),
          padding: EdgeInsets.zero,
          children: <Widget>[
            for (final SettingsCategory category in SettingsCategory.values)
              SettingsListItem(
                key: ValueKey<String>('settings-nav-${category.name}'),
                title: category.title(l10n),
                selected: category == widget.selected,
                focusNode: _nodes[category],
                onTap: () => _choose(category),
              ),
          ],
        ),
      ),
    );
  }
}
