// Der Editor zeigt über dem Senden dieselbe Zeile wie die Karte, wenn die
// Anfrage eine WebSocket-Verbindung öffnet (HUM-058, Zeile 11, Review): Mit
// `E` geht er auch auf, bevor die Karte ihre Zeile zeigen konnte.
//
// Gegen welche Umkehrung der Test gerichtet ist: der Zweig mit
// `WebSocketNotice` im Fuß des Editors weg.

import 'dart:typed_data';

import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/body/body_kind.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/websocket_notice.dart';
import 'package:humanitl/features/editor/providers/draft_provider.dart';

import '../intercept/fixtures.dart';
import 'editor_screen_test.dart' show pumpEditor;

DraftSource source(List<Header> headers) => DraftSource(
  request: HttpRequest(
    method: Method.get,
    scheme: Scheme.https,
    authority: const Authority(host: 'ws.example.com', port: 443),
    pathAndQuery: '/socket',
    headers: headers,
    body: BodyRef(sha256: List<int>.filled(32, 7), size: 0),
  ),
  findings: const <Finding>[],
  bodyText: '',
  bodyKind: BodyKind.text,
  bodyBytes: Uint8List(0),
);

void main() {
  testWidgets('the editor names a websocket above send', (
    WidgetTester tester,
  ) async {
    await pumpEditor(
      tester,
      source: source(<Header>[
        header('Host', 'ws.example.com'),
        header('Connection', 'Upgrade'),
        header('Upgrade', 'websocket'),
      ]),
    );
    expect(find.byKey(const Key('editor-websocket')), findsOneWidget);
    expect(find.byType(WebSocketNotice), findsOneWidget);
    expect(
      find.textContaining('Opens a lasting connection to ws.example.com'),
      findsOneWidget,
    );
  });

  testWidgets('an ordinary request gets no such line', (
    WidgetTester tester,
  ) async {
    await pumpEditor(
      tester,
      source: source(<Header>[header('Host', 'ws.example.com')]),
    );
    expect(find.byKey(const Key('editor-websocket')), findsNothing);
  });
}
