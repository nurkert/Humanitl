/// The detail half of the history screen: everything the daemon recorded
/// about one flow.
///
/// The head is built from the row that is already loaded, so selecting a row
/// answers "which request is this" in the same frame. Only the headers and
/// the bodies wait for the daemon, and they wait in their own place
/// (`docs/UX.md` 2.11).
library;

import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/services.dart';
// `Flow` is a domain type here, not the Flutter layout widget of the same
// name; the widget is never used in this feature.
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/domain/domain.dart';
import '../../core/ipc/daemon_client.dart';
import '../../core/ui/h_diagnostic_card.dart';
import '../../core/ui/ui.dart';
import '../../l10n/l10n.dart';
import 'history_filter_bar.dart';
import 'history_metrics.dart';
import 'history_view.dart';
import 'providers/history_detail.dart';

/// Which side of one flow the detail shows.
enum HistoryDetailTab {
  /// The request as it arrived.
  request,

  /// The answer that came back.
  response,

  /// The request as the person changed it before sending.
  edited,
}

/// The detail of one flow.
class HistoryDetail extends ConsumerStatefulWidget {
  /// Creates the detail of [flow].
  const HistoryDetail({required this.flow, super.key});

  /// The row the detail belongs to.
  final Flow flow;

  @override
  ConsumerState<HistoryDetail> createState() => _HistoryDetailState();
}

class _HistoryDetailState extends ConsumerState<HistoryDetail> {
  HistoryDetailTab _tab = HistoryDetailTab.request;
  bool _copied = false;

  @override
  void didUpdateWidget(HistoryDetail oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.flow.id != widget.flow.id) {
      _tab = HistoryDetailTab.request;
      _copied = false;
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final Flow flow = widget.flow;
    final AsyncValue<FlowDetail> detail = ref.watch(
      historyDetailProvider(flow.id),
    );
    final bool hasEdited = flow.edited;
    return LayoutBuilder(
      builder: (BuildContext context, BoxConstraints constraints) => Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          // The head is capped and scrolls rather than pushing the tabs out of
          // the pane: the split can be dragged small, and at
          // `TextScaler.linear(2.0)` the six facts alone are taller than a
          // short pane (`docs/UX.md` 6, no fixed height around text).
          ConstrainedBox(
            constraints: BoxConstraints(maxHeight: constraints.maxHeight * 0.5),
            child: SingleChildScrollView(child: _Head(flow: flow)),
          ),
          const HHairline(),
          Padding(
            padding: EdgeInsets.symmetric(
              horizontal: tokens.spacing.x3,
              vertical: tokens.spacing.x2,
            ),
            child: Row(
              children: <Widget>[
                for (final HistoryDetailTab tab in HistoryDetailTab.values)
                  if (tab != HistoryDetailTab.edited || hasEdited)
                    Padding(
                      padding: EdgeInsets.only(right: tokens.spacing.x2),
                      child: HButton(
                        key: Key('history-tab-${tab.name}'),
                        variant: _tab == tab
                            ? HButtonVariant.secondary
                            : HButtonVariant.ghost,
                        onPressed: () => setState(() => _tab = tab),
                        child: Text(_tabLabel(l10n, tab)),
                      ),
                    ),
              ],
            ),
          ),
          Flexible(
            child: switch (detail) {
              AsyncData<FlowDetail>(:final FlowDetail value) => _TabBody(
                tab: _tab,
                detail: value,
                copied: _copied,
                onCopy: (String text) {
                  unawaited(Clipboard.setData(ClipboardData(text: text)));
                  setState(() => _copied = true);
                },
              ),
              AsyncError<FlowDetail>(:final Object error) => Padding(
                padding: EdgeInsets.all(tokens.spacing.x3),
                child: _Failure(error: error),
              ),
              _ => const HistoryWaitGate(child: _BodySkeleton(lines: 10)),
            },
          ),
        ],
      ),
    );
  }

  String _tabLabel(AppLocalizations l10n, HistoryDetailTab tab) =>
      switch (tab) {
        HistoryDetailTab.request => l10n.historyDetailTabRequest,
        HistoryDetailTab.response => l10n.historyDetailTabResponse,
        HistoryDetailTab.edited => l10n.historyDetailTabEdited,
      };
}

