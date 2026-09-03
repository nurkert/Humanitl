/// A titled section that folds. Specified for `packages/ui` (HUM-020
/// Schritt 2, `HCollapsible`); it lives here until that package is touched
/// again (handoff). No user string inside: the title comes in localised.
library;

import 'package:flutter/widgets.dart';

import 'ui.dart';

/// A section with a clickable header and a body that folds away.
class HCollapsible extends StatefulWidget {
  /// Creates a section titled [title] around [child].
  const HCollapsible({
    required this.title,
    required this.child,
    this.initiallyOpen = true,
    this.trailing,
    this.semanticsLabel,
    super.key,
  });

  /// The header text, localised.
  final String title;

  /// The body.
  final Widget child;

  /// Whether the body shows at first.
  final bool initiallyOpen;

  /// Something at the right end of the header, for example a count.
  final Widget? trailing;

  /// Screen-reader label of the header; [title] when null.
  final String? semanticsLabel;

  @override
  State<HCollapsible> createState() => _HCollapsibleState();
}

class _HCollapsibleState extends State<HCollapsible>
    with SingleTickerProviderStateMixin {
  late bool _open = widget.initiallyOpen;
  late final AnimationController _controller = AnimationController(
    vsync: this,
    duration: HMotion.arrive,
    value: _open ? 1 : 0,
  );

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  void _toggle() {
    setState(() => _open = !_open);
    if (_open) {
      _controller.forward();
    } else {
      _controller.reverse();
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        Semantics(
          button: true,
          expanded: _open,
          label: widget.semanticsLabel ?? widget.title,
          excludeSemantics: true,
          child: MouseRegion(
            cursor: SystemMouseCursors.click,
            child: GestureDetector(
              behavior: HitTestBehavior.opaque,
              onTap: _toggle,
              child: SizedBox(
                height: HSize.hitMin,
                child: Row(
                  children: <Widget>[
                    RotationTransition(
                      turns: Tween<double>(
                        begin: 0,
                        end: 0.25,
                      ).animate(_controller),
                      child: HGlyphIcon(
                        HGlyph.chevronRight,
                        size: 14,
                        color: tokens.colors.fg2,
                      ),
                    ),
                    SizedBox(width: tokens.spacing.x1),
                    Expanded(
                      child: Text(
                        widget.title,
                        style: tokens.typography.ui12.semibold.tinted(
                          tokens.colors.fg1,
                        ),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                    if (widget.trailing != null) widget.trailing!,
                  ],
                ),
              ),
            ),
          ),
        ),
        ClipRect(
          child: SizeTransition(
            sizeFactor: CurvedAnimation(
              parent: _controller,
              curve: HMotion.enter,
              reverseCurve: HMotion.exit,
            ),
            alignment: Alignment.topCenter,
            child: Padding(
              padding: EdgeInsets.only(
                left: tokens.spacing.x5,
                bottom: tokens.spacing.x2,
              ),
              child: widget.child,
            ),
          ),
        ),
      ],
    );
  }
}
