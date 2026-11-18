/// The settings catalog of the daemon (HUM-534, ADR-0024 point 3).
///
/// Categories, order, titles, help, keywords and the kind of control of every
/// setting come from the daemon (`GetConfig` with `include_catalog`); the app
/// keeps no list of its own. Only rows that need a control of their own are
/// drawn by a builder of the app, found by [SettingsCatalogEntry.id].
library;

/// How a row is operated, as the catalog names it.
enum SettingControl {
  /// On or off.
  toggle('switch'),

  /// One of [SettingsCatalogEntry.choices].
  choice('choice'),

  /// Free text; the daemon checks it.
  text('text'),

  /// A whole number; range and check in the daemon.
  number('number'),

  /// A reference to a secret; never shown resolved, never written through
  /// `SetConfig`.
  secretRef('secret_ref'),

  /// A path on the host.
  path('path'),

  /// A row with a control of its own, found by its id.
  special('special');

  const SettingControl(this.wire);

  /// The name on the wire.
  final String wire;

  /// The control named [wire]; an unknown name is [special], which a client
  /// draws only when it has a builder for the id.
  static SettingControl parse(String wire) {
    for (final SettingControl control in values) {
      if (control.wire == wire) {
        return control;
      }
    }
    return special;
  }
}

/// A category of the settings.
class SettingsCatalogCategory {
  /// Creates a category.
  const SettingsCatalogCategory({
    required this.id,
    required this.order,
    required this.title,
  });

  /// The stable id, such as `models`.
  final String id;

  /// The place in the left column, from 0.
  final int order;

  /// The name.
  final String title;
}

/// One value of a choice with its name.
class SettingsChoice {
  /// Creates a choice.
  const SettingsChoice({required this.value, required this.title});

  /// The value as `SetConfig` takes it.
  final String value;

  /// The name.
  final String title;
}

/// The lock of a setting by a company policy.
class SettingsLock {
  /// Creates a lock.
  const SettingsLock({required this.lockedBy, this.reason = ''});

  /// The policy that locks, such as the path of its file.
  final String lockedBy;

  /// One sentence why, for the row.
  final String reason;
}

/// One entry of the catalog.
class SettingsCatalogEntry {
  /// Creates an entry.
  const SettingsCatalogEntry({
    required this.id,
    required this.category,
    required this.order,
    required this.title,
    this.help = '',
    this.details = '',
    this.keywords = const <String>[],
    this.control = SettingControl.special,
    this.choices = const <SettingsChoice>[],
    this.nextSession = false,
    this.tier = 'basic',
    this.client = '',
    this.origin = '',
    this.lock,
    this.inProjectChoice = false,
  });

  /// The configuration key, or the id of a row without one.
  final String id;

  /// The id of the category.
  final String category;

  /// The place in the category, from 0.
  final int order;

  /// The name.
  final String title;

  /// One short sentence under the name; empty when there is none.
  final String help;

  /// The longer explanation behind the info button; empty when there is
  /// none.
  final String details;

  /// Words that also find the entry; never shown.
  final List<String> keywords;

  /// How the row is operated.
  final SettingControl control;

  /// The values of a choice with their names.
  final List<SettingsChoice> choices;

  /// Whether a running session keeps the old value.
  final bool nextSession;

  /// `basic`, `advanced` or `expert`.
  final String tier;

  /// Empty for every client, `app` for a setting of the app window itself.
  final String client;

  /// The layer the effective value comes from; empty for a row without a
  /// key.
  final String origin;

  /// The lock of a company policy, or null.
  final SettingsLock? lock;

  /// Whether the key may stand in a project choice (HUM-532).
  final bool inProjectChoice;

  /// Whether a company policy locks the value: the row is read only.
  bool get locked => lock != null;
}

/// The catalog in one language.
class SettingsCatalog {
  /// Creates a catalog.
  const SettingsCatalog({
    this.language = 'en',
    this.categories = const <SettingsCatalogCategory>[],
    this.entries = const <SettingsCatalogEntry>[],
  });

  /// An empty catalog, while no daemon answered.
  static const SettingsCatalog empty = SettingsCatalog();

  /// `en` or `de`.
  final String language;

  /// The categories in the order of the left column.
  final List<SettingsCatalogCategory> categories;

  /// The entries, per category in page order.
  final List<SettingsCatalogEntry> entries;

  /// The entry [id], or null.
  SettingsCatalogEntry? entry(String id) {
    for (final SettingsCatalogEntry entry in entries) {
      if (entry.id == id) {
        return entry;
      }
    }
    return null;
  }

  /// The category [id], or null.
  SettingsCatalogCategory? category(String id) {
    for (final SettingsCatalogCategory category in categories) {
      if (category.id == id) {
        return category;
      }
    }
    return null;
  }

  /// The entries of [categoryId] in page order.
  List<SettingsCatalogEntry> entriesOf(String categoryId) =>
      <SettingsCatalogEntry>[
        for (final SettingsCatalogEntry entry in entries)
          if (entry.category == categoryId) entry,
      ]..sort(
        (SettingsCatalogEntry a, SettingsCatalogEntry b) =>
            a.order.compareTo(b.order),
      );
}
