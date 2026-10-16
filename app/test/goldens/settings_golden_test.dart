// Goldens des Einstellungsformulars (HUM-069 mit HUM-140, HUM-054): das
// Formular mit gespeichertem Schlüssel, das Ergebnis von „Prüfen" mit den
// gefundenen Modellen und ein Wert, den der Daemon zurückweist. Je in den
// vier Varianten aus `support/golden_variants.dart`.
//
// Die drei Namen der Spezifikation (`settings_group_with_expert`,
// `settings_search_results`, `settings_env_overridden`) gehören zum Formular
// aus dem Schema, das die zweite Umfangsentscheidung vom 2026-09-18 hinter das
// MVP verschoben hat; siehe `support/golden_names.dart`.
//
// Erneuern mit `make goldens-update`.

import 'package:flutter/widgets.dart';
// `Override` lebt in riverpod 3 im Nebeneingang `misc.dart`.
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/settings/settings_sheet.dart';

import 'support/golden_variants.dart';

/// Ein Daemon mit Endpunkt, Modell und einem Schlüssel im Schlüsselbund.
FakeDaemonClient configured() => FakeDaemonClient.empty()
  ..configValues = const ConfigValues(
    values: <String, Object?>{
      'hold': <String, Object?>{'timeout_secs': 300, 'ask_mode': 'ui'},
      'ui': <String, Object?>{
        'language': 'en',
        'theme': 'dark',
        'notifications': true,
      },
      'sandbox': <String, Object?>{'work_dir': '/home/u/src/acme'},
      'llm': <String, Object?>{
        'endpoint': 'http://192.168.1.20:11434/',
        'models': <Object?>['qwen2.5-coder:14b'],
      },
    },
    configPath: '/home/u/.config/humanitl/config.toml',
  )
  ..llmKey = const LlmKeyStatus(
    set: true,
    store: LlmKeyStore.secretService,
    keyRef: 'secret-service:humanitl/llm',
  )
  ..llmProbe = const LlmProbe(
    endpoint: '',
    models: <String>['gpt-oss:20b', 'phi-4', 'qwen2.5-coder:14b'],
    flavor: LlmFlavor.openAiCompatible,
    endpointIsPrivate: true,
  );

/// Das Blatt über [client], rechts im Bild wie in der Shell.
Widget sheet(GoldenVariant variant, FakeDaemonClient client) => goldenFrame(
  variant: variant,
  background: variant.tokens.colors.bg0,
  overrides: <Override>[daemonClientProvider.overrideWithValue(client)],
  child: Align(
    alignment: Alignment.centerRight,
    child: SettingsSheet(onClose: () {}),
  ),
);

/// Lässt Konfiguration und Schlüsselstatus ankommen.
Future<void> settleSheet(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
  await tester.pumpAndSettle();
}

/// Dasselbe, danach ein Druck auf „Prüfen".
Future<void> probe(WidgetTester tester) async {
  await settleSheet(tester);
  await tester.tap(find.byKey(const Key('settings-check')));
  await tester.pumpAndSettle();
}

/// Dasselbe, danach eine Frist von null Sekunden, die der Daemon abweist.
Future<void> refuseTimeout(WidgetTester tester) async {
  await settleSheet(tester);
  await tester.enterText(find.byKey(const Key('settings-timeout')), '0');
  await tester.testTextInput.receiveAction(TextInputAction.done);
  await tester.pumpAndSettle();
}

void main() {
  const BoxConstraints window = BoxConstraints.tightFor(
    width: 560,
    height: 1100,
  );

  goldenVariants(
    'settings_form',
    constraints: window,
    pumpBeforeTest: settleSheet,
    builder: (GoldenVariant variant) => sheet(variant, configured()),
  );

  goldenVariants(
    'settings_probe_result',
    constraints: window,
    pumpBeforeTest: probe,
    builder: (GoldenVariant variant) => sheet(variant, configured()),
  );

  goldenVariants(
    'settings_refusal',
    constraints: window,
    pumpBeforeTest: refuseTimeout,
    builder: (GoldenVariant variant) => sheet(variant, configured()),
  );
}
