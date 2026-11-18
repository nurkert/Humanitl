// Der Hinweis auf Cloud-Inferenz beim Wählen des Agenten (HUM-516): Ergibt
// der Plan des Daemons einen Anbieter außerhalb dieses Rechners, steht unter
// der Wahl des Agenten einmal der ganze Text; bei einem lokalen Endpunkt
// nichts. Entschieden hat das der Daemon, nicht die Oberfläche.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';

import '../../harness/settings_harness.dart';
import 'settings_sheet_test.dart' show configured, openSettings;

void main() {
  testWidgets('a cloud provider shows the note under the agent choice', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = configured();
    client.sandbox = client.sandbox.copyWith(
      cloudInference: const CloudInference(
        agent: 'opencode',
        provider: 'api.openai.com',
      ),
    );
    await openSettings(tester, client);
    await showSettingsCategory(tester, SettingsCategory.agent);
    final Finder note = find.byKey(const Key('cloud-inference-note'));
    expect(note, findsOneWidget);
    expect(
      find.descendant(
        of: note,
        matching: find.textContaining('api.openai.com'),
      ),
      findsOneWidget,
    );
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('a local endpoint shows no note', (WidgetTester tester) async {
    await openSettings(tester, configured());
    await showSettingsCategory(tester, SettingsCategory.agent);
    expect(find.byKey(const Key('cloud-inference-note')), findsNothing);
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));
}