/// The head: the URL, and the six facts that belong to it.
///
/// The URL is the largest type on this screen (`docs/UX.md` 3.1) and is
/// selectable, because comparing it against a rule is what a person does
/// here. `SelectableRegion` is the widgets-layer equivalent of
/// `SelectableText`; this application has no Material.
class _Head extends StatelessWidget {
  const _Head({required this.flow});

  final Flow flow;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final HFlowState state = historyVisualState(flow);
    final String unknown = l10n.historyUnknownValue;
    return Padding(
      padding: EdgeInsets.all(tokens.spacing.x3),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Row(
            children: <Widget>[
              // The one badge that carries the method hue: in the head there
              // is no rail beside it to be confused with (`docs/UX.md` 3.3,
              // rule 4).
              HMethodBadge(method: flow.methodLabel),
              SizedBox(width: tokens.spacing.x2),
              HStateGlyph(
                state: state,
                semanticsLabel: l10n.flowStateLabel(state),
              ),
              SizedBox(width: tokens.spacing.x1),
              Text(
                l10n.flowStateLabel(state),
                // The text-capable reading, never `stateColor`: that palette
                // is clamped to 3:1 as an area (`docs/UX.md` 6).
                style: tokens.typography.ui12.tinted(
                  tokens.stateTextColor(state),
                ),
              ),
            ],
          ),
          SizedBox(height: tokens.spacing.x1),
          SelectableRegion(
            selectionControls: emptyTextSelectionControls,
            child: Text(
              flow.url,
              style: tokens.typography.mono14.tinted(tokens.colors.fg0),
              maxLines: 2,
            ),
          ),
          SizedBox(height: tokens.spacing.x2),
          Wrap(
            spacing: tokens.spacing.x4,
            runSpacing: tokens.spacing.x1,
            children: <Widget>[
              _Fact(
                label: l10n.historyDetailReceived,
                value: formatHistoryTimestamp(flow.receivedAt),
              ),
              _Fact(
                label: l10n.historyDetailDecision,
                // What was decided, not how the row looks: an allow whose
                // upstream failed was still an allow, and the state is on
                // the glyph right above (`backlog/CONVENTIONS.md` 4.13).
                // A meta request has no decision and never will: the proxy
                // answered it itself, and a dash would read as "unknown"
                // (`backlog/CONVENTIONS.md` 4.13, HUM-103).
                value: switch (flow.decision) {
                  null when flow.meta => l10n.historyDecisionNone,
                  null => unknown,
                  final DecisionKind decision => l10n.flowStateLabel(
                    historyDecisionLabelState(decision),
                  ),
                },
              ),
              _Fact(label: l10n.historyDetailRule, value: _decider(l10n, flow)),
              _Fact(
                label: l10n.historyDetailDuration,
                value: formatHistoryDuration(flow, unknown: unknown),
              ),
              _Fact(
                label: l10n.historyDetailSize,
                value: historyResponseStreaming(flow)
                    // The answer is still running in, so the number is a
                    // running total and says so (`backlog/sprint-2.md`,
                    // HUM-032: bei Response `streaming` die Live-Größe).
                    ? '${formatHistorySizePair(flow, unknown: unknown)} · '
                          '${l10n.historyDetailStreaming}'
                    : formatHistorySizePair(flow, unknown: unknown),
              ),
              _Fact(
                label: l10n.historyDetailFindings,
                value: '${flow.findingCount}',
                findings: flow.findingCount > 0,
              ),
            ],
          ),
        ],
      ),
    );
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
        HistoryDecider.meta => l10n.historyDeciderMeta,
      };
}

/// One label and its value in the head.
class _Fact extends StatelessWidget {
  const _Fact({
    required this.label,
    required this.value,
    this.findings = false,
  });

  final String label;
  final String value;

