// "Start agent" in der Einrichtung geht durch dieselbe eine Frage zur
// Cloud-Inferenz wie der Kopf des Sandbox-Bildschirms (HUM-516, Review).
// Ohne sie lehnte der Daemon den Start mit `LLM_011` ab; der Fake tut
// dasselbe.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';

import '../../harness/app_harness.dart';
import 'setup_test.dart' show startEnabled;

FakeDaemonClient cloudSetup() {
  final FakeDaemonClient client = FakeDaemonClient()
    ..doctorReport = fakeDoctorOk();
  client.sandbox = client.sandbox.copyWith(
    cloudInference: const CloudInference(
      agent: 'opencode',
      provider: 'api.anthropic.com',
    ),
  );
  return client;
}

void main() {
  testWidgets('Start agent asks once; Cancel starts nothing', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = cloudSetup();
    await pumpApp(tester, client: client);
    await pressCtrl(tester, LogicalKeyboardKey.digit6);
    await tester.pump();
    expect(startEnabled(tester), isTrue);

    await tester.tap(find.byKey(const Key('setup-start')));
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('sandbox-cloud-confirm')), findsOneWidget);
    expect(client.startModes, isEmpty, reason: 'nothing started yet');

    await tester.tap(find.byKey(const Key('sandbox-cloud-confirm-cancel')));
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('sandbox-cloud-confirm')), findsNothing);
    expect(client.startModes, isEmpty);
    expect(client.cloudAcknowledgements, isEmpty);
  });

  testWidgets('Start agent confirmed is kept by the daemon and starts', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = cloudSetup();
    await pumpApp(tester, client: client);
    await pressCtrl(tester, LogicalKeyboardKey.digit6);
    await tester.pump();

    await tester.tap(find.byKey(const Key('setup-start')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('sandbox-cloud-confirm-yes')));
    await tester.pumpAndSettle();
    expect(client.cloudAcknowledgements, <String>[
      'opencode api.anthropic.com',
    ]);
    expect(client.startModes, hasLength(1));
    expect(
      client.sandbox.state,
      SandboxState.running,
      reason: 'the daemon gate let the confirmed start through',
    );
  });
}
