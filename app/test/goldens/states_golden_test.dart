// Goldens der Fehler- und Randzustände aus HUM-058, je in den vier Varianten
// aus `support/golden_variants.dart`.
//
// Die Spezifikation nennt elf Zustände unter `app/test/goldens/states/`, je
// dunkel und hell. Seit HUM-054 hat jedes Golden vier Varianten und steht mit
// seinem Namen in `support/golden_names.dart`; dem folgt diese Datei, mit dem
// Präfix `states_` statt eines Unterordners. Vier der elf Zustände hat HUM-054
// schon in dieser Form festgehalten, und sie stehen hier nicht ein zweites
// Mal: `isolation_failed` ist `isolation_panel_failed`, `timeout_banner` ist
// `request_card_timed_out`, `large_body` ist `request_card_large`,
// `binary_body` ist `request_card_binary`. `queue_pressure` fehlt mit seiner
// Zeile 16 (siehe `backlog/sprint-5.md`, HUM-058); `recorder_failure` kam mit
// HUM-250.
//
// Erneuern mit `make goldens-update`.

import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:humanitl/app.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/providers/diagnostics.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/providers/now.dart';
import 'package:humanitl/features/intercept/providers/recorder_health.dart';
import 'package:humanitl/features/intercept/widgets/action_bar.dart';
import 'package:humanitl/features/intercept/widgets/request_card.dart';
import 'package:humanitl/features/shell/providers/connection.dart';

import '../features/intercept/fixtures.dart';
import '../features/sandbox/harness.dart';
import '../harness/ui_state.dart';
import 'intercept_golden_test.dart' as intercept;
import 'support/golden_variants.dart';

/// Die ganze App über [client], in [variant].
Widget app(
  GoldenVariant variant,
  DaemonClient client, {
  List<Override> overrides = const <Override>[],
}) => ProviderScope(
  overrides: <Override>[
    uiStateOverride(),
    daemonClientProvider.overrideWithValue(client),
    connectionHeartbeatProvider.overrideWithValue(null),
    connectionReconnectProvider.overrideWithValue(null),
    ...variant.appOverrides,
    ...overrides,
  ],
  child: const HumanitlApp(),
);

/// Ein Verbindungsaufbau an `ws.example.com`, der auf eine Entscheidung
/// wartet.
FlowDetail websocketRequest() => detailFor(
  heldFlow(
    n: 7,
    deadline: intercept.goldenNow.add(const Duration(minutes: 4, seconds: 20)),
    host: 'ws.example.com',
    path: '/v1/realtime?session=42',
  ),
  headers: <Header>[
    header('host', 'ws.example.com'),
    header('connection', 'Upgrade'),
    header('upgrade', 'websocket'),
    header('sec-websocket-version', '13'),
    header('sec-websocket-key', 'dGhlIHNhbXBsZSBub25jZQ=='),
  ],
);

