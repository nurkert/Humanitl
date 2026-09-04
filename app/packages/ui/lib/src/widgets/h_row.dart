import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';

import '../theme/h_theme.dart';
import '../tokens/flow_state.dart';
import '../tokens/motion.dart';
import '../tokens/spacing.dart';
import '../tokens/tokens.dart';
import '../tokens/typography.dart';
import 'h_animated_fill.dart';
import 'h_focus_ring.dart';

/// Eine Zeile der Queue, der Regelliste oder der History.
///
/// Die Zeile ist der Ort, an dem die meisten Regeln von `docs/UX.md`
/// zusammenlaufen; sie steht deshalb hier einmal vollständig, statt in jedem
/// Screen neu:
///
/// * **Höhe.** [minHeight] ist eine Mindesthöhe und keine feste Höhe. Eine
///   Zeile wechselt ihre Höhe nie, weil sich ihr Zustand ändert (2.9); dass
///   dieselbe Zeile bei größerer Textskalierung höher ist, ist Layout und
///   keine Animation (6). Die drei Dichten heißen [HSize.row] (36),
///   [HSize.rowHistory] (28) und [HSize.rowBody] (24).
/// * **Füllung.** Hover `bg2`, Auswahl `bg3`, nie dieselbe Farbe: `Enter`
///   erlaubt die ausgewählte Zeile, und Erlauben ist unumkehrbar; sähe eine
///   überfahrene Zeile aus wie die ausgewählte, läse jemand die eine und
///   sendete die andere (3.4). Der Wechsel läuft über [HMotion.press], nicht
///   über den Rail-Wisch.
/// * **Rail.** Die Auswahl **ersetzt** die Zustands-Rail über die vollen vier
///   Pixel, sie überlagert sie nicht (3.4, 3.5). Jedes Mitglied einer
///   Mehrfachauswahl trägt dieselbe Akzent-Rail ([inSelection]), auch ohne
///   Cursor. Mit [tintedRail] steht die ruhende Rail als Zehn-Prozent-Tönung
///   da — die eine benannte Ausnahme von der 3:1-Regel, für die Queue, in der
///   per Konstruktion nur `held` steht (3.3, Regel 1 und 10).
/// * **Zweiter Kanal.** [stateGlyph] ist der Slot für das Zustands-Glyph.
///   Farbe ist nie der einzige Kanal: im hellen Theme messen `allowed` und
///   `blocked` unter Deuteranopie 1,01:1 (3.3, Regel 2).
/// * **Fokus.** Die Zeile ist ein Fokusstopp und zeigt den Ring auf ihrer
///   eigenen Kante ([HFocusRing.inline]); eine Zeile von Rand zu Rand hat kein
///   Außen, in das ein Ring passen könnte (5.1, 6).
/// * **Aktionsslot.** [actionSlot] ist immer [HSize.rowActionSlot] breit und
///   bei Ruhe leer. Hover **und** Fokus decken die Aktion darin auf; er
///   ersetzt nichts, verschiebt nichts und lässt keinen Text neu umbrechen
///   (3.4).
///
/// Die Zeile beobachtet nichts. Sie nimmt aufgelöste Werte, also taugt
/// dieselbe Klasse für die lebende und für die gehende Zeile: der Abgang
/// zeichnet ein eingefrorenes Abbild mit stehendem Countdown, und dafür
/// braucht es kein zweites Widget, sondern nur aufgelöste Argumente
/// (2.4, 9, Punkt 9).
class HRow extends StatefulWidget {
  /// Creates a row.
  const HRow({
    required this.state,
    required this.title,
    this.leading,
    this.stateGlyph,
    this.subtitle,
    this.trailing,
    this.actionSlot,
    this.selected = false,
    this.inSelection = false,
    this.tintedRail = false,
    this.minHeight = HSize.row,
    this.onTap,
    this.onHover,
    this.onFocusChange,
    this.focusNode,
    this.autofocus = false,
    this.semanticsLabel,
    this.semanticsValue,
    super.key,
  });

