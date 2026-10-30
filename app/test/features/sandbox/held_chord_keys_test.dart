// Die gehaltenen Kürzeltasten des Terminals leben mit dem Provider-Bereich
// und nicht mit einem Pane (HUM-300, Review-Runde 3).
//
// Ist kein Pane eingehängt -- das schwebende Fenster mangels Platz
// ausgeblendet, das alte Pane fort --, muss ein Loslassen die Taste trotzdem
// freigeben; sonst schluckte das nächste Terminal die Wiederholungen eines
// Drückens, das anderswo begann.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/features/sandbox/widgets/terminal_pane.dart';

void main() {
  testWidgets('a_release_frees_the_key_with_no_terminal_mounted', (
    WidgetTester tester,
  ) async {
    final ProviderContainer container = ProviderContainer();
    addTearDown(container.dispose);
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: const SizedBox.shrink(),
      ),
    );
    final HeldChordKeys held = container.read(heldChordKeysProvider);

    await tester.sendKeyDownEvent(LogicalKeyboardKey.keyP);
    held.add(PhysicalKeyboardKey.keyP);
    expect(held.contains(PhysicalKeyboardKey.keyP), isTrue);

    await tester.sendKeyUpEvent(LogicalKeyboardKey.keyP);
    await tester.pump();
    expect(
      held.contains(PhysicalKeyboardKey.keyP),
      isFalse,
      reason: 'the release landed nowhere, and still ends the hold',
    );
  });

  testWidgets('the_release_handler_goes_with_the_scope', (
    WidgetTester tester,
  ) async {
    final ProviderContainer container = ProviderContainer();
    final HeldChordKeys held = container.read(heldChordKeysProvider);
    container.dispose();

    await tester.sendKeyDownEvent(LogicalKeyboardKey.keyP);
    held.add(PhysicalKeyboardKey.keyP);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.keyP);
    await tester.pump();
    expect(
      held.contains(PhysicalKeyboardKey.keyP),
      isTrue,
      reason: 'a disposed scope no longer listens to the keyboard',
    );
  });
}
