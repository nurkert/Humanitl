/// Wie ein Fund in einer Rumpf-Ansicht aussieht.
///
/// Eine Datei für alle vier Ansichten, damit dieselbe Fundstelle in jeder
/// gleich aussieht und keine von ihnen eine eigene Farbe erfindet. In einer
/// Rumpf-Ansicht sind Funde die einzige Chroma (`docs/UX.md` 3.3, Regel 7):
/// Schlüssel, Werte, Bytes und Hex bleiben auf der `fg`-Leiter.
library;

import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';
import 'body_parser.dart';
import 'body_span.dart';

/// Die Stärke des Unterstrichs unter einem Fund.
const double bodyFindingUnderline = 1;

/// Die Farbe, in der ein Fund markiert wird.
Color bodyFindingColor(HTokens tokens, BodyFindingTone tone) => switch (tone) {
  BodyFindingTone.secret => tokens.stateColor(HFlowState.error),
  BodyFindingTone.personal => tokens.stateColor(HFlowState.held),
};

/// Zu jeder Zeile die Funde, die sie berührt.
///
/// Über die Zeilenanfänge gesucht, nicht über alle Zeilen gelaufen: bei
/// zweihunderttausend Zeilen ist der Unterschied der zwischen einem Frame und
/// einer Sekunde.
Map<int, List<BodyFinding>> findingsByRow(
  List<BodyRow> rows,
  List<BodyFinding> findings,
) {
  final Map<int, List<BodyFinding>> byRow = <int, List<BodyFinding>>{};
  if (rows.isEmpty) {
    return byRow;
  }
  for (final BodyFinding finding in findings) {
    if (!finding.hasRange) {
      continue;
    }
    int at = _rowAt(rows, finding.charStart);
    while (at < rows.length && rows[at].charStart < finding.charEnd) {
      if (rows[at].charEnd > finding.charStart) {
        (byRow[at] ??= <BodyFinding>[]).add(finding);
      }
      at++;
    }
  }
  return byRow;
}

/// Die Zeile, in der [charOffset] steht, oder null.
///
/// Null auch dann, wenn der Versatz **hinter** der letzten Zeile liegt: die
/// Zeilen enden bei [bodyMaxRows], der Text nicht. Ein Sprung auf die letzte
/// gebaute Zeile behauptete sonst, die Fundstelle stünde dort — und dort steht
/// sie gerade nicht.
int? rowOfChar(List<BodyRow> rows, int? charOffset) {
  if (charOffset == null || rows.isEmpty) {
    return null;
  }
  return charOffset >= rows.last.charEnd ? null : _rowAt(rows, charOffset);
}

/// Die Zeile, in der [charOffset] steht.
int _rowAt(List<BodyRow> rows, int charOffset) {
  int low = 0;
  int high = rows.length - 1;
  while (low < high) {
    final int mid = (low + high + 1) ~/ 2;
    if (rows[mid].charStart <= charOffset) {
      low = mid;
    } else {
      high = mid - 1;
    }
  }
  return low;
}

/// Wie streng ein Ton ist. Der strengere gewinnt, wo zwei sich überlagern.
int bodyToneRank(BodyFindingTone tone) =>
    tone == BodyFindingTone.secret ? 1 : 0;

/// [text] als Spans, mit einem Unterstrich unter jedem Fund.
///
/// [offset] ist der Zeichenversatz von [text] im ganzen Rumpf; ohne ihn säße
/// jede Markierung ab der zweiten Zeile daneben. [onHover] bekommt den Fund,
/// über dem der Zeiger steht — ein Zeigerereignis auf dem Span, kein
/// Berührungsziel: in einem Rumpf gibt es nichts anzuklicken.
///
/// Überlagerungen werden zerlegt, nicht verworfen. Ein Geheimnis, das ganz in
/// einer Adresse liegt, verschwände sonst hinter der milderen Einstufung, und
/// der Mensch hielte den Wert für harmloser, als er ist. Deshalb wird über die
/// Grenzen aller Funde gelaufen, und jedes Stück bekommt den strengsten Ton,
/// der es abdeckt.
List<InlineSpan> markedSpans({
  required String text,
  required int offset,
  required List<BodyFinding> findings,
  required TextStyle style,
  required HTokens tokens,
  ValueChanged<BodyFinding?>? onHover,
}) {
  if (findings.isEmpty || text.isEmpty) {
    return <InlineSpan>[TextSpan(text: text, style: style)];
  }
  // Die Grenzen aller Funde, auf den Ausschnitt geklemmt.
  final Set<int> cuts = <int>{0, text.length};
  for (final BodyFinding finding in findings) {
    final int start = (finding.charStart - offset).clamp(0, text.length);
    final int end = (finding.charEnd - offset).clamp(0, text.length);
    if (end > start) {
      cuts
        ..add(start)
        ..add(end);
    }
  }
  final List<int> bounds = cuts.toList()..sort();
  final List<InlineSpan> spans = <InlineSpan>[];
  for (int i = 0; i + 1 < bounds.length; i++) {
    final int from = bounds[i];
    final int to = bounds[i + 1];
    BodyFinding? strongest;
    for (final BodyFinding finding in findings) {
      final int start = (finding.charStart - offset).clamp(0, text.length);
      final int end = (finding.charEnd - offset).clamp(0, text.length);
      if (start > from || end < to || end <= start) {
        continue;
      }
      if (strongest == null ||
          bodyToneRank(finding.tone) > bodyToneRank(strongest.tone)) {
        strongest = finding;
      }
    }
    final String piece = text.substring(from, to);
    if (strongest == null) {
      spans.add(TextSpan(text: piece, style: style));
      continue;
    }
    final BodyFinding marked = strongest;
    final Color color = bodyFindingColor(tokens, marked.tone);
    spans.add(
      TextSpan(
        text: piece,
        style: style.copyWith(
          decoration: TextDecoration.underline,
          decorationColor: color,
          decorationThickness: bodyFindingUnderline,
        ),
        mouseCursor: SystemMouseCursors.basic,
        onEnter: onHover == null
            ? null
            : (PointerEnterEvent _) => onHover(marked),
        onExit: onHover == null ? null : (PointerExitEvent _) => onHover(null),
      ),
    );
  }
  return spans;
}
