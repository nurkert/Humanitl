/// The sandbox backends of this system, as the daemon reports them
/// (HUM-432, `SandboxBackends`).
///
/// Plain value types: a read-only view of an answer of the daemon. The app
/// checks nothing itself (ADR-018); whether a backend is available, which one
/// the configuration chooses and what is missing all come from the daemon.
library;

import 'diagnostic.dart';
import 'diagnostic_codes.dart';

/// How strongly a backend separates the sandbox from the host, from weak to
/// strong; mirror of `IsolationLevel`. The same three names as
/// `sandbox.min_isolation`.
enum IsolationLevel {
  /// A kernel policy over a process that sees the real paths (Seatbelt).
  policy,

  /// Own namespaces over the shared kernel of the host (the backend of the
  /// system on Linux, Podman with `crun`).
  namespaces,

  /// An own guest kernel in a micro VM (the VM backend of the kernel level,
  /// HUM-464).
  kernel,
}

/// One runtime of a backend (`sandbox.podman.runtime`), mirror of
/// `SandboxRuntime`.
final class SandboxRuntimeInfo {
  /// Creates a runtime entry.
  const SandboxRuntimeInfo({
    required this.name,
    this.isolation,
    this.available = false,
    this.selected = false,
    this.diagnostics = const <Diagnostic>[],
  });

  /// `crun`.
  final String name;

  /// The level with this runtime; null for a value this app does not know.
  final IsolationLevel? isolation;

  /// Whether a start with this runtime would pass.
  final bool available;

  /// Whether `sandbox.podman.runtime` chooses this runtime.
  final bool selected;

  /// What stands in the way of a start with this runtime.
  final List<Diagnostic> diagnostics;
}

/// One sandbox backend and what it lacks on this system, mirror of
/// `SandboxBackend`.
final class SandboxBackendInfo {
  /// Creates a backend entry.
  const SandboxBackendInfo({
    required this.name,
    this.isolation,
    this.available = false,
    this.selected = false,
    this.diagnostics = const <Diagnostic>[],
    this.runtimes = const <SandboxRuntimeInfo>[],
  });

  /// The name as `sandbox.backend` writes it, never `auto`.
  final String name;

  /// The level the backend offers; under Podman that of the chosen runtime.
  final IsolationLevel? isolation;

  /// Whether a start with this backend would pass. False never means that
  /// another backend steps in.
  final bool available;

  /// Whether `sandbox.backend` chooses this backend.
  final bool selected;

  /// What stands in the way of a start, each with `why` and where there is
  /// one a `fix`; empty when [available].
  final List<Diagnostic> diagnostics;

  /// The runtimes of the backend where it chooses one (Podman).
  final List<SandboxRuntimeInfo> runtimes;

  /// Whether the daemon admits this backend at all (HUM-440).
  ///
  /// False only when the daemon itself says so with
  /// [DiagnosticCodes.backendNotAdmitted]: such a backend is never offered as
  /// a choice, not even as a disabled one (`SANDBOX_048`). Everything else
  /// that stands in the way of a start keeps the backend in the list, and
  /// the reason stands beside it.
  bool get admitted => !diagnostics.any(
    (Diagnostic d) => d.code == DiagnosticCodes.backendNotAdmitted,
  );

  /// What stands in the way of a start first, or null when [available]:
  /// the first diagnostic of blocking severity, otherwise the first one.
  /// Also null for an unavailable backend without any diagnostic; the
  /// surfaces say "not available" then without a reason.
  Diagnostic? get reason {
    if (available || diagnostics.isEmpty) {
      return null;
    }
    for (final Diagnostic diagnostic in diagnostics) {
      if (diagnostic.severity == Severity.blocking) {
        return diagnostic;
      }
    }
    return diagnostics.first;
  }
}

/// The answer of `SandboxBackends`.
final class SandboxBackendList {
  /// Creates a list.
  const SandboxBackendList({
    this.backends = const <SandboxBackendInfo>[],
    this.minIsolation,
    this.autoBackend,
  });

  /// The backends this kind of system can have, in a fixed order.
  final List<SandboxBackendInfo> backends;

  /// `sandbox.min_isolation` of the configuration the list was measured
  /// against, with the project profile of the plan's folder.
  final IsolationLevel? minIsolation;

  /// The name of the backend `auto` means on this system (HUM-440); null
  /// from a daemon before contract 1.38.
  final String? autoBackend;

  /// The entry of the backend `auto` means, or null when the daemon did not
  /// name one that is listed (HUM-440).
  SandboxBackendInfo? get autoEntry {
    for (final SandboxBackendInfo backend in backends) {
      if (backend.name == autoBackend) {
        return backend;
      }
    }
    return null;
  }

  /// Whether a start with `auto` would pass, as the entry of the backend
  /// behind `auto` says. True when the daemon named none: an older daemon
  /// says nothing about `auto`, and the start reports its own finding.
  bool get autoAvailable => autoEntry?.available ?? true;

  /// The backends a person may choose from, in the daemon's order: every one
  /// the daemon admits, available or not (HUM-440).
  List<SandboxBackendInfo> get offered => <SandboxBackendInfo>[
    for (final SandboxBackendInfo backend in backends)
      if (backend.admitted) backend,
  ];
}
