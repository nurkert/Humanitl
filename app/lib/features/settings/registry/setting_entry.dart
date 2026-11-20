/// One setting as the overlay knows it: where it stands, what it is called,
/// what finds it, and what draws it (HUM-404). Where it stands, what it is
/// called and what finds it come from the catalog of the daemon (HUM-534);
/// the app adds only what draws it.
library;

import 'package:flutter/widgets.dart';

import '../../../core/domain/domain.dart';
import '../../../core/shortcuts/shortcut_catalog.dart';

/// The ids of the rows that are not a key of `config.toml`.
///
/// Every other id of the register is the configuration key the row writes
/// (`FormKeys`), so a `ChangeSetting` fix lands on it without a table in
/// between.
abstract final class SettingIds {
  /// The scale of the application; it lives in the state file of the
  /// interface, not in the configuration (HUM-356).
  static const String uiScale = 'ui.scale';

  /// The version of the daemon the window is connected to.
  static const String daemonVersion = 'daemon.version';

  /// The keyboard shortcuts of the app (HUM-420); the catalog names the
  /// category and this row, the app the shortcuts in it.
  static const String shortcuts = 'shortcuts';

  /// The restart of the daemon, with the control of the shell (HUM-404).
  static const String daemonRestart = 'daemon.restart';

  /// The path of `config.toml`, with Open and Copy path.
  static const String configFile = 'daemon.config_file';

  /// The cards of the saved model servers (HUM-405); the table
  /// `llm.servers` of `config.toml` behind them.
  static const String llmServers = 'llm.servers';

  /// "Who uses which server": `llm.server` of the project and of each
  /// session profile, and the default (HUM-405).
  static const String llmServerUsage = 'llm.server';

  /// "Check connection" of the decision proxy (HUM-511); no key of the
  /// file, the button asks `Egress(check)`.
  static const String egressCheck = 'egress.check';

  /// The cards of the agent depot (HUM-521); no key of the file, the cards
  /// read and write through `Agents` and the keys `agents.<id>.*`.
  static const String agentsDepot = 'agents.depot';

  /// The group of the keyboard shortcuts of [scope] (HUM-420).
  static String shortcutGroup(ShortcutScope scope) => 'shortcuts.${scope.name}';

  /// One keyboard shortcut of the catalogue (HUM-420). The id of the
  /// catalogue is the stable part, so a shortcut keeps its row when its keys
  /// change (HUM-421).
  static String shortcut(AppShortcut shortcut) => 'shortcut.${shortcut.name}';
}

/// What a row builder gets from the page: the configuration as the daemon
/// answered it and the state of the access key.
@immutable
class SettingsPageData {
  /// Creates the data for one build of a page.
  const SettingsPageData({
    required this.config,
    required this.status,
    this.daemonRestart,
    this.onOpenSession,
  });

  /// The configuration as `GetConfig` answered.
  final ConfigValues config;

  /// What the daemon says about the access key.
  final LlmKeyStatus status;

  /// The restart control the shell hands in, or null where no shell is
  /// above (a golden of the overlay alone).
  final Widget? daemonRestart;

  /// Shows the terminal of the running session, as the shell does for any
  /// session (HUM-521); null where no shell is above. A maintenance run of
  /// the agent depot is such a session.
  final VoidCallback? onOpenSession;
}

/// Draws the row of an entry.
typedef SettingBuilder = Widget Function(SettingsPageData data);

/// What the app adds to an entry of the catalog: the builder of its row.
@immutable
class SettingRowSpec {
  /// Creates the spec; [build] null means another row draws this one along
  /// with its own (the endpoint under the detail view of a server, HUM-405).
  const SettingRowSpec({this.build, this.drawnWhen});

  /// Draws the row, or null.
  final SettingBuilder? build;

  /// Whether the page has what the row needs, such as the restart control
  /// of the shell; null means always. Search does not ask: in the app the
  /// shell is always there.
  final bool Function(SettingsPageData data)? drawnWhen;
}

/// One entry of the overlay: a catalog entry of the daemon with the builder
/// of its row, or one of the app's own read-only rows (the keyboard
/// shortcuts).
///
/// Search, the left column and the jump of a `ChangeSetting` fix read the
/// same entry; the row reads its name and help from it as well, so what the
/// search finds is always what the row says.
@immutable
class SettingEntry {
  /// Creates an entry.
  const SettingEntry({
    required this.id,
    required this.category,
    required this.title,
    this.keywords = '',
    this.help,
    this.details,
    this.nextSession = false,
    this.lock,
    this.build,
    this.drawnWhen,
  });

  /// The entry [catalog] with the builder of [spec].
  factory SettingEntry.fromCatalog(
    SettingsCatalogEntry catalog,
    SettingRowSpec? spec,
  ) => SettingEntry(
    id: catalog.id,
    category: catalog.category,
    title: catalog.title,
    help: catalog.help.isEmpty ? null : catalog.help,
    details: catalog.details.isEmpty ? null : catalog.details,
    keywords: catalog.keywords.join(', '),
    nextSession: catalog.nextSession,
    lock: catalog.lock,
    build: spec?.build,
    drawnWhen: spec?.drawnWhen,
  );

  /// The configuration key, or one of [SettingIds].
  final String id;

  /// The id of the category the row stands on.
  final String category;

  /// The name of the setting.
  final String title;

  /// One short sentence under the name, or null.
  final String? help;

  /// The longer explanation behind the info button, or null.
  final String? details;

  /// Comma-separated words that also find the entry; never shown.
  final String keywords;

  /// Whether a running session keeps the old value, so the row says that the
  /// change applies from the next one. Only where it is true.
  final bool nextSession;

  /// The lock of a company policy: the row is read only and names it.
  final SettingsLock? lock;

  /// Draws the row. Null for an entry another entry's builder draws along
  /// with its own, such as the model and the access key under the endpoint
  /// in the detail view of a server (HUM-405).
  final SettingBuilder? build;

  /// Whether the page has what the row needs; null means always.
  final bool Function(SettingsPageData data)? drawnWhen;
}
