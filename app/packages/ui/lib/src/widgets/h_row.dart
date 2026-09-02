import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';

import '../theme/h_theme.dart';
import '../tokens/flow_state.dart';
import '../tokens/motion.dart';
import '../tokens/spacing.dart';
import '../tokens/tokens.dart';
import '../tokens/typography.dart';

/// One line of the queue or of the history.
///
/// Collapsed it is [HSize.row] tall, selected [HSize.rowSelected] with a second
/// line. The four pixel rail on the left carries the state colour; selection
/// adds a two pixel accent rail on top of it, so the content never shifts
/// sideways when the selection moves. Hovered and selected rows fill with
/// `bg3`; a row at rest is transparent.
class HRow extends StatefulWidget {
  /// Creates a row.
  const HRow({
    required this.state,
    required this.title,
    this.leading,
    this.subtitle,
    this.trailing,
    this.selected = false,
    this.onTap,
    this.onHover,
    this.semanticsLabel,
    super.key,
  });

  /// Visual state of the flow this row shows.
  final HFlowState state;

  /// The host, or whatever identifies the row. 13/500.
  final Widget title;

  /// Method badge, state glyph or nothing.
  final Widget? leading;

  /// The second line, shown only while [selected].
  final Widget? subtitle;

  /// Countdown, findings chip, anything right aligned.
  final Widget? trailing;

  /// Whether this row is the selected one.
  final bool selected;

  /// Invoked on tap.
  final VoidCallback? onTap;

  /// Invoked when the pointer enters or leaves.
  final ValueChanged<bool>? onHover;

  /// Screen-reader label for the whole row.
  final String? semanticsLabel;

  @override
  State<HRow> createState() => _HRowState();
}

class _HRowState extends State<HRow> {
  bool _hovered = false;

  void _setHovered(bool value) {
    if (_hovered == value) {
      return;
    }
    setState(() => _hovered = value);
    widget.onHover?.call(value);
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final Color rail = tokens.stateColor(widget.state);
    final double height = widget.selected
        ? tokens.sizes.rowSelected
        : tokens.sizes.row;
    // Hover and selection both lift the row to the highest surface; what
    // tells the selected row apart is the accent rail, not a different fill.
    // The fill must not be bg1: a row usually sits in an HPanel, which is bg1
    // itself, and a hover nobody can see is no hover.
    final Color background = widget.selected || _hovered
        ? tokens.colors.bg3
        : const Color(0x00000000);

    final Widget lines = Column(
      mainAxisAlignment: MainAxisAlignment.center,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        DefaultTextStyle(
          style: tokens.typography.ui13.medium.tinted(tokens.colors.fg0),
          overflow: TextOverflow.ellipsis,
          maxLines: 1,
          child: widget.title,
        ),
        if (widget.selected && widget.subtitle != null)
          DefaultTextStyle(
            style: tokens.typography.mono12.tinted(tokens.colors.fg1),
            overflow: TextOverflow.ellipsis,
            maxLines: 1,
            child: widget.subtitle!,
          ),
      ],
    );

    return Semantics(
      selected: widget.selected,
      button: widget.onTap != null,
      label: widget.semanticsLabel,
      child: MouseRegion(
        onEnter: (PointerEnterEvent _) => _setHovered(true),
        onExit: (PointerExitEvent _) => _setHovered(false),
        cursor: widget.onTap == null
            ? MouseCursor.defer
            : SystemMouseCursors.click,
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: widget.onTap,
          child: AnimatedContainer(
            duration: HMotion.sweep,
            curve: HMotion.enter,
            height: height,
            color: background,
            child: Row(
              children: <Widget>[
                SizedBox(
                  width: HSize.stateRail,
                  height: height,
                  child: Stack(
                    children: <Widget>[
                      Positioned.fill(child: ColoredBox(color: rail)),
                      if (widget.selected)
                        Positioned(
                          left: 0,
                          top: 0,
                          bottom: 0,
                          width: HSize.selectionRail,
                          child: ColoredBox(color: tokens.colors.accent),
                        ),
                    ],
                  ),
                ),
                SizedBox(width: tokens.spacing.x2),
                if (widget.leading != null) ...<Widget>[
                  widget.leading!,
                  SizedBox(width: tokens.spacing.x2),
                ],
                Expanded(child: lines),
                if (widget.trailing != null) ...<Widget>[
                  SizedBox(width: tokens.spacing.x2),
                  widget.trailing!,
                ],
                SizedBox(width: tokens.spacing.x3),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
