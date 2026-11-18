// Die Einstellungen zeichnen, was der Katalog des Daemons sagt (HUM-534):
// Kategorien und Zeilen samt Reihenfolge und Namen kommen aus ihm, ein
// gesperrter Wert ist nicht bedienbar und nennt die Vorgabe.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/settings/registry/settings_registry.dart';

import '../../harness/settings_harness.dart';
import 'settings_overlay_test.dart' show everything, open, page;

/// Ein Daemon mit einem eigenen, kleinen Katalog: zwei Kategorien in
/// umgekehrter alphabetischer Reihenfolge, in der ersten zwei Zeilen ohne
/// eigenen Bauer der App.
class SmallCatalogClient extends FakeDaemonClient {
  SmallCatalogClient() : super(info: FakeDaemonClient.disposableInfo) {
    configValues = const ConfigValues(
      values: <String, Object?>{
        'ui': <String, Object?>{
          'theme': 'dark',
          'notifications': true,
          'future_text': 'old',
        },
      },
    );
  }

  @override
  Future<SettingsCatalog> getSettingsCatalog({String language = ''}) async =>
      const SettingsCatalog(
        categories: <SettingsCatalogCategory>[
          SettingsCatalogCategory(id: 'zulu', order: 0, title: 'Zulu first'),
          SettingsCatalogCategory(id: 'alpha', order: 1, title: 'Alpha last'),
        ],
        entries: <SettingsCatalogEntry>[
          SettingsCatalogEntry(
            id: 'ui.future_text',
            category: 'zulu',
            order: 2,
            title: 'Text from the catalog',
            control: SettingControl.text,
          ),
          SettingsCatalogEntry(
            id: 'alpha.only',
            category: 'alpha',
            order: 0,
            title: 'Only row of alpha',
            control: SettingControl.toggle,
          ),
          SettingsCatalogEntry(
            id: 'ui.future_switch',
            category: 'zulu',
            order: 1,
            title: 'Second from the catalog',
            control: SettingControl.toggle,
          ),
          SettingsCatalogEntry(
            id: 'ui.future_choice',
            category: 'zulu',
            order: 0,
            title: 'First from the catalog',
            help: 'Help that only the catalog knows.',
            control: SettingControl.choice,
            choices: <SettingsChoice>[
              SettingsChoice(value: 'one', title: 'Choice one'),
              SettingsChoice(value: 'two', title: 'Choice two'),
            ],
          ),
        ],
      );
}

/// A daemon whose first catalog answer fails.
class FailingOnceCatalogClient extends FakeDaemonClient {
  FailingOnceCatalogClient() : super(info: FakeDaemonClient.disposableInfo) {
    configValues = everything().configValues;
  }

  int calls = 0;

  @override
  Future<SettingsCatalog> getSettingsCatalog({String language = ''}) async {
    calls++;
    if (calls == 1) {
      throw const DaemonException(
        Diagnostic(code: 'IPC_006', severity: Severity.error, why: 'not yet'),
      );
    }
    return super.getSettingsCatalog(language: language);
  }
}

