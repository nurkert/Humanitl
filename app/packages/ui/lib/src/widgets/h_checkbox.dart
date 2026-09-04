import 'package:flutter/widgets.dart';

import '../theme/h_theme.dart';
import '../tokens/spacing.dart';
import '../tokens/tokens.dart';
import '../tokens/typography.dart';
import 'h_focus_ring.dart';

/// Ein Kästchen, das an oder aus ist, mit dem Satz daneben, der sagt, was das
/// Anhaken kostet.
///
/// Der [hint] ist keine Zierde: wo eine Einstellung Sicherheit kostet, sagt
/// der Text das in einem Satz (`backlog/CONVENTIONS.md` 4.13). Das Kästchen
/// ist ein Fokusstopp, mindestens [HSize.hitMin] hoch, und zeigt den Fokus als
/// Ring außerhalb (`docs/UX.md` 5.1 und 6).
///
/// Ein Kästchen mit `enabled: false` sieht auch so aus: Beschriftung und Haken
/// stehen in `fg2`, der Stufe, die `docs/UX.md` 6 für wirklich deaktivierte
/// Controls freihält, und die Fläche des Hakens trägt nicht den Akzent — der
/// gehört dem, was man anfassen kann (3.3).
class HCheckbox extends StatefulWidget {
  /// Creates a checkbox.
  const HCheckbox({
    required this.label,
    required this.value,
    required this.onChanged,
    this.hint,
    this.enabled = true,
    this.focusNode,
    super.key,
  });

  /// Die Beschriftung neben dem Kästchen, vom Aufrufer bereits übersetzt.
  final String label;

  /// Ob es angehakt ist.
  final bool value;

  /// Wird mit dem neuen Wert gerufen.
  final ValueChanged<bool> onChanged;

  /// Was das Anhaken bedeutet, wenn das nicht offensichtlich ist.
  final String? hint;

  /// Falsch für etwas, das niemand ändern darf.
  final bool enabled;

  /// Ein von außen gehaltener Fokusknoten.
  final FocusNode? focusNode;

  @override
  State<HCheckbox> createState() => _HCheckboxState();
}

class _HCheckboxState extends State<HCheckbox> {
  bool _focused = false;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final String? hint = widget.hint;
    // Deaktiviert heißt sichtbar deaktiviert: `fg2` ist genau dafür da.
    final Color labelColor = widget.enabled
        ? tokens.colors.fg0
        : tokens.colors.fg2;
    final Color hintColor = widget.enabled
        ? tokens.colors.fg1
        : tokens.colors.fg2;
    return Semantics(
      checked: widget.value,
      enabled: widget.enabled,
      label: widget.label,
      excludeSemantics: true,
      child: FocusableActionDetector(
        enabled: widget.enabled,
        focusNode: widget.focusNode,
        mouseCursor: widget.enabled
            ? SystemMouseCursors.click
            : MouseCursor.defer,
        onFocusChange: (bool value) => setState(() => _focused = value),
        actions: <Type, Action<Intent>>{
          ActivateIntent: CallbackAction<ActivateIntent>(
            onInvoke: (ActivateIntent intent) {
              widget.onChanged(!widget.value);
              return null;
            },
          ),
        },
        child: HFocusRing(
          visible: _focused && widget.enabled,
          radius: tokens.radii.control,
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTap: widget.enabled
                ? () => widget.onChanged(!widget.value)
                : null,
            child: ConstrainedBox(
              constraints: BoxConstraints(minHeight: tokens.sizes.hitMin),
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Padding(
                    padding: EdgeInsets.only(top: tokens.spacing.x2),
                    child: _HTick(on: widget.value, enabled: widget.enabled),
                  ),
                  SizedBox(width: tokens.spacing.x2),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Padding(
                          padding: EdgeInsets.only(top: tokens.spacing.x1),
                          child: Text(
                            widget.label,
                            style: tokens.typography.ui13.tinted(labelColor),
                          ),
                        ),
                        if (hint != null)
                          Text(
                            hint,
                            style: tokens.typography.ui12.tinted(hintColor),
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

/// Das Kästchen selbst: leer eine Haarlinie, angehakt die Akzentfüllung mit
/// dem Haken in [HSurfaceColors.onAccent].
///
/// [HSurfaceColors.accentFill] und nicht [HSurfaceColors.accent]: das Wort auf
/// der Fläche erreicht dort 4,5:1 statt 3,73:1, und der Primärbutton macht es
/// seit derselben Runde genauso (`docs/UX.md` 6). Deaktiviert trägt das
/// Kästchen `fg2` statt des Akzents.
class _HTick extends StatelessWidget {
  const _HTick({required this.on, required this.enabled});

  final bool on;
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final Color filled = enabled ? tokens.colors.accentFill : tokens.colors.fg2;
    final Color empty = enabled ? tokens.colors.lineStrong : tokens.colors.fg2;
    return SizedBox.square(
      dimension: 14,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: on ? filled : const Color(0x00000000),
          borderRadius: HRadius.badgeRadius,
          border: Border.all(color: on ? filled : empty),
        ),
        child: on
            ? CustomPaint(painter: _HTickPainter(tokens.colors.onAccent))
            : null,
      ),
    );
  }
}

class _HTickPainter extends CustomPainter {
  _HTickPainter(this.color);

  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    final Paint stroke = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 2
      ..strokeCap = StrokeCap.round
      ..strokeJoin = StrokeJoin.round
      ..color = color;
    final Path path = Path()
      ..moveTo(size.width * 0.22, size.height * 0.52)
      ..lineTo(size.width * 0.42, size.height * 0.74)
      ..lineTo(size.width * 0.78, size.height * 0.28);
    canvas.drawPath(path, stroke);
  }

  @override
  bool shouldRepaint(_HTickPainter oldDelegate) => oldDelegate.color != color;
}