void main() {
  const BoxConstraints window = BoxConstraints.tightFor(
    width: 1280,
    height: 800,
  );

  // Zeile 1: kein Daemon beim Start. Setup-Bildschirm, Karte mit Abhilfe und
  // die Zeile, die die Versuche zählt.
  goldenVariants(
    'states_daemon_missing',
    constraints: window,
    builder: (GoldenVariant variant) =>
        app(variant, FakeDaemonClient.scenario('daemon_missing')),
  );

  // Zeile 3: ein Daemon mit anderem Vertrag.
  goldenVariants(
    'states_incompatible',
    constraints: window,
    builder: (GoldenVariant variant) =>
        app(variant, FakeDaemonClient.incompatible()),
  );

  // Zeile 5: ein Start der Sandbox, der scheiterte.
  goldenVariants(
    'states_sandbox_failed',
    constraints: window,
    pumpBeforeTest: (tester) async {
      await tester.pump();
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 600));
    },
    builder: (GoldenVariant variant) => sandboxUnderTest(
      client: blockedClient(),
      mode: variant.mode,
      locale: variant.locale,
    ),
  );

  // Zeile 14: das Werkzeug in der Sandbox lehnt die CA ab.
  goldenVariants(
    'states_tls_rejected',
    constraints: window,
    builder: (GoldenVariant variant) {
      final List<FlowDetail> details = intercept.goldenDetails();
      return app(
        variant,
        TestDaemonClient(),
        overrides: <Override>[
          nowProvider.overrideWith(() => FixedNow(intercept.goldenNow)),
          flowsProvider.overrideWith(
            () => FixedFlows(<FlowId, Flow>{
              for (final FlowDetail detail in details)
                detail.summary.id: detail.summary,
            }),
          ),
          diagnosticsProvider.overrideWith(
            () => intercept.FixedDiagnostics(<SessionDiagnostic>[
              intercept.goldenDiagnostic(),
            ]),
          ),
        ],
      );
    },
  );

  // Zeile 11: eine Anfrage, die eine WebSocket-Verbindung öffnet.
  goldenVariants(
    'states_websocket_card',
    constraints: const BoxConstraints.tightFor(width: 900, height: 560),
    builder: (GoldenVariant variant) {
      final FlowDetail detail = websocketRequest();
      final TestDaemonClient client = TestDaemonClient();
      client.details[detail.summary.id] = detail;
      return goldenFrame(
        variant: variant,
        overrides: <Override>[
          uiStateOverride(),
          daemonClientProvider.overrideWithValue(client),
          connectionHeartbeatProvider.overrideWithValue(null),
          nowProvider.overrideWith(() => FixedNow(intercept.goldenNow)),
          flowsProvider.overrideWith(
            () => FixedFlows(<FlowId, Flow>{detail.summary.id: detail.summary}),
          ),
        ],
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            Expanded(child: RequestCard(flow: detail.summary)),
            const HHairline(),
            ActionBar(flow: detail.summary),
          ],
        ),
      );
    },
  );

  // Zeile 20: Die Aufzeichnung schreibt nicht (HUM-250). Statusleiste in der
  // Farbe des Fehlers, der Hinweis auf der Karte und die Pause vor der
  // ersten Freigabe an der Stelle der Entscheidungsleiste.
  goldenVariants(
    'states_recorder_failure',
    constraints: window,
    builder: (GoldenVariant variant) {
      final List<FlowDetail> details = intercept.goldenDetails();
      final TestDaemonClient client = TestDaemonClient();
      for (final FlowDetail detail in details) {
        client.details[detail.summary.id] = detail;
      }
      return app(
        variant,
        client,
        overrides: <Override>[
          nowProvider.overrideWith(() => FixedNow(intercept.goldenNow)),
          flowsProvider.overrideWith(
            () => FixedFlows(<FlowId, Flow>{
              for (final FlowDetail detail in details)
                detail.summary.id: detail.summary,
            }),
          ),
          recorderHealthProvider.overrideWith(
            () => _FixedOutage(
              RecorderOutage(
                since: testStart,
                diagnostic: const Diagnostic(
                  code: 'RECORDER_003',
                  severity: Severity.error,
                  title: 'Aufzeichnung konnte nicht schreiben',
                  why:
                      'could not commit a recorder transaction (database or '
                      'disk is full); the rows of this batch are lost',
                ),
              ),
            ),
          ),
          recorderPauseProvider.overrideWith(_OpenPause.new),
        ],
      );
    },
  );
}

/// Eine Störung der Aufzeichnung, die steht, statt aus dem Strom zu kommen.
class _FixedOutage extends RecorderHealth {
  _FixedOutage(this.outage);

  final RecorderOutage outage;

  @override
  RecorderOutage? build() => outage;
}

/// Die Pause der Aufzeichnung, offen und bereit.
class _OpenPause extends RecorderPause {
  @override
  RecorderPauseState build() => RecorderPauseState.armed;
}
