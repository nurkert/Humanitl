/// What the overlay shows, from the settings catalog of the daemon (HUM-404,
/// HUM-534).
///
/// Categories, order, names, help and keywords come from the catalog
/// (`GetConfig` with `include_catalog`); the app keeps no list of settings.
/// This file holds only the builders of rows that need a control of their
/// own, found by the id of their catalog entry ([settingRowSpecs]). An entry
/// without a builder is drawn generically by its control
/// ([GenericSettingRow]); a new entry of the catalog appears without a change
/// here.
///
/// Every value comes from `GetConfig` and goes back through `SetConfig` and
/// `LlmKey`; the rows decide nothing about what a value means (ADR-018).
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/config_providers.dart';
import '../../../l10n/l10n.dart';
import '../categories/agent_category.dart';
import '../categories/agents_category.dart';
import '../categories/daemon_category.dart';
import '../categories/egress_category.dart';
import '../categories/interface_category.dart';
import '../categories/models_category.dart';
import '../categories/network_category.dart';
import '../categories/requests_category.dart';
import '../categories/sandbox_category.dart';
import '../categories/shortcuts_category.dart';
import '../widgets/sandbox_backend_row.dart';
import '../widgets/sandbox_mode_row.dart';
import '../widgets/generic_setting_row.dart';
import '../widgets/update_row.dart';
import 'setting_entry.dart';

export 'setting_entry.dart';

/// The builders of the rows with a control of their own, by catalog id.
///
/// Endpoint, model and key have no builder of their own: the detail view of
/// a server draws them (HUM-405), and a jump there opens the default's.
final Map<String, SettingRowSpec> settingRowSpecs = <String, SettingRowSpec>{
  SettingIds.llmServers: const SettingRowSpec(build: buildModelsCategory),
  FormKeys.llmEndpoint: const SettingRowSpec(),
  FormKeys.llmModels: const SettingRowSpec(),
  FormKeys.llmApiKeyRef: const SettingRowSpec(),
  SettingIds.llmServerUsage: const SettingRowSpec(),
  FormKeys.agentAdapter: SettingRowSpec(
    build: (SettingsPageData data) => AgentRow(config: data.config),
  ),
  // Das Agent-Depot (HUM-521): eine Karte je Agent, alles über `Agents`.
  SettingIds.agentsDepot: SettingRowSpec(
    build: (SettingsPageData data) =>
        AgentsSection(config: data.config, onOpenSession: data.onOpenSession),
  ),
  FormKeys.sandboxWorkDir: SettingRowSpec(
    build: (SettingsPageData data) => WorkDirRow(config: data.config),
  ),
  FormKeys.sandboxMode: SettingRowSpec(
    build: (SettingsPageData data) => SandboxModeRow(config: data.config),
  ),
  // Welche Backends es gibt und welche wählbar sind, sagt `SandboxBackends`
  // (HUM-440); unter macOS steht nur Seatbelt da, ohne Wahl.
  FormKeys.sandboxBackend: SettingRowSpec(
    build: (SettingsPageData data) => SandboxBackendRow(config: data.config),
  ),
  FormKeys.holdTimeoutSecs: SettingRowSpec(
    build: (SettingsPageData data) => TimeoutRow(config: data.config),
  ),
  FormKeys.holdAskMode: SettingRowSpec(
    build: (SettingsPageData data) => AskModeRow(config: data.config),
  ),
  FormKeys.uiLanguage: SettingRowSpec(
    build: (SettingsPageData data) => LanguageRow(config: data.config),
  ),
  FormKeys.uiTheme: SettingRowSpec(
    build: (SettingsPageData data) => ThemeRow(config: data.config),
  ),
  SettingIds.uiScale: SettingRowSpec(
    build: (SettingsPageData data) => const ScaleRow(),
  ),
  FormKeys.uiNotifications: SettingRowSpec(
    build: (SettingsPageData data) => NotificationsRow(config: data.config),
  ),
  FormKeys.tlsTrustSystemStore: SettingRowSpec(
    build: (SettingsPageData data) => TrustStoreRow(config: data.config),
  ),
  // Entscheidungs-Proxy (HUM-511): Adresse, CA und Auflösung über
  // `SetConfig`, die Zugangsdaten und die Prüfung über `Egress`.
  FormKeys.egressProxy: SettingRowSpec(
    build: (SettingsPageData data) => EgressProxyRow(config: data.config),
  ),
  FormKeys.egressProxyCa: SettingRowSpec(
    build: (SettingsPageData data) => EgressCaRow(config: data.config),
  ),
  FormKeys.egressProxyResolve: SettingRowSpec(
    build: (SettingsPageData data) => EgressResolveRow(config: data.config),
  ),
  FormKeys.egressProxyAuthRef: SettingRowSpec(
    build: (SettingsPageData data) => const EgressAuthRow(),
  ),
  SettingIds.egressCheck: SettingRowSpec(
    build: (SettingsPageData data) => const EgressCheckRow(),
  ),
  SettingIds.daemonVersion: SettingRowSpec(
    build: (SettingsPageData data) => const VersionRow(),
  ),
  SettingIds.daemonRestart: SettingRowSpec(
    drawnWhen: (SettingsPageData data) => data.daemonRestart != null,
    build: (SettingsPageData data) => RestartRow(control: data.daemonRestart),
  ),
  FormKeys.updateCheck: SettingRowSpec(
    build: (SettingsPageData data) => UpdateRow(config: data.config),
  ),
  SettingIds.configFile: SettingRowSpec(
    build: (SettingsPageData data) => ConfigFileRow(config: data.config),
  ),
};

