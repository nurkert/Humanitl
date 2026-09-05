/// CSV for recorded flows: the columns of the table, one row per flow.
///
/// RFC 4180: comma separated, CRLF line ends, every field quoted that carries
/// a comma, a quote or a line break, and an inner quote doubled. Bodies are
/// not in it — a spreadsheet is the wrong place for eight mebibytes; HAR and
/// JSON Lines carry them.
library;

import '../../../core/domain/domain.dart';
import '../history_view.dart';
import 'export_entry.dart';

/// The header row, in the order of the columns.
const List<String> csvColumns = <String>[
  'received_at',
  'flow_id',
  'session_id',
  'state',
  'method',
  'scheme',
  'host',
  'port',
  'path',
  'status',
  'request_size',
  'response_size',
  'duration_ms',
  'findings_count',
  'decision',
  'decision_source',
  'block_reason',
  'rule_id',
  'edited',
  'passthrough',
  // Next to `decision`, not inside it: nobody decided about a request the
  // proxy answered itself, so `decision` is empty on those rows (HUM-103).
  'meta',
  'origin_tool',
];

/// [entries] as CSV, header row first.
String encodeCsv(List<HistoryExportEntry> entries) {
  final StringBuffer out = StringBuffer()
    ..write(csvColumns.map(csvField).join(','))
    ..write('\r\n');
  for (final HistoryExportEntry entry in entries) {
    out
      ..write(csvRow(entry.flow).map(csvField).join(','))
      ..write('\r\n');
  }
  return out.toString();
}

/// One flow as the values of [csvColumns].
List<String> csvRow(Flow flow) => <String>[
  formatHistoryIso8601(flow.receivedAt),
  flow.id.value,
  flow.sessionId.value,
  flow.state.name,
  flow.methodLabel,
  flow.scheme.name,
  flow.authority.shownHost,
  '${flow.authority.port}',
  flow.path,
  flow.status == 0 ? '' : '${flow.status}',
  '${flow.requestSize}',
  '${flow.responseSize}',
  flow.duration == null ? '' : '${flow.duration!.inMilliseconds}',
  '${flow.findingCount}',
  flow.decision?.name ?? '',
  flow.decisionSource?.name ?? '',
  flow.blockReason?.name ?? '',
  flow.ruleId?.value ?? '',
  '${flow.edited}',
  '${flow.passthrough}',
  '${flow.meta}',
  flow.originTool,
];

/// [value] quoted where RFC 4180 asks for it.
String csvField(String value) {
  final bool needsQuotes =
      value.contains(',') ||
      value.contains('"') ||
      value.contains('\n') ||
      value.contains('\r');
  if (!needsQuotes) {
    return value;
  }
  return '"${value.replaceAll('"', '""')}"';
}
