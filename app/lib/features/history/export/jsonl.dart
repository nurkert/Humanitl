/// JSON Lines for recorded flows: one flow per line, bodies base64 in
/// `body_b64` with a `truncated` flag beside them.
///
/// The format is meant to be read back, so [encodeJsonl] and [decodeJsonl]
/// are one pair and a test round-trips them.
library;

import 'dart:convert';

import '../../../core/domain/domain.dart';
import '../history_view.dart';
import 'export_entry.dart';

/// [entries] as JSON Lines, one object per line, terminated by a newline.
String encodeJsonl(List<HistoryExportEntry> entries) {
  final StringBuffer out = StringBuffer();
  for (final HistoryExportEntry entry in entries) {
    out.writeln(jsonEncode(jsonlRecord(entry)));
  }
  return out.toString();
}

/// Reads back what [encodeJsonl] wrote. Blank lines are skipped.
List<Map<String, Object?>> decodeJsonl(String text) => <Map<String, Object?>>[
  for (final String line in const LineSplitter().convert(text))
    if (line.trim().isNotEmpty) jsonDecode(line) as Map<String, Object?>,
];

/// One flow as a JSON object.
Map<String, Object?> jsonlRecord(HistoryExportEntry entry) {
  final Flow flow = entry.flow;
  final HttpRequest? request = entry.detail.request;
  final HttpRequest? edited = entry.detail.editedRequest;
  final HttpResponseHead? response = entry.detail.response;
  return <String, Object?>{
    'flow_id': flow.id.value,
    'session_id': flow.sessionId.value,
    'received_at': formatHistoryIso8601(flow.receivedAt),
    'method': flow.methodLabel,
    'scheme': flow.scheme.name,
    'host': flow.authority.host,
    'host_display': flow.authority.shownHost,
    'port': flow.authority.port,
    'path': flow.path,
    'state': flow.state.name,
    'decision': flow.decision?.name,
    'decision_source': flow.decisionSource?.name,
    'block_reason': flow.blockReason?.name,
    'rule_id': flow.ruleId?.value,
    'status': flow.status,
    'request_size': flow.requestSize,
    'response_size': flow.responseSize,
    'duration_ms': flow.duration?.inMilliseconds,
    'findings_count': flow.findingCount,
    'edited': flow.edited,
    'passthrough': flow.passthrough,
    'origin_tool': flow.originTool,
    'upstream_error': flow.upstreamError?.name,
    'request_headers': _headers(request?.headers),
    'edited_request_headers': edited == null ? null : _headers(edited.headers),
    'response_headers': _headers(response?.headers),
    'findings': <Object?>[
      for (final Finding finding in entry.detail.findings)
        <String, Object?>{
          'kind': finding.kind,
          'location': finding.location.name,
          'header_name': finding.headerName,
          'span_start': finding.spanStart,
          'span_end': finding.spanEnd,
          'tier': finding.tier.name,
          'display_prefix': finding.displayPrefix,
          'resolved': finding.resolved,
        },
    ],
    // The request as it arrived, and the request as it went out, are two
    // records with two bodies and two flags. One field carrying whichever
    // body happened to exist, beside the headers of the other, is a record
    // that describes nothing (`backlog/CONVENTIONS.md` 4.13).
    'request_body_b64': _body(entry.originalRequestBody),
    'request_body_truncated': request?.body.truncated ?? false,
    'edited_request_body_b64': _body(entry.editedRequestBody),
    'edited_request_body_truncated': edited?.body.truncated ?? false,
    'response_body_b64': _body(entry.responseBody),
    'response_body_truncated': entry.detail.responseBody?.truncated ?? false,
  };
}

List<Object?> _headers(List<Header>? headers) => <Object?>[
  for (final Header header in headers ?? const <Header>[])
    <String, Object?>{'name': header.name, 'value': header.text},
];

String _body(List<int>? bytes) =>
    bytes == null || bytes.isEmpty ? '' : base64.encode(bytes);
