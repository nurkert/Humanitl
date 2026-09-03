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
