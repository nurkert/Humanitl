// Die Release Valve und der Block-Button (HUM-028): Tippen, Halten,
// Trefferflächen, Fokusring und die Ablehnung eines zu kurzen Drucks.

import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ui/focus_ring.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/widgets/block_button.dart';
import 'package:humanitl/features/intercept/widgets/release_valve.dart';

/// Baut [child] im dunklen Theme, mittig, ohne Provider.
Future<void> pumpControl(WidgetTester tester, Widget child) async {
  await tester.pumpWidget(
    Directionality(
      textDirection: TextDirection.ltr,
      child: HTheme.dark(child: Center(child: child)),
    ),
  );
}

/// Hält [finder] für [duration] gedrückt. Der erste Frame startet den Ticker,
/// erst der nächste bewegt ihn.
Future<void> hold(WidgetTester tester, Finder finder, Duration duration) async {
  final TestGesture gesture = await tester.startGesture(
    tester.getCenter(finder),
  );
  await tester.pump();
  await tester.pump(duration);
  await gesture.up();
  await tester.pump();
}

ReleaseValve valve({
  required VoidCallback onAllow,
  required VoidCallback onAllowRemembered,
  VoidCallback? onToggleOptions,
  VoidCallback? onShortPress,
  bool holdRequired = false,
  Object? holdToken,
  int refusals = 0,
  double? previewHold,
}) => ReleaseValve(
  label: 'Allow',
  holdLabel: 'Allow for session',
  shortcutHint: 'Enter',
  semanticsValue: '4:59 left',
  optionsLabel: 'Duration and scope of the rule',
  onAllow: onAllow,
  onAllowRemembered: onAllowRemembered,
  onToggleOptions: onToggleOptions ?? () {},
  onShortPress: onShortPress,
  holdRequired: holdRequired,
  holdToken: holdToken,
  refusals: refusals,
  previewHold: previewHold,
);

