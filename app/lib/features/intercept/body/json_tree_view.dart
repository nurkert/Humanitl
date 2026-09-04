/// Der JSON-Baum: die Wurzel offen, alles darunter zu, und Kinder erst, wenn
/// jemand sie aufklappt.
///
/// Der Baum liegt flach im Modell ([JsonDocument]); diese Ansicht hält nur,
/// welche Knoten offen sind, und rechnet daraus die sichtbare Liste. Zwei
/// Folgen davon sind der Grund für diese Bauform: das Aufklappen eines Knotens
/// fügt genau dessen Kinder ein und läuft nie über den ganzen Baum, und ein
/// Rumpf, der tausend Ebenen tief verschachtelt ist, bringt weder das Modell
/// noch die Ansicht zum Stehen — beide arbeiten iterativ, und das Modell hört
/// bei [jsonMaxDepth] auf.
///
/// Werte bleiben auf der `fg`-Leiter. Drei Farben für Datentypen ließen die
/// Fundstelle mit der Syntax konkurrieren, und ein Rumpf ist der Ort, an dem
/// ein Geheimnis in Sekunden auffallen muss (`docs/UX.md` 3.3, Regel 7, und
/// Abschnitt 8).
///
/// Ein gekürzter Wert bekommt **keinen** Tooltip. Ein Kurzhinweis mit bis zu
/// zweitausend Zeichen fremdem Text, in der Schrift und im Rahmen der
/// Anwendung, wäre genau die Fläche, gegen die dieser Bildschirm gebaut ist:
/// ein Rumpf, der aussieht wie eine Meldung des Programms. Wer den ganzen Wert
/// braucht, nimmt die Rohansicht — dort steht er in Monospace, mit Zeilennummer
/// und ohne Rahmen von uns.
library;

import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';
import 'body_marks.dart';
import 'body_parser.dart';
import 'body_span.dart';
import 'body_surface.dart';

/// Wie weit eine Ebene einrückt.
const double jsonIndent = 12;

/// Der Durchmesser des Punktes, der einen Pfad zu einem Fund markiert.
const double jsonMarkDot = 4;

/// Der JSON-Baum.
class JsonTreeView extends StatefulWidget {
  /// Creates the tree for [document].
  const JsonTreeView({
    required this.document,
    required this.findings,
    this.focus,
    this.onHover,
    super.key,
  });

  /// Der Baum.
  final JsonDocument document;

  /// Die Funde, für den Ton der Markierung.
  final List<BodyFinding> findings;

  /// Der Fund, zu dem der Baum aufklappen und springen soll.
  final BodyFinding? focus;

  /// Wird gerufen, wenn der Zeiger auf einem Fund steht.
  final ValueChanged<BodyFinding?>? onHover;

  @override
  State<JsonTreeView> createState() => _JsonTreeViewState();
}

class _JsonTreeViewState extends State<JsonTreeView> {
  /// Offen ist zu Beginn nur die Wurzel.
  final Set<int> _expanded = <int>{0};
  late List<int> _visible = visibleJsonNodes(widget.document, _expanded);
  int? _focusRow;

  @override
  void initState() {
    super.initState();
    _revealFocus();
  }

