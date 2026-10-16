// Goldens des Editors (HUM-047, HUM-161, HUM-054): Original und Entwurf mit
// Funden, der Glanz nach „Alle ersetzen", das aufgeklappte Mapping und der
// Hinweis auf kaputtes JSON. Je in den vier Varianten aus
// `support/golden_variants.dart`.
//
// Erneuern mit `make goldens-update`.

import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/widgets.dart' hide Flow;
// `Override` lebt in riverpod 3 im Nebeneingang `misc.dart`.
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/editor/editor_screen.dart';
import 'package:humanitl/features/editor/model/draft.dart';
import 'package:humanitl/features/editor/providers/draft_provider.dart';

import '../features/intercept/fixtures.dart';
import 'support/golden_variants.dart';

const FlowId editorFlow = FlowId('01930000-0000-7000-8000-0000000000ff');

const SessionId editorSession = SessionId(
  '01930000-0000-7000-8000-00000000aaaa',
);

/// Ein Fake, für den jeder Fluss angehalten ist.
///
/// Der Entwurf hört auf den Ereignisstrom und fragt nach einer Lücke nach
/// seinem Fluss; einer, den der Fake nicht kennt, räumte ihn weg.
class AlwaysHeld extends FakeDaemonClient {
  /// Ein leeres Skript zur Testzeit.
  AlwaysHeld() : super(script: const <ScriptedEvent>[], clock: () => testStart);

  @override
  Future<FlowDetail> getFlow(FlowId id) async {
    final FlowDetail detail = detailFor(
      heldFlow(n: 1, deadline: testStart.add(const Duration(minutes: 5))),
    );
    return detail.copyWith(summary: detail.summary.copyWith(id: id));
  }
}

/// Ein Fund im Rumpf über `[start, end)`.
Finding bodyFinding(
  int start,
  int end, {
  String kind = 'email',
  FindingTier tier = FindingTier.regex,
  int hash = 1,
}) => Finding(
  kind: kind,
  location: FindingLocation.body,
  spanStart: start,
  spanEnd: end,
  tier: tier,
  valueHash: List<int>.filled(32, hash),
);

/// Ein JSON-Rumpf mit zwei Adressen und einem AWS-Schlüssel.
DraftSource jsonSource() {
  const String body =
      '{"to":"a@x.de","cc":"b@y.de",\n'
      ' "key":"AKIAIOSFODNN7EXAMPLE",\n'
      ' "text":"please review"}';
  int at(String needle) => body.indexOf(needle);
  return DraftSource(
    session: editorSession,
    request: HttpRequest(
      method: Method.post,
      scheme: Scheme.https,
      authority: const Authority(host: 'api.example.com', port: 443),
      pathAndQuery: '/v1/chat',
      headers: <Header>[
        header('Content-Type', 'application/json'),
        header('Content-Length', '${utf8.encode(body).length}'),
        header('Host', 'api.example.com'),
      ],
      body: BodyRef(
        sha256: List<int>.filled(32, 7),
        size: utf8.encode(body).length,
      ),
    ),
    findings: <Finding>[
      bodyFinding(at('a@x.de'), at('a@x.de') + 6),
      bodyFinding(at('b@y.de'), at('b@y.de') + 6, hash: 2),
      bodyFinding(
        at('AKIA'),
        at('AKIA') + 20,
        kind: 'api_key:aws',
        tier: FindingTier.checksum,
        hash: 3,
      ),
    ],
    bodyText: body,
    bodyKind: BodyKind.json,
    bodyBytes: Uint8List.fromList(utf8.encode(body)),
  );
}

/// Der Editor über [source] in der Variante [variant].
Widget editor(GoldenVariant variant, {bool replaceAll = false}) => goldenFrame(
  variant: variant,
  overrides: <Override>[daemonClientProvider.overrideWithValue(AlwaysHeld())],
  child: EditorScreen(
    flowId: editorFlow,
    source: jsonSource(),
    replaceAllOnOpen: replaceAll,
    onClose: () {},
    onSend: (EditedRequest request, List<Replacement> replacements) {},
  ),
);

/// Lässt den Editor fertig laden: den Zeitgeber des Fakes, den Entwurf, der
/// erst nach dem ersten Rahmen entsteht, und den Rahmen, der ihn zeichnet.
Future<void> settleEditor(WidgetTester tester) async {
  await tester.pump(const Duration(milliseconds: 500));
  await tester.pump();
  await tester.pump();
  await tester.pumpAndSettle();
}

/// Dasselbe, danach das Mapping aufgeklappt.
Future<void> openMapping(WidgetTester tester) async {
  await settleEditor(tester);
  await tester.tap(find.byKey(const Key('editor-mapping-toggle')));
  await tester.pumpAndSettle();
}

/// Dasselbe, danach eine Klammer zu wenig im Entwurf.
Future<void> breakJson(WidgetTester tester) async {
  await settleEditor(tester);
  await tester.enterText(
    find.descendant(
      of: find.byKey(const Key('editor-draft-body')),
      matching: find.byType(EditableText),
    ),
    '{"to":"a@x.de","cc":"b@y.de",\n "text":"please review"',
  );
  await tester.pumpAndSettle();
}

void main() {
  const BoxConstraints window = BoxConstraints.tightFor(
    width: 1200,
    height: 760,
  );

  goldenVariants(
    'editor_split_findings',
    constraints: window,
    pumpBeforeTest: settleEditor,
    builder: (GoldenVariant variant) => editor(variant),
  );

  goldenVariants(
    'editor_replaced_glow',
    constraints: window,
    pumpBeforeTest: settleEditor,
    builder: (GoldenVariant variant) => editor(variant, replaceAll: true),
  );

  goldenVariants(
    'editor_mapping_open',
    constraints: window,
    pumpBeforeTest: openMapping,
    builder: (GoldenVariant variant) => editor(variant, replaceAll: true),
  );

  goldenVariants(
    'editor_json_invalid',
    constraints: window,
    pumpBeforeTest: breakJson,
    builder: (GoldenVariant variant) => editor(variant),
  );
}
