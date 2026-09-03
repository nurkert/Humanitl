// Integrationstest (HUM-019): die App startet gegen den FakeDaemonClient,
// die Rail ist sichtbar, drei Sekunden lang fliegt keine Exception.
//
// Läuft auf dem Linux-Desktop: `flutter test integration_test -d linux`,
// in CI unter `xvfb-run -a`.

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/app.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/features/shell/widgets/icon_rail.dart';
import 'package:humanitl/features/shell/widgets/status_bar.dart';
import 'package:integration_test/integration_test.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('the shell boots against the fake client', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      ProviderScope(
        overrides: [daemonClientProvider.overrideWithValue(FakeDaemonClient())],
        child: const HumanitlApp(),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.byType(IconRail), findsOneWidget);
    expect(find.byType(RailEntry), findsNWidgets(5));
    expect(find.byType(StatusBar), findsOneWidget);

    final Stopwatch clock = Stopwatch()..start();
    while (clock.elapsed < const Duration(seconds: 3)) {
      await tester.pump(const Duration(milliseconds: 100));
      expect(tester.takeException(), isNull);
    }
    expect(find.byType(IconRail), findsOneWidget);
  });
}
