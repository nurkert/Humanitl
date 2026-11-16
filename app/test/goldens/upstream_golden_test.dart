// Goldens des Frage-Modus `upstream` (HUM-512): die Kategorie Anfragen mit
// „Proxy dahinter" als vierter Wahl, einmal gewählt mit eingetragenem
// Entscheidungs-Proxy und einmal ohne ihn, wo das Segment aus ist und der
// Satz darunter sagt, warum; dazu die Moduszeile im Isolations-Reiter unter
// der Zeile „Egress". Je in den vier Varianten aus
// `support/golden_variants.dart`.
//
// Erneuern mit `make goldens-update`.

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/sandbox/widgets/isolation_panel.dart';
import 'package:humanitl/features/settings/registry/setting_entry.dart';

import 'egress_golden_test.dart' show withProxy;
import 'settings_golden_test.dart' show category, configured, sheet;
import 'support/golden_variants.dart';

/// Derselbe Daemon wie in `egress_golden_test.dart`, mit `upstream`.
FakeDaemonClient upstreamChosen() {
  final FakeDaemonClient client = withProxy();
  final Map<String, Object?> values = client.configValues.values;
  client.configValues = ConfigValues(
    values: <String, Object?>{
      ...values,
      'hold': <String, Object?>{'timeout_secs': 300, 'ask_mode': 'upstream'},
    },
    configPath: client.configValues.configPath,
  );
  return client;
}

void main() {
  goldenVariants(
    'settings_ask_mode_upstream',
    constraints: const BoxConstraints.tightFor(width: 1100, height: 700),
    pumpBeforeTest: category(SettingsCategory.requests),
    builder: (GoldenVariant variant) => sheet(variant, upstreamChosen()),
  );

  goldenVariants(
    'settings_ask_mode_no_proxy',
    constraints: const BoxConstraints.tightFor(width: 1100, height: 700),
    pumpBeforeTest: category(SettingsCategory.requests),
    builder: (GoldenVariant variant) => sheet(variant, configured()),
  );

  goldenVariants(
    'isolation_ask_mode_upstream',
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
          askMode: 'upstream',
        ),
        onShowArgv: () {},
      ),
    ),
  );
}
