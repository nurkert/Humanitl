/// The sandbox backends of this system as the daemon lists them, in core
/// (HUM-440).
///
/// The settings and the sandbox screen both offer the choice of the backend,
/// and no feature imports another one (ARCHITECTURE 5). Every statement about
/// a backend comes from `SandboxBackends` (HUM-432); the app checks nothing
/// of its own (ADR-018).
library;

import 'dart:async';

import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../domain/domain.dart';
import 'client_providers.dart';
import 'config_providers.dart';
import 'connection.dart';

part 'sandbox_backends_feed.g.dart';

/// `SandboxBackends` for the plan of this app with [backend] as the backend
/// of the session (`Plan.backend`, HUM-440; null for the setting), asked
/// again whenever the link comes back and after every change of the
/// configuration.
///
/// The plan is the one of this app's snapshots: no folder of its own, so the
/// daemon takes the stored `sandbox.work_dir` with its project profile, and
/// no profile. Folder and profile change only through the configuration, so
/// watching [configProvider] asks again for every change of the plan; the
/// backend of the session is the parameter. Which backend is selected, and
/// whether one lies below `sandbox.min_isolation` of the project, follow
/// from there. A daemon before contract 1.34 does not know the call; the
/// provider then holds its error, and the surfaces offer no choice.
///
/// A list in which the daemon was still checking a backend (HUM-502,
/// [SandboxBackendList.checking]) is asked again after
/// [sandboxBackendsCheckingRetry]; until then, and while the new answer is
/// on its way, the surfaces keep showing this one.
@Riverpod(keepAlive: true, retry: noConfigRetry)
Future<SandboxBackendList> sandboxBackends(Ref ref, String? backend) async {
  ref
    ..watch(linkLiveProvider)
    ..watch(configProvider);
  final SandboxBackendList list = await ref
      .watch(daemonClientProvider)
      .sandboxBackends(backend: backend);
  if (list.checking && ref.mounted) {
    final Timer again = Timer(sandboxBackendsCheckingRetry, ref.invalidateSelf);
    ref.onDispose(again.cancel);
  }
  return list;
}

/// Whether [answer] still holds the list of a previous plan while the one
/// for the current plan is on its way (HUM-510).
///
/// A change of the configuration (another project folder, another lower
/// bound, another setting) or a link that comes back asks the same member of
/// [sandboxBackendsProvider] again, and Riverpod keeps the old list as its
/// value until the answer is there. That list belongs to the old plan; the
/// surfaces show it muted and offer no choice from it, as for a new member
/// without a value (HUM-504). Which settings the plan reads, the app does not
/// judge (ADR-018): every change of the configuration counts.
///
/// The second ask while the daemon is still checking a backend (HUM-502,
/// [sandboxBackendsCheckingRetry]) is no change of the plan: the provider
/// asks itself again with the same inputs, the old list is still the current
/// one, and nothing dims every two seconds. Riverpod tells the two apart: a
/// rebuild after a changed dependency is a reload, one after
/// `invalidateSelf` a refresh.
bool sandboxBackendsOutdated(AsyncValue<SandboxBackendList> answer) =>
    answer.isReloading;

/// How long the list waits before it asks again while the daemon is still
/// checking a backend (HUM-502). The daemon keeps searching in between and
/// answers from its memory, so asking again costs no new search.
const Duration sandboxBackendsCheckingRetry = Duration(seconds: 2);
