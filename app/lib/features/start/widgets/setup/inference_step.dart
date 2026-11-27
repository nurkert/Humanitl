/// Step 3 of the guided agent setup (GitHub #111): where the inference
/// goes.
///
/// Two choices, kept apart: only my model server, the default, and, for an
/// agent whose adapter declares them, the agent's own providers. The first
/// opens the saved servers ([InferenceServers]); the second the cloud
/// notice in plain words with its own confirmation, never ticked in
/// advance. Own providers are `agents.<id>.inference = own`, a key with no
/// project level, so the step says it applies to every project. Under
/// `llm.cloud = block` (`LLM_014`) or a company lock the second choice
/// stays visible and cannot be taken.
library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../../core/domain/domain.dart';
import '../../../../core/ui/agents/agent_inset.dart';
import '../../../../core/ui/ui.dart';
import '../../../../l10n/l10n.dart';
import '../../providers/agent_setup.dart';
import '../../setup/setup_facts.dart';
import '../../setup/setup_steps.dart';
import 'inference_servers.dart';
import 'setup_choice.dart';
import 'setup_step_frame.dart';
import 'setup_text.dart';
import 'setup_view.dart';

/// The inference step.
class InferenceStep extends ConsumerStatefulWidget {
  /// Creates the step over [view].
  const InferenceStep({required this.view, super.key});

  /// The setup as this step sees it.
  final SetupView view;

  @override
  ConsumerState<InferenceStep> createState() => _InferenceStepState();
}

class _InferenceStepState extends ConsumerState<InferenceStep> {
  /// The choice the person picked; null shows the one in use, which is the
  /// model server unless `own` is already set.
  String? _picked;

  /// The saved server the person picked; null shows [initialServer].
  String? _server;

  /// Where the server is saved; null takes this project when the register
  /// knows it.
  ServerLevel? _level;

  /// The confirmation of the cloud notice; never ticked in advance, and
  /// cleared after every attempt.
  bool _understood = false;

  Future<void> _useOwn() async {
    await ref.read(agentSetupProvider.notifier).useOwn();
    if (mounted) {
      setState(() => _understood = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final SetupView view = widget.view;
    final bool offered = ownInferenceOffered(view.catalog, view.agent);
    final String current = offered
        ? inferenceOf(view.config, view.agent)
        : inferenceModelServer;
    final String picked = _picked ?? current;
    final String name = view.agentName;
    final Diagnostic? cloudLock = _cloudLock(view);
    final SettingsLock? keyLock = inferenceEntry(
      view.catalog,
      view.agent,
    )?.lock;
    final bool ownOpen = cloudLock == null && keyLock == null;
    final AgentSetup setup = view.setup;
    final String server = _server ?? initialServer(setup);
    final ServerLevel level =
        _level ?? (setup.target.known ? ServerLevel.project : ServerLevel.all);
    final bool ownCurrent = current == inferenceOwn;
    // Die eine Hauptaktion des Körpers, wenn sie offen ist: dann tritt
    // „Weiter" zurück, und es steht nie mehr als ein Hauptknopf da.
    final bool bodyPrimary = picked == inferenceOwn
        ? !ownCurrent && _understood && !setup.busy
        : serverUsable(setup, server);
    return SetupStepFrame(
      bodyPrimary: bodyPrimary,
      step: view.step,
      title: l10n.agentSetupInferenceTitle,
      lead: l10n.agentSetupInferenceLead,
      variant: picked == inferenceOwn,
      commands: setupCommands(SetupStepId.inference, view.agent, own: offered),
      refusal: view.setup.refusal,
      onBack: view.onBack,
      onNext: view.onNext,
      body: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          SetupChoice(
            key: const Key('agent-setup-inference-server'),
            title: l10n.agentSetupInferenceServer,
            help: l10n.agentSetupInferenceServerHelp,
            selected: picked == inferenceModelServer,
            onSelect: () => setState(() => _picked = inferenceModelServer),
          ),
          if (offered) ...<Widget>[
            SizedBox(height: tokens.spacing.x2),
            SetupChoice(
              key: const Key('agent-setup-inference-own'),
              title: l10n.agentSetupInferenceOwn(name),
              help: l10n.agentSetupInferenceOwnHelp,
              selected: picked == inferenceOwn,
              onSelect: ownOpen
                  ? () => setState(() => _picked = inferenceOwn)
                  : null,
              reason: ownOpen
                  ? null
                  : SetupNote(
                      key: const Key('agent-setup-inference-own-locked'),
                      glyph: HGlyph.lock,
                      color: tokens.colors.fg2,
                      text: keyLock != null
                          ? l10n.settingsAgentsLocked(keyLock.lockedBy)
                          : l10n.agentSetupCloudLocked,
                    ),
            ),
          ],
          SizedBox(height: tokens.spacing.x4),
          // Was zur gewählten Wahl gehört, steht unter ihrem Text, nicht
          // unter ihrer Marke: so liest es sich als Teil der Wahl.
          Padding(
            padding: EdgeInsets.only(left: setupChoiceIndent(tokens)),
            child: picked == inferenceOwn
                ? _OwnProviders(
                    agentName: name,
                    current: ownCurrent,
                    busy: setup.busy,
                    understood: _understood,
                    onUnderstood: (bool on) => setState(() => _understood = on),
                    onUse: () => unawaited(_useOwn()),
                  )
                : InferenceServers(
                    view: view,
                    chosen: server,
                    onChosen: (String name) => setState(() => _server = name),
                    level: level,
                    onLevel: (ServerLevel next) =>
                        setState(() => _level = next),
                  ),
          ),
        ],
      ),
    );
  }

