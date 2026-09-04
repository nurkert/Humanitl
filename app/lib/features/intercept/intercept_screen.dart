/// The Intercept section: the queue, the request card and the decision.
///
/// Three panes (28/44/28 with minimum widths 280/480/260), the action bar
/// under the card, and the keyboard vocabulary of CONVENTIONS 3.9 bound to the
/// intents of `intents.dart`. The screen holds no domain logic: it reads
/// providers and calls notifiers (ARCHITECTURE 5).
///
/// Three rules keep the irreversible half of the keyboard honest
/// (`docs/UX.md` 5.4), and they all live here, where the keys are read: a key
/// repeat never decides, the decision keys stay locked until they come up
/// again, and the selection waits while one of them is down.
library;

// `Flow` is a domain type here, not the Flutter layout widget of the same
// name; the widget is never used in this feature.
import 'package:flutter/foundation.dart' show setEquals;
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/domain/domain.dart';
import '../../core/ui/h_resizable_panes.dart';
import '../../core/ui/ui.dart';
import '../../l10n/l10n.dart';
import 'intents.dart';
import 'providers/decision.dart';
import 'providers/flows.dart';
import 'providers/held_groups.dart';
import 'providers/note.dart';
import 'providers/pane_layout.dart';
import 'providers/queue_freeze.dart';
import 'providers/selection.dart';
import 'rule_sentence.dart';
import 'widgets/action_bar.dart';
import 'widgets/agent_ask_card.dart';
import 'widgets/batch_modal.dart';
import 'widgets/domain_pane_placeholder.dart';
import 'widgets/queue_pane.dart';
import 'widgets/request_card.dart';
import 'widgets/selection_card.dart';

/// The keys that take a decision. Their repeats are swallowed, and each one
/// stays locked until it comes up again.
///
/// Every activator that decides is in here, the modifier chords included:
/// `Ctrl+F` and `Ctrl+Shift+F` allow, `Ctrl+L`, `Ctrl+Shift+L` and
/// `Ctrl+Enter` block. A chord that stayed outside the lock would allow one
/// request, let the selection move on and allow the next one as soon as it
/// armed -- from one finger that never came up (`docs/UX.md` 5.4).
final Set<LogicalKeyboardKey> decisionKeys = <LogicalKeyboardKey>{
  LogicalKeyboardKey.enter,
  LogicalKeyboardKey.numpadEnter,
  LogicalKeyboardKey.keyA,
  LogicalKeyboardKey.keyB,
  LogicalKeyboardKey.keyF,
  LogicalKeyboardKey.keyL,
};

/// True while the shell actually paints this section.
///
/// The shell keeps every section built inside an `IndexedStack`, which wraps
/// each child in a `Visibility`. `Visibility.of` reads that wrapper and makes
/// the caller depend on it, so the screen is rebuilt when it is shown or
/// hidden -- and it keeps the check inside this feature: no feature imports
/// another one (ARCHITECTURE 5). Without such an ancestor -- a test that
/// mounts the screen alone -- the section counts as visible.
bool isSectionVisible(BuildContext context) => Visibility.of(context);

/// True while the focused control answers to `Enter` and `Space` itself.
///
/// No `Shortcuts` above a focusable control may bind `Enter`, `Space` or a
/// single letter so that the focused control goes empty-handed: a keyboard
/// user on "Block" who presses `Enter` must block, not allow (`docs/UX.md`
/// 5.2). A disabled action makes `ShortcutManager.handleKeypress` return
/// `KeyEventResult.ignored`, the key falls through to the default bindings of
/// `WidgetsApp`, and the focused control wins.
///
/// Der Typparameter ist `Intent` und nicht `ActivateIntent`. `Clickable` aus
/// `shadcn_flutter` legt seine Aktion als `CallbackAction` **ohne**
/// Typargument ab, also als `CallbackAction<Intent>`, und Flutter kann die
/// nicht auf `Action<ActivateIntent>` werfen: im Entwicklungsbau bricht
/// `maybeFind<ActivateIntent>` in einer Zusicherung ab, im Auslieferungsbau
/// gibt es still `null` zurück. Damit gälte jede Bildschirmtaste als frei,
/// während ein Control den Fokus hält — und `a` ist unumkehrbar. Flutter
/// nennt `Intent` als den Weg dafür (flutter/flutter#180871).
bool focusedControlHandlesActivate() {
  final BuildContext? context = FocusManager.instance.primaryFocus?.context;
  if (context == null || !context.mounted) {
    return false;
  }
  return Actions.maybeFind<Intent>(context, intent: const ActivateIntent()) !=
      null;
}

