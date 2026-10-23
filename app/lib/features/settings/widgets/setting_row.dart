/// One row of the settings form: what the setting is called, what it does,
/// the control, and the daemon's answer when it refused (HUM-069).
library;

import 'package:flutter/widgets.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';

/// A labelled row with an optional refusal under the control.
///
/// The refusal is the daemon's own sentence (`why`), never one of this form:
/// the side that read the schema knows why a value is wrong (`docs/UX.md`
/// 4.4). It stands where the problem is, under the field, and not in a modal.
class SettingRow extends StatelessWidget {
  /// Creates a row.
  const SettingRow({
    required this.label,
    required this.child,
    this.help,
    this.refusal,
    this.refusalKey,
    this.anchor,
    super.key,
  });

  /// The name of the setting, 13/500.
  final String label;

  /// One sentence about what it does, or null.
  final String? help;

  /// The control.
  final Widget child;

  /// What the daemon said when it refused the last write, or null.
  final Diagnostic? refusal;

  /// The key of the refusal text, for tests.
  final Key? refusalKey;

  /// The node a diagnostic's `ChangeSetting` fix lands on (HUM-275).
  ///
  /// It wraps the control and takes no focus itself; the sheet scrolls to it
  /// and hands the focus to the first control inside it.
  final FocusNode? anchor;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final Diagnostic? refused = refusal;
    return Padding(
      padding: EdgeInsets.symmetric(vertical: tokens.spacing.x2),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Text(
            label,
            style: tokens.typography.ui13.medium.tinted(tokens.colors.fg0),
          ),
          if (help case final String text) ...<Widget>[
            SizedBox(height: tokens.spacing.x1),
            Text(text, style: tokens.typography.ui12.tinted(tokens.colors.fg1)),
          ],
          SizedBox(height: tokens.spacing.x2),
          if (anchor case final FocusNode node)
            Focus(focusNode: node, child: child)
          else
            child,
          if (refused != null) ...<Widget>[
            SizedBox(height: tokens.spacing.x1),
            Text(
              DiagnosticL10n.resolve(refused, context.l10n).cause,
              key: refusalKey,
              style: tokens.typography.ui12.tinted(tokens.stateText.error),
            ),
          ],
        ],
      ),
    );
  }
}

/// The heading of one group of rows.
class SettingGroupTitle extends StatelessWidget {
  /// Creates a heading.
  const SettingGroupTitle(this.text, {super.key});

  /// The heading.
  final String text;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Padding(
      padding: EdgeInsets.only(
        top: tokens.spacing.x4,
        bottom: tokens.spacing.x1,
      ),
      child: Semantics(
        header: true,
        child: Text(
          text,
          style: tokens.typography.ui14.semibold.tinted(tokens.colors.fg0),
        ),
      ),
    );
  }
}
