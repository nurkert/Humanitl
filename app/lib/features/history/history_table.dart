/// The table: a pinned header and one row per recorded flow, virtualised over
/// a fixed row extent.
///
/// The list knows its row height, so scrolling ten thousand rows costs the
/// same as scrolling twenty (`docs/UX.md` 7). `docs/UX.md` 7 names
/// `FixedSpanExtent` of a `TableView` for this; the list here is a
/// `ListView.builder` with `itemExtent`, which is the same guarantee — the
/// extent is known before a row is built — and buys three things a
/// two-dimensional table cannot give: one `Semantics` node per row instead of
/// eleven, a hover and a selection that belong to the row rather than to a
/// cell, and one memoised widget per flow, which is what holds the rebuild
/// count (`docs/UX.md` 7, "Zwei Builds je Entscheidung"). The columns keep
/// their fixed widths; the row is a `Row` of `SizedBox`es over the same
/// metrics the header uses.
///
/// Nothing in this table wraps. Overwidth scrolls sideways, because a wrapped
/// table moves every line under the one a person is comparing (`docs/UX.md`
/// 3.2).
library;

import 'dart:async';
import 'dart:math' as math;

// `Flow` is a domain type here, not the Flutter layout widget of the same
// name; the widget is never used in this feature.
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/domain/domain.dart';
import '../../core/ui/focus_ring.dart';
import '../../core/ui/hover_label.dart';
import '../../core/ui/middle_ellipsis.dart';
import '../../core/ui/ui.dart';
import '../../l10n/l10n.dart';
import 'history_metrics.dart';
import 'history_view.dart';
import 'providers/history_detail.dart';
import 'providers/history_page.dart';
import 'providers/history_query.dart';

/// Width of one monospace character at 12 px, for the middle ellipsis of a
/// path. The advance is a token of the design system, not an estimate.
final double historyMonoCharWidth12 =
    HSize.monoAdvance * HType.mono12.fontSize!;

/// Width of one average character of the UI family at 13 px, for the middle
/// ellipsis of a host.
///
/// An estimate, and deliberately generous: what it lets through,
/// `TextOverflow.ellipsis` still catches, while a stingy estimate would cut a
/// host that fits. The UI family is proportional, so there is no advance
/// token for it; [HSize.monoAdvance] holds for the monospace family only.
const double historyUiCharWidth13 = 6.4;

/// At which fraction of the scroll extent the next page is asked for.
const double historyLoadMoreAt = 0.8;

/// How often any history row has run its `build`.
///
/// Rebuild scope is invisible to the compiler and to a golden, so it
/// regresses silently; a counter is the only way a test can hold it
/// (`docs/UX.md` 7). Tests reset it before the step they measure.
@visibleForTesting
int debugHistoryRowBuilds = 0;

/// The table.
class HistoryTable extends ConsumerStatefulWidget {
  /// Creates the table. [onOpen] runs on a double click on a row.
  const HistoryTable({required this.onOpen, super.key});

  /// What a double click does with a row.
  final void Function(Flow flow) onOpen;

  @override
  ConsumerState<HistoryTable> createState() => HistoryTableState();
}

/// The state of [HistoryTable]; public so that the screen can move the
/// selection from a keyboard action.
class HistoryTableState extends ConsumerState<HistoryTable> {
  final ScrollController _vertical = ScrollController();
  final ScrollController _horizontal = ScrollController();

  /// One widget instance per flow, so that a page arriving at the bottom
  /// leaves every row above it untouched: `Element.updateChild` stops at an
  /// identical child widget, and that is the only way to the rebuild numbers
  /// of `docs/UX.md` 7.
  final Map<String, _CachedRow> _rows = <String, _CachedRow>{};

  bool _atTop = true;

  @override
  void initState() {
    super.initState();
    _vertical.addListener(_scrolled);
  }

  @override
  void dispose() {
    _vertical
      ..removeListener(_scrolled)
      ..dispose();
    _horizontal.dispose();
    super.dispose();
  }

