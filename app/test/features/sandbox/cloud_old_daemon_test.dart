// Gegen einen Daemon vor Vertrag 1.43 startet die App keine Sitzung
// (HUM-516, Review): Er kennt weder den Hinweis auf Cloud-Inferenz noch
// dessen Tor. Der Start sagt, wie er neu startet, und startet nichts.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';

import 'harness.dart';

const DaemonInfo before143 = DaemonInfo(
  daemonVersion: '0.0.9',
  protoMajor: 1,
  protoMinor: 42,
  capabilities: <String>['sandbox.run'],
  sessionId: '018f0001-0000-7000-8000-000000000001',
);

void main() {
  test('only a daemon from contract 1.43 knows cloud inference', () {
    expect(before143.knowsCloudInference, isFalse);
    expect(before143.copyWith(protoMinor: 43).knowsCloudInference, isTrue);
    expect(FakeDaemonClient.defaultInfo.knowsCloudInference, isTrue);
  });

  testWidgets('a start against a daemon before 1.43 is refused with the '
      'restart', (WidgetTester tester) async {
    final SandboxTestClient client = SandboxTestClient(info: before143);
    await pumpSandbox(tester, client: client);

    await tester.tap(find.byKey(const Key('sandbox-start')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 600));
    expect(client.startModes, isEmpty, reason: 'nothing started');
    final SandboxStatus status = statusOf(tester);
    final Diagnostic refused = status.diagnostics.last;
    expect(refused.code, 'CLI_004');
    expect(refused.why, contains('1.42'));
    expect(
      refused.fix,
      const FixAction.copyCommand(
        command: 'systemctl --user restart humanitld.service',
      ),
    );
  });
}