  /// Puts the value in the findings colour: `stateTextColor`, the
  /// text-capable reading of the state palette, never the area colour
  /// (`docs/UX.md` 6).
  final bool findings;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        Text(label, style: tokens.typography.ui12.tinted(tokens.colors.fg1)),
        SizedBox(width: tokens.spacing.x1),
        Text(
          value,
          style: tokens.typography.mono12.tinted(
            findings
                ? tokens.stateTextColor(HFlowState.error)
                : tokens.colors.fg0,
          ),
        ),
      ],
    );
  }
}

/// Headers and body of one tab.
class _TabBody extends ConsumerWidget {
  const _TabBody({
    required this.tab,
    required this.detail,
    required this.copied,
    required this.onCopy,
  });

  final HistoryDetailTab tab;
  final FlowDetail detail;
  final bool copied;
  final ValueChanged<String> onCopy;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final AppLocalizations l10n = context.l10n;
    final List<Header> headers = switch (tab) {
      HistoryDetailTab.request => detail.request?.headers ?? const <Header>[],
      HistoryDetailTab.edited =>
        detail.editedRequest?.headers ?? const <Header>[],
      HistoryDetailTab.response => detail.response?.headers ?? const <Header>[],
    };
    final BodyRef? body = switch (tab) {
      HistoryDetailTab.request => detail.request?.body,
      HistoryDetailTab.edited => detail.editedRequest?.body,
      HistoryDetailTab.response => detail.responseBody,
    };
    final bool missing =
        tab == HistoryDetailTab.response && detail.response == null;
    return LayoutBuilder(
      builder: (BuildContext context, BoxConstraints constraints) {
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            ConstrainedBox(
              constraints: BoxConstraints(
                maxHeight: constraints.maxHeight * 0.45,
              ),
              child: _Headers(
                headers: headers,
                copied: copied,
                onCopy: onCopy,
                emptyLabel: missing
                    ? l10n.historyDetailNoResponse
                    : l10n.historyDetailNoHeaders,
              ),
            ),
            const HHairline(),
            Expanded(child: _Body(reference: body)),
          ],
        );
      },
    );
  }
}

/// The header table of one tab: name, value, and a copy control.
class _Headers extends StatefulWidget {
  const _Headers({
    required this.headers,
    required this.copied,
    required this.onCopy,
    required this.emptyLabel,
  });

  final List<Header> headers;
  final bool copied;
  final ValueChanged<String> onCopy;
  final String emptyLabel;

  @override
  State<_Headers> createState() => _HeadersState();
}

class _HeadersState extends State<_Headers> {
  bool _ascending = true;

  /// Width of the name column. Wide enough for `content-security-policy`,
  /// which is the longest header a person meets often.
  static const double _nameWidth = 180;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final List<Header> sorted = List<Header>.of(widget.headers)
      ..sort(
        (Header a, Header b) =>
            _ascending ? a.name.compareTo(b.name) : b.name.compareTo(a.name),
      );
    // One list, title row included: a column with a fixed head and a
    // scrolling tail overflows as soon as the head grows -- and at
    // `TextScaler.linear(2.0)` it does (`docs/UX.md` 6).
    return ListView.builder(
      padding: EdgeInsets.only(bottom: tokens.spacing.x2),
      itemCount: 1 + (sorted.isEmpty ? 1 : sorted.length),
      itemBuilder: (BuildContext context, int index) {
        if (index == 0) {
          return _title(tokens, l10n, sorted);
        }
        if (sorted.isEmpty) {
          return Padding(
            padding: EdgeInsets.symmetric(horizontal: tokens.spacing.x3),
            child: Text(
              widget.emptyLabel,
              style: tokens.typography.ui13.tinted(tokens.colors.fg1),
            ),
          );
        }
        final Header header = sorted[index - 1];
        return Padding(
          padding: EdgeInsets.symmetric(
            horizontal: tokens.spacing.x3,
            vertical: tokens.spacing.x1 / 2,
          ),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              SizedBox(
                width: _nameWidth,
                child: Text(
                  header.name,
                  style: tokens.typography.mono12.tinted(tokens.colors.fg1),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              SizedBox(width: tokens.spacing.x2),
              Expanded(
                child: Text(
                  header.text,
                  style: tokens.typography.mono12.tinted(tokens.colors.fg0),
                ),
              ),
            ],
          ),
        );
      },
    );
  }

  Widget _title(HTokens tokens, AppLocalizations l10n, List<Header> sorted) =>
      Padding(
        padding: EdgeInsets.fromLTRB(
          tokens.spacing.x3,
          tokens.spacing.x2,
          tokens.spacing.x3,
          tokens.spacing.x1,
        ),
        child: Row(
          children: <Widget>[
            HButton(
              variant: HButtonVariant.ghost,
              onPressed: () => setState(() => _ascending = !_ascending),
              semanticsLabel: _ascending
                  ? l10n.historySortedAscending(l10n.historyDetailHeaders)
                  : l10n.historySortedDescending(l10n.historyDetailHeaders),
              child: Text(l10n.historyDetailHeaders),
            ),
            const Spacer(),
            if (sorted.isNotEmpty)
              HButton(
                variant: HButtonVariant.ghost,
                onPressed: () => widget.onCopy(
                  <String>[
                    for (final Header header in sorted)
                      '${header.name}: ${header.text}',
                  ].join('\n'),
                ),
                child: Text(
                  widget.copied
                      ? l10n.historyDetailCopied
                      : l10n.historyDetailCopy,
                ),
              ),
          ],
        ),
      );
}

