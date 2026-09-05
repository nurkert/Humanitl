// Meta-Anfragen in der Historie (HUM-103): der Vermerk, das Wort, der Filter.
//
// Eine Anfrage an `humanitl.internal` beantwortet der Proxy selbst. Sie steht
// in der Historie, sie trägt keine Entscheidung, und keine Zählung über
// Entscheidungen sieht sie. Diese Datei prüft, dass die Oberfläche genau das
// zeigt — und dass der Fake dieselbe Sprache spricht wie der Daemon.

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ipc/convert.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ipc/generated/humanitl/v1/humanitl.pb.dart' as pb;
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/history/export/csv.dart';
import 'package:humanitl/features/history/export/har.dart';
import 'package:humanitl/features/history/export/jsonl.dart';
import 'package:humanitl/features/history/history_view.dart';
import 'package:humanitl/features/history/providers/history_query.dart';

import 'fixtures.dart';

/// Eine Anfrage an den Meta-Endpunkt, wie der Daemon sie aufzeichnet: ein
/// Status, den der Proxy selbst geschrieben hat, und keine Entscheidung.
Flow metaFlow({String id = 'm1', String path = '/', int status = 200}) =>
    testFlow(
      id: id,
      host: 'humanitl.internal',
      path: path,
      method: Method.get,
      status: status,
      meta: true,
      responseSize: 0,
    );

