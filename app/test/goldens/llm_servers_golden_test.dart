// Goldens der gespeicherten Modellserver (HUM-289): die Liste im Abschnitt
// Modell der Einstellungen, die Rückfrage vor dem Entfernen des aktiven
// Servers und der Schnellwechsel der Statusleiste mit offenem Menü. Je in
// den vier Varianten aus `support/golden_variants.dart`.
//
// Erneuern mit `make goldens-update`.

import 'package:flutter/widgets.dart';
// `Override` lebt in riverpod 3 im Nebeneingang `misc.dart`.
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/shell/widgets/llm_server_item.dart';

import 'settings_golden_test.dart' show configured, settleSheet, sheet;
import 'support/golden_variants.dart';

/// Zwei gespeicherte Server, `home` aktiv mit Schlüssel, `cloud` ohne.
FakeDaemonClient twoServers() => configured()
  ..savedServers = <SavedServer>[
    const SavedServer(
      name: 'cloud',
      endpoint: 'https://api.example.com/v1',
      models: <String>['gpt-oss:20b'],
    ),
    const SavedServer(
      name: 'home',
      endpoint: 'http://192.168.1.20:11434/',
      models: <String>['qwen2.5-coder:14b'],
      keyRef: 'secret-service:humanitl/llm',
    ),
  ]
  ..activeServer = 'home';

/// Das Blatt, bis zur Liste gerollt.
Future<void> showServers(WidgetTester tester) async {
  await settleSheet(tester);
  await tester.ensureVisible(find.byKey(const Key('settings-servers')));
  await tester.pumpAndSettle();
}

/// Dasselbe, danach „Entfernen" am aktiven Server.
Future<void> askRemove(WidgetTester tester) async {
  await showServers(tester);
  final Finder remove = find.byKey(
    const ValueKey<String>('settings-server-remove-home'),
  );
  await tester.ensureVisible(remove);
  await tester.pumpAndSettle();
  await tester.tap(remove);
  await tester.pumpAndSettle();
}

/// Der Eintrag der Statusleiste, das Menü offen.
Future<void> openSwitch(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
  await tester.pumpAndSettle();
  await tester.tap(find.byType(LlmServerItem));
  await tester.pumpAndSettle();
}

void main() {
  const BoxConstraints window = BoxConstraints.tightFor(
    width: 560,
    height: 1100,
  );

  goldenVariants(
    'settings_servers',
    constraints: window,
    pumpBeforeTest: showServers,
    builder: (GoldenVariant variant) => sheet(variant, twoServers()),
  );

  goldenVariants(
    'settings_server_remove',
    constraints: window,
    pumpBeforeTest: askRemove,
    builder: (GoldenVariant variant) => sheet(variant, twoServers()),
  );

  goldenVariants(
    'status_llm_switch',
    constraints: const BoxConstraints.tightFor(width: 420, height: 200),
    pumpBeforeTest: openSwitch,
    builder: (GoldenVariant variant) => goldenFrame(
      variant: variant,
      overrides: <Override>[
        daemonClientProvider.overrideWithValue(twoServers()),
      ],
      child: Align(
        alignment: Alignment.bottomRight,
        child: SizedBox(
          height: variant.tokens.sizes.statusBar,
          child: const Row(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[LlmServerItem()],
          ),
        ),
      ),
    ),
  );
}
