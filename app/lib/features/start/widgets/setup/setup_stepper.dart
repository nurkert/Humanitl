/// The vertical stepper of the guided agent setup (GitHub #111).
///
/// One row per step: a node that says the state by glyph and colour (a
/// check for done, the number in the warning ring for a step that needs a
/// hand, the hourglass while a run goes on, a lock while an earlier step
/// waits), the name, and the state in words, so colour is never the only
/// channel. A hairline joins the nodes; the part below a done step takes the
/// colour of done. The chosen step stands like the chosen tab of the rail:
/// a raised surface with the bar of the brand colour.
library;

import 'package:flutter/widgets.dart';

import '../../../../core/ui/ui.dart';
import '../../../../l10n/l10n.dart';
import '../../setup/setup_steps.dart';
import 'setup_text.dart';

/// Diameter of a node.
const double _node = 22;

/// Height of one row.
const double _rowHeight = 52;

/// The stepper.
class SetupStepper extends StatelessWidget {
  /// Creates the stepper over [steps], with [selected] chosen.
  const SetupStepper({
    required this.steps,
    required this.selected,
    required this.onSelect,
    super.key,
  });

  /// The steps with their state.
  final List<SetupStep> steps;

  /// The step on screen.
  final SetupStepId selected;

  /// Chooses a step.
  final ValueChanged<SetupStepId> onSelect;

  @override
  Widget build(BuildContext context) => Semantics(
    container: true,
    label: context.l10n.agentSetupStepsLabel,
    child: Column(
      key: const Key('agent-setup-stepper'),
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        for (final (int index, SetupStep step) in steps.indexed)
          _StepRow(
            step: step,
            number: index + 1,
            first: index == 0,
            last: index == steps.length - 1,
            previousDone: index > 0 && steps[index - 1].done,
            selected: step.id == selected,
            onTap: () => onSelect(step.id),
          ),
      ],
    ),
  );
}

class _StepRow extends StatefulWidget {
  const _StepRow({
    required this.step,
    required this.number,
    required this.first,
    required this.last,
    required this.previousDone,
    required this.selected,
    required this.onTap,
  });

  final SetupStep step;
  final int number;
  final bool first;
  final bool last;
  final bool previousDone;
  final bool selected;
  final VoidCallback onTap;

  @override
  State<_StepRow> createState() => _StepRowState();
}

class _StepRowState extends State<_StepRow> {
  bool _hovered = false;
  bool _focused = false;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final SetupStep step = widget.step;
    final bool selected = widget.selected;
    final String name = setupStepName(l10n, step.id);
    final String state = setupStateName(l10n, step.state);
    final Color fill = selected
        ? tokens.colors.bg3
        : (_hovered ? tokens.colors.bg2 : const Color(0x00000000));
    return Semantics(
      button: true,
      selected: selected,
      label: '$name, $state',
      excludeSemantics: true,
      child: FocusableActionDetector(
        onShowHoverHighlight: (bool on) => setState(() => _hovered = on),
        onShowFocusHighlight: (bool on) => setState(() => _focused = on),
        mouseCursor: SystemMouseCursors.click,
        actions: <Type, Action<Intent>>{
          ActivateIntent: CallbackAction<ActivateIntent>(
            onInvoke: (ActivateIntent _) {
              widget.onTap();
              return null;
            },
          ),
        },
        child: GestureDetector(
          key: Key('agent-setup-step-${step.id.name}'),
          behavior: HitTestBehavior.opaque,
          onTap: widget.onTap,
          child: HFocusRing.inline(
            visible: _focused,
            radius: tokens.radii.control,
            child: Container(
              height: _rowHeight,
              decoration: BoxDecoration(
                color: fill,
                borderRadius: BorderRadius.circular(tokens.radii.control),
              ),
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  // Die gewählte Zeile trägt nur die Fläche: Gelb steht im
                  // Stepper allein am Knoten des Schritts, der dich braucht.
                  SizedBox(width: tokens.spacing.x3),
                  _Rail(
                    step: step,
                    number: widget.number,
                    first: widget.first,
                    last: widget.last,
                    previousDone: widget.previousDone,
                  ),
                  SizedBox(width: tokens.spacing.x3),
                  Expanded(
                    child: Column(
                      mainAxisAlignment: MainAxisAlignment.center,
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Text(
                          name,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: tokens.typography.ui13.semibold.tinted(
                            selected ? tokens.colors.fg0 : tokens.colors.fg1,
                          ),
                        ),
                        Text(
                          state,
                          key: Key('agent-setup-step-state-${step.id.name}'),
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: tokens.typography.ui12.tinted(
                            setupStateTextColor(tokens, step.state),
                          ),
                        ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// The node with the two halves of the line through it.
class _Rail extends StatelessWidget {
  const _Rail({
    required this.step,
    required this.number,
    required this.first,
    required this.last,
    required this.previousDone,
  });

  final SetupStep step;
  final int number;
  final bool first;
  final bool last;
  final bool previousDone;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    Color line(bool done) => done ? tokens.state.allowed : tokens.colors.line;
    Widget half(bool visible, bool done) => Expanded(
      child: Center(
        child: Container(
          width: HSize.hairline,
          color: visible ? line(done) : const Color(0x00000000),
        ),
      ),
    );
    return SizedBox(
      width: _node,
      child: Column(
        children: <Widget>[
          half(!first, previousDone),
          SetupNode(state: step.state, number: number),
          half(!last, step.done),
        ],
      ),
    );
  }
}

/// The node of a step: its state as a glyph in a ring.
class SetupNode extends StatelessWidget {
  /// Creates the node of a step in [state] with the [number] of the step.
  const SetupNode({required this.state, required this.number, super.key});

  /// The state of the step.
  final SetupStepState state;

  /// The place of the step, shown while it needs a hand.
  final int number;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final (Color ring, Color fill, Widget inner) = switch (state) {
      SetupStepState.done => (
        tokens.state.allowed,
        tokens.tint(tokens.state.allowed),
        HGlyphIcon(HGlyph.check, size: 12, color: tokens.stateText.allowed),
      ),
      SetupStepState.action => (
        tokens.colors.warning,
        tokens.colors.bg2,
        Text(
          '$number',
          style: tokens.typography.ui11.semibold.tinted(tokens.colors.fg0),
        ),
      ),
      SetupStepState.running => (
        tokens.colors.lineStrong,
        tokens.colors.bg2,
        HGlyphIcon(HGlyph.hourglass, size: 12, color: tokens.colors.fg1),
      ),
      SetupStepState.blocked => (
        tokens.colors.line,
        tokens.colors.bg2,
        HGlyphIcon(HGlyph.lock, size: 11, color: tokens.colors.fg2),
      ),
    };
    return Container(
      width: _node,
      height: _node,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: fill,
        shape: BoxShape.circle,
        border: Border.all(color: ring, width: HSize.ringStroke),
      ),
      child: inner,
    );
  }
}
