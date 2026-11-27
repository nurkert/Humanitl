/// The projects of the start window and the starts made from it (HUM-537).
///
/// Everything here is a call to the daemon and its answer (ADR-018): the list
/// is `Projects(list)` in the daemon's order, a start is `Sandbox(Start)` with
/// the project's id in the register (a new folder gets its entry from the
/// first-start screen first, `first_start.dart`, HUM-538), and whether it may
/// start the daemon decides
/// (`CLI_005` while a session runs, `SANDBOX_053` without a folder). The
/// window only switches to the session view once the daemon reports the
/// session running.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/agents_provider.dart';
import '../../../core/ipc/client_diagnostics.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/connection.dart';
import '../../../core/ipc/daemon_client.dart';
import '../../../core/ui/session_view.dart';
import '../setup/setup_steps.dart';
import 'agent_setup.dart';

/// `Projects(list)`, newest start first, as the daemon sorted it.
///
/// Disposed with the start window: coming back from a session view reads
/// the list again, with the session that runs now.
final AsyncNotifierProvider<ProjectsNotifier, List<ProjectEntry>>
projectsProvider =
    AsyncNotifierProvider.autoDispose<ProjectsNotifier, List<ProjectEntry>>(
      ProjectsNotifier.new,
    );

/// The notifier behind [projectsProvider].
class ProjectsNotifier extends AsyncNotifier<List<ProjectEntry>> {
  @override
  Future<List<ProjectEntry>> build() =>
      ref.watch(daemonClientProvider).listProjects();

  /// Reads the list again: after a start, after a cleanup (a deleted
  /// project must not stay in an open list) and on "Try again".
  Future<void> reload() async {
    state = await AsyncValue.guard(
      () => ref.read(daemonClientProvider).listProjects(),
    );
  }
}

/// What a start from the start window is doing.
final class StartLaunch {
  /// Creates the state.
  const StartLaunch({
    this.busyPath,
    this.diagnostics = const <Diagnostic>[],
    this.endedSilently = false,
  });

  /// The folder a start is under way for, or null.
  final String? busyPath;

  /// What the daemon said against the last start.
  final List<Diagnostic> diagnostics;

  /// True when the last start ended without a running session and without
  /// a finding of the daemon; the window says so instead of nothing.
  final bool endedSilently;
}

/// The starts of the start window.
final NotifierProvider<StartLaunchNotifier, StartLaunch> startLaunchProvider =
    NotifierProvider.autoDispose<StartLaunchNotifier, StartLaunch>(
      StartLaunchNotifier.new,
    );

/// The notifier behind [startLaunchProvider].
class StartLaunchNotifier extends Notifier<StartLaunch> {
  @override
  StartLaunch build() => const StartLaunch();

  /// Opens the session view of the running session in [entry].
  void attach(ProjectEntry entry) {
    state = const StartLaunch();
    ref
        .read(sessionViewProvider.notifier)
        .attach(
          SessionView(projectPath: entry.path, projectId: entry.projectId),
        );
  }

  /// Starts a session in the recent project [entry], with its project
  /// choice: the daemon loads it for the folder of the start.
  ///
  /// The project goes out by its id (`Start.project_id`, contract 1.58):
  /// [ProjectEntry.path] is cleaned for display, and a folder with two
  /// spaces or a control character in its name would come back as another
  /// folder. A daemon before 1.58 ignores the id; against it nothing
  /// starts, and the window says how the daemon restarts. While `GetInfo`
  /// has not answered yet, the start waits for it rather than guess.
  ///
  /// A depot agent that is not installed or not signed in opens the guided
  /// setup instead (GitHub #111), as the depot state the window already has
  /// says; nothing starts then.
  Future<void> startRecent(ProjectEntry entry) async {
    if (!entry.startable) {
      return;
    }
    final AgentDepot? depot = ref
        .read(agentsProvider)
        .value
        ?.of(entry.lastAgent);
    if (depot != null && depotNeedsSetup(depot)) {
      state = const StartLaunch();
      await ref
          .read(agentSetupProvider.notifier)
          .open(AgentSetupTarget.of(entry));
      return;
    }
    return startKnown(entry.projectId, entry.path);
  }

