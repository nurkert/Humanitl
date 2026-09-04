/// What the program says while nobody is looking at it (HUM-034).
///
/// One state machine, no strings and no queue: it decides *whether* a
/// notification should stand, *which* request it names and *what* the tray
/// icon shows. The wording is left to the widget that has a `BuildContext`
/// and a locale, and the queue is pushed in from outside -- the shell is the
/// frame that knows both this feature and the intercept queue
/// (`docs/ARCHITECTURE.md` 5, `tools/check-deps.sh`). What is left here is
/// the awkward half, the timing, and it is testable without a desktop and
/// without a widget.
///
/// The rules it keeps, all of them from `docs/UX.md` 4.9:
///
/// * A notification appears only while the window does not have focus. What
///   stands on the screen is not announced a second time. What arrives after
///   the person left is announced, whether or not the queue was empty when
///   they left: the specification names the step from zero to one, but the
///   register describes the condition as "a request is waiting and the window
///   is not in front" (`docs/CONFIG.md`, `ui.notifications`), and that is the
///   honest reading -- somebody who steps away from one waiting request and
///   comes back to sixteen was told nothing under the other one.
/// * At most one notification exists at a time, and at most one is posted per
///   [notificationBundle]. A burst of fifteen arrivals updates one message.
///   The update itself is posted when the window closes, not at the moment of
///   the arrival, so a standing message can name a smaller number than the
///   queue holds for up to one window. That deviation from `docs/UX.md` 4.9
///   is deliberate and written down in `backlog/CONVENTIONS.md` 4.19.
/// * Only an arrival is announced. A queue that changes without a request
///   being added -- a decision, a timeout, a fresh list with the same
///   requests in it -- changes the count and nothing else.
/// * The tray counts held requests and nothing else. When the daemon stops
///   answering, the count is unknown and the tray says so instead of showing
///   the last number it saw.
library;

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../core/domain/domain.dart';
import '../desktop_ports.dart';

part 'attention.g.dart';

/// How long one notification covers the arrivals that follow it.
///
/// Inside this window further arrivals change what the message says, they
/// never add a second one; the change itself is posted once, when the window
/// closes. HUM-034 names the five seconds.
const Duration notificationBundle = Duration(seconds: 5);

/// How long a request has to have waited before the return banner appears.
///
/// Below it the banner would tell a person who stepped away for a moment
/// something they already know (HUM-034).
const Duration returnAfter = Duration(seconds: 60);

/// Whether desktop notifications are wanted.
///
/// The setting is registered: `docs/CONFIG.md` lists `ui.notifications`
/// (boolean, default `true`, tier `advanced`) and next to it `ui.sound`
/// (boolean, default `false`, tier `advanced`, without effect in the MVP).
/// What is missing is not the key but the way to read it -- the daemon client
/// has no `GetConfig` -- so this answers `true` for everybody and a person who
/// switched the setting off still gets messages. The binding follows as soon
/// as the client can ask; until then this is the seam it binds to, and the
/// switch the tests use.
@Riverpod(keepAlive: true)
bool notificationsEnabled(Ref ref) => true;

/// The desktop this program talks to.
///
/// Inert by default; `main` overrides it with the Linux bundle and tests with
/// their fakes. A provider that built a real D-Bus connection by default
/// would make every widget test depend on a session bus.
@Riverpod(keepAlive: true)
DesktopPorts desktopPorts(Ref ref) => DesktopPorts.inert();

/// The request a notification names, with everything needed to word it.
@immutable
class HeldNotice {
  /// Creates a notice.
  const HeldNotice({
    required this.flowId,
    required this.host,
    required this.method,
    required this.path,
    required this.remaining,
    required this.total,
    required this.findings,
    required this.serial,
  });

  /// The request the buttons act on. It travels with the message so that a
  /// press decides the request that was named, never whatever stands on top
  /// of a queue that moved on in the meantime.
  final FlowId flowId;

  /// The host: the one important thing of the message.
  final String host;

