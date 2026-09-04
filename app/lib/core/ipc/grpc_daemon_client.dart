/// The gRPC adapter of [DaemonClient]: one Unix socket, one token, every call
/// translated into domain types and every failure into a [Diagnostic].
///
/// The socket needs `port: 0` on the channel; without it grpc-dart tries TCP
/// (HUM-019 Fallstricke). The token is read from the file next to the socket
/// on every call: the daemon writes a fresh one each start, and a cached token
/// would turn every restart into an `IPC_001`.
library;

import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:grpc/grpc.dart';
import 'package:protobuf/protobuf.dart' show InvalidProtocolBufferException;
import 'package:protobuf/well_known_types/google/protobuf/empty.pb.dart';

import '../domain/domain.dart';
import 'client_diagnostics.dart';
import 'convert.dart';
import 'daemon_client.dart';
import 'generated/humanitl/v1/humanitl.pbgrpc.dart' as pb;
// `Rule` and its parts live in their own wire file, and `humanitl.pb.dart`
// imports it without re-exporting it. Same prefix, one vocabulary.
import 'generated/humanitl/v1/rules.pb.dart' as pb;
import 'proto_version.dart';

/// The trailer in which tonic ships a `Diagnostic` with a failed call.
const String statusDetailsTrailer = 'grpc-status-details-bin';

/// [DaemonClient] over the daemon's Unix socket.
class GrpcDaemonClient implements DaemonClient {
  /// Creates a client for the socket at [socketPath] with the token file at
  /// [tokenPath]. [channel] is injectable for tests; [fake] and [socketFlag]
  /// only shape the fix proposal of a `DAEMON_001`.
  GrpcDaemonClient({
    required this.socketPath,
    required this.tokenPath,
    ClientChannel? channel,
    this.callTimeout = const Duration(seconds: 5),
    this.fake = false,
    this.socketFlag = false,
  }) : _channel =
           channel ??
           ClientChannel(
             InternetAddress(socketPath, type: InternetAddressType.unix),
             port: 0,
             options: const ChannelOptions(
               credentials: ChannelCredentials.insecure(),
             ),
           );

  /// Path of the daemon socket.
  final String socketPath;

  /// Path of the token file.
  final String tokenPath;

  /// Deadline of a unary call. Streams have none.
  final Duration callTimeout;

  /// True when the app expects `humanitld --fake` on the socket.
  final bool fake;

  /// True when the socket was chosen with `--socket`.
  final bool socketFlag;

  final ClientChannel _channel;
  late final pb.HumanitlClient _stub = pb.HumanitlClient(_channel);

  @override
  Future<DaemonInfo> getInfo() async {
    final pb.Info info = await _unary(
      (CallOptions options) => _stub.getInfo(Empty(), options: options),
    );
    return info.toDomain();
  }

  @override
  Stream<FlowEvent> subscribe({
    FlowId? since,
    bool includePassthrough = false,
  }) async* {
    final CallOptions options = await _options(timeout: null);
    final pb.SubscribeRequest request = pb.SubscribeRequest()
      ..sinceFlowId = since?.value ?? ''
      ..includePassthrough = includePassthrough;
    try {
      await for (final pb.FlowEvent event in _stub.subscribe(
        request,
        options: options,
      )) {
        final FlowEvent? domain = event.toDomain();
        if (domain != null) {
          yield domain;
        }
      }
    } on GrpcError catch (error) {
      throw DaemonException(_translate(error));
    } on IOException catch (error) {
      throw DaemonException(_unreachable('$error'));
    }
  }

  @override
  Future<Rule?> decide(FlowId id, Decision decision, {Rule? remember}) async {
    final pb.DecideResponse response = await _unary(
      (CallOptions options) => _stub.decide(
        decision.toProto(id, remember: remember),
        options: options,
      ),
    );
    for (final pb.DecideResult result in response.results) {
      if (!result.applied && result.hasDiagnostic()) {
        throw DaemonException(result.diagnostic.toDomain());
      }
    }
    return response.hasCreatedRule() ? response.createdRule.toDomain() : null;
  }

  @override
  Future<void> removeRule(RuleId id) async {
    final pb.RulesResponse response = await _unary(
      (CallOptions options) =>
          _stub.rules(pb.RulesRequest()..remove = id.value, options: options),
    );
    if (response.hasDiagnostic()) {
      throw DaemonException(response.diagnostic.toDomain());
    }
  }

