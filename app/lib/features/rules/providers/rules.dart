/// The rule set, the tab, the filter and the two things that can be taken
/// back (HUM-033).
///
/// Everything here asks the daemon and keeps what it answered. The screen
/// never evaluates a rule, never sorts one and never decides whether a
/// pattern is legal: an answer to `Rules` carries the whole set in the order
/// it is evaluated, and that order is the only truth this file knows
/// (ADR-018, CONVENTIONS 4.5).
library;

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:humanitl_ui/humanitl_ui.dart' show HMotion;
import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/daemon_client.dart';

part 'rules.g.dart';

/// How often the screen looks at the clock.
///
/// Only a rule with an end time reads it, and its label counts in minutes, so
/// a minute is the finest step that changes anything. It is policy, not
/// motion: `HMotion` carries the guided movements, and the one clock token it
/// has (`HMotion.clockTick`) is the second the countdown ring needs. A minute
/// token belongs next to it (`docs/UX.md` 9).
const Duration ruleClockTick = Duration(minutes: 1);

/// The two tabs. The tab is a group of the rule set, not a way of looking at
/// it: a position counts inside a group, and dragging across a tab would ask
/// for an order the daemon cannot keep (CONVENTIONS 4.5).
enum RuleTab {
  /// The rules in `rules.yaml`, plus the bundled block below them.
  saved,

  /// The rules that live only while this session runs.
  temporary,
}

/// Riverpod 3 retries a failed provider on its own and reports `AsyncLoading`
/// with the error tucked inside while it does; a list that cannot be read
/// would then show its skeleton for ever instead of the reason. Reloading is
/// explicit here: the banner carries the button.
Duration? noRulesRetry(int retryCount, Object error) => null;

/// The whole rule set, in evaluation order, as the daemon last answered it.
///
/// It reloads when the section becomes visible again ([refresh]) and after
/// every change, because every `Rules` answer carries the complete set. The
/// `RulesChanged` event would be the third trigger; it arrives on the
/// subscription of another feature, and a second `Subscribe` for one counter
/// costs more than it is worth (see the report of HUM-033).
@Riverpod(keepAlive: true, retry: noRulesRetry)
class Rules extends _$Rules {
  @override
  Future<RuleSet> build() => ref.watch(daemonClientProvider).listRules();

  /// Asks again. Used when the section becomes visible.
  Future<void> refresh() async {
    final DaemonClient client = ref.read(daemonClientProvider);
    try {
      final RuleSet answered = await client.listRules();
      state = AsyncData<RuleSet>(answered);
      _report(answered);
    } on DaemonException catch (error) {
      state = AsyncError<RuleSet>(error, StackTrace.current);
    }
  }

  /// Creates [rule]. Answers the daemon's diagnostic when it refused, so the
  /// editor can put it under the control that failed (`docs/UX.md` 4.4).
  Future<Diagnostic?> add(Rule rule) async {
    final Diagnostic? failed = await _run(
      (DaemonClient client) => client.addRule(rule),
    );
    if (failed == null) {
      _forgetUndo(rule.id);
    }
    return failed;
  }

  /// Replaces the rule with the same id.
  ///
  /// Not called `update`: the async notifier of Riverpod 3 has a method of
  /// that name with an entirely different meaning, and two `update`s on one
  /// object is one too many.
  Future<Diagnostic?> change(Rule rule) async {
    final Diagnostic? failed = await _run(
      (DaemonClient client) => client.updateRule(rule),
    );
    if (failed == null) {
      _forgetUndo(rule.id);
    }
    return failed;
  }

  /// Deletes [rule] and remembers it for the undo strip.
  Future<Diagnostic?> remove(Rule rule) async {
    final RuleId? id = rule.id;
    if (id == null) {
      return null;
    }
    final Diagnostic? failed = await _run((DaemonClient client) async {
      await client.removeRule(id);
      return client.listRules();
    });
    if (failed == null) {
      _forgetUndo(id);
      ref
          .read(ruleUndoProvider.notifier)
          .offer(RuleUndo(kind: RuleUndoKind.removed, rule: rule));
    }
    return failed;
  }

