/// The project register of the daemon as the start window draws it
/// (HUM-532, HUM-533, HUM-537).
///
/// Plain value types, mirror of `ProjectEntry`, `CleanupSelection`,
/// `CleanupPlanView` and `CleanupResult`. The daemon decides which projects
/// it knows, what a cleanup would delete and what stays (ADR-018, sprint 8
/// boundary 3); the app only draws the answers and sends back what the person
/// confirmed.
library;

/// One project of the register.
final class ProjectEntry {
  /// Creates an entry.
  const ProjectEntry({
    required this.projectId,
    required this.path,
    required this.displayName,
    this.lastStarted,
    this.lastAgent = '',
    this.runs = 0,
    this.running = false,
    this.sessionId = '',
    this.changesWaiting = false,
    this.missing = false,
    this.replaced = false,
    this.hasChoice = false,
    this.terminalClients = 0,
  });

  /// The stable id of the project.
  final String projectId;

  /// The canonical folder on the host, cleaned for display.
  final String path;

  /// The folder name, cleaned for display.
  final String displayName;

  /// When a session last started in it; null without a start.
  final DateTime? lastStarted;

  /// The agent adapter of the last start, empty when unknown.
  final String lastAgent;

  /// How many sessions ran in it.
  final int runs;

  /// True while the session of this daemon runs in the project.
  final bool running;

  /// The id of that session, empty without one.
  final String sessionId;

  /// True when changes of a disposable run wait for a decision.
  final bool changesWaiting;

  /// True when the folder is gone.
  final bool missing;

  /// True for a row another folder at the same path has replaced.
  final bool replaced;

  /// True when the register holds a project choice for it.
  final bool hasChoice;

  /// How many clients hang on the terminal of its running session, writing
  /// and reading (#13): 0 with [running] means the session runs without an
  /// attached terminal, for example after the terminal of `humanitl go`
  /// closed. Read only from a daemon that counts them
  /// (`DaemonInfo.countsTerminalClients`); an older one always says 0.
  final int terminalClients;

  /// Whether a start from the list of recent projects is possible: the
  /// folder is there and the row is the current one at its path.
  bool get startable => !missing && !replaced;

  @override
  bool operator ==(Object other) =>
      other is ProjectEntry &&
      other.projectId == projectId &&
      other.path == path &&
      other.displayName == displayName &&
      other.lastStarted == lastStarted &&
      other.lastAgent == lastAgent &&
      other.runs == runs &&
      other.running == running &&
      other.sessionId == sessionId &&
      other.changesWaiting == changesWaiting &&
      other.missing == missing &&
      other.replaced == replaced &&
      other.hasChoice == hasChoice &&
      other.terminalClients == terminalClients;

  @override
  int get hashCode => Object.hash(
    projectId,
    path,
    displayName,
    lastStarted,
    lastAgent,
    runs,
    running,
    sessionId,
    changesWaiting,
    missing,
    replaced,
    hasChoice,
    terminalClients,
  );
}

/// Whose data a cleanup plan covers (`CleanupSelection`).
///
/// Exactly one of [projectIds], [all] and [unassigned] says whose; the
/// daemon refuses a selection that names none. [includeWaitingChanges] is
/// the explicit choice that lets the plan list the waiting changes of a
/// disposable run.
final class CleanupSelection {
  /// Creates a selection.
  const CleanupSelection({
    this.projectIds = const <String>[],
    this.all = false,
    this.unassigned = false,
    this.includeWaitingChanges = false,
  });

  /// Projects by their id.
  final List<String> projectIds;

  /// Every project of the register.
  final bool all;

  /// Only the sessions recorded without a project.
  final bool unassigned;

  /// Whether the waiting changes of a disposable run belong to the plan.
  final bool includeWaitingChanges;

  /// True when the selection names nobody.
  bool get isEmpty => projectIds.isEmpty && !all && !unassigned;

  @override
  bool operator ==(Object other) =>
      other is CleanupSelection &&
      _sameList(other.projectIds, projectIds) &&
      other.all == all &&
      other.unassigned == unassigned &&
      other.includeWaitingChanges == includeWaitingChanges;

  @override
  int get hashCode => Object.hash(
    Object.hashAll(projectIds),
    all,
    unassigned,
    includeWaitingChanges,
  );
}

/// One item of a cleanup plan (`CleanupItem`).
final class CleanupItem {
  /// Creates an item.
  const CleanupItem({
    required this.kind,
    required this.description,
    this.projectId = '',
    this.path = '',
    this.bytes = 0,
    this.count = 0,
    this.warning = '',
  });

  /// `recorded_flows`, `waiting_changes`, `project_choice` or
  /// `register_entry`.
  final String kind;

  /// The project, empty for the sessions without one.
  final String projectId;

  /// The folder of the project, cleaned for display.
  final String path;

  /// The daemon's sentence of what goes.
  final String description;

  /// About this many bytes become free.
  final int bytes;

  /// How many things go.
  final int count;

  /// The warning every client shows before the question, empty without.
  final String warning;
}

/// A cleanup plan (`CleanupPlanView`): what the daemon would delete, what
/// stays, and the digest that binds a later `cleanup` to exactly this plan.
final class CleanupPlan {
  /// Creates a plan.
  const CleanupPlan({
    required this.items,
    required this.digest,
    required this.notDeleted,
    required this.selection,
    this.kept = const <String>[],
  });

  /// Every item that would go.
  final List<CleanupItem> items;

  /// Binds identity and version of every item; `cleanup` sends it back.
  final String digest;

  /// The fixed line of what is never deleted; every client shows it.
  final String notDeleted;

  /// The selection in the form `cleanup` takes back.
  final CleanupSelection selection;

  /// What the selection would hit but stays, one sentence each.
  final List<String> kept;

  /// The sum of [CleanupItem.bytes].
  int get totalBytes =>
      items.fold<int>(0, (int sum, CleanupItem item) => sum + item.bytes);
}

/// What `cleanup` did (`CleanupResult`).
final class CleanupResult {
  /// Creates a result.
  const CleanupResult({
    required this.deleted,
    required this.failed,
    required this.bytesFreed,
    this.reasons = const <String>[],
  });

  /// How many items went.
  final int deleted;

  /// How many items did not go, or not completely.
  final int failed;

  /// The bytes that became free.
  final int bytesFreed;

  /// Why an item did not go, one sentence each.
  final List<String> reasons;
}

bool _sameList(List<String> a, List<String> b) {
  if (a.length != b.length) {
    return false;
  }
  for (int i = 0; i < a.length; i++) {
    if (a[i] != b[i]) {
      return false;
    }
  }
  return true;
}
