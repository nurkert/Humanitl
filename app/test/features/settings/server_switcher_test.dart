// Die gespeicherten Modellserver im Abschnitt Modell der Einstellungen
// (HUM-289): auflisten, anlegen mit eigenem Schlüssel, umbenennen, entfernen
// mit Rückfrage, aktivieren. Kein Schlüssel steht je in einem Text, und der
// Schlüssel eines Servers geht nur an dessen eigene Adresse.

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/config_providers.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart' show HTextField;
import 'package:humanitl/features/settings/widgets/server_switcher.dart';

import 'settings_sheet_test.dart'
    show anyTextContains, configured, openSettings;

/// Ein Fake, dessen Liste der Server erst auf Zuruf ankommt.
class _SlowServers extends FakeDaemonClient {
  _SlowServers(FakeDaemonClient from) : super(script: const <ScriptedEvent>[]) {
    configValues = from.configValues;
    savedServers = from.savedServers;
    activeServer = from.activeServer;
    llmKey = from.llmKey;
  }

  final Completer<void> listed = Completer<void>();

  @override
  Future<SavedServers> listLlmServers() async {
    await listed.future;
    return super.listLlmServers();
  }
}

/// Ein Schlüssel, den kein Text des Bildschirms je zeigen darf.
const String _secret = 'sk-cloud-0123456789abcdef';

/// Zwei gespeicherte Server, `home` aktiv mit Schlüssel, `cloud` mit eigenem.
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
    );
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

String fieldText(WidgetTester tester, String key) => tester
    .widget<EditableText>(
      find.descendant(
        of: find.byKey(Key(key)),
        matching: find.byType(EditableText),
      ),
    )
    .controller
    .text;

