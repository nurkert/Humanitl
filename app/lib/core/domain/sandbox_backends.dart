/// The sandbox backends of this system, as the daemon reports them
/// (HUM-432, `SandboxBackends`).
///
/// Plain value types: a read-only view of an answer of the daemon. The app
/// checks nothing itself (ADR-018); whether a backend is available, which one
/// the configuration chooses and what is missing all come from the daemon.
library;

import 'diagnostic.dart';

/// How strongly a backend separates the sandbox from the host, from weak to
/// strong; mirror of `IsolationLevel`. The same three names as
/// `sandbox.min_isolation`.
enum IsolationLevel {
  /// A kernel policy over a process that sees the real paths (Seatbelt).
  policy,

  /// Own namespaces over the shared kernel of the host (the backend of the
  /// system on Linux, Podman with `crun`).
  namespaces,

  /// An own guest kernel (Podman with `krun`).
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

  /// `crun` or `krun`.
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

  /// The runtimes of the backend where it has several (Podman).
  final List<SandboxRuntimeInfo> runtimes;
}

/// The answer of `SandboxBackends`.
final class SandboxBackendList {
  /// Creates a list.
  const SandboxBackendList({
    this.backends = const <SandboxBackendInfo>[],
    this.minIsolation,
  });

  /// The backends this kind of system can have, in a fixed order.
  final List<SandboxBackendInfo> backends;

  /// `sandbox.min_isolation` of the configuration without a project.
  final IsolationLevel? minIsolation;
}
