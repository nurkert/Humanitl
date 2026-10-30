/// The left icon rail: one entry per section, `Ctrl+<digit>`, active entry with the
/// accent rail on its left edge.
///
/// Since HUM-254 it has two widths. Collapsed it is the rail of HUM-019,
/// glyphs only, the name in a hover label. Expanded it also shows the name and
/// the shortcut of every section. A button at its foot and `Ctrl+Shift+0` switch
/// between the two.
library;

import 'dart:async';

import 'package:flutter/widgets.dart';

import '../../../core/ui/hover_label.dart';
import '../../../core/ui/shell_glyph.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../section.dart';

/// Width of the collapsed rail, and edge of the square every glyph sits in.
const double railWidth = 48;

/// Width of the expanded rail (HUM-254).
const double railExpandedWidth = 200;

/// From this width on the names fade in.
///
/// Halfway: below it the strip beside the glyphs is too narrow to read a word
/// in, and a name that appeared there would look cut off rather than revealed.
const double railLabelRevealWidth = (railWidth + railExpandedWidth) / 2;

/// The rail.
class IconRail extends StatelessWidget {
  /// Creates the rail with [active] highlighted.
  const IconRail({
    required this.active,
    required this.onSelect,
    this.expanded = false,
    this.onToggle,
    super.key,
  });

  /// The section currently shown.
  final Section active;

  /// Invoked with the section the person clicked.
  final ValueChanged<Section> onSelect;

  /// True while the rail shows names and shortcuts.
  final bool expanded;

  /// Invoked by the button at the foot of the rail. Without it the rail has
  /// no button.
  final VoidCallback? onToggle;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    // The width is a way, and under reduced motion the way goes: the rail
    // jumps to its end width in the next frame (`docs/UX.md` 2.10). The names
    // still fade, because a fade is feedback and not a way.
    return TweenAnimationBuilder<double>(
      tween: Tween<double>(end: expanded ? railExpandedWidth : railWidth),
      duration: HReducedMotion.displace(context, HMotion.arrive),
      curve: expanded ? HMotion.enter : HMotion.exit,
      builder: (BuildContext context, double width, Widget? _) {
        final bool showLabels = expanded && width >= railLabelRevealWidth;
        return SizedBox(
          key: const Key('icon-rail'),
          width: width,
          child: DecoratedBox(
            decoration: BoxDecoration(
              color: tokens.colors.bg1,
              border: Border(right: BorderSide(color: tokens.colors.line)),
            ),
            child: Column(
              children: <Widget>[
                // Kein Abstand über dem ersten Eintrag: er beginnt direkt
                // unter der Linie der Kopfleiste und steht in derselben Reihe
                // wie die Kopfzeilen der Panes daneben (HUM-286).
                //
                // Die Einträge wachsen mit der Schrift wie die Kopfzeilen
                // daneben; wird das Fenster dafür zu niedrig, rollen sie, statt
                // unten überzulaufen.
                Expanded(
                  child: SingleChildScrollView(
                    child: Column(
                      children: <Widget>[
                        for (final Section section in Section.values)
                          RailEntry(
                            section: section,
                            active: section == active,
                            name: section.label(l10n),
                            shortcut: l10n.shellNavShortcutKey(
                              section.shortcutDigit,
                            ),
                            label: l10n.shellNavShortcut(
                              section.label(l10n),
                              l10n.shellNavShortcutKey(section.shortcutDigit),
                            ),
                            width: width,
                            showLabel: showLabels,
                            onTap: () => onSelect(section),
                          ),
                      ],
                    ),
                  ),
                ),
                if (onToggle case final VoidCallback toggle)
                  _ToggleButton(
                    expanded: expanded,
                    width: width,
                    onToggle: toggle,
                  ),
              ],
            ),
          ),
        );
      },
    );
  }
}

/// One entry of the rail: the glyph of its section in a 48 px square, and
/// beside it, while the rail is wider than that, its name and shortcut.
class RailEntry extends StatefulWidget {
  /// Creates an entry.
  const RailEntry({
    required this.section,
    required this.active,
    required this.label,
    required this.onTap,
    this.name = '',
    this.shortcut = '',
    this.width = railWidth,
    this.showLabel = false,
    super.key,
  });

