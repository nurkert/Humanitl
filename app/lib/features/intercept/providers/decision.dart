/// The decision: what the action bar drafts, what it sends and what comes
/// back (HUM-028), for one request or for a whole group (HUM-029).
///
/// Four pieces of state, deliberately apart: the draft of the rule
/// ([RememberDraft]), the arming of the irreversible half ([AllowArmed]), the
/// call in flight ([InterceptDecision]) and the outcome that stays readable
/// afterwards ([LastDecision]). A widget that shows one of them watches one of
/// them; a decision must not rebuild the shell (`docs/UX.md` 7).
library;

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:freezed_annotation/freezed_annotation.dart';
import 'package:humanitl_ui/humanitl_ui.dart' show HMotion;
import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_diagnostics.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/daemon_client.dart';
import '../rule_sentence.dart';
import 'flows.dart';
import 'note.dart';
import 'selection.dart';

part 'decision.freezed.dart';
part 'decision.g.dart';

/// Why a keypress or a click did nothing.
///
/// A refused input is never silent: the control that would have acted shows
/// its fill and runs empty again, and the reason stands beside it
/// (`docs/UX.md` 5.3).
enum RefusalReason {
  /// Nothing is selected.
  noFlow,

  /// The URL of the selection has not been readable for [HMotion.rearm] yet.
  notArmed,

  /// The key that took the last decision is still down.
  keyHeld,

  /// The daemon has not answered the decision before this one.
  sending,

  /// The block control was clicked but not held (`docs/UX.md` 5.4).
  holdIt,

  /// The registrable domain of the selected flow is not known, so a rule
  /// cannot be written for it (`backlog/CONVENTIONS.md` 4.13).
  apexUnknown,

  /// The release valve was clicked but not held, while a finding is
  /// unresolved (`docs/UX.md` 4.7).
  holdToSend,
}

/// Above how many requests a decision asks in a modal first.
///
/// Care grows with reach, not with how dangerous something looks: one request
/// and two to five are protected by time -- the arming of the release valve,
/// the 250 ms hold of Block -- and everything wider gets the one modal this
/// screen has (`docs/UX.md` 5.4). A modal per click would destroy the rhythm
/// of the queue; a modal for twelve requests is the only honest way to show
/// what is about to leave.
const int modalAboveReach = 5;

/// Why a decision asks before it happens.
enum ConfirmReason {
  /// It covers more than [modalAboveReach] requests.
  reach,

  /// It would write a rule that outlives the session (`docs/UX.md` 5.4).
  forever,
}

/// A decision that waits for the modal.
@immutable
class BatchRequest {
  /// Creates a request over [flows].
  const BatchRequest({
    required this.kind,
    required this.flows,
    this.remember = false,
    this.reason = ConfirmReason.reach,
    this.withNote = true,
  });

  /// Allow or block; nothing else reaches a group.
  final DecisionKind kind;

  /// The requests the decision covers, in queue order.
  final List<Flow> flows;

  /// True while the decision would also create the rule of the draft.
  final bool remember;

  /// Why this decision is being asked about.
  final ConfirmReason reason;

  /// Whether the note of the action bar travels with it.
  final bool withNote;

  /// The one host of the batch, or an empty string when it spans several.
  ///
  /// Never a registrable domain worked out here: the public suffix list lives
  /// in the daemon's catalog, and this sentence guards an irreversible send
  /// (`backlog/CONVENTIONS.md` 4.13). Whoever shows it says "n hosts" instead
  /// when [hostCount] is larger than one.
  String get host => hostCount == 1 ? batchHosts(flows).single : '';

  /// How many different hosts the batch covers.
  int get hostCount => batchHosts(flows).length;

  /// How many requests are about to be decided.
  int get length => flows.length;

  @override
  bool operator ==(Object other) =>
      other is BatchRequest &&
      other.kind == kind &&
      other.remember == remember &&
      other.reason == reason &&
      other.withNote == withNote &&
      listEquals(other.flows, flows);

