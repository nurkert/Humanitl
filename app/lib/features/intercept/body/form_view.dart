/// Ein Formularrumpf als Tabelle: Name links, Wert rechts, beides dekodiert.
///
/// Prozentkodierung und das `+` für das Leerzeichen sind der Grund, warum diese
/// Ansicht existiert: `token=ghp%5Fabc&to=a%40b.de` sagt einem Menschen nichts,
/// `token  ghp_abc` sagt ihm alles. Die Dekodierung verschiebt jeden Versatz,
/// also führt das Modell zu jedem Zeichen das Byte mit, aus dem es stammt, und
/// diese Ansicht rechnet die Fundstellen darüber zurück.
library;

import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';
import 'body_marks.dart';
import 'body_parser.dart';
import 'body_span.dart';
import 'body_surface.dart';

/// Wie viele Zeichen die Namensspalte höchstens breit wird.
///
/// Eine feste Pixelbreite wäre eine Wette auf die installierte Schrift; die
/// Spalte wächst deshalb mit dem längsten Namen und hört hier auf, damit ein
/// Rumpf mit einem millionenlangen Feldnamen nicht die ganze Fläche nimmt.
/// Was jenseits dieser Grenze steht, wird nicht gezeichnet — und ein Fund
/// darin gilt deshalb als nicht verortet, statt still zu verschwinden.
const int formNameChars = 512;

/// Wie viele Zeichen eines Wertes gezeichnet werden.
const int formValueChars = 4000;

/// Die Herkunftstabelle von [byteOfChar], auf die gezeichneten [cap] Zeichen
/// beschnitten.
///
/// Der Index einer Code-Unit ist zugleich ihr Platz in dieser Tabelle, also
/// beschreibt der Schnitt genau das, was die Spalte zeigt. Der Abschluss am
/// Ende bleibt erhalten, sonst verlöre das letzte gezeichnete Zeichen seinen
/// Bereich.
List<int> drawnChars(List<int> byteOfChar, int cap) =>
    byteOfChar.length > cap + 1 ? byteOfChar.sublist(0, cap + 1) : byteOfChar;

/// [text], auf die gezeichneten [cap] Zeichen beschnitten.
///
/// Gezeichnet wird genau so viel, wie die Spalte breit ist. Der Schnitt fällt
/// an einer bekannten Stelle, also stimmen die Markierungen davor weiter, und
/// was dahinter liegt, gilt als nicht verortet statt still zu verschwinden.
String drawnText(String text, int cap) =>
    text.length > cap ? text.substring(0, cap) : text;

/// Die Formularansicht.
class FormView extends StatelessWidget {
  /// Creates the form view.
  const FormView({
    required this.pairs,
    required this.findings,
    this.focus,
    this.onHover,
    super.key,
  });

  /// Die Paare des Rumpfs.
  final List<FormPair> pairs;

  /// Die Funde, in Byte-Versätzen.
  final List<BodyFinding> findings;

  /// Der Fund, zu dem gesprungen werden soll.
  final BodyFinding? focus;