void main() {
  group('a meta flow is not a decision', () {
    test('it is never drawn as held', () {
      final Flow flow = metaFlow();
      expect(flow.decision, isNull);
      // Ohne die Sonderbehandlung landete es über `FlowVisualState` bei
      // `held` — und das behauptete, jemand sei gerade dabei zu entscheiden.
      expect(historyVisualState(flow), HFlowState.passthroughLlm);
      expect(historyVisualState(flow), isNot(HFlowState.held));
    });

    test('and never in one of the four decision hues', () {
      const Set<HFlowState> decisions = <HFlowState>{
        HFlowState.allowed,
        HFlowState.allowedEdited,
        HFlowState.blocked,
        HFlowState.autoRule,
        HFlowState.timedOut,
      };
      expect(decisions.contains(historyVisualState(metaFlow())), isFalse);
    });

    test('the rule column says endpoint, not waiting', () {
      expect(historyDecider(metaFlow()), HistoryDecider.meta);
      expect(historyDecider(metaFlow()), isNot(HistoryDecider.pending));
      // Und eine gewöhnliche, noch unentschiedene Zeile bleibt `pending`.
      expect(
        historyDecider(
          testFlow(id: 'h', state: FlowState.held, status: 0, duration: null),
        ),
        HistoryDecider.pending,
      );
    });
  });

  group('the chip writes the term', () {
    test('meta:true, spelled out in the field like the other terms', () {
      expect(HistoryChip.meta.term, 'meta:true');
      const HistoryQuery empty = HistoryQuery();
      final HistoryQuery on = empty.toggle(HistoryChip.meta);
      expect(on.filter, 'meta:true');
      expect(on.has(HistoryChip.meta), isTrue);
      expect(on.toggle(HistoryChip.meta).filter, isEmpty);
    });

    test('every chip has a term or is the passthrough flag', () {
      for (final HistoryChip chip in HistoryChip.values) {
        expect(
          chip == HistoryChip.passthrough
              ? chip.term == null
              : chip.term != null,
          isTrue,
          reason: chip.name,
        );
      }
    });
  });

  group('the fake filter splits the history the way the recorder does', () {
    final DateTime now = DateTime.utc(2026, 9, 5, 12);
    final List<Flow> rows = <Flow>[
      metaFlow(id: 'm1'),
      metaFlow(id: 'm2', path: '/why/018f0004-0000-7000-8000-000000000009'),
      metaFlow(id: 'm3', path: '/ask', status: 202),
      testFlow(
        id: 'a',
        decision: DecisionKind.allow,
        source: DecisionSource.user,
      ),
      testFlow(
        id: 'b',
        decision: DecisionKind.block,
        source: DecisionSource.user,
        blockReason: BlockReason.user,
        status: 403,
      ),
    ];

    List<String> pass(String filter) => rows
        .where(FakeFlowFilter.parse(filter, now).matches)
        .map((Flow flow) => flow.id.value)
        .toList();

    test('meta:true is exactly the three, meta:false exactly the rest', () {
      expect(pass('meta:true'), <String>['m1', 'm2', 'm3']);
      expect(pass('meta:false'), <String>['a', 'b']);
      expect(pass(''), rows.map((Flow flow) => flow.id.value).toList());
    });

    test('no count over decisions sees one', () {
      expect(pass('decision:allow'), <String>['a']);
      expect(pass('decision:block'), <String>['b']);
      expect(pass('meta:true decision:allow'), isEmpty);
    });

    test('a value that is neither true nor false is RECORDER_002', () {
      expect(
        () => FakeFlowFilter.parse('meta:maybe', now),
        throwsA(
          isA<DaemonException>().having(
            (DaemonException error) => error.diagnostic.code,
            'code',
            'RECORDER_002',
          ),
        ),
      );
      expect(
        () => FakeFlowFilter.parse('meta:>0', now),
        throwsA(isA<DaemonException>()),
      );
    });
  });

  group('the exports carry the mark', () {
    test('csv has a column and it is not the decision column', () {
      expect(csvColumns, contains('meta'));
      final int column = csvColumns.indexOf('meta');
      expect(csvRow(metaFlow())[column], 'true');
      expect(csvRow(testFlow(id: 'x'))[column], 'false');
      // Die Entscheidung bleibt leer, wo keine getroffen wurde.
      expect(csvRow(metaFlow())[csvColumns.indexOf('decision')], isEmpty);
    });

    test('jsonl and har carry it next to the decision', () {
      final Map<String, Object?> record = jsonlRecord(testEntry(metaFlow()));
      expect(record['meta'], isTrue);
      expect(record['decision'], isNull);

      final Map<String, Object?> block = humanitlBlock(metaFlow());
      expect(block['meta'], isTrue);
      expect(block['decision'], isNull);
    });
  });

  group('the fake speaks the language of the daemon', () {
    test('fakeFilterKeys is the KEYS list of the recorder, in order', () {
      // Der Fake ist die Vorlage, gegen die die Widget-Tests laufen. Läuft er
      // von der Filtersprache des Recorders weg, prüfen sie eine Erfindung
      // (`backlog/sprint-5.md`, HUM-099). Bis dort eine gemeinsame Tabelle
      // steht, ist dieser Vergleich die Naht.
      final File source = File('../daemon/crates/recorder/src/filter.rs');
      expect(
        source.existsSync(),
        isTrue,
        reason: 'the recorder is next to the app in this repository',
      );
      final String text = source.readAsStringSync();
      final int start = text.indexOf('pub const KEYS: &[&str] = &[');
      expect(start, isNonNegative);
      final int end = text.indexOf('];', start);
      final String body = text.substring(start, end);
      final List<String> keys = RegExp(r'"([a-z_]+)"')
          .allMatches(body)
          .map((RegExpMatch match) => match.group(1)!)
          .toList();
      expect(keys, contains('meta'));
      expect(fakeFilterKeys, keys);
    });

    test('the mark crosses the wire', () {
      // Ohne diese Übertragung sähe die Historie einen Meta-Fluss wie eine
      // Anfrage, über die noch niemand entschieden hat: `decision` ist an
      // beiden leer, und allein `meta` unterscheidet sie.
      final pb.FlowSummary wire = pb.FlowSummary(
        flowId: '018f0004-0000-7000-8000-000000000042',
        sessionId: testSession.value,
        path: '/ask',
        authority: pb.Authority(host: 'humanitl.internal', port: 443),
        state: pb.FlowState.FLOW_STATE_RECORDED,
        status: 202,
        meta: true,
      );
      final Flow flow = wire.toDomain();
      expect(flow.meta, isTrue);
      expect(flow.decision, isNull);
      expect(historyDecider(flow), HistoryDecider.meta);

      final Flow ordinary = pb.FlowSummary(
        flowId: '018f0004-0000-7000-8000-000000000043',
        sessionId: testSession.value,
        path: '/user',
        authority: pb.Authority(host: 'api.github.com', port: 443),
        state: pb.FlowState.FLOW_STATE_RECORDED,
      ).toDomain();
      expect(ordinary.meta, isFalse);
    });

    test('the mark defaults to false, like passthrough', () {
      // `Flow.meta` ist ein Feld wie `passthrough`: Vorgabe `false`, und
      // gesetzt wird es nur, wo der Daemon es setzt.
      expect(testFlow(id: 'x').meta, isFalse);
      expect(metaFlow().meta, isTrue);
      expect(metaFlow().host, 'humanitl.internal');
    });
  });
}
