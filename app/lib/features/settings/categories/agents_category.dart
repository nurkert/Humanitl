/// The category Agents: the agent depot, one card per known agent
/// (HUM-521, ADR-0023 point 8).
///
/// Everything goes through `Agents` and the keys `agents.<id>.*`; the app is
/// a thin client (ADR-018). Which button is open comes from
/// `AgentDepot.actions`, never from a rule of the app.
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/daemon_client.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/agents_provider.dart';
import '../registry/settings_registry.dart';
import '../settings_scope.dart';
import '../widgets/agents/agent_card.dart';
import '../widgets/setting_row.dart';

/// The section: the catalog's sentence about the depot, then the cards.
class AgentsSection extends ConsumerWidget {
  /// Creates the section; [onOpenSession] shows the terminal of a run.
  const AgentsSection({required this.config, this.onOpenSession, super.key});

  /// The configuration as `GetConfig` answered; it holds
  /// `agents.<id>.credential_writeback`.
  final ConfigValues config;

  /// See [SettingsPageData.onOpenSession].
  final VoidCallback? onOpenSession;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final SettingEntry? about = SettingsView.of(
      context,
      ref,
    ).entry(SettingIds.agentsDepot);
    final String title = about?.title ?? '';
    final String help = about?.help ?? '';
    final AsyncValue<AgentsStatus> state = ref.watch(agentsProvider);
    final AgentsStatus? status = state.value;
    final TextStyle quiet = tokens.typography.ui12.tinted(tokens.colors.fg1);
    // Ein Sprung aus der Suche landet auf dem Kopf des Abschnitts, wie bei
    // den Servern der Modelle (HUM-405).
    final SettingsController? scope = SettingsScope.maybeOf(context);
    final Widget section = Padding(
      key: const Key('settings-agents'),
      padding: EdgeInsets.symmetric(
        horizontal: SettingRowGrid.inset,
        vertical: tokens.spacing.x3,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          // Name und Satz des Katalogs, wie über den Servern der Modelle.
          if (title.isNotEmpty)
            Text(
              title,
              style: tokens.typography.ui13.medium.tinted(tokens.colors.fg0),
            ),
          if (help.isNotEmpty) Text(help, style: quiet),
          if (title.isNotEmpty || help.isNotEmpty)
            SizedBox(height: tokens.spacing.x3),
          // Ein Fehler steht vor einem älteren Stand: Sonst zeigte die Karte
          // einen Lauf weiter, von dem der Daemon nichts mehr sagt.
          if (state.hasError && !state.isLoading)
            _LoadFailed(error: state.error)
          else if (status != null)
            for (int i = 0; i < status.agents.length; i++) ...<Widget>[
              if (i > 0) SizedBox(height: tokens.spacing.x3),
              AgentCard(
                key: ValueKey<String>('agent-card-${status.agents[i].agent}'),
                entry: status.agents[i],
                config: config,
                onOpenSession: onOpenSession,
              ),
            ]
          else
            Text(
              l10n.settingsAgentsLoading,
              key: const Key('settings-agents-loading'),
              style: tokens.typography.ui12.tinted(tokens.colors.fg2),
            ),
        ],
      ),
    );
    return scope == null
        ? section
        : KeyedSubtree(
            key: scope.rowKey(SettingIds.agentsDepot),
            child: Focus(
              focusNode: scope.anchor(SettingIds.agentsDepot),
              child: section,
            ),
          );
  }
}

/// `Agents(status)` failed: the daemon's sentence and the way to ask again.
class _LoadFailed extends ConsumerWidget {
  const _LoadFailed({required this.error});

  final Object? error;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final Object? error = this.error;
    final String cause = error is DaemonException
        ? DiagnosticL10n.resolve(error.diagnostic, l10n).cause
        : '';
    return Column(
      key: const Key('settings-agents-failed'),
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Text(
          l10n.settingsAgentsLoadFailed,
          style: tokens.typography.ui13.medium.tinted(tokens.colors.fg0),
        ),
        if (cause.isNotEmpty) ...<Widget>[
          SizedBox(height: tokens.spacing.x1),
          Text(
            cause,
            style: tokens.typography.ui12.tinted(tokens.stateText.error),
          ),
        ],
        SizedBox(height: tokens.spacing.x2),
        HButton(
          key: const Key('settings-agents-retry'),
          variant: HButtonVariant.secondary,
          onPressed: () => ref.invalidate(agentsProvider),
          child: Text(l10n.settingsAgentsRetry),
        ),
      ],
    );
  }
}