  /// The method of the named request.
  final String method;

  /// The path of the named request.
  final String path;

  /// How much of the hold budget was left when the message was worded.
  ///
  /// A notification is a still image; this is turned into a word ("about four
  /// minutes left"), never into a running `mm:ss` that is wrong a second
  /// later.
  final Duration remaining;

  /// How many requests were held when the message was worded.
  final int total;

  /// How many findings the named request carries.
  ///
  /// Above zero the message offers no `Allow` button: sending a request that
  /// carries a secret asks for the held confirmation and a sentence naming
  /// what goes where (`docs/UX.md` 4.7), and neither fits in a notification.
  final int findings;

  /// Counts the postings, so that two messages with the same words are two
  /// events and the adapter knows it has to post again.
  final int serial;

  /// True while the message may offer to send the request on.
  bool get mayAllow => findings == 0;

  /// How many requests wait besides the one that is named.
  int get others => total - 1;

  @override
  bool operator ==(Object other) =>
      other is HeldNotice &&
      other.flowId == flowId &&
      other.host == host &&
      other.method == method &&
      other.path == path &&
      other.remaining == remaining &&
      other.total == total &&
      other.findings == findings &&
      other.serial == serial;

  @override
  int get hashCode => Object.hash(
    flowId,
    host,
    method,
    path,
    remaining,
    total,
    findings,
    serial,
  );
}

/// What the return banner says and where it leads.
@immutable
class ReturnNotice {
  /// Creates a notice.
  const ReturnNotice({required this.flowId, required this.waited});

  /// The longest waiting request; the banner leads there and nowhere else.
  final FlowId flowId;

  /// How long it had been waiting when the window came back.
  final Duration waited;

  @override
  bool operator ==(Object other) =>
      other is ReturnNotice && other.flowId == flowId && other.waited == waited;

  @override
  int get hashCode => Object.hash(flowId, waited);
}

/// Everything the desktop and the shell need to know at once.
@immutable
class AttentionState {
  /// Creates a state.
  const AttentionState({
    this.tray = TrayIconState.idle,
    this.held = 0,
    this.timedOutAway = 0,
    this.notice,
    this.banner,
  });

  /// Which look the tray icon takes.
  final TrayIconState tray;

  /// How many requests are held. Zero while the daemon does not answer,
  /// because then the number is not known, not zero.
  final int held;

  /// How many holds ran out since the window last had focus.
  final int timedOutAway;

  /// The notification that should stand, or null for none.
  final HeldNotice? notice;

  /// The return banner that should stand, or null for none.
  final ReturnNotice? banner;

  @override
  bool operator ==(Object other) =>
      other is AttentionState &&
      other.tray == tray &&
      other.held == held &&
      other.timedOutAway == timedOutAway &&
      other.notice == notice &&
      other.banner == banner;

  @override
  int get hashCode => Object.hash(tray, held, timedOutAway, notice, banner);
}

/// The state machine behind the tray, the notification and the banner.
///
/// It is fed rather than fetching: [heldChanged], [connectionChanged] and
/// [holdTimedOut] come from the shell, [focusChanged] from the window port.
@Riverpod(keepAlive: true)
class Attention extends _$Attention {
  /// True while the window is in front. The program starts in front.
  bool _focused = true;

  /// True while the daemon answers.
  bool _connected = true;

  /// The held requests, in queue order.
  List<Flow> _held = const <Flow>[];

  /// The requests that were held when the queue was last handed over.
  ///
  /// An arrival is a request that was not in this set, never a queue that grew
  /// longer: the queue is recomputed on every event of the stream and handed
  /// over as a fresh list each time, so length and list identity say nothing
  /// about whether anything arrived.
  Set<FlowId> _known = const <FlowId>{};

