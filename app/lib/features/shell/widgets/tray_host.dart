/// The seam between the queue and the desktop (HUM-034).
///
/// It sits in the shell because the shell is the frame that knows both sides
/// (`docs/ARCHITECTURE.md` 5): it feeds the attention machine with the queue,
/// the connection and the timeouts, and it turns what comes back into a tray
/// face, a notification and a window title, in the language of the person.
///
/// It wraps the connection gate rather than the shell, so that a daemon that
/// stops answering does not take the tray with it. That is the moment the
/// tray has to say the number is unknown, and a widget inside the shell would
/// be gone by then.
library;

import 'dart:async';

// `Flow` here is the domain type, never the Flutter layout widget of the
// same name; this file never lays out a wrap.
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/flow_events.dart';
import '../../../l10n/l10n.dart';
import '../../intercept/providers/decision.dart';
import '../../intercept/providers/flows.dart';
import '../../tray/attention_text.dart';
import '../../tray/desktop_ports.dart';
import '../../tray/providers/attention.dart';
import '../../tray/providers/notice.dart';
import '../../tray/tray_diagnostics.dart';
import '../providers/connection.dart';
import '../providers/navigation.dart';
import '../section.dart';

/// Wraps [child] and keeps the desktop in step with it.
class TrayHost extends ConsumerStatefulWidget {
  /// Creates the host.
  const TrayHost({required this.child, super.key});

  /// What the host wraps; the gate and everything under it.
  final Widget child;

  @override
  ConsumerState<TrayHost> createState() => _TrayHostState();
}

class _TrayHostState extends ConsumerState<TrayHost> {
  StreamSubscription<TrayCommand>? _commands;
  StreamSubscription<NotificationAnswer>? _actions;

  /// The face the tray was last asked to draw.
  TrayFace? _face;

  /// The title the window was last given.
  String? _title;

  /// The serial of the message that was last posted; zero for none.
  int _posted = 0;

  DesktopPorts get _ports => ref.read(desktopPortsProvider);

  Attention get _attention => ref.read(attentionProvider.notifier);

  @override
  void initState() {
    super.initState();
    _commands = _ports.tray.commands.listen(_command);
    _actions = _ports.notifications.actions.listen(_action);
    // Providers are not written to while the tree builds; the first feed and
    // the first push wait for the frame to be over.
    WidgetsBinding.instance.addPostFrameCallback((Duration _) => _seed());
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    // The locale may have changed, and the tray keeps its old words until
    // somebody redraws it.
    WidgetsBinding.instance.addPostFrameCallback((Duration _) {
      if (mounted) {
        _face = null;
        _title = null;
        _push(ref.read(attentionProvider));
      }
    });
  }

  @override
  void dispose() {
    unawaited(_commands?.cancel());
    unawaited(_actions?.cancel());
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    ref.listen(heldFlowsProvider, (List<Flow>? previous, List<Flow> next) {
      _attention.heldChanged(next);
    });
    ref.listen(connectionStateProvider, (
      ConnectionStatus? previous,
      ConnectionStatus next,
    ) {
      _attention.connectionChanged(connected: next is ConnectionConnected);
    });
    ref.listen(flowEventsProvider, (
      AsyncValue<FlowEvent>? previous,
      AsyncValue<FlowEvent> next,
    ) {
      next.whenData((FlowEvent event) {
        switch (event) {
          case FlowEventTimedOut():
            _attention.holdTimedOut();
          case FlowEventLagged():
            // The stream reconnects on its own, independently of `GetInfo`
            // and its heartbeat, and marks every connection with a `Lagged`.
            // Everything the queue holds is from before that gap, so the
            // count is unknown until the resync answers.
            _attention.streamGapped();
          default:
            break;
        }
      });
    });
    ref.listen(attentionProvider, (
      AttentionState? previous,
      AttentionState next,
    ) {
      _push(next);
    });
    return widget.child;
  }

  Future<void> _seed() async {
    if (!mounted) {
      return;
    }
    // Only the connection, deliberately not the queue: what the app holds at
    // this moment is nothing, and nothing is not the same as an empty queue.
    // `Subscribe` starts "from now on", so the daemon may well be holding
    // three requests the app has not heard of yet; the resync of the first
    // connection answers with the real queue, and that answer is what the
    // tray waits for (`backlog/CONVENTIONS.md` 4.19).
    _attention.connectionChanged(
      connected: ref.read(connectionStateProvider) is ConnectionConnected,
    );
    _push(ref.read(attentionProvider));
    final Diagnostic? missing = await _ports.tray.start();
    if (missing != null && mounted) {
      ref.read(attentionNoticeProvider.notifier).showOnce(missing);
    }
  }

