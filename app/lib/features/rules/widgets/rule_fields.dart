/// The two things a rule form needs that no control provides: the label above
/// a field and the line under it that says what is wrong.
///
/// Every control itself -- input, segmented control, chips, checkbox -- comes
/// from `packages/ui`; only the layout of one form line lives here.
library;

import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';

/// The label above a field: what the value means, in the smallest UI size.
class RuleFieldLabel extends StatelessWidget {
  /// Creates a label.
  const RuleFieldLabel(this.text, {super.key});

  /// The label, already localised.
  final String text;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Text(
      text,
      style: tokens.typography.ui12.medium.tinted(tokens.colors.fg1),
    );
  }
}

/// One line of the form: a label, a control and, when something is wrong,
/// the reason directly under it.
class RuleField extends StatelessWidget {
  /// Creates a field.
  const RuleField({
    required this.label,
    required this.child,
    this.error,
    this.hint,
    super.key,
  });

  /// The label above the control.
  final String label;

  /// The control.
  final Widget child;

  /// Why the value cannot be used, already localised. The slot is only taken
  /// when there is something to say, and the message stands under the control
  /// that produced it (`docs/UX.md` 4.4).
  final String? error;

  /// A quiet explanation under the control.
  final String? hint;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final String? error = this.error;
    final String? hint = this.hint;
    return Padding(
      padding: EdgeInsets.only(bottom: tokens.spacing.x3),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          RuleFieldLabel(label),
          SizedBox(height: tokens.spacing.x1),
          child,
          if (error != null) ...<Widget>[
            SizedBox(height: tokens.spacing.x1),
            Text(
              error,
              // The text variant of the state colour: the one that reaches
              // 4,5:1 on every surface of both ladders (`docs/UX.md` 6).
              style: tokens.typography.ui12.tinted(
                tokens.stateTextColor(HFlowState.error),
              ),
            ),
          ] else if (hint != null) ...<Widget>[
            SizedBox(height: tokens.spacing.x1),
            Text(hint, style: tokens.typography.ui12.tinted(tokens.colors.fg1)),
          ],
        ],
      ),
    );
  }
}
