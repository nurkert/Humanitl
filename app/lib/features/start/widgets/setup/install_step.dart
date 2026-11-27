/// Step 2 of the guided agent setup (GitHub #111): install and sign in a
/// depot agent.
///
/// Version and sign-in as the depot says, then the one next action: install
/// while nothing is installed, sign in while no login is stored. Both are
/// maintenance runs of `Agents`, the same as in the settings and on the
/// command line, and show in the run panel the settings use, with the URLs
/// of a login, the field for a pasted callback and the way to the terminal
/// of the run. The run lives in the daemon: closing the window does not end
/// it, and the step shows it again when the setup opens.
///
/// For a program on this machine the step has nothing to do: installing
/// and signing in happen there.
library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../../core/domain/domain.dart';
import '../../../../core/ui/agents/agent_inset.dart';
import '../../../../core/ui/agents/agent_run_panel.dart';
import '../../../../core/ui/ui.dart';
import '../../../../l10n/l10n.dart';
import '../../providers/agent_setup.dart';
import '../../setup/setup_steps.dart';
import 'setup_step_frame.dart';
import 'setup_text.dart';
import 'setup_view.dart';

/// The step install and sign in.
class InstallStep extends ConsumerWidget {
  /// Creates the step over [view].
  const InstallStep({required this.view, super.key});

  /// The setup as this step sees it.
  final SetupView view;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final AppLocalizations l10n = context.l10n;
    final AgentDepot? depot = view.depot;
    final String name = view.agentName;
    final bool fromDepot = depot?.source == AgentSource.depot;
    final bool installRunning = depot?.maintenance?.running ?? false;
    return SetupStepFrame(
      step: view.step,
      // Installieren ist die eine Hauptaktion des Schritts; die Anmeldung
      // ist ein Angebot und bleibt ruhig.
      // Ein Konfigurationslauf, der auf Bestätigung wartet, zeigt im Panel
      // seinen eigenen Hauptknopf ("Übernehmen").
      bodyPrimary:
          (fromDepot &&
              (depot?.actions.install ?? false) &&
              !installRunning &&
              !view.setup.busy) ||
          depot?.maintenance?.state == AgentRunState.confirm,
      title: l10n.agentSetupInstallTitle(name),
      lead: fromDepot
          ? l10n.agentSetupInstallLead
          : l10n.agentSetupInstallHostLead(name),
      commands: setupCommands(SetupStepId.install, view.agent),
      refusal: view.setup.refusal,
      onBack: view.onBack,
      onNext: view.onNext,
      body: depot == null || !fromDepot
          ? const SizedBox.shrink()
          : _DepotInstall(view: view, depot: depot),
    );
  }
}

class _DepotInstall extends ConsumerWidget {
  const _DepotInstall({required this.view, required this.depot});

  final SetupView view;
  final AgentDepot depot;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final AgentSetupNotifier setup = ref.read(agentSetupProvider.notifier);
    final bool busy = view.setup.busy;
    final AgentMaintenance? run = depot.maintenance;
    final bool ownRun =
        run != null &&
        (run.task == AgentRunTask.install || run.task == AgentRunTask.login);
    final bool running = ownRun && run.running;
    final bool installNext = depot.actions.install;
    final bool loginNext =
        !installNext && depot.actions.login && !depot.loggedIn;
    if (!depot.depotSupported) {
      return Text(
        l10n.agentSetupInstallNoRecipe(view.agentName),
        style: tokens.typography.ui13.tinted(tokens.colors.fg1),
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        SetupFact(
          label: l10n.settingsAgentsVersion,
          value: Text(
            depot.installed ? depot.version : l10n.settingsAgentsNotInstalled,
            key: const Key('agent-setup-install-version'),
            style: depot.installed
                ? tokens.typography.mono12.tinted(tokens.colors.fg0)
                : tokens.typography.ui13.tinted(tokens.colors.fg1),
          ),
        ),
        SetupFact(
          label: l10n.settingsAgentsLogin,
          value: Text(
            depot.loggedIn
                ? l10n.settingsAgentsLoggedIn
                : l10n.settingsAgentsLoggedOut,
            key: const Key('agent-setup-install-login'),
            style: tokens.typography.ui13.tinted(
              depot.loggedIn ? tokens.colors.fg0 : tokens.colors.fg1,
            ),
          ),
        ),
        for (final Diagnostic found in depot.diagnostics) ...<Widget>[
          SizedBox(height: tokens.spacing.x2),
          AgentFinding(diagnostic: found),
        ],
        if (ownRun) ...<Widget>[
          SizedBox(height: tokens.spacing.x3),
          AgentRunPanel(
            entry: depot,
            run: run,
            busy: busy,
            onSend: (AgentsOp op) async {
              await setup.maintain(op);
              return ref.read(agentSetupProvider)?.refusal == null;
            },
            onOpenSession: run.sandboxId.isEmpty ? null : view.onOpenSession,
          ),
        ],
        if (!running && (installNext || loginNext)) ...<Widget>[
          SizedBox(height: tokens.spacing.x4),
          // Der Knopf vorn, der Satz zu ihm darunter: neben dem Knopf
          // bräche er in einer schmalen Spalte in viele Zeilen.
          Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              HButton(
                key: Key(
                  installNext ? 'agent-setup-install' : 'agent-setup-login',
                ),
                variant: installNext
                    ? HButtonVariant.primary
                    : HButtonVariant.secondary,
                onPressed: busy
                    ? null
                    : () => unawaited(
                        setup.maintain(
                          installNext
                              ? AgentsOp.install(depot.agent)
                              : AgentsOp.login(depot.agent),
                          clears: installCodes,
                        ),
                      ),
                child: Text(
                  installNext
                      ? l10n.agentSetupInstallInstall(view.agentName)
                      : l10n.agentSetupInstallSignIn,
                ),
              ),
              SizedBox(height: tokens.spacing.x2),
              Text(
                installNext
                    ? l10n.agentSetupInstallInstallHint
                    : l10n.agentSetupInstallSignInHint,
                style: tokens.typography.ui12.tinted(tokens.colors.fg2),
              ),
            ],
          ),
        ],
        if (running) ...<Widget>[
          SizedBox(height: tokens.spacing.x2),
          Text(
            l10n.agentSetupRunSurvives,
            key: const Key('agent-setup-run-survives'),
            style: tokens.typography.ui12.tinted(tokens.colors.fg2),
          ),
        ],
      ],
    );
  }
}
