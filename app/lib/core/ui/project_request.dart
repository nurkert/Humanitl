/// The request to show the start window with a folder preselected
/// (HUM-539, ADR-0024 point 5).
///
/// Two sources, one meaning: `--project PATH` on the command line of this
/// run (`humanitl app PATH` started the app), and `FlowEvent.open_in_app`
/// (`humanitl app PATH` found this app connected to the daemon). A request
/// that arrives while the window shows a session goes back to the start
/// window; the session keeps running and stands under "Running" there.
///
/// It lives in `core` because two features read it and no feature imports
/// another one (ARCHITECTURE 5): the start window preselects the folder, the
/// shell brings the window forward. Neither starts anything: a request is a
/// preselection, and a session starts only after "Start" in the window.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../domain/domain.dart';
import '../ipc/client_providers.dart';
import '../ipc/flow_events.dart';
import 'session_view.dart';

/// One request of `humanitl app`.
final class ProjectRequest {
  /// Creates the request for [path]; empty means no preselection.
  const ProjectRequest({required this.path, required this.serial});

  /// The folder to preselect, absolute as the command line canonicalized
  /// it; empty for the start window without a preselection.
  final String path;

  /// Counts the requests of this run, so that the same folder asked twice
  /// is two requests.
  final int serial;

  @override
  bool operator ==(Object other) =>
      other is ProjectRequest && other.path == path && other.serial == serial;

  @override
  int get hashCode => Object.hash(path, serial);
}

/// The request not yet taken by the start window, or null.
final NotifierProvider<ProjectRequestNotifier, ProjectRequest?>
projectRequestProvider =
    NotifierProvider<ProjectRequestNotifier, ProjectRequest?>(
      ProjectRequestNotifier.new,
    );

/// The notifier behind [projectRequestProvider].
class ProjectRequestNotifier extends Notifier<ProjectRequest?> {
  int _serial = 0;

  @override
  ProjectRequest? build() {
    ref.listen(flowEventsProvider, (
      AsyncValue<FlowEvent>? previous,
      AsyncValue<FlowEvent> next,
    ) {
      if (next.value case FlowEventOpenInApp(:final String path)) {
        _ask(path);
      }
    });
    final String? initial = ref.read(launchOptionsProvider).projectPath;
    return initial == null
        ? null
        : ProjectRequest(path: initial, serial: ++_serial);
  }

  void _ask(String path) {
    ref.read(sessionViewProvider.notifier).detach();
    state = ProjectRequest(path: path, serial: ++_serial);
  }

  /// Hands the request to the start window once: returns it and forgets
  /// it, so that a start window built again later does not preselect the
  /// same folder a second time.
  ProjectRequest? take() {
    final ProjectRequest? request = state;
    if (request != null) {
      state = null;
    }
    return request;
  }
}
