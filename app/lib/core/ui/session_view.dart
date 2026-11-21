/// Which project the window shows a session of, or none (HUM-537,
/// ADR-0024 point 5).
///
/// The app opens on the start window and shows the view of a session only
/// once somebody attached to one there, started a recent project or chose a
/// new one. Null is the start window. The project goes with every start the
/// session view makes (`Sandbox(Start)` with `work_dir`), so the global
/// `sandbox.work_dir` is never the silent "current project" of the app
/// (sprint 8, HUM-537 point 2).
///
/// It lives in `core` because three features read it and no feature imports
/// another one (ARCHITECTURE 5): the shell switches on it, the start window
/// sets it, and the sandbox screen starts in its project.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

/// The project of the session view.
final class SessionView {
  /// Creates the view of a session in [projectPath].
  const SessionView({required this.projectPath, this.projectId = ''});

  /// The project folder on the host, as the daemon named it.
  final String projectPath;

  /// The id of the project in the register, empty when not known yet (a new
  /// folder the daemon registers with its first start).
  final String projectId;

  @override
  bool operator ==(Object other) =>
      other is SessionView &&
      other.projectPath == projectPath &&
      other.projectId == projectId;

  @override
  int get hashCode => Object.hash(projectPath, projectId);
}

/// The session view the window shows, or null for the start window.
final NotifierProvider<SessionViewNotifier, SessionView?> sessionViewProvider =
    NotifierProvider<SessionViewNotifier, SessionView?>(
      SessionViewNotifier.new,
    );

/// The notifier behind [sessionViewProvider].
class SessionViewNotifier extends Notifier<SessionView?> {
  /// Creates the notifier. [initial] is for tests that mount the app as it
  /// stands inside a session; the app itself always starts without one.
  SessionViewNotifier([this._initial]);

  final SessionView? _initial;

  @override
  SessionView? build() => _initial;

  /// Shows the session view of [view]'s project.
  void attach(SessionView view) => state = view;

  /// Goes back to the start window. The session keeps running and stands
  /// under "Running" there.
  void detach() => state = null;

  /// Keeps the view and makes [projectPath] the project of its next start:
  /// the folder somebody chose with the folder control inside the session
  /// view, exactly as the dialog returned it, so without a project id. Not
  /// a write of `sandbox.work_dir` from elsewhere (a `humanitl config set`
  /// would otherwise move an attached view silently). Does nothing on the
  /// start window.
  void retarget(String projectPath) {
    final SessionView? now = state;
    if (now == null || now.projectPath == projectPath) {
      return;
    }
    state = SessionView(projectPath: projectPath);
  }
}
