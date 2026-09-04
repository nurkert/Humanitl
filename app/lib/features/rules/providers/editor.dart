/// The rule editor: the draft, what the daemon said about it and the dry run
/// that shows what it would have matched (HUM-033).
///
/// The draft is a [Rule], not a bag of strings: the form edits the value that
/// goes on the wire, so the preview sentence, the dry run and the saved rule
/// are the same object all the way down.
library;

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/daemon_client.dart';
import '../rule_text.dart';
import 'rules.dart';

part 'editor.g.dart';

/// How long the editor waits after the last keystroke before it asks for a
/// dry run.
///
/// Policy, not motion: nothing moves for it. Long enough that typing a host
/// costs one call instead of one per letter, short enough that the answer
/// still feels like an answer to what was typed.
const Duration ruleDryRunDebounce = Duration(milliseconds: 400);

/// What the editor pane shows.
@immutable
class RuleEditorState {
  /// Creates a state. A null [draft] means the editor is closed.
  const RuleEditorState({
    this.draft,
    this.editing,
    this.readOnly = false,
    this.saving = false,
    this.error,
    this.generation = 0,
  });

  /// The editor is closed.
  static const RuleEditorState closed = RuleEditorState();

  /// The rule being written.
  final Rule? draft;

  /// The rule being changed, or null while a new one is being written.
  final RuleId? editing;

  /// True for a bundled rule: it is shown, never changed (RULES_010).
  final bool readOnly;

  /// True while `Rules(add)` or `Rules(update)` is in flight.
  final bool saving;

  /// What the daemon said when it refused to save. Shown under the form, at
  /// the control that failed, with the daemon's own words (`docs/UX.md` 4.4).
  final Diagnostic? error;

  /// Counts every open and every close.
  ///
  /// The form holds text controllers, and they may only be refilled when the
  /// editor was opened on something else -- never while somebody types. Two
  /// opens on the same rule differ in nothing but this number, and that is
  /// exactly what the form needs to tell them apart.
  final int generation;

  /// True while the editor shows something.
  bool get isOpen => draft != null;

  /// True while the draft would create a rule rather than change one.
  bool get isNew => editing == null;

  /// The same state with [error] cleared and the fields given replaced.
  RuleEditorState withDraft(Rule draft) => RuleEditorState(
    draft: draft,
    editing: editing,
    readOnly: readOnly,
    generation: generation,
  );

  @override
  bool operator ==(Object other) =>
      other is RuleEditorState &&
      other.draft == draft &&
      other.editing == editing &&
      other.readOnly == readOnly &&
      other.saving == saving &&
      other.error == error &&
      other.generation == generation;

  @override
  int get hashCode =>
      Object.hash(draft, editing, readOnly, saving, error, generation);
}

/// The editor.
@Riverpod(keepAlive: true, name: 'ruleEditorProvider')
class RuleEditorController extends _$RuleEditorController {
  @override
  RuleEditorState build() => RuleEditorState.closed;

  /// Opens an empty form.
  ///
  /// The action starts at [RuleAction.ask], which is what happens anyway when
  /// no rule matches: a form that opened on `allow` would let one distracted
  /// click widen what leaves the machine (CONVENTIONS 4.13, "defaults on the
  /// safe side"). The lifetime starts at the session for the same reason
  /// (`docs/UX.md` 4.6).
  void openNew() => state = RuleEditorState(
    draft: const Rule(
      action: RuleAction.ask,
      matcher: RuleMatcher(),
      expires: RuleExpiry.session(),
    ),
    generation: state.generation + 1,
  );

  /// Opens [rule]. A bundled rule opens read-only.
  void edit(Rule rule) => state = RuleEditorState(
    draft: rule,
    editing: rule.id,
    readOnly: rule.bundled,
    generation: state.generation + 1,
  );

  /// Opens a new `ask` rule with the match of [bundled], to be put in front
  /// of it. A bundled rule cannot be changed; a rule of one's own above it
  /// wins, and that is the whole mechanism (RULES_010).
  void overrideBundled(Rule bundled) => state = RuleEditorState(
    draft: Rule(
      action: RuleAction.ask,
      matcher: bundled.matcher,
      expires: const RuleExpiry.never(),
      // One-based inside the group: the front of the saved rules, which is
      // where a rule has to stand to win against a bundled one.
      position: 1,
    ),
    generation: state.generation + 1,
  );

