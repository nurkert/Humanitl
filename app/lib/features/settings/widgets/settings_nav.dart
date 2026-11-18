/// The left column of the settings overlay: the categories of the catalog
/// (HUM-404, HUM-534).
library;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

import '../../../core/domain/domain.dart';
import '../../../core/shortcuts/shortcut_catalog.dart';
import '../../../l10n/l10n.dart';
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
    required this.categories,
    required this.selected,
    required this.onSelect,
    super.key,
  });

  /// The categories of the catalog, in the order of the column.
  final List<SettingsCatalogCategory> categories;

  /// The id of the category of the page, or null.
  final String? selected;

  /// Invoked with the id of the chosen category.
  final ValueChanged<String> onSelect;

  @override
  State<SettingsNav> createState() => _SettingsNavState();
}

class _SettingsNavState extends State<SettingsNav> {
  final Map<String, FocusNode> _nodes = <String, FocusNode>{};

  @override
  void dispose() {
    for (final FocusNode node in _nodes.values) {
      node.dispose();
    }
    super.dispose();
  }

  List<String> get _ids => <String>[
    for (final SettingsCatalogCategory category in widget.categories)
      category.id,
  ];

  FocusNode _node(String id) =>
      _nodes.putIfAbsent(id, () => FocusNode(debugLabel: 'settings-nav-$id'));

  /// The category the arrow keys start from: the focused one, else the
  /// chosen one, else the first.
  String? get _current {
    for (final String id in _ids) {
      if (_nodes[id]?.hasFocus ?? false) {
        return id;
      }
    }
    return widget.selected ?? (_ids.isEmpty ? null : _ids.first);
  }

  KeyEventResult _onKey(FocusNode _, KeyEvent event) {
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) {
      return KeyEventResult.ignored;
    }
    final List<String> all = _ids;
    final String? current = _current;
    if (current == null || all.isEmpty) {
      return KeyEventResult.ignored;
    }
    final int at = all.indexOf(current).clamp(0, all.length - 1);
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

  void _choose(String category) {
    widget.onSelect(category);
    _node(category).requestFocus();
  }

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = context.l10n;
    // Ohne Wahl (während der Suche) steht der erste Eintrag im `Tab`.
    final List<String> ids = _ids;
    final String? tabStop = widget.selected ?? (ids.isEmpty ? null : ids.first);
    for (final String id in ids) {
      _node(id).skipTraversal = id != tabStop;
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
            for (final SettingsCatalogCategory category in widget.categories)
              SettingsListItem(
                key: ValueKey<String>('settings-nav-${category.id}'),
                title: category.title,
                selected: category.id == widget.selected,
                focusNode: _node(category.id),
                onTap: () => _choose(category.id),
              ),
          ],
        ),
      ),
    );
  }
}