  void _push(AttentionState state) {
    if (!mounted) {
      return;
    }
    final AppLocalizations l10n = context.l10n;
    final TrayFace face = trayFace(l10n, state);
    if (face != _face) {
      _face = face;
      unawaited(_ports.tray.show(face));
    }
    final String title = windowTitle(l10n, state);
    if (title != _title) {
      _title = title;
      unawaited(_ports.window.setTitle(title));
    }
    final HeldNotice? notice = state.notice;
    if (notice == null) {
      if (_posted != 0) {
        _posted = 0;
        unawaited(_ports.notifications.withdraw());
      }
      return;
    }
    if (notice.serial != _posted) {
      _posted = notice.serial;
      unawaited(_ports.notifications.post(notificationFor(l10n, notice)));
    }
  }

  Future<void> _command(TrayCommand command) async {
    switch (command) {
      case TrayCommand.show:
        // A click on the icon points at no request in particular, so the
        // selection stays where the person left it.
        await _reveal();
      case TrayCommand.quit:
        // The cleanup comes first, because what follows it does not return:
        // `quit` destroys the window, and the two D-Bus connections and the
        // bus name would stay open behind it. The `dbus` package says of its
        // own client that a process which leaves one open may not end at all.
        await _ports.dispose();
        await _ports.window.quit();
    }
  }

  /// Answers a press, for the request the message named and no other.
  ///
  /// The request comes out of the action key, so a press on a popup the
  /// server never replaced is answered for the request that popup was about.
  /// An answer for a request the queue no longer holds is not swallowed: it
  /// gets the registered `IPC_003` and the window, because a button that does
  /// nothing and says nothing is worse than one that refuses out loud.
  Future<void> _action(NotificationAnswer answer) async {
    switch (answer.kind) {
      case NotificationActionKind.show:
        _attention.notificationAnswered();
        if (!ref.read(flowsProvider).containsKey(answer.flowId)) {
          ref
              .read(attentionNoticeProvider.notifier)
              .show(TrayDiagnostics.decidedAlready(answer.flowId));
        }
        await _reveal(answer.flowId);
      case NotificationActionKind.allow:
        await _decide(answer.flowId, const Decision.allow());
      case NotificationActionKind.block:
        await _decide(answer.flowId, const Decision.block());
    }
  }

  /// Decides the request the message named, and only that one.
  ///
  /// A message can outlive what it names, and what it names can change under
  /// it. Two things are therefore checked against the queue as it is now, not
  /// against the queue the message was worded from:
  ///
  /// * The request has left the queue. Nothing is decided in its place -- the
  ///   window comes forward with the registered `IPC_003` instead, because
  ///   deciding whatever moved to the top meanwhile would be a decision
  ///   nobody took.
  /// * The request has acquired a finding since the message went out.
  ///   Analysis and hold are two events and the second can follow the first,
  ///   so a message that offered `Allow` can still be standing when the
  ///   finding arrives. The button does not send it; sending a request that
  ///   carries a secret asks for the held confirmation and a sentence naming
  ///   what goes where (`docs/UX.md` 4.7), and neither fits in a message.
  Future<void> _decide(FlowId id, Decision decision) async {
    final Flow? flow = ref.read(flowsProvider)[id];
    if (flow == null || !flow.isHeld) {
      ref
          .read(attentionNoticeProvider.notifier)
          .show(TrayDiagnostics.decidedAlready(id));
      await _reveal();
      return;
    }
    if (decision is DecisionAllow && flow.findingCount > 0) {
      _attention.notificationAnswered();
      ref
          .read(attentionNoticeProvider.notifier)
          .show(TrayDiagnostics.findingsNeedTheWindow(id, flow.findingCount));
      await _reveal(id);
      return;
    }
    _attention.notificationAnswered();
    // No `reveal`: answering from the message is the whole point, and the
    // window stays where it is (HUM-034 acceptance).
    await ref
        .read(interceptDecisionProvider.notifier)
        .send(id, decision, flow: flow);
  }

  /// Brings the window forward, on [id] when one is named.
  Future<void> _reveal([FlowId? id]) async {
    await _ports.window.reveal();
    if (!mounted) {
      return;
    }
    ref.read(navigationProvider.notifier).go(Section.intercept);
    if (id != null && ref.read(flowsProvider).containsKey(id)) {
      ref.read(selectedFlowIdProvider.notifier).select(id);
    }
  }
}
