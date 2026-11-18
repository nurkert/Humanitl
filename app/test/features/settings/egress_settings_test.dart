// Der Abschnitt „Entscheidungs-Proxy" der Einstellungen (HUM-511): Adresse,
// CA-Datei, wer auflöst, Zugangsdaten über `Egress`, „Verbindung prüfen",
// und die Zeile „Egress" im Isolations-Reiter. Gerechnet wird nichts davon
// in der App: Sie schreibt über `SetConfig` und `Egress` und zeigt, was der
// Daemon antwortet (ADR-018).

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/sandbox/widgets/isolation_panel.dart';
import 'package:humanitl/features/settings/categories/egress_category.dart';
import 'package:humanitl/features/settings/registry/settings_registry.dart';
import 'package:humanitl/l10n/l10n.dart';

import '../../harness/app_harness.dart';
import '../../harness/settings_harness.dart';

final TargetPlatformVariant _linux = TargetPlatformVariant.only(
  TargetPlatform.linux,
);

Future<void> _open(
  WidgetTester tester,
  FakeDaemonClient client, {
  List<Override> overrides = const <Override>[],
}) async {
  await pumpApp(tester, client: client, overrides: overrides);
  await pressCtrl(tester, LogicalKeyboardKey.comma);
  await tester.pumpAndSettle();
  await showSettingsCategory(tester, SettingsCategory.egress);
}

Future<void> _tap(WidgetTester tester, Key key) async {
  await tester.ensureVisible(find.byKey(key));
  await tester.pumpAndSettle();
  await tester.tap(find.byKey(key));
  await tester.pumpAndSettle();
}

String _text(WidgetTester tester, Key key) =>
    tester.widget<Text>(find.byKey(key)).data ?? '';

