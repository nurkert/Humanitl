// Das Absperrband über der Warteschlange (HUM-260, `docs/UX.md` 3.6): Es
// steht, solange mindestens eine Anfrage gehalten ist, und geht, sobald
// nichts mehr hält. Sein Platz ist immer reserviert, damit die Liste darunter
// nicht springt.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';

import 'harness.dart';

const Key tapeKey = Key('intercept-caution-tape');

void main() {
  testWidgets('the tape stands while a request is held and goes with it', (
    WidgetTester tester,
  ) async {
    final FlowDetail first = held(1);
    final FakeDaemonClient client = fakeDaemon(<ScriptedEvent>[
      ...arriveAt(first, const Duration(milliseconds: 100)),
      ScriptedEvent(
        const Duration(seconds: 2),
        (FakeSessionState state, DateTime now) =>
            FlowEvent.timedOut(at: now, flowId: first.summary.id),
      ),
    ]);
    await pumpIntercept(tester, client: client);

    // Nichts hält: kein Band, kein Ticker dafür.
    expect(find.byKey(tapeKey), findsNothing);
    expect(find.byType(HCautionTape), findsNothing);
    final double listTop = tester.getTopLeft(find.byType(AnimatedList)).dy;

    await playScript(tester);
    expect(find.byKey(tapeKey), findsOneWidget);
    expect(tester.getSize(find.byKey(tapeKey)).height, HSize.cautionTape);
    // Das Band nimmt den reservierten Platz ein und schiebt nichts.
    expect(tester.getTopLeft(find.byType(AnimatedList)).dy, listTop);

    // Die Frist läuft ab, die Anfrage verlässt die Warteschlange.
    await tester.pump(const Duration(seconds: 2));
    await tester.pump();
    await tester.pump(const Duration(seconds: 1));
    expect(find.byKey(tapeKey), findsNothing);
    expect(tester.getTopLeft(find.byType(AnimatedList)).dy, listTop);
  });

  testWidgets('a tape in a hidden section stops ticking', (
    WidgetTester tester,
  ) async {
    // Hier läuft das Band wirklich: die Konfiguration der App-Tests hält es
    // sonst an.
    HCautionTape.debugFreeze = false;
    addTearDown(() => HCautionTape.debugFreeze = true);
    final FakeDaemonClient client = fakeDaemon(
      arriveAt(held(1), const Duration(milliseconds: 100)),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    final Finder tape = find.byKey(tapeKey, skipOffstage: false);
    double phase() =>
        (tester
                    .widget<CustomPaint>(
                      find.descendant(
                        of: tape,
                        matching: find.byType(CustomPaint, skipOffstage: false),
                        skipOffstage: false,
                      ),
                    )
                    .painter!
                as HCautionTapePainter)
            .phase
            .value;

    // Sichtbar läuft es.
    final double before = phase();
    await tester.pump(HMotion.cautionTape ~/ 4);
    expect(phase(), isNot(before));

    // In die History gewechselt: das Band bleibt im Baum, aber sein Ticker
    // schweigt, und es zeichnet keinen Frame mehr.
    await pressControl(tester, LogicalKeyboardKey.digit2);
    await tester.pump();
    expect(tape, findsOneWidget);
    expect(TickerMode.valuesOf(tester.element(tape)).enabled, isFalse);
    final double hidden = phase();
    await tester.pump(HMotion.cautionTape ~/ 3);
    expect(phase(), hidden);

    // Zurück in der Warteschlange läuft es weiter.
    await pressControl(tester, LogicalKeyboardKey.digit1);
    await tester.pump();
    await tester.pump(HMotion.cautionTape ~/ 4);
    expect(phase(), isNot(hidden));
  });
}
