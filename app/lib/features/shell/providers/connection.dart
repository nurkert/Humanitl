/// Was `GetInfo` gesagt hat und ob die Shell zeigen darf
/// (`daemonInfoProvider`, `connectionStateProvider`; HUM-019 Spezifikation).
///
/// Der Client selbst steht in `core/ipc/client_providers.dart`, weil ihn jedes
/// Feature braucht und kein Feature aus einem anderen importieren darf
/// (ARCHITECTURE 5).
///
/// The version check lives here, not in the client: the fake reports whatever
/// it is told, and the app decides what it accepts.
library;

import 'dart:async';

import 'package:freezed_annotation/freezed_annotation.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_diagnostics.dart';
import '../../../core/ipc/daemon_client.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/proto_version.dart';

part 'connection.freezed.dart';
part 'connection.g.dart';

/// How often the connection is confirmed with a `GetInfo` while connected,
/// or null for never. Two seconds keeps "daemon stopped" under the five
/// seconds HUM-019 asks for; tests override it.
@Riverpod(keepAlive: true)
Duration? connectionHeartbeat(Ref ref) => const Duration(seconds: 2);

/// Riverpod 3 retries a failed provider on its own (backoff from 200 ms, ten
/// attempts) and reports `AsyncLoading` with the error tucked inside while it
/// does, so the gate would never see the failure. Reconnecting is explicit
/// here: the button and the palette command call [DaemonConnection.retry];
/// the heartbeat only notices a daemon that went away.
Duration? noConnectionRetry(int retryCount, Object error) => null;

/// `GetInfo` plus the version check. Retry with
/// `ref.invalidate(daemonInfoProvider)`; never automatically.
@Riverpod(retry: noConnectionRetry)
Future<DaemonInfo> daemonInfo(Ref ref) async {
  final DaemonInfo info = await ref.watch(daemonClientProvider).getInfo();
  if (!ProtoVersion.isCompatible(info.protoMajor)) {
    throw DaemonException(ClientDiagnostics.protoIncompatible(info));
  }
  return info;
}

/// The three states of the connection gate.
@freezed
sealed class ConnectionStatus with _$ConnectionStatus {
  /// `GetInfo` is in flight.
  const factory ConnectionStatus.connecting() = ConnectionConnecting;

  /// The daemon answered and is compatible.
  const factory ConnectionStatus.connected({required DaemonInfo info}) =
      ConnectionConnected;

  /// No usable daemon; the setup screen shows [diagnostic].
  const factory ConnectionStatus.failed({required Diagnostic diagnostic}) =
      ConnectionFailed;

  const ConnectionStatus._();

  /// The daemon description when connected, otherwise null.
  DaemonInfo? get info => switch (this) {
    ConnectionConnected(:final info) => info,
    _ => null,
  };
}

/// `connectionStateProvider`: derives [ConnectionStatus] from
/// [daemonInfoProvider] and, while connected, keeps confirming the daemon
/// with a heartbeat so that a stopped daemon shows up as a failure.
@Riverpod(keepAlive: true, name: 'connectionStateProvider')
class DaemonConnection extends _$DaemonConnection {
  Timer? _heartbeat;

  @override
  ConnectionStatus build() {
    ref.onDispose(_stopHeartbeat);
    final AsyncValue<DaemonInfo> info = ref.watch(daemonInfoProvider);
    // `isLoading` first: after `retry` Riverpod reports the new attempt with
    // the previous result still attached, and the gate shows the splash,
    // not the stale outcome.
    return switch (info) {
      AsyncValue(isLoading: true) => const ConnectionStatus.connecting(),
      AsyncData(:final value) => _connected(value),
      AsyncError(:final error) => ConnectionStatus.failed(
        diagnostic: diagnosticOf(error),
      ),
      // `AsyncLoading` is caught above; the analyzer cannot see that.
      _ => const ConnectionStatus.connecting(),
    };
  }

  /// Tries again: back to connecting, then whatever `GetInfo` says.
  void retry() => ref.invalidate(daemonInfoProvider);

  ConnectionStatus _connected(DaemonInfo info) {
    _startHeartbeat();
    return ConnectionStatus.connected(info: info);
  }

  void _startHeartbeat() {
    _stopHeartbeat();
    final Duration? interval = ref.read(connectionHeartbeatProvider);
    if (interval != null) {
      _heartbeat = Timer.periodic(interval, (_) => _beat());
    }
  }

  void _stopHeartbeat() {
    _heartbeat?.cancel();
    _heartbeat = null;
  }

  Future<void> _beat() async {
    try {
      final DaemonInfo info = await ref.read(daemonClientProvider).getInfo();
      if (!ref.mounted) {
        return;
      }
      if (!ProtoVersion.isCompatible(info.protoMajor)) {
        _fail(ClientDiagnostics.protoIncompatible(info));
      } else if (state case ConnectionConnected(info: final known)
          when known != info) {
        state = ConnectionStatus.connected(info: info);
      }
    } on Object catch (error) {
      if (ref.mounted) {
        _fail(diagnosticOf(error));
      }
    }
  }

  void _fail(Diagnostic diagnostic) {
    _stopHeartbeat();
    state = ConnectionStatus.failed(diagnostic: diagnostic);
  }

  /// The diagnostic behind [error]: the one a [DaemonException] carries, or
  /// a `DAEMON_001` describing anything else that reached the gate.
  static Diagnostic diagnosticOf(Object error) => switch (error) {
    DaemonException(:final diagnostic) => diagnostic,
    _ => ClientDiagnostics.daemonUnreachable(
      socketPath: '?',
      detail: error.toString(),
    ),
  };
}
