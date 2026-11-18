/// The search of the settings overlay over the catalog (HUM-404, HUM-534).
///
/// Plain and predictable on purpose: every word of the query must occur
/// somewhere in the entry -- its name, help, longer explanation, keywords,
/// the name of its category or its configuration key -- as the daemon's
/// catalog words them in the language of the interface. Case, umlauts and
/// `ß` do not matter. No fuzzy matching: a search that finds a setting for a
/// word it does not contain would be a search nobody can predict.
library;

import 'package:flutter/foundation.dart';

import '../../../core/domain/domain.dart';
import 'settings_registry.dart';

/// The hits of one category, in page order.
@immutable
class SettingsSearchGroup {
  /// Creates a group.
  const SettingsSearchGroup(this.category, this.entries);

  /// The category the hits stand on.
  final SettingsCatalogCategory category;

  /// The hits.
  final List<SettingEntry> entries;
}

/// [text] in the form the search compares: lower case, umlauts as their
/// base letters, `ß` as `ss`, runs of blanks as one.
String foldForSearch(String text) => text
    .toLowerCase()
    .replaceAll('ä', 'a')
    .replaceAll('ö', 'o')
    .replaceAll('ü', 'u')
    .replaceAll('ß', 'ss')
    .replaceAll(RegExp(r'\s+'), ' ')
    .trim();

/// Everything an entry can be found by, folded.
String searchTextOf(SettingEntry entry, SettingsView view) => foldForSearch(
  <String?>[
    entry.title,
    entry.help,
    entry.details,
    entry.keywords,
    view.category(entry.category)?.title,
    entry.id,
  ].whereType<String>().join(' '),
);

/// The entries [query] finds in [view], grouped by category in the order of
/// the left column. Empty for an empty query.
List<SettingsSearchGroup> searchSettings(String query, SettingsView view) {
  final List<String> words = foldForSearch(query)
      .split(' ')
      .where((String word) => word.isNotEmpty)
      .toList();
  if (words.isEmpty) {
    return const <SettingsSearchGroup>[];
  }
  return <SettingsSearchGroup>[
    for (final SettingsCatalogCategory category in view.categories)
      if (<SettingEntry>[
            for (final SettingEntry entry in view.inCategory(category.id))
              if (_matches(searchTextOf(entry, view), words)) entry,
          ]
          case final List<SettingEntry> hits when hits.isNotEmpty)
        SettingsSearchGroup(category, hits),
  ];
}

bool _matches(String text, List<String> words) =>
    words.every((String word) => text.contains(word));