  /// The finding of the lock on cloud inference, from the start or the
  /// proposal.
  static Diagnostic? _cloudLock(SetupView view) {
    for (final Diagnostic d in view.setup.findings) {
      if (d.code == 'LLM_014') {
        return d;
      }
    }
    return null;
  }
}

/// The cloud notice in plain words, its own confirmation, and the button.
class _OwnProviders extends StatelessWidget {
  const _OwnProviders({
    required this.agentName,
    required this.current,
    required this.busy,
    required this.understood,
    required this.onUnderstood,
    required this.onUse,
  });

  final String agentName;
  final bool current;
  final bool busy;
  final bool understood;
  final ValueChanged<bool> onUnderstood;
  final VoidCallback onUse;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    if (current) {
      return SetupNote(
        key: const Key('agent-setup-inference-own-current'),
        glyph: HGlyph.check,
        color: tokens.stateText.allowed,
        text: l10n.agentSetupInferenceOwnCurrent,
      );
    }
    return AgentInset(
      key: const Key('agent-setup-cloud-notice'),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Row(
            children: <Widget>[
              HGlyphIcon(
                HGlyph.triangleAlert,
                size: 14,
                color: tokens.colors.warning,
              ),
              SizedBox(width: tokens.spacing.x2),
              Expanded(child: AgentInsetTitle(l10n.agentSetupCloudTitle)),
            ],
          ),
          SizedBox(height: tokens.spacing.x2),
          Text(
            l10n.agentSetupCloudBody(agentName),
            style: tokens.typography.ui12.tinted(tokens.colors.fg1),
          ),
          SizedBox(height: tokens.spacing.x1),
          Text(
            l10n.agentSetupCloudAllProjects,
            style: tokens.typography.ui12.tinted(tokens.colors.fg2),
          ),
          SizedBox(height: tokens.spacing.x3),
          HCheckbox(
            key: const Key('agent-setup-cloud-confirm'),
            label: l10n.agentSetupCloudConfirm,
            value: understood,
            enabled: !busy,
            onChanged: onUnderstood,
          ),
          AgentInsetActions(
            children: <Widget>[
              HButton(
                key: const Key('agent-setup-cloud-use'),
                variant: HButtonVariant.primary,
                onPressed: understood && !busy ? onUse : null,
                child: Text(l10n.agentSetupCloudUse),
              ),
            ],
          ),
        ],
      ),
    );
  }
}
