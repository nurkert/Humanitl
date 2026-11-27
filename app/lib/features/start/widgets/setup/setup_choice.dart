/// One of the exclusive choices of a setup step (GitHub #111): the source
/// of the program, or where the inference goes.
///
/// A bordered surface with a round mark at the left edge, the name, one
/// sentence on what the choice means, and an optional fact at the right
/// edge (what is installed there). The chosen one stands raised with the
/// mark filled in the primary text colour, like the chosen segment of
/// `HSegmented`, since the brand colour stays with the one action; a disabled
/// one keeps its place, in the quiet colour, with its reason under it.
library;

import 'package:flutter/widgets.dart';

import '../../../../core/ui/ui.dart';

/// Diameter of the round mark.
const double _mark = 14;

/// How far the text of a choice stands from its left edge: padding, mark
/// and gap. What belongs to a choice below it starts here.
double setupChoiceIndent(HTokens tokens) =>
    tokens.spacing.x3 + _mark + tokens.spacing.x3;

/// The choice.
class SetupChoice extends StatefulWidget {
  /// Creates the choice.
  const SetupChoice({
    required this.title,
    required this.help,
    required this.selected,
    required this.onSelect,
    this.trailing,
    this.reason,
    super.key,
  });

  /// The name.
  final String title;

  /// What the choice means, in one sentence.
  final String help;

  /// Whether it is chosen.
  final bool selected;

  /// Chooses it; null disables it.
  final VoidCallback? onSelect;

  /// A fact at the right edge, such as the installed version.
  final Widget? trailing;

  /// Why it cannot be chosen, under the sentence.
  final Widget? reason;

  @override
  State<SetupChoice> createState() => _SetupChoiceState();
}

class _SetupChoiceState extends State<SetupChoice> {
  bool _hovered = false;
  bool _focused = false;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final bool enabled = widget.onSelect != null;
    final bool selected = widget.selected;
    final Color fill = selected
        ? tokens.colors.bg2
        : (_hovered && enabled ? tokens.colors.bg2 : tokens.colors.bg1);
    final Color border = selected
        ? tokens.colors.lineStrong
        : tokens.colors.line;
    final Widget? trailing = widget.trailing;
    final Widget? reason = widget.reason;
    return Semantics(
      inMutuallyExclusiveGroup: true,
      checked: selected,
      enabled: enabled,
      button: true,
      label: widget.title,
      child: FocusableActionDetector(
        enabled: enabled,
        onShowHoverHighlight: (bool on) => setState(() => _hovered = on),
        onShowFocusHighlight: (bool on) => setState(() => _focused = on),
        mouseCursor: enabled
            ? SystemMouseCursors.click
            : SystemMouseCursors.basic,
        actions: <Type, Action<Intent>>{
          ActivateIntent: CallbackAction<ActivateIntent>(
            onInvoke: (ActivateIntent _) {
              widget.onSelect?.call();
              return null;
            },
          ),
        },
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: widget.onSelect,
          child: HFocusRing(
            visible: _focused,
            radius: tokens.radii.card,
            child: AnimatedContainer(
              duration: tokens.motion.press,
              curve: tokens.motion.enter,
              padding: EdgeInsets.all(tokens.spacing.x3),
              decoration: BoxDecoration(
                color: fill,
                borderRadius: BorderRadius.circular(tokens.radii.card),
                border: Border.all(color: border),
              ),
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Padding(
                    // Die Marke steht auf der Mitte der ersten Zeile (13/20).
                    padding: const EdgeInsets.only(top: 3),
                    child: _Mark(selected: selected, enabled: enabled),
                  ),
                  SizedBox(width: tokens.spacing.x3),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Text(
                          widget.title,
                          style: tokens.typography.ui13.semibold.tinted(
                            enabled ? tokens.colors.fg0 : tokens.colors.fg2,
                          ),
                        ),
                        SizedBox(height: tokens.spacing.x1 / 2),
                        Text(
                          widget.help,
                          style: tokens.typography.ui12.tinted(
                            enabled ? tokens.colors.fg1 : tokens.colors.fg2,
                          ),
                        ),
                        if (reason != null) ...<Widget>[
                          SizedBox(height: tokens.spacing.x2),
                          reason,
                        ],
                      ],
                    ),
                  ),
                  if (trailing != null) ...<Widget>[
                    SizedBox(width: tokens.spacing.x3),
                    trailing,
                  ],
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _Mark extends StatelessWidget {
  const _Mark({required this.selected, required this.enabled});

  final bool selected;
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final Color ring = !enabled
        ? tokens.colors.line
        : (selected ? tokens.colors.fg0 : tokens.colors.lineStrong);
    return Container(
      width: _mark,
      height: _mark,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        color: tokens.colors.bg1,
        border: Border.all(color: ring, width: HSize.ringStroke),
      ),
      child: selected
          ? Container(
              width: 6,
              height: 6,
              decoration: BoxDecoration(
                shape: BoxShape.circle,
                color: tokens.colors.fg0,
              ),
            )
          : null,
    );
  }
}

/// A small line with a glyph: a fact at the right edge of a choice, or a
/// reason under it.
class SetupNote extends StatelessWidget {
  /// Creates the line.
  const SetupNote({
    required this.glyph,
    required this.color,
    required this.text,
    this.textColor,
    super.key,
  });

  /// The glyph.
  final HGlyph glyph;

  /// The colour of the glyph.
  final Color color;

  /// The words.
  final String text;

  /// The colour of the words; `fg1` without.
  final Color? textColor;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Row(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Padding(
          padding: const EdgeInsets.only(top: 2),
          child: HGlyphIcon(glyph, size: 12, color: color),
        ),
        SizedBox(width: tokens.spacing.x1 + 2),
        Flexible(
          child: Text(
            text,
            style: tokens.typography.ui12.tinted(
              textColor ?? tokens.colors.fg1,
            ),
          ),
        ),
      ],
    );
  }
}
