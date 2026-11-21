/// The folder `humanitl app PATH` preselected in the start window (HUM-539).
///
/// A request (`core/ui/project_request.dart`) is taken once and asked of
/// the daemon as `Projects.proposal` (ADR-018: whether the path is a folder
/// and whether the register knows it, the daemon says). A known project is
/// marked under "Recent" (or "Running"), a folder the register does not know
/// opens the first-start screen, and a path that is no folder stands as the
/// daemon's finding above the lists. Nothing here starts: a known project
/// starts with its "Start" button, a new folder with "Start" on the
/// first-start screen.
library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/daemon_client.dart';
import '../../../core/ui/project_request.dart';
import 'first_start.dart';

/// What the start window shows of the last request.
final class Preselection {
  /// Creates the state for [path].
  const Preselection({
    required this.path,
    this.projectId = '',
    this.finding,
    this.asking = false,
  });

  /// The folder as the request named it.
  final String path;

  /// The project of the register the folder belongs to, marked in the
  /// list; empty while asking and for a finding.
  final String projectId;

  /// Why the folder cannot be preselected, as the daemon said it (a file,
  /// a folder that is gone).
  final Diagnostic? finding;

  /// True while the daemon has not answered.
  final bool asking;
}

/// The preselection; null without a request or after one without a folder.
final NotifierProvider<PreselectionNotifier, Preselection?>
preselectionProvider =
    NotifierProvider.autoDispose<PreselectionNotifier, Preselection?>(
      PreselectionNotifier.new,
    );

/// The notifier behind [preselectionProvider].
class PreselectionNotifier extends Notifier<Preselection?> {
  /// Counts the requests; only the answer to the latest one counts.
  int _asked = 0;

  @override
  Preselection? build() {
    ref.listen<ProjectRequest?>(projectRequestProvider, (
      ProjectRequest? previous,
      ProjectRequest? next,
    ) {
      if (next != null) {
        _takeSoon();
      }
    });
    if (ref.read(projectRequestProvider) != null) {
      _takeSoon();
    }
    return null;
  }

  /// Forgets the preselection, the finding included.
  void dismiss() {
    _asked++;
    state = null;
  }

  /// Takes the request after the current build: a provider is not written
  /// to while another one builds.
  void _takeSoon() {
    unawaited(
      Future<void>.microtask(() {
        if (!ref.mounted) {
          return;
        }
        final ProjectRequest? request = ref
            .read(projectRequestProvider.notifier)
            .take();
        if (request != null) {
          unawaited(_open(request.path));
        }
      }),
    );
  }

  Future<void> _open(String path) async {
    final int ticket = ++_asked;
    // Ein offener Erststart-Bildschirm stünde sonst vor der neuen Vorwahl
    // (HUM-539, Review). Er bleibt mit seinen Eingaben nur, wenn die Bitte
    // genau seinen Ordner nennt.
    final FirstStart? open = ref.read(firstStartProvider);
    if (open != null) {
      if (path.isNotEmpty && open.path == path) {
        state = null;
        return;
      }
      ref.read(firstStartProvider.notifier).cancel();
    }
    if (path.isEmpty) {
      state = null;
      return;
    }
    state = Preselection(path: path, asking: true);
    final DaemonClient client = ref.read(daemonClientProvider);
    final StartProposal proposal;
    try {
      proposal = await client.projectsProposal(path: path);
    } on DaemonException catch (error) {
      if (ref.mounted && ticket == _asked) {
        state = Preselection(path: path, finding: error.diagnostic);
      }
      return;
    }
    if (!ref.mounted || ticket != _asked) {
      return;
    }
    if (proposal.known) {
      state = Preselection(path: proposal.path, projectId: proposal.projectId);
      return;
    }
    state = null;
    ref.read(firstStartProvider.notifier).show(path, proposal);
  }
}
