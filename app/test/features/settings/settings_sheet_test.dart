// Das Einstellungsformular (HUM-069) mit Modell und Zugangsschlüssel
// (HUM-140): geöffnet über `Ctrl+,`, geschrieben über `SetConfig` und
// `LlmKey`, der Schlüssel nie zurückgelesen.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/settings/providers/file_opener.dart';
import 'package:humanitl/features/shell/providers/theme.dart';
import 'package:humanitl/features/tray/providers/attention.dart';

import '../../harness/app_harness.dart';

/// Ein Schlüssel, den kein Text des Bildschirms je zeigen darf.
const String secret = 'sk-test-0123456789abcdef';

/// Die App mit offenem Formular.
Future<void> openSettings(
  WidgetTester tester,
  FakeDaemonClient client, {
  List<String>? opened,
}) async {
  await pumpApp(
    tester,
    client: client,
    overrides: <Override>[
      fileOpenerProvider.overrideWithValue((String path) async {
        opened?.add(path);
        return true;
      }),
    ],
  );
  await pressCtrl(tester, LogicalKeyboardKey.comma);
  await tester.pumpAndSettle();
}

/// Ob irgendein Text des Bildschirms [needle] enthält.
bool anyTextContains(WidgetTester tester, String needle) {
  for (final Element element in find.byType(Text).evaluate()) {
    final Text text = element.widget as Text;
    final String shown = text.data ?? text.textSpan?.toPlainText() ?? '';
    if (shown.contains(needle)) {
      return true;
    }
  }
  for (final Element element in find.byType(EditableText).evaluate()) {
    final EditableText field = element.widget as EditableText;
    if (!field.obscureText && field.controller.text.contains(needle)) {
      return true;
    }
  }
  return false;
}

/// Was im Feld für den Zugangsschlüssel steht.
String keyFieldText(WidgetTester tester) => tester
    .widget<EditableText>(find.byKey(const Key('settings-key-input')))
    .controller
    .text;

FakeDaemonClient configured() =>
    FakeDaemonClient.empty()
      ..configValues = const ConfigValues(
        values: <String, Object?>{
          'hold': <String, Object?>{'timeout_secs': 300, 'ask_mode': 'ui'},
          'ui': <String, Object?>{
            'language': 'en',
            'theme': 'dark',
            'notifications': true,
          },
          'llm': <String, Object?>{
            'endpoint': 'http://192.168.1.20:11434/',
            'models': <Object?>['qwen2.5-coder:14b'],
          },
        },
        configPath: '/home/u/.config/humanitl/config.toml',
      );

