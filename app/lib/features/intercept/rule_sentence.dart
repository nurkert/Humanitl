/// The rule behind a remembered decision: the draft, the rule it builds and
/// the sentence that shows it before it is created.
///
/// Nothing here draws. The generator is pure so that a unit test can check
/// every combination of duration and scope in both languages, and so that the
/// sentence on the screen and the rule on the wire are built from one place:
/// a sentence that says something else than the rule does is worse than no
/// sentence at all (`backlog/CONVENTIONS.md` 4.13).
library;

import '../../core/domain/domain.dart';
import '../../core/text/rule_sentence.dart' as sentence;
import '../../l10n/l10n.dart';

/// How long a remembered decision holds.
enum RememberDuration {
  /// Decide this one request and create no rule at all.
  once,

  /// While this session runs; the daemon keeps it in memory. The default.
  session,

  /// One hour from the moment the rule is created.
  oneHour,

  /// Forever; the daemon writes it to `rules.yaml`.
  forever,
}

/// What a remembered decision covers.
enum RememberTarget {
  /// This method, this host and this path.
  url,

  /// Every request to this host.
  host,

  /// The registrable domain and everything under it.
  apex,

  /// This host, but only with this method.
  hostMethod,
}

/// One draft of a rule: what the action bar currently offers to remember.
class RuleDraft {
  /// Creates a draft for [flow].
  const RuleDraft({
    required this.duration,
    required this.target,
    required this.flow,
    this.action = RuleAction.allow,
  });

  /// How long the rule would hold.
  final RememberDuration duration;

  /// What the rule would cover.
  final RememberTarget target;

  /// The request the rule is generalised from.
  final Flow flow;

  /// What the rule would do. The action bar builds allow rules from the
  /// release valve and block rules from the block control; the specification
  /// of HUM-028 shows both in its sentence table, so the generator carries the
  /// action instead of assuming one.
  final RuleAction action;

  /// The same draft with another [duration].
  RuleDraft withDuration(RememberDuration duration) =>
      RuleDraft(duration: duration, target: target, flow: flow, action: action);

  /// The same draft with another [target].
  RuleDraft withTarget(RememberTarget target) =>
      RuleDraft(duration: duration, target: target, flow: flow, action: action);

  /// The same draft with another [action].
  RuleDraft withAction(RuleAction action) =>
      RuleDraft(duration: duration, target: target, flow: flow, action: action);

  /// True while the draft creates no rule.
  bool get remembers => duration != RememberDuration.once;
}

/// The rule for `Decide.remember`, or null for [RememberDuration.once].
///
/// [now] is the clock the one-hour expiry is measured from, [apexOf] answers
/// the registrable domain of a host. The apex comes from the daemon
/// (`DomainInfo.apex`), never from a guess in the widget; the caller decides
/// where it takes the answer from and this function stays testable.
///
/// The specification also lists a `session` parameter. The wire has no place
/// for it: `RuleExpiry.session` is an empty message, because the daemon has
/// exactly one session (`backlog/CONVENTIONS.md` 4.12, IPC). A parameter that
/// reaches nothing would only look like a promise.
Rule? buildRule(
  RuleDraft draft, {
  required DateTime now,
  required String Function(String host) apexOf,
}) {
  if (!draft.remembers || hostPattern(draft, apexOf).isEmpty) {
    return null;
  }
  final Flow flow = draft.flow;
  return Rule(
    action: draft.action,
    matcher: _matcherOf(draft, apexOf: apexOf),
    expires: switch (draft.duration) {
      RememberDuration.once => const RuleExpiry.session(),
      RememberDuration.session => const RuleExpiry.session(),
      RememberDuration.oneHour => RuleExpiry.at(
        at: now.add(const Duration(hours: 1)),
      ),
      RememberDuration.forever => const RuleExpiry.never(),
    },
    createdFrom: flow.id,
  );
}

