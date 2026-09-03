import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/features/shell/providers/connection.dart';
import 'package:humanitl/main.dart';

void main() {
  testWidgets('app boots', (tester) async {
    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          daemonClientProvider.overrideWithValue(FakeDaemonClient()),
          connectionHeartbeatProvider.overrideWithValue(null),
        ],
        child: const HumanitlApp(),
      ),
    );
    await tester.pump();
    expect(find.byType(HumanitlApp), findsOneWidget);
    expect(tester.takeException(), isNull);
  });
}