/// The Intercept section.
class InterceptScreen extends ConsumerStatefulWidget {
  /// Creates the section.
  const InterceptScreen({super.key});

  @override
  ConsumerState<InterceptScreen> createState() => _InterceptScreenState();
}

class _InterceptScreenState extends ConsumerState<InterceptScreen> {
  final FocusNode _focus = FocusNode(debugLabel: 'intercept');

  /// The decision keys that took a decision and have not come up since.
  final Set<LogicalKeyboardKey> _consumed = <LogicalKeyboardKey>{};

  bool _visible = true;

  // Both maps are fields, not expressions in `build`: a map rebuilt every
  // frame is a new object for every descendant that depends on it
  // (`docs/UX.md` 7).
  late final Map<ShortcutActivator, Intent> _shortcuts = interceptShortcuts();
  late final Map<Type, Action<Intent>> _actions = _buildActions();

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

  /// Whether a shortcut may fire now.
  ///
  /// A chord (`Ctrl+F`, `Ctrl+L`, `Ctrl+Enter`) works inside a text field, a
  /// single key does not. While the modal of a batch stands, the decision
  /// shortcuts of the screen are out of service altogether: a modal that could
  /// be answered past is no modal (`docs/UX.md` 5.4).
  bool _keysActive({required bool chord}) =>
      _visible &&
      ref.read(batchConfirmProvider) == null &&
      (chord || !isTextInputFocused());

