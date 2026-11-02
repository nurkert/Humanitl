// Die Modellserver in der Kategorie Modelle (HUM-405, vorher HUM-289): Karten
// mit Standard, Host, Modell, Schlüssel und letzter Prüfung, eine
// Detailansicht je Server mit Speichern und Abbrechen, und „Wer nutzt welchen
// Server". Kein Schlüssel steht je in einem Text, nichts geht beim Tippen
// hinaus, und der Schlüssel eines Servers geht nur an dessen eigene Adresse.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/settings/widgets/servers/server_cards.dart';
import 'package:humanitl/features/settings/widgets/servers/server_detail.dart';

import 'settings_sheet_test.dart'
    show anyTextContains, configured, openSettings;

/// Ein Schlüssel, den kein Text des Bildschirms je zeigen darf.
const String _secret = 'sk-cloud-0123456789abcdef';

/// Zwei gespeicherte Server, `home` Standard mit Schlüssel, `cloud` mit
/// eigenem; ein Projekt und ein Profil.
FakeDaemonClient twoServers() {
  final FakeDaemonClient client = configured()
    ..savedServers = <SavedServer>[
      const SavedServer(
        name: 'cloud',
        endpoint: 'https://api.example.com/v1',
        models: <String>['gpt-oss:20b'],
        keyRef: 'secret-service:humanitl/cloud',
      ),
      const SavedServer(
        name: 'home',
        endpoint: 'http://192.168.1.20:11434/',
        models: <String>['qwen2.5-coder:14b'],
        keyRef: 'secret-service:humanitl/llm',
      ),
    ]
    ..activeServer = 'home'
    ..llmKey = const LlmKeyStatus(
      set: true,
      store: LlmKeyStore.secretService,
      keyRef: 'secret-service:humanitl/llm',
    )
    ..profileUses = <ProfileUse>[
      const ProfileUse(
        name: 'llm-only',
        builtin: true,
        description: 'Pure inference.',
      ),
    ]
    ..projectUse = const ProjectUse(dir: '/home/u/src/acme');
  client.serverKeys['cloud'] = const LlmKeyStatus(
    set: true,
    store: LlmKeyStore.secretService,
    keyRef: 'secret-service:humanitl/cloud',
  );
  return client;
}

Future<void> tapKey(WidgetTester tester, String key) async {
  final Finder target = find.byKey(ValueKey<String>(key));
  await tester.ensureVisible(target);
  await tester.pumpAndSettle();
  await tester.tap(target);
  await tester.pumpAndSettle();
}

Future<void> typeInto(WidgetTester tester, String key, String text) async {
  final Finder field = find.descendant(
    of: find.byKey(ValueKey<String>(key)),
    matching: find.byType(EditableText),
  );
  await tester.ensureVisible(field);
  await tester.enterText(field, text);
  await tester.pump();
}

/// Was im Feld für den Zugangsschlüssel steht.
String keyText(WidgetTester tester) => tester
    .widget<EditableText>(find.byKey(const Key('settings-key-input')))
    .controller
    .text;

String fieldText(WidgetTester tester, String key) => tester
    .widget<EditableText>(
      find.descendant(
        of: find.byKey(ValueKey<String>(key)),
        matching: find.byType(EditableText),
      ),
    )
    .controller
    .text;

/// Wählt im Auswahlfeld [picker] den Eintrag [label].
Future<void> pick(WidgetTester tester, String picker, String label) async {
  await tapKey(tester, picker);
  await tester.tap(find.text(label).last);
  await tester.pumpAndSettle();
}