/// The host pattern the rule matches on, as it appears in the rule and in the
/// sentence. Both read the same string, so they cannot drift apart.
///
/// Empty when the scope is the registrable domain and [apexOf] does not know
/// one: the public suffix list lives in the daemon's catalog, and a guessed
/// domain would be a rule nobody asked for
/// (`backlog/CONVENTIONS.md` 4.13). The grid refuses that scope before it gets
/// here; [buildRule] and [ruleSentence] refuse it a second time, because a
/// sentence that promises a rule the daemon never creates is worse than no
/// sentence.
String hostPattern(RuleDraft draft, String Function(String host) apexOf) {
  if (draft.target != RememberTarget.apex) {
    return draft.flow.host;
  }
  final String apex = apexOf(draft.flow.host);
  return apex.isEmpty ? '' : '**.$apex';
}

/// The path a `url` rule matches on: everything before the query.
///
/// A rule that carried the query would match one request and never the next,
/// because agents number their requests.
String pathWithoutQuery(String path) {
  final int query = path.indexOf('?');
  return query < 0 ? path : path.substring(0, query);
}

RuleMatcher _matcherOf(
  RuleDraft draft, {
  required String Function(String host) apexOf,
}) {
  final Flow flow = draft.flow;
  final String host = hostPattern(draft, apexOf);
  return switch (draft.target) {
    RememberTarget.url => RuleMatcher(
      host: host,
      methods: <Method>[flow.method],
      path: pathWithoutQuery(flow.path),
      scheme: flow.scheme,
      port: flow.authority.port,
    ),
    RememberTarget.host => RuleMatcher(host: host),
    RememberTarget.apex => RuleMatcher(host: host),
    RememberTarget.hostMethod => RuleMatcher(
      host: host,
      methods: <Method>[flow.method],
    ),
  };
}

/// Der Entwurf als Satz, in der Sprache des Nutzers.
///
/// Der Satz kommt aus demselben Generator, den der Regel-Bildschirm liest
/// (`core/text/rule_sentence.dart`): erst wird die Regel gebaut, dann wird sie
/// vorgelesen. Zwei Generatoren waren hier einmal zwei Wortlaute für dieselbe
/// Regel, einer vor dem Anlegen und einer danach; das ist genau der Fall, vor
/// dem `backlog/CONVENTIONS.md` 4.13 warnt.
///
/// Leer, solange der Entwurf nichts merkt oder die registrierbare Domäne
/// fehlt: dann entsteht keine Regel, und es gibt nichts vorzulesen.
///
/// [now] ist die Uhr, gegen die die Stundenregel gerechnet wird; ohne Angabe
/// die aktuelle. Beides -- die Regel und ihr Satz -- liest dieselbe Uhr, sonst
/// stünde im Satz eine andere Frist als in der Regel.
String ruleSentence(
  RuleDraft draft,
  AppLocalizations l10n, {
  required String Function(String host) apexOf,
  DateTime? now,
}) {
  final DateTime clock = now ?? DateTime.now();
  final Rule? rule = buildRule(draft, now: clock, apexOf: apexOf);
  return rule == null ? '' : sentence.ruleSentence(rule, l10n, now: clock);
}

/// The label of [duration] on its segment.
String durationLabel(RememberDuration duration, AppLocalizations l10n) =>
    switch (duration) {
      RememberDuration.once => l10n.interceptDurationOnce,
      RememberDuration.session => l10n.interceptDurationSession,
      RememberDuration.oneHour => l10n.interceptDurationHour,
      RememberDuration.forever => l10n.interceptDurationForever,
    };

/// The label of [target] on its segment.
String targetLabel(RememberTarget target, AppLocalizations l10n) =>
    switch (target) {
      RememberTarget.url => l10n.interceptTargetUrl,
      RememberTarget.host => l10n.interceptTargetHost,
      RememberTarget.apex => l10n.interceptTargetApex,
      RememberTarget.hostMethod => l10n.interceptTargetHostMethod,
    };

/// The label the release valve carries for [duration]: it says what pressing
/// it does, not what it is called.
String allowLabel(RememberDuration duration, AppLocalizations l10n) =>
    switch (duration) {
      RememberDuration.once => l10n.interceptAllowButton,
      RememberDuration.session => l10n.interceptAllowForSession,
      RememberDuration.oneHour => l10n.interceptAllowForHour,
      RememberDuration.forever => l10n.interceptAllowAlways,
    };
