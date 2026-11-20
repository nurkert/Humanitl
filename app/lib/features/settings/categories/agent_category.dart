/// The category Agent: which coding agent a new session starts (HUM-274,
/// HUM-404).
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/cloud_inference_provider.dart';
import '../../../core/ui/cloud_inference_tag.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../widgets/setting_controls.dart';
import '../widgets/setting_row.dart';

/// `agent.adapter`. Welche Ids es gibt, prüft der Daemon; die Zeile bietet
/// die seines Registers.
///
/// Geht die Inferenz des gewählten Agenten an einen Anbieter außerhalb
/// dieses Rechners, steht darunter einmal der Hinweis darauf (HUM-516); ob
/// das so ist, sagt der Plan des Daemons.
class AgentRow extends ConsumerWidget {
  /// Creates the row for [config].
  const AgentRow({required this.config, super.key});

  /// The configuration as `GetConfig` answered.
  final ConfigValues config;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final CloudInference? cloud = ref
        .watch(plannedCloudInferenceProvider)
        .value;
    final Widget row = _row(context);
    if (cloud == null) {
      return row;
    }
    final HTokens tokens = HTheme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        row,
        Padding(
          padding: EdgeInsets.fromLTRB(
            SettingRowGrid.inset,
            tokens.spacing.x1,
            SettingRowGrid.inset,
            tokens.spacing.x3,
          ),
          child: CloudInferenceNote(cloud: cloud),
        ),
      ],
    );
  }

  Widget _row(BuildContext context) {
    final AppLocalizations l10n = context.l10n;
    return SettingRow(
      settingId: FormKeys.agentAdapter,
      refusalKey: const Key('settings-agent-refusal'),
      child: HSegmented<String>(
        key: const Key('settings-agent'),
        options: <HSegmentOption<String>>[
          HSegmentOption<String>(
            value: 'opencode',
            label: l10n.settingsAgentOpenCode,
          ),
          HSegmentOption<String>(value: 'pi', label: l10n.settingsAgentPi),
          HSegmentOption<String>(
            value: 'claude',
            label: l10n.settingsAgentClaude,
          ),
        ],
        selected: config.agentAdapter,
        onSelect: (String adapter) =>
            writeSetting(context, FormKeys.agentAdapter, adapter),
      ),
    );
  }
}
