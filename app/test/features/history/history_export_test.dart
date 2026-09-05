// Die Encoder des Exports: HAR 1.2 gegen die Pflichtfelder der Spezifikation,
// JSON Lines im Hin und Zurück, CSV nach RFC 4180 und curl als Kommandozeile.

import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/features/history/export/csv.dart';
import 'package:humanitl/features/history/export/curl.dart';
import 'package:humanitl/features/history/export/export_entry.dart';
import 'package:humanitl/features/history/export/har.dart';
import 'package:humanitl/features/history/export/history_export.dart';
import 'package:humanitl/features/history/export/jsonl.dart';

import 'fixtures.dart';

/// The three flows of the acceptance criterion: an allow with a body, a
/// block and a timeout.
List<HistoryExportEntry> _threeFlows() => <HistoryExportEntry>[
  testEntry(
    testFlow(
      id: '018f0004-0000-7000-8000-000000000001',
      decision: DecisionKind.allow,
      source: DecisionSource.user,
      path: '/graphql?first=10&q=a%20b',
    ),
  ),
  testEntry(
    testFlow(
      id: '018f0004-0000-7000-8000-000000000002',
      host: 'models.dev',
      path: '/api.json',
      method: Method.get,
      decision: DecisionKind.block,
      source: DecisionSource.rule,
      blockReason: BlockReason.rule,
      ruleId: testRule,
      status: 403,
      responseSize: 96,
    ),
    requestBody: '',
    responseBody: '{"error":"blocked"}',
  ),
  testEntry(
    testFlow(
      id: '018f0004-0000-7000-8000-000000000003',
      host: 'slow.example.org',
      path: '/wait',
      method: Method.get,
      decision: DecisionKind.timedOut,
      source: DecisionSource.timeout,
      blockReason: BlockReason.timeout,
      status: 504,
      responseSize: 0,
      duration: null,
    ),
    requestBody: '',
    responseBody: '',
  ),
];

/// The fields HAR 1.2 declares required for a log, an entry, a request and a
/// response. Written out rather than loaded from a schema file, because the
/// repository carries no `har-1.2.json` and a schema nobody can see is not a
/// check.
void _expectValidHar(Map<String, Object?> document, int entryCount) {
  final Map<String, Object?> log = document['log']! as Map<String, Object?>;
  expect(log['version'], '1.2');
  final Map<String, Object?> creator = log['creator']! as Map<String, Object?>;
  expect(creator['name'], isA<String>());
  expect(creator['version'], isA<String>());
  final List<Object?> entries = log['entries']! as List<Object?>;
  expect(entries, hasLength(entryCount));
  for (final Object? raw in entries) {
    final Map<String, Object?> entry = raw! as Map<String, Object?>;
    expect(entry['startedDateTime'], isA<String>());
    expect(
      DateTime.tryParse(entry['startedDateTime']! as String),
      isNotNull,
      reason: 'startedDateTime is ISO 8601',
    );
    expect(entry['time'], isA<int>());
    expect(entry['cache'], isA<Map<Object?, Object?>>());

    final Map<String, Object?> request =
        entry['request']! as Map<String, Object?>;
    for (final String field in <String>[
      'method',
      'url',
      'httpVersion',
      'cookies',
      'headers',
      'queryString',
      'headersSize',
      'bodySize',
    ]) {
      expect(request[field], isNotNull, reason: 'request.$field');
    }
    expect(Uri.tryParse(request['url']! as String), isNotNull);
    for (final Object? header in request['headers']! as List<Object?>) {
      final Map<String, Object?> pair = header! as Map<String, Object?>;
      expect(pair['name'], isA<String>());
      expect(pair['value'], isA<String>());
    }

    final Map<String, Object?> response =
        entry['response']! as Map<String, Object?>;
    for (final String field in <String>[
      'status',
      'statusText',
      'httpVersion',
      'cookies',
      'headers',
      'content',
      'redirectURL',
      'headersSize',
      'bodySize',
    ]) {
      expect(response[field], isNotNull, reason: 'response.$field');
    }
    final Map<String, Object?> content =
        response['content']! as Map<String, Object?>;
    expect(content['size'], isA<int>());
    expect(content['mimeType'], isA<String>());
    if (content['text'] != null) {
      expect(content['text'], isA<String>());
    }

    final Map<String, Object?> timings =
        entry['timings']! as Map<String, Object?>;
    for (final String field in <String>['send', 'wait', 'receive']) {
      expect(timings[field], isA<int>());
    }
  }
}

