/// One entry of a list in the settings overlay: a category in the left
/// column, or a hit of the search (HUM-404).
library;

import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';

/// A full-width entry with the look of a rail entry of the shell.
///
/// Hover `bg2`, chosen `bg3` with the 2 px accent mark on the left edge, as
/// in the icon rail: the overlay brings no look of its own. The ring of the
/// keyboard focus sits on the entry's own edge (`HFocusRing.inline`).
class SettingsListItem extends StatefulWidget {
  /// Creates an entry titled [title].
  const SettingsListItem({
    required this.title,
    required this.onTap,
    this.subtitle,
    this.selected = false,
    this.focusNode,
    super.key,
  });

  /// The name, 13/500 when chosen, 13 otherwise.
  final String title;

  /// One line under the name, or null.
  final String? subtitle;

  /// Whether this entry is the chosen one.
  final bool selected;

  /// Invoked on click, `Enter` and `Space`.
  final VoidCallback onTap;

  /// A node held from outside, for keys that move between entries.
  final FocusNode? focusNode;

  @override
  State<SettingsListItem> createState() => _SettingsListItemState();
}

class _SettingsListItemState extends State<SettingsListItem> {
  bool _hovered = false;
  bool _focused = false;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final bool selected = widget.selected;
    final Color fill = selected
        ? tokens.colors.bg3
        : _hovered
        ? tokens.colors.bg2
        : const Color(0x00000000);
    final String? subtitle = widget.subtitle;
    return Semantics(
      button: true,
      selected: selected,
      child: FocusableActionDetector(
        focusNode: widget.focusNode,
        mouseCursor: SystemMouseCursors.click,
        onShowHoverHighlight: (bool on) => setState(() => _hovered = on),
        onShowFocusHighlight: (bool on) => setState(() => _focused = on),
        actions: <Type, Action<Intent>>{
          ActivateIntent: CallbackAction<ActivateIntent>(
            onInvoke: (ActivateIntent _) {
              widget.onTap();
              return null;
            },
          ),
        },
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: widget.onTap,
          child: HFocusRing.inline(
            visible: _focused,
            child: AnimatedContainer(
              duration: HMotion.press,
              color: fill,
              constraints: BoxConstraints(minHeight: tokens.sizes.row),
              child: Stack(
                alignment: AlignmentDirectional.centerStart,
                children: <Widget>[
                  if (selected)
                    Positioned(
                      left: 0,
                      top: 0,
                      bottom: 0,
                      width: HSize.selectionRail,
                      child: ColoredBox(color: tokens.colors.accent),
                    ),
                  Padding(
                    padding: EdgeInsets.symmetric(
                      horizontal: tokens.spacing.x3,
                      vertical: tokens.spacing.x2,
                    ),
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Text(
                          widget.title,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: selected
                              ? tokens.typography.ui13.medium.tinted(
                                  tokens.colors.fg0,
                                )
                              : tokens.typography.ui13.tinted(
                                  _hovered
                                      ? tokens.colors.fg0
                                      : tokens.colors.fg1,
                                ),
                        ),
                        if (subtitle != null)
                          Text(
                            subtitle,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: tokens.typography.ui12.tinted(
                              tokens.colors.fg2,
                            ),
                          ),
                      ],
                    ),
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
