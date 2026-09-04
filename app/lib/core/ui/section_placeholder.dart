/// The body of a section whose content arrives with a later issue: the title
/// and one quiet line, nothing that could be mistaken for state.
library;

import 'package:flutter/widgets.dart';

import 'ui.dart';

/// A titled empty section.
class SectionPlaceholder extends StatelessWidget {
  /// Creates a placeholder with [title] and [hint], both localised.
  const SectionPlaceholder({
    required this.title,
    required this.hint,
    super.key,
  });

  /// The section title.
  final String title;

  /// The line below it.
  final String hint;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Padding(
      padding: EdgeInsets.all(tokens.spacing.x6),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Text(
            title,
            style: tokens.typography.ui20.semibold.tinted(tokens.colors.fg0),
          ),
          SizedBox(height: tokens.spacing.x2),
          // `fg1`, nicht `fg2`: die dritte Stufe misst 3,03:1 bis 3,90:1 und
          // ist wirklich deaktivierten Controls vorbehalten. Ein Satz, den
          // jemand lesen soll, steht in `fg1` (`docs/UX.md` 6).
          Text(hint, style: tokens.typography.ui13.tinted(tokens.colors.fg1)),
        ],
      ),
    );
  }
}
