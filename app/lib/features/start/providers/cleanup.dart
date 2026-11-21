/// The cleanup tab of the start window (HUM-537, ADR-0024 point 14).
///
/// The person chooses whose data goes; the daemon answers with the plan, item
/// by item with its size, what stays and the fixed line of what is never
/// deleted (`Projects.cleanup_plan`). Only a confirmation of exactly that plan
/// deletes: `Projects.cleanup` gets back the selection and the digest the plan
/// came with, and the daemon checks the digest again in the deleting
/// transaction. The app keeps no list of its own of what could go
/// (sprint 8 boundaries 2 and 3).
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/daemon_client.dart';
import 'projects.dart';

/// What the cleanup tab shows.
final class CleanupState {
  /// Creates the state.
  const CleanupState({
    this.projectIds = const <String>{},
    this.unassigned = false,
    this.includeWaitingChanges = false,
    this.busy = false,
    this.plan,
    this.result,
    this.diagnostic,
  });

  /// The projects chosen, by id.
  final Set<String> projectIds;

  /// Whether the sessions without a project are chosen.
  final bool unassigned;

  /// Whether the waiting changes of disposable runs may be in the plan.
  final bool includeWaitingChanges;

  /// True while a plan or a cleanup is under way.
  final bool busy;

  /// The plan waiting for confirmation; the confirmation shows while set.
  final CleanupPlan? plan;

  /// What the last confirmed cleanup did.
  final CleanupResult? result;

  /// What the daemon said against the last plan or cleanup.
  final Diagnostic? diagnostic;

  /// The selection as `cleanup_plan` takes it.
  CleanupSelection get selection => CleanupSelection(
    projectIds: projectIds.toList(growable: false),
    unassigned: unassigned && projectIds.isEmpty,
    includeWaitingChanges: includeWaitingChanges,
  );

  /// Whether anything is chosen.
  bool get hasSelection => projectIds.isNotEmpty || unassigned;

  CleanupState _copy({
    Set<String>? projectIds,
    bool? unassigned,
    bool? includeWaitingChanges,
    bool? busy,
    CleanupPlan? Function()? plan,
    CleanupResult? Function()? result,
    Diagnostic? Function()? diagnostic,
  }) => CleanupState(
    projectIds: projectIds ?? this.projectIds,
    unassigned: unassigned ?? this.unassigned,
    includeWaitingChanges: includeWaitingChanges ?? this.includeWaitingChanges,
    busy: busy ?? this.busy,
    plan: plan == null ? this.plan : plan(),
    result: result == null ? this.result : result(),
    diagnostic: diagnostic == null ? this.diagnostic : diagnostic(),
  );
}

/// The cleanup tab.
final NotifierProvider<CleanupNotifier, CleanupState> cleanupProvider =
    NotifierProvider.autoDispose<CleanupNotifier, CleanupState>(
      CleanupNotifier.new,
    );

/// The notifier behind [cleanupProvider].
class CleanupNotifier extends Notifier<CleanupState> {
  @override
  CleanupState build() => const CleanupState();

  /// Chooses or drops the project [projectId]. A selection is either
  /// projects or the sessions without one, as the daemon takes exactly one
  /// kind of target; choosing a project drops the other.
  void toggleProject(String projectId) {
    final Set<String> ids = <String>{...state.projectIds};
    if (!ids.remove(projectId)) {
      ids.add(projectId);
    }
    state = state._copy(
      projectIds: ids,
      unassigned: ids.isEmpty ? state.unassigned : false,
      result: () => null,
    );
  }

  /// Chooses or drops the sessions without a project; drops chosen
  /// projects, for the same reason as [toggleProject].
  void toggleUnassigned() {
    final bool on = !state.unassigned;
    state = state._copy(
      unassigned: on,
      projectIds: on ? const <String>{} : state.projectIds,
      result: () => null,
    );
  }

  /// Lets the plan list the waiting changes of disposable runs, or not.
  void toggleWaitingChanges() => state = state._copy(
    includeWaitingChanges: !state.includeWaitingChanges,
    result: () => null,
  );

  /// Asks the daemon for the plan of the selection. Deletes nothing; the
  /// confirmation shows the answer.
  Future<void> review() async {
    if (!state.hasSelection || state.busy) {
      return;
    }
    state = state._copy(busy: true, diagnostic: () => null, result: () => null);
    final DaemonClient client = ref.read(daemonClientProvider);
    try {
      final CleanupPlan plan = await client.projectsCleanupPlan(
        state.selection,
      );
      if (!ref.mounted) {
        return;
      }
      state = state._copy(busy: false, plan: () => plan);
    } on DaemonException catch (error) {
      if (!ref.mounted) {
        return;
      }
      state = state._copy(busy: false, diagnostic: () => error.diagnostic);
    }
  }

  /// Closes the confirmation; nothing is deleted and nothing is sent.
  /// While a confirmed cleanup is under way it does nothing: the question
  /// stays until the daemon answered, so the result is never hidden behind
  /// a closed dialog.
  void cancel() {
    if (state.busy) {
      return;
    }
    state = state._copy(plan: () => null);
  }

  /// Deletes exactly the plan on screen: its selection and its digest go
  /// back to the daemon, which deletes only when the plan is still the same.
  /// The list of projects is read again afterwards, whatever the answer.
  Future<void> confirm() async {
    final CleanupPlan? plan = state.plan;
    if (plan == null || plan.items.isEmpty || state.busy) {
      return;
    }
    state = state._copy(busy: true);
    final DaemonClient client = ref.read(daemonClientProvider);
    try {
      final CleanupResult result = await client.projectsCleanup(
        plan.selection,
        plan.digest,
      );
      if (!ref.mounted) {
        return;
      }
      state = CleanupState(
        includeWaitingChanges: state.includeWaitingChanges,
        result: result,
      );
    } on DaemonException catch (error) {
      if (!ref.mounted) {
        return;
      }
      state = state._copy(
        busy: false,
        plan: () => null,
        diagnostic: () => error.diagnostic,
      );
    }
    await ref.read(projectsProvider.notifier).reload();
  }

  /// Forgets the finding of the last plan or cleanup.
  void dismiss() => state = state._copy(diagnostic: () => null);
}
