/// The header section of the request card.
///
/// Values that carry a credential are masked until the person asks to see
/// them: a screenshot of the card, or a shoulder next to it, must not hand
/// out a token that the request itself is being held for.
library;

import 'package:flutter/gestures.dart';
// `Flow` is a domain type here, not the Flutter layout widget of the same
// name; the widget is never used in this feature.
import 'package:flutter/widgets.dart' hide Flow;

import '../../../core/domain/domain.dart';
import '../../../core/ui/h_collapsible.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import 'key_value_table.dart';

/// Header names whose value is masked by default, lowercase.
const Set<String> maskedHeaders = <String>{
  'authorization',
  'proxy-authorization',
  'cookie',
  'set-cookie',
  'x-api-key',
};

/// True when the value of [name] is masked until it is revealed.
bool isMaskedHeader(String name) => maskedHeaders.contains(name.toLowerCase());

/// The collapsible header section.
class SectionHeaders extends StatefulWidget {
  /// Creates the section for [headers].
  const SectionHeaders({required this.headers, super.key});

  /// The headers of the request, in the order the daemon reported them.
  final List<Header> headers;

  @override
  State<SectionHeaders> createState() => _SectionHeadersState();
}

class _SectionHeadersState extends State<SectionHeaders> {
  final Set<int> _revealed = <int>{};

  @override
  void didUpdateWidget(SectionHeaders oldWidget) {
    super.didUpdateWidget(oldWidget);
    // Another request, another set of headers: nothing stays uncovered.
    if (!identical(oldWidget.headers, widget.headers)) {
      _revealed.clear();
    }
  }

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = context.l10n;
    final HTokens tokens = HTheme.of(context);
    final List<Header> headers = widget.headers;
    final List<KeyValue> rows = <KeyValue>[
      for (int i = 0; i < headers.length; i++)
        KeyValue(
          headers[i].name,
          isMaskedHeader(headers[i].name) && !_revealed.contains(i)
              ? l10n.interceptMaskedValue
              : headers[i].text,
        ),
    ];
    return HCollapsible(
      title: l10n.interceptSectionHeaders(headers.length),
      child: headers.isEmpty
          ? Text(
              l10n.interceptHeadersEmpty,
              style: tokens.typography.ui12.tinted(tokens.colors.fg2),
            )
          : KeyValueTable(
              rows: rows,
              trailing: (BuildContext context, int index) {
                if (!isMaskedHeader(headers[index].name)) {
                  return null;
                }
                final bool shown = _revealed.contains(index);
                return EyeToggle(
                  key: Key('header-eye-${headers[index].name.toLowerCase()}'),
                  revealed: shown,
                  semanticsLabel: shown
                      ? l10n.interceptHideValue(headers[index].name)
                      : l10n.interceptRevealValue(headers[index].name),
                  onPressed: () => setState(() {
                    if (shown) {
                      _revealed.remove(index);
                    } else {
                      _revealed.add(index);
                    }
                  }),
                );
              },
            ),
    );
  }
}

/// The eye that uncovers a masked value.
class EyeToggle extends StatefulWidget {
  /// Creates a toggle.
  const EyeToggle({
    required this.revealed,
    required this.onPressed,
    required this.semanticsLabel,
    super.key,
  });

  /// Whether the value is currently shown.
  final bool revealed;

  /// Flips the state.
  final VoidCallback onPressed;

  /// Screen-reader label, already localised.
  final String semanticsLabel;

  @override
  State<EyeToggle> createState() => _EyeToggleState();
}

class _EyeToggleState extends State<EyeToggle> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Semantics(
      button: true,
      label: widget.semanticsLabel,
      child: MouseRegion(
        cursor: SystemMouseCursors.click,
        onEnter: (PointerEnterEvent _) => setState(() => _hovered = true),
        onExit: (PointerExitEvent _) => setState(() => _hovered = false),
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: widget.onPressed,
          child: SizedBox(
            width: HSize.hitMin,
            height: tokens.typography.mono12.fontSize! * 1.6,
            child: Center(
              child: CustomPaint(
                size: const Size.square(16),
                painter: _EyePainter(
                  color: _hovered ? tokens.colors.fg0 : tokens.colors.fg2,
                  crossed: widget.revealed,
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// Lucide `eye` and `eye-off`, painted like the glyphs of `packages/ui`.
class _EyePainter extends CustomPainter {
  _EyePainter({required this.color, required this.crossed});

  final Color color;
  final bool crossed;

  static const double _viewBox = 24;

  @override
  void paint(Canvas canvas, Size size) {
    final double scale = size.shortestSide / _viewBox;
    canvas.save();
    canvas.scale(scale, scale);
    final Paint stroke = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 2
      ..strokeCap = StrokeCap.round
      ..strokeJoin = StrokeJoin.round
      ..color = color;
    final Path lid = Path()
      ..moveTo(2, 12)
      ..quadraticBezierTo(7, 5, 12, 5)
      ..quadraticBezierTo(17, 5, 22, 12)
      ..quadraticBezierTo(17, 19, 12, 19)
      ..quadraticBezierTo(7, 19, 2, 12);
    canvas
      ..drawPath(lid, stroke)
      ..drawCircle(const Offset(12, 12), 3, stroke);
    if (crossed) {
      canvas.drawLine(const Offset(3, 3), const Offset(21, 21), stroke);
    }
    canvas.restore();
  }

  @override
  bool shouldRepaint(_EyePainter oldDelegate) =>
      oldDelegate.color != color || oldDelegate.crossed != crossed;
}