void main() {
  group('har_export_valid', () {
    test('three flows produce a valid HAR 1.2 log with _humanitl', () {
      final List<HistoryExportEntry> entries = _threeFlows();
      final String text = encodeHar(
        entries: entries,
        creatorVersion: '0.0.0-test',
      );
      final Map<String, Object?> document =
          jsonDecode(text) as Map<String, Object?>;
      _expectValidHar(document, 3);

      final List<Object?> logEntries =
          (document['log']! as Map<String, Object?>)['entries']!
              as List<Object?>;
      for (final Object? raw in logEntries) {
        final Map<String, Object?> entry = raw! as Map<String, Object?>;
        final Map<String, Object?> humanitl =
            entry['_humanitl']! as Map<String, Object?>;
        for (final String field in <String>[
          'flow_id',
          'session_id',
          'decision',
          'block_reason',
          'rule_id',
          'findings_count',
          'edited',
          'passthrough',
        ]) {
          expect(humanitl.containsKey(field), isTrue, reason: field);
        }
      }
    });

    test('a blocked flow answers 403, whatever the recorder stored', () {
      final Map<String, Object?> entry = harEntry(_threeFlows()[1]);
      expect((entry['response']! as Map<String, Object?>)['status'], 403);
      expect(
        (entry['_humanitl']! as Map<String, Object?>)['decision'],
        'block',
      );
    });

    test('the query string is parsed out of the path and decoded', () {
      expect(harQueryString('/graphql?first=10&q=a%20b'), <Object?>[
        <String, Object?>{'name': 'first', 'value': '10'},
        <String, Object?>{'name': 'q', 'value': 'a b'},
      ]);
      expect(harQueryString('/graphql'), isEmpty);
      expect(harQueryString('/graphql?'), isEmpty);
    });

    test('binary content is base64 and says so', () {
      final ExportedBytes payload = ExportedBytes.of(
        Uint8List.fromList(<int>[0xff, 0xfe, 0x00, 0x01]),
      );
      expect(payload.base64Encoded, isTrue);
      expect(payload.encoding, 'base64');
      expect(base64.decode(payload.text), <int>[0xff, 0xfe, 0x00, 0x01]);
    });
  });

  group('jsonl_roundtrip', () {
    test('every line reads back as the object that was written', () {
      final List<HistoryExportEntry> entries = _threeFlows();
      final String text = encodeJsonl(entries);
      final List<Map<String, Object?>> back = decodeJsonl(text);
      expect(back, hasLength(entries.length));
      for (int i = 0; i < entries.length; i++) {
        expect(back[i], jsonlRecord(entries[i]));
        expect(back[i]['flow_id'], entries[i].flow.id.value);
      }
    });

    test('a body travels base64 with its truncation flag', () {
      final Map<String, Object?> record = decodeJsonl(
        encodeJsonl(<HistoryExportEntry>[_threeFlows().first]),
      ).single;
      expect(
        utf8.decode(base64.decode(record['request_body_b64']! as String)),
        '{"query":"{ viewer }"}',
      );
      expect(record['request_body_truncated'], isFalse);
      expect(record['response_body_truncated'], isFalse);
    });

    test('a file ends with a newline, so it can be concatenated', () {
      expect(encodeJsonl(_threeFlows()), endsWith('\n'));
    });
  });

  group('csv', () {
    test('the header row names every column and every row has as many', () {
      final List<String> lines = encodeCsv(_threeFlows()).split('\r\n')
        ..removeWhere((String line) => line.isEmpty);
      expect(lines.first.split(','), csvColumns);
      for (final String line in lines.skip(1)) {
        expect(line.split(',').length, csvColumns.length);
      }
    });

    test('a field with a comma or a quote is quoted the RFC 4180 way', () {
      expect(csvField('plain'), 'plain');
      expect(csvField('a,b'), '"a,b"');
      expect(csvField('say "hi"'), '"say ""hi"""');
      expect(csvField('two\nlines'), '"two\nlines"');
    });

    test('an unfinished value is empty, never a zero', () {
      final List<String> row = csvRow(
        testFlow(id: 'x', status: 0, duration: null),
      );
      expect(row[csvColumns.indexOf('status')], '');
      expect(row[csvColumns.indexOf('duration_ms')], '');
    });
  });

  group('curl', () {
    test('one flow becomes a command with its headers and a body file', () {
      final String command = encodeCurl(_threeFlows().first);
      expect(command, startsWith("curl -X POST 'https://api.github.com/"));
      expect(command, contains("-H 'content-type: application/json'"));
      expect(command, contains('--data-binary @'));
      expect(command, contains(curlBodyFileName));
    });

    test('a request without a body reads no file', () {
      expect(encodeCurl(_threeFlows()[1]), isNot(contains('--data-binary')));
    });

    test('a single quote in a value cannot break out of the quoting', () {
      expect(shellQuote("it's"), r"'it'\''s'");
    });
  });

  group('the export seam', () {
    test('every format encodes to UTF-8 bytes and counts its flows', () {
      final List<HistoryExportEntry> entries = _threeFlows();
      for (final HistoryExportFormat format in <HistoryExportFormat>[
        HistoryExportFormat.har,
        HistoryExportFormat.jsonl,
        HistoryExportFormat.csv,
      ]) {
        final HistoryExportResult result = encodeHistoryExport(
          format: format,
          entries: entries,
          creatorVersion: '0.0.0-test',
        );
        expect(result.flowCount, 3);
        expect(result.format, format);
        expect(utf8.decode(result.bytes), isNotEmpty);
        expect(format.fileExtension, isNotEmpty);
      }
    });

    test('curl refuses more than one flow instead of exporting the first', () {
      expect(
        () => encodeHistoryExport(
          format: HistoryExportFormat.curl,
          entries: _threeFlows(),
          creatorVersion: '0.0.0-test',
        ),
        throwsArgumentError,
      );
      final HistoryExportResult single = encodeHistoryExport(
        format: HistoryExportFormat.curl,
        entries: <HistoryExportEntry>[_threeFlows().first],
        creatorVersion: '0.0.0-test',
      );
      expect(single.bodyFile, isNotNull);
    });
  });

  // HUM-120: Der Daemon vermerkt einen abgeschnittenen Antwort-Rumpf als
  // gekürzt. Bis dieses Issue kam das in HAR und CSV nicht an: Beide gaben
  // die halbe Antwort als ganze aus. JSON Lines trug die Marke schon.
  group('truncated response bodies', () {
    /// Ein Fluss, dessen Antwort mitten im Strom abgeschnitten wurde.
    HistoryExportEntry cutEntry() {
      final Flow flow = testFlow(
        id: '018f0004-0000-7000-8000-0000000000c0',
        host: 'ollama.internal',
        path: '/api/chat',
        status: 200,
        responseSize: 11,
      );
      final FlowDetail detail = testDetail(flow);
      return HistoryExportEntry(
        detail: detail.copyWith(
          responseBody: BodyRef(
            sha256: List<int>.filled(32, 9),
            size: 11,
            truncated: true,
            contentType: 'text/event-stream',
          ),
        ),
        requestBody: Uint8List.fromList(utf8.encode('{}')),
        responseBody: Uint8List.fromList(utf8.encode('data: {"i":0')),
      );
    }

    test('har says in the comment and in _humanitl that the answer was cut', () {
      final Map<String, Object?> entry = harEntry(cutEntry());
      final Map<String, Object?> response =
          entry['response']! as Map<String, Object?>;
      final Map<String, Object?> content =
          response['content']! as Map<String, Object?>;
      // Die Bytes sind da, und trotzdem steht die Marke daneben: Genau das
      // fehlte, denn `text` allein sieht aus wie eine vollständige Antwort.
      expect(content['text'], isNotEmpty);
      expect(content['comment'], harBodyTruncated);
      final Map<String, Object?> block =
          entry['_humanitl']! as Map<String, Object?>;
      expect(block['response_body_truncated'], isTrue);
    });

    test('har leaves a complete answer unmarked', () {
      final Map<String, Object?> entry = harEntry(_threeFlows().first);
      final Map<String, Object?> response =
          entry['response']! as Map<String, Object?>;
      final Map<String, Object?> content =
          response['content']! as Map<String, Object?>;
      expect(content.containsKey('comment'), isFalse);
      final Map<String, Object?> block =
          entry['_humanitl']! as Map<String, Object?>;
      expect(block['response_body_truncated'], isFalse);
    });

    test('har keeps both notes apart where a body was not kept at all', () {
      // Kein Rumpf aufgezeichnet und zugleich abgeschnitten: zwei
      // verschiedene Tatsachen, und beide stehen im Kommentar.
      final HistoryExportEntry cut = cutEntry();
      final Map<String, Object?> entry = harEntry(
        HistoryExportEntry(detail: cut.detail, requestBody: cut.requestBody),
      );
      final Map<String, Object?> response =
          entry['response']! as Map<String, Object?>;
      final Map<String, Object?> content =
          response['content']! as Map<String, Object?>;
      expect(content['comment'], contains(harBodyTruncated));
      expect(content['comment'], contains(harBodyNotRecorded));
    });

    test('csv carries the mark in its own column', () {
      final String csv = encodeCsv(<HistoryExportEntry>[cutEntry()]);
      final List<String> lines = csv.split('\r\n')
        ..removeWhere((String line) => line.isEmpty);
      final int column = csvColumns.indexOf('response_body_truncated');
      expect(column, isNonNegative, reason: 'the column has to exist');
      expect(lines.first.split(',')[column], 'response_body_truncated');
      expect(lines[1].split(',')[column], 'true');
    });

    test('csv says false for an answer nothing marked', () {
      final String csv = encodeCsv(_threeFlows());
      final List<String> lines = csv.split('\r\n')
        ..removeWhere((String line) => line.isEmpty);
      final int column = csvColumns.indexOf('response_body_truncated');
      for (final String line in lines.skip(1)) {
        expect(line.split(',')[column], 'false');
      }
    });
  });
}