/// The recorded body of one tab.
class _Body extends ConsumerWidget {
  const _Body({required this.reference});

  final BodyRef? reference;

  /// Width of one monospace character at 12 px; the body scrolls sideways
  /// rather than wrapping, so its width has to be known before it is laid
  /// out (`docs/UX.md` 3.2). The advance is a token, not an estimate.
  static final double _charWidth = HSize.monoAdvance * HType.mono12.fontSize!;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final BodyRef? reference = this.reference;
    if (reference == null || reference.isEmpty) {
      return Padding(
        padding: EdgeInsets.all(tokens.spacing.x3),
        child: Text(
          l10n.historyDetailNoBody,
          style: tokens.typography.ui13.tinted(tokens.colors.fg1),
        ),
      );
    }
    final AsyncValue<HistoryBody> body = ref.watch(
      historyBodyProvider(reference),
    );
    return switch (body) {
      AsyncData<HistoryBody>(:final HistoryBody value) => _lines(
        context,
        tokens,
        l10n,
        value,
      ),
      AsyncError<HistoryBody>(:final Object error) => Padding(
        padding: EdgeInsets.all(tokens.spacing.x3),
        child: _Failure(error: error),
      ),
      _ => const HistoryWaitGate(child: _BodySkeleton(lines: 10)),
    };
  }

  Widget _lines(
    BuildContext context,
    HTokens tokens,
    AppLocalizations l10n,
    HistoryBody body,
  ) {
    if (body.binary) {
      return Padding(
        padding: EdgeInsets.all(tokens.spacing.x3),
        child: Text(
          l10n.historyDetailBinaryBody(
            formatHistoryCompactSize(body.byteCount),
          ),
          style: tokens.typography.ui13.tinted(tokens.colors.fg1),
        ),
      );
    }
    int longest = 0;
    for (final String line in body.lines) {
      longest = math.max(longest, line.length);
    }
    final double width = math.max(
      longest * _charWidth + tokens.spacing.x6,
      MediaQuery.sizeOf(context).width,
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        // Two causes, two sentences: the recorder stopping early is not the
        // same event as this view capping the lines it draws, and a sentence
        // that blamed the recorder for the view would send somebody looking
        // in the wrong place.
        if (body.truncated || body.linesCapped)
          Padding(
            padding: EdgeInsets.fromLTRB(
              tokens.spacing.x3,
              tokens.spacing.x1,
              tokens.spacing.x3,
              0,
            ),
            child: Text(
              // Both can be true at once, and then both are said: the
              // recorder stopped early *and* this view draws only the first
              // lines of what it kept.
              <String>[
                if (body.truncated)
                  l10n.historyDetailBodyTruncated(
                    formatHistoryCompactSize(body.byteCount),
                  ),
                if (body.linesCapped)
                  l10n.historyDetailLinesCapped(
                    l10n.historyTotalExact(body.lines.length),
                  ),
              ].join(' '),
              style: tokens.typography.ui12.tinted(
                tokens.stateTextColor(HFlowState.error),
              ),
            ),
          ),
        Expanded(
          child: SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: SizedBox(
              width: width,
              child: ListView.builder(
                itemExtent: historyBodyRowHeight,
                itemCount: body.lines.length,
                itemBuilder: (BuildContext context, int index) => Padding(
                  padding: EdgeInsets.symmetric(horizontal: tokens.spacing.x3),
                  child: Align(
                    alignment: Alignment.centerLeft,
                    child: Text(
                      body.lines[index],
                      style: tokens.typography.mono12.tinted(tokens.colors.fg0),
                      maxLines: 1,
                      softWrap: false,
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }
}

/// Holds a waiting display back until waiting is worth showing.
///
/// Nothing for the first [HMotion.waitVisible] (150 ms), because a display
/// shorter than a reaction time reads as a flicker (`docs/UX.md` 2.11). What
/// this gate does not do is hold the display for [HMotion.waitMinVisible]
/// once it is up; that half of the rule lives where the waiting ends, in the
/// table's own gate, because a `FutureProvider` swaps its child in one frame
/// and a wrapper cannot delay a sibling.
class HistoryWaitGate extends StatefulWidget {
  /// Wraps the waiting display [child].
  const HistoryWaitGate({required this.child, super.key});

  /// What is shown once waiting is admitted.
  final Widget child;

  @override
  State<HistoryWaitGate> createState() => _HistoryWaitGateState();
}

class _HistoryWaitGateState extends State<HistoryWaitGate> {
  bool _visible = false;
  Timer? _appear;

  @override
  void initState() {
    super.initState();
    _appear = Timer(HMotion.waitVisible, () {
      if (mounted) {
        setState(() => _visible = true);
      }
    });
  }

  @override
  void dispose() {
    _appear?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) =>
      _visible ? widget.child : const SizedBox.expand();
}

/// Hairlines in the body density while the body is on its way.
class _BodySkeleton extends StatelessWidget {
  const _BodySkeleton({required this.lines});

  final int lines;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    // A list, not a column: the skeleton stands for lines that scroll, and a
    // column of them would overflow a short pane instead of being cut off
    // where the real lines are. The four lengths are fractions of the pane,
    // not pixel counts, so the sketch stays a sketch at any width.
    const List<double> shares = <double>[0.7, 0.4, 0.85, 0.55];
    return ExcludeSemantics(
      child: LayoutBuilder(
        builder: (BuildContext context, BoxConstraints constraints) =>
            ListView.builder(
              padding: EdgeInsets.all(tokens.spacing.x3),
              physics: const NeverScrollableScrollPhysics(),
              itemExtent: historyBodyRowHeight,
              itemCount: lines,
              itemBuilder: (BuildContext context, int index) => Align(
                alignment: Alignment.centerLeft,
                child: HHairline(
                  color: HColorDerivation.fade(tokens.colors.fg2, 0.4),
                  length:
                      (constraints.maxWidth - tokens.spacing.x6) *
                      shares[index % shares.length],
                ),
              ),
            ),
      ),
    );
  }
}

/// A failed load, anchored where the content would have stood.
class _Failure extends StatelessWidget {
  const _Failure({required this.error});

  final Object error;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final Object error = this.error;
    final Diagnostic? diagnostic = error is DaemonException
        ? error.diagnostic
        : null;
    if (diagnostic == null) {
      return Text(
        l10n.historyDetailFailedTitle,
        style: tokens.typography.ui13.tinted(
          tokens.stateTextColor(HFlowState.error),
        ),
      );
    }
    return HDiagnosticCard(
      code: diagnostic.code,
      severityLabel: historySeverityLabel(l10n, diagnostic.severity),
      color: historySeverityColor(tokens, diagnostic.severity),
      title: l10n.historyDetailFailedTitle,
      why: diagnostic.why,
      docsUrl: diagnostic.docsUrl,
      width: double.infinity,
    );
  }
}
