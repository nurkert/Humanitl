/// HAR 1.2 for recorded flows.
///
/// The mapping is the one of `backlog/sprint-2.md`, HUM-032. Everything the
/// format has no field for goes into `_humanitl`, which is what the
/// specification's underscore prefix is for: a viewer ignores it, and the
/// decision, the reason and the rule are exactly what makes a Humanitl export
/// worth reading twice.
///
/// Reference: http://www.softwareishard.com/blog/har-12-spec/
library;

import 'dart:convert';

import '../../../core/domain/domain.dart';
import '../history_view.dart';
import 'export_entry.dart';

/// The version of the format this writes.
const String harVersion = '1.2';

/// The name the creator block carries.
const String harCreatorName = 'Humanitl';

/// What `content.comment` says where a body was not kept.
///
/// English and not from ARB: a HAR file is read by tools and by people who
/// did not necessarily export it, and the format's own language is English.
const String harBodyNotRecorded =
    'the body was not recorded; content.size is what the daemon counted';

/// The whole `log` object for [entries].
Map<String, Object?> harLog({
  required List<HistoryExportEntry> entries,
  required String creatorVersion,
}) => <String, Object?>{
  'log': <String, Object?>{
    'version': harVersion,
    'creator': <String, Object?>{
      'name': harCreatorName,
      'version': creatorVersion,
    },
    'entries': <Object?>[
      for (final HistoryExportEntry e in entries) harEntry(e),
    ],
  },
};

/// [entries] as a HAR document.
String encodeHar({
  required List<HistoryExportEntry> entries,
  required String creatorVersion,
}) =>
    const JsonEncoder.withIndent('  ')
        .convert(harLog(entries: entries, creatorVersion: creatorVersion));

/// One `log.entries` element.
Map<String, Object?> harEntry(HistoryExportEntry entry) {
  final Flow flow = entry.flow;
  final Duration? duration = flow.duration;
  return <String, Object?>{
    'startedDateTime': formatHistoryIso8601(flow.receivedAt),
    'time': duration?.inMilliseconds ?? 0,
    'request': _request(entry),
    'response': _response(entry),
    'cache': const <String, Object?>{},
    'timings': _timings(duration),
    '_humanitl': humanitlBlock(flow),
  };
}

/// The `_humanitl` block: what the format has no field for.
Map<String, Object?> humanitlBlock(Flow flow) => <String, Object?>{
  'flow_id': flow.id.value,
  'session_id': flow.sessionId.value,
  'decision': flow.decision?.name,
  'block_reason': flow.blockReason?.name,
  'rule_id': flow.ruleId?.value,
  'findings_count': flow.findingCount,
  'edited': flow.edited,
  'passthrough': flow.passthrough,
  // Next to `decision`, not inside it: a meta request went nowhere and
  // nobody decided about it, so `decision` is null on those rows (HUM-103).
  'meta': flow.meta,
};

Map<String, Object?> _request(HistoryExportEntry entry) {
  final Flow flow = entry.flow;
  final HttpRequest? request =
      entry.detail.editedRequest ?? entry.detail.request;
  final ExportedBytes body = ExportedBytes.of(entry.requestBody);
  final String pathAndQuery = request?.pathAndQuery ?? flow.path;
  final String url =
      '${flow.scheme.name}://${flow.authority.display(flow.scheme)}$pathAndQuery';
  final String contentType = request?.body.contentType ?? '';
  return <String, Object?>{
    'method': flow.methodLabel,
    'url': url,
    'httpVersion': request?.version ?? '',
    'cookies': const <Object?>[],
    'headers': _headers(request?.headers),
    'queryString': harQueryString(pathAndQuery),
    if (body.text.isNotEmpty)
      'postData': <String, Object?>{
        'mimeType': contentType,
        'text': body.text,
        if (body.encoding != null) 'encoding': body.encoding,
      },
    'headersSize': -1,
    // The size of the body this entry carries. `flow.requestSize` is the
    // one that arrived; where the person edited the request, what went out
    // is another body, and naming the old size beside the new bytes would
    // describe neither.
    'bodySize': entry.requestBody?.length ?? flow.requestSize,
  };
}

Map<String, Object?> _response(HistoryExportEntry entry) {
  final Flow flow = entry.flow;
  final HttpResponseHead? head = entry.detail.response;
  final ExportedBytes body = ExportedBytes.of(entry.responseBody);
  // A blocked flow never reached its target; the answer the agent saw is the
  // 403 the proxy wrote, and that is what the export shows.
  final bool blocked = flow.decision == DecisionKind.block;
  final int status = blocked ? 403 : (head?.status ?? flow.status);
  return <String, Object?>{
    'status': status,
    'statusText': '',
    'httpVersion': head?.version ?? '',
    'cookies': const <Object?>[],
    'headers': _headers(head?.headers),
    'content': <String, Object?>{
      'size': flow.responseSize,
      'mimeType': entry.detail.responseBody?.contentType ?? '',
      // No `text` where no bytes were recorded: an empty string beside a
      // size of nine hundred reads as an empty answer, which is a different
      // fact from "the body was not kept" (`backlog/CONVENTIONS.md` 4.13).
      // HAR has a field for saying so, and it is `comment`.
      if (body.text.isNotEmpty) 'text': body.text,
      if (body.text.isNotEmpty && body.encoding != null)
        'encoding': body.encoding,
      if (body.text.isEmpty && flow.responseSize > 0)
        'comment': harBodyNotRecorded,
    },
    'redirectURL': '',
    'headersSize': -1,
    'bodySize': flow.responseSize,
  };
}

/// The `timings` block.
///
/// The wire `FlowSummary` carries no hold time, so `wait` is zero and the
/// whole duration is charged to `receive`. Guessing a split would put a
/// number in a file that nobody measured (`backlog/CONVENTIONS.md` 4.13).
Map<String, Object?> _timings(Duration? duration) => <String, Object?>{
  'send': 0,
  'wait': 0,
  'receive': duration?.inMilliseconds ?? 0,
};

List<Object?> _headers(List<Header>? headers) => <Object?>[
  for (final Header header in headers ?? const <Header>[])
    <String, Object?>{'name': header.name, 'value': header.text},
];

/// The `queryString` array, parsed out of [pathAndQuery].
///
/// Percent decoding is best effort: a value that is not valid percent
/// encoding is kept as it stood, because the export is evidence and must not
/// silently change a byte a person is looking for.
List<Object?> harQueryString(String pathAndQuery) {
  final int mark = pathAndQuery.indexOf('?');
  if (mark < 0 || mark + 1 >= pathAndQuery.length) {
    return const <Object?>[];
  }
  final String query = pathAndQuery.substring(mark + 1);
  return <Object?>[
    for (final String pair in query.split('&'))
      if (pair.isNotEmpty)
        () {
          final int equals = pair.indexOf('=');
          final String name = equals < 0 ? pair : pair.substring(0, equals);
          final String value = equals < 0 ? '' : pair.substring(equals + 1);
          return <String, Object?>{
            'name': _decode(name),
            'value': _decode(value),
          };
        }(),
  ];
}

String _decode(String value) {
  try {
    return Uri.decodeQueryComponent(value);
  } on ArgumentError {
    return value;
  } on FormatException {
    return value;
  }
}
