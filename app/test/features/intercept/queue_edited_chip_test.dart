// Der Chip „Edited“ im Queue-Abgang (HUM-047): Nach einer bearbeiteten
// Freigabe trägt die entschiedene Zeile das Wort, nicht nur Farbe und Glyph.
// Gefahren über den Fake-Daemon und den Entscheidungsweg der Oberfläche, nicht
// über eine von Hand gebaute Zeile: Gemessen ist, dass `allowEdited` bis in
// den Streifen durchkommt.

import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/edited_badge.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/providers/decision.dart';

import 'fixtures.dart';
import 'harness.dart';

/// Hält einen Flow an, entscheidet ihn mit [decision] über denselben Notifier
/// wie Aktionsleiste und Editor und liefert den Streifen seiner Zeile.
Future<Finder> _decideAndFindStrip(
  WidgetTester tester,
  Decision decision,
) async {
  final FlowDetail detail = held(1);
  final FakeDaemonClient client = fakeDaemon(holdScript(<FlowDetail>[detail]));
  await pumpIntercept(tester, client: client);
  await playScript(tester);

  final FlowId id = detail.summary.id;
  await containerOf(tester)
      .read(interceptDecisionProvider.notifier)
      .send(id, decision);
  await tester.pump();
  await tester.pump();

  expect(client.decisions.single.decision, decision);
  final Finder strip = find.byKey(Key('queue-strip-${id.value}'));
  expect(strip, findsOneWidget, reason: 'the decided row stands as a strip');
  return strip;
}

void main() {
  testWidgets('an edited send leaves the Edited chip in the queue strip', (
    WidgetTester tester,
  ) async {
    final Finder strip = await _decideAndFindStrip(
      tester,
      const Decision.allowEdited(
        request: EditedRequest(
          method: Method.get,
          url: 'https://registry.npmjs.org/react',
          body: <int>[],
        ),
      ),
    );

    final Finder chip = find.descendant(
      of: strip,
      matching: find.byType(EditedBadge),
    );
    expect(chip, findsOneWidget);
    expect(
      find.descendant(of: chip, matching: find.text('Edited')),
      findsOneWidget,
      reason: 'the chip says the word, not only the colour',
    );
    // Die Farbe ist die des Zustands, in dem die Zeile gezeichnet wird: Der
    // Chip ist das `HBadge` der Findings-Chips, kein eigenes Bauteil.
    final HBadge badge = tester.widget<HBadge>(
      find.descendant(of: chip, matching: find.byType(HBadge)),
    );
    final HTokens tokens = HTheme.of(tester.element(chip));
    expect(badge.color, tokens.state.allowedEdited);
    // Das Wort steht in der Textfarbe des Zustands, nicht in `fg2`: Die
    // Zeile des Abgangs hat kein `onTap`, und ihr `Clickable` reichte
    // `disabled` an den Badge weiter, bis `HBadge` eine Grenze zog.
    final RenderParagraph word = tester.renderObject<RenderParagraph>(
      find.descendant(of: chip, matching: find.text('Edited')),
    );
    expect(word.text.style?.color, tokens.stateText.allowedEdited);
    expect(word.text.style?.color, isNot(tokens.colors.fg2));
  });

  testWidgets('a plain allow leaves no Edited chip', (
    WidgetTester tester,
  ) async {
    final Finder strip = await _decideAndFindStrip(
      tester,
      const Decision.allow(),
    );
    expect(
      find.descendant(of: strip, matching: find.byType(EditedBadge)),
      findsNothing,
      reason: 'only an edited request says it was edited',
    );
    expect(find.text('Edited'), findsNothing);
  });
}
