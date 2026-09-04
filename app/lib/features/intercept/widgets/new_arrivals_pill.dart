/// The pill over the top of the queue: what arrived while somebody read
/// (`docs/UX.md` 2.8).
///
/// It lies over the topmost row and is never inserted into the column: a pill
/// in the header would push every row down by its own height, which is exactly
/// what the freezing exists to prevent. It is a focus stop and it has a key
/// (`Shift+J`), because without one a keyboard user could never merge the
/// arrivals without reaching for the mouse.
library;

import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';

/// How much of the row underneath stays readable through the pill.
const double pillOpacity = 0.92;

/// The `+n new` pill.
class NewArrivalsPill extends StatelessWidget {
  /// Creates the pill for [count] waiting arrivals.
  const NewArrivalsPill({
    required this.count,
    required this.onMerge,
    super.key,
  });

  /// How many requests wait outside the frozen order.
  final int count;

  /// Takes them into the list.
  final VoidCallback onMerge;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return Align(
      alignment: Alignment.topCenter,
      child: Padding(
        padding: EdgeInsets.only(top: tokens.spacing.x2),
        child: Opacity(
          opacity: pillOpacity,
          child: DecoratedBox(
            decoration: BoxDecoration(
              color: tokens.colors.bg2,
              borderRadius: HRadius.controlRadius,
              border: Border.all(color: tokens.colors.line),
            ),
            child: HButton(
              key: const Key('intercept-new-pill'),
              variant: HButtonVariant.ghost,
              onPressed: onMerge,
              // A control that counts held requests stays accent: it can be
              // touched (`docs/UX.md` 3.3, rule 8).
              semanticsLabel: l10n.interceptNewSinceHint(count),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  Text(
                    l10n.interceptNewSinceReading(count),
                    style: tokens.typography.ui11.medium.tinted(
                      tokens.colors.accent,
                    ),
                  ),
                  SizedBox(width: tokens.spacing.x2),
                  Text(
                    l10n.interceptKeyMergeArrivals,
                    style: tokens.typography.mono11.tinted(tokens.colors.fg1),
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
