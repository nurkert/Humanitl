// Der Frage-Modus `upstream` in der App (HUM-512): „Proxy dahinter" als
// vierte Wahl in den Einstellungen, wählbar nur mit eingetragenem
// Entscheidungs-Proxy, die Moduszeile im Isolations-Reiter und das Wort in
// der Regelspalte der Historie. Entschieden wird nichts davon in der App:
// Sie schreibt `hold.ask_mode` über `SetConfig` und zeigt, was der Daemon in
// `SandboxEvent.Status.ask_mode` und `DECISION_SOURCE_UPSTREAM` sagt
// (ADR-018).

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/convert.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ipc/generated/humanitl/v1/humanitl.pb.dart' as pb;
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/history/history_view.dart';
import 'package:humanitl/features/sandbox/widgets/isolation_panel.dart';
import 'package:humanitl/l10n/l10n.dart';

import '../../harness/app_harness.dart';
import '../../harness/settings_harness.dart';

final TargetPlatformVariant _linux = TargetPlatformVariant.only(
  TargetPlatform.linux,
);

/// Ein Daemon mit diesem Frage-Modus und, wenn [proxy] nicht leer ist, einem
/// Entscheidungs-Proxy in `config.toml`.
FakeDaemonClient _client({String askMode = 'ui', String proxy = ''}) {
  final FakeDaemonClient client = FakeDaemonClient.empty()
    ..configValues = ConfigValues(
      values: <String, Object?>{
        'hold': <String, Object?>{'timeout_secs': 300, 'ask_mode': askMode},
        if (proxy.isNotEmpty)
          'egress': <String, Object?>{
            'proxy': 'http://$proxy',
            'proxy_resolve': 'proxy',
          },
      },
      configPath: '/home/u/.config/humanitl/config.toml',
    );
  if (proxy.isNotEmpty) {
    client.egressStatus = EgressStatus(
      configured: EgressRoute(proxy: proxy, resolve: 'proxy'),
    );
  }
  return client;
}

Future<void> _openRequests(WidgetTester tester, FakeDaemonClient client) async {
  await pumpApp(tester, client: client);
  await pressCtrl(tester, LogicalKeyboardKey.comma);
  await tester.pumpAndSettle();
  await showSettingsCategory(tester, SettingsCategory.requests);
}

void main() {
  testWidgets('without a decision proxy the fourth choice is off and says '
      'why', (WidgetTester tester) async {
    final FakeDaemonClient client = _client();
    await _openRequests(tester, client);

    expect(
      find.byKey(const Key('settings-ask-mode-needs-proxy')),
      findsOneWidget,
    );
    await tapSegment(tester, const Key('settings-ask-mode'), 'Proxy behind');
    expect(
      client.configWrites.where(
        ((String, String) write) => write.$1 == FormKeys.holdAskMode,
      ),
      isEmpty,
      reason: 'a segment that is off writes nothing',
    );
  }, variant: _linux);

  testWidgets('with a decision proxy the fourth choice writes upstream and '
      'names the proxy', (WidgetTester tester) async {
    final FakeDaemonClient client = _client(proxy: '127.0.0.1:8080');
    await _openRequests(tester, client);

    expect(
      find.byKey(const Key('settings-ask-mode-needs-proxy')),
      findsNothing,
    );
    await tapSegment(tester, const Key('settings-ask-mode'), 'Proxy behind');
    expect(client.configWrites.last, (FormKeys.holdAskMode, 'upstream'));
    await tester.pumpAndSettle();
    expect(
      tester
          .widget<Text>(find.byKey(const Key('settings-ask-mode-upstream')))
          .data,
      startsWith('Without a rule: decided by 127.0.0.1:8080.'),
    );
  }, variant: _linux);

  testWidgets('the isolation tab carries the mode line', (
    WidgetTester tester,
  ) async {
    Future<String?> line(String? askMode, EgressRoute route) async {
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
                status: SandboxStatus(egress: route, askMode: askMode),
                onShowArgv: () {},
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      final Finder found = find.byKey(const Key('sandbox-isolation-ask-mode'));
      return found.evaluate().isEmpty ? null : tester.widget<Text>(found).data;
    }

    const EgressRoute proxy = EgressRoute(
      proxy: '127.0.0.1:8080',
      resolve: 'proxy',
    );
    expect(
      await line('upstream', proxy),
      'Without a rule: decided by 127.0.0.1:8080. Nothing waits here except a '
      'redact rule. The proxy resolves names, so a public name that points '
      'into your network is its to refuse.',
    );
    expect(
      await line(
        'upstream',
        const EgressRoute(proxy: '127.0.0.1:8080', resolve: 'humanitl'),
      ),
      contains('Humanitl resolves and checks every address first'),
    );
    expect(
      await line('upstream', EgressRoute.direct),
      startsWith('Without a rule: the decision proxy should decide, but'),
      reason: 'no proxy is active: no "decided by ." with an empty address',
    );
    expect(
      await line('ui', EgressRoute.direct),
      'Without a rule: asked in the app.',
    );
    expect(
      await line('none', EgressRoute.direct),
      'Without a rule: refused at once.',
    );
    expect(
      await line(null, EgressRoute.direct),
      isNull,
      reason: 'a daemon before 1.42 says nothing, and the line is missing',
    );
  }, variant: _linux);

  test('the snapshot carries the ask mode, and a flow decided upstream is no '
      'manual decision', () {
    final SandboxStatus status = pb.SandboxEvent_Status(askMode: 'upstream')
        .toDomain();
    expect(status.askMode, 'upstream');
    expect(pb.SandboxEvent_Status().toDomain().askMode, isNull);

    final Flow flow = pb.FlowSummary(
      flowId: '018f0004-0000-7000-8000-000000000512',
      sessionId: '018f0001-0000-7000-8000-000000000001',
      path: '/x',
      authority: pb.Authority(host: 'example.com', port: 443),
      state: pb.FlowState.FLOW_STATE_RECORDED,
      decision: pb.DecisionKind.DECISION_KIND_ALLOW,
      decisionSource: pb.DecisionSource.DECISION_SOURCE_UPSTREAM,
      status: 403,
    ).toDomain();
    expect(flow.decisionSource, DecisionSource.upstream);
    expect(historyDecider(flow), HistoryDecider.upstream);
    expect(historyForwarded(flow), isTrue);
    final AppLocalizations en = lookupAppLocalizations(const Locale('en'));
    final AppLocalizations de = lookupAppLocalizations(const Locale('de'));
    expect(historyForwardedLabel(en, flow), 'Forwarded, proxy answered 403');
    expect(
      historyForwardedLabel(de, flow),
      'Weitergereicht, Proxy antwortete 403',
    );
  });
}