  @override
  Future<RuleSet> listRules() => _rules(pb.RulesRequest()..list = Empty());

  @override
  Future<RuleSet> addRule(Rule rule) =>
      _rules(pb.RulesRequest()..add = rule.toProto());

  @override
  Future<RuleSet> updateRule(Rule rule) =>
      _rules(pb.RulesRequest()..update = rule.toProto());

  @override
  Future<RuleSet> reorderRules(List<RuleId> order) => _rules(
    pb.RulesRequest()
      ..reorder = (pb.RulesRequest_Reorder()
        ..ruleIdsInOrder.addAll(order.map((RuleId id) => id.value))),
  );

  @override
  Future<RuleSet> makeRulePermanent(RuleId id) =>
      _rules(pb.RulesRequest()..makePermanent = id.value);

  /// A reload never throws on the findings it carries: a refused file leaves
  /// the rules that were in force in force, and the findings are the answer,
  /// not an error (`daemon/crates/ipc/src/rules.rs`).
  @override
  Future<RuleSet> reloadRules() =>
      _rules(pb.RulesRequest()..reload = Empty(), raiseFirst: false);

  @override
  Future<DryRun> dryRunRule(Rule rule, {int limit = dryRunScanDefault}) async {
    final pb.RulesResponse response = await _unary(
      (CallOptions options) => _stub.rules(
        pb.RulesRequest()
          ..dryRun = (pb.RulesRequest_DryRun()
            ..rule = rule.toProto()
            ..limit = limit),
        options: options,
      ),
    );
    return DryRun(
      matches: List<Flow>.unmodifiable(
        response.dryRunMatches.map(
          (pb.FlowSummary summary) => summary.toDomain(),
        ),
      ),
      scanned: response.dryRunScanned,
    );
  }

  /// One `Rules` call, translated.
  ///
  /// With [raiseFirst] a diagnostic in the answer is thrown: the operation
  /// reported something it could not do, and the caller asked for an outcome,
  /// not for a report.
  Future<RuleSet> _rules(
    pb.RulesRequest request, {
    bool raiseFirst = true,
  }) async {
    final pb.RulesResponse response = await _unary(
      (CallOptions options) => _stub.rules(request, options: options),
    );
    if (raiseFirst && response.hasDiagnostic()) {
      throw DaemonException(response.diagnostic.toDomain());
    }
    return RuleSet(
      rules: List<Rule>.unmodifiable(
        response.rules.map((pb.Rule rule) => rule.toDomain()),
      ),
      diagnostics: List<Diagnostic>.unmodifiable(
        response.diagnostics.map((pb.Diagnostic d) => d.toDomain()),
      ),
    );
  }

  @override
  Future<FlowPage> listFlows(
    FlowFilter filter, {
    int limit = 200,
    String? cursor,
  }) async {
    final pb.FlowPage page = await _unary(
      (CallOptions options) => _stub.listFlows(
        filter.toProto(limit: limit, cursor: cursor),
        options: options,
      ),
    );
    return page.toDomain();
  }

  @override
  Future<FlowDetail> getFlow(FlowId id) async {
    final pb.FlowDetail detail = await _unary(
      (CallOptions options) =>
          _stub.getFlow(pb.FlowRef()..flowId = id.value, options: options),
    );
    return detail.toDomain();
  }

  @override
  Stream<Uint8List> getBody(BodyRef ref) async* {
    final CallOptions options = await _options(timeout: null);
    try {
      await for (final pb.BodyChunk chunk in _stub.getBody(
        ref.toProto(),
        options: options,
      )) {
        yield Uint8List.fromList(chunk.data);
      }
    } on GrpcError catch (error) {
      throw DaemonException(_translate(error));
    } on IOException catch (error) {
      throw DaemonException(_unreachable('$error'));
    }
  }

  @override
  Future<void> close() => _channel.shutdown();

  Future<T> _unary<T>(Future<T> Function(CallOptions options) call) async {
    final CallOptions options = await _options(timeout: callTimeout);
    try {
      return await call(options);
    } on GrpcError catch (error) {
      throw DaemonException(_translate(error));
    } on IOException catch (error) {
      throw DaemonException(_unreachable('$error'));
    }
  }

  Future<CallOptions> _options({required Duration? timeout}) async {
    final String token = await _readToken();
    return CallOptions(
      metadata: <String, String>{ProtoVersion.tokenMetadataKey: token},
      timeout: timeout,
    );
  }

