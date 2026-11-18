// Goldens des Entscheidungs-Proxys (HUM-511): die Kategorie in den
// Einstellungen mit Adresse, CA, Auflösung, abgelegten Zugangsdaten, dem
// Hinweis auf den Neustart und einer gelungenen Prüfung; dazu die Zeile
// „Egress" im Isolations-Reiter. Je in den vier Varianten aus
// `support/golden_variants.dart`.
//
// Erneuern mit `make goldens-update`.

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/sandbox/widgets/isolation_panel.dart';

import '../harness/settings_harness.dart' show SettingsCategory;

import 'settings_golden_test.dart' show category, configured, sheet;
import 'support/golden_variants.dart';

/// Ein Daemon, in dessen Datei ein Entscheidungs-Proxy steht, der noch
/// direkt hinausgeht, bis er neu startet.
FakeDaemonClient withProxy() {
  final FakeDaemonClient client = configured();
  client
    ..configValues = ConfigValues(
      values: <String, Object?>{
        ...client.configValues.values,
        'egress': <String, Object?>{
          'proxy': 'http://127.0.0.1:8080',
          'proxy_ca': '/home/u/.mitmproxy/mitmproxy-ca-cert.pem',
          'proxy_resolve': 'proxy',
        },
      },
      configPath: client.configValues.configPath,
    )
    ..egressStatus = const EgressStatus(
      configured: EgressRoute(proxy: '127.0.0.1:8080', resolve: 'proxy'),
      proxyCa: '/home/u/.mitmproxy/mitmproxy-ca-cert.pem',
      authSet: true,
      authRef: 'secret-service:humanitl/egress-proxy',
    )
    ..egressCheckResult = const EgressStatus(
      configured: EgressRoute(proxy: '127.0.0.1:8080', resolve: 'proxy'),
      checkTarget: 'example.com:443',
      reachable: true,
    );
  return client;
}

/// Die Kategorie, danach ein Druck auf „Verbindung prüfen".
Future<void> checked(WidgetTester tester) async {
  await category(SettingsCategory.egress)(tester);
  final Finder button = find.byKey(const Key('settings-egress-check-now'));
  await tester.ensureVisible(button);
  await tester.pumpAndSettle();
  await tester.tap(button);
  await tester.pumpAndSettle();
}

void main() {
  goldenVariants(
    'settings_egress',
    constraints: const BoxConstraints.tightFor(width: 1100, height: 900),
    pumpBeforeTest: checked,
    builder: (GoldenVariant variant) => sheet(variant, withProxy()),
  );

  goldenVariants(
    'isolation_egress',
    constraints: const BoxConstraints.tightFor(width: 900, height: 760),
    builder: (GoldenVariant variant) => goldenFrame(
      variant: variant,
      background: variant.tokens.colors.bg0,
      overrides: <Override>[
        daemonClientProvider.overrideWithValue(FakeDaemonClient.empty()),
      ],
      child: IsolationPanel(
        status: const SandboxStatus(
          llmEndpoint: 'http://192.168.1.20:11434/',
          egress: EgressRoute(proxy: '127.0.0.1:8080', resolve: 'proxy'),
        ),
        onShowArgv: () {},
      ),
    ),
  );
}
