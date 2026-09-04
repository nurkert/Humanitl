// Der Export von der Schaltfläche bis zur geschriebenen Datei: Umfang,
// Format, die Grenze bei 5000, der Fortschritt in Zeilen, der abgebrochene
// Dialog und der genannte Pfad.

import 'dart:convert';
import 'dart:typed_data';
import 'dart:io';
import 'dart:async';

import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
// `Override` lebt in riverpod 3 im Nebeneingang `misc.dart`.
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/history/export/export_entry.dart';
import 'package:humanitl/features/history/export/curl.dart';
import 'package:humanitl/features/history/export/har.dart';
import 'package:humanitl/features/history/export/history_export.dart';
import 'package:humanitl/features/history/export/jsonl.dart';
import 'package:humanitl/features/history/providers/history_detail.dart';
import 'package:humanitl/features/history/providers/history_export.dart';
import 'package:humanitl/features/history/providers/history_page.dart';

import 'fixtures.dart';
import 'harness.dart';

/// A target that writes nowhere and remembers what it was handed.
class RecordingTarget implements HistoryExportTarget {
  /// The result of the last save.
  HistoryExportResult? saved;

  /// The file name the dialog was offered.
  String? offeredName;

  /// What [save] answers; empty means the person dismissed the dialog.
  List<String> answer = <String>['/tmp/humanitl-export.har'];

  @override
  Future<List<String>> save(
    HistoryExportResult result, {
    required String fileName,
    required String dialogTitle,
  }) async {
    saved = result;
    offeredName = fileName;
    return answer;
  }
}

/// The encoder without the isolate hop.
Future<HistoryExportResult> _syncEncoder({
  required HistoryExportFormat format,
  required List<HistoryExportEntry> entries,
  required String creatorVersion,
}) async => encodeHistoryExport(
  format: format,
  entries: entries,
  creatorVersion: creatorVersion,
);

Future<ProviderContainer> _open(
  WidgetTester tester, {
  required FakeDaemonClient client,
  required RecordingTarget target,
}) async {
  final ProviderContainer container = await pumpHistory(
    tester,
    client: client,
    overrides: <Override>[
      historyExportTargetProvider.overrideWithValue(target),
      // `Isolate.run` needs real time; a widget test runs on a fake clock.
      historyExportEncoderProvider.overrideWithValue(_syncEncoder),
    ],
  );
  await tester.tap(find.byKey(const Key('history-export-open')));
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 300));
  return container;
}

Future<void> _save(WidgetTester tester, ProviderContainer container) async {
  await tester.tap(find.byKey(const Key('history-export-save')));
  for (int i = 0; i < 200; i++) {
    await tester.pump(const Duration(milliseconds: 16));
    if (!container.read(historyExportProvider).running) {
      break;
    }
  }
  await tester.pump(const Duration(milliseconds: 300));
}

/// A fake whose `ListFlows` can be stopped after a chosen page.
class GatedClient extends FakeDaemonClient {
  GatedClient({int count = 600}) : super(script: const <ScriptedEvent>[]) {
    seedHistory(state, count: count, start: historyEpoch);
  }

  /// From this call on, `ListFlows` waits.
  int gateFrom = -1;
  int _calls = 0;

  /// The call that is waiting, if one is.
  Completer<void>? waiting;

  /// Lets the pending call through.
  void release() {
    waiting?.complete();
    waiting = null;
    gateFrom = -1;
  }

  @override
  Future<FlowPage> listFlows(
    FlowFilter filter, {
    int limit = 200,
    String? cursor,
  }) async {
    if (gateFrom >= 0 && _calls++ >= gateFrom) {
      final Completer<void> gate = Completer<void>();
      waiting = gate;
      await gate.future;
    }
    return super.listFlows(filter, limit: limit, cursor: cursor);
  }
}

/// A fake that keeps handing back the same cursor.
class StuckCursorClient extends FakeDaemonClient {
  StuckCursorClient({int count = 600})
    : super(script: const <ScriptedEvent>[]) {
    seedHistory(state, count: count, start: historyEpoch);
  }

  /// How often `ListFlows` was called.
  int listCalls = 0;
  String? _stuck;

