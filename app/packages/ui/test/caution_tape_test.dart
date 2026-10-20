// Das Absperrband (HUM-260): läuft gleichmäßig und nahtlos, steht unter
// reduzierter Bewegung, zeichnet nichts, wenn es nicht zu sehen ist, und
// baut beim Laufen nicht neu.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl_ui/humanitl_ui.dart';

import 'harness.dart';

const Size tapeSize = Size(480, HSize.cautionTape);

Finder get _paint => find.descendant(
  of: find.byType(HCautionTape),
  matching: find.byType(CustomPaint),
);

HCautionTapePainter painterOf(WidgetTester tester) =>
    tester.widget<CustomPaint>(_paint).painter! as HCautionTapePainter;

double phaseOf(WidgetTester tester) => painterOf(tester).phase.value;

/// Die Streifen, die auf dem Band zu sehen sind, also die, die rechts der
/// linken Kante enden.
List<double> visibleStarts(double value) =>
    HCautionTapePainter.stripeStarts(value, tapeSize)
        .where(
          (double x) => x + tapeSize.height + HMotion.cautionTapePeriod / 2 > 0,
        )
        .toList();

void main() {
  setUp(() => HCautionTape.debugFreeze = false);

  testWidgets('the tape runs at an even pace and wraps without a jump', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(harness(const HCautionTape()));
    expect(tester.binding.hasScheduledFrame, isTrue);
    final double start = phaseOf(tester);
    expect(start, 0);
    // Ein Viertel der Dauer, ein Viertel der Periode, jedes Mal: linear.
    final Duration quarter = HMotion.cautionTape ~/ 4;
    for (int i = 1; i <= 3; i++) {
      await tester.pump(quarter);
      expect(phaseOf(tester), closeTo(i / 4, 0.01), reason: 'step $i');
    }
    // Über das Ende hinaus beginnt es von vorn und läuft vorwärts weiter,
    // nicht rückwärts.
    await tester.pump(quarter * 2);
    expect(phaseOf(tester), closeTo(0.25, 0.01));
    expect(tester.binding.hasScheduledFrame, isTrue);
  });

  test('a full period draws the same stripes as none', () {
    // Nahtlos: kurz vor dem Ende eines Durchlaufs liegen die sichtbaren
    // Streifen dort, wo sie am Anfang des nächsten liegen.
    final List<double> atStart = visibleStarts(0);
    final List<double> atEnd = visibleStarts(0.9999);
    expect(atEnd, hasLength(atStart.length));
    for (int i = 0; i < atStart.length; i++) {
      expect(atEnd[i], closeTo(atStart[i], 0.01), reason: 'stripe $i');
    }
    expect(HCautionTapePainter.stripeStarts(1, tapeSize), <double>[
      ...HCautionTapePainter.stripeStarts(0, tapeSize),
    ]);
    // Und die Streifen bedecken das Band von Kante zu Kante.
    final List<double> starts = HCautionTapePainter.stripeStarts(
      0.37,
      tapeSize,
    );
    expect(starts.first, lessThanOrEqualTo(-tapeSize.height));
    expect(starts.last, lessThan(tapeSize.width));
    expect(
      starts.last + HMotion.cautionTapePeriod,
      greaterThanOrEqualTo(tapeSize.width),
    );
  });

  testWidgets('reduced motion: the stripes stand still', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      MediaQuery(
        data: const MediaQueryData(disableAnimations: true),
        child: HTheme(
          tokens: HTokens.dark,
          child: const Directionality(
            textDirection: TextDirection.ltr,
            child: Align(
              alignment: Alignment.topLeft,
              child: SizedBox(width: 480, child: HCautionTape()),
            ),
          ),
        ),
      ),
    );
    expect(tester.binding.hasScheduledFrame, isFalse);
    await tester.pump(HMotion.cautionTape ~/ 3);
    expect(phaseOf(tester), 0);
    expect(tester.binding.hasScheduledFrame, isFalse);
    // Die Streifen sind trotzdem da: der Weg fällt weg, die Aussage bleibt.
    expect(find.byType(CustomPaint), findsWidgets);
    expect(
      tester.getSize(find.byType(HCautionTape)),
      const Size(480, HSize.cautionTape),
    );
  });

  testWidgets('a hidden tape schedules no frame', (WidgetTester tester) async {
    await tester.pumpWidget(
      harness(const TickerMode(enabled: false, child: HCautionTape())),
    );
    expect(tester.binding.hasScheduledFrame, isFalse);
    await tester.pump(HMotion.cautionTape);
    expect(phaseOf(tester), 0);
    expect(tester.binding.hasScheduledFrame, isFalse);
    // Sichtbar geworden läuft es los.
    await tester.pumpWidget(
      harness(const TickerMode(enabled: true, child: HCautionTape())),
    );
    expect(tester.binding.hasScheduledFrame, isTrue);
  });

  testWidgets('running repaints the tape, it never rebuilds it', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(harness(const HCautionTape()));
    final CustomPaint before = tester.widget<CustomPaint>(_paint);
    await tester.pump(HMotion.cautionTape ~/ 3);
    expect(phaseOf(tester), greaterThan(0));
    // Dasselbe Widget: der Maler hört auf den Controller, das Widget baut
    // nicht in jedem Frame neu.
    expect(identical(tester.widget<CustomPaint>(_paint), before), isTrue);
    // Und es zeichnet hinter einer eigenen Grenze, damit die Umgebung nicht
    // mitzeichnet.
    expect(
      find.descendant(
        of: find.byType(HCautionTape),
        matching: find.byType(RepaintBoundary),
      ),
      findsOneWidget,
    );
  });

  testWidgets('debugFreeze holds every tape still for the app tests', (
    WidgetTester tester,
  ) async {
    HCautionTape.debugFreeze = true;
    addTearDown(() => HCautionTape.debugFreeze = false);
    await tester.pumpWidget(harness(const HCautionTape(height: 8)));
    expect(tester.binding.hasScheduledFrame, isFalse);
    expect(tester.getSize(find.byType(HCautionTape)).height, 8);
  });

  test('the tape wears the brand, not a state', () {
    expect(HCautionTapePainter.yellow, HColors.accent);
    expect(HCautionTapePainter.black, HColors.onAccent);
    for (final Color state in HStateColors.dark.all) {
      expect(HCautionTapePainter.yellow, isNot(state));
    }
    // Schwarz auf Gelb wie die Marke, und mit Abstand lesbar.
    expect(
      HColorDerivation.contrast(
        HCautionTapePainter.yellow,
        HCautionTapePainter.black,
      ),
      greaterThan(7),
    );
  });
}