  /// Puts a removed rule back where it stood.
  ///
  /// The position is a wish: the rule that stood there may have moved on, and
  /// the daemon hangs the rule at the end rather than refusing. A failed wish
  /// is therefore tried once more without one (`backlog/sprint-2.md`,
  /// HUM-033, Fallstricke).
  Future<Diagnostic?> restore(Rule rule) async {
    final Diagnostic? placed = await add(rule);
    if (placed == null) {
      return null;
    }
    return add(rule.copyWith(position: 0));
  }

  /// Puts the end time of [before] back on the rule that stands now.
  ///
  /// Nur die Frist, nie der ganze Schnappschuss: „Dauerhaft machen" hat genau
  /// die Frist geändert, also nimmt „Rückgängig" genau sie zurück. Schriebe es
  /// den ganzen Zustand von vorhin zurück, machte ein Knopf mit der Aufschrift
  /// „Undo" aus einer inzwischen engeren Regel wieder die weitere -- und ein
  /// Rückgängig, dessen Reichweite jemand falsch rät, ist schlimmer als keines
  /// (`docs/UX.md` 4.5).
  ///
  /// Kennt der Satz die Regel nicht mehr, fragt der Aufruf trotzdem: der
  /// Daemon besitzt die Wahrheit und antwortet mit `IPC_005` und seinen
  /// eigenen Worten (ADR-018). `update` legt nie eine Regel an, also kann
  /// dieser Weg nichts wiederbeleben.
  Future<Diagnostic?> restoreExpiry(Rule before) async {
    final RuleId? id = before.id;
    if (id == null) {
      return null;
    }
    Rule? current = ruleById(state.value, id);
    if (current == null) {
      await refresh();
      current = ruleById(state.value, id);
    }
    return change(current?.copyWith(expires: before.expires) ?? before);
  }

  /// The new order of the tab, as the complete list of rule ids.
  Future<Diagnostic?> reorder(List<RuleId> order) =>
      _run((DaemonClient client) => client.reorderRules(order));

  /// Moves a session rule into `rules.yaml`, or drops the end time of a saved
  /// one; both make the rule permanent, and the daemon has one call for the
  /// first case and none for the second.
  Future<Diagnostic?> makePermanent(Rule rule) async {
    final RuleId? id = rule.id;
    if (id == null) {
      return null;
    }
    final Diagnostic? failed = rule.expires is RuleExpirySession
        ? await _run((DaemonClient client) => client.makeRulePermanent(id))
        : await _run(
            (DaemonClient client) => client.updateRule(
              rule.copyWith(expires: const RuleExpiry.never()),
            ),
          );
    if (failed == null) {
      _forgetUndo(id);
      ref
          .read(ruleUndoProvider.notifier)
          .offer(RuleUndo(kind: RuleUndoKind.madePermanent, rule: rule));
    }
    return failed;
  }

  /// Switches a bundled rule off or back on.
  ///
  /// Der Zustand kommt aus der Antwort des Daemons und nirgends sonst: `_run`
  /// setzt den Regelsatz, den die Antwort trägt, und gibt bei einer Ablehnung
  /// dessen `Diagnostic` zurück. Ein vorweggenommener Zustand zeigte für die
  /// Dauer eines Fehlschlags eine Regel als abgeschaltet, die weiter
  /// entscheidet, und das ist genau die Behauptung, die dieses Feature
  /// abstellt (CONVENTIONS 4.13).
  ///
  /// Kein Rückgängig-Streifen: der Schalter ist sein eigenes Rückgängig, und
  /// ein Angebot daneben wäre dieselbe Handlung zweimal (`docs/UX.md` 4.5).
  Future<Diagnostic?> setDisabled(Rule rule, {required bool disabled}) async {
    final RuleId? id = rule.id;
    if (id == null) {
      return null;
    }
    return _run(
      (DaemonClient client) => client.setRuleDisabled(id, disabled: disabled),
    );
  }

