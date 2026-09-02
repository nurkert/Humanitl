import 'package:flutter/widgets.dart';

import '../theme/h_theme.dart';
import '../tokens/spacing.dart';
import '../tokens/tokens.dart';
import '../tokens/typography.dart';
import 'h_hairline.dart';

/// A persistent region of the shell: panel background, hairline border, no
/// radius and no shadow.
class HPanel extends StatelessWidget {
  /// Creates a panel around [child].
  const HPanel({
    required this.child,
    this.title,
    this.actions = const <Widget>[],
    this.padding = const EdgeInsets.all(HSpace.panelPadding),
    this.semanticsLabel,
    super.key,
  });

  /// The panel body.
  final Widget child;

  /// Optional heading, 13/600.
  final Widget? title;

  /// Optional actions on the right of the heading.
  final List<Widget> actions;

  /// Padding around [child].
  final EdgeInsetsGeometry padding;

  /// Screen-reader label of the whole panel.
  final String? semanticsLabel;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final Widget? title = this.title;
    return Semantics(
      container: true,
      label: semanticsLabel,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: tokens.colors.bg1,
          border: Border.all(color: tokens.colors.line),
          borderRadius: BorderRadius.circular(tokens.radii.panel),
        ),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            if (title != null || actions.isNotEmpty) ...<Widget>[
              Padding(
                padding: const EdgeInsets.symmetric(
                  horizontal: HSpace.panelPadding,
                  vertical: HSpace.x2,
                ),
                child: Row(
                  children: <Widget>[
                    if (title != null)
                      Expanded(
                        child: DefaultTextStyle(
                          style: tokens.typography.ui13.semibold.tinted(
                            tokens.colors.fg0,
                          ),
                          child: title,
                        ),
                      )
                    else
                      const Spacer(),
                    ...actions,
                  ],
                ),
              ),
              const HHairline(),
            ],
            Padding(padding: padding, child: child),
          ],
        ),
      ),
    );
  }
}
