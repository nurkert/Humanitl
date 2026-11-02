/// Whether the daemon intercepts, and the two ways to change it (HUM-407).
///
/// The pause lives in the daemon, never here: this notifier reads it from
/// every heartbeat (`GetInfo`), follows every change on the event stream
/// ([FlowEventInterceptChanged]) and asks the daemon to pause or resume. What
/// the pause means -- what passes, what stays blocked, when it ends -- the
/// daemon decides (ADR-018). Every client sees the same pause: one started
/// from the command line turns the pill and the tape on here as well.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/connection.dart';
import '../../../core/ipc/daemon_client.dart';
import '../../../core/ipc/flow_events.dart';

/// The daemon's intercept status; [InterceptStatus.on] until it said
/// otherwise, and always for a daemon that cannot pause.
final NotifierProvider<InterceptPause, InterceptStatus> interceptPauseProvider =
    NotifierProvider<InterceptPause, InterceptStatus>(InterceptPause.new);

/// Whether the connected daemon can pause intercepting at all; only then is
/// the pill a button.
///
/// Read from the connection itself, which every heartbeat updates, so a
/// daemon that gains or loses the capability is seen within one heartbeat.
final Provider<bool> interceptPausableProvider = Provider<bool>(
  (Ref ref) =>
      ref.watch(connectionStateProvider).info?.offersInterceptPause ?? false,
);

/// Reads, follows and changes the pause of intercepting.
///
/// **The heartbeat is the authority.** Every `GetInfo` carries the daemon's
/// instance (`Info.session_id`), its capabilities and its intercept status
/// together, and the connection asks for it every two seconds
/// ([connectionHeartbeat]). Each answer is taken as one piece: a new instance
/// replaces instance, capability and status at once; the same instance moves
/// the status forward when its revision is not below the shown one.
///
/// **Between heartbeats** a reply to `Intercept` or a stream event may move
/// the display forward, but only when it carries the instance of the latest
/// heartbeat and a revision not below the shown one. Everything else — an
/// empty instance, another daemon's late answer, a new daemon the heartbeat
/// has not reported yet — is ignored, without probing or guessing.
///
/// Whatever goes wrong in between therefore heals with the next heartbeat:
/// the app shows a pause at the latest one heartbeat, two seconds, after it
/// began, caution tape included. The daemon writes it to the audit log at
/// once.
class InterceptPause extends Notifier<InterceptStatus> {
  /// The instance of the latest heartbeat; null before the first.
  String? _instance;

  /// Ob der Daemon des letzten Herzschlags pausieren kann.
  bool _pausable = false;

  @override
  InterceptStatus build() {
    final DaemonInfo? info = ref.read(connectionStateProvider).info;
    _instance = info?.sessionId;
    _pausable = info?.offersInterceptPause ?? false;
    ref.listen<DaemonInfo?>(
      connectionStateProvider.select((ConnectionStatus status) => status.info),
      (DaemonInfo? previous, DaemonInfo? next) {
        if (next != null) {
          _heartbeat(next);
        }
      },
    );
    // Every change of any client arrives here, the own ones included.
    ref.listen(flowEventsProvider, (
      AsyncValue<FlowEvent>? previous,
      AsyncValue<FlowEvent> next,
    ) {
      if (next.value case FlowEventInterceptChanged(
        :final InterceptStatus status,
      )) {
        _offer(status);
      }
    }, fireImmediately: true);
    return _fromInfo(info);
  }

  DaemonClient get _client => ref.read(daemonClientProvider);

  /// The status [info] reports, or "on" for a daemon that cannot pause.
  static InterceptStatus _fromInfo(DaemonInfo? info) =>
      info != null && info.offersInterceptPause
      ? info.intercept ?? InterceptStatus.on
      : InterceptStatus.on;

  /// Takes one heartbeat as a whole.
  void _heartbeat(DaemonInfo info) {
    final InterceptStatus reported = _fromInfo(info);
    final bool fresh = info.sessionId != _instance;
    _instance = info.sessionId;
    _pausable = info.offersInterceptPause;
    // Ein Daemon, der nicht pausieren kann, ist nicht pausiert, gleich
    // welche Revision vorher galt: Sonst bliebe das Band stehen, ohne dass
    // die Pille es aufheben könnte.
    if (fresh || !_pausable || reported.revision >= state.revision) {
      state = reported;
    }
  }

  /// Applies a reply or an event of the daemon the latest heartbeat named,
  /// when it is not older than what is shown.
  void _offer(InterceptStatus status) {
    if (!ref.mounted ||
        !_pausable ||
        _instance == null ||
        status.instance != _instance ||
        status.revision < state.revision) {
      return;
    }
    state = status;
  }

  /// Pauses intercepting. Throws a [DaemonException] when the daemon refuses.
  Future<void> pause() => _change(InterceptOp.pause);

  /// Intercepts again. Throws a [DaemonException] when the daemon refuses.
  Future<void> resume() => _change(InterceptOp.resume);

  Future<void> _change(InterceptOp op) async {
    _offer(await _client.intercept(op));
  }
}
