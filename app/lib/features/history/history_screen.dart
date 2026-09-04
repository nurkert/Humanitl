/// The History section: filter, table, detail.
///
/// The screen owns three things and no more: the keyboard, the split between
/// table and detail, and what a double click does with a row. Everything else
/// is in the four widgets below it and in the two providers.
///
/// There is no shared transition into this list. `docs/UX.md` 2.9b allows the
/// `Hero` from a decided card into the history row only while both screens
/// stand on the screen at once; the shell shows one section at a time in an
/// `IndexedStack`, so the condition is not met and the transition is left out
/// rather than faked.
library;

import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
// `Flow` is a domain type here, not the Flutter layout widget of the same
// name; the widget is never used in this feature.
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/domain/domain.dart';
import '../../core/ipc/flow_handoff.dart';
import '../../core/shortcuts/intents.dart';
import '../../core/ui/h_diagnostic_card.dart';
import '../../core/ui/ui.dart';
import '../../l10n/l10n.dart';
import 'history_detail.dart';
import 'history_export_menu.dart';
import 'history_filter_bar.dart';
import 'history_table.dart';
import 'history_view.dart';
import 'providers/history_detail.dart';
import 'providers/history_page.dart';

/// How often the screen has asked for the keyboard.
///
/// Claiming it in every build would pull it back out of the shell's rail one
/// frame after somebody tabbed there; the counter is what a test can hold
/// against that, because the defect is invisible from inside this screen —
/// its own focus node counts a focused child as focused.
@visibleForTesting
int debugHistoryFocusClaims = 0;

/// The bindings of the History section.
///
/// Every activator here has an action in [HistoryScreen]; a widget test
/// compares the two sets, because a bound key that does nothing is worse than
/// an unbound one (`docs/UX.md` 5.3).
Map<ShortcutActivator, Intent>
historyShortcuts() => <ShortcutActivator, Intent>{
  const SingleActivator(LogicalKeyboardKey.enter): const OpenFlowIntent(),
  const SingleActivator(LogicalKeyboardKey.numpadEnter): const OpenFlowIntent(),
  const SingleActivator(LogicalKeyboardKey.slash): const FilterIntent(),
  const SingleActivator(LogicalKeyboardKey.keyJ): const NextFlowIntent(),
  const SingleActivator(LogicalKeyboardKey.arrowDown): const NextFlowIntent(),
  const SingleActivator(LogicalKeyboardKey.keyK): const PrevFlowIntent(),
  const SingleActivator(LogicalKeyboardKey.arrowUp): const PrevFlowIntent(),
};

/// Opens the selected row: a held request in the queue, anything else in the
/// sheet. The keyboard equivalent of the double click (`docs/UX.md` 5.1).
///
/// Local to this screen rather than in `core/shortcuts`: it means nothing
/// anywhere else, and a shared intent without a second user is a guess about
/// the future.
class OpenFlowIntent extends Intent {
  /// Creates the intent.
  const OpenFlowIntent();
}

/// How much of the height the detail takes by default.
const double historyDetailShare = 0.4;

/// The smallest share either half of the split can be squeezed to.
const double historySplitMin = 0.2;

/// Width of the detail sheet a double click opens.
const double historySheetWidth = 520;

/// The History section.
class HistoryScreen extends ConsumerStatefulWidget {
  /// Creates the section.
  const HistoryScreen({this.exportOpen = false, super.key});

  /// Opens the export modal at once. Only a golden passes it: a modal that a
  /// test has to click open cannot be photographed in one frame.
  final bool exportOpen;

  @override
  ConsumerState<HistoryScreen> createState() => _HistoryScreenState();
}

class _HistoryScreenState extends ConsumerState<HistoryScreen> {
  final FocusNode _filterFocus = FocusNode(debugLabel: 'history-filter');
  final FocusNode _tableFocus = FocusNode(debugLabel: 'history-table');
  final GlobalKey<HistoryTableState> _tableKey = GlobalKey<HistoryTableState>(
    debugLabel: 'history-table',
  );

  Flow? _sheetFlow;
  late bool _exportOpen = widget.exportOpen;

  late final Map<Type, Action<Intent>> _actions = <Type, Action<Intent>>{
    OpenFlowIntent: _SingleKeyAction<OpenFlowIntent>(_openSelected),
    FilterIntent: _SingleKeyAction<FilterIntent>(_focusFilter),
    NextFlowIntent: _SingleKeyAction<NextFlowIntent>(() => _move(1)),
    PrevFlowIntent: _SingleKeyAction<PrevFlowIntent>(() => _move(-1)),
  };