void main() {
  testWidgets('the address, the CA file and the resolver are written as '
      'keys of config.toml', (WidgetTester tester) async {
    final FakeDaemonClient client = FakeDaemonClient.empty();
    await _open(
      tester,
      client,
      overrides: <Override>[
        caFileChooserProvider.overrideWithValue(
          ({String? dialogTitle}) async =>
              '/home/u/.mitmproxy/mitmproxy-ca-cert.pem',
        ),
      ],
    );

    final Finder address = find.byKey(const Key('settings-egress-proxy'));
    await tester.ensureVisible(address);
    await tester.enterText(address, 'http://127.0.0.1:8080');
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pumpAndSettle();
    expect(client.configWrites.last, (
      FormKeys.egressProxy,
      'http://127.0.0.1:8080',
    ));

    await _tap(tester, const Key('settings-egress-ca-choose'));
    expect(client.configWrites.last, (
      FormKeys.egressProxyCa,
      '/home/u/.mitmproxy/mitmproxy-ca-cert.pem',
    ));

    await tapSegment(tester, const Key('settings-egress-resolve'), 'Humanitl');
    expect(client.configWrites.last, (FormKeys.egressProxyResolve, 'humanitl'));

    // Ein leeres Feld nimmt die Adresse heraus, statt sie leer zu setzen.
    await tester.enterText(address, '');
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pumpAndSettle();
    expect(client.configWrites.last, (FormKeys.egressProxy, '-'));
  }, variant: _linux);

  testWidgets('credentials go through Egress, the field keeps nothing, and '
      'the state comes from the daemon', (WidgetTester tester) async {
    final FakeDaemonClient client = FakeDaemonClient.empty();
    await _open(tester, client);
    expect(
      _text(tester, const Key('settings-egress-auth-state')),
      'No credentials stored',
    );

    final Finder input = find.byKey(const Key('settings-egress-auth-input'));
    await tester.ensureVisible(input);
    await tester.enterText(input, 'alice:pw');
    await _tap(tester, const Key('settings-egress-auth-save'));
    expect(client.egressCalls, contains(EgressOp.setAuth));
    expect(client.egressStatus.authSet, isTrue);
    expect(
      tester.widget<EditableText>(input).controller.text,
      isEmpty,
      reason: 'the typed credentials do not stay on the screen',
    );
    expect(
      _text(tester, const Key('settings-egress-auth-state')),
      'Stored in the keyring',
    );

    await _tap(tester, const Key('settings-egress-auth-remove'));
    expect(client.egressCalls, contains(EgressOp.removeAuth));
    expect(
      _text(tester, const Key('settings-egress-auth-state')),
      'No credentials stored',
    );

    // Eine Absage des Daemons steht an der Zeile.
    await tester.enterText(input, 'no-colon');
    await _tap(tester, const Key('settings-egress-auth-save'));
    expect(
      find.byKey(const Key('settings-egress-auth-refusal')),
      findsOneWidget,
    );
  }, variant: _linux);

  testWidgets('Check connection shows success, or the finding of the daemon', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.empty()
      ..egressCheckResult = const EgressStatus(
        checkTarget: 'example.com:443',
        reachable: true,
      );
    await _open(tester, client);
    await _tap(tester, const Key('settings-egress-check-now'));
    expect(client.egressCalls, contains(EgressOp.check));
    expect(
      _text(tester, const Key('settings-egress-check-result')),
      'Tunnel to example.com:443 and TLS handshake through the proxy '
      'succeeded.',
    );

    client.egressCheckResult = const EgressStatus(
      checkTarget: 'example.com:443',
      diagnostics: <Diagnostic>[
        Diagnostic(
          code: 'PROXY_014',
          severity: Severity.error,
          why: 'the decision proxy at 127.0.0.1:8080 answered 407',
        ),
      ],
    );
    await _tap(tester, const Key('settings-egress-check-now'));
    expect(
      _text(tester, const Key('settings-egress-check-result')),
      contains('answered 407'),
    );
  }, variant: _linux);

  testWidgets('a pending restart is said at the check row', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.empty()
      ..egressStatus = const EgressStatus(
        configured: EgressRoute(proxy: '127.0.0.1:8080', resolve: 'proxy'),
      );
    await _open(tester, client);
    expect(find.byKey(const Key('settings-egress-restart')), findsOneWidget);
  }, variant: _linux);

  testWidgets('the restart hint stands right after the address is saved', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.empty();
    await _open(tester, client);
    final Finder hint = find.byKey(const Key('settings-egress-restart'));
    expect(hint, findsNothing);

    // Was der Daemon nach dem Schreiben sagen wird; gelesen wird es erst,
    // wenn die App neu fragt.
    client.egressStatus = const EgressStatus(
      configured: EgressRoute(proxy: '127.0.0.1:8080', resolve: 'proxy'),
    );
    final Finder address = find.byKey(const Key('settings-egress-proxy'));
    await tester.ensureVisible(address);
    await tester.enterText(address, 'http://127.0.0.1:8080');
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pumpAndSettle();
    expect(hint, findsOneWidget);
  }, variant: _linux);

  test('the decision proxy rows come from the catalog, in its order', () async {
    final SettingsView view = await fakeSettingsView();
    expect(
      view
          .inCategory(SettingsCategory.egress)
          .map((SettingEntry entry) => entry.id),
      <String>[
        FormKeys.egressProxy,
        FormKeys.egressProxyCa,
        FormKeys.egressProxyResolve,
        FormKeys.egressProxyAuthRef,
        SettingIds.egressCheck,
      ],
    );
  });

  testWidgets('the isolation tab names the way out and who resolves', (
    WidgetTester tester,
  ) async {
    Future<String> line(EgressRoute route) async {
      await tester.pumpWidget(
        ProviderScope(
          overrides: <Override>[
            daemonClientProvider.overrideWithValue(FakeDaemonClient.empty()),
          ],
          child: WidgetsApp(
            color: const Color(0xFF000000),
            localizationsDelegates: AppLocalizations.localizationsDelegates,
            supportedLocales: AppLocalizations.supportedLocales,
            builder: (BuildContext context, Widget? _) => HTheme(
              tokens: HTokens.dark,
              child: IsolationPanel(
                status: SandboxStatus(egress: route),
                onShowArgv: () {},
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      return _text(tester, const Key('sandbox-isolation-egress'));
    }

    expect(await line(EgressRoute.direct), startsWith('Egress: direct.'));
    expect(
      await line(const EgressRoute(proxy: '127.0.0.1:8080', resolve: 'proxy')),
      'Egress: through 127.0.0.1:8080. The proxy resolves names and sees the '
      'decrypted traffic.',
    );
    expect(
      await line(
        const EgressRoute(proxy: '127.0.0.1:8080', resolve: 'humanitl'),
      ),
      contains('Humanitl resolves'),
    );
  }, variant: _linux);
}