/// The entries of the overlay for one catalog, in one language.
///
/// The catalog decides categories, order and texts. The app adds the
/// builders of [settingRowSpecs] and expands the row [SettingIds.shortcuts]
/// into its keyboard shortcuts, which the daemon does not know.
class SettingsView {
  /// The view of [catalog]; [l10n] names the shortcuts.
  SettingsView(this.catalog, AppLocalizations l10n)
    : categories = <SettingsCatalogCategory>[...catalog.categories]
        ..sort(
          (SettingsCatalogCategory a, SettingsCatalogCategory b) =>
              a.order.compareTo(b.order),
        ),
      entries = <SettingEntry>[
        for (final SettingsCatalogCategory category in catalog.categories)
          for (final SettingsCatalogEntry entry in catalog.entriesOf(
            category.id,
          ))
            if (entry.id == SettingIds.shortcuts)
              ...shortcutEntries(l10n, category: category.id)
            else
              SettingEntry.fromCatalog(entry, _specOf(entry)),
      ];

  /// The catalog the view was made of.
  final SettingsCatalog catalog;

  /// The categories in the order of the left column.
  final List<SettingsCatalogCategory> categories;

  /// Every entry, by category in column order and within one in page order.
  final List<SettingEntry> entries;

  /// The view of the catalog the daemon answered in the language of
  /// [context], or of an empty one while none answered.
  static SettingsView of(BuildContext context, WidgetRef ref) {
    final AppLocalizations l10n = context.l10n;
    final SettingsCatalog catalog =
        ref.watch(settingsCatalogProvider(l10n.localeName)).value ??
        SettingsCatalog.empty;
    final SettingsView? last = _last;
    if (last != null &&
        identical(last.catalog, catalog) &&
        identical(_lastL10n, l10n)) {
      return last;
    }
    _lastL10n = l10n;
    return _last = SettingsView(catalog, l10n);
  }

  static SettingsView? _last;
  static AppLocalizations? _lastL10n;

  /// The entry with [id], or null.
  SettingEntry? entry(String id) {
    for (final SettingEntry entry in entries) {
      if (entry.id == id) {
        return entry;
      }
    }
    return null;
  }

  /// The category with [id], or null.
  SettingsCatalogCategory? category(String id) {
    for (final SettingsCatalogCategory category in categories) {
      if (category.id == id) {
        return category;
      }
    }
    return null;
  }

  /// The entries of [category], in page order.
  List<SettingEntry> inCategory(String category) => <SettingEntry>[
    for (final SettingEntry entry in entries)
      if (entry.category == category) entry,
  ];

  /// The builder of [entry]: the app's own, or a generic row for a key the
  /// catalog describes well enough; a special row without a builder of the
  /// app is not drawn.
  static SettingRowSpec? _specOf(SettingsCatalogEntry entry) {
    if (settingRowSpecs[entry.id] case final SettingRowSpec spec) {
      return spec;
    }
    if (entry.control == SettingControl.special ||
        entry.control == SettingControl.secretRef ||
        (entry.client.isNotEmpty && entry.client != 'app')) {
      return null;
    }
    return SettingRowSpec(
      build: (SettingsPageData data) =>
          GenericSettingRow(entry: entry, config: data.config),
    );
  }
}
