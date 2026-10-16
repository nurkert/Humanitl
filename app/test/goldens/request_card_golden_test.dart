// Goldens der Request-Karte (HUM-020, HUM-030, HUM-054): JSON mit neun
// Kopfzeilen und einem Fund im Baum, Formular, Binärrumpf in Hex, ein Rumpf
// über der Anzeigegrenze und eine abgelaufene Anfrage mit stiller Leiste. Je
// in den vier Varianten aus `support/golden_variants.dart`.
//
// Erneuern mit `make goldens-update`.

import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/foundation.dart' show listEquals;
import 'package:flutter/widgets.dart' hide Flow;
// `Override` lebt in riverpod 3 im Nebeneingang `misc.dart`.
import 'package:flutter_riverpod/misc.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/providers/now.dart';
import 'package:humanitl/features/intercept/widgets/action_bar.dart';
import 'package:humanitl/features/intercept/widgets/request_card.dart';
import 'package:humanitl/features/shell/providers/connection.dart';

import '../features/intercept/fixtures.dart';
import '../harness/ui_state.dart';
import 'support/golden_variants.dart';

/// Die Uhr aller Bilder dieser Datei: 40 Sekunden nach dem Anhalten.
final DateTime goldenNow = testStart.add(const Duration(seconds: 40));

/// Ein Client, der zu jedem Verweis genau die Bytes liefert, die der Test ihm
/// gegeben hat, auch Bytes, die kein UTF-8 sind.
///
/// Der Doppelgänger aus `fixtures.dart` liefert den Text von `bodyPreview`;
/// ein PNG-Kopf oder ein abgeschnittener Rumpf von 50 MB ist so nicht zu
/// beschreiben.
class BytesDaemonClient extends TestDaemonClient {
  /// Die Bytes je Prüfsumme.
  final List<(List<int>, Uint8List)> bodies = <(List<int>, Uint8List)>[];

  @override
  Stream<BodyChunk> getBodyChunks(BodyRef ref) {
    for (final (List<int> sha, Uint8List bytes) in bodies) {
      if (listEquals(sha, ref.sha256)) {
        return Stream<BodyChunk>.value(BodyChunk(data: bytes, last: true));
      }
    }
    return super.getBodyChunks(ref);
  }
}

/// Eine gehaltene POST-Anfrage an [host] mit [bytes] als Rumpf.
///
/// [size] ist, was der Daemon ankündigt; ohne Angabe die Länge von [bytes].
/// [seed] macht die Prüfsumme der Karte eindeutig.
({FlowDetail detail, Uint8List bytes}) post({
  required int seed,
  required String host,
  required String path,
  required String contentType,
  required Uint8List bytes,
  List<Header> headers = const <Header>[],
  List<Finding> findings = const <Finding>[],
  int? size,
  bool truncated = false,
}) {
  final Flow flow = heldFlow(
    n: seed,
    deadline: goldenNow.add(const Duration(minutes: 4, seconds: 20)),
    method: Method.post,
    host: host,
    path: path,
    requestSize: size ?? bytes.length,
  ).copyWith(findingCount: findings.length);
  return (
    detail: FlowDetail(
      summary: flow,
      findings: findings,
      request: HttpRequest(
        method: flow.method,
        scheme: flow.scheme,
        authority: flow.authority,
        pathAndQuery: flow.path,
        headers: headers,
        body: BodyRef(
          sha256: List<int>.filled(32, seed),
          size: size ?? bytes.length,
          truncated: truncated,
          contentType: contentType,
        ),
        version: 'HTTP/1.1',
      ),
    ),
    bytes: bytes,
  );
}

/// Ein Fund im Rumpf über `[start, end)`.
Finding bodyFinding(
  int start,
  int end, {
  String kind = 'email',
  FindingTier tier = FindingTier.regex,
}) => Finding(
  kind: kind,
  location: FindingLocation.body,
  spanStart: start,
  spanEnd: end,
  tier: tier,
  valueHash: List<int>.filled(32, start),
);

/// JSON mit einem GitHub-Schlüssel darin und neun Kopfzeilen.
({FlowDetail detail, Uint8List bytes}) jsonRequest() {
  const String body =
      '{"repo":"acme/api","token":"ghp_A1B2C3D4E5","labels":["bug"],'
      '"assignee":{"login":"octo","mail":"octo@example.org"},"draft":false}';
  final int token = body.indexOf('ghp_A1B2C3D4E5');
  return post(
    seed: 1,
    host: 'api.github.com',
    path: '/repos/acme/api/issues',
    contentType: 'application/json',
    bytes: Uint8List.fromList(utf8.encode(body)),
    headers: <Header>[
      header('accept', 'application/vnd.github+json'),
      header('authorization', 'Bearer ghp_R8kQexample'),
      header('content-type', 'application/json'),
      header('content-length', '${utf8.encode(body).length}'),
      header('user-agent', 'opencode/0.4.2'),
      header('x-github-api-version', '2022-11-28'),
      header('accept-encoding', 'gzip, br'),
      header('x-request-id', '6f1c2d4e-0b9a-4c71-8d55-1e2f3a4b5c6d'),
      header('connection', 'keep-alive'),
    ],
    findings: <Finding>[
      bodyFinding(
        token,
        token + 14,
        kind: 'api_key:github',
        tier: FindingTier.checksum,
      ),
    ],
  );
}

