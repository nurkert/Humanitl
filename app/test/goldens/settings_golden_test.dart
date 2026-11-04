// Goldens des Einstellungs-Overlays (HUM-069 mit HUM-140, HUM-054, als
// Overlay mit Kategorien und Suche seit HUM-404): die Kategorie Modelle mit
// gespeichertem Schlüssel, das Ergebnis von „Prüfen", ein Wert, den der
// Daemon zurückweist, die Kategorie Ansicht, Treffer der Suche und eine Suche
// ohne Treffer. Je in den vier Varianten aus `support/golden_variants.dart`.
//
// `settings_group_with_expert` und `settings_env_overridden` gehören zum
// Formular aus dem Schema, das die zweite Umfangsentscheidung vom 2026-09-18
// hinter das MVP verschoben hat; siehe `support/golden_names.dart`.
//
// Erneuern mit `make goldens-update`.

import 'package:flutter/widgets.dart';
// `Override` lebt in riverpod 3 im Nebeneingang `misc.dart`.
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/settings/registry/setting_entry.dart';
import 'package:humanitl/features/settings/settings_overlay.dart';

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
  )
  // Der Endpunkt als der eine gespeicherte Server, wie der Daemon eine
  // Konfiguration ohne Einträge listet (HUM-289, HUM-405).
  ..savedServers = const <SavedServer>[
    SavedServer(
      name: 'default',
      endpoint: 'http://192.168.1.20:11434/',
      models: <String>['qwen2.5-coder:14b'],
      keyRef: 'secret-service:humanitl/llm',
    ),
  ]
  ..activeServer = 'default';

/// Das Overlay über [client], über dem ganzen Bild wie in der Shell.
Widget sheet(GoldenVariant variant, FakeDaemonClient client) => goldenFrame(
  variant: variant,
  background: variant.tokens.colors.bg0,
  overrides: <Override>[daemonClientProvider.overrideWithValue(client)],
  child: SettingsOverlay(onClose: () {}),
);

/// Lässt Konfiguration und Schlüsselstatus ankommen und das Overlay
/// einfahren.
Future<void> settleSheet(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
  await tester.pumpAndSettle();
}

/// Dasselbe, danach die Detailansicht des Standards und ein Druck auf
/// „Prüfen" (HUM-405).
Future<void> probe(WidgetTester tester) async {
  await settleSheet(tester);
  await tester.tap(
    find.byKey(const ValueKey<String>('settings-server-default')),
  );
  await tester.pumpAndSettle();
  await tester.tap(find.byKey(const Key('settings-check')));
  await tester.pumpAndSettle();
}

/// Die Kategorie [category].
Future<void> Function(WidgetTester) category(SettingsCategory category) =>
    (WidgetTester tester) async {
      await settleSheet(tester);
      await tester.tap(
        find.byKey(ValueKey<String>('settings-nav-${category.name}')),
      );
      await tester.pumpAndSettle();
    };

/// Die Kategorie Anfragen, danach eine Frist von null Sekunden, die der
/// Daemon abweist.
Future<void> refuseTimeout(WidgetTester tester) async {
  await category(SettingsCategory.requests)(tester);
  await tester.enterText(find.byKey(const Key('settings-timeout')), '0');
  await tester.testTextInput.receiveAction(TextInputAction.done);
  await tester.pumpAndSettle();
}

/// [text] im Suchfeld.
Future<void> Function(WidgetTester) search(String text) =>
    (WidgetTester tester) async {
      await settleSheet(tester);
      await tester.enterText(
        find.descendant(
          of: find.byKey(const Key('settings-search')),
          matching: find.byType(EditableText),
        ),
        text,
      );
      await tester.pumpAndSettle();
    };

void main() {
  // Ein Fenster der Shell: Das Overlay bedeckt drei Viertel davon.
  const BoxConstraints window = BoxConstraints.tightFor(
    width: 1100,
    height: 900,
  );
  // Das kleinste Fenster der Aufgabe, für die Seiten mit wenig Inhalt.
  const BoxConstraints small = BoxConstraints.tightFor(width: 960, height: 600);

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
    constraints: small,
    pumpBeforeTest: refuseTimeout,
    builder: (GoldenVariant variant) => sheet(variant, configured()),
  );

  goldenVariants(
    'settings_category',
    constraints: small,
    pumpBeforeTest: category(SettingsCategory.interface),
    builder: (GoldenVariant variant) => sheet(variant, configured()),
  );

  // Die Tastenkürzel (HUM-420): Gruppen, Namen links, Kappen rechts.
  goldenVariants(
    'settings_shortcuts',
    constraints: window,
    pumpBeforeTest: category(SettingsCategory.shortcuts),
    builder: (GoldenVariant variant) => sheet(variant, configured()),
  );

  // „e" trifft in beiden Sprachen Zeilen mehrerer Kategorien.
  goldenVariants(
    'settings_search_results',
    constraints: small,
    pumpBeforeTest: search('e'),
    builder: (GoldenVariant variant) => sheet(variant, configured()),
  );

  goldenVariants(
    'settings_search_empty',
    constraints: small,
    pumpBeforeTest: search('zzqx'),
    builder: (GoldenVariant variant) => sheet(variant, configured()),
  );
}