  /// Which section.
  final Section section;

  /// True for the shown section.
  final bool active;

  /// Tooltip and screen-reader label, localised.
  final String label;

  /// The name of the section, shown beside the glyph while expanded.
  final String name;

  /// The shortcut of the section, shown at the right edge while expanded.
  final String shortcut;

  /// The width of the rail in this frame.
  final double width;

  /// Whether the name and the shortcut are faded in.
  final bool showLabel;

  /// Invoked on tap.
  final VoidCallback onTap;

  @override
  State<RailEntry> createState() => _RailEntryState();
}

class _RailEntryState extends State<RailEntry> {
  bool _hovered = false;

  @override
  void initState() {
    super.initState();
    if (widget.active) {
      _reveal();
    }
  }

  @override
  void didUpdateWidget(RailEntry oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.active && !oldWidget.active) {
      _reveal();
    }
  }

  /// Scrolls the rail so the active entry and its mark are fully visible.
  ///
  /// At a large text scale the entries scroll in a low window; a section
  /// chosen by shortcut must not keep its mark clipped (HUM-286).
  void _reveal() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) {
        unawaited(
          Scrollable.ensureVisible(
            context,
            duration: HReducedMotion.displace(context, HMotion.arrive),
          ),
        );
      }
    });
  }

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
    final Widget target = MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: widget.onTap,
        child: SizedBox(
          width: widget.width,
          height: HPaneHeader.scaleRow(context, tokens.sizes.railItemHeight),
          child: Stack(
            children: <Widget>[
              Positioned.fill(child: ColoredBox(color: fill)),
              // Fläche und Akzent füllen die ganze Höhe des Eintrags, Kante
              // an Kante: Die Markierung ist die Reihe, nicht ein Kasten in
              // ihr (HUM-286).
              if (widget.active)
                Positioned(
                  key: const Key('icon-rail-active-mark'),
                  left: 0,
                  top: 0,
                  bottom: 0,
                  width: HSize.selectionRail,
                  child: ColoredBox(color: tokens.colors.accent),
                ),
              Positioned(
                left: 0,
                top: 0,
                bottom: 0,
                width: railWidth,
                child: Center(
                  child: ShellGlyphIcon(_glyph(widget.section), color: stroke),
                ),
              ),
              if (widget.width > railWidth)
                Positioned(
                  left: railWidth,
                  top: 0,
                  bottom: 0,
                  right: 0,
                  child: RailLabelStrip(
                    visible: widget.showLabel,
                    child: Row(
                      children: <Widget>[
                        Expanded(
                          child: Text(
                            widget.name,
                            maxLines: 1,
                            softWrap: false,
                            overflow: TextOverflow.ellipsis,
                            style: tokens.typography.ui13.tinted(stroke),
                          ),
                        ),
                        SizedBox(width: tokens.spacing.x2),
                        // At most half the strip: under a large text scale
                        // the shortcut is shortened, never pushed past the
                        // edge.
                        ConstrainedBox(
                          constraints: const BoxConstraints(
                            maxWidth: RailLabelStrip.contentWidth / 2,
                          ),
                          child: Text(
                            widget.shortcut,
                            maxLines: 1,
                            softWrap: false,
                            overflow: TextOverflow.ellipsis,
                            style: tokens.typography.ui12.tinted(
                              tokens.colors.fg2,
                            ),
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
            ],
          ),
        ),
      ),
    );
    return Semantics(
      button: true,
      selected: widget.active,
      label: widget.label,
      // Expanded, the name already stands in the row; a hover label beside it
      // would say the same thing twice. Switched off, not left out: leaving
      // it out would give the row a new parent, rebuild it with a fresh
      // strip, and the names would vanish at once instead of fading out
      // while the rail shrinks.
      child: HoverLabel(
        label: widget.label,
        enabled: !widget.showLabel,
        child: target,
      ),
    );
  }

  static ShellGlyph _glyph(Section section) => switch (section) {
    Section.intercept => ShellGlyph.intercept,
    Section.history => ShellGlyph.history,
    Section.rules => ShellGlyph.rules,
    Section.sandbox => ShellGlyph.sandbox,
    Section.audit => ShellGlyph.audit,
    Section.setup => ShellGlyph.setup,
  };
}

/// The strip beside the glyphs that holds a name, revealed by the growing
/// rail rather than squeezed into it.
///
/// [child] is laid out at the full width of the expanded rail in every frame
/// and clipped to the width there is. A row of name and shortcut squeezed into
/// the width of the moment would overflow in almost every frame of the
/// animation, and a name that wrapped and unwrapped while the rail grew would
/// flicker (HUM-254).
///
/// It fades in over [HMotion.arrive] when [visible] turns true, starting from
/// nothing the first time it is built, and fades out the same way. The fade
/// runs with [AnimationBehavior.preserve]: under reduced motion the width of
/// the rail jumps, but the fade is feedback and keeps its duration
/// (`docs/UX.md` 2.10). A controller with the default behaviour would shrink
/// it to five percent as soon as the platform asks for less motion.
class RailLabelStrip extends StatefulWidget {
  /// Creates the strip.
  const RailLabelStrip({required this.visible, required this.child, super.key});

  /// Whether the content is faded in.
  final bool visible;

  /// Name and shortcut.
  final Widget child;

  /// The width the content is laid out at: the expanded rail without the
  /// glyph column.
  static const double contentWidth = railExpandedWidth - railWidth;

  @override
  State<RailLabelStrip> createState() => _RailLabelStripState();
}

class _RailLabelStripState extends State<RailLabelStrip>
    with SingleTickerProviderStateMixin {
  late final AnimationController _fade = AnimationController(
    vsync: this,
    duration: HMotion.arrive,
    animationBehavior: AnimationBehavior.preserve,
  );

  late final CurvedAnimation _opacity = CurvedAnimation(
    parent: _fade,
    curve: HMotion.enter,
    reverseCurve: HMotion.exit,
  );

  @override
  void initState() {
    super.initState();
    if (widget.visible) {
      _fade.forward();
    }
  }

  @override
  void didUpdateWidget(RailLabelStrip oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.visible != oldWidget.visible) {
      widget.visible ? _fade.forward() : _fade.reverse();
    }
  }

  @override
  void dispose() {
    _opacity.dispose();
    _fade.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return ClipRect(
      child: OverflowBox(
        alignment: AlignmentDirectional.centerStart,
        minWidth: RailLabelStrip.contentWidth,
        maxWidth: RailLabelStrip.contentWidth,
        child: Padding(
          padding: EdgeInsetsDirectional.only(end: tokens.spacing.x3),
          child: FadeTransition(opacity: _opacity, child: widget.child),
        ),
      ),
    );
  }
}

/// The button at the foot of the rail that expands and collapses it.
///
/// It stays in the glyph column in both widths, so it does not run away from
/// the pointer that just clicked it; its chevron points the way the rail will
/// move.
class _ToggleButton extends StatelessWidget {
  const _ToggleButton({
    required this.expanded,
    required this.width,
    required this.onToggle,
  });

  final bool expanded;
  final double width;
  final VoidCallback onToggle;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    // `Ctrl+Shift+0` seit HUM-356; `Ctrl+0` setzt die Skalierung zurück.
    final String shortcut = l10n.shellRailShortcutKey;
    final String label = expanded
        ? l10n.shellRailCollapse(shortcut)
        : l10n.shellRailExpand(shortcut);
    return Padding(
      padding: EdgeInsets.only(bottom: tokens.spacing.x2),
      child: SizedBox(
        width: width,
        // Eine Zeile der Rail wie jeder Eintrag darüber (HUM-286); die
        // Breite der Glyphenspalte bleibt [railWidth].
        height: HPaneHeader.scaleRow(context, tokens.sizes.railItemHeight),
        child: Align(
          alignment: AlignmentDirectional.centerStart,
          child: SizedBox(
            width: railWidth,
            child: Center(
              child: Semantics(
                expanded: expanded,
                child: HoverLabel(
                  label: label,
                  child: Transform.flip(
                    flipX: expanded,
                    child: HIconButton(
                      key: const Key('icon-rail-toggle'),
                      glyph: HGlyph.chevronRight,
                      semanticsLabel: label,
                      onPressed: onToggle,
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