  /// Visual state of the flow this row shows.
  final HFlowState state;

  /// Der Host, oder was die Zeile sonst benennt. 13/500.
  final Widget title;

  /// Method badge oder was sonst vor dem Titel steht.
  final Widget? leading;

  /// Das Zustands-Glyph, mit oder ohne Countdown-Ring.
  ///
  /// Steht zwischen Rail und [leading], weil der Zustand vor der Methode
  /// gelesen wird (`docs/UX.md` 3.4).
  final Widget? stateGlyph;

  /// Die zweite Zeile, nur sichtbar, solange [selected] gilt.
  ///
  /// Die Queue benutzt sie nicht mehr: ihre Zeile bleibt in jedem Zustand
  /// einzeilig, weil eine zweite Zeile nur wiederholt, was die Karte daneben
  /// ohnehin zeigt (`docs/UX.md` 8, Abweichung zu HUM-020).
  final Widget? subtitle;

  /// Countdown, Findings-Chip, alles Rechtsbündige.
  final Widget? trailing;

  /// Die Aktion am rechten Rand, aufgedeckt bei Hover oder Fokus.
  final Widget? actionSlot;

  /// Whether this row is the selected one.
  final bool selected;

  /// Ob die Zeile Mitglied einer Mehrfachauswahl ist, ohne den Cursor zu
  /// tragen. Die Rail sagt, was ausgewählt ist, die Füllung, wo der Cursor
  /// steht (`docs/UX.md` 3.5).
  final bool inSelection;

  /// Ob die ruhende Zustands-Rail als Tönung statt in voller Sättigung steht.
  final bool tintedRail;

  /// Die Mindesthöhe der Zeile, eine der drei Dichten aus `docs/UX.md` 3.2.
  final double minHeight;

  /// Invoked on tap.
  final VoidCallback? onTap;

  /// Invoked when the pointer enters or leaves.
  final ValueChanged<bool>? onHover;

  /// Invoked when the row takes or loses the keyboard focus.
  final ValueChanged<bool>? onFocusChange;

  /// Ein von außen gehaltener Fokusknoten, damit ein Screen eine Zeile
  /// fokussieren kann, ohne sie anzuklicken.
  final FocusNode? focusNode;

  /// Nimmt den Fokus, sobald die Zeile zum ersten Mal gebaut wird.
  final bool autofocus;

  /// Screen-reader label for the whole row.
  final String? semanticsLabel;

  /// Der Semantics-Value der Zeile, üblicherweise die verbleibende Frist.
  ///
  /// Die Frist gehört nicht ins Label: ein Label mit `mm:ss` ändert sich
  /// einmal je Sekunde, und ein Screenreader wiederholt es dann jedes Mal
  /// vollständig (`docs/UX.md` 6).
  final String? semanticsValue;

  @override
  State<HRow> createState() => _HRowState();
}

class _HRowState extends State<HRow> {
  bool _hovered = false;
  bool _focused = false;

  void _setHovered(bool value) {
    if (_hovered == value) {
      return;
    }
    setState(() => _hovered = value);
    widget.onHover?.call(value);
  }