  void _scrolled() {
    if (!_vertical.hasClients) {
      return;
    }
    final ScrollPosition position = _vertical.position;
    final bool atTop = position.pixels <= 0;
    if (atTop != _atTop) {
      setState(() => _atTop = atTop);
      // The provider decides what follows: at the head an arrival joins the
      // rows, away from it it waits in the pill.
      ref.read(historyPageProvider.notifier).setAtHead(atTop);
    }
    final double extent = position.maxScrollExtent;
    if (extent > 0 && position.pixels >= extent * historyLoadMoreAt) {
      unawaited(ref.read(historyPageProvider.notifier).loadMore());
    }
  }

  /// Moves the selection by [delta] rows and keeps it in view.
  void moveSelection(int delta) {
    final List<Flow> rows = ref.read(historyPageProvider).rows;
    if (rows.isEmpty) {
      return;
    }
    final FlowId? current = ref.read(historySelectionProvider);
    final int index = rows.indexWhere((Flow row) => row.id == current);
    final int next = index < 0
        ? (delta > 0 ? 0 : rows.length - 1)
        : (index + delta).clamp(0, rows.length - 1);
    ref.read(historySelectionProvider.notifier).select(rows[next].id);
    _reveal(next);
  }

  void _reveal(int index) {
    if (!_vertical.hasClients) {
      return;
    }
    final double extent = _rowExtent(context);
    final double top = index * extent;
    final ScrollPosition position = _vertical.position;
    final double viewport = position.viewportDimension;
    if (top < position.pixels) {
      _vertical.jumpTo(math.max(0, top));
    } else if (top + extent > position.pixels + viewport) {
      _vertical.jumpTo(
        math.min(position.maxScrollExtent, top + extent - viewport),
      );
    }
  }

