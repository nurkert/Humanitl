/// Words and colours of the guided agent setup that depend on a value
/// (GitHub #111).
library;

import 'dart:ui' show Color;

import '../../../../core/ui/ui.dart';
import '../../../../l10n/l10n.dart';
import '../../setup/setup_steps.dart';

/// The name of [step].
String setupStepName(AppLocalizations l10n, SetupStepId step) => switch (step) {
  SetupStepId.source => l10n.agentSetupStepSource,
  SetupStepId.install => l10n.agentSetupStepInstall,
  SetupStepId.inference => l10n.agentSetupStepInference,
  SetupStepId.update => l10n.agentSetupStepUpdate,
  SetupStepId.ready => l10n.agentSetupStepReady,
};

/// The state of a step in words.
String setupStateName(AppLocalizations l10n, SetupStepState state) =>
    switch (state) {
      SetupStepState.done => l10n.agentSetupStateDone,
      SetupStepState.action => l10n.startAgentNeedsSetup,
      SetupStepState.running => l10n.agentSetupStateRunning,
      SetupStepState.blocked => l10n.agentSetupStateBlocked,
    };

/// The colour of the words of a state: neutral, the step that needs a hand
/// in the primary text colour. The state colour stays on the node alone.
Color setupStateTextColor(HTokens tokens, SetupStepState state) =>
    switch (state) {
      SetupStepState.done => tokens.colors.fg1,
      SetupStepState.action => tokens.colors.fg0,
      SetupStepState.running => tokens.colors.fg1,
      SetupStepState.blocked => tokens.colors.fg2,
    };

/// The commands that do what [step] does for the agent [agent] (CLI parity,
/// ADR-018); [own] adds the key of the agent's own providers.
List<String> setupCommands(
  SetupStepId step,
  String agent, {
  bool own = false,
}) => switch (step) {
  SetupStepId.source => <String>['humanitl agents source $agent host|depot'],
  SetupStepId.install => <String>[
    'humanitl agents install $agent',
    'humanitl agents login $agent',
  ],
  SetupStepId.inference => <String>[
    'humanitl llm server use NAME',
    if (own) 'humanitl config set agents.$agent.inference own',
  ],
  SetupStepId.update => <String>[
    'humanitl agents update $agent [--version VERSION]',
  ],
  SetupStepId.ready => <String>['humanitl run'],
};