  @override
  void didUpdateWidget(JsonTreeView oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!identical(oldWidget.document, widget.document)) {
      _expanded
        ..clear()
        ..add(0);
      _visible = visibleJsonNodes(widget.document, _expanded);
      _focusRow = null;
    }
    if (widget.focus != oldWidget.focus) {
      _revealFocus();
    }
  }

  /// Klappt den Pfad zum gesuchten Fund auf und merkt sich seine Zeile.
  ///
  /// Ohne das Aufklappen zeigte der Sprung auf eine Zeile, die noch gar nicht
  /// da ist -- der Baum steht bis auf die Wurzel zu.
  void _revealFocus() {
    final BodyFinding? focus = widget.focus;
    if (focus == null) {
      _focusRow = null;
      return;
    }
    final int node = nodeOfFinding(widget.document, focus.index);
    if (node < 0) {
      _focusRow = null;
      return;
    }
    int parent = widget.document.nodes[node].parent;
    while (parent >= 0) {
      _expanded.add(parent);
      parent = widget.document.nodes[parent].parent;
    }
    _visible = visibleJsonNodes(widget.document, _expanded);
    final int row = _visible.indexOf(node);
    _focusRow = row < 0 ? null : row;
  }

  void _toggle(int node) {
    setState(() {
      if (!_expanded.remove(node)) {
        _expanded.add(node);
      }
      _visible = visibleJsonNodes(widget.document, _expanded);
    });
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final Map<int, BodyFinding> byIndex = <int, BodyFinding>{
      for (final BodyFinding finding in widget.findings) finding.index: finding,
    };
    return BodySurface(
      focusRow: _focusRow,
      focusOffset: _focusRow == null
          ? null
          : widget.document.nodes[_visible[_focusRow!]].depth * jsonIndent,
      contentWidth: _width(context, tokens),
      itemCount: _visible.length,
      itemBuilder: (BuildContext context, int index) {
        final int at = _visible[index];
        return _JsonRow(
          key: ValueKey<int>(at),
          document: widget.document,
          node: at,
          expanded: _expanded.contains(at),
          findings: byIndex,
          onToggle: _toggle,
          onHover: widget.onHover,
        );
      },
    );
  }

  /// Eine grobe Breite: die tiefste sichtbare Einrückung plus die längste
  /// Zeile. Genauer zu rechnen hieße, jede Zeile zu messen, und das gehört
  /// nicht in ein Layout-Callback (`docs/UX.md` 7).
  double _width(BuildContext context, HTokens tokens) {
    int longest = 0;
    int deepest = 0;
    for (final int at in _visible) {
      final JsonNode node = widget.document.nodes[at];
      final int length =
          (node.key.length > jsonKeyChars ? jsonKeyChars : node.key.length) +
          node.display.length +
          4;
      if (length > longest) {
        longest = length;
      }
      if (node.depth > deepest) {
        deepest = node.depth;
      }
    }
    final double advance = monoAdvance(context, tokens.typography.mono13);
    return deepest * jsonIndent +
        HSize.glyph +
        jsonMarkDot +
        tokens.spacing.x4 +
        advance * (longest + 4);
  }
}

/// Der Knoten, an dem der Fund mit der Nummer [finding] steht, oder -1.
int nodeOfFinding(JsonDocument document, int finding) {
  for (final MapEntry<int, List<JsonMark>> entry
      in document.findingsByNode.entries) {
    for (final JsonMark mark in entry.value) {
      if (mark.finding == finding) {
        return entry.key;
      }
    }
  }
  return -1;
}

/// Die Knoten, die bei [expanded] sichtbar sind, in Vorordnung.
///
/// Iterativ über einen eigenen Stapel; die Tiefe des Baums darf den
/// Aufrufstapel nie erreichen.
List<int> visibleJsonNodes(JsonDocument document, Set<int> expanded) {
  if (document.nodes.isEmpty) {
    return const <int>[];
  }
  final List<int> visible = <int>[];
  final List<int> stack = <int>[0];
  while (stack.isNotEmpty) {
    final int at = stack.removeLast();
    visible.add(at);
    if (!expanded.contains(at)) {
      continue;
    }
    final List<int> children = <int>[];
    int child = document.nodes[at].firstChild;
    while (child >= 0) {
      children.add(child);
      child = document.nodes[child].nextSibling;
    }
    for (int i = children.length - 1; i >= 0; i--) {
      stack.add(children[i]);
    }
  }
  return visible;
}

/// Eine Zeile des Baums.
class _JsonRow extends StatelessWidget {
  const _JsonRow({
    required this.document,
    required this.node,
    required this.expanded,
    required this.findings,
    required this.onToggle,
    required this.onHover,
    super.key,
  });

