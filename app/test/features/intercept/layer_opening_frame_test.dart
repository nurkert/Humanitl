// Zwischen dem Öffnen einer Ebene und ihrem ersten Bild darf keine Taste
// mehr entscheiden (HUM-303, Befund Codex).
//
// „Regel anlegen" auf der Karte des Agenten öffnet ein Blatt. Bis das Blatt
// gebaut ist, vergeht ein Frame; ein `Enter` in diesem Frame ginge sonst an
// die Warteschlange, die den Fokus noch hält, und gäbe die ausgewählte
// Anfrage frei. Der Knopf nimmt der Warteschlange deshalb die Tastatur, bevor
// das Blatt entsteht (`HLayerScope.claimKeyboard`).

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/intercept/widgets/agent_ask_card.dart';
import 'package:humanitl/features/intercept/widgets/queue_pane.dart';

import 'agent_ask_card_test.dart' show askScript;
import 'harness.dart';

void main() {
  testWidgets('enter_before_the_rule_sheet_is_built_decides_nothing', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fakeDaemon(<ScriptedEvent>[
      ...arriveAt(held(1), const Duration(milliseconds: 50)),
      ...askScript(
        text: 'bitte pypi.org freischalten',
        suggestedHost: 'pypi.org',
        suggestedPath: '/simple/',
      ),
    ]);
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    expect(find.byType(AgentAskCard), findsOneWidget);

    // Fokus in die Warteschlange, Auswahl scharf (`HMotion.rearm`).
    await tester.tap(find.byType(QueuePane));
    await tester.pump(const Duration(milliseconds: 500));
    await tester.pump();

    // Klick auf „Regel anlegen" und sofort `Enter`, ohne dass ein Frame
    // dazwischen das Blatt gebaut hätte.
    await tester.tap(find.byKey(const Key('intercept-agent-ask-open-rule')));
    expect(find.byType(AgentAskRuleSheet), findsNothing, reason: 'not built');
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    await tester.pump();
    expect(client.decisions, isEmpty, reason: 'Enter decided nothing');
    expect(find.byType(AgentAskRuleSheet), findsOneWidget);
  });

  /// Gegenprobe: Derselbe Aufbau entscheidet mit `Enter`, solange keine Ebene
  /// aufgeht. Sonst wäre der Test oben auch ohne den Griff grün.
  testWidgets('without_a_layer_the_same_enter_allows', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fakeDaemon(<ScriptedEvent>[
      ...arriveAt(held(1), const Duration(milliseconds: 50)),
      ...askScript(text: 'bitte pypi.org freischalten'),
    ]);
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await tester.tap(find.byType(QueuePane));
    await tester.pump(const Duration(milliseconds: 500));
    await tester.pump();

    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    await tester.pump();
    expect(client.decisions, hasLength(1));
  });
}