  /// Closes the editor without saving.
  void close() => state = RuleEditorState(generation: state.generation + 1);

  /// Replaces the draft.
  void setDraft(Rule draft) {
    if (state.draft == null || state.readOnly) {
      return;
    }
    state = state.withDraft(draft);
  }

  /// Replaces the matcher of the draft.
  void setMatcher(RuleMatcher matcher) {
    final Rule? draft = state.draft;
    if (draft != null) {
      setDraft(draft.copyWith(matcher: matcher));
    }
  }

  /// Adds or removes [method] from the draft.
  void toggleMethod(Method method) {
    final Rule? draft = state.draft;
    if (draft == null) {
      return;
    }
    final List<Method> methods = List<Method>.of(draft.matcher.methods);
    if (!methods.remove(method)) {
      methods.add(method);
    }
    // Kept in the order of the enum, so that `GET,POST` reads the same way
    // whichever chip was tapped first.
    methods.sort((Method a, Method b) => a.index.compareTo(b.index));
    setMatcher(
      draft.matcher.copyWith(methods: List<Method>.unmodifiable(methods)),
    );
  }

  /// Saves the draft: `Rules(add)` for a new rule, `Rules(update)` for one
  /// that exists. The editor stays open when the daemon refuses, with the
  /// diagnostic under the form.
  Future<void> save() async {
    final RuleEditorState current = state;
    final Rule? draft = current.draft;
    if (draft == null || current.readOnly || current.saving) {
      return;
    }
    state = RuleEditorState(
      draft: draft,
      editing: current.editing,
      saving: true,
      generation: current.generation,
    );
    final Rules rules = ref.read(rulesProvider.notifier);
    final Diagnostic? failed = current.isNew
        ? await rules.add(draft)
        : await rules.change(draft);
    if (failed == null) {
      state = RuleEditorState(generation: current.generation + 1);
      return;
    }
    state = RuleEditorState(
      draft: draft,
      editing: current.editing,
      error: failed,
      generation: current.generation,
    );
  }
}

/// The draft reduced to what decides a match.
///
/// The dry run asks the daemon, and it must not ask again because somebody
/// typed a letter in the note: the note, the id and the place change nothing
/// about which requests a rule would have matched. The lifetime stays, and
/// deliberately: the engine skips a rule whose end time has passed, so a rule
/// that is over matches nothing, and a key that dropped the lifetime would
/// answer for a different rule than the one being written.
Rule dryRunKey(Rule draft) =>
    Rule(action: draft.action, matcher: draft.matcher, expires: draft.expires);

/// What [rule] would have matched among the recorded requests.
///
/// Debounced by [ruleDryRunDebounce]: while somebody types, every keystroke
/// builds a new instance of this provider and disposes the one before it, and
/// a disposed instance never gets as far as the call. Three quick changes are
/// therefore one round trip, not three (`backlog/sprint-2.md`, HUM-033).
@riverpod
Future<DryRun> ruleDryRun(Ref ref, Rule rule) async {
  if (!rulePassesPreCheck(rule)) {
    // Nothing to ask about yet. The panel says so; asking would answer a
    // pattern the daemon is going to refuse anyway.
    return DryRun.empty;
  }
  // A cancellable wait, not `Future.delayed`: a timer that outlives the
  // provider it belongs to would fire into a disposed instance, and in a
  // widget test it outlives the whole tree.
  final Completer<void> wait = Completer<void>();
  bool disposed = false;
  final Timer timer = Timer(ruleDryRunDebounce, () {
    if (!wait.isCompleted) {
      wait.complete();
    }
  });
  ref.onDispose(() {
    disposed = true;
    timer.cancel();
    if (!wait.isCompleted) {
      wait.complete();
    }
  });
  await wait.future;
  if (disposed) {
    // Somebody kept typing. The instance that survives asks.
    return DryRun.empty;
  }
  return ref.read(daemonClientProvider).dryRunRule(rule);
}
