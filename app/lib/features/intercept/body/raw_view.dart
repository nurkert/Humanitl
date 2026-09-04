/// Der Rumpf als Text, mit Zeilennummern und den Fundstellen darunter.
///
/// Kein Umbruch, waagerechtes Scrollen: ein Umbruch erzeugte sichtbare Zeilen,
/// die die Versätze der Funde verschieben, und eine falsch gesetzte Fundstelle
/// ist schlimmer als eine lange Zeile (`docs/UX.md` 3.2, HUM-030 Fallstricke).
/// Die Zeilen liegen deshalb schon im Modell fest, samt ihrem Zeichenversatz,
/// und diese Ansicht rechnet keinen einzigen nach.
///
/// Nichts hier ist anklickbar. Ein Rumpf ist Inhalt und kein Bedienelement;
/// eine Zeile, die auf einen Klick reagierte, wäre der erste Schritt zu einer
/// Oberfläche, die der Absender mitgestaltet.
library;

import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';
import 'body_marks.dart';
import 'body_parser.dart';
import 'body_span.dart';
import 'body_surface.dart';

/// Die Rohansicht.
///
/// Nicht `RawView`: so heißt bereits ein Widget von Flutter selbst, und zwei
/// Namen für zwei Dinge sind billiger als ein Präfix an jedem Import.
class RawBodyView extends StatelessWidget {
  /// Creates the raw view for [text].
  const RawBodyView({
    required this.text,
    required this.findings,
    this.focus,
    this.onHover,
    super.key,
  });

  /// Der Rumpf als Text.
  final BodyText text;

  /// Die Funde, auf Zeichen umgerechnet.
  final List<BodyFinding> findings;

  /// Der Fund, zu dem gesprungen werden soll.
  final BodyFinding? focus;

  /// Wird gerufen, wenn der Zeiger auf einem Fund steht.
  final ValueChanged<BodyFinding?>? onHover;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final TextStyle style = tokens.typography.mono13.tinted(tokens.colors.fg0);
    final TextStyle gutter = tokens.typography.mono13.tinted(tokens.colors.fg2);
    final Map<int, List<BodyFinding>> marks = findingsByRow(
      text.rows,
      findings,
    );
    final double advance = monoAdvance(context, style);
    final double numbers = advance * ('${text.rows.length}'.length + 1);
    final int? focusRow = rowOfChar(text.rows, focus?.charStart);
    return BodySurface(
      focusRow: focusRow,
      focusOffset: focusRow == null
          ? null
          : numbers +
                tokens.spacing.x2 +
                advance * (focus!.charStart - text.rows[focusRow].charStart),
      contentWidth:
          numbers + tokens.spacing.x2 + advance * (text.longestRow + 2),
      itemCount: text.rows.length,
      itemBuilder: (BuildContext context, int index) {
        final BodyRow row = text.rows[index];
        return Row(
          key: ValueKey<int>(index),
          crossAxisAlignment: CrossAxisAlignment.center,
          children: <Widget>[
            SizedBox(
              width: numbers,
              child: Text(
                row.continued ? '' : '${row.line}',
                style: gutter,
                textAlign: TextAlign.right,
                textDirection: TextDirection.ltr,
                maxLines: 1,
              ),
            ),
            SizedBox(width: tokens.spacing.x2),
            Text.rich(
              TextSpan(
                children: markedSpans(
                  text: text.slice(row),
                  offset: row.charStart,
                  findings: marks[index] ?? const <BodyFinding>[],
                  style: style,
                  tokens: tokens,
                  onHover: onHover,
                ),
              ),
              softWrap: false,
              maxLines: 1,
              textDirection: TextDirection.ltr,
            ),
          ],
        );
      },
    );
  }
}