  final JsonDocument document;
  final int node;
  final bool expanded;
  final Map<int, BodyFinding> findings;
  final ValueChanged<int> onToggle;
  final ValueChanged<BodyFinding?>? onHover;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final JsonNode entry = document.nodes[node];
    final List<JsonMark> marks =
        document.findingsByNode[node] ?? const <JsonMark>[];
    final TextStyle keyStyle = tokens.typography.mono13.tinted(
      tokens.colors.fg0,
    );
    final TextStyle valueStyle = tokens.typography.mono13.tinted(
      switch (entry.type) {
        JsonNodeType.string => tokens.colors.fg1,
        JsonNodeType.nul || JsonNodeType.elided => tokens.colors.fg2,
        _ => tokens.colors.fg0,
      },
    );
    final String keyText = entry.key.isNotEmpty
        ? _keyText(entry.key)
        : entry.index >= 0
        ? '${entry.index}'
        : '';
    final String valueText = _valueText(entry);
    // Der Wert steht in Anführungszeichen; die Versätze des Modells zählen sie
    // nicht mit, also wird die Marke um diese eine Stelle verschoben.
    final int quote = entry.type == JsonNodeType.string ? 1 : 0;
    final Widget row = Row(
      crossAxisAlignment: CrossAxisAlignment.center,
      children: <Widget>[
        SizedBox(width: entry.depth * jsonIndent),
        SizedBox(
          width: HSize.glyph,
          child: entry.isContainer && entry.childCount > 0
              ? Transform.rotate(
                  angle: expanded ? 1.5707963267948966 : 0,
                  child: HGlyphIcon(
                    HGlyph.chevronRight,
                    size: HSize.glyph,
                    color: tokens.colors.fg2,
                  ),
                )
              : const SizedBox.shrink(),
        ),
        if (document.markedAncestors.contains(node)) ...<Widget>[
          SizedBox(width: tokens.spacing.x1),
          _MarkDot(color: bodyFindingColor(tokens, _ancestorTone())),
        ],
        SizedBox(width: tokens.spacing.x1),
        Text.rich(
          TextSpan(
            children: markedSpans(
              text: keyText,
              offset: 0,
              findings: _marksFor(marks, inKey: true, shift: 0),
              style: keyStyle,
              tokens: tokens,
              onHover: onHover,
            ),
          ),
          softWrap: false,
          maxLines: 1,
          textDirection: TextDirection.ltr,
        ),
        SizedBox(width: tokens.spacing.x2),
        Text.rich(
          TextSpan(
            children: markedSpans(
              text: valueText,
              offset: 0,
              findings: _marksFor(marks, inKey: false, shift: quote),
              style: valueStyle,
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
    if (!entry.isContainer || entry.childCount == 0) {
      return row;
    }
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTap: () => onToggle(node),
      child: row,
    );
  }

  /// Die Marken dieses Knotens, die in die Spalte [inKey] gehören, um [shift]
  /// verschoben und mit den Angaben des Fundes angereichert.
  List<BodyFinding> _marksFor(
    List<JsonMark> marks, {
    required bool inKey,
    required int shift,
  }) => <BodyFinding>[
    for (final JsonMark mark in marks)
      if (mark.inKey == inKey && findings[mark.finding] != null)
        BodyFinding(
          index: mark.finding,
          kind: findings[mark.finding]!.kind,
          tier: findings[mark.finding]!.tier,
          tone: findings[mark.finding]!.tone,
          byteStart: findings[mark.finding]!.byteStart,
          byteEnd: findings[mark.finding]!.byteEnd,
          charStart: mark.start + shift,
          charEnd: mark.end + shift,
          needle: findings[mark.finding]!.needle,
        ),
  ];

  /// Der Schlüssel, so weit er gezeichnet wird.
  String _keyText(String key) =>
      key.length > jsonKeyChars ? '${key.substring(0, jsonKeyChars)}…' : key;

  /// Der Ton eines Pfadpunktes: der ernstere der Funde darunter.
  BodyFindingTone _ancestorTone() =>
      findings.values.any(
        (BodyFinding finding) => finding.tone == BodyFindingTone.secret,
      )
      ? BodyFindingTone.secret
      : BodyFindingTone.personal;

  /// Was rechts vom Schlüssel steht.
  String _valueText(JsonNode entry) => switch (entry.type) {
    JsonNodeType.object => expanded ? '' : '{${entry.childCount}}',
    JsonNodeType.array => expanded ? '' : '[${entry.childCount}]',
    JsonNodeType.elided => '…',
    JsonNodeType.string => '"${entry.display}"',
    _ => entry.display,
  };
}

/// Der Punkt auf dem Weg zu einem Fund.
class _MarkDot extends StatelessWidget {
  const _MarkDot({required this.color});

  final Color color;

  @override
  Widget build(BuildContext context) => Container(
    width: jsonMarkDot,
    height: jsonMarkDot,
    decoration: BoxDecoration(color: color, shape: BoxShape.circle),
  );
}
