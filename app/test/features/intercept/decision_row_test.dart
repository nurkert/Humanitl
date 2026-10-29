// Die Reihenfolge, in der die Zeile der Entscheidungen nachgibt (HUM-307).
//
// Mit Kästen fester Breite statt echter Controls: So liegt jede Stufe auf
// einem bekannten Pixel, und ein Test an der Grenze bricht, sobald eine Stufe
// zu früh oder zu spät greift.
//
// Ventil 150, „Edit + Allow" 100, Notiz 60, Block 120; Abstand 160, kleinster
// Abstand 96, schmalstes Ventil 120, 12 px zwischen Ventil und Edit. Damit:
//   alles auf natürlicher Breite mit 96 px Abstand ab 150+12+100+36+180 = 478,
//   ohne Edit ab 150+36+180 = 366, mit schmalerem Ventil ab 120+36+180 = 336,
//   darunter Block unter Allow.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/features/intercept/widgets/decision_row.dart';

/// Ein Ventil, 150 px natürlich breit, das bis [narrowest] schrumpfen kann,
/// wie das echte, dessen Beschriftung sich kürzt.
class Valve extends LeafRenderObjectWidget {
  const Valve({this.narrowest = 100, super.key});

  final double narrowest;

  @override
  RenderValve createRenderObject(BuildContext context) =>
      RenderValve(narrowest);

  @override
  void updateRenderObject(BuildContext context, RenderValve renderObject) =>
      renderObject.narrowest = narrowest;
}

class RenderValve extends RenderBox {
  RenderValve(this.narrowest);

  double narrowest;

  @override
  void performLayout() => size = constraints.constrain(const Size(150, 36));

  @override
  Size computeDryLayout(covariant BoxConstraints constraints) =>
      constraints.constrain(const Size(150, 36));

  @override
  double computeMinIntrinsicWidth(double height) => narrowest;

  @override
  double computeMaxIntrinsicWidth(double height) => 150;

  @override
  bool hitTestSelf(Offset position) => true;
}

const Key valve = Key('valve');
const Key edit = Key('edit');
const Key note = Key('note');
const Key block = Key('block');

/// The focus node inside the edit control of the last [pumpRow].
late FocusNode editNode;

/// How often `Enter` on the edit control got through.
int activations = 0;

Future<void> pumpRow(
  WidgetTester tester,
  double width, {
  double narrowest = 100,
  double editWidth = 100,
}) async {
  await tester.pumpWidget(
    Directionality(
      textDirection: TextDirection.ltr,
      child: Align(
        alignment: Alignment.topLeft,
        child: SizedBox(
          width: width,
          child: DecisionRow(
            valve: Valve(key: valve, narrowest: narrowest),
            edit: CallbackShortcuts(
              bindings: <ShortcutActivator, VoidCallback>{
                const SingleActivator(LogicalKeyboardKey.enter): () =>
                    activations++,
              },
              child: Focus(
                focusNode: editNode,
                child: SizedBox(
                  key: edit,
                  width: editWidth,
                  height: 32,
                  child: const ColoredBox(color: Color(0xFF000000)),
                ),
              ),
            ),
            note: const SizedBox(key: note, width: 60, height: 32),
            block: const SizedBox(key: block, width: 120, height: 36),
            gap: 160,
            minGap: 96,
            minDecisionWidth: 120,
            editSpacing: 12,
            noteSpacing: 0,
            lineSpacing: 12,
          ),
        ),
      ),
    ),
  );
}

/// Whether the gate in front of the edit control lets focus through.
bool gateOpen(WidgetTester tester) => tester
    .widgetList<Focus>(
      find.ancestor(of: find.byKey(edit), matching: find.byType(Focus)),
    )
    .map((Focus focus) => focus.focusNode)
    .whereType<FocusNode>()
    .firstWhere((FocusNode node) => node.debugLabel == 'DecisionRow edit gate')
    .descendantsAreFocusable;

Rect rectOf(WidgetTester tester, Key key) => tester.getRect(find.byKey(key));

bool editShown(WidgetTester tester) =>
    find.byKey(edit).hitTestable().evaluate().isNotEmpty;