  /// The row height at the current text scale.
  ///
  /// The density of `docs/UX.md` 3.2 is a minimum, not a fixed height: at
  /// `TextScaler.linear(2.0)` a 12 px monospace line alone measures 32 px, and
  /// a fixed 28 would swallow the overflow silently (`docs/UX.md` 6).
  static double _rowExtent(BuildContext context) {
    final TextScaler scaler = MediaQuery.textScalerOf(context);
    final double line = scaler.scale(HType.mono12.fontSize!) * (16 / 12);
    final double padded = line + HSpace.x2;
    return math.max(historyRowHeight, padded);
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final HistoryPageState page = ref.watch(historyPageProvider);
    final HistoryQuery query = ref.watch(historyQueryProvider);
    // A filter the daemon refused is answered under the field, and the list
    // says nothing at all: "matches 0 of 1,284" beside "the filter cannot be
    // read" would claim a search that never ran.
    final bool refused = page.failure?.code == historyFilterInvalidCode;
    _evict(page.rows);
    return LayoutBuilder(
      builder: (BuildContext context, BoxConstraints constraints) {
        final double width = historyTableWidth(constraints.maxWidth);
        final double extent = _rowExtent(context);
        return SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          controller: _horizontal,
          child: SizedBox(
            width: width,
            height: constraints.maxHeight,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                _HeaderRow(width: width, query: query),
                const HHairline(),
                Expanded(
                  child: refused
                      ? const SizedBox.expand()
                      : page.isEmpty
                      ? _EmptyBody(page: page, query: query)
                      : Stack(
                          children: <Widget>[
                            // Both thresholds of `docs/UX.md` 2.11 live in
                            // `HWait`, so no screen invents them again
                            // (`docs/UX.md` 9, Punkt 19).
                            HWait(
                              loading: page.loading && page.rows.isEmpty,
                              skeleton: const HSkeleton(
                                rows: _skeletonCount,
                                rowHeight: HSize.rowHistory,
                              ),
                              child: Semantics(
                                label: l10n.historyTableLabel,
                                child: ListView.builder(
                                  key: const Key('history-list'),
                                  controller: _vertical,
                                  itemExtent: extent,
                                  itemCount: page.rows.length,
                                  itemBuilder: (
                                    BuildContext context,
                                    int index,
                                  ) => _row(page.rows[index], width),
                                ),
                              ),
                            ),
                            // Arrivals the list could not place itself: at
                            // the head an unfiltered arrival joins the rows
                            // directly and never gets here.
                            if (page.waiting > 0)
                              Positioned(
                                top: tokens.spacing.x1,
                                left: 0,
                                right: 0,
                                child: Center(
                                  child: _NewRowsPill(
                                    count: page.waiting,
                                    onMerge: () {
                                      final HistoryPageNotifier notifier = ref
                                          .read(historyPageProvider.notifier);
                                      // Under a filter only the recorder can
                                      // say what matches, so the pill asks
                                      // it; otherwise the rows are already
                                      // here and only have to be moved up.
                                      if (page.missed > 0) {
                                        unawaited(notifier.refresh());
                                      } else {
                                        notifier.merge();
                                      }
                                      if (_vertical.hasClients) {
                                        _vertical.jumpTo(0);
                                      }
                                    },
                                  ),
                                ),
                              ),
                          ],
                        ),
                ),
                // The next page is sketched where it will stand: under the
                // last loaded row (`docs/UX.md` 2.11).
                if (!refused && page.rows.isNotEmpty)
                  HWait(
                    loading: page.loadingMore,
                    skeleton: const HSkeleton(
                      rows: _moreSkeletonCount,
                      rowHeight: HSize.rowHistory,
                    ),
                    child: const SizedBox.shrink(),
                  ),
                if (!refused) ...<Widget>[
                  const HHairline(),
                  _Footer(page: page),
                ],
              ],
            ),
          ),
        );
      },
    );
  }

  /// How many skeleton rows stand in for the page on its way.
  ///
  /// The page the daemon returns is [historyPageSize] rows long, but a
  /// skeleton says "this much is coming and it will stand here"; more than a
  /// screenful of them says nothing further (`docs/UX.md` 2.11).
  static const int _skeletonCount = 12;

  /// How many rows are sketched under the list while the next page arrives.
  ///
  /// Fewer than the first screenful: the answer is coming to a place that is
  /// already full, and the sketch only has to say "more, here".
  static const int _moreSkeletonCount = 4;

  Widget _row(Flow flow, double width) {
    final _CachedRow? cached = _rows[flow.id.value];
    if (cached != null && cached.flow == flow && cached.width == width) {
      return cached.widget;
    }
    final Widget widget = HistoryRow(
      key: ValueKey<String>(flow.id.value),
      flow: flow,
      tableWidth: width,
      onOpen: this.widget.onOpen,
    );
    _rows[flow.id.value] = _CachedRow(flow, width, widget);
    return widget;
  }

  void _evict(List<Flow> rows) {
    if (_rows.length <= rows.length) {
      return;
    }
    final Set<String> live = <String>{
      for (final Flow flow in rows) flow.id.value,
    };
    _rows.removeWhere((String id, _CachedRow _) => !live.contains(id));
  }
}

class _CachedRow {
  const _CachedRow(this.flow, this.width, this.widget);

  final Flow flow;
  final double width;
  final Widget widget;
}

/// One row of the table.
///
/// The row itself is `HRow`: the design system owns the fill, the rail, the
/// focus ring, the action slot and the minimum height, so a table row and a
/// queue row cannot drift apart (`docs/UX.md` 3.4, 3.5, 9). What is left here
/// is the part a table has and a list does not — eleven columns of fixed
/// width, laid out over the same metrics as the pinned header.
///
/// It reads the selection itself, keyed to its own id, so that moving the
/// selection rebuilds two rows and not two hundred (`docs/UX.md` 7).
class HistoryRow extends ConsumerWidget {
  /// Creates a row.
  const HistoryRow({
    required this.flow,
    required this.tableWidth,
    required this.onOpen,
    super.key,
  });

  /// The flow this row shows.
  final Flow flow;

  /// The width the columns are laid out in.
  final double tableWidth;

