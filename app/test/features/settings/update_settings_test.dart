// Die Zeile zu Updates in den Einstellungen (HUM-382): `update.check` und
// „Jetzt prüfen".

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/settings/widgets/setting_controls.dart';
import 'package:humanitl/features/shell/providers/language.dart';
import 'package:humanitl/l10n/l10n.dart';

import '../../harness/app_harness.dart';
import '../../harness/settings_harness.dart';

final Finder _toggle = find.byKey(const Key('settings-update-check'));
final Finder _checkNow = find.byKey(const Key('settings-update-check-now'));
final Finder _result = find.byKey(const Key('settings-update-result'));

Future<void> _open(WidgetTester tester, FakeDaemonClient client) async {
  await pumpApp(tester, client: client);
  await pressCtrl(tester, LogicalKeyboardKey.comma);
  await tester.pumpAndSettle();
  await showSettingsCategory(tester, SettingsCategory.daemon);
}

String _resultText(WidgetTester tester) =>
    tester.widget<Text>(_result).data ?? '';

void main() {
  testWidgets('the switch reads update.check, default on, and writes it', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.empty();
    await _open(tester, client);
    expect(tester.widget<OnOffControl>(_toggle).value, isTrue);

    await tapSegment(tester, const Key('settings-update-check'), 'Off');
    expect(client.configWrites.last, ('update.check', 'false'));
    expect(tester.widget<OnOffControl>(_toggle).value, isFalse);
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('Check now asks with force and says up to date', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.empty()
      ..updateStatus = const UpdateStatus(
        currentVersion: '0.0.42',
        latestVersion: '0.0.42',
      );
    await _open(tester, client);
    await tester.ensureVisible(_checkNow);
    await tester.pumpAndSettle();
    await tester.tap(_checkNow);
    await tester.pumpAndSettle();
    expect(client.updateChecks.last, isTrue);
    expect(_resultText(tester), 'Humanitl 0.0.42 is up to date.');
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('Check now names a newer release', (WidgetTester tester) async {
    final FakeDaemonClient client = FakeDaemonClient.empty()
      ..updateStatus = const UpdateStatus(
        currentVersion: '0.0.41',
        latestVersion: '0.0.42',
        available: true,
      );
    await _open(tester, client);
    await tester.ensureVisible(_checkNow);
    await tester.pumpAndSettle();
    await tester.tap(_checkNow);
    await tester.pumpAndSettle();
    expect(_resultText(tester), 'Humanitl 0.0.42 is available.');
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('Check now says why a check failed', (WidgetTester tester) async {
    final FakeDaemonClient client = FakeDaemonClient.empty();
    await _open(tester, client);
    client.updateCheckFailure = const Diagnostic(
      code: 'UPDATE_002',
      severity: Severity.error,
      why: 'the signature of update-macos.json does not match the key',
    );
    await tester.ensureVisible(_checkNow);
    await tester.pumpAndSettle();
    await tester.tap(_checkNow);
    await tester.pumpAndSettle();
    expect(
      _resultText(tester),
      'The check failed: the signature of update-macos.json does not match '
      'the key',
    );
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  // HUM-569: Der Grund wird beim Zeichnen übersetzt, nicht beim Scheitern
  // festgehalten; ein Wechsel der Sprache danach erreicht auch ihn.
  testWidgets('a failed check follows a later change of the language', (
    WidgetTester tester,
  ) async {
    const String sentence =
        'the signature of update-macos.json does not match the key';
    final FakeDaemonClient client = FakeDaemonClient.empty();
    await _open(tester, client);
    client.updateCheckFailure = const Diagnostic(
      code: 'UPDATE_002',
      severity: Severity.error,
      why: sentence,
    );
    await tester.ensureVisible(_checkNow);
    await tester.pumpAndSettle();
    await tester.tap(_checkNow);
    await tester.pumpAndSettle();
    expect(_resultText(tester), contains(sentence));

    ProviderScope.containerOf(tester.element(_result))
        .read(languageProvider.notifier)
        .set(AppLanguage.de);
    await tester.pumpAndSettle();

    // Die Einstellungen bauen ihre Zeilen beim Sprachwechsel neu auf (der
    // Katalog hängt an der Sprache); die Zeile darf danach fehlen, aber nie
    // den englischen Satz ohne Hinweis zeigen.
    final AppLocalizations de = lookupAppLocalizations(const Locale('de'));
    expect(find.textContaining(sentence), findsNothing);
    if (_result.evaluate().isNotEmpty) {
      expect(_resultText(tester), contains(de.diagUPDATE002Why));
    }
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));
}
