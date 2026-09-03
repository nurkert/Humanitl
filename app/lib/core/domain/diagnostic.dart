/// A finding with cause and proposal, mirror of `humanitl_core::Diagnostic`.
///
/// Every error a person could see is one of these. The code is registered in
/// `daemon/crates/core-types/src/diagnostics/codes.rs`; the client never
/// invents a code that is not in that register.
library;

import 'package:freezed_annotation/freezed_annotation.dart';

import 'rule.dart';

part 'diagnostic.freezed.dart';
part 'diagnostic.g.dart';

/// How urgent a diagnostic is.
enum Severity {
  /// Worth knowing.
  info,

  /// Something is off, the product still works.
  warning,

  /// Something failed.
  error,

  /// Nothing works until this is fixed.
  blocking,
}

/// What the person can do about the cause.
@freezed
sealed class FixAction with _$FixAction {
  /// Set an environment variable.
  const factory FixAction.setEnv({required String key, required String value}) =
      FixActionSetEnv;

  /// Add the proposed rule.
  const factory FixAction.addRule({required Rule rule}) = FixActionAddRule;

  /// Install the user service.
  const factory FixAction.installService() = FixActionInstallService;

  /// Change a configuration key.
  const factory FixAction.changeSetting({
    required String key,
    required String value,
  }) = FixActionChangeSetting;

  /// Copy a shell command to the clipboard.
  const factory FixAction.copyCommand({required String command}) =
      FixActionCopyCommand;

  /// Open a URL.
  const factory FixAction.openUrl({required String url}) = FixActionOpenUrl;

  /// Remount a path read-only.
  const factory FixAction.remountReadOnly({required String path}) =
      FixActionRemountReadOnly;

  /// Reads a fix action from JSON.
  factory FixAction.fromJson(Map<String, Object?> json) =>
      _$FixActionFromJson(json);
}

/// A diagnostic: code, severity, fixed title, variable cause, optional fix
/// and documentation link.
@freezed
abstract class Diagnostic with _$Diagnostic {
  /// Creates a diagnostic.
  const factory Diagnostic({
    required String code,
    required Severity severity,
    @Default('') String title,
    @Default('') String why,
    FixAction? fix,
    String? docsUrl,
  }) = _Diagnostic;

  const Diagnostic._();

  /// Reads a diagnostic from JSON.
  factory Diagnostic.fromJson(Map<String, Object?> json) =>
      _$DiagnosticFromJson(json);

  /// The area of the code, lowercase: `daemon` for `DAEMON_001`.
  String get area {
    final int underscore = code.lastIndexOf('_');
    return (underscore <= 0 ? code : code.substring(0, underscore))
        .toLowerCase();
  }

  /// True for [Severity.error] and [Severity.blocking].
  bool get isFailure =>
      severity == Severity.error || severity == Severity.blocking;
}
