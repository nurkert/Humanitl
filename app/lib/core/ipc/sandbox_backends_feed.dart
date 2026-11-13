/// The sandbox backends of this system as the daemon lists them, in core
/// (HUM-440).
///
/// The settings and the sandbox screen both offer the choice of the backend,
/// and no feature imports another one (ARCHITECTURE 5). Every statement about
/// a backend comes from `SandboxBackends` (HUM-432); the app checks nothing
/// of its own (ADR-018).
library;

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
@Riverpod(keepAlive: true, retry: noConfigRetry)
Future<SandboxBackendList> sandboxBackends(Ref ref, String? backend) {
  ref
    ..watch(linkLiveProvider)
    ..watch(configProvider);
  return ref.watch(daemonClientProvider).sandboxBackends(backend: backend);
}
