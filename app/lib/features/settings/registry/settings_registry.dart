/// The register of every setting the overlay shows (HUM-404).
///
/// One list instead of a hand-written column: the left column, the search
/// and the jump of a `ChangeSetting` fix all read it, and a page draws its
/// entries in this order. A new setting is one entry here and one row
/// widget in the file of its category; nothing else needs to know of it.
///
/// Every value comes from `GetConfig` and goes back through `SetConfig` and
/// `LlmKey`; the rows decide nothing about what a value means (ADR-018).
library;

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_diagnostics.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../categories/agent_category.dart';
import '../categories/daemon_category.dart';
import '../categories/interface_category.dart';
import '../categories/models_category.dart';
import '../categories/network_category.dart';
import '../categories/requests_category.dart';
import '../categories/sandbox_category.dart';
import '../categories/shortcuts_category.dart';
import '../widgets/sandbox_backend_row.dart';
import '../widgets/sandbox_mode_row.dart';
import '../widgets/update_row.dart';
import 'setting_entry.dart';

export 'setting_entry.dart';

/// Every entry, by category in the order of [SettingsCategory] and within
/// one in the order of its page.
final List<SettingEntry> settingsRegistry = <SettingEntry>[
  // Modelle (HUM-405): Die Karten der Server, die Detailansicht eines
  // Servers und die Tabelle „Wer nutzt welchen Server" zeichnet
  // `ModelsCategory` in einem Stück. Endpunkt, Modell und Schlüssel sind
  // Zeilen der Detailansicht; ein Sprung dorthin öffnet die des Standards.
  SettingEntry(
    id: SettingIds.llmServers,
    category: SettingsCategory.models,
    title: (AppLocalizations l10n) => l10n.settingsServersLabel,
    help: (AppLocalizations l10n) => l10n.settingsServersIntro,
    keywords: (AppLocalizations l10n) => l10n.settingsKeywordsServers,
    build: buildModelsCategory,
  ),
  SettingEntry(
    id: FormKeys.llmEndpoint,
    category: SettingsCategory.models,
    title: (AppLocalizations l10n) => l10n.settingsEndpointLabel,
    help: (AppLocalizations l10n) => l10n.settingsEndpointHelp,
    keywords: (AppLocalizations l10n) => l10n.settingsKeywordsEndpoint,
    nextSession: true,
  ),
  SettingEntry(
    id: FormKeys.llmModels,
    category: SettingsCategory.models,
    title: (AppLocalizations l10n) => l10n.settingsModelLabel,
    keywords: (AppLocalizations l10n) => l10n.settingsKeywordsModel,
    nextSession: true,
  ),
  SettingEntry(
    id: FormKeys.llmApiKeyRef,
    category: SettingsCategory.models,
    title: (AppLocalizations l10n) => l10n.settingsKeyLabel,
    help: (AppLocalizations l10n) => l10n.settingsKeyHelpShort,
    details: (AppLocalizations l10n) => l10n.settingsKeyHelp,
    keywords: (AppLocalizations l10n) => l10n.settingsKeywordsKey,
    nextSession: true,
  ),
  SettingEntry(
    id: SettingIds.llmServerUsage,
    category: SettingsCategory.models,
    title: (AppLocalizations l10n) => l10n.settingsServerUsageTitle,
    help: (AppLocalizations l10n) => l10n.settingsServerUsageHelp,
    keywords: (AppLocalizations l10n) => l10n.settingsKeywordsServerUsage,
  ),
  SettingEntry(
    id: FormKeys.agentAdapter,
    category: SettingsCategory.agent,
    title: (AppLocalizations l10n) => l10n.settingsAgentLabel,
    help: (AppLocalizations l10n) => l10n.settingsAgentHelpShort,
    details: (AppLocalizations l10n) => l10n.settingsAgentHelp,
    keywords: (AppLocalizations l10n) => l10n.settingsKeywordsAgent,
    nextSession: true,
    build: (SettingsPageData data) => AgentRow(config: data.config),
  ),
  SettingEntry(
    id: FormKeys.sandboxWorkDir,
    category: SettingsCategory.sandbox,
    title: (AppLocalizations l10n) => l10n.settingsWorkDirLabel,
    help: (AppLocalizations l10n) => sandboxUsesRealPaths
        ? l10n.settingsWorkDirHelpPolicy
        : l10n.settingsWorkDirHelp,
    keywords: (AppLocalizations l10n) => l10n.settingsKeywordsWorkDir,
    nextSession: true,
    build: (SettingsPageData data) => WorkDirRow(config: data.config),
  ),
  SettingEntry(
    id: FormKeys.sandboxMode,
    category: SettingsCategory.sandbox,
    title: (AppLocalizations l10n) => l10n.settingsSandboxModeLabel,
    help: (AppLocalizations l10n) => l10n.settingsSandboxModeHelpShort,
    details: (AppLocalizations l10n) => l10n.settingsSandboxModeHelp,
    keywords: (AppLocalizations l10n) => l10n.settingsKeywordsSandboxMode,
    nextSession: true,
    // Nur wo es mehr als einen Modus gibt (HUM-357); auf macOS weder Zeile
    // noch Treffer.
    shownWhen: (DaemonInfo? info) => info?.offersDisposable ?? false,
    build: (SettingsPageData data) => SandboxModeRow(config: data.config),
  ),
  // Das Backend der nächsten Sitzung (HUM-440). Welche es gibt und welche
  // wählbar sind, sagt `SandboxBackends`; unter macOS steht nur Seatbelt
  // da, und die Zeile nennt es ohne Wahl.
  SettingEntry(
    id: FormKeys.sandboxBackend,
    category: SettingsCategory.sandbox,
    title: (AppLocalizations l10n) => l10n.settingsSandboxBackendLabel,
    help: (AppLocalizations l10n) => l10n.settingsSandboxBackendHelp,
    keywords: (AppLocalizations l10n) => l10n.settingsKeywordsSandboxBackend,
    nextSession: true,
    build: (SettingsPageData data) => SandboxBackendRow(config: data.config),
  ),
  SettingEntry(
    id: FormKeys.holdTimeoutSecs,
    category: SettingsCategory.requests,
    title: (AppLocalizations l10n) => l10n.settingsTimeoutLabel,
    help: (AppLocalizations l10n) => l10n.settingsTimeoutHelp,
    keywords: (AppLocalizations l10n) => l10n.settingsKeywordsTimeout,
    build: (SettingsPageData data) => TimeoutRow(config: data.config),
  ),
  SettingEntry(
    id: FormKeys.holdAskMode,
    category: SettingsCategory.requests,
    title: (AppLocalizations l10n) => l10n.settingsAskModeLabel,
    help: (AppLocalizations l10n) => l10n.settingsAskModeHelp,
    keywords: (AppLocalizations l10n) => l10n.settingsKeywordsAskMode,
    build: (SettingsPageData data) => AskModeRow(config: data.config),
  ),
  SettingEntry(
    id: FormKeys.uiLanguage,
    category: SettingsCategory.interface,
    title: (AppLocalizations l10n) => l10n.settingsLanguageLabel,
    keywords: (AppLocalizations l10n) => l10n.settingsKeywordsLanguage,
    build: (SettingsPageData data) => LanguageRow(config: data.config),
  ),
  SettingEntry(
    id: FormKeys.uiTheme,
    category: SettingsCategory.interface,
    title: (AppLocalizations l10n) => l10n.settingsThemeLabel,
    keywords: (AppLocalizations l10n) => l10n.settingsKeywordsTheme,
    build: (SettingsPageData data) => ThemeRow(config: data.config),
  ),
  SettingEntry(
    id: SettingIds.uiScale,
    category: SettingsCategory.interface,
    title: (AppLocalizations l10n) => l10n.settingsScaleLabel,
    help: (AppLocalizations l10n) => l10n.settingsScaleHelpShort,
    details: (AppLocalizations l10n) =>
        l10n.settingsScaleHelp(l10n.shellScalePercent(100)),
    keywords: (AppLocalizations l10n) => l10n.settingsKeywordsScale,
    build: (SettingsPageData data) => const ScaleRow(),
  ),
  SettingEntry(
    id: FormKeys.uiNotifications,
    category: SettingsCategory.interface,
    title: (AppLocalizations l10n) => l10n.settingsNotificationsLabel,
    help: (AppLocalizations l10n) => l10n.settingsNotificationsHint,
    keywords: (AppLocalizations l10n) => l10n.settingsKeywordsNotifications,
    build: (SettingsPageData data) => NotificationsRow(config: data.config),
  ),
  // Tastenkürzel (HUM-420): eine Gruppe je Bereich, darin jedes Kürzel des
  // Katalogs, nur zum Lesen.
  ...shortcutEntries(),
  SettingEntry(
    id: FormKeys.tlsTrustSystemStore,
    category: SettingsCategory.network,
    title: (AppLocalizations l10n) => l10n.settingsTrustSystemStoreLabel,
    help: (AppLocalizations l10n) => l10n.settingsTrustSystemStoreHint,
    details: (AppLocalizations l10n) =>
        l10n.settingsTrustSystemStoreHelp(ClientDiagnostics.restartUnitCommand),
    keywords: (AppLocalizations l10n) => l10n.settingsKeywordsTrust,
    build: (SettingsPageData data) => TrustStoreRow(config: data.config),
  ),
  SettingEntry(
    id: SettingIds.daemonVersion,
    category: SettingsCategory.daemon,
    title: (AppLocalizations l10n) => l10n.settingsVersionLabel,
    help: (AppLocalizations l10n) => l10n.settingsVersionHelp,
    keywords: (AppLocalizations l10n) => l10n.settingsKeywordsVersion,
    build: (SettingsPageData data) => const VersionRow(),
  ),
  SettingEntry(
    id: SettingIds.daemonRestart,
    category: SettingsCategory.daemon,
    title: (AppLocalizations l10n) => l10n.settingsRestartLabel,
    help: (AppLocalizations l10n) => l10n.settingsRestartHelp,
    keywords: (AppLocalizations l10n) => l10n.settingsKeywordsRestart,
    drawnWhen: (SettingsPageData data) => data.daemonRestart != null,
    build: (SettingsPageData data) => RestartRow(control: data.daemonRestart),
  ),
  SettingEntry(
    id: FormKeys.updateCheck,
    category: SettingsCategory.daemon,
    title: (AppLocalizations l10n) => l10n.settingsUpdateCheckLabel,
    help: (AppLocalizations l10n) => l10n.settingsUpdateCheckHelpShort,
    details: (AppLocalizations l10n) => l10n.settingsUpdateCheckHelp,
    keywords: (AppLocalizations l10n) => l10n.settingsKeywordsUpdate,
    build: (SettingsPageData data) => UpdateRow(config: data.config),
  ),
  SettingEntry(
    id: SettingIds.configFile,
    category: SettingsCategory.daemon,
    title: (AppLocalizations l10n) => l10n.settingsFileLabel,
    help: (AppLocalizations l10n) => l10n.settingsFileHelp,
    keywords: (AppLocalizations l10n) => l10n.settingsKeywordsConfigFile,
    build: (SettingsPageData data) => ConfigFileRow(config: data.config),
  ),
];

/// The entry with [id]. Throws for an id the register does not know: a row
/// without an entry would be a row the search cannot find.
SettingEntry settingEntry(String id) =>
    settingsRegistry.firstWhere((SettingEntry entry) => entry.id == id);

/// The entry with [id], or null.
SettingEntry? settingEntryOrNull(String id) {
  for (final SettingEntry entry in settingsRegistry) {
    if (entry.id == id) {
      return entry;
    }
  }
  return null;
}

/// The entries of [category] a daemon described by [info] offers, in page
/// order.
List<SettingEntry> settingsOf(SettingsCategory category, DaemonInfo? info) =>
    <SettingEntry>[
      for (final SettingEntry entry in settingsRegistry)
        if (entry.category == category && entry.shownFor(info)) entry,
    ];
