/// Der Text-Diff einer Änderung, wie der Daemon ihn geschrieben hat
/// (HUM-351).
///
/// Der Daemon liefert `diff -u` mit maskierten Steuer- und Bidi-Zeichen; die
/// Ansicht zeigt ihn Zeichen für Zeichen, ohne Umbruch und ohne eigene
/// Rechnung. Gefärbt wird nur nach dem ersten Zeichen jeder Zeile: `+` als
/// hinzugefügt, `-` als entfernt, `@@` und die beiden Kopfzeilen leise. Das
/// Zeichen selbst bleibt stehen, damit die Farbe nie allein trägt
/// (`docs/UX.md` 3.3).
///
/// Entfernt ist Orange (`error`) und nicht Rot: Rot heißt in diesem Programm
/// ausschließlich „blockiert".
library;

import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';

/// Wie eine Zeile eines Unified-Diffs gezeichnet wird.
enum DiffLineKind {
  /// `--- a/…` und `+++ b/…`.
  header,

  /// `@@ -1,3 +1,4 @@`.
  hunk,

  /// Eine Zeile, die hinzukommt.
  added,

  /// Eine Zeile, die wegfällt.
  removed,

  /// Kontext und alles andere.
  context,
}

/// Die Art von [line], nur nach ihrem Anfang.
DiffLineKind diffLineKind(String line) {
  if (line.startsWith('+++ ') || line.startsWith('--- ')) {
    return DiffLineKind.header;
  }
  if (line.startsWith('@@')) {
    return DiffLineKind.hunk;
  }
  if (line.startsWith('+')) {
    return DiffLineKind.added;
  }
  if (line.startsWith('-')) {
    return DiffLineKind.removed;
  }
  return DiffLineKind.context;
}

/// Der Diff [diff] in Monospace, Zeile für Zeile gefärbt.
class DiffView extends StatelessWidget {
  /// Zeigt [diff].
  const DiffView({required this.diff, super.key});

  /// Der Diff, wie ihn der Daemon geschickt hat.
  final String diff;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final List<String> lines = diff.endsWith('\n')
        ? diff.substring(0, diff.length - 1).split('\n')
        : diff.split('\n');
    final TextStyle base = tokens.typography.mono12;
    return Container(
      decoration: BoxDecoration(
        color: tokens.colors.bg0,
        border: Border.all(color: tokens.colors.line),
        borderRadius: BorderRadius.circular(tokens.radii.control),
      ),
      padding: EdgeInsets.symmetric(vertical: tokens.spacing.x1),
      // Die Zeilen laufen nicht um, die Ansicht rollt waagerecht. Jede Zeile
      // ist so breit wie die längste, mindestens so breit wie die Ansicht,
      // damit die Fläche einer Zeile von Rand zu Rand reicht.
      child: LayoutBuilder(
        builder: (BuildContext context, BoxConstraints constraints) =>
            SingleChildScrollView(
              scrollDirection: Axis.horizontal,
              child: ConstrainedBox(
                constraints: BoxConstraints(minWidth: constraints.maxWidth),
                child: IntrinsicWidth(
                  // Wie die Kommandozeile: `SelectableRegion`, weil die App
                  // auf `WidgetsApp` steht und `SelectableText` nicht
                  // erreicht.
                  child: SelectableRegion(
                    selectionControls: emptyTextSelectionControls,
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: <Widget>[
                        for (final String line in lines)
                          _DiffLine(line: line, base: base, tokens: tokens),
                      ],
                    ),
                  ),
                ),
              ),
            ),
      ),
    );
  }
}

class _DiffLine extends StatelessWidget {
  const _DiffLine({
    required this.line,
    required this.base,
    required this.tokens,
  });

  final String line;
  final TextStyle base;
  final HTokens tokens;

  @override
  Widget build(BuildContext context) {
    final DiffLineKind kind = diffLineKind(line);
    final (Color text, Color? fill) = switch (kind) {
      DiffLineKind.header => (tokens.colors.fg2, null),
      DiffLineKind.hunk => (tokens.colors.fg1, tokens.colors.bg2),
      DiffLineKind.added => (
        tokens.stateText.allowed,
        HColorDerivation.tint(tokens.state.allowed),
      ),
      DiffLineKind.removed => (
        tokens.stateText.error,
        HColorDerivation.tint(tokens.state.error),
      ),
      DiffLineKind.context => (tokens.colors.fg0, null),
    };
    return Container(
      key: ValueKey<String>('diff-line-${kind.name}'),
      color: fill,
      padding: EdgeInsets.symmetric(horizontal: tokens.spacing.x2),
      child: Text(
        // Eine leere Zeile behält ihre Höhe.
        line.isEmpty ? ' ' : line,
        maxLines: 1,
        softWrap: false,
        style: base.tinted(text),
      ),
    );
  }
}