  /// Reads `rules.yaml` again. What the daemon found goes into the banner,
  /// including the report of a reload that changed nothing: a reload that
  /// says nothing looks like a reload that did not happen.
  Future<void> reload() async {
    final DaemonClient client = ref.read(daemonClientProvider);
    try {
      final RuleSet answered = await client.reloadRules();
      state = AsyncData<RuleSet>(answered);
      // Die Datei kann alles enthalten, auch eine andere Fassung der Regel,
      // über die der Streifen gerade spricht. Ein Angebot, dessen Bezug
      // niemand mehr kennt, wird zurückgenommen statt geraten.
      ref.read(ruleUndoProvider.notifier).clear();
      ref.read(rulesBannerProvider.notifier).show(answered.diagnostics);
    } on DaemonException catch (error) {
      ref.read(rulesBannerProvider.notifier).show(<Diagnostic>[
        error.diagnostic,
      ]);
    }
  }

  /// Runs one operation and keeps the set it answers. The diagnostic of a
  /// refused operation is answered, never swallowed and never turned into a
  /// bare string (BACKLOG.md, principle 7).
  Future<Diagnostic?> _run(Future<RuleSet> Function(DaemonClient) call) async {
    try {
      final RuleSet answered = await call(ref.read(daemonClientProvider));
      state = AsyncData<RuleSet>(answered);
      _report(answered);
      return null;
    } on DaemonException catch (error) {
      return error.diagnostic;
    }
  }

  /// Bringt die Befunde einer geglückten Antwort über die Liste.
  ///
  /// Jede `Rules`-Antwort kann sie tragen, nicht nur die des Reloads; heute
  /// füllt der Daemon sie nur dort, und eine Antwort, deren Befunde niemand
  /// liest, wäre eine stille Warnung (`docs/UX.md` 4.4). Ein leeres Feld
  /// löscht nichts: was der Reload gemeldet hat, verschwindet nicht, weil
  /// jemand danach eine Regel angelegt hat.
  void _report(RuleSet answered) {
    if (answered.diagnostics.isNotEmpty) {
      ref.read(rulesBannerProvider.notifier).show(answered.diagnostics);
    }
  }

  /// Verwirft ein Rückgängig-Angebot, das von der Regel [id] handelt.
  ///
  /// Jede spätere Änderung an derselben Regel entwertet das Angebot: der
  /// Streifen spricht über einen Zustand, den es nicht mehr gibt, und sein
  /// Knopf trüge weiter die Aufschrift „Undo" (`docs/UX.md` 4.5).
  void _forgetUndo(RuleId? id) {
    if (id == null) {
      return;
    }
    if (ref.read(ruleUndoProvider)?.rule.id == id) {
      ref.read(ruleUndoProvider.notifier).clear();
    }
  }
}

/// Which tab shows.
@Riverpod(keepAlive: true)
class RuleTabSelection extends _$RuleTabSelection {
  @override
  RuleTab build() => RuleTab.saved;

  /// Shows [tab].
  void select(RuleTab tab) => state = tab;
}

/// What the filter field holds. It filters host and note, nothing else: a
/// filter that also searched the action would answer "block" with every
/// blocking rule and hide the one somebody meant.
@Riverpod(keepAlive: true)
class RuleQuery extends _$RuleQuery {
  @override
  String build() => '';

  /// Sets the filter.
  void set(String query) => state = query;

  /// Clears it. The empty state of a filtered list offers this.
  void clear() => state = '';
}

/// The rules of one tab: what can be dragged, what cannot, and how much the
/// filter cut away.
@immutable
class RuleChain {
  /// Creates a chain.
  const RuleChain({
    this.rules = const <Rule>[],
    this.bundled = const <Rule>[],
    this.ownTotal = 0,
    this.bundledTotal = 0,
    this.otherTab = 0,
  });

  /// An empty chain.
  static const RuleChain empty = RuleChain();

  /// The rules of the tab that the person owns, in evaluation order, after
  /// the filter.
  final List<Rule> rules;

  /// The bundled rules below them, after the filter. Only the saved tab has
  /// any: bundled rules are permanent by definition.
  final List<Rule> bundled;

  /// How many rules of the person the tab holds before the filter. It is the
  /// size of the group a position counts in, so it is what a row says it is
  /// one of (CONVENTIONS 4.5).
  final int ownTotal;

  /// How many bundled rules the tab holds before the filter -- their own
  /// group, with its own count.
  final int bundledTotal;