void main() {
  testWidgets('the key card says "set" and never shows the value', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = configured()
      ..llmKey = const LlmKeyStatus(
        set: true,
        store: LlmKeyStore.secretService,
        keyRef: 'secret-service:humanitl/llm',
      );
    await openSettings(tester, client);

    expect(find.byKey(const Key('settings-sheet')), findsOneWidget);
    expect(find.text('Set, in the system keyring'), findsOneWidget);

    // Ein neuer Schlüssel geht hinein, und danach steht er nirgends: nicht
    // im Feld, nicht in einem Text, und der Fake kennt nur seine Länge.
    await tester.enterText(find.byKey(const Key('settings-key-input')), secret);
    await tester.pump();
    expect(anyTextContains(tester, secret), isFalse);
    final EditableText input = tester.widget<EditableText>(
      find.byKey(const Key('settings-key-input')),
    );
    expect(input.obscureText, isTrue);

    await tester.tap(find.byKey(const Key('settings-key-store')));
    await tester.pumpAndSettle();
    expect(client.setKeyLengths, <int>[secret.length]);
    expect(input.controller.text, isEmpty, reason: 'emptied after storing');
    expect(anyTextContains(tester, secret), isFalse);
    expect(find.text('Set, in the system keyring'), findsOneWidget);
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets(
    'a key in the fallback file says so, and "remove" takes it away',
    (WidgetTester tester) async {
      final FakeDaemonClient client = configured()
        ..llmKey = const LlmKeyStatus(
          set: true,
          store: LlmKeyStore.file,
          keyRef: 'file:llm',
        );
      await openSettings(tester, client);
      expect(find.byKey(const Key('settings-key-state')), findsOneWidget);
      expect(
        tester.widget<Text>(find.byKey(const Key('settings-key-state'))).data,
        contains('in a file only you can read'),
      );

      await tester.tap(find.byKey(const Key('settings-key-remove')));
      await tester.pumpAndSettle();
      expect(client.llmKey.set, isFalse);
      expect(find.text('Not set'), findsOneWidget);
    },
    variant: TargetPlatformVariant.only(TargetPlatform.linux),
  );

  testWidgets(
    '"Check" calls ProbeLlm, shows the models, and a click picks one',
    (WidgetTester tester) async {
      final FakeDaemonClient client = configured()
        ..llmProbe = const LlmProbe(
          endpoint: '',
          models: <String>['gpt-oss:20b', 'phi-4'],
          flavor: LlmFlavor.openAiCompatible,
          endpointIsPrivate: true,
        );
      await openSettings(tester, client);

      await tester.tap(find.byKey(const Key('settings-check')));
      await tester.pumpAndSettle();
      expect(client.probedEndpoints, <String>['http://192.168.1.20:11434/']);
      expect(client.probeKeyUses, <ProbeKeyUse>[ProbeKeyUse.none]);
      expect(find.byKey(const Key('settings-probe-result')), findsOneWidget);
      expect(find.text('gpt-oss:20b'), findsOneWidget);

      await tester.tap(
        find.byKey(const ValueKey<String>('settings-model-phi-4')),
      );
      await tester.pumpAndSettle();
      expect(client.configWrites.last, (
        'llm.models',
        '["phi-4","qwen2.5-coder:14b"]',
      ));
    },
    variant: TargetPlatformVariant.only(TargetPlatform.linux),
  );

  testWidgets(
    '"Check" sends a typed key once and otherwise asks for the stored one',
    (WidgetTester tester) async {
      final FakeDaemonClient client = configured()
        ..llmKey = const LlmKeyStatus(
          set: true,
          store: LlmKeyStore.secretService,
          keyRef: 'secret-service:humanitl/llm',
        );
      await openSettings(tester, client);

      await tester.tap(find.byKey(const Key('settings-check')));
      await tester.pumpAndSettle();
      await tester.enterText(
        find.byKey(const Key('settings-key-input')),
        secret,
      );
      await tester.tap(find.byKey(const Key('settings-check')));
      await tester.pumpAndSettle();
      expect(client.probeKeyUses, <ProbeKeyUse>[
        ProbeKeyUse.stored,
        ProbeKeyUse.typed,
      ]);
      expect(find.byKey(const Key('settings-probe-key-sent')), findsOneWidget);
      expect(client.setKeyLengths, isEmpty, reason: 'a check stores nothing');
      expect(
        keyFieldText(tester),
        isEmpty,
        reason: 'the typed key went out once and does not wait in the form',
      );

      await tester.tap(find.byKey(const Key('settings-check')));
      await tester.pumpAndSettle();
      expect(client.probeKeyUses.last, ProbeKeyUse.stored);
    },
    variant: TargetPlatformVariant.only(TargetPlatform.linux),
  );

  testWidgets('a key typed for one endpoint does not go out to the next one', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = configured();
    await openSettings(tester, client);

    await tester.enterText(find.byKey(const Key('settings-key-input')), secret);
    await tester.enterText(
      find.byKey(const Key('settings-endpoint')),
      'http://192.168.1.30:11434/',
    );
    await tester.pump();
    expect(
      keyFieldText(tester),
      isEmpty,
      reason: 'editing the endpoint empties the key before anything is saved',
    );
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pumpAndSettle();
    expect(client.configWrites.last, (
      'llm.endpoint',
      'http://192.168.1.30:11434/',
    ));
    expect(keyFieldText(tester), isEmpty);

    await tester.tap(find.byKey(const Key('settings-check')));
    await tester.pumpAndSettle();
    expect(client.probedEndpoints.last, 'http://192.168.1.30:11434/');
    expect(client.probeKeyUses, <ProbeKeyUse>[ProbeKeyUse.none]);
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets(
    'an endpoint outside the private network carries LLM_006 beside it',
    (WidgetTester tester) async {
      final FakeDaemonClient client = configured();
      await openSettings(tester, client);
      await tester.enterText(
        find.byKey(const Key('settings-endpoint')),
        'https://api.example.com/v1',
      );
      await tester.tap(find.byKey(const Key('settings-check')));
      await tester.pumpAndSettle();
      expect(find.byKey(const Key('settings-probe-LLM_006')), findsOneWidget);
    },
    variant: TargetPlatformVariant.only(TargetPlatform.linux),
  );

  testWidgets(
    'a segment writes through SetConfig, and the app follows ui.theme',
    (WidgetTester tester) async {
      final FakeDaemonClient client = configured();
      await openSettings(tester, client);
      final ProviderContainer container = ProviderScope.containerOf(
        tester.element(find.byKey(const Key('settings-sheet'))),
      );
      expect(container.read(themeModeProvider), HThemeMode.dark);

      await tester.ensureVisible(find.text('Light'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Light'));
      await tester.pumpAndSettle();
      expect(client.configWrites.last, ('ui.theme', 'light'));
      expect(container.read(themeModeProvider), HThemeMode.light);

      await tester.ensureVisible(
        find.byKey(const Key('settings-notifications')),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const Key('settings-notifications')));
      await tester.pumpAndSettle();
      expect(client.configWrites.last, ('ui.notifications', 'false'));
      expect(container.read(notificationsEnabledProvider), isFalse);
    },
    variant: TargetPlatformVariant.only(TargetPlatform.linux),
  );

  testWidgets('a value the daemon refuses shows its reason under the field', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = configured();
    await openSettings(tester, client);
    await tester.enterText(find.byKey(const Key('settings-timeout')), '0');
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('settings-timeout-refusal')), findsOneWidget);
    expect(
      tester
          .widget<Text>(find.byKey(const Key('settings-timeout-refusal')))
          .data,
      contains('out of range'),
    );
    expect(
      client.configWrites.where(
        ((String, String) write) => write.$1 == 'hold.timeout_secs',
      ),
      isEmpty,
    );
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets(
    'the form says when a change takes effect and opens config.toml',
    (WidgetTester tester) async {
      final List<String> opened = <String>[];
      await openSettings(tester, configured(), opened: opened);
      expect(find.byKey(const Key('settings-restart-note')), findsOneWidget);
      expect(find.text('/home/u/.config/humanitl/config.toml'), findsOneWidget);
      await tester.ensureVisible(find.byKey(const Key('settings-open-file')));
      await tester.tap(find.byKey(const Key('settings-open-file')));
      await tester.pumpAndSettle();
      expect(opened, <String>['/home/u/.config/humanitl/config.toml']);
    },
    variant: TargetPlatformVariant.only(TargetPlatform.linux),
  );

  testWidgets(
    'the system certificate switch is on when tls.trust_system_store is unset',
    (WidgetTester tester) async {
      // HUM-257: Ohne Wert gilt die Vorgabe des Schemas, und die ist an.
      await openSettings(tester, configured());
      expect(
        tester
            .widget<HCheckbox>(
              find.byKey(const Key('settings-trust-system-store')),
            )
            .value,
        isTrue,
      );
    },
    variant: TargetPlatformVariant.only(TargetPlatform.linux),
  );

  testWidgets(
    'the system certificate switch reads tls.trust_system_store and writes it',
    (WidgetTester tester) async {
      // HUM-257: Das Opt-out steht im Formular. Ohne Wert gilt die Vorgabe
      // des Schemas (an); gesetzt auf `false` zeigt der Schalter aus, und ein
      // Klick schreibt den neuen Wert über SetConfig.
      final Finder toggle = find.byKey(
        const Key('settings-trust-system-store'),
      );
      final FakeDaemonClient client = configured();
      client.configValues = ConfigValues(
        values: <String, Object?>{
          ...client.configValues.values,
          'tls': <String, Object?>{'trust_system_store': false},
        },
        configPath: client.configValues.configPath,
      );
      await openSettings(tester, client);
      expect(tester.widget<HCheckbox>(toggle).value, isFalse);

      await tester.ensureVisible(toggle);
      await tester.pumpAndSettle();
      await tester.tap(toggle);
      await tester.pumpAndSettle();
      expect(client.configWrites.last, ('tls.trust_system_store', 'true'));
      expect(tester.widget<HCheckbox>(toggle).value, isTrue);
    },
    variant: TargetPlatformVariant.only(TargetPlatform.linux),
  );
  // HUM-283: a key pasted with quotes and a line break arrives without them,
  // and the field never shows the key.
  testWidgets('a key pasted with quotes is stored without them', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = configured();
    await openSettings(tester, client);
    await tester.enterText(
      find.byKey(const Key('settings-key-input')),
      ' "$secret"\n',
    );
    await tester.pump();
    expect(
      keyFieldText(tester) == secret,
      isTrue,
      reason: 'the pasted key still carries quotes or blanks',
    );
    await tester.tap(find.byKey(const Key('settings-key-store')));
    await tester.pumpAndSettle();
    expect(client.setKeyLengths, <int>[secret.length]);
    expect(anyTextContains(tester, secret), isFalse);
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  // HUM-283: the endpoint pasted with its trailing quote is written clean.
  testWidgets('a pasted endpoint is written without its stray quote', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = configured();
    await openSettings(tester, client);
    await tester.enterText(
      find.byKey(const Key('settings-endpoint')),
      'http://192.168.1.30:11434/"',
    );
    await tester.pump();
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pumpAndSettle();
    expect(client.configWrites.last, (
      'llm.endpoint',
      'http://192.168.1.30:11434/',
    ));
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  // HUM-274: the agent of the next session is chosen in the form. The control
  // shows the configured adapter, and a click writes the other one through
  // SetConfig.
  testWidgets('the agent switch shows agent.adapter and writes it', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = configured();
    client.configValues = ConfigValues(
      values: <String, Object?>{
        ...client.configValues.values,
        'agent': <String, Object?>{'adapter': 'pi'},
      },
      configPath: client.configValues.configPath,
    );
    await openSettings(tester, client);
    final Finder control = find.byKey(const Key('settings-agent'));
    expect(tester.widget<HSegmented<String>>(control).selected, 'pi');

    final Finder openCode = find.descendant(
      of: control,
      matching: find.text('OpenCode'),
    );
    await tester.ensureVisible(openCode);
    await tester.pumpAndSettle();
    await tester.tap(openCode);
    await tester.pumpAndSettle();
    expect(client.configWrites.last, ('agent.adapter', 'opencode'));
    expect(tester.widget<HSegmented<String>>(control).selected, 'opencode');
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('the agent switch shows OpenCode when agent.adapter is unset', (
    WidgetTester tester,
  ) async {
    await openSettings(tester, configured());
    expect(
      tester
          .widget<HSegmented<String>>(find.byKey(const Key('settings-agent')))
          .selected,
      'opencode',
    );
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));
}
