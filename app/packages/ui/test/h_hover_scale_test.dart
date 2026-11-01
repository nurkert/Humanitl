import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl_ui/humanitl_ui.dart';

import 'harness.dart';

/// Hover in einer Liste: animiert, nur Skalierung um die Mitte, nichts
/// verschiebt sich (HUM-406).
///
/// Der Test misst, was ein Mensch beim Fahren durch die Regelliste sah: Die
/// überfahrene Zeile zuckte, weil ihre Schrift zwei Pixel nach unten sprang,
/// sobald der Aktionsslot aufging. Er fällt, wenn der Effekt wieder als
/// Verschiebung kommt, wenn er die Layoutgröße der Zeile ändert oder die
/// Nachbarn bewegt, und wenn er schlagartig statt animiert kommt.
void main() {
  Widget row(int n) => HRow(
    key: Key('row-$n'),
    state: HFlowState.allowed,
    onTap: () {},
    stateGlyph: const HStateGlyph(state: HFlowState.allowed),
    title: Text('registry-$n.example.org', key: Key('title-$n')),
    trailing: Text('always', key: Key('trailing-$n')),
    // Ein Control von [HSize.hitMin], wie der Papierkorb der Regelliste:
    // höher als die Schrift, und genau das ließ sie springen.
    actionSlot: SizedBox.square(key: Key('slot-$n'), dimension: HSize.hitMin),
  );

  Widget list({bool reduced = false}) => harness(
    Builder(
      builder: (BuildContext context) => MediaQuery(
        data: MediaQuery.of(context).copyWith(disableAnimations: reduced),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[row(0), row(1), row(2)],
        ),
      ),
    ),
  );

  Future<TestGesture> mouse(WidgetTester tester) async {
    final TestGesture gesture = await tester.createGesture(
      kind: PointerDeviceKind.mouse,
    );
    await gesture.addPointer(location: const Offset(470, 400));
    addTearDown(gesture.removePointer);
    await tester.pump();
    return gesture;
  }

  /// Was gemalt wird: die Lage von Titel und Nachbarzeile auf dem Schirm,
  /// mit jeder Transformation darüber.
  Rect title(WidgetTester tester) =>
      tester.getRect(find.byKey(const Key('title-1')));
  Rect trailing(WidgetTester tester) =>
      tester.getRect(find.byKey(const Key('trailing-1')));
  Rect neighbour(WidgetTester tester, int n) =>
      tester.getRect(find.byKey(Key('title-$n')));

  /// Die Layoutgröße der Zeile, ohne jede gemalte Transformation.
  Size layoutSize(WidgetTester tester) =>
      tester.renderObject<RenderBox>(find.byKey(const Key('row-1'))).size;

  /// Prüft, dass [now] aus [before] durch eine reine Skalierung um [centre]
  /// hervorgeht, und gibt den Faktor zurück.
  double scaleAbout(Rect before, Rect now, Offset centre, String what) {
    final double factor = now.width / before.width;
    expect(
      now.height / before.height,
      moreOrLessEquals(factor, epsilon: 1e-6),
      reason: '$what: uniform scale, no stretch',
    );
    final Offset expected = centre + (before.center - centre) * factor;
    expect(
      now.center.dx,
      moreOrLessEquals(expected.dx, epsilon: 1e-6),
      reason: '$what: no horizontal translation',
    );
    expect(
      now.center.dy,
      moreOrLessEquals(expected.dy, epsilon: 1e-6),
      reason: '$what: no vertical translation',
    );
    return factor;
  }

  testWidgets('hover scales the row around its centre and moves nothing', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(list());
    final TestGesture pointer = await mouse(tester);

    final Rect title0 = title(tester);
    final Rect trailing0 = trailing(tester);
    final Rect above = neighbour(tester, 0);
    final Rect below = neighbour(tester, 2);
    final Size size = layoutSize(tester);
    final Rect rowRect = tester.getRect(find.byKey(const Key('row-1')));
    final Offset centre = rowRect.center;
    // Der Inhalt steht mittig in der Zeile, nicht oben angeschlagen: Oben
    // angeschlagen sprang er, sobald der Slot ein höheres Control aufdeckte.
    expect(title0.center.dy, moreOrLessEquals(centre.dy, epsilon: 0.5));

    await pointer.moveTo(centre);
    await tester.pump();
    // Animiert, nicht schlagartig: im ersten Frame steht die Zeile noch.
    expect(title(tester), title0, reason: 'the first frame has not moved');
    expect(find.byKey(const Key('slot-1')), findsOneWidget);

    // Mitten im Übergang und am Ende: dieselben Messungen.
    Future<double> measure() async {
      expect(neighbour(tester, 0), above, reason: 'row above stays');
      expect(neighbour(tester, 2), below, reason: 'row below stays');
      expect(layoutSize(tester), size, reason: 'layout size stays');
      final double factor = scaleAbout(title0, title(tester), centre, 'title');
      expect(
        scaleAbout(trailing0, trailing(tester), centre, 'trailing'),
        moreOrLessEquals(factor, epsilon: 1e-6),
      );
      return factor;
    }

    await tester.pump(HMotion.press ~/ 2);
    final double halfway = await measure();
    await tester.pump(HMotion.press ~/ 2);
    final double arrived = await measure();
    final List<double> factors = <double>[halfway, arrived];
    // Er wächst über die Dauer und kommt an: die Seitenkanten genau
    // [HMotion.hoverGrow] weiter außen.
    expect(factors.first, greaterThan(1));
    expect(factors.first, lessThan(factors.last));
    expect(
      factors.last,
      moreOrLessEquals(1 + 2 * HMotion.hoverGrow / rowRect.width),
    );

    // Und zurück, ohne Rest.
    await pointer.moveTo(const Offset(470, 400));
    await tester.pumpAndSettle();
    expect(title(tester), title0);
    expect(neighbour(tester, 2), below);
  });

  testWidgets('under reduced motion only the colour changes', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(list(reduced: true));
    final TestGesture pointer = await mouse(tester);
    final Rect title0 = title(tester);
    final Rect below = neighbour(tester, 2);
    final Color rest = tester
        .widget<HAnimatedFill>(
          find.descendant(
            of: find.byKey(const Key('row-1')),
            matching: find.byType(HAnimatedFill),
          ),
        )
        .color;

    await pointer.moveTo(tester.getCenter(find.byKey(const Key('row-1'))));
    await tester.pumpAndSettle();
    expect(title(tester), title0, reason: 'no scale, no translation');
    expect(neighbour(tester, 2), below);
    final Color hovered = tester
        .widget<HAnimatedFill>(
          find.descendant(
            of: find.byKey(const Key('row-1')),
            matching: find.byType(HAnimatedFill),
          ),
        )
        .color;
    expect(hovered, isNot(rest), reason: 'the fill still answers');
    expect(hovered, HTokens.dark.colors.bg2);
  });
  testWidgets('a 28 px row keeps its height when the slot reveals a button', (
    WidgetTester tester,
  ) async {
    // Die Dichte der History: Zeile und Zielfläche sind beide 28 px, und der
    // Knopf im Slot misst mit Fokusring 32 (HUM-406, Review).
    Widget historyRow(int n) => HRow(
      key: Key('row-$n'),
      state: HFlowState.allowed,
      minHeight: HSize.rowHistory,
      onTap: () {},
      title: Text('host-$n.example.org', key: Key('title-$n')),
      actionSlot: HIconButton(
        key: Key('slot-$n'),
        glyph: HGlyph.close,
        semanticsLabel: 'remove $n',
        onPressed: () {},
      ),
    );
    await tester.pumpWidget(
      harness(
        Column(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[historyRow(0), historyRow(1), historyRow(2)],
        ),
      ),
    );
    final TestGesture pointer = await mouse(tester);
    final Rect below = neighbour(tester, 2);
    expect(layoutSize(tester).height, HSize.rowHistory);

    await pointer.moveTo(tester.getCenter(find.byKey(const Key('row-1'))));
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('slot-1')), findsOneWidget);
    expect(layoutSize(tester).height, HSize.rowHistory, reason: 'no growth');
    expect(neighbour(tester, 2), below, reason: 'row below stays');
    // Die volle Zielfläche, in beiden Richtungen.
    final Size target = tester.getSize(
      find
          .descendant(
            of: find.byKey(const Key('slot-1')),
            matching: find.byType(SizedBox),
          )
          .last,
    );
    expect(target.width, moreOrLessEquals(HSize.hitMin, epsilon: 0.5));
    expect(target.height, moreOrLessEquals(HSize.hitMin, epsilon: 0.5));
    expect(tester.takeException(), isNull);
  });

  testWidgets('the painted fringe is not a hover and nothing keeps ticking', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(list());
    final TestGesture pointer = await mouse(tester);
    final RenderBox row = tester.renderObject<RenderBox>(
      find.byKey(const Key('row-1')),
    );
    final Rect layout = row.localToGlobal(Offset.zero) & row.size;
    // Erst ganz aufgezogen, dann einen Pixel rechts neben der Layoutkante:
    // Dort malt die skalierte Zeile noch, aber getroffen wird sie nicht.
    await pointer.moveTo(layout.center);
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('slot-1')), findsOneWidget);
    await pointer.moveTo(Offset(layout.right + 1, layout.center.dy));
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('slot-1')), findsNothing);
    // Einen Pixel über der Zeile: das ist die Zeile darüber, nicht diese.
    await pointer.moveTo(Offset(layout.center.dx, layout.top - 1));
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('slot-1')), findsNothing);
    expect(find.byKey(const Key('slot-0')), findsOneWidget);

    // Kein Controller läuft weiter, wenn die Zeilen gehen.
    await tester.pumpWidget(const SizedBox());
    expect(tester.binding.transientCallbackCount, 0);
  });
}
