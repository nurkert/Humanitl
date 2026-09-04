// Was ein großer Rumpf kostet. Kein Frame-Budget -- in
// `AutomatedTestWidgetsFlutterBinding` rastert nichts, also wäre jede
// `FrameTiming` bedeutungslos (`docs/UX.md` 7). Gemessen wird, was hier
// bestimmbar ist: die Zeit bis zum Modell und die Zahl der Zeilen, die eine
// Ansicht dafür baut.

import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/intercept/body/body_kind.dart';
import 'package:humanitl/features/intercept/body/body_parser.dart';
import 'package:humanitl/features/intercept/body/json_tree_view.dart';

import 'harness.dart';

/// Acht Mebibyte JSON, so wie das Fake-Szenario `big_body` sie liefert.
Uint8List eightMebibytes() {
  final StringBuffer buffer = StringBuffer('{"records":[');
  int i = 0;
  while (buffer.length < bodyMaxBytes - 256) {
    if (i > 0) {
      buffer.write(',');
    }
    buffer.write('{"id":$i,"name":"record $i","tags":["a","b"],"ok":true}');
    i++;
  }
  buffer.write(']}');
  return Uint8List.fromList(utf8.encode(buffer.toString()));
}

void main() {
  test(
    'the big_body scenario delivers eight mebibytes and one finding',
    () async {
      // Das Szenario aus den Akzeptanzkriterien, ohne Bildschirm: der Fake
      // liefert wirklich acht Mebibyte, der Fund sitzt darin, und die Erkennung
      // nennt den Rumpf JSON und nicht "zu groß".
      final FakeDaemonClient client = FakeDaemonClient.scenario('big_body');
      addTearDown(client.close);
      final List<FlowEvent> events = <FlowEvent>[];
      final StreamSubscription<FlowEvent> subscription = client
          .subscribe()
          .listen(events.add);
      await Future<void>.delayed(const Duration(milliseconds: 400));
      await subscription.cancel();

      final FlowEventReceived received = events
          .whereType<FlowEventReceived>()
          .single;
      final BodyRef reference =
          client.state.details[received.flow.id]!.request!.body;
      expect(reference.size, greaterThan(8 * 1024 * 1024 - 4096));
      expect(reference.size, lessThanOrEqualTo(bodyMaxBytes));

      final BytesBuilder buffer = BytesBuilder(copy: false);
      await for (final Uint8List chunk in client.getBody(reference)) {
        buffer.add(chunk);
      }
      final Uint8List bytes = buffer.takeBytes();
      expect(bytes.length, reference.size);
      expect(detectBodyKind(bytes, 'application/json'), BodyKind.json);

      final Finding finding = events
          .whereType<FlowEventAnalyzed>()
          .single
          .findings
          .single;
      expect(
        const Utf8Decoder().convert(bytes, finding.spanStart, finding.spanEnd),
        'big.body@example.org',
      );
    },
  );

  test('eight mebibytes of json are taken apart off the ui isolate', () async {
    // Kein Wanduhr-Gatter: was `flutter test` misst, hängt an der Last der
    // anderen Testdateien (`docs/UX.md` 7). Gemessen wird stattdessen, was
    // eigentlich gemeint ist -- dass die Schleife dieses Isolates währenddessen
    // weiterläuft. Der Vergleich ist selbstkalibrierend: die längste Pause
    // während des Zerlegens auf dem anderen Isolate wird gegen die Zeit
    // gehalten, die dasselbe Zerlegen hier gekostet hätte.
    final Uint8List bytes = eightMebibytes();
    expect(bytes.length, greaterThan(bodyIsolateThreshold));
    expect(bytes.length, lessThanOrEqualTo(bodyMaxBytes));
    expect(detectBodyKind(bytes, 'application/json'), BodyKind.json);

    final Stopwatch clock = Stopwatch()..start();
    int last = 0;
    int longestPause = 0;
    final Timer ticker = Timer.periodic(const Duration(milliseconds: 1), (
      Timer _,
    ) {
      final int now = clock.elapsedMilliseconds;
      if (now - last > longestPause) {
        longestPause = now - last;
      }
      last = now;
    });
    final ParsedBody parsed = await parseBodyAsync(
      bytes,
      BodyKind.json,
      const <Finding>[],
    );
    ticker.cancel();
    final int offIsolate = clock.elapsedMilliseconds;

    final Stopwatch inline = Stopwatch()..start();
    parseBody(bytes, BodyKind.json, const <Finding>[]);
    inline.stop();

    // ignore: avoid_print
    print(
      '8 MiB JSON: ${parsed.json!.nodes.length} nodes, '
      '${parsed.text!.rows.length} rows, '
      '$offIsolate ms off the isolate, '
      '${inline.elapsedMilliseconds} ms inline, '
      'longest pause on this isolate $longestPause ms',
    );
    expect(parsed.json, isNotNull);
    expect(parsed.json!.capped, isTrue);
    expect(parsed.json!.nodes.length, lessThanOrEqualTo(jsonMaxNodes));
    expect(longestPause, lessThan(inline.elapsedMilliseconds ~/ 2));
  });

  testWidgets('a huge tree builds only the rows of its viewport', (
    WidgetTester tester,
  ) async {
    final ParsedBody parsed = parseBody(
      eightMebibytes(),
      BodyKind.json,
      const <Finding>[],
    );
    await tester.binding.setSurfaceSize(const Size(700, 300));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await pumpBody(
      tester,
      JsonTreeView(document: parsed.json!, findings: parsed.findings),
      size: const Size(700, 300),
    );
    // Die Wurzel ist offen, darunter steht ein Feld mit Zehntausenden
    // Einträgen; gebaut werden trotzdem nur die Zeilen des Ausschnitts.
    expect(find.byType(ListView), findsOneWidget);
    expect(tester.widgetList<Row>(find.byType(Row)).length, lessThan(40));
  });
}
