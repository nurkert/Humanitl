// Die kleinen Funktionen hinter Zeile und Karte: Kürzen in der Mitte, Diff
// für die AnimatedList, Formate und der Query-Parser.
//
// Die registrierbare Domain steht nicht mehr darunter: Sie kommt seit HUM-091
// aus `FlowSummary.apex`, und die geratene Tabelle `psl.dart` ist mit ihren
// Tests entfallen.

import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/text/format.dart';
import 'package:humanitl/core/ui/middle_ellipsis.dart';
import 'package:humanitl/features/intercept/list_diff.dart';
import 'package:humanitl/features/intercept/widgets/key_value_table.dart';
import 'package:humanitl/features/intercept/widgets/section_headers.dart';
import 'package:humanitl/features/intercept/widgets/section_query.dart';

import 'fixtures.dart';

void main() {
  test('middle_ellipsis', () {
    expect(
      middleEllipsis('/very/long/path/to/file.json', 16),
      '/very/…file.json',
    );
    expect(middleEllipsis('/short', 16), '/short');
    expect(middleEllipsis('/abc', 2), '…');
    // Ein Ersatzpaar wird nie zerschnitten.
    expect(middleEllipsis('👍👍👍👍👍', 3).runes.length, 3);
  });

  test('list_diff', () {
    final List<FlowId> before = <FlowId>[
      testFlowId(1),
      testFlowId(2),
      testFlowId(3),
    ];
    expect(listDiff(before, before), isEmpty);

    expect(
      listDiff(before, <FlowId>[testFlowId(1), testFlowId(3)]),
      <QueueEdit>[const QueueEdit(QueueEditKind.remove, 1)],
    );
    expect(
      listDiff(before, <FlowId>[
        testFlowId(0),
        testFlowId(1),
        testFlowId(2),
        testFlowId(3),
      ]),
      <QueueEdit>[const QueueEdit(QueueEditKind.insert, 0)],
    );
    // Zwei fallen weg, einer kommt: die Entfernungen zuerst, von hinten.
    expect(
      listDiff(before, <FlowId>[testFlowId(2), testFlowId(4)]),
      <QueueEdit>[
        const QueueEdit(QueueEditKind.remove, 2),
        const QueueEdit(QueueEditKind.remove, 0),
        const QueueEdit(QueueEditKind.insert, 1),
      ],
    );
  });

  test('formatCountdown und formatBytes', () {
    expect(formatCountdown(const Duration(seconds: 12)), '00:12');
    expect(formatCountdown(const Duration(minutes: 5)), '05:00');
    expect(formatCountdown(const Duration(seconds: -3)), '00:00');
    expect(formatCountdown(const Duration(hours: 2)), '120:00');

    expect(formatBytes(0), '0 B');
    expect(formatBytes(512), '512 B');
    expect(formatBytes(1200), '1.2 kB');
    expect(formatBytes(3400000), '3.4 MB');
  });

  test('parseQuery', () {
    expect(parseQuery('/graphql'), isEmpty);
    expect(parseQuery('/search?q=humanitl&limit=10'), <KeyValue>[
      const KeyValue('q', 'humanitl'),
      const KeyValue('limit', '10'),
    ]);
    // Ein kaputtes Escape bleibt roh stehen, statt zu verschwinden.
    expect(parseQuery('/x?a=%zz').single.value, '%zz');
    expect(parseQuery('/x?flag').single, const KeyValue('flag', ''));
  });

  test('isMaskedHeader', () {
    expect(isMaskedHeader('Authorization'), isTrue);
    expect(isMaskedHeader('cookie'), isTrue);
    expect(isMaskedHeader('X-Api-Key'), isTrue);
    expect(isMaskedHeader('content-type'), isFalse);
  });
}