  /// Reads the decision keys before the shortcuts do.
  ///
  /// A repeat is swallowed here as well as refused by the activator: a repeat
  /// means a finger that has not moved, and a finger that has not moved has
  /// not read the next URL. A key that already decided is refused out loud
  /// until it comes up again, and while any decision key is down the selection
  /// stays where it is.
  KeyEventResult _onKey(FocusNode node, KeyEvent event) {
    final LogicalKeyboardKey key = event.logicalKey;
    // The lock does not ask whether a text field has the focus: the chords
    // decide there as well, so they have to be locked there as well. A key
    // that never decided is never refused, so typing stays untouched.
    if (!decisionKeys.contains(key) || !_visible) {
      return KeyEventResult.ignored;
    }
    // The release is read before anything else, the open modal included: a
    // key that came up while the modal stood would otherwise stay locked, and
    // the next press would be refused although no finger is down.
    if (event is KeyUpEvent) {
      _release(key);
      return KeyEventResult.ignored;
    }
    if (ref.read(batchConfirmProvider) != null) {
      return KeyEventResult.ignored;
    }
    if (event is KeyRepeatEvent) {
      // A repeat is a finger that has not moved, and a finger that has not
      // moved has not read the next URL. Inside a text field the same repeat
      // is somebody holding a letter down, and it belongs to the field: the
      // chords that decide there ignore repeats in their activator anyway.
      return isTextInputFocused()
          ? KeyEventResult.ignored
          : KeyEventResult.handled;
    }
    if (event is KeyDownEvent && _consumed.contains(key)) {
      ref.read(lastRefusalProvider.notifier).refuse(RefusalReason.keyHeld);
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  /// Unlocks [key] after it came up; the selection follows when the last one
  /// is released.
  void _release(LogicalKeyboardKey key) {
    _consumed.remove(key);
    if (_consumed.isEmpty) {
      ref.read(selectedFlowIdProvider.notifier).setDecisionKeyDown(false);
    }
  }

  /// Locks every decision key that is down right now.
  ///
  /// Called after a decision was accepted. The keys stay locked until they
  /// come up, and the selection waits for the same moment.
  void _lockPressedKeys() {
    final Set<LogicalKeyboardKey> pressed =
        HardwareKeyboard.instance.logicalKeysPressed;
    final Iterable<LogicalKeyboardKey> down = decisionKeys.where(
      pressed.contains,
    );
    if (down.isEmpty) {
      return;
    }
    _consumed.addAll(down);
    ref.read(selectedFlowIdProvider.notifier).setDecisionKeyDown(true);
  }

  /// What the next decision covers: the members of the selection, or the
  /// cursor alone (`docs/UX.md` 3.5).
  List<Flow> _chosen() => ref.read(selectedFlowsProvider).flows;

  /// The group the cursor stands in, or whatever is selected.
  ///
  /// Arrivals that the freeze holds back are not part of it. They are in the
  /// queue but not on the screen, and a decision must never cover a request
  /// nobody could read (`docs/UX.md` 2.8, 5.4).
  List<Flow> _group() {
    if (ref.read(selectionProvider).length > 1) {
      return _chosen();
    }
    final FlowId? id = ref.read(selectedFlowIdProvider);
    final HeldGroup? group = id == null
        ? null
        : ref.read(heldGroupsProvider).groupOf(id);
    if (group == null) {
      return _chosen();
    }
    final Set<FlowId> waiting = ref.read(pendingArrivalsProvider);
    return <Flow>[
      for (final Flow flow in group.flows)
        if (!waiting.contains(flow.id)) flow,
    ];
  }

  /// Selects the group of the cursor, without deciding anything.
  void _selectGroup() => ref.read(selectionProvider.notifier).all(<FlowId>[
    for (final Flow flow in _group()) flow.id,
  ]);

  /// `Ctrl+Shift+F`: the first press selects the group, the second sends it.
  ///
  /// Sending is irreversible and the card has to show what it covers. As long
  /// as the selection is not the group, the chord only selects it: the card
  /// turns into the summary of all of them, and the next press decides with
  /// that summary on the screen (`docs/UX.md` 3.5, 5.4).
  void _allowGroup() {
    final List<Flow> group = _group();
    final Set<FlowId> ids = <FlowId>{for (final Flow flow in group) flow.id};
    final Set<FlowId> selected = ref.read(selectionProvider);
    if (group.length > 1 && !setEquals(selected, ids)) {
      ref.read(selectionProvider.notifier).all(ids);
      return;
    }
    _allow(_chosen());
  }

  void _allow(List<Flow> flows) {
    _lockPressedKeys();
    // A key cannot slip sideways into the wrong control, so it counts as the
    // confirmation the hold of `docs/UX.md` 4.7 asks the pointer for; the
    // arming of 5.4 still stands in front of it.
    ref
        .read(interceptDecisionProvider.notifier)
        .allowMany(flows, confirmed: true);
  }

  void _block(List<Flow> flows) {
    _lockPressedKeys();
    ref.read(interceptDecisionProvider.notifier).blockMany(flows);
  }

  /// Moves the cursor and tells the queue that a key did it: the order stays
  /// frozen for `HMotion.freezeAfterKey` afterwards (`docs/UX.md` 2.8).
  void _move(void Function() step) {
    step();
    ref.read(queueKeyboardNavProvider.notifier).touch();
  }

  /// Folds the group of the cursor, or opens it.
  void _foldGroup({required bool open}) {
    final FlowId? id = ref.read(selectedFlowIdProvider);
    final HeldGroup? group = id == null
        ? null
        : ref.read(heldGroupsProvider).groupOf(id);
    if (group != null && group.isBurst) {
      ref.read(expandedGroupsProvider.notifier).setOpen(group, open);
    }
  }

  Map<Type, Action<Intent>> _buildActions() => <Type, Action<Intent>>{
    AllowIntent: _DecisionAction<AllowIntent>(
      chord: (AllowIntent intent) => intent.chord,
      active: _keysActive,
      onDecide: (AllowIntent intent) => _allow(_chosen()),
    ),
    BlockIntent: _DecisionAction<BlockIntent>(
      chord: (BlockIntent intent) => intent.chord,
      active: _keysActive,
      onDecide: (BlockIntent intent) => _block(_chosen()),
    ),
    // The two chords work inside the note field as well; every single key
    // below does not, and says so through `isEnabled` rather than by
    // swallowing the key (see [_ScreenAction]).
    AllowGroupIntent: _ScreenAction<AllowGroupIntent>(
      chord: true,
      active: _keysActive,
      onAct: (AllowGroupIntent intent) => _allowGroup(),
    ),
    BlockGroupIntent: _ScreenAction<BlockGroupIntent>(
      chord: true,
      active: _keysActive,
      onAct: (BlockGroupIntent intent) => _block(_group()),
    ),
    SelectGroupIntent: _ScreenAction<SelectGroupIntent>(
      active: _keysActive,
      onAct: (SelectGroupIntent intent) => _selectGroup(),
    ),
    MergeArrivalsIntent: _ScreenAction<MergeArrivalsIntent>(
      active: _keysActive,
      onAct: (MergeArrivalsIntent intent) =>
          ref.read(queueMergeRequestProvider.notifier).request(),
    ),
    ToggleGroupIntent: _ScreenAction<ToggleGroupIntent>(
      active: _keysActive,
      onAct: (ToggleGroupIntent intent) => _foldGroup(open: intent.open),
    ),
    NoteIntent: _ScreenAction<NoteIntent>(
      active: _keysActive,
      onAct: (NoteIntent intent) => ref.read(blockNoteProvider.notifier).open(),
    ),
    NextFlowIntent: _ScreenAction<NextFlowIntent>(
      active: _keysActive,
      onAct: (NextFlowIntent intent) =>
          _move(ref.read(selectedFlowIdProvider.notifier).next),
    ),
    PrevFlowIntent: _ScreenAction<PrevFlowIntent>(
      active: _keysActive,
      onAct: (PrevFlowIntent intent) =>
          _move(ref.read(selectedFlowIdProvider.notifier).previous),
    ),
    OpenRememberIntent: _ScreenAction<OpenRememberIntent>(
      active: _keysActive,
      onAct: (OpenRememberIntent intent) =>
          ref.read(rememberDraftProvider.notifier).open(),
    ),
    RememberDurationIntent: _ScreenAction<RememberDurationIntent>(
      active: _keysActive,
      onAct: (RememberDurationIntent intent) => ref
          .read(rememberDraftProvider.notifier)
          .setDuration(RememberDuration.values[intent.index]),
    ),
    RememberTargetIntent: _ScreenAction<RememberTargetIntent>(
      active: _keysActive,
      onAct: (RememberTargetIntent intent) => ref
          .read(rememberDraftProvider.notifier)
          .setTarget(RememberTarget.values[intent.index]),
    ),
  };

  @override
  Widget build(BuildContext context) {
    ref.listen<FlowId?>(selectedFlowIdProvider, (
      FlowId? previous,
      FlowId? next,
    ) {
      // A failure belongs to the request it happened on, and so does the
      // reason a key was refused. A batch that stopped in the middle moves the
      // cursor itself -- the requests that did go leave the queue -- and the
      // card of the request that did not go must survive that move
      // (`docs/UX.md` 4.4).
      final DecisionProgress progress = ref.read(interceptDecisionProvider);
      if (progress is DecisionFailed && progress.flowId == next) {
        return;
      }
      ref.read(interceptDecisionProvider.notifier).clear();
      ref.read(lastRefusalProvider.notifier).clear();
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
    // A scalar, never the snapshot: a decision must not rebuild this screen
    // (`docs/UX.md` 7). A screen shows one empty state, not one per pane, so
    // while the queue is empty the panes beside it stay still (3.2).
    final bool queueEmpty = ref.watch(
      visibleQueueFlowsProvider.select(
        (QueueSnapshot snapshot) => snapshot.flows.isEmpty,
      ),
    );
    final List<double> ratios = ref.watch(paneRatiosProvider);
    // What the next decision covers, for the card in the middle: a group is
    // decided over as a whole, so the card has to show the whole of it
    // (`docs/UX.md` 3.5).
    final List<Flow> chosen = ref.watch(selectedFlowsProvider).flows;
    final BatchRequest? asking = ref.watch(batchConfirmProvider);
    final AgentAskRuleDraft? ruleDraft = ref.watch(agentAskRuleDraftProvider);
    return Shortcuts(
      shortcuts: _shortcuts,
      child: Actions(
        actions: _actions,
        child: Focus(
          focusNode: _focus,
          onKeyEvent: _onKey,
          child: Listener(
            behavior: HitTestBehavior.translucent,
            onPointerDown: (PointerDownEvent _) => _focus.requestFocus(),
            child: ColoredBox(
              color: tokens.colors.bg0,
              child: Stack(
                fit: StackFit.expand,
                children: <Widget>[
                  HResizablePanes(
                    ratios: ratios,
                    minWidths: <double>[
                      tokens.sizes.paneMinQueue,
                      tokens.sizes.paneMinInspector,
                      tokens.sizes.paneMinContext,
                    ],
                    onRatiosChanged: ref.read(paneRatiosProvider.notifier).set,
                    children: <Widget>[
                      const QueuePane(),
                      _InspectorPane(
                        flow: selected,
                        selection: chosen,
                        queueEmpty: queueEmpty,
                      ),
                      DomainPanePlaceholder(flow: selected),
                    ],
                  ),
                  // The sheet hangs on the right edge and leaves the panes
                  // where they are: it asks for a rule, it does not decide
                  // anything, so it never dims the screen behind it
                  // (`docs/UX.md` 2.2). It comes from the agent's card in the
                  // queue (HUM-073).
                  if (ruleDraft != null)
                    Positioned(
                      top: 0,
                      right: 0,
                      bottom: 0,
                      child: AgentAskRuleSheet(draft: ruleDraft),
                    ),
                  // The one modal of this screen, above everything, with the
                  // background dimmed and nothing behind it reachable.
                  if (asking != null) BatchModal(request: asking),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// An action of this screen that gives the key back when it may not act.
///
/// `ShortcutManager.handleKeypress` returns `KeyEventResult.handled` for every
/// action it invokes, because `Action.consumesKey` is true by default, and a
/// handled key never reaches the text input system. A single letter bound on
/// the screen would therefore be missing from every note somebody types --
/// "nutze PyPI statt GitHub" would lose its `a`, `b` and `n`. Refusing inside
/// `invoke` is too late; the refusal belongs in [isEnabled], which makes the
/// manager return `ignored` and lets the key fall through (`docs/UX.md` 5.2).
class _ScreenAction<T extends Intent> extends Action<T> {
  _ScreenAction({
    required this.active,
    required this.onAct,
    this.chord = false,
  });

  /// True for a modifier chord, which also works inside a text field.
  final bool chord;

  /// Whether the keys of this screen may act at all right now.
  final bool Function({required bool chord}) active;

  /// What the key does.
  final void Function(T intent) onAct;

  @override
  bool isEnabled(T intent) => active(chord: chord);

  @override
  Object? invoke(T intent) {
    onAct(intent);
    return null;
  }
}

/// A decision bound to a single key, which yields to a focused control.
///
/// The mechanism of `docs/UX.md` 5.2: `Action.overridable` does not help --
/// an overridable action is overridden by an *ancestor*, never by a focused
/// descendant, and the collision happens earlier anyway, because `Shortcuts`
/// maps `Enter` to an intent before any action lookup runs. It yields for the
/// same reason as [_ScreenAction] while the keys are out of service.
class _DecisionAction<T extends Intent> extends Action<T> {
  _DecisionAction({
    required this.chord,
    required this.active,
    required this.onDecide,
  });

  /// True for the modifier chords, which work everywhere, also on a control.
  final bool Function(T intent) chord;

  /// Whether the keys of this screen may act at all right now.
  final bool Function({required bool chord}) active;

  /// Takes the decision.
  final void Function(T intent) onDecide;

  @override
  bool isEnabled(T intent) =>
      active(chord: chord(intent)) &&
      (chord(intent) || !focusedControlHandlesActivate());

  @override
  Object? invoke(T intent) {
    onDecide(intent);
    return null;
  }
}

/// The middle pane: the card of the selected request above its action bar.
class _InspectorPane extends StatelessWidget {
  const _InspectorPane({
    required this.flow,
    required this.selection,
    required this.queueEmpty,
  });

  final Flow? flow;

  /// Everything the next decision covers. More than one request replaces the
  /// card of one URL with the summary of all of them: a card that showed one
  /// of twelve while the bar sends twelve would be a dark pattern
  /// (`backlog/CONVENTIONS.md` 4.13).
  final List<Flow> selection;

  /// True while nothing waits at all; the pane then says nothing, because the
  /// queue beside it already does (`docs/UX.md` 3.2).
  final bool queueEmpty;

  @override
  Widget build(BuildContext context) {
    final Flow? flow = this.flow;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Expanded(
          child: selection.length > 1
              ? SelectionCard(flows: selection)
              : flow != null
              ? RequestCard(flow: flow)
              : queueEmpty
              ? const SizedBox.shrink()
              : const _NothingSelected(),
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
              style: tokens.typography.ui13.medium.tinted(tokens.colors.fg1),
            ),
            SizedBox(height: tokens.spacing.x2),
            Text(
              l10n.interceptCardEmptyHint,
              textAlign: TextAlign.center,
              style: tokens.typography.ui12.tinted(tokens.colors.fg1),
            ),
          ],
        ),
      ),
    );
  }
}