/// Ein Formular mit einer Adresse darin.
({FlowDetail detail, Uint8List bytes}) formRequest() {
  const String body =
      'grant_type=refresh_token&scope=read+write&mail=a%40b.de&state=xyz';
  final int mail = body.indexOf('a%40b.de');
  return post(
    seed: 2,
    host: 'auth.example.com',
    path: '/oauth/token',
    contentType: 'application/x-www-form-urlencoded',
    bytes: Uint8List.fromList(utf8.encode(body)),
    headers: <Header>[
      header('content-type', 'application/x-www-form-urlencoded'),
    ],
    findings: <Finding>[bodyFinding(mail, mail + 8)],
  );
}

/// Bytes, die kein Text sind: ein PNG-Kopf und Rauschen.
({FlowDetail detail, Uint8List bytes}) binaryRequest() => post(
  seed: 3,
  host: 'uploads.example.com',
  path: '/v1/blobs',
  contentType: 'application/octet-stream',
  bytes: Uint8List.fromList(<int>[
    0x89,
    0x50,
    0x4E,
    0x47,
    0x0D,
    0x0A,
    0x1A,
    0x0A,
    ...List<int>.generate(152, (int i) => (i * 37 + 11) % 256),
  ]),
  headers: <Header>[header('content-type', 'application/octet-stream')],
);

/// Ein Rumpf von 50 MB, von dem nur der Anfang geladen wird.
({FlowDetail detail, Uint8List bytes}) largeRequest() {
  final Uint8List prefix = Uint8List.fromList(
    utf8.encode('{"rows":[${List<String>.filled(64, '{"id":1}').join(',')}'),
  );
  return post(
    seed: 4,
    host: 'storage.googleapis.com',
    path: '/upload/storage/v1/b/acme/o?uploadType=media',
    contentType: 'application/json',
    bytes: prefix,
    size: 50 * 1000 * 1000,
    headers: <Header>[header('content-type', 'application/json')],
  );
}

/// Feste Uhr, feste Queue und ein Client mit den Bytes von [requests].
List<Override> overridesFor(
  List<({FlowDetail detail, Uint8List bytes})> requests,
) {
  final BytesDaemonClient client = BytesDaemonClient();
  for (final ({FlowDetail detail, Uint8List bytes}) request in requests) {
    client.details[request.detail.summary.id] = request.detail;
    client.bodies.add((request.detail.request!.body.sha256, request.bytes));
  }
  return <Override>[
    uiStateOverride(),
    daemonClientProvider.overrideWithValue(client),
    connectionHeartbeatProvider.overrideWithValue(null),
    nowProvider.overrideWith(() => FixedNow(goldenNow)),
    flowsProvider.overrideWith(
      () => FixedFlows(<FlowId, Flow>{
        for (final ({FlowDetail detail, Uint8List bytes}) request in requests)
          request.detail.summary.id: request.detail.summary,
      }),
    ),
  ];
}

/// Die Karte zu [request] in der Variante [variant].
Widget card(
  GoldenVariant variant,
  ({FlowDetail detail, Uint8List bytes}) request,
) => goldenFrame(
  variant: variant,
  overrides: overridesFor(<({FlowDetail detail, Uint8List bytes})>[request]),
  child: RequestCard(flow: request.detail.summary),
);

void main() {
  const BoxConstraints cardBox = BoxConstraints.tightFor(
    width: 680,
    height: 700,
  );

  goldenVariants(
    'request_card_json',
    constraints: cardBox,
    builder: (GoldenVariant variant) => card(variant, jsonRequest()),
  );

  goldenVariants(
    'request_card_form',
    constraints: const BoxConstraints.tightFor(width: 680, height: 360),
    builder: (GoldenVariant variant) => card(variant, formRequest()),
  );

  goldenVariants(
    'request_card_binary',
    constraints: const BoxConstraints.tightFor(width: 680, height: 420),
    builder: (GoldenVariant variant) => card(variant, binaryRequest()),
  );

  goldenVariants(
    'request_card_large',
    constraints: const BoxConstraints.tightFor(width: 680, height: 420),
    builder: (GoldenVariant variant) => card(variant, largeRequest()),
  );

  // Die Frist lief ab, bevor jemand entschied: das Band in der Karte und die
  // Leiste darunter, deren Entscheidungen nichts mehr tun.
  goldenVariants(
    'request_card_timed_out',
    constraints: const BoxConstraints.tightFor(width: 900, height: 520),
    builder: (GoldenVariant variant) {
      final ({FlowDetail detail, Uint8List bytes}) held = formRequest();
      final Flow timedOut = held.detail.summary.copyWith(
        state: FlowState.decided,
        decision: DecisionKind.timedOut,
        decisionSource: DecisionSource.timeout,
        deadline: goldenNow.subtract(const Duration(seconds: 1)),
      );
      final ({FlowDetail detail, Uint8List bytes}) request = (
        detail: held.detail.copyWith(summary: timedOut),
        bytes: held.bytes,
      );
      return goldenFrame(
        variant: variant,
        overrides: overridesFor(<({FlowDetail detail, Uint8List bytes})>[
          request,
        ]),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            Expanded(child: RequestCard(flow: timedOut)),
            const HHairline(),
            ActionBar(flow: timedOut),
          ],
        ),
      );
    },
  );
}
