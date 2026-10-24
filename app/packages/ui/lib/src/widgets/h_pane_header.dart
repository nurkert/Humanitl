import 'package:flutter/widgets.dart';

import '../theme/h_theme.dart';
import '../tokens/spacing.dart';
import 'h_hairline.dart';

/// The header row of a pane or a sheet, with the divider under it (HUM-286).
///
/// Every pane header of the shell is this widget, and it alone sets the
/// height: [HSizeTokens.paneHeaderHeight], the divider included. The content
/// stands vertically centred in that row; a header that reached its height
/// through padding of its own would grow or shrink with its font, and the
/// line under it would no longer continue into the header next door
/// (`docs/UX.md`, "Layout grid"). The height follows the text scale, the same
/// for every header (see [rowHeight]), and content that does not fit is clipped.
///
/// The divider belongs to the header, the upper of the two regions it
/// separates. What stands below starts directly under it and draws no top
/// border of its own.
class HPaneHeader extends StatelessWidget {
  /// Creates a header around [child].
  const HPaneHeader({
    required this.child,
    this.divider,
    this.dividerHeight = HSize.border,
    this.padding = const EdgeInsetsDirectional.symmetric(
      horizontal: HSpace.panelPadding,
    ),
    super.key,
  });

  /// The content of the row, usually a title and its actions.
  final Widget child;

  /// What separates the header from the region below. Null draws a hairline.
  final Widget? divider;

  /// The height of [divider], taken from the height of the row so that the
  /// header as a whole keeps the token.
  final double dividerHeight;

  /// Horizontal inset of [child]. Vertical padding has no place here: the
  /// token sets the height.
  final EdgeInsetsGeometry padding;

  /// The largest factor the row follows the text scale by.
  ///
  /// Twice the text is the largest scale the app is drawn and tested for
  /// (`docs/UX.md` 6); beyond it the row stays at twice its height and its
  /// content is clipped instead of pushing the line out of the grid.
  static const double maxScale = 2;

  /// The height of the whole header row, divider included, at the ambient
  /// text scale.
  ///
  /// One value for every header and for the rail entries beside them, so the
  /// row stays shared when the text grows: [HSizeTokens.paneHeaderHeight]
  /// scaled by the [MediaQuery] text scaler, clamped to 1 to [maxScale] and
  /// rounded to a whole pixel, so the line under it never lands between two.
  static double rowHeight(BuildContext context) =>
      scaleRow(context, HTheme.of(context).sizes.paneHeaderHeight);

  /// [height] scaled the way [rowHeight] scales the header row; the rail
  /// scales `railItemHeight` with it, so both stay one row.
  static double scaleRow(BuildContext context, double height) {
    final TextScaler scaler = MediaQuery.textScalerOf(context)
        .clamp(minScaleFactor: 1, maxScaleFactor: maxScale);
    return scaler.scale(height).roundToDouble();
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        // Eine feste Höhe, keine Mindesthöhe: Wüchse eine Kopfzeile mit
        // ihrem Inhalt, liefe ihre Linie bei großer Schrift aus der Reihe
        // der Nachbarn. Was nicht hineinpasst, wird abgeschnitten.
        ClipRect(
          child: SizedBox(
            height: rowHeight(context) - dividerHeight,
            child: Padding(
              padding: padding,
              child: Align(
                alignment: AlignmentDirectional.centerStart,
                child: child,
              ),
            ),
          ),
        ),
        SizedBox(height: dividerHeight, child: divider ?? const HHairline()),
      ],
    );
  }
}
