// Fixtures of the history tests: flows in every visual state, and the export
// entries built from them.

import 'dart:convert';
import 'dart:typed_data';

import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/features/history/export/export_entry.dart';

/// The session every fixture flow belongs to.
const SessionId testSession = SessionId('018f0004-0000-7000-8000-000000000001');

/// The rule the automatic decisions carry.
const RuleId testRule = RuleId('018f0005-0000-7000-8000-0000000000b7');

/// A fixed moment, so a formatter test never depends on the clock.
final DateTime testEpoch = DateTime.utc(2026, 9, 3, 8, 15, 30);

/// A flow with the fields every fixture shares.
Flow testFlow({
  required String id,
  String host = 'api.github.com',
  String path = '/graphql',
  Method method = Method.post,
  FlowState state = FlowState.recorded,
  DecisionKind? decision,
  DecisionSource? source,
  BlockReason? blockReason,
  RuleId? ruleId,
  int status = 200,
  int requestSize = 512,
  int responseSize = 2048,
  Duration? duration = const Duration(milliseconds: 120),
  int findingCount = 0,
  bool edited = false,
  bool passthrough = false,
  UpstreamError? upstreamError,
  DateTime? receivedAt,
  DateTime? deadline,
}) => Flow(
  id: FlowId(id),
  sessionId: testSession,
  receivedAt: receivedAt ?? testEpoch,
  method: method,
  scheme: Scheme.https,
  authority: Authority(host: host, port: 443),
  path: path,
  state: state,
  decision: decision,
  decisionSource: source,
  blockReason: blockReason,
  ruleId: ruleId,
  status: status,
  requestSize: requestSize,
  responseSize: responseSize,
  duration: duration,
  findingCount: findingCount,
  edited: edited,
  passthrough: passthrough,
  upstreamError: upstreamError,
  deadline: deadline,
);

/// A detail around [flow], with headers and a body of [body].
FlowDetail testDetail(Flow flow, {String body = '{"query":"{ viewer }"}'}) {
  final Uint8List bytes = Uint8List.fromList(utf8.encode(body));
  return FlowDetail(
    summary: flow,
    request: HttpRequest(
      method: flow.method,
      scheme: flow.scheme,
      authority: flow.authority,
      pathAndQuery: flow.path,
      headers: <Header>[
        Header(name: 'content-type', value: utf8.encode('application/json')),
        Header(name: 'accept', value: utf8.encode('application/json')),
      ],
      body: BodyRef(
        sha256: List<int>.filled(32, 7),
        size: bytes.length,
        contentType: 'application/json',
      ),
      version: 'HTTP/1.1',
    ),
    response: flow.status == 0
        ? null
        : HttpResponseHead(
            status: flow.status,
            version: 'HTTP/1.1',
            headers: <Header>[
              Header(name: 'server', value: utf8.encode('github.com')),
            ],
          ),
    responseBody: BodyRef(
      sha256: List<int>.filled(32, 9),
      size: 24,
      contentType: 'application/json',
    ),
  );
}

/// An export entry around [flow].
HistoryExportEntry testEntry(
  Flow flow, {
  String requestBody = '{"query":"{ viewer }"}',
  String responseBody = '{"data":{"viewer":1}}',
}) => HistoryExportEntry(
  detail: testDetail(flow, body: requestBody),
  requestBody: Uint8List.fromList(utf8.encode(requestBody)),
  responseBody: Uint8List.fromList(utf8.encode(responseBody)),
);