  @override
  void dispose() {
    _filterFocus.dispose();
    _tableFocus.dispose();
    super.dispose();
  }

  /// True while this section is the visible branch of the shell's stack.
  bool _visible = false;

  /// Takes the keyboard the first time the section becomes visible again.
  void _claimFocusOnceVisible(bool visible) {
    if (visible == _visible) {
      return;
    }
    _visible = visible;
    if (!visible) {
      return;
    }
    debugHistoryFocusClaims++;
    WidgetsBinding.instance.addPostFrameCallback((Duration _) {
      if (mounted && _visible && !_tableFocus.hasFocus) {
        _tableFocus.requestFocus();
      }
    });
  }

  void _focusFilter() => _filterFocus.requestFocus();

  void _move(int delta) => _tableKey.currentState?.moveSelection(delta);

  /// What a double click does with [flow].
  ///
  /// A held request belongs on the screen where it can be decided, so the
  /// history asks for it to be shown there and the shell carries the request
  /// out: a feature may not reach into another feature, and the shell is what
  /// composes the sections (ARCHITECTURE 5). Anything else is finished, and a
  /// sheet is the place to read it at full height.
  void _open(Flow flow) {
    if (flow.isHeld) {
      ref.read(flowHandoffProvider.notifier).request(flow.id);
      return;
    }
    ref.read(historySelectionProvider.notifier).select(flow.id);
    // The sheet keeps the focus with itself and closes on `Escape`, but only
    // once the focus is inside it; the table is holding it right now.
    _tableFocus.unfocus();
    setState(() => _sheetFlow = flow);
  }

  /// Opens the selected row, the way a double click would.
  void _openSelected() {
    final FlowId? id = ref.read(historySelectionProvider);
    if (id == null) {
      return;
    }
    final Flow? flow = ref
        .read(historyPageProvider)
        .rows
        .where((Flow row) => row.id == id)
        .firstOrNull;
    if (flow != null) {
      _open(flow);
    }
  }

  void _closeSheet() {
    setState(() => _sheetFlow = null);
    _tableFocus.requestFocus();
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final Diagnostic? failure = ref.watch(
      historyPageProvider.select((HistoryPageState page) => page.failure),
    );
    // A refused filter is answered under the filter field, where it was
    // caused; everything else is a data error and gets the banner over the
    // list (`docs/UX.md` 4.4).
    final bool dataError =
        failure != null && failure.code != historyFilterInvalidCode;
    // The section that is on screen owns the keyboard. The shell builds all
    // five at once in an `IndexedStack`, so an `autofocus` here would take
    // the focus away from the queue at start-up. Which section that is comes
    // from the shell, and a feature cannot ask the shell; `TickerMode` is on
    // exactly for the visible branch of an `IndexedStack`, so it answers the
    // same question without the import.
    //
    // Claimed once per becoming visible, never on every build: a focus that
    // is taken back in the next frame is a focus nobody can move away, and
    // the rail of the shell is one Tab away.
    _claimFocusOnceVisible(TickerMode.valuesOf(context).enabled);
    final FlowId? selected = ref.watch(historySelectionProvider);
    final Flow? selectedFlow = selected == null
        ? null
        : ref.watch(
            historyPageProvider.select(
              (HistoryPageState page) =>
                  page.rows.where((Flow row) => row.id == selected).firstOrNull,
            ),
          );
    final Flow? sheetFlow = _sheetFlow;
    return Shortcuts(
      shortcuts: historyShortcuts(),
      child: Actions(
        actions: _actions,
        child: Focus(
          focusNode: _tableFocus,
          child: Stack(
            children: <Widget>[
              ColoredBox(
                color: tokens.colors.bg0,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[
                    HistoryFilterBar(
                      focusNode: _filterFocus,
                      trailing: HistoryExportButton(
                        onOpen: () => setState(() => _exportOpen = true),
                      ),
                    ),
                    const HHairline(),
                    if (dataError)
                      Padding(
                        padding: EdgeInsets.all(tokens.spacing.x3),
                        child: HDiagnosticCard(
                          code: failure.code,
                          severityLabel: historySeverityLabel(
                            l10n,
                            failure.severity,
                          ),
                          color: historySeverityColor(tokens, failure.severity),
                          title: l10n.historyLoadFailedTitle,
                          why: failure.why,
                          docsUrl: failure.docsUrl,
                          width: double.infinity,
                          fix: HButton(
                            variant: HButtonVariant.secondary,
                            onPressed: () => unawaited(
                              ref.read(historyPageProvider.notifier).reload(),
                            ),
                            child: Text(l10n.historyReload),
                          ),
                        ),
                      ),
                    Expanded(
                      child: _VerticalSplit(
                        // A click into the table gives it the keyboard, the
                        // way a desktop list does; `J` and `K` then work
                        // without a detour over the rail.
                        top: Listener(
                          onPointerDown: (PointerDownEvent _) =>
                              _tableFocus.requestFocus(),
                          child: HistoryTable(key: _tableKey, onOpen: _open),
                        ),
                        bottom: selectedFlow == null
                            ? const _DetailPlaceholder()
                            : HistoryDetail(
                                key: ValueKey<String>(selectedFlow.id.value),
                                flow: selectedFlow,
                              ),
                      ),
                    ),
                  ],
                ),
              ),
              if (_exportOpen)
                HistoryExportModal(
                  onClose: () {
                    setState(() => _exportOpen = false);
                    _tableFocus.requestFocus();
                  },
                ),
              if (sheetFlow != null)
                Positioned(
                  top: 0,
                  right: 0,
                  bottom: 0,
                  child: _SlideInSheet(
                    // `HSheet` keeps the focus with itself and closes on
                    // `Escape` once `onClose` is set; a second focus scope
                    // and a second Escape binding over it would only be two.
                    child: HSheet(
                      title: Text(
                        l10n.historySheetTitle(
                          sheetFlow.methodLabel,
                          sheetFlow.host,
                        ),
                      ),
                      closeSemanticsLabel: l10n.historySheetClose,
                      onClose: _closeSheet,
                      width: historySheetWidth,
                      child: HistoryDetail(flow: sheetFlow),
                    ),
                  ),
                ),
            ],
          ),
        ),
      ),
    );
  }
}