  /// How many rules the other tab holds. The chain runs across both tabs, and
  /// a tab that hid that would show an order that is not the order.
  ///
  /// Gezählt wird, was der andere Tab hält, nicht was in Kraft ist -- dieselbe
  /// Zählweise wie im Kopf, damit im selben Bild nicht zwei Zahlen über
  /// dieselbe Menge stehen. Der Satz darüber bleibt trotzdem wahr: nur eine
  /// Sitzungsregel ist temporär, und eine Sitzungsregel läuft nie ab, also
  /// sind „gehalten" und „in Kraft" für den Tab, der eine Auswertung behauptet
  /// (`rulesChainSessionFirst`), dieselbe Zahl.
  final int otherTab;

  /// How many rules the tab holds before the filter.
  int get total => ownTotal + bundledTotal;

  /// How many rules the filter kept.
  int get matched => rules.length + bundled.length;

  /// True while the tab holds nothing at all.
  bool get isEmpty => total == 0;

  @override
  bool operator ==(Object other) =>
      other is RuleChain &&
      other.ownTotal == ownTotal &&
      other.bundledTotal == bundledTotal &&
      other.otherTab == otherTab &&
      listEquals(rules, other.rules) &&
      listEquals(bundled, other.bundled);

  @override
  int get hashCode => Object.hash(
    ownTotal,
    bundledTotal,
    otherTab,
    Object.hashAll(rules),
    Object.hashAll(bundled),
  );
}

/// The chain the list pane draws: the current tab, filtered.
///
/// A value type rather than a bare list, so that a rebuild of the set that
/// changes nothing this pane shows rebuilds no row (`docs/UX.md` 7).
@Riverpod(keepAlive: true)
RuleChain visibleRules(Ref ref) {
  final RuleSet? set = ref.watch(rulesProvider).value;
  if (set == null) {
    return RuleChain.empty;
  }
  final RuleTab tab = ref.watch(ruleTabSelectionProvider);
  final String query = ref.watch(ruleQueryProvider).trim().toLowerCase();
  final List<Rule> own = <Rule>[];
  final List<Rule> bundled = <Rule>[];
  int ownTotal = 0;
  int bundledTotal = 0;
  int otherTab = 0;
  for (final Rule rule in set.rules) {
    if (tabOf(rule) != tab) {
      otherTab++;
      continue;
    }
    if (rule.bundled) {
      bundledTotal++;
    } else {
      ownTotal++;
    }
    if (!_matchesQuery(rule, query)) {
      continue;
    }
    (rule.bundled ? bundled : own).add(rule);
  }
  return RuleChain(
    rules: List<Rule>.unmodifiable(own),
    bundled: List<Rule>.unmodifiable(bundled),
    ownTotal: ownTotal,
    bundledTotal: bundledTotal,
    otherTab: otherTab,
  );
}

/// Die Regel mit [id], so wie der Daemon sie zuletzt geantwortet hat, oder
/// null.
///
/// Der eine Weg zum gespeicherten Zustand einer Regel. Ein Formular hält einen
/// Entwurf, und ein Entwurf ist keine Regel: wer wissen will, was gerade gilt,
/// fragt den Regelsatz und nicht das Formular (ADR-018).
Rule? ruleById(RuleSet? set, RuleId? id) {
  if (set == null || id == null) {
    return null;
  }
  for (final Rule rule in set.rules) {
    if (rule.id == id) {
      return rule;
    }
  }
  return null;
}

/// The tab a rule belongs to. A rule with an end time is written to the file
/// like any other saved rule; only a session rule disappears with the
/// session, and only it is temporary (CONVENTIONS 4.5).
RuleTab tabOf(Rule rule) =>
    rule.expires is RuleExpirySession ? RuleTab.temporary : RuleTab.saved;

bool _matchesQuery(Rule rule, String query) {
  if (query.isEmpty) {
    return true;
  }
  return rule.matcher.host.toLowerCase().contains(query) ||
      (rule.note ?? '').toLowerCase().contains(query);
}