void main() {
  setUp(() {
    editNode = FocusNode(debugLabel: 'edit');
    activations = 0;
  });
  tearDown(() => editNode.dispose());

  testWidgets('with room everything stands at its size, 160 px apart', (
    WidgetTester tester,
  ) async {
    await pumpRow(tester, 800);

    expect(editShown(tester), isTrue);
    expect(rectOf(tester, valve), const Rect.fromLTWH(0, 0, 150, 36));
    expect(rectOf(tester, edit), const Rect.fromLTWH(162, 2, 100, 32));
    expect(rectOf(tester, block), const Rect.fromLTWH(680, 0, 120, 36));
    expect(rectOf(tester, note), const Rect.fromLTWH(620, 2, 60, 32));
    expect(
      rectOf(tester, block).left - rectOf(tester, edit).right,
      greaterThanOrEqualTo(160),
    );
  });

  testWidgets('the distance shrinks to 96 px before the edit control goes', (
    WidgetTester tester,
  ) async {
    await pumpRow(tester, 478);

    expect(editShown(tester), isTrue);
    expect(rectOf(tester, block).left - rectOf(tester, edit).right, 96);
  });

  testWidgets('one pixel less and the edit control leaves the line', (
    WidgetTester tester,
  ) async {
    await pumpRow(tester, 477);

    expect(editShown(tester), isFalse);
    expect(rectOf(tester, valve).width, 150);
    expect(gateOpen(tester), isFalse);
    expect(rectOf(tester, block).top, rectOf(tester, valve).top);
  });

  testWidgets('then the valve shrinks, down to its narrowest', (
    WidgetTester tester,
  ) async {
    await pumpRow(tester, 366);
    expect(rectOf(tester, valve).width, 150);

    await pumpRow(tester, 336);
    expect(rectOf(tester, valve).width, 120);
    expect(rectOf(tester, block).top, rectOf(tester, valve).top);
    expect(rectOf(tester, block).left - rectOf(tester, valve).right, 96);
  });

  testWidgets('only below that Block moves under Allow, right aligned', (
    WidgetTester tester,
  ) async {
    await pumpRow(tester, 335);

    final Rect allow = rectOf(tester, valve);
    final Rect blocked = rectOf(tester, block);
    expect(blocked.top, greaterThanOrEqualTo(allow.bottom));
    expect(blocked.right, 335);
    expect(rectOf(tester, note).right, blocked.left);
    // 150 + 12 + 100 = 262 passt in 335: Das Edit bleibt neben dem Ventil.
    expect(editShown(tester), isTrue);
    expect(rectOf(tester, edit).top, lessThan(blocked.top));
    expect(tester.takeException(), isNull);
  });

  testWidgets('a wider edit control wins its room back when the pane grows', (
    WidgetTester tester,
  ) async {
    await pumpRow(tester, 400);
    expect(editShown(tester), isFalse);
    await pumpRow(tester, 600);
    expect(editShown(tester), isTrue);
    expect(gateOpen(tester), isTrue);
  });

  testWidgets('the valve never shrinks below what its content needs', (
    WidgetTester tester,
  ) async {
    // Bei doppelter Schrift brauchen Polster, Kürzel und Chevron des echten
    // Ventils mehr als 120 px; darunter liefe seine innere Zeile über.
    await pumpRow(tester, 346, narrowest: 130);
    expect(rectOf(tester, valve).width, 130);
    expect(rectOf(tester, block).top, rectOf(tester, valve).top);

    await pumpRow(tester, 345, narrowest: 130);
    expect(
      rectOf(tester, block).top,
      greaterThanOrEqualTo(rectOf(tester, valve).bottom),
    );
    expect(rectOf(tester, valve).width, 150);
  });

  testWidgets('a focused edit control loses focus in the frame it leaves', (
    WidgetTester tester,
  ) async {
    await pumpRow(tester, 800);
    editNode.requestFocus();
    await tester.pump();
    expect(editNode.hasFocus, isTrue);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    expect(activations, 1);

    // Ein einziger Frame: Der Pane wird schmal, das Edit verlässt die
    // Zeile, und `Enter` gleich danach öffnet nichts mehr.
    await pumpRow(tester, 400);
    expect(editShown(tester), isFalse);
    expect(editNode.hasFocus, isFalse);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    expect(activations, 1);
    expect(gateOpen(tester), isFalse);
  });

  testWidgets('an edit control too wide for the valve line gets its own', (
    WidgetTester tester,
  ) async {
    // 150 + 12 + 200 = 362 passt nicht in 335: drei Zeilen, je 12 px
    // Abstand.
    await pumpRow(tester, 335, editWidth: 200);

    expect(rectOf(tester, valve).top, 0);
    expect(rectOf(tester, edit).top, 36 + 12);
    expect(rectOf(tester, block).top, 36 + 12 + 32 + 12);
    expect(rectOf(tester, block).right, 335);
  });
}
