// Goldens der gespeicherten Modellserver (HUM-289, frisch gedacht in
// HUM-405, Liste und Detailansicht seit HUM-414): die Liste mit dem Standard
// daneben und „Wer nutzt welchen Server", die Detailansicht eines Servers
// mit ungespeicherter Änderung, ein neuer Server, die Rückfrage vor dem
// Entfernen eines Servers, der Schnellwechsel der
// Statusleiste mit offenem Menü und die eine Rückfrage vor dem Start, wenn
// das Projekt einen unbestätigten Server wählt. Je in den vier Varianten aus
// `support/golden_variants.dart`.
//
// Erneuern mit `make goldens-update`.

import 'package:flutter/widgets.dart';
// `Override` lebt in riverpod 3 im Nebeneingang `misc.dart`.
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/sandbox/widgets/server_confirm_dialog.dart';
import 'package:humanitl/features/shell/widgets/llm_server_item.dart';

import 'settings_golden_test.dart' show configured, settleSheet, sheet;
import 'support/golden_variants.dart';

/// Drei gespeicherte Server, `home` Standard mit Schlüssel, `cloud` ohne,
/// `work` mit eigenem; ein Projekt, das `work` wählt, und ein Profil.
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
    const SavedServer(
      name: 'work',
      endpoint: 'https://llm.corp.example:8443/v1',
      models: <String>['llama-3.3-70b'],
      keyRef: 'secret-service:humanitl/work',
    ),
  ]
  ..activeServer = 'home'
  ..profileUses = const <ProfileUse>[
    ProfileUse(
      name: 'llm-only',
      server: 'cloud',
      description:
          'Pure inference. Only the configured LLM endpoint is reachable.',
    ),
  ]
  ..projectUse = const ProjectUse(dir: '/home/u/src/acme', server: 'work');

/// Das Overlay mit der Liste und dem Standard daneben.
Future<void> showServers(WidgetTester tester) async {
  await settleSheet(tester);
}

/// Die Detailansicht von [name].
Future<void> openServer(WidgetTester tester, String name) async {
  await settleSheet(tester);
  await tester.tap(find.byKey(ValueKey<String>('settings-server-$name')));
  await tester.pumpAndSettle();
}

/// Die Detailansicht von `cloud` mit einer geänderten Adresse.
Future<void> editCloud(WidgetTester tester) async {
  await openServer(tester, 'cloud');
  await tester.enterText(
    find.descendant(
      of: find.byKey(const Key('settings-endpoint')),
      matching: find.byType(EditableText),
    ),
    'https://api.example.com/v2',
  );
  await tester.pumpAndSettle();
}

/// Die Detailansicht eines neuen Servers.
Future<void> addServer(WidgetTester tester) async {
  await settleSheet(tester);
  await tester.tap(find.byKey(const Key('settings-server-add')));
  await tester.pumpAndSettle();
}

/// Die Detailansicht von `cloud` und die Rückfrage vor dem Entfernen.
Future<void> askRemove(WidgetTester tester) async {
  await openServer(tester, 'cloud');
  final Finder remove = find.byKey(const Key('settings-server-remove'));
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
  // Ein Fenster der Shell: Das Overlay (HUM-404) bedeckt drei Viertel davon.
  const BoxConstraints window = BoxConstraints.tightFor(
    width: 1100,
    height: 1000,
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
    'settings_server_detail',
    constraints: window,
    pumpBeforeTest: editCloud,
    builder: (GoldenVariant variant) => sheet(variant, twoServers()),
  );

  goldenVariants(
    'settings_server_new',
    constraints: window,
    pumpBeforeTest: addServer,
    builder: (GoldenVariant variant) => sheet(variant, twoServers()),
  );

  goldenVariants(
    'sandbox_server_confirm',
    constraints: const BoxConstraints.tightFor(width: 720, height: 420),
    pumpBeforeTest: (WidgetTester tester) async {
      await tester.pump();
      await tester.pumpAndSettle();
    },
    builder: (GoldenVariant variant) => goldenFrame(
      variant: variant,
      background: variant.tokens.colors.bg0,
      overrides: <Override>[
        daemonClientProvider.overrideWithValue(twoServers()),
      ],
      child: ServerConfirmDialog(
        server: const LlmServerChoice(
          name: 'work',
          source: LlmServerSource.project,
          detail: '/home/u/src/acme/.humanitl/profile.toml',
          projectDir: '/home/u/src/acme',
          unconfirmed: Diagnostic(code: 'CONFIG_022', severity: Severity.error),
        ),
        onCancel: () {},
        onConfirm: () {},
      ),
    ),
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