void main() {
  final TargetPlatformVariant linux = TargetPlatformVariant.only(
    TargetPlatform.linux,
  );

  testWidgets(
    'the list marks the active server, and "Use" switches the fields below',
    (WidgetTester tester) async {
      final FakeDaemonClient client = twoServers();
      await openSettings(tester, client);

      expect(
        find.byKey(const ValueKey<String>('settings-server-active-home')),
        findsOneWidget,
      );
      expect(
        find.byKey(const ValueKey<String>('settings-server-active-cloud')),
        findsNothing,
      );
      expect(
        find.byKey(const ValueKey<String>('settings-server-use-home')),
        findsNothing,
        reason: 'the active server offers no "Use"',
      );
      expect(find.byKey(const Key('settings-servers-help')), findsOneWidget);
      expect(
        fieldText(tester, 'settings-endpoint'),
        'http://192.168.1.20:11434/',
      );

      await tapKey(tester, 'settings-server-use-cloud');

      expect(client.serverRequests, <String>['activate cloud']);
      expect(client.activeServer, 'cloud');
      expect(
        find.byKey(const ValueKey<String>('settings-server-active-cloud')),
        findsOneWidget,
      );
      expect(
        fieldText(tester, 'settings-endpoint'),
        'https://api.example.com/v1',
        reason: 'the endpoint field follows the active server',
      );
      expect(fieldText(tester, 'settings-model'), 'gpt-oss:20b');
      expect(anyTextContains(tester, _secret), isFalse);
    },
    variant: linux,
  );

  testWidgets(
    'a new server is checked with the typed key at the typed address, then '
    'saved with its key under its own name',
    (WidgetTester tester) async {
      final FakeDaemonClient client = twoServers();
      await openSettings(tester, client);

      await tapKey(tester, 'settings-server-add');
      await tester.enterText(
        find.descendant(
          of: find.byKey(const Key('settings-server-name')),
          matching: find.byType(EditableText),
        ),
        'lab',
      );
      await tester.enterText(
        find.descendant(
          of: find.byKey(const Key('settings-server-endpoint')),
          matching: find.byType(EditableText),
        ),
        'http://192.168.1.30:8080/v1',
      );
      await tester.enterText(
        find.byKey(const Key('settings-server-key-input')),
        _secret,
      );
      await tester.pump();
      expect(anyTextContains(tester, _secret), isFalse);

      await tapKey(tester, 'settings-server-add-check');
      expect(client.probedEndpoints.last, 'http://192.168.1.30:8080/v1');
      expect(client.probeKeyUses.last, ProbeKeyUse.typed);
      expect(client.probedServers.last, '', reason: 'a typed key, no server');

      expect(
        find.byKey(const Key('settings-server-add-probe')),
        findsOneWidget,
      );
      await tester.enterText(
        find.descendant(
          of: find.byKey(const Key('settings-server-model')),
          matching: find.byType(EditableText),
        ),
        'llama3.1:8b',
      );
      await tapKey(tester, 'settings-server-save');

      expect(client.serverRequests, <String>['add lab']);
      expect(client.setKeyLengths, <int>[_secret.length]);
      expect(client.serverKeys['lab']?.keyRef, 'secret-service:humanitl/lab');
      expect(client.activeServer, 'home', reason: 'adding does not switch');
      expect(
        find.byKey(const ValueKey<String>('settings-server-lab')),
        findsOneWidget,
      );
      expect(find.byKey(const Key('settings-server-add-form')), findsNothing);
      expect(anyTextContains(tester, _secret), isFalse);
    },
    variant: linux,
  );

  testWidgets(
    'checking a saved server that is not active asks for its own key at its '
    'own address',
    (WidgetTester tester) async {
      final FakeDaemonClient client = twoServers();
      await openSettings(tester, client);

      await tapKey(tester, 'settings-server-check-cloud');

      expect(client.probedEndpoints, <String>['https://api.example.com/v1']);
      expect(client.probedServers, <String>['cloud']);
      expect(client.probeKeyUses, <ProbeKeyUse>[ProbeKeyUse.stored]);
      expect(
        find.byKey(const ValueKey<String>('settings-server-checked-cloud')),
        findsOneWidget,
      );
    },
    variant: linux,
  );

  testWidgets(
    'removing asks first, names the next active server and deletes the key '
    'only with the box ticked',
    (WidgetTester tester) async {
      final FakeDaemonClient client = twoServers();
      await openSettings(tester, client);

      await tapKey(tester, 'settings-server-remove-home');
      expect(
        find.byKey(const Key('settings-server-remove-confirm')),
        findsOneWidget,
      );
      expect(find.text('cloud becomes the active server.'), findsOneWidget);

      await tapKey(tester, 'settings-server-remove-cancel');
      expect(client.serverRequests, isEmpty, reason: 'cancel changes nothing');

      await tapKey(tester, 'settings-server-remove-home');
      await tapKey(tester, 'settings-server-remove-yes');
      expect(client.serverRequests, <String>[
        'remove home delete-key use cloud',
      ]);
      expect(client.activeServer, 'cloud');
      expect(
        find.byKey(const ValueKey<String>('settings-server-home')),
        findsNothing,
      );
    },
    variant: linux,
  );

  testWidgets('without the tick the key of a removed server stays', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = twoServers();
    await openSettings(tester, client);

    await tapKey(tester, 'settings-server-remove-cloud');
    expect(
      find.byKey(const Key('settings-server-remove-next')),
      findsNothing,
      reason: 'a server that is not active needs no successor',
    );
    await tapKey(tester, 'settings-server-remove-key');
    await tapKey(tester, 'settings-server-remove-yes');
    expect(client.serverRequests, <String>['remove cloud']);
    expect(client.activeServer, 'home');
  }, variant: linux);

  testWidgets('renaming keeps the server and refuses a name that is none', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = twoServers();
    await openSettings(tester, client);

    await tapKey(tester, 'settings-server-rename-cloud');
    final Finder field = find.descendant(
      of: find.byKey(const Key('settings-server-rename-field')),
      matching: find.byType(EditableText),
    );
    await tester.enterText(field, 'Not A Name');
    await tapKey(tester, 'settings-server-rename-save');
    expect(client.serverRequests, isEmpty);
    expect(find.byKey(const Key('settings-servers-refusal')), findsOneWidget);

    await tester.enterText(field, 'openai');
    await tapKey(tester, 'settings-server-rename-save');
    expect(client.serverRequests, <String>['rename cloud openai']);
    expect(
      find.byKey(const ValueKey<String>('settings-server-openai')),
      findsOneWidget,
    );
    expect(find.byKey(const Key('settings-servers-refusal')), findsNothing);
  }, variant: linux);

  testWidgets('without a saved server the switcher says so', (
    WidgetTester tester,
  ) async {
    await openSettings(tester, configured());
    expect(find.byKey(const Key('settings-servers-empty')), findsOneWidget);
    expect(find.byKey(const Key('settings-server-add')), findsOneWidget);
  }, variant: linux);

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
      await tester.enterText(
        find.descendant(
          of: find.byKey(const Key('settings-server-name')),
          matching: find.byType(EditableText),
        ),
        'lab',
      );
      await tester.enterText(
        find.descendant(
          of: find.byKey(const Key('settings-server-endpoint')),
          matching: find.byType(EditableText),
        ),
        'http://192.168.1.30:8080/v1',
      );
      await tester.enterText(
        find.byKey(const Key('settings-server-key-input')),
        _secret,
      );
      await tapKey(tester, 'settings-server-save');

      expect(client.serverRequests, <String>['add lab']);
      final String said = tester
          .widget<Text>(find.byKey(const Key('settings-server-add-refusal')))
          .data!;
      expect(said, contains('The server lab is saved'));
      expect(said, isNot(contains('exists already')));
      expect(
        find.byKey(const ValueKey<String>('settings-server-lab')),
        findsOneWidget,
      );

      client.setLlmKeyFailure = null;
      await tester.enterText(
        find.byKey(const Key('settings-server-key-input')),
        _secret,
      );
      await tapKey(tester, 'settings-server-save');
      expect(client.serverRequests, <String>[
        'add lab',
      ], reason: 'no second add');
      expect(client.serverKeys['lab']?.set, isTrue);
      expect(find.byKey(const Key('settings-server-add-form')), findsNothing);
    },
    variant: linux,
  );

  testWidgets(
    'an endpoint typed for one server is refused once another is active',
    (WidgetTester tester) async {
      final FakeDaemonClient client = twoServers();
      await openSettings(tester, client);

      // Ein anderer Client schaltet um, bevor dieses Formular schreibt.
      client.activeServer = 'cloud';
      final Finder field = find.descendant(
        of: find.byKey(const Key('settings-endpoint')),
        matching: find.byType(EditableText),
      );
      await tester.enterText(field, 'http://192.168.1.21:11434/');
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await tester.pumpAndSettle();

      expect(client.configWrites, isEmpty);
      expect(
        find.byKey(const Key('settings-endpoint-refusal')),
        findsOneWidget,
      );
    },
    variant: linux,
  );

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

  testWidgets(
    'while the server list loads the fields take no input and write nothing',
    (WidgetTester tester) async {
      final _SlowServers client = _SlowServers(twoServers());
      await openSettings(tester, client);

      bool enabled(String key) =>
          tester.widget<HTextField>(find.byKey(Key(key))).enabled;
      expect(enabled('settings-endpoint'), isFalse);
      expect(enabled('settings-model'), isFalse);
      expect(client.configWrites, isEmpty);

      client.listed.complete();
      await tester.pumpAndSettle();
      expect(enabled('settings-endpoint'), isTrue);
      expect(enabled('settings-model'), isTrue);
    },
    variant: linux,
  );

  testWidgets(
    'when the list arrives, the fields show the active server and write '
    'under its name',
    (WidgetTester tester) async {
      // Die Konfiguration nennt noch den Endpunkt von `home`, die Liste sagt
      // schon `cloud`: Die Felder zeigen die Werte von `cloud`, nie die von
      // `home` unter dem Namen `cloud`.
      final FakeDaemonClient client = twoServers()..activeServer = 'cloud';
      await openSettings(tester, client);
      expect(
        fieldText(tester, 'settings-endpoint'),
        'https://api.example.com/v1',
      );
      expect(fieldText(tester, 'settings-model'), 'gpt-oss:20b');

      final Finder field = find.descendant(
        of: find.byKey(const Key('settings-endpoint')),
        matching: find.byType(EditableText),
      );
      await tester.enterText(field, 'https://api.example.com/v2');
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await tester.pumpAndSettle();
      expect(client.configWrites.last, (
        'llm.endpoint',
        'https://api.example.com/v2',
      ));
      expect(
        client.savedServers
            .firstWhere((SavedServer server) => server.name == 'cloud')
            .endpoint,
        'https://api.example.com/v2',
      );
    },
    variant: linux,
  );

  testWidgets(
    'a key stored from a form bound to one server never goes to the server '
    'switched to meanwhile',
    (WidgetTester tester) async {
      // `home` hat noch keinen Schlüssel; der eingegebene ist seiner.
      final FakeDaemonClient client = twoServers()..llmKey = LlmKeyStatus.unset;
      await openSettings(tester, client);

      // Der Mensch tippt für `home`; ein anderer Client schaltet auf `cloud`.
      final Finder endpoint = find.descendant(
        of: find.byKey(const Key('settings-endpoint')),
        matching: find.byType(EditableText),
      );
      await tester.enterText(endpoint, 'http://192.168.1.21:11434/');
      await tester.pump();
      await client.activateLlmServer('cloud');
      ProviderScope.containerOf(
        tester.element(find.byKey(const Key('settings-sheet'))),
      ).invalidate(llmServersProvider);
      await tester.pumpAndSettle();
      final String cloudRef = client.llmKey.keyRef;

      await tester.enterText(
        find.byKey(const Key('settings-key-input')),
        _secret,
      );
      await tapKey(tester, 'settings-key-store');

      expect(client.setKeyLengths, <int>[_secret.length]);
      expect(client.serverKeys['home']?.set, isTrue, reason: 'the key of home');
      expect(
        client.llmKey.keyRef,
        cloudRef,
        reason: 'the key went to the server switched to meanwhile',
      );
      expect(client.activeServer, 'cloud');
    },
    variant: linux,
  );

  test('the fake refuses an unnamed key write while servers exist', () async {
    final FakeDaemonClient client = twoServers();
    await expectLater(
      client.setLlmKey(_secret),
      throwsA(isA<DaemonException>()),
    );
  });

  test('the fake refuses an unnamed view write while servers exist', () async {
    final FakeDaemonClient client = twoServers();
    await expectLater(
      client.setConfig('llm.endpoint', 'http://x.lan/'),
      throwsA(isA<DaemonException>()),
    );
  });

  test('a shown endpoint carries no user, password or query', () {
    expect(
      displayEndpoint('https://user:pw@api.example.com/v1?token=x#y'),
      'https://api.example.com/v1',
    );
    expect(displayEndpoint('not a url'), 'not a url');
  });
}