  /// What a double click, or the affordance in the action slot, does.
  final void Function(Flow flow) onOpen;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    debugHistoryRowBuilds++;
    final AppLocalizations l10n = context.l10n;
    final HFlowState state = historyVisualState(flow);
    final String stateLabel = l10n.flowStateLabel(state);
    final bool selected = ref.watch(
      historySelectionProvider.select((FlowId? id) => id == flow.id),
    );
    // A double click opens; the single tap inside `HRow` selects. Both
    // recognisers join the same arena, so the first tap selects and the
    // second opens -- the behaviour of a desktop list.
    return GestureDetector(
      behavior: HitTestBehavior.deferToChild,
      onDoubleTap: () => onOpen(flow),
      child: HRow(
        state: state,
        minHeight: HSize.rowHistory,
        selected: selected,
        semanticsLabel: l10n.historyRowSemantics(
          stateLabel,
          flow.methodLabel,
          flow.host,
          flow.path,
        ),
        onTap: () =>
            ref.read(historySelectionProvider.notifier).select(flow.id),
        // A 28 px column has no room for the word, so semantics and the hover
        // label carry it -- always, in every view (`docs/UX.md` 6).
        stateGlyph: SizedBox(
          width: historyStateSlot,
          child: HoverLabel(
            label: stateLabel,
            child: Align(
              alignment: Alignment.centerLeft,
              child: HStateGlyph(state: state, semanticsLabel: stateLabel),
            ),
          ),
        ),
        // The pointer path to what a double click does, uncovered by hover
        // and by focus, never by hover alone (`docs/UX.md` 5.1). The label
        // names where it leads, because a held request goes to the queue and
        // everything else opens where it stands.
        actionSlot: HIconButton(
          glyph: HGlyph.chevronRight,
          onPressed: () => onOpen(flow),
          semanticsLabel: flow.isHeld
              ? l10n.historyOpenRowInQueue
              : l10n.historyOpenRow,
        ),
        title: Row(
          children: <Widget>[
            for (final HistoryColumn column in HistoryColumn.values)
              SizedBox(
                width: historyColumnWidth(column, tableWidth),
                child: Padding(
                  padding: EdgeInsets.only(right: historyCellGap),
                  child: _Cell(
                    column: column,
                    flow: flow,
                    state: state,
                    width:
                        historyColumnWidth(column, tableWidth) - historyCellGap,
                  ),
                ),
              ),
          ],
        ),
      ),
    );
  }
}

/// The content of one cell.
class _Cell extends StatelessWidget {
  const _Cell({
    required this.column,
    required this.flow,
    required this.state,
    required this.width,
  });

