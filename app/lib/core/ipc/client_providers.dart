/// Der Client, durch den die ganze Anwendung mit dem Daemon spricht.
///
/// Er liegt in `core/ipc`, nicht in einem Feature: Intercept, History und die
/// Shell brauchen denselben Client, und ein Feature darf nicht aus einem
/// anderen importieren (ARCHITECTURE 5). Widget-Tests überschreiben
/// [daemonClientProvider] mit einem [FakeDaemonClient].
library;

import 'package:flutter/foundation.dart' show visibleForTesting;
import 'package:riverpod_annotation/riverpod_annotation.dart';

import 'daemon_client.dart';
import 'daemon_paths.dart';
import 'fake_daemon_client.dart';
import 'grpc_daemon_client.dart';
import 'launch_options.dart';

part 'client_providers.g.dart';

/// How the app was started. `main` overrides this with the parsed options;
/// the default is the real daemon on the XDG socket.
@Riverpod(keepAlive: true)
LaunchOptions launchOptions(Ref ref) => const LaunchOptions();

/// The client the whole app talks through, chosen by [launchOptionsProvider]
/// (CONVENTIONS 4.7). Widget tests override it with a [FakeDaemonClient].
@Riverpod(keepAlive: true)
DaemonClient daemonClient(Ref ref) {
  final LaunchOptions options = ref.watch(launchOptionsProvider);
  final DaemonClient client = switch (options.mode) {
    ClientMode.fakeClient => FakeDaemonClient.scenario(options.scenario ?? ''),
    ClientMode.daemon || ClientMode.fakeDaemon => _grpcClient(options),
  };
  ref.onDispose(client.close);
  return client;
}

GrpcDaemonClient _grpcClient(LaunchOptions options) => grpcClientFor(options);

/// The gRPC client for [options]; [resolve] stands in for
/// [DaemonPaths.resolve] in tests.
///
/// The client learns here whether the runtime directory is the fallback below
/// the temporary directory: only then does an untrusted directory propose one
/// of the user's own (HUM-212).
@visibleForTesting
GrpcDaemonClient grpcClientFor(
  LaunchOptions options, {
  DaemonPaths Function() resolve = DaemonPaths.resolve,
}) {
  final String? socket = options.socketPath;
  final DaemonPaths paths = socket == null
      ? resolve()
      : DaemonPaths.besideSocket(socket);
  return GrpcDaemonClient(
    socketPath: paths.socket,
    tokenPath: paths.token,
    fake: options.mode == ClientMode.fakeDaemon,
    socketFlag: socket != null,
    runtimeFallback: paths.fallbackUsed,
  );
}
