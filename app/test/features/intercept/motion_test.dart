// Bewegung der Queue (docs/UX.md 2.2, 2.4, 2.10).
//
// Geprüft wird das, was unter reduzierter Bewegung anders sein muss: der Weg
// fällt weg, die Rückmeldung nicht. Die Zeile, die geht, behält dann ihre Höhe
// und blendet an Ort und Stelle aus — ohne Höhe hätte das Ausblenden nichts,
// worüber es laufen könnte.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/intercept/providers/now.dart';
import 'package:humanitl/features/intercept/widgets/queue_pane.dart';

import 'fixtures.dart';
import 'harness.dart';

/// Die Verschiebungen, die die Queue gerade zeichnet.
List<Offset> slides(WidgetTester tester) => tester
    .widgetList<SlideTransition>(
      find.descendant(
        of: find.byType(QueuePane),
        matching: find.byType(SlideTransition),
      ),
    )
    .map((SlideTransition widget) => widget.position.value)
    .toList();

/// Die Höhenfaktoren der gehenden Zeilen.
List<double> heights(WidgetTester tester) => tester
    .widgetList<SizeTransition>(
      find.descendant(
        of: find.byType(QueuePane),
        matching: find.byType(SizeTransition),
      ),
    )
    .map((SizeTransition widget) => widget.sizeFactor.value)
    .toList();

/// Wartet, bis die entschiedene Zeile den Abgang begonnen hat.
///
/// Die Frames haben die Länge null, damit die Bewegung dabei nicht abläuft:
/// geprüft wird ihr erster Zustand, nicht ihr letzter.
Future<SizeTransition> leavingSize(WidgetTester tester) async {
  final Finder finder = find.descendant(
    of: find.byType(QueuePane),
    matching: find.byType(SizeTransition),
  );
  for (int i = 0; i < 6; i++) {
    if (finder.evaluate().isNotEmpty) {
      return tester.widget<SizeTransition>(finder.first);
    }
    await tester.pump();
  }
  fail('the decided row never started to leave');
}

/// Der Weg der Zeile, die gerade geht.
Offset leavingSlide(WidgetTester tester) => tester
    .widgetList<SlideTransition>(
      find.descendant(
        of: find.byType(QueuePane),
        matching: find.byType(SlideTransition),
      ),
    )
    .last
    .position
    .value;

/// Ein Skript mit einer Ankunft nach 100 ms.
List<ScriptedEvent> oneArrival() =>
    arriveAt(held(1), const Duration(milliseconds: 100));

void main() {
  testWidgets('an arriving row travels 8 px from above', (
    WidgetTester tester,
  ) async {
    await pumpIntercept(tester, client: fakeDaemon(oneArrival()));
    await tester.pump(const Duration(milliseconds: 120));
    // Mitten in `HMotion.arrive`.
    await tester.pump(const Duration(milliseconds: 60));

    expect(
      slides(tester).any((Offset offset) => offset.dy != 0),
      isTrue,
      reason: 'the arrival has a way',
    );
  });

  testWidgets('under reduced motion the way falls away', (
    WidgetTester tester,
  ) async {
    await pumpIntercept(
      tester,
      client: fakeDaemon(oneArrival()),
      disableAnimations: true,
    );
    await tester.pump(const Duration(milliseconds: 120));
    await tester.pump(const Duration(milliseconds: 60));

    expect(slides(tester), everyElement(Offset.zero));
  });

  testWidgets('under reduced motion a leaving row keeps its height', (
    WidgetTester tester,
  ) async {
    await pumpIntercept(
      tester,
      client: fakeDaemon(oneArrival()),
      disableAnimations: true,
    );
    // Die Ankunft liegt bei 100 ms, die Armierung dauert `HMotion.rearm`.
    await playScript(tester, const Duration(milliseconds: 600));

    await tester.sendKeyEvent(LogicalKeyboardKey.keyA);
    await tester.pump();
    await tester.pump();
    // Der Bestätigungsstreifen steht drei Sekunden; erst wenn die Uhr der
    // Oberfläche weiter ist, verlässt die Zeile den Schnappschuss.
    (containerOf(tester).read(nowProvider.notifier) as FixedNow).moveTo(
      testStart.add(const Duration(seconds: 4)),
    );

    final SizeTransition size = await leavingSize(tester);
    // Der Kollaps entfällt, das Ausblenden bleibt: eine Zeile ohne Höhe hätte
    // nichts, worüber das Ausblenden laufen könnte (docs/UX.md 2.10).
    expect(size.sizeFactor, isA<AlwaysStoppedAnimation<double>>());
    expect(size.sizeFactor.value, 1.0);
    expect(leavingSlide(tester), Offset.zero);
    await tester.pumpAndSettle();
  });
}