  @override
  int get hashCode =>
      Object.hash(kind, remember, reason, withNote, Object.hashAll(flows));
}

/// The distinct hosts of [flows], in the order they were met.
///
/// What a confirmation names has to be true for every request in it. Two hosts
/// are two hosts; the domain under them is the daemon's answer, not ours, and
/// a guessed one would name a public suffix as a domain
/// (`backlog/CONVENTIONS.md` 4.13).
List<String> batchHosts(List<Flow> flows) {
  final List<String> hosts = <String>[];
  for (final Flow flow in flows) {
    if (!hosts.contains(flow.host)) {
      hosts.add(flow.host);
    }
  }
  return hosts;
}

/// The batch decision that waits for an answer, or null.
///
/// A provider and not a field of the screen, because the modal, the action
/// bar and the shortcuts of the screen all have to know that it is open: a
/// modal takes the decision keys of the screen out of service while it stands
/// (`docs/UX.md` 5.4).
@Riverpod(keepAlive: true)
class BatchConfirm extends _$BatchConfirm {
  @override
  BatchRequest? build() => null;

  /// Asks before [request] happens.
  void ask(BatchRequest request) => state = request;

  /// Closes the modal without deciding anything.
  void cancel() => state = null;
}

/// A refused input, with a counter so that the same reason twice in a row is
/// two visible refusals and not one.
@immutable
class Refusal {
  /// Creates a refusal.
  const Refusal({required this.reason, required this.serial});

  /// Why the input did nothing.
  final RefusalReason reason;

  /// How many refusals came before this one; makes two equal reasons differ.
  final int serial;

  @override
  bool operator ==(Object other) =>
      other is Refusal && other.reason == reason && other.serial == serial;

  @override
  int get hashCode => Object.hash(reason, serial);
}

/// What the action bar would remember, and whether it shows the grid.
@immutable
class RememberState {
  /// Creates a draft state.
  const RememberState({
    this.open = false,
    this.duration = RememberDuration.session,
    this.target = RememberTarget.host,
  });

  /// True while the grid is visible. A closed grid remembers nothing: `Enter`
  /// allows once and unchanged (BACKLOG.md 5).
  final bool open;

  /// The duration the grid shows. The default is the session, so that opening
  /// the grid offers the smallest rule that outlives the request.
  final RememberDuration duration;

  /// The scope the grid shows.
  final RememberTarget target;

  /// The duration that actually applies: nothing while the grid is closed.
  RememberDuration get effective => open ? duration : RememberDuration.once;

  /// True while a decision would create a rule.
  bool get remembers => effective != RememberDuration.once;

  /// A copy with the given fields replaced.
  RememberState copyWith({
    bool? open,
    RememberDuration? duration,
    RememberTarget? target,
  }) => RememberState(
    open: open ?? this.open,
    duration: duration ?? this.duration,
    target: target ?? this.target,
  );

  @override
  bool operator ==(Object other) =>
      other is RememberState &&
      other.open == open &&
      other.duration == duration &&
      other.target == target;

  @override
  int get hashCode => Object.hash(open, duration, target);
}

/// The last refused input, until the control has shown it.
///
/// Lives in a provider because the refusal happens where the key is read --
/// on the screen -- and is shown where the control stands, in the action bar.
@Riverpod(keepAlive: true)
class LastRefusal extends _$LastRefusal {
  int _serial = 0;

  @override
  Refusal? build() => null;

  /// Records a refused input with [reason].
  void refuse(RefusalReason reason) {
    _serial++;
    state = Refusal(reason: reason, serial: _serial);
  }

  /// Forgets the refusal, once a decision went through.
  void clear() => state = null;
}

/// The draft of the rule behind the next decision.
///
/// Resets with every new selection: a rule drafted for one request must never
/// travel to the next one unseen (`backlog/CONVENTIONS.md` 4.13).
@Riverpod(keepAlive: true)
class RememberDraft extends _$RememberDraft {
  @override
  RememberState build() {
    ref.watch(selectedFlowIdProvider);
    return const RememberState();
  }

