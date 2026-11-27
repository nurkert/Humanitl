/// What every step of the guided agent setup reads (GitHub #111), in one
/// value, so a step takes one argument and not eight.
library;

import 'package:flutter/widgets.dart';

import '../../../../core/domain/domain.dart';
import '../../providers/agent_setup.dart';
import '../../setup/setup_steps.dart';

/// The setup as one step sees it.
final class SetupView {
  /// Creates the view.
  const SetupView({
    required this.setup,
    required this.step,
    required this.depot,
    required this.config,
    required this.catalog,
    this.onBack,
    this.onNext,
    this.onOpenSession,
  });

  /// The state of the setup.
  final AgentSetup setup;

  /// The step on screen.
  final SetupStep step;

  /// The depot entry of the agent; null while `Agents` did not answer, or
  /// for an agent the depot does not know.
  final AgentDepot? depot;

  /// The configuration as `GetConfig` answered.
  final ConfigValues? config;

  /// The settings catalog of the daemon.
  final SettingsCatalog? catalog;

  /// The step before; null on the first.
  final VoidCallback? onBack;

  /// The step after; null on the last.
  final VoidCallback? onNext;

  /// Shows the terminal of a running maintenance run.
  final VoidCallback? onOpenSession;

  /// The agent's adapter id.
  String get agent => setup.agent;

  /// The name of the agent to show.
  String get agentName {
    final String shown = depot?.displayName ?? '';
    if (shown.isNotEmpty) {
      return shown;
    }
    final String label = setup.proposal?.field('agent')?.chosen?.label ?? '';
    return label.isNotEmpty ? label : agent;
  }
}
