/// The search of the settings overlay over the register (HUM-404).
///
/// Plain and predictable on purpose: every word of the query must occur
/// somewhere in the entry -- its name, help, longer explanation, keywords,
/// the name of its category or its configuration key -- in the language of
/// the interface. Case, umlauts and `ß` do not matter. No fuzzy matching: a
/// search that finds a setting for a word it does not contain would be a
/// search nobody can predict.
library;

import 'package:flutter/foundation.dart';

import '../../../core/domain/domain.dart';
import '../../../l10n/l10n.dart';
import 'settings_registry.dart';

/// The hits of one category, in page order.
@immutable
class SettingsSearchGroup {
  /// Creates a group.
  const SettingsSearchGroup(this.category, this.entries);

  /// The category the hits stand on.
  final SettingsCategory category;

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
String searchTextOf(SettingEntry entry, AppLocalizations l10n) => foldForSearch(
  <String?>[
    entry.title(l10n),
    entry.help?.call(l10n),
    entry.details?.call(l10n),
    entry.keywords(l10n),
    entry.category.title(l10n),
    entry.id,
  ].whereType<String>().join(' '),
);

/// The entries [query] finds for a daemon described by [info], grouped by
/// category in the order of the left column. Empty for an empty query.
List<SettingsSearchGroup> searchSettings(
  String query,
  AppLocalizations l10n,
  DaemonInfo? info,
) {
  final List<String> words = foldForSearch(query)
      .split(' ')
      .where((String word) => word.isNotEmpty)
      .toList();
  if (words.isEmpty) {
    return const <SettingsSearchGroup>[];
  }
  return <SettingsSearchGroup>[
    for (final SettingsCategory category in SettingsCategory.values)
      if (<SettingEntry>[
            for (final SettingEntry entry in settingsOf(category, info))
              if (_matches(searchTextOf(entry, l10n), words)) entry,
          ]
          case final List<SettingEntry> hits when hits.isNotEmpty)
        SettingsSearchGroup(category, hits),
  ];
}

bool _matches(String text, List<String> words) =>
    words.every((String word) => text.contains(word));