  void _setFocused(bool value) {
    if (_focused == value) {
      return;
    }
    setState(() => _focused = value);
    widget.onFocusChange?.call(value);
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final Color stateColor = tokens.stateColor(widget.state);
    // Die Auswahl ersetzt die Zustands-Rail über die vollen vier Pixel; sie
    // überlagert nicht ihre linke Hälfte.
    final Color rail = widget.selected || widget.inSelection
        ? tokens.colors.accent
        : widget.tintedRail
        ? tokens.tint(stateColor)
        : stateColor;
    // Die Füllung sagt, wo der Cursor steht, die Rail, was ausgewählt ist.
    final Color background = widget.selected
        ? tokens.colors.bg3
        : _hovered
        ? tokens.colors.bg2
        : const Color(0x00000000);
    final bool revealed = _hovered || _focused;

    final Widget lines = Column(
      // MainAxisSize.min, sonst nimmt die Spalte die volle verfügbare Höhe und
      // die Zeile mit ihr: die Mindesthöhe ist eine Untergrenze, keine Höhe.
      mainAxisSize: MainAxisSize.min,
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

    // Keine animierte Höhe: nur die Füllung wechselt, und sie wechselt mit
    // HMotion.press (`docs/UX.md` 2.9 und 9, Punkt 11).
    //
    // Die Rail liegt im Stack und nicht in der Zeile, damit sie die volle
    // Höhe trägt, auch wenn die Textskalierung die Zeile höher macht als
    // ihre Mindesthöhe.
    final Widget row = HAnimatedFill(
      color: background,
      builder: (BuildContext context, Color fill) => Container(
        constraints: BoxConstraints(minHeight: widget.minHeight),
        // `decoration` und nicht `color`: die einzige `ColoredBox` der Zeile
        // bleibt so die Zustands-Rail, und ein Test, der nach ihr sucht,
        // findet nicht den Hintergrund.
        decoration: BoxDecoration(color: fill),
        child: Stack(
          children: <Widget>[
            Positioned(
              left: 0,
              top: 0,
              bottom: 0,
              width: HSize.stateRail,
              child: ColoredBox(color: rail),
            ),
            Padding(
              padding: const EdgeInsets.only(left: HSize.stateRail),
              child: Row(
                children: <Widget>[
                  SizedBox(width: tokens.spacing.x2),
                  if (widget.stateGlyph != null) ...<Widget>[
                    widget.stateGlyph!,
                    SizedBox(width: tokens.spacing.x2),
                  ],
                  if (widget.leading != null) ...<Widget>[
                    widget.leading!,
                    SizedBox(width: tokens.spacing.x2),
                  ],
                  Expanded(child: lines),
                  if (widget.trailing != null) ...<Widget>[
                    SizedBox(width: tokens.spacing.x2),
                    widget.trailing!,
                  ],
                  if (widget.actionSlot != null) ...<Widget>[
                    SizedBox(width: tokens.spacing.x2),
                    SizedBox(
                      width: HSize.rowActionSlot,
                      child: revealed ? widget.actionSlot : null,
                    ),
                  ],
                  SizedBox(width: tokens.spacing.x3),
                ],
              ),
            ),
          ],
        ),
      ),
    );

    return Semantics(
      selected: widget.selected,
      button: widget.onTap != null,
      label: widget.semanticsLabel,
      value: widget.semanticsValue,
      child: FocusableActionDetector(
        enabled: widget.onTap != null,
        focusNode: widget.focusNode,
        autofocus: widget.autofocus,
        onFocusChange: _setFocused,
        actions: <Type, Action<Intent>>{
          ActivateIntent: CallbackAction<ActivateIntent>(
            onInvoke: (ActivateIntent intent) {
              widget.onTap?.call();
              return null;
            },
          ),
        },
        // Hover kommt aus einer MouseRegion und nicht aus
        // onShowHoverHighlight: der Rückruf wartet, bis der Highlight-Modus
        // "traditional" ist, und bis zum ersten Tasten- oder Mausereignis der
        // Sitzung ist er es nicht. Die Affordanz im Aktionsslot darf davon
        // nicht abhängen.
        child: MouseRegion(
          onEnter: (PointerEnterEvent _) => _setHovered(true),
          onExit: (PointerExitEvent _) => _setHovered(false),
          cursor: widget.onTap == null
              ? MouseCursor.defer
              : SystemMouseCursors.click,
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTap: widget.onTap,
            child: HFocusRing.inline(visible: _focused, child: row),
          ),
        ),
      ),
    );
  }
}
