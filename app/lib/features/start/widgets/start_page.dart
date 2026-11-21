/// The frame every tab of the start window shares except the settings
/// (HUM-537): a column of fixed width under a title row, a card for a list,
/// and the small label over a group.
library;

import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';

/// The widest the content of a tab gets; wider windows centre it.
const double startPageWidth = 880;

/// A tab page: the title with its one action at the right edge, then
/// [children], scrolled when they do not fit.
class StartPage extends StatelessWidget {
  /// Creates the page.
  const StartPage({
    required this.title,
    required this.children,
    this.action,
    this.subtitle,
    this.intro,
    super.key,
  });

  /// The heading of the tab.
  final String title;

  /// The one action of the tab, at the right end of the title row.
  final Widget? action;

  /// A widget right under the title, such as the folder of a project.
  final Widget? subtitle;

  /// A line under the title, when the tab needs one.
  final String? intro;

  /// The content.
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final String? intro = this.intro;
    final Widget? subtitle = this.subtitle;
    return ColoredBox(
      color: tokens.colors.bg0,
      child: SingleChildScrollView(
        padding: EdgeInsets.symmetric(
          horizontal: tokens.spacing.x6,
          vertical: tokens.spacing.x6,
        ),
        child: Align(
          alignment: Alignment.topCenter,
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: startPageWidth),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Row(
                  children: <Widget>[
                    Expanded(
                      child: Text(
                        title,
                        style: tokens.typography.ui20
                            .copyWith(fontWeight: FontWeight.w600)
                            .tinted(tokens.colors.fg0),
                      ),
                    ),
                    ?action,
                  ],
                ),
                if (subtitle != null) ...<Widget>[
                  SizedBox(height: tokens.spacing.x1),
                  subtitle,
                ],
                if (intro != null) ...<Widget>[
                  SizedBox(height: tokens.spacing.x2),
                  Text(
                    intro,
                    style: tokens.typography.ui13.tinted(tokens.colors.fg1),
                  ),
                ],
                SizedBox(height: tokens.spacing.x5),
                ...children,
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// A list surface: one step above the page, a hairline border, the radius
/// of a card.
class StartCard extends StatelessWidget {
  /// Creates the card around [child].
  const StartCard({required this.child, super.key});

  /// The content.
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return DecoratedBox(
      decoration: BoxDecoration(
        color: tokens.colors.bg1,
        borderRadius: BorderRadius.circular(tokens.radii.card),
        border: Border.all(color: tokens.colors.line),
      ),
      child: ClipRRect(
        borderRadius: BorderRadius.circular(tokens.radii.card),
        child: child,
      ),
    );
  }
}

/// The small label over a group of rows.
class StartSectionLabel extends StatelessWidget {
  /// Creates the label.
  const StartSectionLabel({required this.text, super.key});

  /// The words.
  final String text;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Padding(
      padding: EdgeInsets.only(
        left: tokens.spacing.x1,
        bottom: tokens.spacing.x2,
      ),
      child: Text(
        text.toUpperCase(),
        style: tokens.typography.ui11
            .copyWith(fontWeight: FontWeight.w600, letterSpacing: 0.6)
            .tinted(tokens.colors.fg2),
      ),
    );
  }
}