  /// Shows the grid.
  void open() => state = state.copyWith(open: true);

  /// Shows or hides the grid.
  void toggle() => state = state.copyWith(open: !state.open);

  /// Chooses [duration]; opens the grid, because a choice nobody can see is
  /// not a choice.
  void setDuration(RememberDuration duration) =>
      state = state.copyWith(open: true, duration: duration);

  /// Chooses [target].
  ///
  /// The registrable domain is the one scope the app cannot work out itself:
  /// the public suffix list lives in the daemon's catalog, and a client that
  /// guessed would offer a rule for a domain nobody registered. While the
  /// daemon has not said what the apex is, the scope is refused with a reason
  /// instead of being written from a guess (`backlog/CONVENTIONS.md` 4.13).
  void setTarget(RememberTarget target) {
    if (target == RememberTarget.apex &&
        ref.read(selectedApexProvider).isEmpty) {
      ref.read(lastRefusalProvider.notifier).refuse(RefusalReason.apexUnknown);
      return;
    }
    state = state.copyWith(open: true, target: target);
  }

  /// Forgets the draft, after a decision consumed it.
  void reset() => state = const RememberState();
}

/// True once the URL of the selected flow has stood still for
/// [HMotion.rearm].
///
/// Allowing is irreversible, `Enter` is one key, and the selection moves on
/// after every decision. Blocking is not gated: the agent may retry
/// (`docs/UX.md` 5.4).
@Riverpod(keepAlive: true)
class AllowArmed extends _$AllowArmed {
  Timer? _timer;

  @override
  bool build() {
    // Every new selection arms again, including the one the program sets
    // itself after a decision.
    ref.watch(selectedFlowIdProvider);
    _timer?.cancel();
    _timer = Timer(HMotion.rearm, () {
      if (ref.mounted) {
        state = true;
      }
    });
    ref.onDispose(() => _timer?.cancel());
    return false;
  }
}

/// What the action bar is doing.
@freezed
sealed class DecisionProgress with _$DecisionProgress {
  /// Nothing is in flight.
  const factory DecisionProgress.idle() = DecisionIdle;

  /// `Decide` is in flight for [flowId].
  const factory DecisionProgress.sending({
    required FlowId flowId,
    required DecisionKind kind,
  }) = DecisionSending;

  /// The daemon refused; the card under the bar says why.
  const factory DecisionProgress.failed({
    required FlowId flowId,
    required Diagnostic diagnostic,
  }) = DecisionFailed;

  const DecisionProgress._();

  /// True while a decision waits for the daemon.
  bool get isSending => this is DecisionSending;
}

/// Sends decisions and remembers what came back.
///
/// The only mutation the intercept screen performs, and the one place that
/// decides whether a decision may happen at all: the keyboard and the pointer
/// call the same two methods, so a rule that holds for one holds for both
/// (ADR-018 -- the surface is a thin client).
@Riverpod(keepAlive: true)
class InterceptDecision extends _$InterceptDecision {
  @override
  DecisionProgress build() => const DecisionProgress.idle();

