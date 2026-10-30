/// Which failures of the last operation the person has closed on the Sandbox
/// screen (HUM-302, review).
///
/// The screen shows two kinds of findings, and closing means something
/// different for each:
///
/// * **A finding that stands** -- `LLM_006` for a public endpoint, a warning
///   about the profile -- comes back with every status for as long as its
///   cause stays. Closing it is remembered across statuses and restarts
///   (`dismissedFindingsProvider` in `core/ui_state`).
/// * **A failure of an operation** -- a refused start (`SANDBOX_006`, any
///   finding of severity error or blocking), a folder dialog that did not
///   open (`UI_003`) -- is an event. Closing it hides this one card, in
///   memory only, and a new operation brings the next failure back even when
///   it reads the same: a repeated start that fails again must say so.
///
/// The memory of the failures of the daemon ends when a new operation is
/// asked for (plan, start, stop, folder choice) and survives refreshes of
/// the status. A folder dialog failure counts as closed only for the very
/// object that was closed; the next failed dialog is a new object.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui_state/dismissed_findings.dart';

/// Whether [diagnostic] is a failure of an operation rather than a finding
/// that stands.
bool isOperationFailure(Diagnostic diagnostic) => diagnostic.isFailure;

/// The closed failures.
typedef ClosedFailuresState = ({Set<String> failures, Diagnostic? dialog});

/// The store of the closed failures.
final NotifierProvider<ClosedFailures, ClosedFailuresState>
closedFailuresProvider = NotifierProvider<ClosedFailures, ClosedFailuresState>(
  ClosedFailures.new,
);

/// The notifier behind [closedFailuresProvider].
class ClosedFailures extends Notifier<ClosedFailuresState> {
  @override
  ClosedFailuresState build() => (failures: const <String>{}, dialog: null);

  /// Forgets the closed failures of the daemon: a new operation (plan,
  /// start, stop, a folder choice) was just asked for, and whatever it
  /// reports is news, even when it reads like the last one. Called by the
  /// sandbox notifier when it sends such a call, not when the state changes:
  /// a start refused before it ever reached `starting` must show again too
  /// (HUM-302, review). A refresh of the status is no operation and keeps
  /// them.
  void newOperation() {
    if (state.failures.isNotEmpty) {
      state = (failures: const <String>{}, dialog: state.dialog);
    }
  }

  /// True when [diagnostic], a failure of the daemon, was closed during the
  /// current operation.
  bool coversFailure(Diagnostic diagnostic) =>
      state.failures.contains(findingFingerprint(diagnostic));

  /// True when [failure] is the very folder dialog failure that was closed.
  bool coversDialog(Diagnostic failure) => identical(state.dialog, failure);

  /// Closes [diagnostic], a failure of the daemon, for this operation.
  void closeFailure(Diagnostic diagnostic) => state = (
    failures: <String>{...state.failures, findingFingerprint(diagnostic)},
    dialog: state.dialog,
  );

  /// Closes the folder dialog failure [failure].
  void closeDialog(Diagnostic failure) =>
      state = (failures: state.failures, dialog: failure);
}
