/// The Intercept section: the queue, the request card and the decision.
///
/// Three panes (28/44/28 with minimum widths 280/480/260), the action bar
/// under the card, and the keyboard vocabulary of CONVENTIONS 3.9 bound to
/// the intents of `core/shortcuts`. The screen holds no domain logic: it
/// reads providers and calls notifiers (ARCHITECTURE 5).
library;

import 'dart:async';

// `Flow` is a domain type here, not the Flutter layout widget of the same
// name; the widget is never used in this feature.
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/domain/domain.dart';
import '../../core/shortcuts/intents.dart';
import '../../core/ui/h_resizable_panes.dart';
import '../../core/ui/ui.dart';
import '../../l10n/l10n.dart';
import 'providers/flows.dart';
import 'providers/pane_layout.dart';
import 'widgets/action_bar.dart';
import 'widgets/domain_pane_placeholder.dart';
import 'widgets/queue_pane.dart';
import 'widgets/request_card.dart';

/// True while the shell actually paints this section.
///
/// The shell keeps every section built inside an `IndexedStack`, which wraps
/// each child in a `Visibility`. `Visibility.of` reads that wrapper and makes
/// the caller depend on it, so the screen is rebuilt when it is shown or
/// hidden -- and it keeps the check inside this feature: no feature imports
/// another one (ARCHITECTURE 5). Without such an ancestor -- a test that
/// mounts the screen alone -- the section counts as visible.
bool isSectionVisible(BuildContext context) => Visibility.of(context);

/// The Intercept section.
class InterceptScreen extends ConsumerStatefulWidget {
  /// Creates the section.
  const InterceptScreen({super.key});

  @override
  ConsumerState<InterceptScreen> createState() => _InterceptScreenState();
}

class _InterceptScreenState extends ConsumerState<InterceptScreen> {
  final FocusNode _focus = FocusNode(debugLabel: 'intercept');
  bool _visible = true;

  @override
  void initState() {
    super.initState();
    FocusManager.instance.addListener(_syncFocus);
    WidgetsBinding.instance.addPostFrameCallback((Duration _) => _syncFocus());
  }

  @override
  void dispose() {
    FocusManager.instance.removeListener(_syncFocus);
    _focus.dispose();
    super.dispose();
  }

  /// Keeps the keyboard where the person is looking.
  ///
  /// While the section shows, the screen takes the focus as soon as nothing
  /// more specific wants it: the shell parks the focus on its own root node
  /// at startup and again every time the command palette closes, and that
  /// node is an ancestor of this one, so taking over from it steals nothing.
  /// While the section is hidden the `IndexedStack` excludes this subtree
  /// from focus, which would leave the window with no focus at all and the
  /// shell without its own `Ctrl+1..5`; the focus is therefore handed back up.
  void _syncFocus() {
    if (!mounted) {
      return;
    }
    if (!_visible) {
      _handFocusBack();
      return;
    }
    if (_focus.hasFocus) {
      return;
    }
    final FocusNode? primary = FocusManager.instance.primaryFocus;
    if (primary == null ||
        primary == FocusManager.instance.rootScope ||
        _focus.ancestors.contains(primary)) {
      _focus.requestFocus();
    }
  }

  void _handFocusBack() {
    final FocusNode? primary = FocusManager.instance.primaryFocus;
    if (primary != null &&
        primary != FocusManager.instance.rootScope &&
        !_focus.ancestors.contains(primary)) {
      // Something else -- the palette, another section -- has the keyboard.
      return;
    }
    for (final FocusNode node in _focus.ancestors) {
      if (node is! FocusScopeNode && node.canRequestFocus) {
        node.requestFocus();
        return;
      }
    }
  }

  /// Whether a shortcut may fire now. A chord (`Ctrl+F`, `Ctrl+L`) works
  /// inside a text field, a single key does not.
  bool _keysActive({required bool chord}) =>
      _visible && (chord || !isTextInputFocused());

