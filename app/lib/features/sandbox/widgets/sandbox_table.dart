/// The dense table both evidence tabs are built from (HUM-040).
///
/// It never truncates. A path the person cannot read whole is a path they
/// cannot check, and this screen exists to be checked: the columns take the
/// width their content needs and the table scrolls sideways, the way code and
/// hex do (`docs/UX.md` 10, "Alle Abstände ...").
library;

import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';

/// One cell: its text and how it is drawn.
@immutable
class SandboxCell {
  /// A cell showing [text].
  const SandboxCell(this.text, {this.mono = false, this.color, this.strong});

  /// The text. Never shortened.
  final String text;

  /// True for a path, a value or anything else that is compared character by
  /// character (CONVENTIONS 4.13).
  final bool mono;

  /// The hue, or null for the ordinary foreground.
  final Color? color;

  /// True for the one cell of a row that carries its meaning.
  final bool? strong;
}

/// One row of a [SandboxTable].
@immutable
class SandboxRowData {
  /// A row of [cells].
  const SandboxRowData({required this.cells, this.key});

  /// The cells, left to right; as many as the table has columns.
  final List<SandboxCell> cells;

  /// Identity of the row, for tests.
  final LocalKey? key;
}

/// A table with a heading row, hairlines between rows and no truncation.
class SandboxTable extends StatelessWidget {
  /// Creates a table of [rows] under [columns].
  const SandboxTable({
    required this.columns,
    required this.rows,
    this.scrollKey,
    super.key,
  });

  /// The column headings, already localised.
  final List<String> columns;

  /// The rows, in the order the daemon reported them. This screen never
  /// sorts: the order of the mounts is the order of the command line, and a
  /// re-sorted table would no longer match the proof it comes from.
  final List<SandboxRowData> rows;

  /// Keeps the scroll position across a tab change.
  final PageStorageKey<String>? scrollKey;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return SingleChildScrollView(
      key: scrollKey,
      padding: EdgeInsets.symmetric(vertical: tokens.spacing.x1),
      child: SingleChildScrollView(
        scrollDirection: Axis.horizontal,
        padding: EdgeInsets.symmetric(horizontal: tokens.spacing.x3),
        child: Table(
          defaultColumnWidth: const IntrinsicColumnWidth(),
          defaultVerticalAlignment: TableCellVerticalAlignment.middle,
          children: <TableRow>[
            TableRow(
              children: <Widget>[
                for (final String column in columns) _head(tokens, column),
              ],
            ),
            for (final SandboxRowData row in rows)
              TableRow(
                decoration: BoxDecoration(
                  border: Border(
                    top: BorderSide(
                      color: tokens.colors.line,
                      width: HSize.hairline,
                    ),
                  ),
                ),
                children: <Widget>[
                  // `TableRow` ist kein Widget und traegt deshalb keinen
                  // Schluessel, den ein Test finden koennte; er sitzt auf der
                  // ersten Zelle, die die Zeile benennt.
                  for (final (int index, SandboxCell cell) in row.cells.indexed)
                    index == 0
                        ? KeyedSubtree(key: row.key, child: _cell(tokens, cell))
                        : _cell(tokens, cell),
                ],
              ),
          ],
        ),
      ),
    );
  }

  Widget _head(HTokens tokens, String text) => Padding(
    padding: EdgeInsets.only(
      right: tokens.spacing.x6,
      bottom: tokens.spacing.x1,
    ),
    child: Text(
      text,
      style: tokens.typography.ui11.semibold.tinted(tokens.colors.fg2),
    ),
  );

  Widget _cell(HTokens tokens, SandboxCell cell) {
    final TextStyle base = cell.mono
        ? tokens.typography.mono12
        : tokens.typography.ui12;
    final TextStyle style = (cell.strong ?? false)
        ? base.semibold.tinted(cell.color ?? tokens.colors.fg0)
        : base.tinted(cell.color ?? tokens.colors.fg1);
    return Padding(
      padding: EdgeInsets.only(
        right: tokens.spacing.x6,
        top: tokens.spacing.x1,
        bottom: tokens.spacing.x1,
      ),
      child: ConstrainedBox(
        constraints: const BoxConstraints(minHeight: HSize.rowBody),
        child: Align(
          alignment: Alignment.centerLeft,
          child: Text(cell.text, style: style, softWrap: false),
        ),
      ),
    );
  }
}
