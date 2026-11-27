/// Step 5 of the guided agent setup (GitHub #111): what the session will
/// be, and the start.
///
/// Agent, version, source, where the inference goes and the isolation, each
/// as the daemon answered it; the start is the same start as on the row of
/// the project. While a step before is open, the start waits.
library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../../core/domain/domain.dart';
import '../../../../core/ui/ui.dart';
import '../../../../l10n/l10n.dart';
import '../../providers/agent_setup.dart';
import '../../providers/projects.dart';
import '../../setup/setup_facts.dart';
import '../../setup/setup_steps.dart';
import 'setup_step_frame.dart';
import 'setup_text.dart';
import 'setup_view.dart';

/// The summary.
class ReadyStep extends ConsumerWidget {
  /// Creates the step over [view].
  const ReadyStep({required this.view, super.key});

  /// The setup as this step sees it.
  final SetupView view;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final AgentSetup setup = view.setup;
    final AgentDepot? depot = view.depot;
    final bool fromDepot = depot?.source == AgentSource.depot;
    final StartProposal? proposal = setup.proposal;
    final bool ready = view.step.done;
    final StartLaunch launch = ref.watch(startLaunchProvider);
    final TextStyle value = tokens.typography.ui13.tinted(tokens.colors.fg0);
    return SetupStepFrame(
      step: view.step,
      bodyPrimary: true,
      title: l10n.agentSetupReadyTitle,
      lead: ready ? l10n.agentSetupReadyLead : l10n.agentSetupReadyNotYet,
      commands: setupCommands(SetupStepId.ready, view.agent),
      refusal: setup.refusal,
      onBack: view.onBack,
      body: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          SetupFact(
            label: l10n.agentSetupFactAgent,
            value: Text(view.agentName, style: value),
          ),
          SetupFact(
            label: l10n.agentSetupFactVersion,
            value: Text(
              fromDepot && depot != null && depot.installed
                  ? depot.version
                  : l10n.agentSetupFactHostVersion,
              key: const Key('agent-setup-ready-version'),
              style: fromDepot
                  ? tokens.typography.mono12.tinted(tokens.colors.fg0)
                  : value,
            ),
          ),
          SetupFact(
            label: l10n.agentSetupFactSource,
            value: Text(
              fromDepot ? l10n.agentSetupFactDepot : l10n.agentSetupFactHost,
              style: value,
            ),
          ),
          SetupFact(
            label: l10n.agentSetupFactInference,
            value: Text(
              _inference(l10n, view),
              key: const Key('agent-setup-ready-inference'),
              style: value,
            ),
          ),
          SetupFact(
            label: l10n.agentSetupFactIsolation,
            value: Text(
              _isolation(proposal),
              key: const Key('agent-setup-ready-isolation'),
              style: value,
            ),
          ),
          SizedBox(height: tokens.spacing.x4),
          Row(
            children: <Widget>[
              HButton(
                key: const Key('agent-setup-start'),
                variant: HButtonVariant.primary,
                size: HButtonSize.md,
                onPressed:
                    ready && setup.target.known && launch.busyPath == null
                    ? () => unawaited(_start(ref, setup.target))
                    : null,
                child: Text(
                  setup.projectName.isEmpty
                      ? l10n.startStart
                      : l10n.agentSetupStart(setup.projectName),
                ),
              ),
              if (!setup.target.known) ...<Widget>[
                SizedBox(width: tokens.spacing.x3),
                Expanded(
                  child: Text(
                    l10n.agentSetupStartFromChoice,
                    style: tokens.typography.ui12.tinted(tokens.colors.fg2),
                  ),
                ),
              ],
            ],
          ),
        ],
      ),
    );
  }

  /// Where the inference goes, in words: the agent's own providers, or the
  /// server the proposal chose for the project.
  static String _inference(AppLocalizations l10n, SetupView view) {
    if (ownInferenceOffered(view.catalog, view.agent) &&
        inferenceOf(view.config, view.agent) == inferenceOwn) {
      return l10n.agentSetupFactOwn;
    }
    final String server =
        view.setup.proposal?.field('llm_server')?.chosen?.label ??
        view.setup.servers?.activeServer?.name ??
        '';
    return server.isEmpty ? l10n.agentSetupFactNoServer : server;
  }

  /// The isolation level the proposal chose, in the daemon's words.
  static String _isolation(StartProposal? proposal) =>
      proposal?.field('isolation')?.chosen?.label ?? '';

  static Future<void> _start(WidgetRef ref, AgentSetupTarget target) async {
    ref.read(agentSetupProvider.notifier).close();
    await ref
        .read(startLaunchProvider.notifier)
        .startKnown(target.projectId, target.path);
  }
}