  void _decide(Decision decision) {
    final Flow? flow = ref.read(selectedFlowProvider);
    if (flow == null || !flow.isHeld) {
      return;
    }
    if (ref.read(interceptDecisionProvider).isSending) {
      return;
    }
    unawaited(
      ref.read(interceptDecisionProvider.notifier).send(flow.id, decision),
    );
  }

  Map<Type, Action<Intent>> _actions() => <Type, Action<Intent>>{
    AllowIntent: CallbackAction<AllowIntent>(
      onInvoke: (AllowIntent intent) {
        if (_keysActive(chord: intent.chord)) {
          _decide(const Decision.allow());
        }
        return null;
      },
    ),
    BlockIntent: CallbackAction<BlockIntent>(
      onInvoke: (BlockIntent intent) {
        if (_keysActive(chord: intent.chord)) {
          _decide(const Decision.block());
        }
        return null;
      },
    ),
    NextFlowIntent: CallbackAction<NextFlowIntent>(
      onInvoke: (NextFlowIntent intent) {
        if (_keysActive(chord: false)) {
          ref.read(selectedFlowIdProvider.notifier).next();
        }
        return null;
      },
    ),
    PrevFlowIntent: CallbackAction<PrevFlowIntent>(
      onInvoke: (PrevFlowIntent intent) {
        if (_keysActive(chord: false)) {
          ref.read(selectedFlowIdProvider.notifier).previous();
        }
        return null;
      },
    ),
  };

  @override
  Widget build(BuildContext context) {
    ref.listen<FlowId?>(selectedFlowIdProvider, (
      FlowId? previous,
      FlowId? next,
    ) {
      // A failure belongs to the request it happened on.
      ref.read(interceptDecisionProvider.notifier).clear();
    });
    final bool visible = isSectionVisible(context);
    if (visible != _visible) {
      _visible = visible;
      WidgetsBinding.instance.addPostFrameCallback(
        (Duration _) => _syncFocus(),
      );
    }
    final HTokens tokens = HTheme.of(context);
    final Flow? selected = ref.watch(selectedFlowProvider);
    final List<double> ratios = ref.watch(paneRatiosProvider);
    return Shortcuts(
      shortcuts: interceptShortcuts(),
      child: Actions(
        actions: _actions(),
        child: Focus(
          focusNode: _focus,
          child: Listener(
            behavior: HitTestBehavior.translucent,
            onPointerDown: (PointerDownEvent _) => _focus.requestFocus(),
            child: ColoredBox(
              color: tokens.colors.bg0,
              child: HResizablePanes(
                ratios: ratios,
                minWidths: <double>[
                  tokens.sizes.paneMinQueue,
                  tokens.sizes.paneMinInspector,
                  tokens.sizes.paneMinContext,
                ],
                onRatiosChanged: ref.read(paneRatiosProvider.notifier).set,
                children: <Widget>[
                  const QueuePane(),
                  _InspectorPane(flow: selected),
                  DomainPanePlaceholder(flow: selected),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// The middle pane: the card of the selected request above its action bar.
class _InspectorPane extends StatelessWidget {
  const _InspectorPane({required this.flow});

  final Flow? flow;

  @override
  Widget build(BuildContext context) {
    final Flow? flow = this.flow;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Expanded(
          child: flow == null
              ? const _NothingSelected()
              : RequestCard(flow: flow),
        ),
        ActionBar(flow: flow),
      ],
    );
  }
}

/// What the middle pane shows while no request is selected.
class _NothingSelected extends StatelessWidget {
  const _NothingSelected();

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return Center(
      child: Padding(
        padding: EdgeInsets.all(tokens.spacing.x6),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            Text(
              l10n.interceptCardEmptyTitle,
              style: tokens.typography.ui13.medium.tinted(tokens.colors.fg2),
            ),
            SizedBox(height: tokens.spacing.x2),
            Text(
              l10n.interceptCardEmptyHint,
              textAlign: TextAlign.center,
              style: tokens.typography.ui12.tinted(tokens.colors.fg2),
            ),
          ],
        ),
      ),
    );
  }
}
