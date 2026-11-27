/// The way from a finding of the first-start screen into the guided agent
/// setup (GitHub #111): a finding that a step solves gets a button that
/// opens that step, instead of standing there as text alone.
library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../../core/domain/domain.dart';
import '../../../../core/ui/ui.dart';
import '../../../../l10n/l10n.dart';
import '../../providers/agent_setup.dart';
import '../../setup/setup_steps.dart';

/// The button.
class SetupLink extends ConsumerWidget {
  /// Creates the button for [findings] of the agent [agent] in the folder
  /// [path] (and the project [projectId], when the register knows it).
  const SetupLink({
    required this.path,
    required this.agent,
    required this.agentName,
    required this.findings,
    this.projectId = '',
    super.key,
  });

  /// The folder.
  final String path;

  /// The project, empty for a new folder.
  final String projectId;

  /// The adapter id of the agent.
  final String agent;

  /// The name of the agent to show.
  final String agentName;

  /// The findings that lead into the setup.
  final List<Diagnostic> findings;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    SetupStepId? step;
    for (final Diagnostic d in findings) {
      step ??= setupStepFor(d.code);
    }
    return HButton(
      key: Key('agent-setup-link-$agent'),
      size: HButtonSize.sm,
      onPressed: () => unawaited(
        ref
            .read(agentSetupProvider.notifier)
            .open(
              AgentSetupTarget(
                path: path,
                projectId: projectId,
                agent: agent,
                findings: findings,
              ),
              step: step,
            ),
      ),
      child: Text(context.l10n.startAgentSetup(agentName)),
    );
  }
}