  /// Sends the selected request on.
  ///
  /// Refuses, with a reason, while nothing is selected, while another decision
  /// waits for the daemon, and while the URL has not been readable for
  /// [HMotion.rearm]. [remember] forces the rule of the draft even when the
  /// grid is closed; that is what holding the release valve does.
  ///
  /// [confirmed] says that the person did more than click: they held the
  /// control down, or they used a key, which cannot slip sideways into the
  /// wrong control. While a finding is unresolved, only a confirmed send goes
  /// through (`docs/UX.md` 4.7).
  Future<void> allow({bool remember = false, bool confirmed = false}) async {
    final Flow? flow = _decidable();
    if (flow == null) {
      return;
    }
    if (!ref.read(allowArmedProvider)) {
      _refuse(RefusalReason.notArmed);
      return;
    }
    if (!confirmed && ref.read(selectedFindingsProvider).isNotEmpty) {
      _refuse(RefusalReason.holdToSend);
      return;
    }
    final RememberState draft = ref.read(rememberDraftProvider);
    final RememberDuration duration = remember && !draft.open
        ? RememberDuration.session
        : draft.effective;
    if (duration == RememberDuration.forever) {
      // A rule that outlives the session is the second reach the modal exists
      // for (`docs/UX.md` 5.4).
      _ask(DecisionKind.allow, <Flow>[flow], remember, ConfirmReason.forever);
      return;
    }
    await _send(
      flow,
      const Decision.allow(),
      _rule(flow, draft, duration, RuleAction.allow),
    );
  }

  /// Refuses the selected request.
  ///
  /// Not gated by the arming: the agent may retry a block, so a block taken
  /// too early costs a retry and not a secret (`docs/UX.md` 5.4). The note of
  /// the field, if one is open, travels with it (HUM-072); the daemon
  /// sanitises it again before the agent reads it in the `403`.
  Future<void> block() async {
    final Flow? flow = _decidable();
    if (flow == null) {
      return;
    }
    final RememberState draft = ref.read(rememberDraftProvider);
    if (draft.effective == RememberDuration.forever) {
      _ask(DecisionKind.block, <Flow>[flow], false, ConfirmReason.forever);
      return;
    }
    await _send(
      flow,
      Decision.block(note: ref.read(blockNoteProvider).outgoing),
      _rule(flow, draft, draft.effective, RuleAction.block),
    );
  }

  /// Puts a decision in front of the modal.
  void _ask(
    DecisionKind kind,
    List<Flow> flows,
    bool remember,
    ConfirmReason reason, {
    bool withNote = true,
  }) => ref
      .read(batchConfirmProvider.notifier)
      .ask(
        BatchRequest(
          kind: kind,
          flows: flows,
          remember: remember,
          reason: reason,
          withNote: withNote,
        ),
      );

  /// Decides [id] directly, for the block affordance of a queue row, which
  /// acts on the row under the pointer and not on the selection.
  Future<void> send(FlowId id, Decision decision, {Flow? flow}) =>
      _send(flow, decision, null, id: id);

  /// Sends [flows] on, as one act (HUM-029).
  ///
  /// One request goes the single path of HUM-028, so nothing about a lone
  /// decision changes. Above [modalAboveReach] the decision asks first: it is
  /// the widest-reaching act of this screen and it cannot be taken back
  /// (`docs/UX.md` 5.4). [remember] applies the rule of the draft once, with
  /// the first flow; the others are decided explicitly, so none of them
  /// depends on a rule that may not exist yet.
  Future<void> allowMany(
    List<Flow> flows, {
    bool remember = false,
    bool confirmed = false,
  }) async {
    if (flows.length == 1) {
      await allow(remember: remember, confirmed: confirmed);
      return;
    }
    if (!_batchable(flows)) {
      return;
    }
    if (!ref.read(allowArmedProvider)) {
      _refuse(RefusalReason.notArmed);
      return;
    }
    // Over a whole group the row's own count is what is known about the
    // others: their detail was never fetched, and the safe reading of a
    // finding nobody looked at is that it is unresolved (`docs/UX.md` 4.7).
    if (!confirmed &&
        (ref.read(selectedFindingsProvider).isNotEmpty ||
            flows.any((Flow flow) => flow.findingCount > 0))) {
      _refuse(RefusalReason.holdToSend);
      return;
    }
    final ConfirmReason? asks = _reasonToAsk(flows, remember);
    if (asks != null) {
      _ask(DecisionKind.allow, flows, remember, asks);
      return;
    }
    await _many(flows, DecisionKind.allow, remember: remember);
  }

