/// Der Rumpf als Hex: Versatz, sechzehn Bytes, dieselben sechzehn als ASCII.
///
/// Die letzte Ansicht, die immer etwas zeigt. Was kein Text ist, hat hier
/// trotzdem eine Gestalt, und ein Fund steht auch hier an seiner Stelle —
/// sowohl in der Byte- als auch in der Zeichenspalte, damit niemand die
/// Ansicht wechselt und eine Fundstelle dabei verliert.
library;

import 'dart:typed_data';

import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';
import 'body_marks.dart';
import 'body_span.dart';
import 'body_surface.dart';

/// Wie viele Bytes eine Zeile trägt.
const int hexBytesPerRow = 16;

/// Die Hex-Ansicht.
class HexView extends StatelessWidget {
  /// Creates the hex view over [bytes], at most [limit] of them.
  const HexView({
    required this.bytes,
    required this.findings,
    required this.limit,
    this.focus,
    this.onHover,
    super.key,
  });

  /// Die Bytes des Rumpfs.
  final Uint8List bytes;

  /// Die Funde, in Byte-Versätzen.
  final List<BodyFinding> findings;

  /// Wie viele Bytes gezeigt werden.
  final int limit;

  /// Der Fund, zu dem gesprungen werden soll.
  final BodyFinding? focus;

  /// Wird gerufen, wenn der Zeiger auf einem Fund steht.
  final ValueChanged<BodyFinding?>? onHover;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final TextStyle style = tokens.typography.mono12.tinted(tokens.colors.fg0);
    final TextStyle dim = tokens.typography.mono12.tinted(tokens.colors.fg2);
    final int shown = bytes.length < limit ? bytes.length : limit;
    final int rows = (shown + hexBytesPerRow - 1) ~/ hexBytesPerRow;
    final double advance = monoAdvance(context, style);
    return BodySurface(
      focusRow: focus == null ? null : focus!.byteStart ~/ hexBytesPerRow,
      focusOffset: focus == null
          ? null
          : advance * (8 + (focus!.byteStart % hexBytesPerRow) * 3) +
                tokens.spacing.x2,
      // Acht Ziffern Versatz, sechzehn Bytes zu drei Zeichen ohne das letzte
      // Leerzeichen, dann sechzehn Zeichen ASCII; dazwischen zwei Rinnen.
      contentWidth:
          advance * (8 + hexBytesPerRow * 3 - 1 + hexBytesPerRow) +
          tokens.spacing.x2 * 2,
      itemCount: rows,
      itemBuilder: (BuildContext context, int index) {
        final int from = index * hexBytesPerRow;
        final int to = from + hexBytesPerRow < shown
            ? from + hexBytesPerRow
            : shown;
        return Row(
          key: ValueKey<int>(index),
          crossAxisAlignment: CrossAxisAlignment.center,
          children: <Widget>[
            Text(
              from.toRadixString(16).padLeft(8, '0'),
              style: dim,
              textDirection: TextDirection.ltr,
              maxLines: 1,
            ),
            SizedBox(width: tokens.spacing.x2),
            Text.rich(
              TextSpan(children: _cells(tokens, style, from, to, hex: true)),
              softWrap: false,
              maxLines: 1,
              textDirection: TextDirection.ltr,
            ),
            SizedBox(width: tokens.spacing.x2),
            Text.rich(
              TextSpan(children: _cells(tokens, style, from, to, hex: false)),
              softWrap: false,
              maxLines: 1,
              textDirection: TextDirection.ltr,
            ),
          ],
        );
      },
    );
  }

  /// Die Zellen einer Zeile, jede mit ihrem eigenen Ton, falls ein Fund über
  /// ihr liegt.
  List<InlineSpan> _cells(
    HTokens tokens,
    TextStyle style,
    int from,
    int to, {
    required bool hex,
  }) {
    final List<InlineSpan> spans = <InlineSpan>[];
    for (int at = from; at < to; at++) {
      final BodyFinding? finding = _findingAt(at);
      final int byte = bytes[at];
      final String text = hex
          ? '${byte.toRadixString(16).padLeft(2, '0')}${at + 1 < to ? ' ' : ''}'
          // Nur druckbares ASCII; alles andere ist ein Punkt. Ein Rumpf, der
          // seine eigene Anzeige mitschriebe, hätte hier sonst die Bühne.
          : (byte >= 0x20 && byte <= 0x7E ? String.fromCharCode(byte) : '.');
      if (finding == null) {
        spans.add(TextSpan(text: text, style: style));
        continue;
      }
      final Color color = bodyFindingColor(tokens, finding.tone);
      spans.add(
        TextSpan(
          text: text,
          style: style.copyWith(
            decoration: TextDecoration.underline,
            decorationColor: color,
            decorationThickness: bodyFindingUnderline,
          ),
          onEnter: onHover == null ? null : (_) => onHover!(finding),
          onExit: onHover == null ? null : (_) => onHover!(null),
        ),
      );
    }
    return spans;
  }

  /// Der Fund, der auf Byte [at] liegt, oder null.
  BodyFinding? _findingAt(int at) {
    for (final BodyFinding finding in findings) {
      if (at >= finding.byteStart && at < finding.byteEnd) {
        return finding;
      }
    }
    return null;
  }
}