  final HistoryColumn column;
  final Flow flow;
  final HFlowState state;
  final double width;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final String unknown = l10n.historyUnknownValue;
    switch (column) {
      case HistoryColumn.time:
        return _text(
          tokens,
          formatHistoryTime(flow.receivedAt),
          tokens.typography.mono12,
          tokens.colors.fg1,
        );
      case HistoryColumn.method:
        // Neutral in a list: a reddish DELETE badge beside a red rail reads
        // as two blocks, not as a verb and a state (`docs/UX.md` 3.3, rule
        // 4). The hue belongs to the one badge in the detail head.
        return Align(
          alignment: Alignment.centerLeft,
          child: HMethodBadge(method: flow.methodLabel, neutral: true),
        );
      case HistoryColumn.host:
        return _text(
          tokens,
          middleEllipsis(flow.host, (width / historyUiCharWidth13).floor()),
          tokens.typography.ui13,
          tokens.colors.fg0,
        );
      case HistoryColumn.path:
        return _text(
          tokens,
          middleEllipsis(flow.path, (width / historyMonoCharWidth12).floor()),
          tokens.typography.mono12,
          tokens.colors.fg1,
        );
      case HistoryColumn.status:
        final bool failed = flow.status >= 400;
        return _number(
          tokens,
          l10n,
          formatHistoryStatus(flow, unknown: unknown),
          failed ? tokens.colors.fg0 : tokens.colors.fg1,
        );
      case HistoryColumn.size:
        return _number(
          tokens,
          l10n,
          formatHistorySizePair(flow, unknown: unknown),
          tokens.colors.fg1,
        );
      case HistoryColumn.duration:
        return _number(
          tokens,
          l10n,
          formatHistoryDuration(flow, unknown: unknown),
          tokens.colors.fg1,
        );
      case HistoryColumn.findings:
        // A zero is not printed: the column carries the one chroma a resting
        // row may have (`docs/UX.md` 4.7), and ten thousand grey zeros beside
        // it would be the loudest thing on the screen. The count is in the
        // semantics either way.
        //
        // The digits carry the colour, as `backlog/sprint-2.md` asks. They
        // may again: `tokens.stateTextColor` is the text-capable reading of
        // the state palette and measures 5,62:1 in the light theme and
        // 6,01:1 in the dark one, over the 4,5:1 `docs/UX.md` 6 demands. The
        // area colour `tokens.state.error` would not — it is clamped to 3:1.
        return Semantics(
          label: l10n.historyFindingsSemantics(flow.findingCount),
          excludeSemantics: true,
          child: flow.findingCount == 0
              ? const SizedBox.shrink()
              : _number(
                  tokens,
                  l10n,
                  '${flow.findingCount}',
                  tokens.stateTextColor(HFlowState.error),
                ),
        );
      case HistoryColumn.rule:
        return _text(
          tokens,
          _decider(l10n, flow),
          tokens.typography.mono11,
          tokens.colors.fg1,
        );
      case HistoryColumn.edited:
        if (!flow.edited) {
          return const SizedBox.shrink();
        }
        return Semantics(
          label: l10n.historyEditedMark,
          child: Align(
            child: SizedBox.square(
              dimension: HSpace.x1,
              child: DecoratedBox(
                decoration: BoxDecoration(
                  color: tokens.colors.accent,
                  shape: BoxShape.circle,
                ),
              ),
            ),
          ),
        );
    }
  }

  String _decider(AppLocalizations l10n, Flow flow) =>
      switch (historyDecider(flow)) {
        HistoryDecider.rule => l10n.historyDeciderRule(
          flow.ruleId == null ? '' : historyRuleShort(flow.ruleId!),
        ),
        HistoryDecider.manual => l10n.historyDeciderManual,
        HistoryDecider.timeout => l10n.historyDeciderTimeout,
        HistoryDecider.passthrough => l10n.historyDeciderPassthrough,
        HistoryDecider.pending => l10n.historyDeciderPending,
      };

  Widget _text(HTokens tokens, String value, TextStyle style, Color color) =>
      Align(
        alignment: Alignment.centerLeft,
        child: Text(
          value,
          style: style.tinted(color),
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          softWrap: false,
        ),
      );

  /// A number, end aligned so that two rows can be compared digit by digit,
  /// and never a bare em dash to a screen reader.
  Widget _number(
    HTokens tokens,
    AppLocalizations l10n,
    String value,
    Color color,
  ) => Align(
    alignment: Alignment.centerRight,
    // An em dash is a placeholder for the eye; a screen reader hears the
    // word, and nothing else, so it is never mistaken for a zero
    // (`backlog/CONVENTIONS.md` 4.13).
    child: Semantics(
      label: value == l10n.historyUnknownValue
          ? l10n.historyUnknownSemantics
          : null,
      excludeSemantics: value == l10n.historyUnknownValue,
      child: Text(
        value,
        style: tokens.typography.mono12.tinted(color),
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
        softWrap: false,
        textAlign: TextAlign.right,
      ),
    ),
  );
}

/// The pinned header.
class _HeaderRow extends ConsumerWidget {
  const _HeaderRow({required this.width, required this.query});

  final double width;
  final HistoryQuery query;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    return ColoredBox(
      color: tokens.colors.bg1,
      // A minimum, not a height: no box that holds text has a fixed one, or
      // it swallows the overflow silently at a larger text scale
      // (`docs/UX.md` 6).
      child: ConstrainedBox(
        constraints: const BoxConstraints(minHeight: historyHeaderHeight),
        child: Row(
          children: <Widget>[
            SizedBox(width: HSize.stateRail + HSpace.x2),
            SizedBox(
              width: historyStateSlot,
              child: _HeaderLabel(text: context.l10n.historyColumnState),
            ),
            SizedBox(width: HSpace.x2),
            for (final HistoryColumn column in HistoryColumn.values)
              SizedBox(
                width: historyColumnWidth(column, width),
                child: Padding(
                  padding: EdgeInsets.only(right: historyCellGap),
                  child: _HeaderCell(column: column, query: query),
                ),
              ),
            SizedBox(width: historyRowTrailing),
          ],
        ),
      ),
    );
  }
}