  /// Refuses [flows], as one act.
  ///
  /// Blocking is not gated by the arming -- the agent may retry -- but it is
  /// gated by reach: above [modalAboveReach] the modal names the host and
  /// what the agent gets instead.
  Future<void> blockMany(List<Flow> flows, {bool withNote = true}) async {
    if (flows.length == 1 && withNote) {
      await block();
      return;
    }
    if (!_batchable(flows)) {
      return;
    }
    final ConfirmReason? asks = _reasonToAsk(flows, false);
    if (asks != null) {
      _ask(DecisionKind.block, flows, false, asks, withNote: withNote);
      return;
    }
    await _many(flows, DecisionKind.block, withNote: withNote);
  }

  /// Why this decision has to be asked about first, or null.
  ///
  /// Reach and permanence are the two things a modal exists for; where both
  /// apply, the wider one names the modal (`docs/UX.md` 5.4).
  ///
  /// A decision that spans several hosts always asks, however few requests it
  /// covers: the queue groups by the registrable domain of `psl.dart`, and
  /// that table guesses. Two strangers under one guessed domain are two
  /// registrants, and the modal is where their hosts are listed before
  /// anything leaves (`backlog/CONVENTIONS.md` 4.13).
  ConfirmReason? _reasonToAsk(List<Flow> flows, bool remember) {
    if (flows.length > modalAboveReach || batchHosts(flows).length > 1) {
      return ConfirmReason.reach;
    }
    final RememberState draft = ref.read(rememberDraftProvider);
    final RememberDuration duration = remember && !draft.open
        ? RememberDuration.session
        : draft.effective;
    return duration == RememberDuration.forever ? ConfirmReason.forever : null;
  }

  /// Asks about sending every held request (`Queue: allow all…`).
  ///
  /// The only "allow all" in the program, and it exists as a palette command
  /// and nowhere else: no control on the screen carries that label, and this
  /// one never sends silently -- it always opens the modal, which names the
  /// hosts and lists the requests first (HUM-029, `docs/UX.md` 4.6).
  void askAllowAll() {
    final List<Flow> held = ref.read(heldFlowsProvider);
    if (held.isEmpty || state.isSending) {
      return;
    }
    _ask(DecisionKind.allow, held, false, ConfirmReason.reach);
  }

  /// Carries out the batch the modal was asking about.
  Future<void> confirmBatch() async {
    final BatchRequest? request = ref.read(batchConfirmProvider);
    if (request == null) {
      return;
    }
    ref.read(batchConfirmProvider.notifier).cancel();
    await _many(
      request.flows,
      request.kind,
      remember: request.remember,
      withNote: request.withNote,
    );
  }

  /// True while [flows] can be decided at all; records the reason if not.
  bool _batchable(List<Flow> flows) {
    if (state.isSending) {
      _refuse(RefusalReason.sending);
      return false;
    }
    if (flows.isEmpty || flows.any((Flow flow) => !flow.isHeld)) {
      _refuse(RefusalReason.noFlow);
      return false;
    }
    return true;
  }