  /// Starts the project [projectId] of the register, whose folder the list
  /// shows as [path], with its project choice; the daemon takes the folder
  /// from the register row (contract 1.58).
  Future<void> startKnown(String projectId, String path) async {
    if (state.busyPath != null) {
      return;
    }
    DaemonInfo? info = ref.read(daemonInfoProvider).value;
    if (info == null) {
      state = StartLaunch(busyPath: path);
      try {
        info = await ref.read(daemonInfoProvider.future);
      } on Object {
        info = null;
      }
      if (!ref.mounted) {
        return;
      }
      state = const StartLaunch();
    }
    if (!(info?.startsByProjectId ?? false)) {
      state = StartLaunch(
        diagnostics: <Diagnostic>[
          ClientDiagnostics.projectStartDaemonTooOld(info?.protoVersion ?? '?'),
        ],
      );
      return;
    }
    return _start(path, projectId: projectId);
  }

  /// Starts and switches to the session view as soon as the daemon reports
  /// the session running.
  ///
  /// The daemon keeps the stream of a start open for the whole session
  /// (`accompany` in `daemon/crates/ipc/src/sandbox.rs`); waiting for its
  /// end would leave the window on "Starting…" until the agent exits. The
  /// first `running` without a refusing finding before it is the answer,
  /// and leaving the loop cancels the subscription. The session does not
  /// depend on this stream.
  Future<void> _start(String path, {required String projectId}) async {
    if (state.busyPath != null) {
      return;
    }
    state = StartLaunch(busyPath: path);
    final DaemonClient client = ref.read(daemonClientProvider);
    final List<Diagnostic> said = <Diagnostic>[];
    bool running = false;
    bool refused() => said.any(
      (Diagnostic d) =>
          d.severity == Severity.error || d.severity == Severity.blocking,
    );
    try {
      await for (final SandboxUpdate update in client.startSandbox(
        projectId: projectId,
      )) {
        switch (update) {
          case SandboxUpdateDiagnostic(:final Diagnostic diagnostic):
            said.add(diagnostic);
          // Ein Befund, der den Start aufhält, hält auch den Wechsel auf:
          // Bei `CLI_005` meldet der Daemon nach dem Befund den Zustand der
          // Sitzung, die schon läuft, und die ist nicht die dieses Projekts.
          case SandboxUpdateStatus(:final SandboxStatus status)
              when status.state == SandboxState.running && !refused():
            running = true;
          default:
            break;
        }
        if (running) {
          break;
        }
      }
    } on DaemonException catch (error) {
      said.add(error.diagnostic);
    }
    if (!ref.mounted) {
      return;
    }
    if (running) {
      state = const StartLaunch();
      ref
          .read(sessionViewProvider.notifier)
          .attach(SessionView(projectPath: path, projectId: projectId));
      return;
    }
    // Ein Befund, den ein Schritt der Einrichtung löst, führt in diesen
    // Schritt statt als Text unter die Liste (GitHub #111).
    if (leadsIntoSetup(said)) {
      state = const StartLaunch();
      await ref
          .read(agentSetupProvider.notifier)
          .open(
            AgentSetupTarget(
              path: path,
              projectId: projectId,
              agent: _lastAgent(projectId),
              displayName: _displayName(projectId),
              findings: List<Diagnostic>.unmodifiable(said),
            ),
          );
      return;
    }
    state = StartLaunch(
      diagnostics: List<Diagnostic>.unmodifiable(said),
      endedSilently: said.isEmpty,
    );
  }

  ProjectEntry? _entry(String projectId) {
    for (final ProjectEntry entry
        in ref.read(projectsProvider).value ?? const <ProjectEntry>[]) {
      if (entry.projectId == projectId) {
        return entry;
      }
    }
    return null;
  }

  String _lastAgent(String projectId) => _entry(projectId)?.lastAgent ?? '';

  String _displayName(String projectId) => _entry(projectId)?.displayName ?? '';

  /// Forgets the findings of the last start.
  void dismiss() => state = const StartLaunch();
}
