/// Rules as value types, mirror of `humanitl_core::rule` and `rules.proto`.
library;

import 'package:freezed_annotation/freezed_annotation.dart';

import 'http.dart';
import 'ids.dart';

part 'rule.freezed.dart';
part 'rule.g.dart';

/// What happens to a matching request.
enum RuleAction {
  /// Let it through.
  allow,

  /// Refuse it.
  block,

  /// Hold it for the human, the default.
  ask,

  /// Let it through after redacting findings.
  redact,
}

/// Condition of a rule. Empty fields mean "any".
@freezed
abstract class RuleMatcher with _$RuleMatcher {
  /// Creates a matcher.
  const factory RuleMatcher({
    @Default('') String host,
    @Default(<Method>[]) List<Method> methods,
    @Default('') String path,
    Scheme? scheme,
    @Default(0) int port,
    Upgrade? upgrade,
  }) = _RuleMatcher;

  /// Reads a matcher from JSON.
  factory RuleMatcher.fromJson(Map<String, Object?> json) =>
      _$RuleMatcherFromJson(json);
}

/// How long a rule lives.
@freezed
sealed class RuleExpiry with _$RuleExpiry {
  /// Forever; written to `rules.yaml`.
  const factory RuleExpiry.never() = RuleExpiryNever;

  /// Only for the running session; kept in memory.
  const factory RuleExpiry.session() = RuleExpirySession;

  /// Until a point in time.
  const factory RuleExpiry.at({required DateTime at}) = RuleExpiryAt;

  /// Reads an expiry from JSON.
  factory RuleExpiry.fromJson(Map<String, Object?> json) =>
      _$RuleExpiryFromJson(json);
}

/// One rule of the ordered list. First match wins; session rules are
/// evaluated before persistent ones (CONVENTIONS 4.5).
@freezed
abstract class Rule with _$Rule {
  /// Creates a rule. [id] is null while the daemon has not assigned one.
  const factory Rule({
    @RuleIdConverter() RuleId? id,
    required RuleAction action,
    required RuleMatcher matcher,
    @Default(RuleExpiry.session()) RuleExpiry expires,
    @Default(false) bool stream,
    @FlowIdConverter() FlowId? createdFrom,
    @Default(false) bool bundled,
    String? note,
    DateTime? createdAt,
    @Default(0) int position,
    @Default(0) int hitCount,
    @Default(false) bool allowPrivate,
  }) = _Rule;

  /// Reads a rule from JSON.
  factory Rule.fromJson(Map<String, Object?> json) => _$RuleFromJson(json);
}

/// True when [rule] has run out at [now].
///
/// A rule with an end time in the past is skipped by the engine
/// (`RuleSet::evaluate`), so it decides nothing: it matches no request, and a
/// screen that drew it as if it did would claim more than is true
/// (`backlog/CONVENTIONS.md` 4.13). A session rule of the running session
/// never runs out.
bool ruleExpiredAt(Rule rule, DateTime now) => switch (rule.expires) {
  RuleExpiryAt(:final DateTime at) => !at.isAfter(now),
  RuleExpiryNever() || RuleExpirySession() => false,
};

/// Why a host pattern cannot be read.
///
/// The names mirror the reasons `HostPattern::parse` gives in
/// `daemon/crates/core-types/src/rule.rs`; the daemon keeps the last word,
/// and this only lets a form say what is wrong before the round trip.
enum HostPatternProblem {
  /// No pattern at all. A rule always names a host: an empty field would be
  /// a rule over every host, and that is written `**`.
  empty,

  /// A wildcard shares its label with text, as in `*api.example.com`.
  wildcardInLabel,

  /// Two dots in a row, or a leading or trailing dot.
  emptyLabel,

  /// `ip:` or `cidr:` without a readable address behind it.
  notAnAddress,

  /// A fixed label that is not a label: a space, a slash, a colon.
  notALabel,
}

/// What is wrong with [pattern], or null when the daemon has to decide.
///
/// Deliberately not a full parser: the pre-check exists so that a form can
/// answer while somebody types, and every pattern it lets through is still
/// checked by the engine (`backlog/sprint-2.md`, HUM-033).
HostPatternProblem? hostPatternProblem(String pattern) {
  if (pattern.isEmpty) {
    return HostPatternProblem.empty;
  }
  for (final String prefix in const <String>['ip:', 'cidr:']) {
    if (pattern.startsWith(prefix)) {
      final String rest = pattern.substring(prefix.length);
      final String address = prefix == 'cidr:' ? rest.split('/').first : rest;
      final bool complete = prefix == 'ip:' || rest.contains('/');
      return complete && address.isNotEmpty && _looksLikeAddress(address)
          ? null
          : HostPatternProblem.notAnAddress;
    }
  }
  for (final String label in pattern.split('.')) {
    if (label.isEmpty) {
      return HostPatternProblem.emptyLabel;
    }
    if (label == '*' || label == '**') {
      continue;
    }
    if (label.contains('*')) {
      return HostPatternProblem.wildcardInLabel;
    }
    if (!_labelPattern.hasMatch(label)) {
      return HostPatternProblem.notALabel;
    }
  }
  return null;
}

/// Why a path pattern cannot be read.
enum PathPatternProblem {
  /// A pattern that starts with `~` is a regular expression, and this one is
  /// not one the engine could build.
  invalidRegex,
}

/// What is wrong with [pattern], or null. An empty path matches every path
/// and is not a problem.
PathPatternProblem? pathPatternProblem(String pattern) {
  if (!pattern.startsWith('~')) {
    return null;
  }
  try {
    RegExp(pattern.substring(1));
  } on FormatException {
    return PathPatternProblem.invalidRegex;
  }
  return null;
}

/// Rough shape of an IPv4 or IPv6 literal. `Uri.parseIPv6Address` throws on
/// anything else, and an exception is the cheapest complete check there is.
bool _looksLikeAddress(String text) {
  try {
    if (text.contains(':')) {
      Uri.parseIPv6Address(text);
    } else {
      Uri.parseIPv4Address(text);
    }
  } on FormatException {
    return false;
  }
  return true;
}

/// A label after normalisation: letters, digits and inner hyphens.
final RegExp _labelPattern = RegExp(r'^[^\s./:*?#]+$');