class _HeaderCell extends ConsumerStatefulWidget {
  const _HeaderCell({required this.column, required this.query});

  final HistoryColumn column;
  final HistoryQuery query;

  @override
  ConsumerState<_HeaderCell> createState() => _HeaderCellState();
}

/// The heading of a column that cannot be sorted by, the state column.
class _HeaderLabel extends StatelessWidget {
  const _HeaderLabel({required this.text});

  final String text;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Align(
      alignment: Alignment.centerLeft,
      child: Text(
        text,
        style: tokens.typography.ui11.medium.tinted(tokens.colors.fg1),
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
        softWrap: false,
      ),
    );
  }
}

class _HeaderCellState extends ConsumerState<_HeaderCell> {
  bool _focused = false;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final HistoryColumn column = widget.column;
    final HistorySort? sort = column.sort;
    final bool active = sort != null && widget.query.sort == sort;
    final String label = _label(l10n, column);
    final Widget text = Text(
      label,
      style: tokens.typography.ui11.medium.tinted(
        // `fg2` is reserved for controls that are really disabled; a heading
        // somebody reads is `fg1` or better (`docs/UX.md` 6).
        active ? tokens.colors.fg0 : tokens.colors.fg1,
      ),
      maxLines: 1,
      overflow: TextOverflow.ellipsis,
      softWrap: false,
    );
    final Widget content = Align(
      alignment: column.numeric ? Alignment.centerRight : Alignment.centerLeft,
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Flexible(child: text),
          if (active) ...<Widget>[
            SizedBox(width: HSpace.x1),
            // The chevron of the design system, turned: down for descending,
            // up for ascending. No new glyph for a direction that already has
            // one (`docs/UX.md` 2.1).
            Transform.rotate(
              angle: widget.query.descending ? _quarterTurn : -_quarterTurn,
              child: HGlyphIcon(
                HGlyph.chevronRight,
                size: HSpace.x3,
                color: tokens.colors.fg1,
              ),
            ),
          ],
        ],
      ),
    );
    if (sort == null) {
      return content;
    }
    return Semantics(
      button: true,
      label: active
          ? (widget.query.descending
                ? l10n.historySortedDescending(label)
                : l10n.historySortedAscending(label))
          : l10n.historySortBy(label),
      excludeSemantics: true,
      child: FocusableActionDetector(
        onFocusChange: (bool value) => setState(() => _focused = value),
        actions: <Type, Action<Intent>>{
          ActivateIntent: CallbackAction<ActivateIntent>(
            onInvoke: (ActivateIntent _) {
              ref.read(historyQueryProvider.notifier).orderBy(sort);
              return null;
            },
          ),
        },
        mouseCursor: SystemMouseCursors.click,
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: () => ref.read(historyQueryProvider.notifier).orderBy(sort),
          child: FocusRing(
            visible: _focused,
            radius: tokens.radii.badge,
            child: content,
          ),
        ),
      ),
    );
  }

  /// A quarter turn in radians; the chevron points right at rest.
  static const double _quarterTurn = 1.5707963267948966;

  String _label(AppLocalizations l10n, HistoryColumn column) =>
      switch (column) {
        HistoryColumn.time => l10n.historyColumnTime,
        HistoryColumn.method => l10n.historyColumnMethod,
        HistoryColumn.host => l10n.historyColumnHost,
        HistoryColumn.path => l10n.historyColumnPath,
        HistoryColumn.status => l10n.historyColumnStatus,
        HistoryColumn.size => l10n.historyColumnSize,
        HistoryColumn.duration => l10n.historyColumnDuration,
        HistoryColumn.findings => l10n.historyColumnFindings,
        HistoryColumn.rule => l10n.historyColumnRule,
        HistoryColumn.edited => l10n.historyColumnEdited,
      };
}

/// The pill over the first row: requests arrived while the list was elsewhere.
class _NewRowsPill extends StatelessWidget {
  const _NewRowsPill({required this.count, required this.onMerge});

  final int count;
  final VoidCallback onMerge;

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = context.l10n;
    return HButton(
      key: const Key('history-new-pill'),
      variant: HButtonVariant.secondary,
      onPressed: onMerge,
      child: Text(l10n.historyNewPill(count)),
    );
  }
}