void main() {
  final TargetPlatformVariant linux = TargetPlatformVariant.only(
    TargetPlatform.linux,
  );

  testWidgets('categories and rows come from the catalog, in its order', (
    WidgetTester tester,
  ) async {
    await open(tester, client: SmallCatalogClient());

    final double zulu = tester
        .getTopLeft(find.byKey(const ValueKey<String>('settings-nav-zulu')))
        .dy;
    final double alpha = tester
        .getTopLeft(find.byKey(const ValueKey<String>('settings-nav-alpha')))
        .dy;
    expect(zulu, lessThan(alpha), reason: 'the order of the catalog');
    expect(find.text('Zulu first'), findsWidgets);
    expect(find.text('Alpha last'), findsOneWidget);
    // Keine Kategorie, die nur die App kennt.
    expect(
      find.byKey(const ValueKey<String>('settings-nav-models')),
      findsNothing,
    );

    expect(page('zulu'), findsOneWidget);
    final double first = tester
        .getTopLeft(find.text('First from the catalog'))
        .dy;
    final double second = tester
        .getTopLeft(find.text('Second from the catalog'))
        .dy;
    expect(first, lessThan(second), reason: 'the page order of the catalog');
    expect(find.text('Help that only the catalog knows.'), findsOneWidget);
    expect(find.text('Choice two'), findsOneWidget);

    await showSettingsCategory(tester, 'alpha');
    expect(find.text('Only row of alpha'), findsOneWidget);
    expect(tester.takeException(), isNull);
  }, variant: linux);

  // Mutationsbeweis: Prüfte `SettingRow` die Sperre nicht, stünde der Satz
  // nicht da, und der Klick auf „Light" schriebe über `SetConfig`.
  testWidgets('a locked value is read only and names the policy', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = everything()
      ..policyLocks[FormKeys.uiTheme] = '/etc/humanitl/policy.toml';
    await open(tester, client: client);
    await showSettingsCategory(tester, SettingsCategory.interface);

    expect(
      find.text(
        'Fixed by the company policy /etc/humanitl/policy.toml; read only.',
      ),
      findsOneWidget,
    );
    final Finder light = find.descendant(
      of: find.byKey(const Key('settings-theme')),
      matching: find.text('Light'),
    );
    await tester.ensureVisible(light);
    await tester.tap(light, warnIfMissed: false);
    await tester.pumpAndSettle();
    expect(client.configWrites, isEmpty, reason: 'nothing was written');

    // Eine Zeile ohne Sperre daneben bleibt bedienbar.
    expect(
      find.byKey(ValueKey<String>('setting-lock-${FormKeys.uiNotifications}')),
      findsNothing,
    );
    expect(tester.takeException(), isNull);
  }, variant: linux);

  testWidgets('a ConfigChanged of another client reloads the rows', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = everything();
    await open(tester, client: client);
    await showSettingsCategory(tester, SettingsCategory.interface);
    expect(
      find.byKey(ValueKey<String>('setting-lock-${FormKeys.uiTheme}')),
      findsNothing,
    );
    // Ein anderer Client legt die Sperre; der Daemon meldet die Änderung.
    client.policyLocks[FormKeys.uiTheme] = '/etc/humanitl/policy.toml';
    await client.setConfig(FormKeys.uiNotifications, 'false');
    await tester.pumpAndSettle();
    expect(
      find.byKey(ValueKey<String>('setting-lock-${FormKeys.uiTheme}')),
      findsOneWidget,
    );
  }, variant: linux);

  // Review HUM-534. Mutationsbeweis: Ohne `SettingLockGuard` um die Server
  // stünde weder der Satz da noch die Sperre der Bedienung.
  testWidgets('the model servers are read only under a lock', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = everything()
      ..policyLocks[SettingIds.llmServers] = '/etc/humanitl/policy.toml';
    await open(tester, client: client);
    await showSettingsCategory(tester, SettingsCategory.models);
    expect(
      find.byKey(ValueKey<String>('setting-lock-${SettingIds.llmServers}')),
      findsOneWidget,
    );
    expect(
      find.byKey(ValueKey<String>('setting-locked-${SettingIds.llmServers}')),
      findsOneWidget,
    );
    expect(tester.takeException(), isNull);
  }, variant: linux);

  // Review HUM-534. Mutationsbeweis: Ohne `didUpdateWidget` bliebe „old" im
  // Feld, und ein späteres Verlassen schriebe es zurück.
  testWidgets('a generic text field follows a config_changed', (
    WidgetTester tester,
  ) async {
    final SmallCatalogClient client = SmallCatalogClient();
    await open(tester, client: client);
    EditableText field() => tester.widget<EditableText>(
      find.descendant(
        of: find.byKey(
          const ValueKey<String>('settings-generic-ui.future_text'),
        ),
        matching: find.byType(EditableText),
      ),
    );
    expect(field().controller.text, 'old');
    client.configValues = const ConfigValues(
      values: <String, Object?>{
        'ui': <String, Object?>{'future_text': 'new'},
      },
    );
    await client.setConfig(FormKeys.uiNotifications, 'false');
    await tester.pumpAndSettle();
    expect(field().controller.text, 'new');
  }, variant: linux);

  testWidgets('a failed catalog answer offers to ask again', (
    WidgetTester tester,
  ) async {
    final FailingOnceCatalogClient client = FailingOnceCatalogClient();
    await open(tester, client: client);
    expect(find.byKey(const Key('settings-load-retry')), findsOneWidget);
    await tester.tap(find.byKey(const Key('settings-load-retry')));
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('settings-load-retry')), findsNothing);
    expect(page(SettingsCategory.models), findsOneWidget);
  }, variant: linux);

  // Review HUM-534, Runde 2: `llm.active` hat keine Zeile, sperrt aber die
  // Server; die Zeile der Server sagt es und nimmt keine Bedienung an.
  testWidgets('a lock on llm.active makes the server rows read only', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = everything()
      ..policyLocks['llm.active'] = '/etc/humanitl/policy.toml';
    await open(tester, client: client);
    await showSettingsCategory(tester, SettingsCategory.models);
    expect(
      find.byKey(ValueKey<String>('setting-locked-${SettingIds.llmServers}')),
      findsOneWidget,
    );
    expect(
      find.text(
        'Fixed by the company policy /etc/humanitl/policy.toml; read only.',
      ),
      findsWidgets,
    );
  }, variant: linux);
}
