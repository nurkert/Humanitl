/// The guided agent setup in the projects tab of the start window
/// (GitHub #111).
///
/// The page of the start window with the project as its subtitle; under it
/// the stepper on the left and the chosen step on its card on the right,
/// both on the same top edge. Narrower than [_sideBySide] the stepper
/// stands above the card. Everything on it is the daemon's answer: the depot
/// state comes from `Agents` (the state the settings share), the rest from
/// [AgentSetup].
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../../core/domain/domain.dart';
import '../../../../core/ipc/agents_provider.dart';
import '../../../../core/ipc/config_providers.dart';
import '../../../../core/ui/session_view.dart';
import '../../../../core/ui/ui.dart';
import '../../../../l10n/l10n.dart';
import '../../providers/agent_setup.dart';
import '../../setup/setup_facts.dart';
import '../../setup/setup_steps.dart';
import '../start_diagnostic.dart';
import '../start_page.dart';
import 'inference_step.dart';
import 'install_step.dart';
import 'ready_step.dart';
import 'setup_stepper.dart';
import 'setup_view.dart';
import 'source_step.dart';
import 'update_step.dart';

/// Width of the stepper column.
const double setupStepperWidth = 216;

/// The narrowest content that keeps stepper and card side by side.
const double _sideBySide = 640;

/// The setup for [state].
class AgentSetupPane extends ConsumerWidget {
  /// Creates the pane.
  const AgentSetupPane({required this.state, super.key});

  /// What the setup shows.
  final AgentSetup state;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final AgentSetupNotifier notifier = ref.read(agentSetupProvider.notifier);
    final AgentDepot? depot = ref.watch(agentsProvider).value?.of(state.agent);
    final ConfigValues? config = ref.watch(configProvider).value;
    final SettingsCatalog? catalog = ref
        .watch(settingsCatalogProvider(l10n.localeName))
        .value;
    final List<SetupStep> steps = setupSteps(
      setupFacts(
        depot: depot,
        proposal: state.proposal,
        servers: state.servers,
        findings: state.findings,
        config: config,
        catalog: catalog,
        agent: state.agent,
      ),
    );
    final SetupStepId selected = state.step ?? firstOpenStep(steps);
    // Einmal geöffnet, bleibt der Schritt stehen, bis jemand einen anderen
    // wählt: Eine fertige Installation springt nicht unter den Händen zur
    // Übersicht, und die Anmeldung bleibt sichtbar.
    if (state.step == null && !state.loading) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (ref.read(agentSetupProvider)?.step == null) {
          notifier.select(selected);
        }
      });
    }
    final int index = SetupStepId.values.indexOf(selected);
    final SetupView view = SetupView(
      setup: state,
      step: steps[index],
      depot: depot,
      config: config,
      catalog: catalog,
      onBack: index == 0
          ? null
          : () => notifier.select(SetupStepId.values[index - 1]),
      onNext: index == SetupStepId.values.length - 1
          ? null
          : () => notifier.select(SetupStepId.values[index + 1]),
      onOpenSession: () => ref
          .read(sessionViewProvider.notifier)
          .attach(
            SessionView(
              projectPath: state.target.path,
              projectId: state.target.projectId,
            ),
          ),
    );
    final String agentName = view.agentName;
    final Widget stepper = SetupStepper(
      steps: steps,
      selected: selected,
      onSelect: notifier.select,
    );
    final Widget panel = state.loading
        ? const HSkeleton(rows: 4)
        : KeyedSubtree(
            key: ValueKey<SetupStepId>(selected),
            child: _step(view),
          );
    return StartPage(
      key: const Key('agent-setup'),
      title: agentName.isEmpty
          ? l10n.agentSetupTitleNoAgent
          : l10n.agentSetupTitle(agentName),
      subtitle: HMiddleEllipsis(
        state.target.path,
        key: const Key('agent-setup-path'),
        style: tokens.typography.mono11.tinted(tokens.colors.fg2),
      ),
      intro: l10n.agentSetupIntro,
      action: HButton(
        key: const Key('agent-setup-close'),
        variant: HButtonVariant.ghost,
        onPressed: notifier.close,
        child: Text(l10n.agentSetupClose),
      ),
      children: <Widget>[
        // Was den Start aufhielt und kein Schritt löst, steht über den
        // Schritten, wie unter der Liste: nichts, was der Daemon sagte,
        // geht verloren.
        for (final Diagnostic finding in uncoveredFindings(
          state.target.findings,
        ))
          Padding(
            padding: EdgeInsets.only(bottom: tokens.spacing.x3),
            child: StartDiagnostic(diagnostic: finding),
          ),
        LayoutBuilder(
          builder: (BuildContext context, BoxConstraints constraints) {
            if (constraints.maxWidth < _sideBySide) {
              return Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  stepper,
                  SizedBox(height: tokens.spacing.x4),
                  panel,
                ],
              );
            }
            return Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                SizedBox(width: setupStepperWidth, child: stepper),
                SizedBox(width: tokens.spacing.x6),
                Expanded(child: panel),
              ],
            );
          },
        ),
      ],
    );
  }

  static Widget _step(SetupView view) => switch (view.step.id) {
    SetupStepId.source => SourceStep(view: view),
    SetupStepId.install => InstallStep(view: view),
    SetupStepId.inference => InferenceStep(view: view),
    SetupStepId.update => UpdateStep(view: view),
    SetupStepId.ready => ReadyStep(view: view),
  };
}
