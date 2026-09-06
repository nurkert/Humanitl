// `HPopover` (HUM-044): ein Kasten mit einer Spitze, der auf etwas daneben
// zeigt.
//
// Die drei Eigenschaften, die kein Zufall sind, stehen hier als Test: Er traegt
// keinen Text selbst, er nimmt keinen Fokus, und `Esc` erreicht ihn nur, wenn
// der Host sie ihm gibt -- er stiehlt sie niemandem.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl_ui/humanitl_ui.dart';

import 'harness.dart';

void main() {
  testWidgets('it shows the words it is given and nothing else', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      harness(
        HPopover(
          title: 'A title',
          body: 'A sentence.',
          closeLabel: 'Close',
          onClose: () {},
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('A title'), findsOneWidget);
    expect(find.text('A sentence.'), findsOneWidget);
    // Die Beschriftung des Knopfs ist Semantik, kein sichtbarer Text: Das
    // Control ist ein Glyph.
    expect(find.text('Close'), findsNothing);
    expect(find.byType(HIconButton), findsOneWidget);
  });

  testWidgets('the close button reports once', (WidgetTester tester) async {
    int closed = 0;
    await tester.pumpWidget(
      harness(
        HPopover(
          title: 'A title',
          body: 'A sentence.',
          closeLabel: 'Close',
          onClose: () => closed++,
        ),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byType(HIconButton));
    await tester.pump();
    expect(closed, 1);
  });

  testWidgets('it binds no key of its own', (WidgetTester tester) async {
    // `Esc` gehoert dem Host: Die Taste kommt dort an, wo der Fokus ist, und
    // dieser Kasten nimmt keinen. Ein `Shortcuts` hier waere eine Bindung, die
    // nie feuert (`docs/UX.md` 5.3); im Programm bindet sie der
    // Intercept-Bildschirm.
    int closed = 0;
    final FocusNode host = FocusNode(debugLabel: 'host');
    addTearDown(host.dispose);
    await tester.pumpWidget(
      harness(
        keyboard(
          Focus(
            focusNode: host,
            autofocus: true,
            child: HPopover(
              title: 'A title',
              body: 'A sentence.',
              closeLabel: 'Close',
              onClose: () => closed++,
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(closed, 0, reason: 'the key belongs to whoever has the focus');
    // Und keine Zuordnung im Kasten nennt die Taste: Der Knopf bringt seine
    // eigene mit (`Enter`, `Space`, die Pfeile), `Esc` steht in keiner.
    for (final Shortcuts shortcuts in tester.widgetList<Shortcuts>(
      find.descendant(
        of: find.byType(HPopover),
        matching: find.byType(Shortcuts),
      ),
    )) {
      expect(
        shortcuts.shortcuts.keys.whereType<SingleActivator>().map(
          (SingleActivator activator) => activator.trigger,
        ),
        isNot(contains(LogicalKeyboardKey.escape)),
      );
    }
  });

  testWidgets('it takes no focus of its own', (WidgetTester tester) async {
    // Der Kasten liegt im Baum vor dem, worauf er zeigt. Ein Fokusstopp darin
    // naehme der Entscheidung den ersten Tabulator (`docs/UX.md` 5.2).
    final FocusNode after = FocusNode(debugLabel: 'after');
    addTearDown(after.dispose);
    await tester.pumpWidget(
      harness(
        keyboard(
          Column(
            children: <Widget>[
              HPopover(
                title: 'A title',
                body: 'A sentence.',
                closeLabel: 'Close',
                onClose: () {},
              ),
              Focus(focusNode: after, child: const SizedBox(height: 20)),
            ],
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    after.requestFocus();
    await tester.pump();
    expect(after.hasPrimaryFocus, isTrue);
  });

  testWidgets('the arrow sits on the edge it points from', (
    WidgetTester tester,
  ) async {
    for (final HPopoverArrow arrow in HPopoverArrow.values) {
      await tester.pumpWidget(
        harness(
          HPopover(
            title: 'A title',
            body: 'A sentence.',
            closeLabel: 'Close',
            onClose: () {},
            arrow: arrow,
          ),
        ),
      );
      await tester.pumpAndSettle();

      final Rect tip = tester.getRect(
        find.descendant(
          of: find.byType(HPopover),
          matching: find.byWidgetPredicate(
            (Widget widget) =>
                widget is CustomPaint &&
                widget.size ==
                    const Size(HPopover.arrowWidth, HPopover.arrowHeight),
          ),
        ),
      );
      final Rect whole = tester.getRect(find.byType(HPopover));
      if (arrow == HPopoverArrow.down) {
        expect(tip.bottom, closeTo(whole.bottom, 0.5), reason: '$arrow');
      } else {
        expect(tip.top, closeTo(whole.top, 0.5), reason: '$arrow');
      }
    }
  });

  testWidgets('it arrives from HMotion.arriveOffset above and fades in', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      harness(
        HPopover(
          title: 'A title',
          body: 'A sentence.',
          closeLabel: 'Close',
          onClose: () {},
        ),
      ),
    );

    final double start = tester.getTopLeft(find.text('A title')).dy;
    expect(fade(tester).opacity.value, 0);

    await tester.pumpAndSettle();
    final double rest = tester.getTopLeft(find.text('A title')).dy;
    expect(
      start,
      closeTo(rest - HMotion.arriveOffset, 0.01),
      reason: 'acht Pixel von oben herunter (`docs/UX.md` 2.1)',
    );
    // Die Umhuellung verlaesst den Baum, sobald sie nichts mehr zu tun hat
    // (`docs/UX.md` 7).
    expect(fades, findsNothing);
  });

  testWidgets('reduced motion takes the travel and leaves the fade', (
    WidgetTester tester,
  ) async {
    // Weniger Weg, nicht weniger Rueckmeldung (`docs/UX.md` 2.10): Die Strecke
    // laeuft ueber `HReducedMotion` und wird null, das Ausblenden behaelt seine
    // volle Dauer und bleibt die Auskunft, dass der Kasten neu ist.
    await tester.pumpWidget(
      harness(
        MediaQuery(
          data: const MediaQueryData(disableAnimations: true),
          child: HPopover(
            title: 'A title',
            body: 'A sentence.',
            closeLabel: 'Close',
            onClose: () {},
          ),
        ),
      ),
    );

    final double start = tester.getTopLeft(find.text('A title')).dy;
    expect(fade(tester).opacity.value, 0);

    await tester.pump(HMotion.arrive ~/ 2);
    expect(
      tester.getTopLeft(find.text('A title')).dy,
      start,
      reason: 'kein Weg unter reduzierter Bewegung',
    );
    final double half = fade(tester).opacity.value;
    expect(half, greaterThan(0));
    expect(half, lessThan(1));

    await tester.pumpAndSettle();
    expect(
      tester.getTopLeft(find.text('A title')).dy,
      start,
      reason: 'der Kasten stand von Anfang an, wo er stehen bleibt',
    );
    expect(find.text('A title'), findsOneWidget);
  });

  testWidgets('the fade keeps its duration when the platform reports it', (
    WidgetTester tester,
  ) async {
    // Ohne `AnimationBehavior.preserve` skaliert Flutter die Dauer auf fuenf
    // Prozent, sobald die Plattform `disableAnimations` meldet: Aus 180 ms
    // wuerden neun, und uebrig bliebe ein Kasten, der aufpoppt (2.10).
    tester.binding.platformDispatcher.accessibilityFeaturesTestValue =
        const FakeAccessibilityFeatures(disableAnimations: true);
    addTearDown(
      tester.binding.platformDispatcher.clearAccessibilityFeaturesTestValue,
    );
    await tester.pumpWidget(
      harness(
        HPopover(
          title: 'A title',
          body: 'A sentence.',
          closeLabel: 'Close',
          onClose: () {},
        ),
      ),
    );

    await tester.pump(HMotion.arrive ~/ 2);
    expect(fade(tester).opacity.value, lessThan(1));

    await tester.pumpAndSettle();
    expect(find.text('A title'), findsOneWidget);
  });
}

/// Die Ueberblendung, an der die Ankunft haengt.
///
/// `FadeTransition` und nicht `Opacity` in einem Builder: Die Transition
/// schreibt in ihr Renderobjekt, statt den Kindbaum je Frame neu zu bauen
/// (`docs/UX.md` 7).
Finder get fades => find.descendant(
  of: find.byType(HPopover),
  matching: find.byType(FadeTransition),
);

/// Die aeusserste Ueberblendung des Popovers in diesem Frame.
FadeTransition fade(WidgetTester tester) =>
    tester.widget<FadeTransition>(fades.first);
