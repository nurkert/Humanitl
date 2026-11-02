/// The category Agent: which coding agent a new session starts (HUM-274,
/// HUM-404).
library;

import 'package:flutter/widgets.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../widgets/setting_controls.dart';
import '../widgets/setting_row.dart';

/// `agent.adapter`. Welche Ids es gibt, prüft der Daemon; die Zeile bietet
/// die seines Registers.
class AgentRow extends StatelessWidget {
  /// Creates the row for [config].
  const AgentRow({required this.config, super.key});

  /// The configuration as `GetConfig` answered.
  final ConfigValues config;

  @override
  Widget build(BuildContext context) {
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
        ],
        selected: config.agentAdapter,
        onSelect: (String adapter) =>
            writeSetting(context, FormKeys.agentAdapter, adapter),
      ),
    );
  }
}