/// An action that steps aside while somebody types.
///
/// `Shortcuts` maps a single letter before any action lookup runs, so a bound
/// letter would swallow the keystroke inside the filter field. A disabled
/// action makes `ShortcutManager.handleKeypress` return `ignored`, and the
/// key reaches the field (`docs/UX.md` 5.2).
class _SingleKeyAction<T extends Intent> extends Action<T> {
  _SingleKeyAction(this.run);

  final VoidCallback run;

  /// False while somebody types, and false while the focus sits on a control
  /// that would handle the key itself. A disabled action lets
  /// `ShortcutManager.handleKeypress` answer `ignored`, the key falls through
  /// to the default bindings of `WidgetsApp`, and the focused control wins
  /// (`docs/UX.md` 5.2).
  @override
  bool get isActionEnabled => !isTextInputFocused() && !_focusTakesActivate();

  static bool _focusTakesActivate() {
    final BuildContext? context = FocusManager.instance.primaryFocus?.context;
    if (context == null) {
      return false;
    }
    return Actions.maybeFind<ActivateIntent>(
          context,
          intent: const ActivateIntent(),
        ) !=
        null;
  }

  @override
  Object? invoke(T intent) {
    run();
    return null;
  }
}

/// The detail area before a row is selected.
class _DetailPlaceholder extends StatelessWidget {
  const _DetailPlaceholder();

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return Padding(
      padding: EdgeInsets.all(tokens.spacing.x3),
      child: Align(
        alignment: Alignment.topLeft,
        child: Text(
          l10n.historyDetailEmptyTitle,
          style: tokens.typography.ui13.tinted(tokens.colors.fg1),
        ),
      ),
    );
  }
}

/// Table over detail, with a splitter between them.
///
/// The share lives in a [ValueNotifier] that only this widget listens to: a
/// state write per pointer move would rebuild the whole screen at the frame
/// rate of the mouse (`docs/UX.md` 7). The drag follows the pointer one to
/// one; only the freed gap animates, and here nothing does (2.9).
class _VerticalSplit extends StatefulWidget {
  const _VerticalSplit({required this.top, required this.bottom});

  final Widget top;
  final Widget bottom;

  @override
  State<_VerticalSplit> createState() => _VerticalSplitState();
}

class _VerticalSplitState extends State<_VerticalSplit> {
  final ValueNotifier<double> _share = ValueNotifier<double>(
    historyDetailShare,
  );
  bool _dragging = false;

  @override
  void dispose() {
    _share.dispose();
    super.dispose();
  }