void main() {
  testWidgets('the hold keeps its full time when animations are off', (
    WidgetTester tester,
  ) async {
    // Die Plattform meldet `disableAnimations`, und ein `AnimationController`
    // mit dem normalen Verhalten skaliert dann jede Dauer auf fünf Prozent:
    // aus 400 ms würden 20 ms und ein gewöhnlicher Klick wäre eine
    // Bestätigung. Das Halten ist keine Zierde, es ist die Zeit, in der eine
    // Entscheidung zurückgenommen werden kann (docs/UX.md 2.10, 4.7).
    tester.binding.platformDispatcher.accessibilityFeaturesTestValue =
        const FakeAccessibilityFeatures(disableAnimations: true);
    addTearDown(
      tester.binding.platformDispatcher.clearAccessibilityFeaturesTestValue,
    );
    int sent = 0;
    int refused = 0;
    await pumpControl(
      tester,
      valve(
        holdRequired: true,
        onAllow: () => sent++,
        onAllowRemembered: () => fail('the hold sends once, it remembers not'),
        onShortPress: () => refused++,
      ),
    );

    await hold(
      tester,
      find.byKey(const Key('intercept-valve-hold')),
      const Duration(milliseconds: 30),
    );

    expect(sent, 0, reason: '30 ms are not 400 ms, whatever the platform says');
    expect(refused, 1);
  });

  testWidgets('while a finding is open only the hold sends', (
    WidgetTester tester,
  ) async {
    int sent = 0;
    int refused = 0;
    await pumpControl(
      tester,
      valve(
        holdRequired: true,
        onAllow: () => sent++,
        onAllowRemembered: () => fail('the hold sends once, it remembers not'),
        onShortPress: () => refused++,
      ),
    );

    // Ein Klick sagt nur, warum er nichts tut (docs/UX.md 4.7).
    await tester.tap(find.byKey(const Key('intercept-valve-hold')));
    await tester.pump();
    expect(sent, 0);
    expect(refused, 1);

    await hold(
      tester,
      find.byKey(const Key('intercept-valve-hold')),
      HMotion.holdToConfirm + const Duration(milliseconds: 50),
    );
    expect(sent, 1);
  });

  testWidgets('a hold whose request changes decides nothing', (
    WidgetTester tester,
  ) async {
    int sent = 0;
    Future<void> build(String token) => pumpControl(
      tester,
      valve(
        holdToken: token,
        onAllow: () => fail('a short press does not send here'),
        onAllowRemembered: () => sent++,
      ),
    );
    await build('flow-1');

    final TestGesture gesture = await tester.startGesture(
      tester.getCenter(find.byKey(const Key('intercept-valve-hold'))),
    );
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 100));
    // Die Auswahl wandert unter dem Finger weiter.
    await build('flow-2');
    await tester.pump(HMotion.holdToConfirm + const Duration(milliseconds: 50));
    await gesture.up();
    await tester.pump();

    expect(
      sent,
      0,
      reason: 'a hold decides what it was started on, or nothing',
    );
  });
  testWidgets('a tap allows once', (WidgetTester tester) async {
    int once = 0;
    int remembered = 0;
    await pumpControl(
      tester,
      valve(onAllow: () => once++, onAllowRemembered: () => remembered++),
    );

    await tester.tap(find.byKey(const Key('intercept-valve-hold')));
    await tester.pump();

    expect(once, 1);
    expect(remembered, 0);
  });

  testWidgets('holding past the token allows for the session', (
    WidgetTester tester,
  ) async {
    int once = 0;
    int remembered = 0;
    await pumpControl(
      tester,
      valve(onAllow: () => once++, onAllowRemembered: () => remembered++),
    );

    await hold(
      tester,
      find.byKey(const Key('intercept-valve-hold')),
      HMotion.holdToConfirm + const Duration(milliseconds: 50),
    );

    expect(remembered, 1);
    expect(once, 0);
  });

  testWidgets('letting go too early allows once', (WidgetTester tester) async {
    int once = 0;
    int remembered = 0;
    await pumpControl(
      tester,
      valve(onAllow: () => once++, onAllowRemembered: () => remembered++),
    );

    await hold(
      tester,
      find.byKey(const Key('intercept-valve-hold')),
      const Duration(milliseconds: 300),
    );

    expect(once, 1);
    expect(remembered, 0);
  });

  testWidgets('the label says what the hold will do', (
    WidgetTester tester,
  ) async {
    await pumpControl(
      tester,
      valve(onAllow: () {}, onAllowRemembered: () {}, previewHold: 0.5),
    );

    expect(find.text('Allow for session'), findsOneWidget);
    expect(find.text('Allow'), findsNothing);
  });

  testWidgets('the chevron opens the grid', (WidgetTester tester) async {
    int toggled = 0;
    await pumpControl(
      tester,
      valve(
        onAllow: () {},
        onAllowRemembered: () {},
        onToggleOptions: () => toggled++,
      ),
    );

    await tester.tap(find.byKey(const Key('intercept-valve-options')));
    await tester.pump();

    expect(toggled, 1);
  });

  testWidgets('both decisions are larger than everything else', (
    WidgetTester tester,
  ) async {
    await pumpControl(
      tester,
      Column(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          valve(onAllow: () {}, onAllowRemembered: () {}),
          BlockButton(
            label: 'Block',
            shortcutHint: 'B',
            semanticsValue: '4:59 left',
            onBlock: () {},
          ),
        ],
      ),
    );

    // `HSize.hitDecision`: 32 px hoch, 120 px breit, gemessen und nicht
    // behauptet (docs/UX.md 5.4 und 6).
    for (final Finder control in <Finder>[
      find.byType(ReleaseValve),
      find.byType(BlockButton),
    ]) {
      final Size size = tester.getSize(control);
      expect(
        size.height,
        greaterThanOrEqualTo(HSize.hitDecision.height),
        reason: '$control',
      );
      expect(
        size.width,
        greaterThanOrEqualTo(HSize.hitDecision.width),
        reason: '$control',
      );
    }
  });

  testWidgets('the focus ring is there at once and never fades in', (
    WidgetTester tester,
  ) async {
    await pumpControl(tester, valve(onAllow: () {}, onAllowRemembered: () {}));
    expect(tester.widget<FocusRing>(find.byType(FocusRing)).visible, isFalse);

    FocusScope.of(tester.element(find.byType(ReleaseValve))).nextFocus();
    // Der Fokuswechsel selbst wird in einem Microtask angewandt; der Ring
    // steht im Frame danach vollständig da. Er blendet nicht ein: der Zustand
    // ist an oder aus, es gibt keinen dritten (docs/UX.md 6).
    await tester.pump();
    await tester.pump();

    expect(tester.widget<FocusRing>(find.byType(FocusRing)).visible, isTrue);
    await tester.pump(const Duration(milliseconds: 200));
    expect(tester.widget<FocusRing>(find.byType(FocusRing)).visible, isTrue);
  });

  testWidgets('block needs the hold and says so when it does not get it', (
    WidgetTester tester,
  ) async {
    int blocked = 0;
    int refused = 0;
    await pumpControl(
      tester,
      BlockButton(
        label: 'Block',
        shortcutHint: 'B',
        semanticsValue: '4:59 left',
        onBlock: () => blocked++,
        onShortPress: () => refused++,
      ),
    );

    await hold(
      tester,
      find.byType(BlockButton),
      const Duration(milliseconds: 100),
    );
    expect(blocked, 0);
    expect(refused, 1);

    await hold(
      tester,
      find.byType(BlockButton),
      HMotion.holdToBlock + const Duration(milliseconds: 50),
    );
    expect(blocked, 1);
    expect(refused, 1);
  });

  testWidgets('reduced motion keeps the feedback, only the distance goes', (
    WidgetTester tester,
  ) async {
    int remembered = 0;
    await tester.pumpWidget(
      MediaQuery(
        data: const MediaQueryData(disableAnimations: true),
        child: Directionality(
          textDirection: TextDirection.ltr,
          child: HTheme.dark(
            child: Center(
              child: valve(
                onAllow: () {},
                onAllowRemembered: () => remembered++,
              ),
            ),
          ),
        ),
      ),
    );

    await hold(
      tester,
      find.byKey(const Key('intercept-valve-hold')),
      HMotion.holdToConfirm + const Duration(milliseconds: 50),
    );

    // Die Halte-Bestätigung ist Rückmeldung, kein Weg: sie behält ihre volle
    // Dauer (docs/UX.md 2.10).
    expect(remembered, 1);
  });
}
