import 'package:flutter/widgets.dart';

import '../theme/h_theme.dart';
import '../tokens/colors.dart';
import '../tokens/spacing.dart';
import '../tokens/tokens.dart';
import '../tokens/typography.dart';

/// A centred card over a scrim.
///
/// Modals are reserved for destructive confirmations — blocking more than five
/// flows at once, deleting a forever rule, stopping a running sandbox. A modal
/// is never used to make a normal decision; that happens in the queue.
class HModal extends StatelessWidget {
  /// Creates a modal.
  const HModal({
    required this.title,
    required this.child,
    this.actions = const <Widget>[],
    this.onDismiss,
    this.width = 420,
    this.scrimSemanticsLabel,
    super.key,
  });

  /// The heading, 16/600.
  final Widget title;

  /// The body of the card.
  final Widget child;

  /// Buttons, right aligned below the body.
  final List<Widget> actions;

  /// Invoked when the scrim is tapped. Null makes the modal non-dismissible.
  final VoidCallback? onDismiss;

  /// Width of the card.
  final double width;

  /// Screen-reader label of the scrim.
  final String? scrimSemanticsLabel;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Stack(
      fit: StackFit.expand,
      children: <Widget>[
        Semantics(
          button: onDismiss != null,
          label: scrimSemanticsLabel,
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTap: onDismiss,
            child: ColoredBox(
              color: HColorDerivation.fade(tokens.colors.bg0, 0.72),
            ),
          ),
        ),
        Center(
          child: SizedBox(
            width: width,
            child: DecoratedBox(
              decoration: BoxDecoration(
                color: tokens.colors.bg2,
                borderRadius: HRadius.cardRadius,
                border: Border.all(color: tokens.colors.lineStrong),
              ),
              child: Padding(
                padding: const EdgeInsets.all(HSpace.x4),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[
                    DefaultTextStyle(
                      style: tokens.typography.ui16.semibold.tinted(
                        tokens.colors.fg0,
                      ),
                      child: title,
                    ),
                    SizedBox(height: tokens.spacing.x2),
                    DefaultTextStyle(
                      style: tokens.typography.ui13.tinted(tokens.colors.fg1),
                      child: child,
                    ),
                    if (actions.isNotEmpty) ...<Widget>[
                      SizedBox(height: tokens.spacing.x4),
                      Row(
                        mainAxisAlignment: MainAxisAlignment.end,
                        children: <Widget>[
                          for (final Widget action in actions) ...<Widget>[
                            SizedBox(width: tokens.spacing.x2),
                            action,
                          ],
                        ],
                      ),
                    ],
                  ],
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }
}
