/// The geometry of the history table: one row height and eleven column
/// widths, in one place, so that the header and the rows cannot drift apart.
///
/// `docs/UX.md` 2.1 forbids a bare number in a feature file, so every value
/// here is either a token of `packages/ui` or built from the spacing scale.
/// The two densities come from `HSize.rowHistory` (28) and `HSize.rowBody`
/// (24), the tokens `docs/UX.md` 9 point 3 asks for. `HSize.hitMin` is
/// deliberately *not* borrowed for the 28 px columns: it means the smallest
/// hit target, which is something else.
library;

import '../../core/ui/ui.dart';

/// Height of one table row: the middle of the three densities (`docs/UX.md`
/// 3.2). A minimum, not a fixed height — the table scales it with the text
/// scaler so that a person at `TextScaler.linear(2.0)` reads whole lines.
const double historyRowHeight = HSize.rowHistory;

/// Height of one line in a body view, the third density.
const double historyBodyRowHeight = HSize.rowBody;

/// Height of the pinned header, the same density as a row.
const double historyHeaderHeight = HSize.rowHistory;

/// What the table is sorted by: the four keys the recorder can order on
/// (`backlog/CONVENTIONS.md` 4.14).
enum HistorySort {
  /// Arrival time; the default.
  time,

  /// Target host, alphabetically.
  host,

  /// Duration.
  duration,

  /// Request plus response size.
  size;

  /// The word `ListFlows.order_by` expects (`humanitl-ipc`, `order_of`).
  String get wire => switch (this) {
    HistorySort.time => 'ts',
    HistorySort.host => 'host',
    HistorySort.duration => 'duration',
    HistorySort.size => 'size',
  };
}

/// The smallest width the path column is given before the table scrolls
/// sideways.
const double historyPathMinWidth = 260;

/// Which value a column shows. The order is the order on screen.
///
/// The `seq` column of `backlog/sprint-2.md` is missing on purpose: the wire
/// `FlowSummary` carries no sequence number, only the recorder row does, so
/// there is nothing to print. Inventing a running number over the loaded page
/// would change under every filter and claim an order the daemon never gave.
///
/// The state column is missing for a different reason: `HRow` has a slot for
/// the state glyph and puts it left of everything else, so the row does not
/// draw it a second time. Its width is [historyStateSlot], and the header
/// labels it there.
enum HistoryColumn {
  /// Arrival time, `HH:mm:ss`.
  time,

  /// The HTTP method, neutral in a list (`docs/UX.md` 3.3, rule 4).
  method,

  /// The target host.
  host,

  /// Path and query.
  path,

  /// The response status.
  status,

  /// Request and response size.
  size,

  /// How long the flow took, in milliseconds.
  duration,

  /// How many findings the detectors reported.
  findings,

  /// What decided: a rule, a person, the clock, the passthrough.
  rule,

  /// Whether the request went out edited.
  edited;

  /// Width in logical pixels; [path] takes whatever is left above
  /// [historyPathMinWidth].
  double get width => switch (this) {
    HistoryColumn.time => 72,
    HistoryColumn.method => 64,
    HistoryColumn.host => 220,
    HistoryColumn.path => historyPathMinWidth,
    HistoryColumn.status => 56,
    HistoryColumn.size => 72,
    HistoryColumn.duration => 64,
    HistoryColumn.findings => 64,
    HistoryColumn.rule => 140,
    HistoryColumn.edited => HSize.rowHistory,
  };

  /// True for the one column that takes the leftover width.
  bool get flexible => this == HistoryColumn.path;

  /// True when the numbers in this column are read against each other and
  /// therefore end-aligned.
  bool get numeric => switch (this) {
    HistoryColumn.status ||
    HistoryColumn.size ||
    HistoryColumn.duration ||
    HistoryColumn.findings => true,
    _ => false,
  };

  /// The sort key a click on this header selects, or null when the recorder
  /// cannot order by this column.
  HistorySort? get sort => switch (this) {
    HistoryColumn.time => HistorySort.time,
    HistoryColumn.host => HistorySort.host,
    HistoryColumn.size => HistorySort.size,
    HistoryColumn.duration => HistorySort.duration,
    _ => null,
  };
}

/// Padding to the right of a cell's content, so two columns never touch.
const double historyCellGap = HSpace.x2;

/// Width of the state glyph slot, the one `HRow` fills.
const double historyStateSlot = HSize.rowHistory;

/// Width of everything left of the first column.
///
/// The same arithmetic `HRow` does: the rail, a gap, the glyph slot, a gap.
/// The header repeats it so that column and heading stand in one line; if the
/// row's layout ever changes, this constant is where the header follows.
const double historyRowLeading =
    HSize.stateRail + HSpace.x2 + historyStateSlot + HSpace.x2;

/// Width of everything right of the last column.
///
/// A gap, the action slot `HRow` always reserves, and the 12 px right gutter
/// every pane ends with (`docs/UX.md` 3.2).
const double historyRowTrailing = HSpace.x2 + HSize.rowActionSlot + HSpace.x3;

/// The width the table needs when the path column is at its minimum.
final double historyMinTableWidth =
    historyRowLeading +
    HistoryColumn.values.fold<double>(
      0,
      (double sum, HistoryColumn column) => sum + column.width,
    ) +
    historyRowTrailing;

/// How wide the table is inside [available] pixels.
///
/// Never below [historyMinTableWidth]: overwidth becomes gutter, underwidth
/// becomes a horizontal scroll. A table that wrapped would move the byte
/// offsets a person compares (`docs/UX.md` 3.2).
double historyTableWidth(double available) =>
    available > historyMinTableWidth ? available : historyMinTableWidth;

/// The width of [column] inside a table of [tableWidth] pixels.
double historyColumnWidth(HistoryColumn column, double tableWidth) {
  if (!column.flexible) {
    return column.width;
  }
  final double extra = tableWidth - historyMinTableWidth;
  return historyPathMinWidth + (extra > 0 ? extra : 0);
}
