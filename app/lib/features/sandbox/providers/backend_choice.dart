/// The backend the next start asks for, chosen on the sandbox screen
/// (HUM-440).
///
/// The choice applies to the starts of this app, never to the setting: it
/// goes out as `sandbox.backend` in `Start.cli_overrides`, and
/// `config.toml` stays as it is. Its default is the setting. Every snapshot
/// carries it as `Plan.backend`, so the findings under the start button are
/// the ones of a start on the chosen backend.
library;

import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/config_providers.dart';
import '../../../core/ipc/sandbox_backends_feed.dart';

part 'backend_choice.g.dart';

/// The backend somebody chose for the next start, or null while nobody did.
///
/// Null means: no entry in `Start.cli_overrides`, and the daemon takes
/// `sandbox.backend` from its configuration, the same value the choice then
/// shows. As with the switch "Disposable" (HUM-357), a choice equal to the
/// setting does not go along either ([sandboxBackendOverride]).
@Riverpod(keepAlive: true)
class SandboxBackendChoice extends _$SandboxBackendChoice {
  @override
  String? build() => null;

  /// Chooses [backend] for the next start.
  void choose(String backend) => state = backend;
}

/// The backend `sandbox.backend` of [config] writes, `auto` when it is
/// empty.
String _setting(ConfigValues config) {
  final String written = config.sandboxBackend.trim();
  return written.isEmpty ? FormKeys.sandboxBackendAuto : written;
}

/// Whether [list] lets a person choose [backend] now: `auto` when the entry
/// behind it is available ([SandboxBackendList.autoAvailable]), any other
/// name when the daemon lists it as admitted and available. Without a list
/// nothing is known, and the choice stands.
bool _choosable(String backend, SandboxBackendList? list) {
  if (list == null) {
    return true;
  }
  if (backend == FormKeys.sandboxBackendAuto) {
    return list.autoAvailable;
  }
  return list.offered.any(
    (SandboxBackendInfo entry) => entry.name == backend && entry.available,
  );
}

/// The backend the choice shows: [choice] while [list] lets it be chosen,
/// otherwise `sandbox.backend` of [config]. Null while [config] has not
/// arrived: the surfaces then show no selected segment, rather than `auto`
/// beside a setting that says otherwise.
///
/// A choice that stops being available (a project raised the lower bound, a
/// backend went missing) falls back to the setting, and so does what the
/// start sends ([sandboxBackendOverride]); the screen never shows one backend
/// and starts another.
String? shownSandboxBackend(
  String? choice,
  ConfigValues? config,
  SandboxBackendList? list,
) {
  if (config == null) {
    return null;
  }
  if (choice != null && _choosable(choice, list)) {
    return choice;
  }
  return _setting(config);
}

/// What a start sends as `sandbox.backend` in `Start.cli_overrides`, and the
/// snapshot as `Plan.backend`: the backend [shownSandboxBackend] shows when
/// it differs from the setting in [config], otherwise null.
String? sandboxBackendOverride(
  String? choice,
  ConfigValues? config,
  SandboxBackendList? list,
) {
  final String? shown = shownSandboxBackend(choice, config, list);
  if (shown == null || config == null) {
    return null;
  }
  return shown == _setting(config) ? null : shown;
}

/// The backend of the session as it goes out (HUM-440): `Plan.backend` of
/// every snapshot and of `SandboxBackends` on the sandbox screen, and
/// `sandbox.backend` in `Start.cli_overrides`. Null while nobody chose, or
/// the choice equals the setting or cannot be chosen now.
///
/// Measured against the list for the setting (`sandboxBackendsProvider(null)`):
/// whether a backend is available does not depend on which one is selected,
/// and the list for the session's backend depends on this value itself.
@Riverpod(keepAlive: true)
String? sessionSandboxBackend(Ref ref) {
  final String? choice = ref.watch(sandboxBackendChoiceProvider);
  if (choice == null) {
    return null;
  }
  return sandboxBackendOverride(
    choice,
    ref.watch(configProvider).value,
    ref.watch(sandboxBackendsProvider(null)).value,
  );
}