  /// Moves the split by [pixels] of the [height] available.
  void _move(double pixels, double height) {
    final double next = _share.value - pixels / math.max(height, 1);
    _share.value = next.clamp(historySplitMin, 1 - historySplitMin);
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return LayoutBuilder(
      builder: (BuildContext context, BoxConstraints constraints) {
        final double height = constraints.maxHeight;
        return ValueListenableBuilder<double>(
          valueListenable: _share,
          builder: (BuildContext context, double share, Widget? _) {
            final double detail = height * share;
            return Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Expanded(child: widget.top),
                _Splitter(
                  key: const Key('history-splitter'),
                  label: l10n.historySplitterLabel,
                  active: _dragging,
                  color: tokens.colors.line,
                  onStart: () => setState(() => _dragging = true),
                  onUpdate: (double pixels) => _move(pixels, height),
                  onEnd: () => setState(() => _dragging = false),
                ),
                SizedBox(height: detail, child: widget.bottom),
              ],
            );
          },
        );
      },
    );
  }
}

/// Nudges the split by one step.
class _NudgeIntent extends Intent {
  const _NudgeIntent(this.direction);

  /// -1 up, 1 down.
  final double direction;
}

/// The handle between table and detail.
///
/// The vertical counterpart of the splitter in `core/ui/h_resizable_panes`,
/// down to the focus ring and the arrow keys: every pointer gesture has a key
/// (`docs/UX.md` 5.1), and the height is [HSize.splitter], not a spacing
/// token that happens to be the same number.
class _Splitter extends StatefulWidget {
  const _Splitter({
    required this.label,
    required this.active,
    required this.color,
    required this.onStart,
    required this.onUpdate,
    required this.onEnd,
    super.key,
  });

  final String label;
  final bool active;
  final Color color;
  final VoidCallback onStart;
  final ValueChanged<double> onUpdate;
  final VoidCallback onEnd;

  @override
  State<_Splitter> createState() => _SplitterState();
}

class _SplitterState extends State<_Splitter> {
  bool _focused = false;

  void _nudge(double direction) {
    widget
      ..onStart()
      ..onUpdate(direction * HSize.splitterStep)
      ..onEnd();
  }

  @override
  Widget build(BuildContext context) {
    final bool marked = widget.active || _focused;
    return Semantics(
      label: widget.label,
      slider: true,
      child: FocusableActionDetector(
        mouseCursor: SystemMouseCursors.resizeRow,
        onFocusChange: (bool value) => setState(() => _focused = value),
        shortcuts: <ShortcutActivator, Intent>{
          const SingleActivator(LogicalKeyboardKey.arrowUp): const _NudgeIntent(
            -1,
          ),
          const SingleActivator(LogicalKeyboardKey.arrowDown):
              const _NudgeIntent(1),
        },
        actions: <Type, Action<Intent>>{
          _NudgeIntent: CallbackAction<_NudgeIntent>(
            onInvoke: (_NudgeIntent intent) {
              _nudge(intent.direction);
              return null;
            },
          ),
        },
        child: HFocusRing.inline(
          visible: _focused,
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            dragStartBehavior: DragStartBehavior.down,
            onVerticalDragStart: (DragStartDetails _) => widget.onStart(),
            onVerticalDragUpdate: (DragUpdateDetails d) =>
                widget.onUpdate(d.delta.dy),
            onVerticalDragEnd: (DragEndDetails _) => widget.onEnd(),
            onVerticalDragCancel: widget.onEnd,
            child: SizedBox(
              height: HSize.splitter,
              child: Center(
                child: SizedBox(
                  // No literal: the dragged line is twice the resting
                  // hairline (`docs/UX.md` 2.1).
                  height: marked ? HSize.splitterActive : HSize.hairline,
                  child: ColoredBox(color: widget.color),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// The sheet, arriving from the edge it hangs on.
///
/// 180 ms, `enter`, eight pixels from the right plus a fade; under reduced
/// motion the path is gone and the fade stays (`docs/UX.md` 2.2 and 2.10).
class _SlideInSheet extends StatefulWidget {
  const _SlideInSheet({required this.child});

  final Widget child;

  @override
  State<_SlideInSheet> createState() => _SlideInSheetState();
}

class _SlideInSheetState extends State<_SlideInSheet>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller = AnimationController(
    vsync: this,
    duration: HMotion.arrive,
  )..forward();

  late final CurvedAnimation _curve = CurvedAnimation(
    parent: _controller,
    curve: HMotion.enter,
  );

  @override
  void dispose() {
    _curve.dispose();
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    // The offset is a fraction of the sheet's own width, which is what
    // `SlideTransition` takes; eight logical pixels of a 520 px sheet.
    final double offset =
        HReducedMotion.distance(context, HMotion.arriveOffset) /
        historySheetWidth;
    return FadeTransition(
      opacity: _curve,
      child: SlideTransition(
        position: Tween<Offset>(
          begin: Offset(offset, 0),
          end: Offset.zero,
        ).animate(_curve),
        child: widget.child,
      ),
    );
  }
}
