/// Where the inference of the next session goes, for the places where the
/// agent or its endpoint is chosen (HUM-516).
///
/// The daemon answers it in its snapshot (`Sandbox(Status)`), the same
/// question the sandbox screen asks; a `Plan` would also carry the choices
/// of that screen, and the settings have none to make. The app decides
/// nothing itself (ADR-018). Asked again whenever the configuration changed,
/// so the note in the settings follows the choice that was just made.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../domain/domain.dart';
import 'config_providers.dart';
import 'client_providers.dart';
import 'daemon_client.dart';

/// The cloud inference the next start would get, or
/// null when it stays on this machine or in the own network. Null as well
/// while the daemon cannot plan: a note that cannot be backed by an answer
/// does not stand.
final FutureProvider<CloudInference?> plannedCloudInferenceProvider =
    FutureProvider.autoDispose<CloudInference?>((Ref ref) async {
      ref.watch(configProvider);
      final DaemonClient client = ref.watch(daemonClientProvider);
      CloudInference? found;
      try {
        await for (final SandboxUpdate update in client.sandboxStatus()) {
          if (update is SandboxUpdateStatus) {
            found = update.status.cloudInference;
          }
        }
      } on DaemonException {
        return null;
      }
      return found;
    });
