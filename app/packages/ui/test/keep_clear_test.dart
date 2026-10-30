// Ein Bereich, über den sich nichts Schwebendes legen darf (HUM-300).
//
// Gezählt wird nur, was gezeigt wird und eine Fläche hat: ein Bereich in
// einem verdeckten Kind eines `IndexedStack` steht nicht auf dem Schirm, und
// ein leerer Bereich hält nichts frei.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl_ui/humanitl_ui.dart';

const Key _root = Key('root');

Widget _scene({required int shown, double emptyHeight = 0}) => Directionality(
  textDirection: TextDirection.ltr,
  // Links oben im Testfenster und nicht über das ganze Fenster gezogen.
  child: Align(
    alignment: Alignment.topLeft,
    child: SizedBox(
      key: _root,
      width: 400,
      height: 300,
      child: Stack(
        children: <Widget>[
          IndexedStack(
            index: shown,
            children: const <Widget>[
              Align(
                alignment: Alignment.topLeft,
                child: HKeepClear(child: SizedBox(width: 100, height: 50)),
              ),
              Align(
                alignment: Alignment.bottomRight,
                child: HKeepClear(child: SizedBox(width: 60, height: 40)),
              ),
            ],
          ),
          Positioned(
            left: 200,
            top: 10,
            child: HKeepClear(child: SizedBox(width: 80, height: emptyHeight)),
          ),
        ],
      ),
    ),
  ),
);

List<Rect> _rects(WidgetTester tester) =>
    HKeepClear.rectsIn(tester.renderObject<RenderBox>(find.byKey(_root)));

void main() {
  testWidgets('only_the_shown_child_of_an_indexed_stack_counts', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(_scene(shown: 0));
    expect(_rects(tester), <Rect>[const Rect.fromLTWH(0, 0, 100, 50)]);

    await tester.pumpWidget(_scene(shown: 1));
    expect(_rects(tester), <Rect>[const Rect.fromLTWH(340, 260, 60, 40)]);
  });

  testWidgets('an_empty_region_keeps_nothing_clear', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(_scene(shown: 0));
    expect(_rects(tester), hasLength(1));

    await tester.pumpWidget(_scene(shown: 0, emptyHeight: 30));
    expect(_rects(tester), contains(const Rect.fromLTWH(200, 10, 80, 30)));
  });

  testWidgets('a_change_is_announced_by_the_end_of_the_frame', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(_scene(shown: 0));
    await tester.pump();
    final int before = HKeepClear.changes.value;

    // Ein Frame ohne Änderung meldet nichts.
    await tester.pump();
    expect(HKeepClear.changes.value, before);

    await tester.pumpWidget(_scene(shown: 1));
    expect(HKeepClear.changes.value, greaterThan(before));
  });

  testWidgets('a_removed_region_is_gone', (WidgetTester tester) async {
    await tester.pumpWidget(_scene(shown: 0));
    expect(_rects(tester), isNotEmpty);
    await tester.pumpWidget(
      const Directionality(
        textDirection: TextDirection.ltr,
        child: SizedBox(key: _root, width: 400, height: 300),
      ),
    );
    expect(_rects(tester), isEmpty);
  });
}