void main() {
  final TargetPlatformVariant linux = TargetPlatformVariant.only(
    TargetPlatform.linux,
  );

  testWidgets(
    'the cards name host, model, key and the default with a star, and the '
    'add card stands last',
    (WidgetTester tester) async {
      await openSettings(tester, twoServers());

      expect(
        find.byKey(const ValueKey<String>('settings-server-star-home')),
        findsOneWidget,
      );
      expect(
        find.byKey(const ValueKey<String>('settings-server-star-cloud')),
        findsNothing,
      );
      expect(find.text('Default'), findsWidgets);
      expect(find.text('192.168.1.20:11434'), findsOneWidget);
      expect(find.text('api.example.com'), findsOneWidget);
      expect(find.text('gpt-oss:20b'), findsOneWidget);
      for (final String name in <String>['home', 'cloud']) {
        final Finder card = find.byKey(
          ValueKey<String>('settings-server-$name'),
        );
        expect(
          find.descendant(of: card, matching: find.text('Key stored')),
          findsOneWidget,
        );
        expect(
          find.descendant(of: card, matching: find.text('Not checked yet')),
          findsOneWidget,
        );
      }
      expect(find.byKey(const Key('settings-server-add')), findsOneWidget);
      // Der Standard steht vorn, die Karte zum Hinzufügen hinten.
      // In Leserichtung: erst die Zeile, dann die Spalte.
      double order(Finder card) {
        final Offset at = tester.getTopLeft(card);
        return at.dy * 10000 + at.dx;
      }

      final double home = order(
        find.byKey(const ValueKey<String>('settings-server-home')),
      );
      final double cloud = order(
        find.byKey(const ValueKey<String>('settings-server-cloud')),
      );
      final double add = order(find.byKey(const Key('settings-server-add')));
      expect(home < cloud && cloud < add, isTrue);
      // Die Felder des aktiven Servers stehen nicht mehr über den Karten.
      expect(find.byKey(const Key('settings-endpoint')), findsNothing);
    },
    variant: linux,
  );

  testWidgets(
    'a server that is not the default changes address and model in its '
    'detail view, nothing goes out while typing, and it stays not the '
    'default',
    (WidgetTester tester) async {
      final FakeDaemonClient client = twoServers();
      await openSettings(tester, client);

      await tapKey(tester, 'settings-server-cloud');
      expect(
        find.byKey(const ValueKey<String>('settings-server-detail-cloud')),
        findsOneWidget,
      );
      expect(
        fieldText(tester, 'settings-endpoint'),
        'https://api.example.com/v1',
      );
      await typeInto(tester, 'settings-endpoint', 'https://api.example.org/v2');
      await typeInto(tester, 'settings-model', 'gpt-oss:120b');
      await tester.pumpAndSettle();
      expect(client.serverRequests, isEmpty, reason: 'typing sends nothing');
      expect(client.probedEndpoints, isEmpty);
      expect(client.configWrites, isEmpty);
      expect(find.byKey(const Key('settings-server-unsaved')), findsOneWidget);

      await tapKey(tester, 'settings-server-save');
      expect(client.serverRequests, <String>['edit cloud']);
      final SavedServer cloud = client.savedServers.firstWhere(
        (SavedServer server) => server.name == 'cloud',
      );
      expect(cloud.endpoint, 'https://api.example.org/v2');
      expect(cloud.models, <String>['gpt-oss:120b', 'gpt-oss:20b']);
      expect(client.activeServer, 'home', reason: 'editing never switches');
      expect(client.configWrites, isEmpty, reason: 'no write of the flat view');
    },
    variant: linux,
  );

  testWidgets(
    'a key typed in the detail view of a server goes only to the origin of '
    'exactly this server',
    (WidgetTester tester) async {
      final FakeDaemonClient client = twoServers();
      await openSettings(tester, client);
      await tapKey(tester, 'settings-server-cloud');

      await tester.enterText(
        find.byKey(const Key('settings-key-input')),
        _secret,
      );
      await tester.pump();
      expect(anyTextContains(tester, _secret), isFalse);

      await tapKey(tester, 'settings-check');
      expect(client.probedEndpoints, <String>['https://api.example.com/v1']);
      expect(client.probedServers, <String>['cloud']);
      expect(client.probeKeyUses, <ProbeKeyUse>[ProbeKeyUse.typed]);
      expect(
        keyText(tester),
        isEmpty,
        reason: 'the typed key went out once and is gone',
      );

      // Ein Schlüssel für diese Adresse geht nicht an die nächste: Wer die
      // Adresse ändert, leert das Feld.
      await tester.enterText(
        find.byKey(const Key('settings-key-input')),
        _secret,
      );
      await typeInto(
        tester,
        'settings-endpoint',
        'https://evil.example.net/v1',
      );
      expect(keyText(tester), isEmpty);
      await typeInto(tester, 'settings-endpoint', 'https://api.example.com/v1');

      await tester.enterText(
        find.byKey(const Key('settings-key-input')),
        _secret,
      );
      await tester.pump();
      await tapKey(tester, 'settings-server-save');
      expect(client.setKeyLengths, <int>[_secret.length]);
      expect(client.setKeyServers, <String>[
        'cloud',
      ], reason: 'stored under this server, never under the default');
      expect(client.serverKeys['cloud']?.set, isTrue);
      expect(client.llmKey.keyRef, 'secret-service:humanitl/llm');
      expect(anyTextContains(tester, _secret), isFalse);
    },
    variant: linux,
  );

  testWidgets(
    'Check without a typed key asks for this server\'s stored key, and the '
    'card shows the result of this app session',
    (WidgetTester tester) async {
      final FakeDaemonClient client = twoServers();
      await openSettings(tester, client);
      await tapKey(tester, 'settings-server-cloud');
      await tapKey(tester, 'settings-check');
      expect(client.probedServers, <String>['cloud']);
      expect(client.probeKeyUses, <ProbeKeyUse>[ProbeKeyUse.stored]);

      await tapKey(tester, 'settings-server-back');
      final Finder line = find.byKey(
        const ValueKey<String>('settings-server-checked-cloud'),
      );
      expect(
        find.descendant(of: line, matching: find.textContaining('found')),
        findsOneWidget,
      );
      expect(
        find.descendant(
          of: find.byKey(
            const ValueKey<String>('settings-server-checked-home'),
          ),
          matching: find.text('Not checked yet'),
        ),
        findsOneWidget,
      );
    },
    variant: linux,
  );

  testWidgets('Cancel drops what was typed; "Make default" switches', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = twoServers();
    await openSettings(tester, client);
    await tapKey(tester, 'settings-server-cloud');
    await typeInto(tester, 'settings-endpoint', 'https://other.example/v1');
    await tapKey(tester, 'settings-server-cancel');
    expect(
      fieldText(tester, 'settings-endpoint'),
      'https://api.example.com/v1',
    );
    expect(client.serverRequests, isEmpty);

    await tapKey(tester, 'settings-server-make-default');
    expect(client.serverRequests, <String>['activate cloud']);
    expect(
      find.byKey(const Key('settings-server-detail-default')),
      findsOneWidget,
    );
    expect(find.byKey(const Key('settings-server-make-default')), findsNothing);
  }, variant: linux);

  testWidgets(
    'removing under More asks first, Cancel preselected, and names the next '
    'default',
    (WidgetTester tester) async {
      final FakeDaemonClient client = twoServers();
      await openSettings(tester, client);
      await tapKey(tester, 'settings-server-home');
      await tapKey(tester, 'settings-server-more');
      await tester.tap(find.text('Remove'));
      await tester.pumpAndSettle();
      expect(
        find.byKey(const Key('settings-server-remove-confirm')),
        findsOneWidget,
      );
      expect(find.text('cloud becomes the default server.'), findsOneWidget);
      final Element cancel = tester.element(
        find.byKey(const Key('settings-server-remove-cancel')),
      );
      expect(
        Focus.of(cancel).hasFocus || Focus.of(cancel).hasPrimaryFocus,
        isTrue,
        reason: 'Cancel is the preselected answer',
      );

      await tapKey(tester, 'settings-server-remove-cancel');
      expect(client.serverRequests, isEmpty);

      await tapKey(tester, 'settings-server-more');
      await tester.tap(find.text('Remove'));
      await tester.pumpAndSettle();
      await tapKey(tester, 'settings-server-remove-yes');
      expect(client.serverRequests, <String>[
        'remove home delete-key use cloud',
      ]);
      expect(
        find.byKey(const ValueKey<String>('settings-server-home')),
        findsNothing,
      );
    },
    variant: linux,
  );

  testWidgets('renaming under More keeps the view on the renamed server', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = twoServers();
    await openSettings(tester, client);
    await tapKey(tester, 'settings-server-cloud');
    await tapKey(tester, 'settings-server-more');
    await tester.tap(find.text('Rename'));
    await tester.pumpAndSettle();
    await typeInto(tester, 'settings-server-rename-field', 'Not A Name');
    await tapKey(tester, 'settings-server-rename-save');
    expect(client.serverRequests, isEmpty);
    expect(find.byKey(const Key('settings-server-refusal')), findsOneWidget);

    await typeInto(tester, 'settings-server-rename-field', 'openai');
    await tapKey(tester, 'settings-server-rename-save');
    expect(client.serverRequests, <String>['rename cloud openai']);
    expect(
      find.byKey(const ValueKey<String>('settings-server-detail-openai')),
      findsOneWidget,
    );
  }, variant: linux);

  testWidgets(
    'a new server is checked with the typed key at the typed address, then '
    'saved with its key under its own name; without a server the first '
    'becomes the default',
    (WidgetTester tester) async {
      final FakeDaemonClient client = configured();
      await openSettings(tester, client);
      await tapKey(tester, 'settings-server-add');
      await typeInto(tester, 'settings-server-name', 'lab');
      await typeInto(
        tester,
        'settings-endpoint',
        'http://192.168.1.30:8080/v1',
      );
      await tester.enterText(
        find.byKey(const Key('settings-key-input')),
        _secret,
      );
      await tester.pump();
      await tapKey(tester, 'settings-check');
      expect(client.probedEndpoints.last, 'http://192.168.1.30:8080/v1');
      expect(client.probeKeyUses.last, ProbeKeyUse.typed);
      expect(client.probedServers.last, '', reason: 'a typed key, no server');

      await tester.enterText(
        find.byKey(const Key('settings-key-input')),
        _secret,
      );
      await typeInto(tester, 'settings-model', 'llama3.1:8b');
      await tapKey(tester, 'settings-server-save');
      expect(client.serverRequests, <String>['add lab']);
      expect(client.setKeyServers, <String>['lab']);
      expect(client.activeServer, 'lab', reason: 'one server is the default');
      expect(
        find.byKey(const ValueKey<String>('settings-server-star-lab')),
        findsOneWidget,
      );
      expect(anyTextContains(tester, _secret), isFalse);
    },
    variant: linux,
  );

  testWidgets(
    'a key refused after the server was saved says so and is offered again',
    (WidgetTester tester) async {
      final FakeDaemonClient client = twoServers()
        ..setLlmKeyFailure = const Diagnostic(
          code: 'CONFIG_020',
          severity: Severity.error,
          why: 'the access key was not stored: the keyring did not answer',
        );
      await openSettings(tester, client);
      await tapKey(tester, 'settings-server-add');
      await typeInto(tester, 'settings-server-name', 'lab');
      await typeInto(
        tester,
        'settings-endpoint',
        'http://192.168.1.30:8080/v1',
      );
      await tester.enterText(
        find.byKey(const Key('settings-key-input')),
        _secret,
      );
      await tapKey(tester, 'settings-server-save');
      expect(client.serverRequests, <String>['add lab']);
      final String said = tester
          .widget<Text>(find.byKey(const Key('settings-server-refusal')))
          .data!;
      expect(said, contains('The server lab is saved'));

      client.setLlmKeyFailure = null;
      await tester.enterText(
        find.byKey(const Key('settings-key-input')),
        _secret,
      );
      await tapKey(tester, 'settings-server-save');
      expect(client.serverRequests, <String>[
        'add lab',
      ], reason: 'no second add');
      expect(client.serverKeys['lab']?.set, isTrue);
      expect(find.byKey(const Key('settings-server-cards')), findsOneWidget);
    },
    variant: linux,
  );

  testWidgets('removing a key happens on Save, never on the click alone', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = twoServers();
    await openSettings(tester, client);
    await tapKey(tester, 'settings-server-cloud');
    await tapKey(tester, 'settings-key-remove');
    expect(client.serverKeys['cloud']?.set, isTrue);
    expect(find.text('The key is removed when you save.'), findsOneWidget);
    await tapKey(tester, 'settings-server-save');
    expect(client.serverKeys['cloud'], isNull);
    expect(client.activeServer, 'home');
  }, variant: linux);

  testWidgets(
    'who uses which server: the default, the project and a profile are set '
    'and removed in the app',
    (WidgetTester tester) async {
      final FakeDaemonClient client = twoServers();
      await openSettings(tester, client);
      expect(find.text('Project acme'), findsOneWidget);
      expect(
        find.text('From the project, in .humanitl/profile.toml'),
        findsOneWidget,
      );
      expect(find.text('Profile llm-only'), findsOneWidget);

      await pick(tester, 'settings-usage-pick-project', 'cloud');
      expect(
        client.serverRequests.last,
        'assign cloud project /home/u/src/acme',
      );
      expect(client.projectUse?.server, 'cloud');
      expect(
        find.byKey(const ValueKey<String>('settings-usage-warning-project')),
        findsNothing,
        reason: 'an assignment in the app is the confirmation',
      );

      await pick(tester, 'settings-usage-pick-profile-llm-only', 'cloud');
      expect(client.serverRequests.last, 'assign cloud profile llm-only');
      await pick(tester, 'settings-usage-pick-project', 'Default (home)');
      expect(client.serverRequests.last, 'assign  project /home/u/src/acme');
      expect(client.projectUse?.server, '');

      await pick(tester, 'settings-usage-pick-default', 'cloud');
      expect(client.serverRequests.last, 'activate cloud');
    },
    variant: linux,
  );

  testWidgets('a project server nobody confirmed says the start asks', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = twoServers()
      ..projectUse = const ProjectUse(dir: '/home/u/src/acme', server: 'cloud');
    await openSettings(tester, client);
    expect(
      find.byKey(const ValueKey<String>('settings-usage-warning-project')),
      findsOneWidget,
    );
  }, variant: linux);

  testWidgets(
    'a new address with another origin says before Save that the stored key '
    'goes, and Save takes it away; the same origin keeps it',
    (WidgetTester tester) async {
      final FakeDaemonClient client = twoServers();
      await openSettings(tester, client);
      await tapKey(tester, 'settings-server-cloud');
      final Finder state = find.byKey(const Key('settings-key-state'));

      await typeInto(tester, 'settings-endpoint', 'https://api.example.com/v2');
      expect(
        tester.widget<Text>(state).data,
        isNot(contains('removed when you save')),
        reason: 'the same origin keeps the key',
      );
      await typeInto(
        tester,
        'settings-endpoint',
        'https://other.example.net/v1',
      );
      expect(
        tester.widget<Text>(state).data,
        contains('removed when you save'),
      );
      await tapKey(tester, 'settings-server-save');
      final SavedServer cloud = client.savedServers.firstWhere(
        (SavedServer server) => server.name == 'cloud',
      );
      expect(cloud.endpoint, 'https://other.example.net/v1');
      expect(cloud.keyRef, isEmpty, reason: 'the key of the old address went');
      expect(client.serverKeys['cloud'], isNull);
      expect(
        find.byKey(const ValueKey<String>('settings-server-key-cloud')),
        findsNothing,
        reason: 'the detail view stays open',
      );
      expect(tester.widget<Text>(state).data, 'No key');
    },
    variant: linux,
  );

  testWidgets('every card of the grid has the same height, the add card too', (
    WidgetTester tester,
  ) async {
    await openSettings(tester, twoServers());
    final double server = tester
        .getSize(find.byKey(const ValueKey<String>('settings-server-home')))
        .height;
    final double add = tester
        .getSize(find.byKey(const Key('settings-server-add')))
        .height;
    expect(add, server);
  }, variant: linux);

  testWidgets(
    'a server the project gets through a profile it names shows on the row, '
    'with the warning, and the picker stays on its own line',
    (WidgetTester tester) async {
      final FakeDaemonClient client = twoServers()
        ..projectUse = const ProjectUse(
          dir: '/home/u/src/acme',
          effectiveServer: 'cloud',
          viaProfile: 'llm-only',
        );
      await openSettings(tester, client);
      expect(
        find.textContaining('Through the profile llm-only it names: cloud'),
        findsOneWidget,
      );
      expect(
        find.byKey(const ValueKey<String>('settings-usage-warning-project')),
        findsOneWidget,
      );
      expect(
        find.descendant(
          of: find.byKey(const Key('settings-usage-pick-project')),
          matching: find.text('Default (home)'),
        ),
        findsOneWidget,
        reason: 'the picker shows the project\'s own line, which is empty',
      );
    },
    variant: linux,
  );

  test('the fake refuses the default profile and unknown servers', () async {
    final FakeDaemonClient client = twoServers();
    await expectLater(
      client.assignLlmServer(server: 'cloud', profile: 'default'),
      throwsA(isA<DaemonException>()),
    );
    await expectLater(
      client.assignLlmServer(server: 'gone', projectDir: '/p'),
      throwsA(isA<DaemonException>()),
    );
    await expectLater(
      client.editLlmServer('gone', endpoint: 'http://x.lan/'),
      throwsA(isA<DaemonException>()),
    );
  });

  test('the probe of a saved server compares origins, not text', () async {
    final FakeDaemonClient client = twoServers();
    final LlmProbe probe = await client.probeLlm(
      'https://API.example.com:443/v1/models',
      useStoredKey: true,
      server: 'cloud',
    );
    expect(probe.keySent, isTrue);
    final LlmProbe other = await client.probeLlm(
      'https://evil.example.net/v1',
      useStoredKey: true,
      server: 'cloud',
    );
    expect(other.keySent, isFalse);
  });

  test('a shown endpoint carries no user, password or query', () {
    expect(
      displayEndpoint('https://user:pw@api.example.com/v1?token=x#y'),
      'https://api.example.com/v1',
    );
    expect(displayEndpoint('not a url'), 'not a url');
    expect(serverHost('http://192.168.1.20:11434/v1'), '192.168.1.20:11434');
  });
}
