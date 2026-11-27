/// The frame every step of the guided agent setup shares (GitHub #111):
/// the picture, the title and one sentence on what the step changes, the
/// body, the finding of the daemon against the last action, and a footer
/// with the command line that does the same and the way back and on.
library;

import 'package:flutter/widgets.dart';

import '../../../../core/domain/domain.dart';
import '../../../../core/ui/ui.dart';
import '../../../../core/ui/untranslated_wording.dart';
import '../../../../l10n/l10n.dart';
import '../../setup/setup_steps.dart';
import '../start_page.dart';
import 'setup_emblem.dart';

/// One step on its card.
class SetupStepFrame extends StatelessWidget {
  /// Creates the frame of [step].
  const SetupStepFrame({
    required this.step,
    required this.title,
    required this.lead,
    required this.commands,
    required this.body,
    this.variant = false,
    this.bodyPrimary = false,
    this.refusal,
    this.onBack,
    this.onNext,
    super.key,
  });

  /// The step and its state.
  final SetupStep step;

  /// The heading of the step.
  final String title;

  /// One sentence on what the step changes.
  final String lead;

  /// The command lines that do the same.
  final List<String> commands;

  /// The content.
  final Widget body;

  /// The form of the picture (see [SetupEmblem.variant]).
  final bool variant;

  /// Whether the body shows an enabled primary action now. "Continue" is
  /// then quiet: a surface never has two enabled primary buttons.
  final bool bodyPrimary;

  /// What the daemon said against the last action.
  final Diagnostic? refusal;

  /// The previous step; null on the first.
  final VoidCallback? onBack;

  /// The next step; null on the last. The primary action when the step is
  /// done, a quiet one otherwise, where the step's own action is primary.
  final VoidCallback? onNext;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final Diagnostic? refusal = this.refusal;
    final VoidCallback? onNext = this.onNext;
    final VoidCallback? onBack = this.onBack;
    return StartCard(
      key: Key('agent-setup-panel-${step.id.name}'),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Padding(
            padding: EdgeInsets.all(tokens.spacing.x5),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    SetupEmblem(
                      step: step.id,
                      state: step.state,
                      variant: variant,
                    ),
                    SizedBox(width: tokens.spacing.x4),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: <Widget>[
                          Text(
                            title,
                            key: const Key('agent-setup-title'),
                            style: tokens.typography.ui16.semibold.tinted(
                              tokens.colors.fg0,
                            ),
                          ),
                          SizedBox(height: tokens.spacing.x1),
                          Text(
                            lead,
                            style: tokens.typography.ui13.tinted(
                              tokens.colors.fg1,
                            ),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
                SizedBox(height: tokens.spacing.x5),
                body,
                if (refusal != null) ...<Widget>[
                  SizedBox(height: tokens.spacing.x3),
                  DiagnosticCause(
                    diagnostic: refusal,
                    textKey: const Key('agent-setup-refusal'),
                    style: tokens.typography.ui12.tinted(
                      tokens.stateText.error,
                    ),
                  ),
                ],
              ],
            ),
          ),
          const HHairline(),
          Padding(
            padding: EdgeInsets.symmetric(
              horizontal: tokens.spacing.x5,
              vertical: tokens.spacing.x3,
            ),
            child: Row(
              children: <Widget>[
                Expanded(child: _Commands(commands: commands)),
                SizedBox(width: tokens.spacing.x4),
                if (onBack != null)
                  HButton(
                    key: const Key('agent-setup-back'),
                    variant: HButtonVariant.ghost,
                    onPressed: onBack,
                    child: Text(l10n.agentSetupBack),
                  ),
                if (onNext != null) ...<Widget>[
                  SizedBox(width: tokens.spacing.x2),
                  HButton(
                    key: const Key('agent-setup-next'),
                    variant: step.done && !bodyPrimary
                        ? HButtonVariant.primary
                        : HButtonVariant.secondary,
                    onPressed: onNext,
                    child: Text(l10n.agentSetupContinue),
                  ),
                ],
              ],
            ),
          ),
        ],
      ),
    );
  }
}

/// The command lines of a step, small, under their label.
class _Commands extends StatelessWidget {
  const _Commands({required this.commands});

  final List<String> commands;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Column(
      key: const Key('agent-setup-commands'),
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Text(
          context.l10n.agentSetupCli,
          style: tokens.typography.ui11.medium.tinted(tokens.colors.fg2),
        ),
        for (final String command in commands)
          Text(
            command,
            style: tokens.typography.mono11.tinted(tokens.colors.fg1),
          ),
      ],
    );
  }
}

/// Width of the label column of every labelled row of the setup.
const double setupLabelWidth = 112;

/// A label and its value or control, the one row shape of the setup: facts,
/// the level of a server and the fields of a new server. The label stands
/// in a column of [setupLabelWidth] on the baseline of the value; with
/// [stacked] it stands above it, for a control that needs the whole width.
class SetupLabelledRow extends StatelessWidget {
  /// Creates the row.
  const SetupLabelledRow({
    required this.label,
    required this.child,
    this.stacked = false,
    super.key,
  });

  /// The label.
  final String label;

  /// The value or the control.
  final Widget child;

  /// Whether the label stands above [child].
  final bool stacked;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final Widget text = Text(
      label,
      style: tokens.typography.ui12.tinted(tokens.colors.fg2),
    );
    if (stacked) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          text,
          SizedBox(height: tokens.spacing.x2),
          child,
        ],
      );
    }
    return Row(
      crossAxisAlignment: CrossAxisAlignment.baseline,
      textBaseline: TextBaseline.alphabetic,
      children: <Widget>[
        SizedBox(width: setupLabelWidth, child: text),
        SizedBox(width: tokens.spacing.x3),
        Expanded(child: child),
      ],
    );
  }
}

/// A fact of the depot or the summary: a [SetupLabelledRow] with its room.
class SetupFact extends StatelessWidget {
  /// Creates the row.
  const SetupFact({required this.label, required this.value, super.key});

  /// The label.
  final String label;

  /// The value.
  final Widget value;

  @override
  Widget build(BuildContext context) => Padding(
    padding: EdgeInsets.symmetric(vertical: HTheme.of(context).spacing.x1),
    child: SetupLabelledRow(label: label, child: value),
  );
}
