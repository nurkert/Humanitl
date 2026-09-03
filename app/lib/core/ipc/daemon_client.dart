/// The port through which the app talks to the daemon (ARCHITECTURE 5).
///
/// Two adapters implement it: [GrpcDaemonClient] over the Unix socket and
/// [FakeDaemonClient] in process. Screens are built against this interface
/// and never see either adapter; widget tests always run against the fake.
///
/// [GrpcDaemonClient]: grpc_daemon_client.dart
/// [FakeDaemonClient]: fake_daemon_client.dart
library;

import 'dart:typed_data';

import '../domain/domain.dart';

/// Everything a client can ask the daemon (`service Humanitl`, MVP subset).
///
/// Every failure surfaces as a [DaemonException] carrying a [Diagnostic]
/// with a registered code; callers never see a raw transport error.
abstract class DaemonClient {
  /// `GetInfo`: version, protocol and capabilities of the daemon.
  Future<DaemonInfo> getInfo();

  /// `Subscribe`: the event stream, from now on or from [since].
  ///
  /// The stream ends with an error when the daemon goes away; reconnecting
  /// with backoff is the job of the provider that owns the stream (HUM-020).
  Stream<FlowEvent> subscribe({FlowId? since, bool includePassthrough = false});

  /// `Decide`: decides one held flow, optionally creating [remember] first.
  ///
  /// Throws a [DaemonException] with the daemon's diagnostic when the flow was
  /// not decided (`IPC_003` once it is no longer held).
  Future<void> decide(FlowId id, Decision decision, {Rule? remember});

  /// `ListFlows`: one page of the history.
  Future<FlowPage> listFlows(
    FlowFilter filter, {
    int limit = 200,
    String? cursor,
  });

  /// `GetFlow`: everything about one flow.
  Future<FlowDetail> getFlow(FlowId id);

  /// `GetBody`: the content behind a body reference, in chunks.
  Stream<Uint8List> getBody(BodyRef ref);

  /// Releases the transport. The client is unusable afterwards.
  Future<void> close();
}

/// A daemon call failed. [diagnostic] says why in a form a person can read.
class DaemonException implements Exception {
  /// Wraps [diagnostic].
  const DaemonException(this.diagnostic);

  /// The cause, with a registered code.
  final Diagnostic diagnostic;

  /// The code of [diagnostic], for quick matching.
  String get code => diagnostic.code;

  @override
  String toString() => 'DaemonException(${diagnostic.code}: ${diagnostic.why})';
}