/// The footer: how much of the match is loaded, and whether the count is
/// exact.
///
/// It keeps its line while the first page is on its way, empty rather than
/// absent: a footer that appeared with the answer would push the list up by
/// its own height in the frame the rows arrive, and nothing moves when a wait
/// ends (`docs/UX.md` 2.11).
class _Footer extends StatelessWidget {
  const _Footer({required this.page});

  final HistoryPageState page;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final bool silent = page.loading && page.rows.isEmpty;
    final String loaded = l10n.historyTotalExact(page.rows.length);
    // `capped` comes from the daemon, which knows whether it stopped counting
    // (`backlog/CONVENTIONS.md` 4.13).
    final String total = page.capped
        ? l10n.historyTotalAtLeast(page.total)
        : l10n.historyTotalExact(page.total);
    return Padding(
      padding: EdgeInsets.fromLTRB(
        historyRowLeading,
        HSpace.x1,
        historyRowTrailing,
        HSpace.x1,
      ),
      child: Align(
        alignment: Alignment.centerLeft,
        child: Text(
          silent
              ? ''
              : page.windowFull
              ? l10n.historyWindowFull(loaded, total)
              : l10n.historyLoadedOf(loaded, total),
          style: tokens.typography.ui12.tinted(tokens.colors.fg1),
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
        ),
      ),
    );
  }
}

/// What stands where the rows would be when there are none.
class _EmptyBody extends ConsumerWidget {
  const _EmptyBody({required this.page, required this.query});

  final HistoryPageState page;
  final HistoryQuery query;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    if (query.isUnfiltered && page.hiddenPassthrough) {
      // Not empty at all: the passthrough chip is hiding every row. The
      // agent calls its model first, so this is the ordinary case, and the
      // way back is the chip (`backlog/sprint-2.md`, Fallstricke).
      return Padding(
        key: const Key('history-empty-passthrough'),
        padding: EdgeInsets.all(tokens.spacing.x6),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            Flexible(
              child: Text(
                l10n.historyEmptyPassthroughOnly,
                style: tokens.typography.ui13.tinted(tokens.colors.fg0),
              ),
            ),
            SizedBox(width: tokens.spacing.x3),
            HButton(
              variant: HButtonVariant.ghost,
              onPressed: () => ref
                  .read(historyQueryProvider.notifier)
                  .toggle(HistoryChip.passthrough),
              child: Text(l10n.historyChipPassthroughHidden),
            ),
          ],
        ),
      );
    }
    if (query.isUnfiltered) {
      // Nothing has happened yet: the sentence names the next event, never
      // the absence (`docs/UX.md` 4.1).
      return Padding(
        padding: EdgeInsets.all(tokens.spacing.x6),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            Text(
              l10n.historyEmptyTitle,
              style: tokens.typography.ui16.semibold.tinted(tokens.colors.fg0),
            ),
            SizedBox(height: tokens.spacing.x2),
            Text(
              l10n.historyEmptyHint,
              style: tokens.typography.ui13.tinted(tokens.colors.fg1),
            ),
          ],
        ),
      );
    }
    // A filter cut the set away: the sentence names the filter, the hit count
    // and the way back, and the way back is a control (`docs/UX.md` 4.1).
    final String total = page.unfilteredTotal < 0
        ? ''
        : (page.unfilteredCapped
              ? l10n.historyTotalAtLeast(page.unfilteredTotal)
              : l10n.historyTotalExact(page.unfilteredTotal));
    return Padding(
      padding: EdgeInsets.all(tokens.spacing.x6),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.center,
        children: <Widget>[
          Flexible(
            child: Text(
              total.isEmpty
                  ? l10n.historyEmptyFilteredUnknownTotal(query.filter)
                  : l10n.historyEmptyFiltered(query.filter, total),
              style: tokens.typography.ui13.tinted(tokens.colors.fg1),
            ),
          ),
          SizedBox(width: tokens.spacing.x3),
          HButton(
            variant: HButtonVariant.ghost,
            onPressed: ref.read(historyQueryProvider.notifier).reset,
            child: Text(l10n.historyFilterReset),
          ),
        ],
      ),
    );
  }
}