  /// The token file is written by the daemon at start and removed at exit;
  /// an unreadable file therefore means "no daemon", not "bad token".
  Future<String> _readToken() async {
    try {
      final String token = (await File(tokenPath).readAsString()).trim();
      if (token.isEmpty) {
        throw DaemonException(_unreachable('token file $tokenPath is empty'));
      }
      return token;
    } on IOException catch (error) {
      throw DaemonException(_unreachable('cannot read $tokenPath: $error'));
    }
  }

  Diagnostic _translate(GrpcError error) => diagnosticFromGrpcError(
    error,
    socketPath: socketPath,
    tokenPath: tokenPath,
    fake: fake,
    socketFlag: socketFlag,
  );

  Diagnostic _unreachable(String detail) => ClientDiagnostics.daemonUnreachable(
    socketPath: socketPath,
    detail: detail,
    fake: fake,
    socketFlag: socketFlag,
  );
}

/// Translates a failed call into a [Diagnostic].
///
/// A diagnostic the daemon put into the status details wins, code and all.
/// Without one, `UNAUTHENTICATED` is `IPC_001` and everything else is
/// `DAEMON_001` with the gRPC status in `why`: when the only call of this
/// issue, `GetInfo`, fails for any other reason, the daemon is not usable,
/// and the register has no code for "some other transport failure".
Diagnostic diagnosticFromGrpcError(
  GrpcError error, {
  required String socketPath,
  required String tokenPath,
  bool fake = false,
  bool socketFlag = false,
}) {
  final Diagnostic? shipped = diagnosticFromTrailers(error.trailers);
  if (shipped != null) {
    return shipped;
  }
  final String detail = <String>[
    grpcStatusName(error.code),
    if (error.message case final String message when message.isNotEmpty)
      message,
  ].join(': ');
  if (error.code == StatusCode.unauthenticated) {
    return ClientDiagnostics.tokenRejected(
      tokenPath: tokenPath,
      detail: detail,
    );
  }
  return ClientDiagnostics.daemonUnreachable(
    socketPath: socketPath,
    detail: detail,
    fake: fake,
    socketFlag: socketFlag,
  );
}

/// Reads the `Diagnostic` tonic ships in [statusDetailsTrailer], if any.
///
/// grpc-dart itself tries to parse the trailer as `google.rpc.Status` and
/// gives up quietly; the raw base64 stays in the trailers, which is where
/// this reads it. Both base64 alphabets and missing padding are accepted.
Diagnostic? diagnosticFromTrailers(Map<String, String>? trailers) {
  final String? raw = trailers?[statusDetailsTrailer];
  if (raw == null || raw.isEmpty) {
    return null;
  }
  try {
    final String normalized = raw.replaceAll('-', '+').replaceAll('_', '/');
    final String padded = normalized.padRight(
      (normalized.length + 3) & ~3,
      '=',
    );
    final pb.Diagnostic message = pb.Diagnostic.fromBuffer(
      base64.decode(padded),
    );
    if (message.code.isEmpty) {
      return null;
    }
    return message.toDomain();
  } on InvalidProtocolBufferException {
    return null;
  } on FormatException {
    return null;
  }
}

/// The name of a gRPC status code, for `why` lines.
String grpcStatusName(int code) => switch (code) {
  StatusCode.ok => 'OK',
  StatusCode.cancelled => 'CANCELLED',
  StatusCode.unknown => 'UNKNOWN',
  StatusCode.invalidArgument => 'INVALID_ARGUMENT',
  StatusCode.deadlineExceeded => 'DEADLINE_EXCEEDED',
  StatusCode.notFound => 'NOT_FOUND',
  StatusCode.alreadyExists => 'ALREADY_EXISTS',
  StatusCode.permissionDenied => 'PERMISSION_DENIED',
  StatusCode.resourceExhausted => 'RESOURCE_EXHAUSTED',
  StatusCode.failedPrecondition => 'FAILED_PRECONDITION',
  StatusCode.aborted => 'ABORTED',
  StatusCode.outOfRange => 'OUT_OF_RANGE',
  StatusCode.unimplemented => 'UNIMPLEMENTED',
  StatusCode.internal => 'INTERNAL',
  StatusCode.unavailable => 'UNAVAILABLE',
  StatusCode.dataLoss => 'DATA_LOSS',
  StatusCode.unauthenticated => 'UNAUTHENTICATED',
  _ => 'STATUS_$code',
};
