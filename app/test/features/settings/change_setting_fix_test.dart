// Ein `ChangeSetting` führt in das Feld, das ihn behebt (HUM-275).
//
// `LLM_002` sagte auf dem Rechner des Besitzers „verlangt einen
// Zugangsschlüssel", und nichts auf dem Bildschirm führte dorthin, wo man
// einen einträgt. Seit HUM-275 trägt der Befund `ChangeSetting` auf
// `llm.api_key_ref`, und die Oberfläche macht daraus „Open settings": Das
// Blatt geht auf, scrollt zum Feld des Schlüssels und gibt ihm den Fokus.
//
// Rot, wenn der Knopf fehlt, wenn er das Blatt ohne Fokus öffnet, oder wenn
// ein Schlüssel, den das Blatt zeigt, kein Ziel darin hat.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';

import '../../harness/app_harness.dart';

/// `LLM_002` so, wie der Daemon ihn seit HUM-275 schickt.
Diagnostic llm002({String key = FormKeys.llmApiKeyRef}) => Diagnostic(
  code: 'LLM_002',
  severity: Severity.error,
  title: 'LLM-Endpoint verlangt eine Anmeldung',
  why:
      'the LLM server answered 401 at http://192.168.1.20:11434/v1/models. '
      'It requires an access key, and none was sent.',
  fix: FixAction.changeSetting(key: key, value: ''),
);

FakeDaemonClient needsKey({String key = FormKeys.llmApiKeyRef}) =>
    // Die Zeile `sandbox.mode` steht nur bei einem Daemon, der
    // `sandbox.disposable` nennt (HUM-357).
    (key == FormKeys.sandboxMode
          ? FakeDaemonClient(info: FakeDaemonClient.disposableInfo)
          : FakeDaemonClient())
      ..llmProbeFailure = llm002(key: key)
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
      );

/// Die Modell-Zeile des Setup-Bildschirms fragt den Endpunkt und zeigt
/// `LLM_002`: die Zustandszeile aus dem Bericht des Besitzers.
Future<void> showLlm002InSetup(
  WidgetTester tester,
  FakeDaemonClient client,
) async {
  await pumpApp(tester, client: client);
  await pressCtrl(tester, LogicalKeyboardKey.digit6);
  await tester.pump();
  await tester.enterText(
    find.byKey(const Key('setup-llm-endpoint')),
    'http://192.168.1.20:11434',
  );
  await tester.pump();
  await tester.tap(find.byKey(const Key('setup-llm-probe')));
  await tester.pump();
  await tester.pump();
  expect(find.text('LLM_002'), findsWidgets);
}

/// Ob der Fokus in dem Widget mit [key] liegt, auf ihm selbst oder darunter.
bool focusIsWithin(Key key) {
  final BuildContext? focused = FocusManager.instance.primaryFocus?.context;
  if (focused == null) {
    return false;
  }
  if (focused.widget.key == key) {
    return true;
  }
  bool found = false;
  focused.visitAncestorElements((Element element) {
    found = element.widget.key == key;
    return !found;
  });
  return found;
}

/// Das Feld, auf dem jeder Schlüssel des Blatts landet. Der Projektordner
/// fehlt: Seinen Fix trägt der Ordnerwähler des Bildschirms (HUM-262).
const Map<String, Key> fieldOf = <String, Key>{
  FormKeys.holdTimeoutSecs: Key('settings-timeout'),
  FormKeys.holdAskMode: Key('settings-ask-mode'),
  FormKeys.uiLanguage: Key('settings-language'),
  FormKeys.uiTheme: Key('settings-theme'),
  FormKeys.uiNotifications: Key('settings-notifications'),
  FormKeys.llmEndpoint: Key('settings-endpoint'),
  FormKeys.llmModels: Key('settings-model'),
  FormKeys.tlsTrustSystemStore: Key('settings-trust-system-store'),
  FormKeys.agentAdapter: Key('settings-agent'),
  FormKeys.llmApiKeyRef: Key('settings-key-input'),
  FormKeys.sandboxMode: Key('settings-sandbox-mode'),
};

void main() {
  testWidgets('change_setting_fix: LLM_002 opens the sheet with the key '
      'field focused', (WidgetTester tester) async {
    await showLlm002InSetup(tester, needsKey());
    expect(find.byKey(const Key('settings-sheet')), findsNothing);

    final Finder button = find.byKey(const Key('fix-open-settings'));
    expect(button, findsOneWidget);
    expect(find.text('Open settings'), findsOneWidget);
    await tester.tap(button);
    await tester.pumpAndSettle();

    expect(find.byKey(const Key('settings-sheet')), findsOneWidget);
    expect(
      focusIsWithin(const Key('settings-key-input')),
      isTrue,
      reason: 'the access-key field has the focus',
    );
    expect(
      find.byKey(const Key('settings-key-input')).hitTestable(),
      findsOneWidget,
      reason: 'and it is on screen',
    );
    expect(tester.takeException(), isNull);
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('the same fix inside the sheet lands on the key field again', (
    WidgetTester tester,
  ) async {
    await pumpApp(tester, client: needsKey());
    await pressCtrl(tester, LogicalKeyboardKey.comma);
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const Key('settings-check')));
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('settings-probe-failure')), findsOneWidget);
    expect(focusIsWithin(const Key('settings-key-input')), isFalse);

    await tester.tap(
      find.descendant(
        of: find.byKey(const Key('settings-probe-failure')),
        matching: find.byKey(const Key('fix-open-settings')),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('settings-sheet')), findsOneWidget);
    expect(focusIsWithin(const Key('settings-key-input')), isTrue);
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('every key the sheet shows has a field the fix lands on', (
    WidgetTester tester,
  ) async {
    expect(
      fieldOf.keys.toSet(),
      FormKeys.shownInSheet.difference(<String>{FormKeys.sandboxWorkDir}),
    );
    for (final MapEntry<String, Key> entry in fieldOf.entries) {
      await showLlm002InSetup(tester, needsKey(key: entry.key));
      await tester.tap(find.byKey(const Key('fix-open-settings')));
      await tester.pumpAndSettle();
      expect(
        focusIsWithin(entry.value),
        isTrue,
        reason: '${entry.key} focuses ${entry.value}',
      );
      expect(
        find.byKey(entry.value).hitTestable(),
        findsWidgets,
        reason: '${entry.key} scrolls ${entry.value} into view',
      );
      await tester.pumpWidget(const SizedBox.shrink());
    }
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('the project folder keeps its own fix and no settings button', (
    WidgetTester tester,
  ) async {
    await showLlm002InSetup(tester, needsKey(key: FormKeys.sandboxWorkDir));
    expect(find.byKey(const Key('fix-open-settings')), findsNothing);
    expect(find.text('Change sandbox.work_dir'), findsOneWidget);
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('a key the sheet does not show keeps the badge', (
    WidgetTester tester,
  ) async {
    await showLlm002InSetup(tester, needsKey(key: 'proxy.listen'));
    expect(find.byKey(const Key('fix-open-settings')), findsNothing);
    expect(find.text('Change proxy.listen'), findsOneWidget);
    expect(find.byType(HBadge), findsWidgets);
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));
}
