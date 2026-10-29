// Eine Ebene über der Arbeit nimmt die Tastatur beim Öffnen und gibt sie
// beim Schließen zurück (HUM-303).
//
// `FocusScope(autofocus: true)` allein gibt nach, sobald im Bereich darum
// schon etwas den Fokus hat; genau das ist in der Anwendung immer so. Gemessen
// wird deshalb mit einem Knoten, der den Fokus hält, bevor die Ebene kommt.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl_ui/humanitl_ui.dart';

import 'harness.dart';

/// Ein Knoten mit Fokus und, je nach [open], eine Ebene daneben.
Widget _scene(FocusNode work, {required bool open, FocusNode? inside}) =>
    harness(
      Column(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Focus(
            focusNode: work,
            autofocus: true,
            child: const SizedBox(height: 20),
          ),
          if (open)
            HLayerScope(
              child: Focus(
                focusNode: inside ?? FocusNode(debugLabel: 'unused'),
                child: const SizedBox(height: 20),
              ),
            ),
        ],
      ),
    );

void main() {
  testWidgets('a_layer_takes_the_keyboard_from_a_focused_node_and_returns_it', (
    WidgetTester tester,
  ) async {
    final FocusNode work = FocusNode(debugLabel: 'work');
    addTearDown(work.dispose);
    await tester.pumpWidget(_scene(work, open: false));
    await tester.pump();
    expect(work.hasPrimaryFocus, isTrue);

    await tester.pumpWidget(_scene(work, open: true));
    await tester.pump();
    await tester.pump();
    expect(work.hasFocus, isFalse, reason: 'the layer took the keyboard');
    expect(
      FocusManager.instance.primaryFocus?.debugLabel,
      'layer',
      reason: 'and holds it in its own scope',
    );

    await tester.pumpWidget(_scene(work, open: false));
    await tester.pump();
    expect(work.hasPrimaryFocus, isTrue, reason: 'given back on closing');
  });

  testWidgets('a_node_inside_the_layer_that_took_the_keyboard_keeps_it', (
    WidgetTester tester,
  ) async {
    final FocusNode work = FocusNode(debugLabel: 'work');
    final FocusNode field = FocusNode(debugLabel: 'field');
    addTearDown(work.dispose);
    addTearDown(field.dispose);
    await tester.pumpWidget(_scene(work, open: false));
    await tester.pump();

    await tester.pumpWidget(_scene(work, open: true, inside: field));
    field.requestFocus();
    await tester.pump();
    await tester.pump();
    expect(field.hasPrimaryFocus, isTrue);
  });

  testWidgets('a_layer_somebody_already_left_does_not_pull_the_keyboard_back', (
    WidgetTester tester,
  ) async {
    final FocusNode work = FocusNode(debugLabel: 'work');
    addTearDown(work.dispose);
    await tester.pumpWidget(_scene(work, open: false));
    await tester.pump();
    await tester.pumpWidget(_scene(work, open: true));
    await tester.pump();
    await tester.pump();

    // Weg aus der Ebene, auf den Bereich um alles.
    FocusManager.instance.primaryFocus?.unfocus();
    await tester.pump();
    expect(FocusManager.instance.primaryFocus?.debugLabel, isNot('layer'));
    final FocusNode? before = FocusManager.instance.primaryFocus;
    expect(work.hasFocus, isFalse);

    await tester.pumpWidget(_scene(work, open: false));
    await tester.pump();
    expect(FocusManager.instance.primaryFocus, before);
  });

  /// Eine Ebene, die aus einer anderen heraus aufgeht und sie überlebt, gibt
  /// die Tastatur am Ende dem zurück, der sie vor der ersten hatte -- so wie
  /// die Palette, die „Allow all" ausführt, schließt und das Modal öffnet.
  testWidgets(
    'a_layer_opened_from_a_closing_layer_returns_to_the_first_holder',
    (WidgetTester tester) async {
      final FocusNode work = FocusNode(debugLabel: 'work');
      addTearDown(work.dispose);
      Widget scene({required bool first, required bool second}) => harness(
        Column(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            Focus(
              focusNode: work,
              autofocus: true,
              child: const SizedBox(height: 20),
            ),
            if (first)
              const HLayerScope(key: Key('first'), child: SizedBox(height: 20)),
            if (second)
              const HLayerScope(
                key: Key('second'),
                child: SizedBox(height: 20),
              ),
          ],
        ),
      );
      await tester.pumpWidget(scene(first: false, second: false));
      await tester.pump();
      // Wie jeder Öffner: erst beanspruchen, dann bauen. Das Parken löscht
      // das Gedächtnis des Bereichs; zurück findet die Tastatur nur über die
      // Übergabe zwischen den Ebenen.
      HLayerScope.claimKeyboard();
      await tester.pumpWidget(scene(first: true, second: false));
      await tester.pump();
      await tester.pump();
      expect(work.hasFocus, isFalse);

      // Die zweite geht im selben Bild auf, in dem die erste schließt.
      await tester.pumpWidget(scene(first: false, second: true));
      await tester.pump();
      await tester.pump();
      expect(work.hasFocus, isFalse, reason: 'the second layer has it');

      await tester.pumpWidget(scene(first: false, second: false));
      await tester.pump();
      expect(work.hasPrimaryFocus, isTrue, reason: 'back to the first holder');
    },
  );
}