/// The complete order of rule ids after the rule at [from] in [tab] moved to
/// [to].
///
/// The daemon sorts every group by the list it is given and ignores the ids
/// it does not find, so the list is sent whole: the rules of the other tab
/// keep their order because they keep their place in it, and bundled rules
/// are left out because they cannot move at all (`rules_store.rs`,
/// `reorder_all`).
List<RuleId> chainOrderAfterMove(
  RuleSet set,
  RuleTab tab, {
  required int from,
  required int to,
}) {
  final List<RuleId> session = _idsOf(set, RuleTab.temporary);
  final List<RuleId> saved = _idsOf(set, RuleTab.saved);
  final List<RuleId> moving = tab == RuleTab.saved ? saved : session;
  if (from < 0 || from >= moving.length) {
    return <RuleId>[...session, ...saved];
  }
  final RuleId id = moving.removeAt(from);
  // `to` counts in the list without the row that is moving: the reorderable
  // list adjusts it before it calls back, and `Alt` plus an arrow key means
  // the same thing.
  moving.insert(to.clamp(0, moving.length), id);
  return <RuleId>[...session, ...saved];
}

List<RuleId> _idsOf(RuleSet set, RuleTab tab) => <RuleId>[
  for (final Rule rule in set.rules)
    if (!rule.bundled && tabOf(rule) == tab && rule.id != null) rule.id!,
];

/// What the diagnostics banner above the list shows.
///
/// A failure that belongs to one control stays under that control; this is
/// for what belongs to the list as a whole: a refused file, a reorder the
/// daemon did not take, a reload report (`docs/UX.md` 4.4).
@Riverpod(keepAlive: true)
class RulesBanner extends _$RulesBanner {
  @override
  List<Diagnostic> build() => const <Diagnostic>[];

  /// Shows [diagnostics], oldest finding first.
  void show(List<Diagnostic> diagnostics) =>
      state = List<Diagnostic>.unmodifiable(diagnostics);

  /// Shows a single finding.
  void showOne(Diagnostic diagnostic) => show(<Diagnostic>[diagnostic]);

  /// Clears the banner.
  void clear() => state = const <Diagnostic>[];
}

/// Which of the two reversible changes the strip offers to take back.
enum RuleUndoKind {
  /// The rule was deleted.
  removed,

  /// A rule that used to end became permanent.
  madePermanent,
}

/// One offer to undo, as it stands in the strip above the list.
@immutable
class RuleUndo {
  /// Creates an offer.
  const RuleUndo({required this.kind, required this.rule});

  /// What happened.
  final RuleUndoKind kind;

  /// The rule as it was before, including the place it stood in.
  final Rule rule;

  @override
  bool operator ==(Object other) =>
      other is RuleUndo && other.kind == kind && other.rule == rule;

  @override
  int get hashCode => Object.hash(kind, rule);
}

/// The strip above the list, for [HMotion.undoWindow].
///
/// Undo takes back the rule, never a request that already went out
/// (`docs/UX.md` 4.5). After the window only the strip is gone; the rule
/// itself stays deletable in this very list.
@Riverpod(keepAlive: true, name: 'ruleUndoProvider')
class RuleUndoOffer extends _$RuleUndoOffer {
  Timer? _timer;

  @override
  RuleUndo? build() {
    ref.onDispose(() => _timer?.cancel());
    return null;
  }

  /// Offers [undo] for [HMotion.undoWindow].
  void offer(RuleUndo undo) {
    _timer?.cancel();
    state = undo;
    _timer = Timer(HMotion.undoWindow, clear);
  }

  /// Takes the offer back.
  void clear() {
    _timer?.cancel();
    _timer = null;
    state = null;
  }

  /// Performs the undo and clears the strip.
  Future<Diagnostic?> apply() async {
    final RuleUndo? undo = state;
    if (undo == null) {
      return null;
    }
    clear();
    final Rules rules = ref.read(rulesProvider.notifier);
    return switch (undo.kind) {
      RuleUndoKind.removed => rules.restore(undo.rule),
      RuleUndoKind.madePermanent => rules.restoreExpiry(undo.rule),
    };
  }
}

/// The clock the expiry labels read, one step every [ruleClockTick].
///
/// It stops as soon as nothing watches it, and only a rule with an end time
/// does. A provider that filters or sorts never sees it (`docs/UX.md` 7).
@riverpod
Stream<DateTime> ruleClock(Ref ref) async* {
  yield DateTime.now();
  yield* Stream<DateTime>.periodic(ruleClockTick, (_) => DateTime.now());
}
