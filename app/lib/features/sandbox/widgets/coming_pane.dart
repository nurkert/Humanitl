/// A pane that says what will stand here and what stands in for it (HUM-040).
///
/// The terminal and the isolation panel are built in later issues. An empty
/// box would read as a fault, and half a terminal would be worse than none:
/// this says what is coming and where the same information can be had in the
/// meantime (`docs/UX.md` 4.1).
library;

import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';

/// An explained placeholder.
class ComingPane extends StatelessWidget {
  /// Says [text] under the optional [title].
  const ComingPane({required this.text, this.title, this.action, super.key});

  /// The heading, or null for a pane inside a tab that already has one.
  final String? title;

  /// What will be here, and what stands in for it.
  final String text;

  /// An optional control that leads to the stand-in.
  final Widget? action;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final String? title = this.title;
    final Widget? action = this.action;
    return Padding(
      padding: EdgeInsets.all(tokens.spacing.x3),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          if (title != null) ...<Widget>[
            Text(
              title,
              style: tokens.typography.ui13.semibold.tinted(tokens.colors.fg1),
            ),
            SizedBox(height: tokens.spacing.x2),
          ],
          ConstrainedBox(
            constraints: BoxConstraints(
              maxWidth: HSize.measureWidth(tokens.typography.ui13.fontSize!),
            ),
            child: Text(
              text,
              style: tokens.typography.ui13.tinted(tokens.colors.fg1),
            ),
          ),
          if (action != null) ...<Widget>[
            SizedBox(height: tokens.spacing.x3),
            action,
          ],
        ],
      ),
    );
  }
}
