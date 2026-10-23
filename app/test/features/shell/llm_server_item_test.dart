// Der Schnellwechsel des Modellservers in der Statusleiste (HUM-289): nur mit
// einer Wahl sichtbar, und ein Klick auf einen anderen Server aktiviert ihn
// für die nächste Sitzung.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/shell/widgets/llm_server_item.dart';

import '../../harness/app_harness.dart';

FakeDaemonClient withServers(List<String> names) => FakeDaemonClient.empty()
  ..savedServers = <SavedServer>[
    for (final String name in names)
      SavedServer(name: name, endpoint: 'http://$name.lan:11434/'),
  ]
  ..activeServer = names.isEmpty ? '' : names.first;

void main() {
  testWidgets('two saved servers: the item names the active one and switches', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = withServers(<String>['home', 'cloud'])
      ..savedServers.sort(
        (SavedServer a, SavedServer b) => a.name.compareTo(b.name),
      );
    await pumpApp(tester, client: client);
    await tester.pumpAndSettle();

    final Finder item = find.descendant(
      of: find.byType(LlmServerItem),
      matching: find.text('LLM home'),
    );
    expect(item, findsOneWidget);

    await tester.tap(item);
    await tester.pumpAndSettle();
    expect(find.text('home (active)'), findsOneWidget);

    await tester.tap(find.text('cloud'));
    await tester.pumpAndSettle();
    expect(client.serverRequests, <String>['activate cloud']);
    expect(client.activeServer, 'cloud');
    expect(find.text('LLM cloud'), findsOneWidget);
  });

  testWidgets('one saved server or none: nothing to switch, nothing drawn', (
    WidgetTester tester,
  ) async {
    await pumpApp(tester, client: withServers(<String>['home']));
    await tester.pumpAndSettle();
    expect(find.text('LLM home'), findsNothing);
    expect(
      find.descendant(
        of: find.byType(LlmServerItem),
        matching: find.byType(Text),
      ),
      findsNothing,
    );
  });
}
