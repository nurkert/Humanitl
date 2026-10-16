// Golden der Pause mit offenen Funden (HUM-049, HUM-054): drei Funde, drei
// Entscheidungen und der Weg zurück, an der Stelle der Aktionsleiste. In den
// vier Varianten aus `support/golden_variants.dart`.
//
// Erneuern mit `make goldens-update`.

import 'package:flutter/widgets.dart' hide Flow;
// `Override` lebt in riverpod 3 im Nebeneingang `misc.dart`.
import 'package:flutter_riverpod/misc.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/features/intercept/providers/findings_pause.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/providers/now.dart';
import 'package:humanitl/features/intercept/widgets/action_bar.dart';
import 'package:humanitl/features/shell/providers/connection.dart';

import '../features/intercept/fixtures.dart';
import '../harness/ui_state.dart';
import 'support/golden_variants.dart';

/// Die Uhr: 40 Sekunden nach dem Anhalten.
final DateTime goldenNow = testStart.add(const Duration(seconds: 40));

/// Eine Anfrage mit drei Funden: ein Schlüssel, eine Adresse, eine IBAN.
FlowDetail threeFindings() => detailFor(
  heldFlow(
    n: 1,
    deadline: testStart.add(const Duration(minutes: 5)),
    method: Method.post,
    host: 'api.example.com',
    apex: 'example.com',
    path: '/v1/chat',
    requestSize: 612,
  ).copyWith(findingCount: 3),
  apex: 'example.com',
  findings: <Finding>[
    testFinding(),
    testFinding(kind: 'email'),
    testFinding(kind: 'iban'),
  ],
);

/// Eine Pause, die über [id] offen steht.
class OpenPause extends OpenFindingsPause {
  /// Offen über [id].
  OpenPause(this.id);

  /// Die Anfrage der Pause.
  final FlowId id;

  @override
  FlowId? build() => id;
}

void main() {
  goldenVariants(
    'findings_pause',
    constraints: const BoxConstraints.tightFor(width: 900, height: 320),
    builder: (GoldenVariant variant) {
      final FlowDetail detail = threeFindings();
      final TestDaemonClient client = TestDaemonClient();
      client.details[detail.summary.id] = detail;
      return goldenFrame(
        variant: variant,
        overrides: <Override>[
          uiStateOverride(),
          daemonClientProvider.overrideWithValue(client),
          connectionHeartbeatProvider.overrideWithValue(null),
          nowProvider.overrideWith(() => FixedNow(goldenNow)),
          flowsProvider.overrideWith(
            () => FixedFlows(<FlowId, Flow>{detail.summary.id: detail.summary}),
          ),
          openFindingsPauseProvider.overrideWith(
            () => OpenPause(detail.summary.id),
          ),
        ],
        child: Align(
          alignment: Alignment.bottomCenter,
          child: ActionBar(flow: detail.summary, onPseudonymize: () {}),
        ),
      );
    },
  );
}