  /// True while the queue the app holds has not been confirmed.
  ///
  /// It starts true, because a program that has just started knows nothing:
  /// `Subscribe` means "from now on", so the daemon may be holding requests
  /// this client has never heard of, and an icon that says "the queue is open"
  /// before the first answer states something nobody checked.
  ///
  /// It goes true again on every gap. `GetInfo` and the event stream
  /// reconnect independently, each with its own backoff of up to 30 seconds,
  /// so a connection that is back says nothing about the queue; only the
  /// first [heldChanged] after the gap does, and until it arrives the count is
  /// unknown rather than the one from before (`backlog/CONVENTIONS.md` 4.13
  /// and 4.19).
  bool _stale = true;

  /// True while a notification stands.
  bool _live = false;

  /// The open bundling window; null while none is open.
  Timer? _bundle;

  /// True when the message became untrue while a bundling window was open.
  bool _pending = false;

  /// The message that stands.
  HeldNotice? _notice;

  /// The banner that stands.
  ReturnNotice? _banner;

  /// How often a message was posted.
  int _serial = 0;

  /// How many holds ran out since the window last had focus.
  int _timedOutAway = 0;

  @override
  AttentionState build() {
    final DesktopPorts ports = ref.watch(desktopPortsProvider);
    final StreamSubscription<bool> focus = ports.window.focus.listen(
      (bool focused) => _focusAt(focused),
    );
    ref.onDispose(() {
      unawaited(focus.cancel());
      _bundle?.cancel();
      _bundle = null;
    });
    return _compose();
  }

  /// Hands the machine the held requests, in queue order.
  void heldChanged(List<Flow> held) {
    final Set<FlowId> before = _known;
    final Set<FlowId> now = <FlowId>{for (final Flow flow in held) flow.id};
    _held = held;
    _known = now;
    // This is the answer the tray waits for: the queue was heard from, so what
    // it shows from here on comes from the daemon and not from a snapshot of
    // one that has stopped answering, nor from a client that has not asked yet.
    _stale = false;
    final ReturnNotice? banner = _banner;
    if (banner != null && !now.contains(banner.flowId)) {
      // The request the banner led to was decided; a banner that points at
      // nothing is worse than none.
      _banner = null;
    }
    _announce(arrived: now.any((FlowId id) => !before.contains(id)));
    state = _compose();
  }

  /// Tells the machine whether the daemon still answers.
  void connectionChanged({required bool connected}) {
    if (connected == _connected) {
      return;
    }
    _connected = connected;
    if (!connected) {
      // The queue the app remembers is a snapshot of a daemon that stopped
      // answering. Nothing may be claimed from it any more -- and nothing
      // after the connection returns either, until the queue itself has been
      // heard from again. See [_stale].
      _drop();
      _banner = null;
      _stale = true;
    }
    state = _compose();
  }

  /// Tells the machine that the event stream had a gap.
  ///
  /// A `Lagged` says that events were missed, and everything the queue holds
  /// is from before the gap. The stream reconnects on its own, with a backoff
  /// of up to 30 seconds and independently of `GetInfo`, so this is the only
  /// notice the machine gets that its queue is a snapshot: [connectionChanged]
  /// hangs on the heartbeat and never sees it. The count is unknown until the
  /// resync answers with a queue (`backlog/CONVENTIONS.md` 4.19).
  void streamGapped() {
    if (_stale) {
      return;
    }
    _stale = true;
    _drop();
    _banner = null;
    state = _compose();
  }

  /// Tells the machine that a hold ran out.
  ///
  /// A timeout is the one event nobody triggered, and the tray is the only
  /// place a person who is elsewhere can learn about it (`docs/UX.md` 4.8).
  void holdTimedOut() {
    if (_focused) {
      return;
    }
    _timedOutAway++;
    state = _compose();
  }

  /// Tells the machine that the window came to the front or left it.
  void focusChanged({required bool focused}) => _focusAt(focused);

  /// Tells the machine that a person answered the message.
  ///
  /// The message goes off the screen, but the conversation stays open: a
  /// person who just pressed a button is not nagged by the next arrival, they
  /// are kept up to date by it -- rate limited like every other update.
  void notificationAnswered() {
    if (_notice == null) {
      return;
    }
    _notice = null;
    state = _compose();
  }

