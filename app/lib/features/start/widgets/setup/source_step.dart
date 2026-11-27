/// Step 1 of the guided agent setup (GitHub #111): where the agent's
/// program comes from.
///
/// Two choices, each with what lies there: the program on this machine,
/// found or not, as the proposal of the daemon says, and the agent depot
/// with its installed version. Choosing the other one shows one sentence on
/// what changes and one button; the button is `Agents(set_source)`, the
/// same call as the settings and `humanitl agents source`. A company lock
/// stands read-only with the policy named.
library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../../core/domain/domain.dart';
import '../../../../core/ui/ui.dart';
import '../../../../core/ui/untranslated_wording.dart';
import '../../../../l10n/l10n.dart';
import '../../providers/agent_setup.dart';
import '../../setup/setup_facts.dart';
import '../../setup/setup_steps.dart';
import 'setup_choice.dart';
import 'setup_step_frame.dart';
import 'setup_text.dart';
import 'setup_view.dart';

/// The source step.
class SourceStep extends ConsumerStatefulWidget {
  /// Creates the step over [view].
  const SourceStep({required this.view, super.key});

  /// The setup as this step sees it.
  final SetupView view;

  @override
  ConsumerState<SourceStep> createState() => _SourceStepState();
}

class _SourceStepState extends ConsumerState<SourceStep> {
  /// The source the person picked; null shows the one in use.
  String? _picked;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final SetupView view = widget.view;
    final AgentDepot? depot = view.depot;
    final String current = depot?.source ?? AgentSource.host;
    final String picked = _picked ?? current;
    final bool locked =
        depot != null && (depot.locked || !depot.actions.sourceHost);
    final AgentSourceOption? host = proposedSource(
      view.setup.proposal,
      view.agent,
      AgentSource.host,
    );
    final bool busy = view.setup.busy;
    final String name = view.agentName;
    return SetupStepFrame(
      step: view.step,
      bodyPrimary: picked != current && !busy,
      title: l10n.agentSetupSourceTitle(name),
      lead: l10n.agentSetupSourceLead,
      variant: current == AgentSource.depot,
      commands: setupCommands(SetupStepId.source, view.agent),
      refusal: view.setup.refusal,
      onBack: view.onBack,
      onNext: view.onNext,
      body: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          SetupChoice(
            key: const Key('agent-setup-source-host'),
            title: l10n.agentSetupSourceHost,
            help: l10n.agentSetupSourceHostHelp(name),
            selected: picked == AgentSource.host,
            onSelect: locked || busy
                ? null
                : () => setState(() => _picked = AgentSource.host),
            trailing: _HostFact(option: host),
            reason: _reason(context, host),
          ),
          SizedBox(height: tokens.spacing.x2),
          SetupChoice(
            key: const Key('agent-setup-source-depot'),
            title: l10n.agentSetupSourceDepot,
            help: l10n.agentSetupSourceDepotHelp,
            selected: picked == AgentSource.depot,
            onSelect: busy || !(depot?.actions.sourceDepot ?? false)
                ? null
                : () => setState(() => _picked = AgentSource.depot),
            trailing: _DepotFact(depot: depot),
          ),
          if (depot != null && depot.locked) ...<Widget>[
            SizedBox(height: tokens.spacing.x3),
            SetupNote(
              key: const Key('agent-setup-source-locked'),
              glyph: HGlyph.lock,
              color: tokens.colors.fg1,
              text: l10n.settingsAgentsLocked(depot.lockedBy),
            ),
          ],
          if (picked != current) ...<Widget>[
            SizedBox(height: tokens.spacing.x4),
            _Change(
              to: picked,
              agentName: name,
              busy: busy,
              onConfirm: () => unawaited(_apply(picked)),
            ),
          ],
        ],
      ),
    );
  }

  Future<void> _apply(String source) async {
    await ref.read(agentSetupProvider.notifier).setSource(source);
    if (mounted) {
      setState(() => _picked = null);
    }
  }

  /// Why the program is not on the host, from the daemon's reason.
  Widget? _reason(BuildContext context, AgentSourceOption? host) {
    if (host == null || host.available || host.reasons.isEmpty) {
      return null;
    }
    final HTokens tokens = HTheme.of(context);
    return DiagnosticCause(
      diagnostic: host.reasons.first,
      style: tokens.typography.ui12.tinted(tokens.colors.fg2),
    );
  }
}

/// What changes with the other source, and the button that changes it.
class _Change extends StatelessWidget {
  const _Change({
    required this.to,
    required this.agentName,
    required this.busy,
    required this.onConfirm,
  });

  final String to;
  final String agentName;
  final bool busy;
  final VoidCallback onConfirm;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final bool depot = to == AgentSource.depot;
    return Row(
      children: <Widget>[
        Expanded(
          child: Text(
            depot
                ? l10n.agentSetupSourceToDepot(agentName)
                : l10n.agentSetupSourceToHost(agentName),
            key: const Key('agent-setup-source-change'),
            style: tokens.typography.ui12.tinted(tokens.colors.fg1),
          ),
        ),
        SizedBox(width: tokens.spacing.x4),
        HButton(
          key: const Key('agent-setup-source-apply'),
          variant: HButtonVariant.primary,
          onPressed: busy ? null : onConfirm,
          child: Text(
            depot
                ? l10n.agentSetupSourceUseDepot
                : l10n.agentSetupSourceUseHost,
          ),
        ),
      ],
    );
  }
}

/// Whether the program lies on this machine.
class _HostFact extends StatelessWidget {
  const _HostFact({required this.option});

  final AgentSourceOption? option;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final AgentSourceOption? option = this.option;
    if (option == null) {
      return const SizedBox.shrink();
    }
    return option.available
        ? SetupNote(
            key: const Key('agent-setup-host-found'),
            glyph: HGlyph.check,
            color: tokens.stateText.allowed,
            text: l10n.agentSetupSourceFound,
          )
        : SetupNote(
            key: const Key('agent-setup-host-missing'),
            glyph: HGlyph.triangleAlert,
            color: tokens.colors.warning,
            text: l10n.agentSetupSourceNotFound,
          );
  }
}

/// What the depot holds.
class _DepotFact extends StatelessWidget {
  const _DepotFact({required this.depot});

  final AgentDepot? depot;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final AgentDepot? depot = this.depot;
    if (depot == null) {
      return const SizedBox.shrink();
    }
    if (!depot.depotSupported) {
      return SetupNote(
        glyph: HGlyph.minus,
        color: tokens.colors.fg2,
        text: l10n.agentSetupSourceNoRecipe,
      );
    }
    return depot.installed
        ? SetupNote(
            key: const Key('agent-setup-depot-installed'),
            glyph: HGlyph.check,
            color: tokens.stateText.allowed,
            text: l10n.agentSetupSourceInstalled(depot.version),
          )
        : SetupNote(
            key: const Key('agent-setup-depot-empty'),
            glyph: HGlyph.minus,
            color: tokens.colors.fg2,
            text: l10n.agentSetupSourceNotInstalled,
          );
  }
}