  /// Sends one decision per flow, in order, with the rule attached once.
  ///
  /// Sequentially and not with `Future.wait`: the rule has to exist before the
  /// decisions that follow it, and a daemon that answers out of order would
  /// otherwise hold what the rule already covers (HUM-029, pitfall).
  Future<void> _many(
    List<Flow> flows,
    DecisionKind kind, {
    bool remember = false,
    bool withNote = true,
  }) async {
    if (flows.isEmpty || state.isSending) {
      return;
    }
    final RememberState draft = ref.read(rememberDraftProvider);
    final RememberDuration duration = remember && !draft.open
        ? RememberDuration.session
        : draft.effective;
    final Rule? rule = _rule(
      flows.first,
      draft,
      duration,
      kind == DecisionKind.block ? RuleAction.block : RuleAction.allow,
    );
    final Decision decision = kind == DecisionKind.block
        // A block out of a row acts on the row under the pointer, not on the
        // selection; the note of the selected request has nothing to do with
        // it and does not travel (HUM-072).
        ? Decision.block(
            note: withNote ? ref.read(blockNoteProvider).outgoing : null,
          )
        : const Decision.allow();
    ref.read(lastRefusalProvider.notifier).clear();
    state = DecisionProgress.sending(flowId: flows.first.id, kind: kind);
    final DaemonClient client = ref.read(daemonClientProvider);
    Rule? created;
    int size = 0;
    int done = 0;
    try {
      for (int i = 0; i < flows.length; i++) {
        final Rule? back = await client.decide(
          flows[i].id,
          decision,
          remember: i == 0 ? rule : null,
        );
        if (!ref.mounted) {
          return;
        }
        created ??= back;
        size += flows[i].requestSize;
        done++;
      }
    } on DaemonException catch (error) {
      _abort(flows, done, kind, size, created, error.diagnostic);
      return;
    } on Object catch (error) {
      _abort(
        flows,
        done,
        kind,
        size,
        created,
        ClientDiagnostics.daemonUnreachable(
          socketPath: '?',
          detail: error.toString(),
        ),
      );
      return;
    }
    state = const DecisionProgress.idle();
    _record(flows, kind, size, created);
    _consumeDrafts();
    ref.read(selectionProvider.notifier).clear();
  }

  /// Records what a run of decisions did, for the strip and the announcement.
  void _record(List<Flow> flows, DecisionKind kind, int size, Rule? rule) => ref
      .read(lastDecisionProvider.notifier)
      .record(
        flowId: flows.first.id,
        kind: kind,
        host: batchHosts(flows).first,
        size: size,
        rule: rule,
        count: flows.length,
        hostCount: batchHosts(flows).length,
      );

  /// Stops a run that failed in the middle, without losing what already left.
  ///
  /// [done] requests are out and cannot be taken back, so they are recorded
  /// as what they are; the diagnostic hangs on the first request that did not
  /// go, because that is the one somebody can try again (`docs/UX.md` 4.4).
  void _abort(
    List<Flow> flows,
    int done,
    DecisionKind kind,
    int size,
    Rule? rule,
    Diagnostic diagnostic,
  ) {
    if (!ref.mounted) {
      return;
    }
    if (done > 0) {
      _record(flows.sublist(0, done), kind, size, rule);
      _consumeDrafts();
    }
    state = DecisionProgress.failed(
      flowId: flows[done < flows.length ? done : flows.length - 1].id,
      diagnostic: diagnostic,
    );
  }

  /// Forgets the rule draft and the note; both belong to the decision that
  /// just happened and to no other.
  void _consumeDrafts() {
    ref.read(rememberDraftProvider.notifier).reset();
    ref.read(blockNoteProvider.notifier).close();
  }

  /// The selected flow, or null with the reason recorded.
  Flow? _decidable() {
    if (state.isSending) {
      _refuse(RefusalReason.sending);
      return null;
    }
    final Flow? flow = ref.read(selectedFlowProvider);
    if (flow == null || !flow.isHeld) {
      _refuse(RefusalReason.noFlow);
      return null;
    }
    return flow;
  }

  Rule? _rule(
    Flow flow,
    RememberState draft,
    RememberDuration duration,
    RuleAction action,
  ) => buildRule(
    RuleDraft(
      duration: duration,
      target: draft.target,
      flow: flow,
      action: action,
    ),
    now: DateTime.now(),
    apexOf: ref.read(apexResolverProvider),
  );

  void _refuse(RefusalReason reason) =>
      ref.read(lastRefusalProvider.notifier).refuse(reason);