  @override
  Future<FlowPage> listFlows(
    FlowFilter filter, {
    int limit = 200,
    String? cursor,
  }) async {
    listCalls++;
    final FlowPage page = await super.listFlows(
      filter,
      limit: limit,
      cursor: cursor,
    );
    // A daemon that cannot go on but keeps naming the same cursor. It has to
    // be a cursor the daemon would accept, or the next call fails on reading
    // it instead of on standing still.
    _stuck ??= page.nextCursor;
    return page.copyWith(nextCursor: _stuck ?? '');
  }
}

/// A fake that keeps answering with the same rows under a new cursor.
class RepeatingClient extends FakeDaemonClient {
  RepeatingClient({int count = 600}) : super(script: const <ScriptedEvent>[]) {
    seedHistory(state, count: count, start: historyEpoch);
  }

  /// How often `ListFlows` was called.
  int listCalls = 0;

  @override
  Future<FlowPage> listFlows(
    FlowFilter filter, {
    int limit = 200,
    String? cursor,
  }) async {
    listCalls++;
    final FlowPage page = await super.listFlows(filter, limit: limit);
    // Always the first page, always a cursor nobody has seen before.
    return page.copyWith(nextCursor: 'fresh-$listCalls');
  }
}

void main() {
  testWidgets('the menu offers the filtered set and names the format', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final RecordingTarget target = RecordingTarget();
    final ProviderContainer container = await _open(
      tester,
      client: client,
      target: target,
    );
    final int visible = client.state.flows.values
        .where((Flow flow) => !flow.passthrough)
        .length;
    expect(find.text('Export requests'), findsOneWidget);
    expect(find.text('$visible matching requests'), findsOneWidget);
    expect(find.text('HAR'), findsOneWidget);
    expect(find.text('JSONL'), findsOneWidget);
    expect(find.text('CSV'), findsOneWidget);
    // curl covers exactly one flow, so it is not on offer for a whole set.
    expect(find.text('CURL'), findsNothing);
    expect(
      container.read(historyExportProvider).phase,
      HistoryExportPhase.idle,
    );
  });

  testWidgets('saving writes the filtered set and names the file', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final RecordingTarget target = RecordingTarget();
    final ProviderContainer container = await _open(
      tester,
      client: client,
      target: target,
    );
    await _save(tester, container);

    final HistoryExportState job = container.read(historyExportProvider);
    expect(job.phase, HistoryExportPhase.done);
    expect(job.written, <String>['/tmp/humanitl-export.har']);
    expect(target.saved, isNotNull);
    expect(target.saved!.format, HistoryExportFormat.har);
    expect(target.offeredName, endsWith('.har'));

    final int visible = client.state.flows.values
        .where((Flow flow) => !flow.passthrough)
        .length;
    expect(target.saved!.flowCount, visible);
    // The bytes really are a HAR document with as many entries.
    final Map<String, Object?> document =
        jsonDecode(utf8.decode(target.saved!.bytes)) as Map<String, Object?>;
    final Map<String, Object?> log = document['log']! as Map<String, Object?>;
    expect((log['entries']! as List<Object?>), hasLength(visible));
    // And the path is on screen, so nobody has to guess where it went.
    expect(find.textContaining('/tmp/humanitl-export.har'), findsOneWidget);
  });

  testWidgets('a dismissed dialog says that nothing was written', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 12);
    final RecordingTarget target = RecordingTarget()..answer = <String>[];
    final ProviderContainer container = await _open(
      tester,
      client: client,
      target: target,
    );
    await _save(tester, container);

    expect(
      container.read(historyExportProvider).phase,
      HistoryExportPhase.cancelled,
    );
    expect(
      find.text('The dialog was closed. No file was written.'),
      findsOneWidget,
    );
  });

  testWidgets('the selected request can be exported on its own, as curl', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final RecordingTarget target = RecordingTarget()
      ..answer = <String>['/tmp/one.sh', '/tmp/request.body'];
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
      overrides: <Override>[
        historyExportTargetProvider.overrideWithValue(target),
        historyExportEncoderProvider.overrideWithValue(_syncEncoder),
      ],
    );
    final Flow post = container
        .read(historyPageProvider)
        .rows
        .firstWhere((Flow flow) => flow.method == Method.post);
    container.read(historySelectionProvider.notifier).select(post.id);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 600));

    await tester.tap(find.byKey(const Key('history-export-open')));
    await tester.pump();
    expect(find.text('The selected request'), findsOneWidget);
    await tester.tap(find.text('CURL'));
    await tester.pump();
    await _save(tester, container);

    expect(target.saved!.format, HistoryExportFormat.curl);
    expect(target.saved!.flowCount, 1);
    expect(utf8.decode(target.saved!.bytes), startsWith('curl -X POST '));
    expect(target.offeredName, endsWith('.sh'));
    // Both files are named, so nobody moves one without the other.
    expect(find.textContaining('/tmp/request.body'), findsOneWidget);
  });

  testWidgets('a set larger than the cap says so before anything runs', (
    WidgetTester tester,
  ) async {
    // Every twelfth flow is passthrough and the default query leaves it out,
    // so the set has to be larger than the cap by more than that.
    final FakeDaemonClient client = FakeDaemonClient.history(
      count: historyExportMaxFlows + 1500,
    );
    final RecordingTarget target = RecordingTarget();
    await _open(tester, client: client, target: target);
    expect(
      find.textContaining('An export covers at most 5,000 requests'),
      findsOneWidget,
    );
  });

  testWidgets('the export covers more than the window the table holds', (
    WidgetTester tester,
  ) async {
    // The screen keeps 2000 rows; an export of the filtered set pages the
    // daemon itself and is not limited to what is on screen.
    final FakeDaemonClient client = FakeDaemonClient.history(count: 2400);
    final RecordingTarget target = RecordingTarget();
    final ProviderContainer container = await _open(
      tester,
      client: client,
      target: target,
    );
    await _save(tester, container);
    final int visible = client.state.flows.values
        .where((Flow flow) => !flow.passthrough)
        .length;
    expect(
      container.read(historyPageProvider).rows.length,
      lessThanOrEqualTo(historyMaxRows),
    );
    expect(target.saved!.flowCount, visible);
    expect(visible, greaterThan(historyMaxRows));
  });

  testWidgets('the modal says what the file will carry, before it is written', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 12);
    final RecordingTarget target = RecordingTarget();
    await _open(tester, client: client, target: target);
    // Whoever passes the file on has to know beforehand; there is no
    // redaction yet (`docs/SECURITY.md`, Aufzeichnung).
    final Finder warning = find.byKey(const Key('history-export-contents'));
    expect(warning, findsOneWidget);
    final String text = tester.widget<Text>(warning).data!;
    for (final String named in <String>['headers', 'bodies', 'clear text']) {
      expect(text, contains(named), reason: named);
    }
    expect(target.saved, isNull, reason: 'nothing was written yet');
  });

  testWidgets('the progress counts against the filtered set, not itself', (
    WidgetTester tester,
  ) async {
    final GatedClient client = GatedClient(count: 600);
    final RecordingTarget target = RecordingTarget();
    final ProviderContainer container = await _open(
      tester,
      client: client,
      target: target,
    );
    final int matched = container.read(historyPageProvider).total;
    expect(matched, greaterThan(historyPageSize));

    client.gateFrom = 1;
    await tester.tap(find.byKey(const Key('history-export-save')));
    for (int i = 0; i < 60; i++) {
      await tester.pump(const Duration(milliseconds: 16));
      if (client.waiting != null) {
        break;
      }
    }
    // The first page is in, the rest is not: the total is what the filter
    // matches, never the number already collected (`docs/UX.md` 2.11).
    final HistoryExportState job = container.read(historyExportProvider);
    expect(job.done, historyPageSize);
    expect(job.total, matched);
    expect(job.total, greaterThan(job.done));

    client.release();
    await _save(tester, container);
    expect(
      container.read(historyExportProvider).phase,
      HistoryExportPhase.done,
    );
  });

  testWidgets('a cursor that stops moving ends the paging', (
    WidgetTester tester,
  ) async {
    final StuckCursorClient client = StuckCursorClient(count: 600);
    final RecordingTarget target = RecordingTarget();
    final ProviderContainer container = await _open(
      tester,
      client: client,
      target: target,
    );
    await _save(tester, container);
    // Without the guard the loop would page for ever on the same cursor.
    expect(
      container.read(historyExportProvider).phase,
      HistoryExportPhase.done,
    );
    expect(client.listCalls, lessThan(10));
  });

  testWidgets(
    'the second file of a curl export is named before it is written',
    (WidgetTester tester) async {
      final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
      final RecordingTarget target = RecordingTarget();
      final ProviderContainer container = await pumpHistory(
        tester,
        client: client,
        overrides: <Override>[
          historyExportTargetProvider.overrideWithValue(target),
          historyExportEncoderProvider.overrideWithValue(_syncEncoder),
        ],
      );
      container
          .read(historySelectionProvider.notifier)
          .select(container.read(historyPageProvider).rows.first.id);
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 600));
      await tester.tap(find.byKey(const Key('history-export-open')));
      await tester.pump();

      // No second file is announced while a single-file format is chosen.
      expect(find.byKey(const Key('history-export-beside')), findsNothing);

      await tester.tap(find.text('CURL'));
      await tester.pump();
      final Finder beside = find.byKey(const Key('history-export-beside'));
      expect(beside, findsOneWidget);
      expect(tester.widget<Text>(beside).data, contains(curlBodyFileName));
      expect(target.saved, isNull, reason: 'said before, not after');
    },
  );

  test('a body file never overwrites one that is already there', () async {
    final Directory dir = await Directory.systemTemp.createTemp('humanitl');
    addTearDown(() => dir.delete(recursive: true));
    // Something of somebody else's, under the name the export wants.
    final File taken = File('${dir.path}/$curlBodyFileName');
    await taken.writeAsString('not mine to lose');

    final HistoryExportResult result = encodeHistoryExport(
      format: HistoryExportFormat.curl,
      entries: <HistoryExportEntry>[
        testEntry(testFlow(id: 'curl-one'), requestBody: '{"a":1}'),
      ],
      creatorVersion: '0.0.0-test',
    );
    final List<String> written = await const FilePickerExportTarget()
        .writeBeside(result, '${dir.path}/command.sh');

    expect(written, hasLength(2));
    expect(written.first, '${dir.path}/command.sh');
    expect(written.last, isNot(taken.path));
    expect(await taken.readAsString(), 'not mine to lose');
    expect(await File(written.last).readAsString(), '{"a":1}');
  });

  test(
    'an empty set says nothing matched, not that a dialog was closed',
    () async {
      final FakeDaemonClient client = FakeDaemonClient.history(count: 12);
      final RecordingTarget target = RecordingTarget();
      final ProviderContainer container = ProviderContainer(
        overrides: <Override>[
          daemonClientProvider.overrideWithValue(client),
          historyExportTargetProvider.overrideWithValue(target),
          historyExportEncoderProvider.overrideWithValue(_syncEncoder),
        ],
      );
      addTearDown(container.dispose);
      // Nothing is selected, so the scope of one row is empty.
      await container
          .read(historyExportProvider.notifier)
          .run(
            format: HistoryExportFormat.har,
            scope: HistoryExportScope.selected,
            fileName: 'x.har',
            dialogTitle: 'x',
          );
      expect(
        container.read(historyExportProvider).phase,
        HistoryExportPhase.empty,
      );
      expect(target.saved, isNull);
    },
  );

  test('a page that adds no row ends the paging', () async {
    final RepeatingClient client = RepeatingClient(count: 600);
    final RecordingTarget target = RecordingTarget();
    final ProviderContainer container = ProviderContainer(
      overrides: <Override>[
        daemonClientProvider.overrideWithValue(client),
        historyExportTargetProvider.overrideWithValue(target),
        historyExportEncoderProvider.overrideWithValue(_syncEncoder),
      ],
    );
    addTearDown(container.dispose);
    await container
        .read(historyExportProvider.notifier)
        .run(
          format: HistoryExportFormat.jsonl,
          scope: HistoryExportScope.filtered,
          fileName: 'x.jsonl',
          dialogTitle: 'x',
        );
    // A daemon that names a fresh cursor every time but repeats its rows
    // would turn for ever without the second guard.
    expect(
      container.read(historyExportProvider).phase,
      HistoryExportPhase.done,
    );
    expect(client.listCalls, lessThan(10));
  });

  test('an edited request keeps both bodies apart in JSONL and HAR', () {
    final Uint8List original = Uint8List.fromList(utf8.encode('{"was":1}'));
    final Uint8List edited = Uint8List.fromList(
      utf8.encode('{"went":2,"out":true}'),
    );
    final Flow flow = testFlow(id: 'edited-one', edited: true);
    final FlowDetail base = testDetail(flow);
    final HistoryExportEntry entry = HistoryExportEntry(
      detail: base.copyWith(editedRequest: base.request),
      requestBody: edited,
      originalBody: original,
      responseBody: null,
    );

    final Map<String, Object?> record = jsonlRecord(entry);
    expect(
      utf8.decode(base64.decode(record['request_body_b64']! as String)),
      '{"was":1}',
    );
    expect(
      utf8.decode(base64.decode(record['edited_request_body_b64']! as String)),
      '{"went":2,"out":true}',
    );

    // HAR carries the request that went out, and its size, not the old one.
    final Map<String, Object?> request =
        harEntry(entry)['request']! as Map<String, Object?>;
    expect(
      (request['postData']! as Map<String, Object?>)['text'],
      '{"went":2,"out":true}',
    );
    expect(request['bodySize'], edited.length);
    expect(request['bodySize'], isNot(flow.requestSize));
  });

  test('a response HAR carries no text where no body was recorded', () {
    final HistoryExportEntry entry = HistoryExportEntry(
      detail: FlowDetail(
        summary: testFlow(id: 'no-body', responseSize: 900),
        responseBody: const BodyRef(
          sha256: <int>[],
          size: 900,
          contentType: 'application/json',
        ),
      ),
    );
    final Map<String, Object?> content =
        (harEntry(entry)['response']! as Map<String, Object?>)['content']!
            as Map<String, Object?>;
    // An empty string beside a size of 900 would read as an empty answer.
    expect(content.containsKey('text'), isFalse);
    expect(content['size'], 900);
    expect(content['comment'], harBodyNotRecorded);
  });

  test('a response HAR carries the text where a body was recorded', () {
    final HistoryExportEntry entry = testEntry(
      testFlow(id: 'with-body'),
      responseBody: '{"data":{"viewer":1}}',
    );
    final Map<String, Object?> content =
        (harEntry(entry)['response']! as Map<String, Object?>)['content']!
            as Map<String, Object?>;
    expect(content['text'], '{"data":{"viewer":1}}');
    expect(content.containsKey('comment'), isFalse);
  });

  test(
    'the isolate encoder produces the same bytes as the direct one',
    () async {
      final FakeDaemonClient client = FakeDaemonClient.history(count: 3);
      final List<HistoryExportEntry> entries = <HistoryExportEntry>[
        for (final Flow flow in client.state.flows.values)
          HistoryExportEntry(detail: await client.getFlow(flow.id)),
      ];
      // The production path really runs; a plain test has real time for it.
      final HistoryExportResult onIsolate = await isolateHistoryExportEncoder(
        format: HistoryExportFormat.jsonl,
        entries: entries,
        creatorVersion: '0.0.0-test',
      );
      final HistoryExportResult direct = encodeHistoryExport(
        format: HistoryExportFormat.jsonl,
        entries: entries,
        creatorVersion: '0.0.0-test',
      );
      expect(onIsolate.bytes, direct.bytes);
      expect(onIsolate.flowCount, entries.length);
    },
  );

  test('a JSONL export of the filtered set reads back line by line', () async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 8);
    final RecordingTarget target = RecordingTarget();
    final ProviderContainer container = ProviderContainer(
      overrides: <Override>[
        daemonClientProvider.overrideWithValue(client),
        historyExportTargetProvider.overrideWithValue(target),
        historyExportEncoderProvider.overrideWithValue(_syncEncoder),
      ],
    );
    addTearDown(container.dispose);
    container.listen(
      historyPageProvider,
      (HistoryPageState? previous, HistoryPageState next) {},
    );
    for (int i = 0; i < 100; i++) {
      if (!container.read(historyPageProvider).loading) {
        break;
      }
      await Future<void>.delayed(Duration.zero);
    }
    await container
        .read(historyExportProvider.notifier)
        .run(
          format: HistoryExportFormat.jsonl,
          scope: HistoryExportScope.filtered,
          fileName: 'x.jsonl',
          dialogTitle: 'x',
        );
    final List<Map<String, Object?>> back = decodeJsonl(
      utf8.decode(target.saved!.bytes),
    );
    expect(back, hasLength(target.saved!.flowCount));
    expect(back.first['flow_id'], isA<String>());
  });
}
