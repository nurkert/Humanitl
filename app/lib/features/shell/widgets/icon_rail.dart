/// The left icon rail: five sections, `Ctrl+1..5`, active entry with the
/// accent rail on its left edge.
library;

import 'package:flutter/widgets.dart';

import '../../../core/ui/hover_label.dart';
import '../../../core/ui/shell_glyph.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../section.dart';

/// Width of the rail.
const double railWidth = 48;

/// The rail.
class IconRail extends StatelessWidget {
  /// Creates the rail with [active] highlighted.
  const IconRail({required this.active, required this.onSelect, super.key});

  /// The section currently shown.
  final Section active;

  /// Invoked with the section the person clicked.
  final ValueChanged<Section> onSelect;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return SizedBox(
      width: railWidth,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: tokens.colors.bg1,
          border: Border(right: BorderSide(color: tokens.colors.line)),
        ),
        child: Column(
          children: <Widget>[
            SizedBox(height: tokens.spacing.x2),
            for (final Section section in Section.values)
              RailEntry(
                section: section,
                active: section == active,
                label: l10n.shellNavShortcut(
                  section.label(l10n),
                  'Ctrl+${section.shortcutDigit}',
                ),
                onTap: () => onSelect(section),
              ),
          ],
        ),
      ),
    );
  }
}

/// One entry of the rail: a 48 px square with the glyph of its section.
class RailEntry extends StatefulWidget {
  /// Creates an entry.
  const RailEntry({
    required this.section,
    required this.active,
    required this.label,
    required this.onTap,
    super.key,
  });

  /// Which section.
  final Section section;

  /// True for the shown section.
  final bool active;

  /// Tooltip and screen-reader label, localised.
  final String label;

  /// Invoked on tap.
  final VoidCallback onTap;

  @override
  State<RailEntry> createState() => _RailEntryState();
}

class _RailEntryState extends State<RailEntry> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final Color fill = widget.active
        ? tokens.colors.bg3
        : _hovered
        ? tokens.colors.bg2
        : const Color(0x00000000);
    final Color stroke = widget.active || _hovered
        ? tokens.colors.fg0
        : tokens.colors.fg1;
    return Semantics(
      button: true,
      selected: widget.active,
      label: widget.label,
      child: HoverLabel(
        label: widget.label,
        child: MouseRegion(
          cursor: SystemMouseCursors.click,
          onEnter: (_) => setState(() => _hovered = true),
          onExit: (_) => setState(() => _hovered = false),
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTap: widget.onTap,
            child: SizedBox(
              width: railWidth,
              height: railWidth,
              child: Stack(
                children: <Widget>[
                  Positioned.fill(child: ColoredBox(color: fill)),
                  if (widget.active)
                    Positioned(
                      left: 0,
                      top: tokens.spacing.x2,
                      bottom: tokens.spacing.x2,
                      width: HSize.selectionRail,
                      child: ColoredBox(color: tokens.colors.accent),
                    ),
                  Center(
                    child: ShellGlyphIcon(
                      _glyph(widget.section),
                      color: stroke,
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

  static ShellGlyph _glyph(Section section) => switch (section) {
    Section.intercept => ShellGlyph.intercept,
    Section.history => ShellGlyph.history,
    Section.rules => ShellGlyph.rules,
    Section.sandbox => ShellGlyph.sandbox,
    Section.audit => ShellGlyph.audit,
  };
}
