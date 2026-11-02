// Die Modellserver in der Kategorie Modelle (HUM-405, vorher HUM-289; seit
// HUM-414 Liste und Detailansicht): eine ruhige Liste mit Name, Host und dem
// Standard, daneben die Detailansicht des gewählten Servers mit Speichern und
// Abbrechen, und „Wer nutzt welchen Server". Kein Schlüssel steht je in einem
// Text, nichts geht beim Tippen hinaus, und der Schlüssel eines Servers geht
// nur an dessen eigene Adresse.

import 'dart:ui' show Tristate;

import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/config_providers.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/settings/widgets/servers/server_list.dart';

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

/// Ob der Fokus in dem Widget mit [key] steht.
bool focusWithin(Key key) {
  final BuildContext? context = FocusManager.instance.primaryFocus?.context;
  if (context == null) {
    return false;
  }
  if (context.widget.key == key) {
    return true;
  }
  bool found = false;
  context.visitAncestorElements((Element element) {
    found = element.widget.key == key;
    return !found;
  });
  return found;
}

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
    'the list names each server with its host, marks the one default in '
    'words, holds nothing else, and shows the default beside it',
    (WidgetTester tester) async {
      await openSettings(tester, twoServers());

      expect(
        find.byKey(const ValueKey<String>('settings-server-star-home')),
        findsOneWidget,
      );
      expect(
        find.byKey(const ValueKey<String>('settings-server-default-home')),
        findsOneWidget,
      );
      expect(
        find.byKey(const ValueKey<String>('settings-server-star-cloud')),
        findsNothing,
      );
      final Finder list = find.byKey(const Key('settings-server-list'));
      expect(
        find.descendant(of: list, matching: find.text('192.168.1.20:11434')),
        findsOneWidget,
      );
      expect(
        find.descendant(of: list, matching: find.text('api.example.com:443')),
        findsOneWidget,
      );
      // Nichts sonst in der Liste: kein Modell, kein Schlüssel, kein Feld.
      for (final String absent in <String>[
        'gpt-oss:20b',
        'qwen2.5-coder:14b',
        'Key stored',
        'Not checked yet',
      ]) {
        expect(
          find.descendant(of: list, matching: find.text(absent)),
          findsNothing,
        );
      }
      expect(
        find.descendant(of: list, matching: find.byType(EditableText)),
        findsNothing,
      );
      // Der Standard steht vorn, die Zeile zum Hinzufügen hinten.
      double top(Finder row) => tester.getTopLeft(row).dy;
      final double home = top(
        find.byKey(const ValueKey<String>('settings-server-home')),
      );
      final double cloud = top(
        find.byKey(const ValueKey<String>('settings-server-cloud')),
      );
      final double add = top(find.byKey(const Key('settings-server-add')));
      expect(home < cloud && cloud < add, isTrue);
      // Ohne eigene Wahl ist der Standard gewählt, und seine Detailansicht
      // steht neben der Liste.
      expect(
        find.byKey(const ValueKey<String>('settings-server-detail-home')),
        findsOneWidget,
      );
      expect(
        tester.getTopLeft(find.byKey(const Key('settings-endpoint'))).dx >
            tester.getTopRight(list).dx,
        isTrue,
      );
      final SemanticsHandle semantics = tester.ensureSemantics();
      final SemanticsNode row = tester.getSemantics(
        find.byKey(const ValueKey<String>('settings-server-home')),
      );
      expect(row.label, 'home, 192.168.1.20:11434, Default, 1 of 2');
      expect(row.flagsCollection.isSelected, Tristate.isTrue);
      semantics.dispose();
    },
    variant: linux,
  );

  testWidgets(
    'arrow keys move the choice and the detail follows; Enter goes in, '
    'Escape goes back to the list',
    (WidgetTester tester) async {
      await openSettings(tester, twoServers());
      await tapKey(tester, 'settings-server-home');
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowUp);
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      await tester.pump();
      expect(
        find.byKey(const ValueKey<String>('settings-server-detail-home')),
        findsOneWidget,
        reason: 'the detail waits until the keys rest',
      );
      await tester.pump(ServerListState.settleAfter);
      await tester.pumpAndSettle();
      expect(
        find.byKey(const ValueKey<String>('settings-server-detail-cloud')),
        findsOneWidget,
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(
        focusWithin(const Key('settings-server-name')),
        isTrue,
        reason: 'Enter goes into the detail view',
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(
        focusWithin(const ValueKey<String>('settings-server-cloud')),
        isTrue,
        reason: 'Escape goes back to the chosen row',
      );
      expect(find.byKey(const Key('settings-overlay')), findsOneWidget);
    },
    variant: linux,
  );

  testWidgets(
    'another row while the form holds a change asks: keep editing, discard '
    'or save',
    (WidgetTester tester) async {
      final FakeDaemonClient client = twoServers();
      await openSettings(tester, client);
      await tapKey(tester, 'settings-server-cloud');
      await typeInto(tester, 'settings-model', 'gpt-oss:120b');
      await tapKey(tester, 'settings-server-home');
      expect(find.byKey(const Key('settings-server-leave')), findsOneWidget);
      expect(
        find.byKey(const ValueKey<String>('settings-server-detail-cloud')),
        findsOneWidget,
        reason: 'the change is not dropped by the click',
      );

      await tapKey(tester, 'settings-server-keep-editing');
      expect(find.byKey(const Key('settings-server-leave')), findsNothing);
      expect(fieldText(tester, 'settings-model'), 'gpt-oss:120b');

      await tapKey(tester, 'settings-server-home');
      await tapKey(tester, 'settings-server-save');
      expect(client.serverRequests, <String>['edit cloud']);
      expect(
        find.byKey(const ValueKey<String>('settings-server-detail-home')),
        findsOneWidget,
      );

      await tapKey(tester, 'settings-server-cloud');
      await typeInto(tester, 'settings-model', 'other');
      await tapKey(tester, 'settings-server-home');
      await tapKey(tester, 'settings-server-discard');
      expect(client.serverRequests, <String>['edit cloud']);
      expect(
        find.byKey(const ValueKey<String>('settings-server-detail-home')),
        findsOneWidget,
      );
    },
    variant: linux,
  );

  testWidgets(
    'without any server the page says what that means and adds one; a pasted '
    'address proposes the name and says where it points',
    (WidgetTester tester) async {
      final FakeDaemonClient client = configured()
        ..savedServers = <SavedServer>[]
        ..activeServer = '';
      await openSettings(tester, client);
      expect(find.byKey(const Key('settings-server-empty')), findsOneWidget);
      expect(
        find.textContaining('can’t reach a model until you add one'),
        findsOneWidget,
      );
      await tapKey(tester, 'settings-server-add');
      await typeInto(tester, 'settings-endpoint', 'http://api.groq.example/v1');
      expect(fieldText(tester, 'settings-server-name'), 'groq');
      expect(find.text('Remote: api.groq.example'), findsOneWidget);
      expect(
        find.byKey(const Key('settings-endpoint-plain-http')),
        findsOneWidget,
      );
      expect(find.text('Sent only to http://api.groq.example'), findsOneWidget);
      await typeInto(tester, 'settings-endpoint', 'http://127.0.0.1:11434/v1');
      expect(fieldText(tester, 'settings-server-name'), 'local');
      expect(find.text('This machine'), findsOneWidget);
      expect(
        find.byKey(const Key('settings-endpoint-plain-http')),
        findsNothing,
      );
      expect(client.probedEndpoints, isEmpty, reason: 'nothing went out');
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
    'row shows the result of this app session',
    (WidgetTester tester) async {
      final FakeDaemonClient client = twoServers();
      await openSettings(tester, client);
      await tapKey(tester, 'settings-server-cloud');
      await tapKey(tester, 'settings-check');
      expect(client.probedServers, <String>['cloud']);
      expect(client.probeKeyUses, <ProbeKeyUse>[ProbeKeyUse.stored]);

      expect(
        find.byKey(const ValueKey<String>('settings-server-checked-cloud')),
        findsOneWidget,
      );
      expect(
        find.byKey(const ValueKey<String>('settings-server-checked-home')),
        findsNothing,
        reason: 'no dot before a check',
      );
      final SemanticsHandle semantics = tester.ensureSemantics();
      expect(
        tester
            .getSemantics(
              find.byKey(const ValueKey<String>('settings-server-cloud')),
            )
            .label,
        contains('found'),
        reason: 'the dot has words',
      );
      semantics.dispose();
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
    expect(
      tester
          .widget<HButton>(
            find.byKey(const Key('settings-server-make-default')),
          )
          .onPressed,
      isNull,
    );
    expect(find.text('This is the default'), findsOneWidget);
  }, variant: linux);

  testWidgets(
    'the default is not removed while another could take its place; another '
    'server asks first, Cancel preselected, and names its profiles',
    (WidgetTester tester) async {
      final FakeDaemonClient client = twoServers()
        ..profileUses = const <ProfileUse>[
          ProfileUse(name: 'llm-only', server: 'cloud'),
        ];
      await openSettings(tester, client);
      await tapKey(tester, 'settings-server-home');
      expect(
        tester
            .widget<HButton>(find.byKey(const Key('settings-server-remove')))
            .onPressed,
        isNull,
      );
      expect(find.textContaining('can’t be removed'), findsOneWidget);

      await tapKey(tester, 'settings-server-cloud');
      expect(find.text('Used by: profile llm-only'), findsOneWidget);
      await tapKey(tester, 'settings-server-remove');
      expect(
        find.byKey(const Key('settings-server-remove-confirm')),
        findsOneWidget,
      );
      expect(
        find.byKey(const Key('settings-server-remove-profiles')),
        findsOneWidget,
      );
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

      await tapKey(tester, 'settings-server-remove');
      await tapKey(tester, 'settings-server-remove-yes');
      expect(client.serverRequests, <String>['remove cloud delete-key']);
      expect(
        find.byKey(const ValueKey<String>('settings-server-cloud')),
        findsNothing,
      );
      expect(
        find.byKey(const ValueKey<String>('settings-server-detail-home')),
        findsOneWidget,
      );
    },
    variant: linux,
  );

  testWidgets('the name is a field; Save renames and keeps the server chosen', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = twoServers();
    await openSettings(tester, client);
    await tapKey(tester, 'settings-server-cloud');
    await typeInto(tester, 'settings-server-name', 'Not A Name');
    await tapKey(tester, 'settings-server-save');
    expect(client.serverRequests, isEmpty);
    expect(find.byKey(const Key('settings-server-name-error')), findsOneWidget);

    await typeInto(tester, 'settings-server-name', 'home');
    expect(find.text('A server named home exists already.'), findsOneWidget);
    expect(
      tester
          .widget<HButton>(find.byKey(const Key('settings-server-save')))
          .onPressed,
      isNull,
    );

    // Der Name zuerst: Eine Absage des Namens käme nie nach Adresse oder
    // Schlüssel.
    await typeInto(tester, 'settings-server-name', 'openai');
    await typeInto(tester, 'settings-model', 'gpt-oss:120b');
    await tapKey(tester, 'settings-server-save');
    expect(client.serverRequests, <String>[
      'rename cloud openai',
      'edit openai',
    ]);
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
      expect(
        find.byKey(const ValueKey<String>('settings-server-detail-lab')),
        findsOneWidget,
      );
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
        isNot(contains('Saving removes the stored key')),
        reason: 'the same origin keeps the key',
      );
      await typeInto(
        tester,
        'settings-endpoint',
        'https://other.example.net/v1',
      );
      expect(
        tester.widget<Text>(state).data,
        contains('Saving removes the stored key'),
      );
      await tapKey(tester, 'settings-server-save');
      final SavedServer cloud = client.savedServers.firstWhere(
        (SavedServer server) => server.name == 'cloud',
      );
      expect(cloud.endpoint, 'https://other.example.net/v1');
      expect(cloud.keyRef, isEmpty, reason: 'the key of the old address went');
      expect(client.serverKeys['cloud'], isNull);
      expect(
        find.byKey(const ValueKey<String>('settings-server-detail-cloud')),
        findsOneWidget,
        reason: 'the detail view stays open',
      );
      expect(tester.widget<Text>(state).data, 'No key');
    },
    variant: linux,
  );

  testWidgets('every row of the list has the same height, the add row too', (
    WidgetTester tester,
  ) async {
    await openSettings(tester, twoServers());
    double height(Finder row) => tester.getSize(row).height;
    final double home = height(
      find.byKey(const ValueKey<String>('settings-server-home')),
    );
    expect(
      height(find.byKey(const ValueKey<String>('settings-server-cloud'))),
      home,
    );
    expect(height(find.byKey(const Key('settings-server-add'))), home);
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

  testWidgets('Enter in the key field does not save', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = twoServers();
    await openSettings(tester, client);
    await tapKey(tester, 'settings-server-cloud');
    await tester.enterText(
      find.byKey(const Key('settings-key-input')),
      _secret,
    );
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pumpAndSettle();
    expect(client.setKeyLengths, isEmpty, reason: 'only Save stores a key');
    expect(find.byKey(const Key('settings-server-unsaved')), findsOneWidget);
  }, variant: linux);

  testWidgets(
    'a server another client removes while the form holds a draft keeps the '
    'draft, says so, and saves it as a new server',
    (WidgetTester tester) async {
      final FakeDaemonClient client = twoServers();
      await openSettings(tester, client);
      await tapKey(tester, 'settings-server-cloud');
      await typeInto(tester, 'settings-model', 'gpt-oss:120b');
      client.savedServers = client.savedServers
          .where((SavedServer server) => server.name != 'cloud')
          .toList();
      ProviderScope.containerOf(
        tester.element(find.byKey(const Key('settings-overlay'))),
      ).invalidate(llmServersProvider);
      await tester.pumpAndSettle();
      expect(
        find.byKey(const Key('settings-server-removed-elsewhere')),
        findsOneWidget,
      );
      expect(fieldText(tester, 'settings-model'), 'gpt-oss:120b');
      await tapKey(tester, 'settings-server-save-as-new');
      expect(client.serverRequests, <String>['add cloud']);
      expect(
        find.byKey(const ValueKey<String>('settings-server-detail-cloud')),
        findsOneWidget,
      );
      expect(
        find.byKey(const Key('settings-server-removed-elsewhere')),
        findsNothing,
      );
    },
    variant: linux,
  );

  testWidgets(
    'an address changed elsewhere does not replace a draft or empty its key',
    (WidgetTester tester) async {
      final FakeDaemonClient client = twoServers();
      await openSettings(tester, client);
      await tapKey(tester, 'settings-server-cloud');
      await tester.enterText(
        find.byKey(const Key('settings-key-input')),
        _secret,
      );
      await tester.pump();
      client.savedServers = <SavedServer>[
        for (final SavedServer server in client.savedServers)
          server.name == 'cloud'
              ? SavedServer(
                  name: 'cloud',
                  endpoint: 'https://api.example.org/v9',
                  models: server.models,
                  keyRef: server.keyRef,
                )
              : server,
      ];
      ProviderScope.containerOf(
        tester.element(find.byKey(const Key('settings-overlay'))),
      ).invalidate(llmServersProvider);
      await tester.pumpAndSettle();
      expect(keyText(tester), _secret, reason: 'the typed key stays');
      expect(
        fieldText(tester, 'settings-endpoint'),
        'https://api.example.com/v1',
      );
      expect(
        find.byKey(const Key('settings-endpoint-elsewhere')),
        findsOneWidget,
      );
    },
    variant: linux,
  );

  testWidgets('the removal question names the project that uses the server', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = twoServers()
      ..projectUse = const ProjectUse(
        dir: '/home/u/src/acme',
        server: 'cloud',
        effectiveServer: 'cloud',
      );
    await openSettings(tester, client);
    await tapKey(tester, 'settings-server-cloud');
    await tapKey(tester, 'settings-server-remove');
    expect(
      tester
          .widget<Text>(
            find.byKey(const Key('settings-server-remove-profiles')),
          )
          .data,
      contains('project acme'),
    );
  }, variant: linux);

  testWidgets(
    'closing the overlay with a draft asks first; Keep editing stays, Discard '
    'closes',
    (WidgetTester tester) async {
      await openSettings(tester, twoServers());
      await tapKey(tester, 'settings-server-cloud');
      await typeInto(tester, 'settings-model', 'other');
      await tapKey(tester, 'settings-close');
      expect(find.byKey(const Key('settings-server-leave')), findsOneWidget);
      expect(find.byKey(const Key('settings-overlay')), findsOneWidget);
      await tapKey(tester, 'settings-server-keep-editing');
      expect(fieldText(tester, 'settings-model'), 'other');

      await tapKey(tester, 'settings-nav-agent');
      expect(find.byKey(const Key('settings-server-leave')), findsOneWidget);
      await tapKey(tester, 'settings-server-keep-editing');

      await tapKey(tester, 'settings-close');
      await tapKey(tester, 'settings-server-discard');
      expect(find.byKey(const Key('settings-overlay')), findsNothing);
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

  test('an address says where it points, read from the text alone', () {
    ServerPlaceKind kind(String endpoint) => serverPlace(endpoint)!.kind;
    expect(kind('http://localhost:11434'), ServerPlaceKind.thisMachine);
    expect(kind('http://127.0.0.1:8080'), ServerPlaceKind.thisMachine);
    expect(kind('http://192.168.1.20:11434'), ServerPlaceKind.localNetwork);
    expect(kind('http://10.0.0.5'), ServerPlaceKind.localNetwork);
    expect(kind('http://172.20.0.5'), ServerPlaceKind.localNetwork);
    expect(kind('http://172.32.0.5'), ServerPlaceKind.remote);
    expect(kind('http://gpu.lan:8000'), ServerPlaceKind.localNetwork);
    expect(kind('https://api.example.com/v1'), ServerPlaceKind.remote);
    expect(serverPlace('http://api.example.com/v1')!.plainOverInternet, isTrue);
    expect(
      serverPlace('https://api.example.com/v1')!.plainOverInternet,
      isFalse,
    );
    expect(serverPlace('http://192.168.1.20')!.plainOverInternet, isFalse);
    expect(serverPlace('not a url'), isNull);
    expect(
      serverOrigin('https://u:p@API.example.com:8443/v1?x=1'),
      'https://api.example.com:8443',
    );
    expect(
      serverOrigin('https://api.example.com/v1'),
      'https://api.example.com',
    );
    expect(proposeServerName('https://api.openai.example/v1'), 'openai');
    expect(proposeServerName('https://llm.corp.example:8443/v1'), 'corp');
    expect(proposeServerName('http://192.168.1.30:8080'), '192-168-1-30');
    expect(proposeServerName('http://localhost:11434'), 'local');
    expect(proposeServerName(''), '');
  });
}