  /// Takes the return banner away; the person read it.
  void dismissBanner() {
    if (_banner == null) {
      return;
    }
    _banner = null;
    state = _compose();
  }

  void _focusAt(bool focused) {
    if (_focused == focused) {
      return;
    }
    _focused = focused;
    if (focused) {
      // What stands on the screen is not announced a second time, and the
      // alert the tray carried is what the person came back for.
      _drop();
      _timedOutAway = 0;
      _banner = _returnBanner();
    } else {
      // Everything that is waiting at this moment was seen: the person is
      // leaving a window that shows it. From here on every arrival is news,
      // and the only thing that holds it back is the bundling window.
      _known = <FlowId>{for (final Flow flow in _held) flow.id};
    }
    state = _compose();
  }

  /// Decides whether this change is worth a notification.
  ///
  /// [arrived] says whether a request that was not held before is held now.
  void _announce({required bool arrived}) {
    if (!_connected || _held.isEmpty) {
      _drop();
      return;
    }
    if (_focused || !ref.read(notificationsEnabledProvider)) {
      return;
    }
    if (!arrived) {
      // The queue changed without a request being added: one was decided, one
      // ran out, or the stream delivered the same requests in a fresh list.
      // None of that is news, and announcing it would post a new message
      // every few seconds for as long as traffic flows.
      return;
    }
    if (_bundle != null) {
      // A message stands and its window is open: this arrival is folded into
      // it and posted once, when the window closes. That is the difference
      // between telling and nagging, and it is the only thing that holds an
      // arrival back. While no window is open -- nothing stands, or the last
      // one has run out -- the arrival is posted at once.
      _pending = true;
      return;
    }
    _post();
  }

  void _post() {
    _serial++;
    _notice = _composeNotice();
    _live = true;
    _bundle?.cancel();
    _bundle = Timer(notificationBundle, _flush);
  }

  void _flush() {
    _bundle = null;
    if (!_pending) {
      return;
    }
    _pending = false;
    if (!_live || _focused || !_connected || _held.isEmpty) {
      return;
    }
    _post();
    state = _compose();
  }

  void _drop() {
    _pending = false;
    _bundle?.cancel();
    _bundle = null;
    _live = false;
    _notice = null;
  }

  HeldNotice _composeNotice() {
    final Flow oldest = _oldest();
    return HeldNotice(
      flowId: oldest.id,
      host: oldest.host,
      method: oldest.methodLabel,
      path: oldest.path,
      remaining: oldest.remainingAt(DateTime.now()),
      total: _held.length,
      findings: oldest.findingCount,
      serial: _serial,
    );
  }

  ReturnNotice? _returnBanner() {
    if (!_connected || _stale || _held.isEmpty) {
      return null;
    }
    final Flow oldest = _oldest();
    final Duration waited = oldest.heldFor(DateTime.now());
    if (waited < returnAfter) {
      return null;
    }
    return ReturnNotice(flowId: oldest.id, waited: waited);
  }

  /// The request that has waited longest.
  ///
  /// Not the first row of the queue: that one is sorted by deadline, and a
  /// request with a shorter budget can stand above one that arrived earlier.
  Flow _oldest() {
    Flow oldest = _held.first;
    for (final Flow flow in _held) {
      if (_since(flow).isBefore(_since(oldest))) {
        oldest = flow;
      }
    }
    return oldest;
  }

  static DateTime _since(Flow flow) => flow.heldAt ?? flow.receivedAt;

  AttentionState _compose() {
    if (!_connected || _stale) {
      return const AttentionState(tray: TrayIconState.offline);
    }
    final int held = _held.length;
    return AttentionState(
      tray: _timedOutAway > 0
          ? TrayIconState.alert
          : (held == 0 ? TrayIconState.idle : TrayIconState.held),
      held: held,
      timedOutAway: _timedOutAway,
      notice: _notice,
      banner: held == 0 ? null : _banner,
    );
  }
}