  Future<void> _send(
    Flow? flow,
    Decision decision,
    Rule? remember, {
    FlowId? id,
  }) async {
    final FlowId? flowId = id ?? flow?.id;
    if (flowId == null || state.isSending) {
      return;
    }
    ref.read(lastRefusalProvider.notifier).clear();
    state = DecisionProgress.sending(flowId: flowId, kind: decision.kind);
    try {
      final Rule? created = await ref
          .read(daemonClientProvider)
          .decide(flowId, decision, remember: remember);
      if (!ref.mounted) {
        return;
      }
      state = const DecisionProgress.idle();
      ref
          .read(lastDecisionProvider.notifier)
          .record(
            flowId: flowId,
            kind: decision.kind,
            host: flow?.host ?? '',
            size: flow?.requestSize ?? 0,
            rule: created,
          );
      _consumeDrafts();
    } on DaemonException catch (error) {
      if (ref.mounted) {
        state = DecisionProgress.failed(
          flowId: flowId,
          diagnostic: error.diagnostic,
        );
      }
    } on Object catch (error) {
      if (ref.mounted) {
        state = DecisionProgress.failed(
          flowId: flowId,
          diagnostic: ClientDiagnostics.daemonUnreachable(
            socketPath: '?',
            detail: error.toString(),
          ),
        );
      }
    }
  }

  /// Forgets a failure, so the next selection starts clean.
  void clear() {
    if (!state.isSending) {
      state = const DecisionProgress.idle();
    }
  }
}

/// What became of the last decision, as long as it is worth saying.
@freezed
sealed class DecisionOutcome with _$DecisionOutcome {
  /// Nothing was decided yet, or the window has passed.
  const factory DecisionOutcome.none() = DecisionOutcomeNone;

  /// A decision went through; [rule] is set when it created one.
  ///
  /// [count] is how many requests it covered: the strip and the announcement
  /// say "12 sent" instead of naming one of twelve (HUM-029). [hostCount] is
  /// how many hosts those were, so that the sentence can say "3 hosts" where
  /// it cannot name one.
  const factory DecisionOutcome.done({
    required FlowId flowId,
    required DecisionKind kind,
    required String host,
    required int size,
    Rule? rule,
    @Default(1) int count,
    @Default(1) int hostCount,
  }) = DecisionOutcomeDone;

  /// The rule was taken back again.
  const factory DecisionOutcome.undone() = DecisionOutcomeUndone;

  /// Taking the rule back failed; the rule screen can still delete it.
  const factory DecisionOutcome.undoFailed({required Diagnostic diagnostic}) =
      DecisionOutcomeUndoFailed;

  const DecisionOutcome._();

  /// The rule that can still be taken back, or null.
  Rule? get undoable => switch (this) {
    DecisionOutcomeDone(:final Rule? rule) => rule?.id == null ? null : rule,
    _ => null,
  };
}

/// The outcome of the last decision, and the undo of the rule it created.
///
/// "Undo" always means the rule, never the request: the request is already
/// out, and a promise to call it back would be a lie (`docs/UX.md` 4.5). The
/// strip disappears after [HMotion.undoWindow]; the rule stays deletable in
/// the rules screen.
@Riverpod(keepAlive: true)
class LastDecision extends _$LastDecision {
  Timer? _window;

  @override
  DecisionOutcome build() {
    ref.onDispose(() => _window?.cancel());
    return const DecisionOutcome.none();
  }

  /// Records a decision the daemon accepted.
  void record({
    required FlowId flowId,
    required DecisionKind kind,
    required String host,
    required int size,
    Rule? rule,
    int count = 1,
    int hostCount = 1,
  }) {
    _window?.cancel();
    state = DecisionOutcome.done(
      flowId: flowId,
      kind: kind,
      host: host,
      size: size,
      rule: rule,
      count: count,
      hostCount: hostCount,
    );
    _window = Timer(HMotion.undoWindow, () {
      if (ref.mounted) {
        state = const DecisionOutcome.none();
      }
    });
  }