  /// Wird gerufen, wenn der Zeiger auf einem Fund steht.
  final ValueChanged<BodyFinding?>? onHover;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final TextStyle name = tokens.typography.mono12.tinted(tokens.colors.fg1);
    final TextStyle value = tokens.typography.mono12.tinted(tokens.colors.fg0);
    int longest = 0;
    int widestName = 4;
    for (final FormPair pair in pairs) {
      if (pair.value.length > longest) {
        longest = pair.value.length;
      }
      if (pair.name.length > widestName) {
        widestName = pair.name.length;
      }
    }
    final double advance = monoAdvance(context, value);
    final int nameChars = widestName > formNameChars
        ? formNameChars
        : widestName;
    final int valueChars = longest > formValueChars ? formValueChars : longest;
    final double nameWidth = advance * (nameChars + 1);
    final int? focusRow = formRowOf(pairs, focus);
    return BodySurface(
      focusRow: focusRow,
      focusOffset: focusRow == null ? null : nameWidth + tokens.spacing.x2,
      contentWidth: nameWidth + tokens.spacing.x2 + advance * (valueChars + 2),
      itemCount: pairs.length,
      itemBuilder: (BuildContext context, int index) {
        final FormPair pair = pairs[index];
        return Row(
          key: ValueKey<int>(index),
          crossAxisAlignment: CrossAxisAlignment.center,
          children: <Widget>[
            SizedBox(
              width: nameWidth,
              child: Text.rich(
                TextSpan(
                  children: markedSpans(
                    text: drawnText(pair.name, formNameChars),
                    offset: 0,
                    findings: formMarks(
                      drawnChars(pair.nameByteOfChar, formNameChars),
                      findings,
                    ),
                    style: name,
                    tokens: tokens,
                    onHover: onHover,
                  ),
                ),
                softWrap: false,
                maxLines: 1,
                textDirection: TextDirection.ltr,
              ),
            ),
            SizedBox(width: tokens.spacing.x2),
            Text.rich(
              TextSpan(
                children: markedSpans(
                  text: drawnText(pair.value, formValueChars),
                  offset: 0,
                  findings: formMarks(
                    drawnChars(pair.byteOfChar, formValueChars),
                    findings,
                  ),
                  style: value,
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

/// Die Funde, die in einem Feld mit der Tabelle [byteOfChar] stehen, auf die
/// Zeichen dieses Feldes umgerechnet.
///
/// Je Fund entsteht ein Eintrag **je zusammenhängendem Lauf**, nicht einer vom
/// ersten bis zum letzten Treffer. Ein Bereich, der zwei getrennte Stellen
/// überspannte, unterstriche die harmlosen Zeichen dazwischen mit — und ein
/// Unterstrich sagt „hier steht etwas Gefährliches".
///
/// Ein Zeichen zählt als getroffen, wenn sein Byte-Bereich den des Fundes
/// schneidet. Prozentkodierung macht aus drei Bytes ein Zeichen; ein Fund, der
/// nur die Hex-Ziffern trifft, gehört trotzdem auf dieses Zeichen.
List<BodyFinding> formMarks(List<int> byteOfChar, List<BodyFinding> findings) {
  final List<BodyFinding> marks = <BodyFinding>[];
  final int chars = byteOfChar.length - 1;
  if (chars < 1) {
    return marks;
  }
  for (final BodyFinding finding in findings) {
    if (finding.byteEnd <= finding.byteStart) {
      continue;
    }
    int run = -1;
    for (int i = 0; i <= chars; i++) {
      final bool hit =
          i < chars &&
          byteOfChar[i] < finding.byteEnd &&
          byteOfChar[i + 1] > finding.byteStart;
      if (hit && run < 0) {
        run = i;
      } else if (!hit && run >= 0) {
        marks.add(
          BodyFinding(
            index: finding.index,
            kind: finding.kind,
            tier: finding.tier,
            tone: finding.tone,
            byteStart: finding.byteStart,
            byteEnd: finding.byteEnd,
            charStart: run,
            charEnd: i,
            needle: finding.needle,
          ),
        );
        run = -1;
      }
    }
  }
  return marks;
}

/// Die Zeile, in der [focus] steht, oder null.
int? formRowOf(List<FormPair> pairs, BodyFinding? focus) {
  if (focus == null) {
    return null;
  }
  for (int i = 0; i < pairs.length; i++) {
    final List<BodyFinding> marks = <BodyFinding>[
      ...formMarks(
        drawnChars(pairs[i].nameByteOfChar, formNameChars),
        <BodyFinding>[focus],
      ),
      ...formMarks(
        drawnChars(pairs[i].byteOfChar, formValueChars),
        <BodyFinding>[focus],
      ),
    ];
    if (marks.isNotEmpty) {
      return i;
    }
  }
  return null;
}

/// Welche Funde in dieser Tabelle überhaupt vorkommen.
///
/// Der Rest steht nicht in einem Feld — etwa weil der Rumpf kein sauberes
/// Formular ist —, und die Kopfzeile sagt das, statt ihn verschwinden zu
/// lassen.
Set<int> formLocatedFindings(
  List<FormPair> pairs,
  List<BodyFinding> findings,
) => <int>{
  for (final FormPair pair in pairs) ...<int>[
    for (final BodyFinding mark in formMarks(
      drawnChars(pair.nameByteOfChar, formNameChars),
      findings,
    ))
      mark.index,
    for (final BodyFinding mark in formMarks(
      drawnChars(pair.byteOfChar, formValueChars),
      findings,
    ))
      mark.index,
  ],
};
