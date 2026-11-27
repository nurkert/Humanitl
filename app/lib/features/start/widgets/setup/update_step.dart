/// Step 4 of the guided agent setup (GitHub #111): keep a depot agent up
/// to date.
///
/// The installed version with its signature, the versions the depot holds,
/// one button that updates to the vendor's current version and one per
/// held version to go back. Each is `Agents(update)`, the same as
/// `humanitl agents update`: a maintenance run that keeps the login and the
/// configuration copy and takes the new version over only when every check
/// passes, so a failed update leaves the installed one in place. The run
/// shows in the run panel of the settings.
library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../../core/domain/domain.dart';
import '../../../../core/ui/agents/agent_run_panel.dart';
import '../../../../core/ui/ui.dart';
import '../../../../l10n/l10n.dart';
import '../../providers/agent_setup.dart';
import '../../setup/setup_steps.dart';
import 'setup_step_frame.dart';
import 'setup_text.dart';
import 'setup_view.dart';

/// How many held versions are offered to go back to.
const int _backChoices = 3;

/// The update step.
class UpdateStep extends ConsumerWidget {
  /// Creates the step over [view].
  const UpdateStep({required this.view, super.key});

  /// The setup as this step sees it.
  final SetupView view;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final AppLocalizations l10n = context.l10n;
    final AgentDepot? depot = view.depot;
    final String name = view.agentName;
    final bool fromDepot = depot?.source == AgentSource.depot;
    return SetupStepFrame(
      step: view.step,
      bodyPrimary:
          (view.step.state == SetupStepState.action &&
              (depot?.actions.update ?? false) &&
              !view.setup.busy) ||
          depot?.maintenance?.state == AgentRunState.confirm,
      title: l10n.agentSetupUpdateTitle(name),
      lead: fromDepot
          ? l10n.agentSetupUpdateLead
          : l10n.agentSetupUpdateHostLead(name),
      commands: setupCommands(SetupStepId.update, view.agent),
      refusal: view.setup.refusal,
      onBack: view.onBack,
      onNext: view.onNext,
      body: depot != null && fromDepot && depot.installed
          ? _DepotUpdate(view: view, depot: depot)
          : const SizedBox.shrink(),
    );
  }
}

class _DepotUpdate extends ConsumerWidget {
  const _DepotUpdate({required this.view, required this.depot});

  final SetupView view;
  final AgentDepot depot;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final AgentSetupNotifier setup = ref.read(agentSetupProvider.notifier);
    final AgentMaintenance? run = depot.maintenance;
    final bool ownRun = run != null && run.task == AgentRunTask.update;
    final bool open = depot.actions.update && !view.setup.busy;
    final List<String> older = depot.olderVersions
        .take(_backChoices)
        .toList(growable: false);
    final bool verified = depot.signatureVerified;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        SetupFact(
          label: l10n.agentSetupUpdateInstalled,
          value: Row(
            children: <Widget>[
              Text(
                depot.version,
                key: const Key('agent-setup-update-version'),
                style: tokens.typography.mono12.tinted(tokens.colors.fg0),
              ),
              SizedBox(width: tokens.spacing.x2),
              HBadge(
                text: verified
                    ? l10n.settingsAgentsSignatureVerified
                    : l10n.settingsAgentsSignatureNone,
                color: verified ? tokens.state.allowed : tokens.colors.warning,
              ),
            ],
          ),
        ),
        SetupFact(
          label: l10n.agentSetupUpdateHeld,
          value: Text(
            older.isEmpty ? l10n.agentSetupUpdateNone : older.join(', '),
            key: const Key('agent-setup-update-held'),
            style: older.isEmpty
                ? tokens.typography.ui13.tinted(tokens.colors.fg1)
                : tokens.typography.mono12.tinted(tokens.colors.fg1),
          ),
        ),
        SizedBox(height: tokens.spacing.x2),
        Text(
          l10n.agentSetupUpdateCheckNote,
          style: tokens.typography.ui12.tinted(tokens.colors.fg2),
        ),
        if (ownRun) ...<Widget>[
          SizedBox(height: tokens.spacing.x3),
          AgentRunPanel(
            entry: depot,
            run: run,
            busy: view.setup.busy,
            onSend: (AgentsOp op) async {
              await setup.maintain(op);
              return ref.read(agentSetupProvider)?.refusal == null;
            },
            onOpenSession: run.sandboxId.isEmpty ? null : view.onOpenSession,
          ),
        ],
        SizedBox(height: tokens.spacing.x4),
        Wrap(
          spacing: tokens.spacing.x2,
          runSpacing: tokens.spacing.x2,
          children: <Widget>[
            HButton(
              key: const Key('agent-setup-update'),
              variant: view.step.state == SetupStepState.action
                  ? HButtonVariant.primary
                  : HButtonVariant.secondary,
              onPressed: open
                  ? () =>
                        unawaited(setup.maintain(AgentsOp.update(depot.agent)))
                  : null,
              child: Text(l10n.agentSetupUpdateToCurrent),
            ),
            for (final String version in older)
              HButton(
                key: Key('agent-setup-update-back-$version'),
                variant: HButtonVariant.ghost,
                onPressed: open
                    ? () => unawaited(
                        setup.maintain(
                          AgentsOp.update(depot.agent, version: version),
                        ),
                      )
                    : null,
                child: Text(l10n.agentSetupUpdateBackTo(version)),
              ),
          ],
        ),
      ],
    );
  }
}
