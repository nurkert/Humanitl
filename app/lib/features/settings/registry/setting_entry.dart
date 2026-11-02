/// One setting as the overlay knows it: where it stands, what it is called,
/// what finds it, and what draws it (HUM-404).
library;

import 'package:flutter/widgets.dart';

import '../../../core/domain/domain.dart';
import '../../../l10n/l10n.dart';

/// The categories of the overlay, in the order of the left column.
enum SettingsCategory {
  /// The saved model servers, each with endpoint, models and key, and who
  /// uses which one (HUM-405).
  models,

  /// The coding agent a new session starts.
  agent,

  /// The project folder and where the agent writes.
  sandbox,

  /// How long a held request waits and where it is asked.
  requests,

  /// Language, appearance, scale and notifications of this window.
  interface,

  /// The certificates the daemon trusts.
  network,

  /// The background service itself: version, updates, `config.toml`.
  daemon;

  /// The name in the left column and over the page.
  String title(AppLocalizations l10n) => switch (this) {
    SettingsCategory.models => l10n.settingsCategoryModels,
    SettingsCategory.agent => l10n.settingsCategoryAgent,
    SettingsCategory.sandbox => l10n.settingsCategorySandbox,
    SettingsCategory.requests => l10n.settingsCategoryRequests,
    SettingsCategory.interface => l10n.settingsCategoryInterface,
    SettingsCategory.network => l10n.settingsCategoryNetwork,
    SettingsCategory.daemon => l10n.settingsCategoryDaemon,
  };
}

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
  });

  /// The configuration as `GetConfig` answered.
  final ConfigValues config;

  /// What the daemon says about the access key.
  final LlmKeyStatus status;

  /// The restart control the shell hands in, or null where no shell is
  /// above (a golden of the overlay alone).
  final Widget? daemonRestart;
}

/// Draws the row of an entry.
typedef SettingBuilder = Widget Function(SettingsPageData data);

/// One entry of the register.
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
    required this.keywords,
    this.help,
    this.details,
    this.nextSession = false,
    this.shownWhen,
    this.drawnWhen,
    this.build,
  });

  /// The configuration key, or one of [SettingIds].
  final String id;

  /// The page the row stands on.
  final SettingsCategory category;

  /// The name of the setting.
  final String Function(AppLocalizations l10n) title;

  /// One short sentence under the name, or null.
  final String? Function(AppLocalizations l10n)? help;

  /// The longer explanation behind the info button, or null.
  final String Function(AppLocalizations l10n)? details;

  /// Comma-separated words that also find the entry; never shown.
  final String Function(AppLocalizations l10n) keywords;

  /// Whether a running session keeps the old value, so the row says that the
  /// change applies from the next one. Only where it is true.
  final bool nextSession;

  /// Whether the daemon offers the setting at all; null means always. A row
  /// the daemon does not offer is neither drawn nor found.
  final bool Function(DaemonInfo? info)? shownWhen;

  /// Whether the page has what the row needs, such as the restart control
  /// of the shell; null means always. Search does not ask: in the app the
  /// shell is always there.
  final bool Function(SettingsPageData data)? drawnWhen;

  /// Draws the row. Null for an entry another entry's builder draws along
  /// with its own, such as the model and the access key under the endpoint
  /// in the detail view of a server (HUM-405).
  final SettingBuilder? build;

  /// Whether the entry stands in the overlay for a daemon described by
  /// [info].
  bool shownFor(DaemonInfo? info) => shownWhen?.call(info) ?? true;
}