  /// Takes back the rule the last decision created.
  Future<void> undo() async {
    final Rule? rule = state.undoable;
    final RuleId? id = rule?.id;
    if (id == null) {
      return;
    }
    try {
      await ref.read(daemonClientProvider).removeRule(id);
      if (ref.mounted) {
        _window?.cancel();
        state = const DecisionOutcome.undone();
        _window = Timer(HMotion.undoWindow, () {
          if (ref.mounted) {
            state = const DecisionOutcome.none();
          }
        });
      }
    } on DaemonException catch (error) {
      if (ref.mounted) {
        state = DecisionOutcome.undoFailed(diagnostic: error.diagnostic);
      }
    } on Object catch (error) {
      if (ref.mounted) {
        state = DecisionOutcome.undoFailed(
          diagnostic: ClientDiagnostics.daemonUnreachable(
            socketPath: '?',
            detail: error.toString(),
          ),
        );
      }
    }
  }

  /// Clears the strip, for a new selection.
  void clear() {
    _window?.cancel();
    state = const DecisionOutcome.none();
  }
}

/// The unresolved findings of the selected request.
///
/// [count] is what the row already says; [known] is what the detail added.
/// While the detail is still on its way the count stands alone, and the safe
/// reading of a finding nobody has looked at is that it is unresolved: the
/// send keeps its hold, the sentence keeps the number, and the name follows
/// as soon as the daemon has answered (`docs/UX.md` 4.7).
@immutable
class FindingSet {
  /// Creates a set of [count] findings, of which [known] are described.
  const FindingSet({this.count = 0, this.known = const <Finding>[]});

  /// Nothing was found.
  static const FindingSet none = FindingSet();

  /// How many findings are unresolved.
  final int count;

  /// The ones the detail describes, in the order the daemon found them.
  final List<Finding> known;

  /// True while at least one finding is unresolved.
  bool get isNotEmpty => count > 0;

  /// The first described finding, or null while only the number is known.
  Finding? get first => known.isEmpty ? null : known.first;

  @override
  bool operator ==(Object other) =>
      other is FindingSet &&
      other.count == count &&
      listEquals(other.known, known);

  @override
  int get hashCode => Object.hash(count, Object.hashAll(known));
}

/// The unresolved findings of the selected flow.
@Riverpod(keepAlive: true)
FindingSet selectedFindings(Ref ref) {
  final Flow? flow = ref.watch(selectedFlowProvider);
  if (flow == null) {
    return FindingSet.none;
  }
  final List<Finding>? found = ref
      .watch(flowDetailProvider(flow.id))
      .value
      ?.findings;
  if (found == null) {
    return FindingSet(count: flow.findingCount);
  }
  final List<Finding> open = <Finding>[
    for (final Finding finding in found)
      if (!finding.resolved) finding,
  ];
  return FindingSet(count: open.length, known: open);
}

/// The registrable domain of the selected flow, as the daemon knows it.
///
/// Empty while the daemon has not said one. The answer belongs to the catalog
/// (HUM-031), which carries the public suffix list; a client that guessed it
/// would write a rule for a domain nobody registered, and the sentence above
/// the bar would promise a rule the daemon never creates. The bundled table in
/// `psl.dart` groups the queue and does nothing else.
@Riverpod(keepAlive: true)
String selectedApex(Ref ref) {
  final FlowId? id = ref.watch(selectedFlowIdProvider);
  if (id == null) {
    return '';
  }
  return ref.watch(flowDetailProvider(id)).value?.domain?.apex ?? '';
}

/// The registrable domain of a host, as the rule draft needs it.
///
/// Answers only for the host of the selected flow, and only what the daemon
/// said; every other host and every unknown apex comes back empty, and an
/// empty apex means the scope is not available (see [selectedApex]).
@Riverpod(keepAlive: true)
String Function(String host) apexResolver(Ref ref) {
  final Flow? flow = ref.watch(selectedFlowProvider);
  final String apex = ref.watch(selectedApexProvider);
  return (String host) =>
      apex.isNotEmpty && flow != null && host == flow.host ? apex : '';
}
