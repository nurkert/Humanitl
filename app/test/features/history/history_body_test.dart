// Die Rümpfe im History-Detail (HUM-116): dieselbe Ansicht wie in der
// Warteschlange. Baum, Formular oder Hex je nach Rumpf, ausgepackt nach den
// Kopfzeilen der eigenen Seite, und die Funde der Anfrage an ihrer Stelle.
// Jeder Test hier wird rot, sobald das Detail wieder eigene Textzeilen
// zeichnet oder einer Seite die Kopfzeilen oder die Funde der anderen reicht.

import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/body/body_decode.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/text/format.dart';
import 'package:humanitl/features/history/providers/history_detail.dart';
import 'package:humanitl/l10n/l10n.dart';

import 'harness.dart';

/// Die englischen Texte, ohne einen Baum zu bauen.
final AppLocalizations english = lookupAppLocalizations(const Locale('en'));

/// Der Schlüssel, unter dem der Fake die Bytes zu [reference] hält.
String _keyOf(BodyRef reference) => reference.sha256
    .map((int byte) => byte.toRadixString(16).padLeft(2, '0'))
    .join();

/// Das erste Detail des Szenarios, auf das [where] passt.
FlowDetail _first(
  FakeDaemonClient client,
  bool Function(FlowDetail detail) where,
) => client.state.details.values.firstWhere(where);

/// Die erste aufgezeichnete Anfrage mit ungepacktem JSON-Rumpf.
FlowDetail _plainJson(FakeDaemonClient client) => _first(
  client,
  (FlowDetail detail) =>
      detail.summary.status == 200 &&
      detail.request!.body.contentType == 'application/json' &&
      !detail.request!.body.isEmpty &&
      contentEncodingOf(detail.request!.headers).isEmpty &&
      detail.findings.isEmpty,
);

/// Legt [bytes] als Anfrage-Rumpf von [detail] ab und gibt das neue Detail
/// zurück.
FlowDetail _recordRequest(
  FakeDaemonClient client,
  FlowDetail detail,
  List<int> bytes, {
  required String contentType,
}) {
  final BodyRef reference = detail.request!.body.copyWith(
    size: bytes.length,
    contentType: contentType,
  );
  client.state.bodies[_keyOf(reference)] = Uint8List.fromList(bytes);
  final FlowDetail changed = detail.copyWith(
    request: detail.request!.copyWith(body: reference),
  );
  client.state.details[detail.summary.id] = changed;
  return changed;
}

/// Zeigt [id] im Detail und wartet, bis der Rumpf zerlegt dasteht.
Future<void> _show(
  WidgetTester tester,
  ProviderContainer container,
  FlowId id,
) async {
  container.read(historySelectionProvider.notifier).select(id);
  for (int i = 0; i < 6; i++) {
    await tester.pump(const Duration(milliseconds: 200));
  }
}

/// Wechselt auf den Tab [name] und wartet auf seinen Rumpf.
Future<void> _tab(WidgetTester tester, String name) async {
  await tester.tap(find.byKey(Key('history-tab-$name')));
  for (int i = 0; i < 6; i++) {
    await tester.pump(const Duration(milliseconds: 200));
  }
}

/// Jeder unterstrichene Abschnitt der Rohansicht, in Lesereihenfolge.
List<String> _underlined(WidgetTester tester) {
  final List<String> marked = <String>[];
  final Finder texts = find.descendant(
    of: find.byKey(const Key('body-raw')),
    matching: find.byType(RichText),
  );
  for (final Element element in texts.evaluate()) {
    (element.widget as RichText).text.visitChildren((InlineSpan span) {
      if (span is TextSpan &&
          span.style?.decoration == TextDecoration.underline) {
        marked.add(span.text ?? '');
      }
      return true;
    });
  }
  return marked;
}

void main() {
  testWidgets('history_detail_uses_body_view', (WidgetTester tester) async {
    // Ein JSON-Rumpf zeigt den Baum, ein Formular die Tabelle, Binärdaten
    // die Hex-Ansicht. Das alte Detail zeichnete für alle drei Textzeilen,
    // für Binärdaten nur einen Satz über ihre Größe.
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final FlowDetail json = _plainJson(client);
    final FlowDetail form = _recordRequest(
      client,
      _first(
        client,
        (FlowDetail detail) => detail.summary.method == Method.patch,
      ),
      utf8.encode('name=nils&team=platform'),
      contentType: 'application/x-www-form-urlencoded',
    );
    // Die Telemetrie des Szenarios geht als Protobuf hinaus.
    final FlowDetail binary = _first(
      client,
      (FlowDetail detail) =>
          detail.request?.body.contentType == 'application/x-protobuf',
    );
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );

    await _show(tester, container, json.summary.id);
    expect(find.byKey(const Key('body-tree')), findsOneWidget);

    await _show(tester, container, form.summary.id);
    expect(find.byKey(const Key('body-form')), findsOneWidget);
    expect(find.textContaining('platform', findRichText: true), findsWidgets);

    await _show(tester, container, binary.summary.id);
    expect(find.byKey(const Key('body-hex')), findsOneWidget);
    expect(find.byKey(const Key('body-tree')), findsNothing);
  });

  testWidgets('history_gzip_body_is_readable', (WidgetTester tester) async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    // Die Anfrage: der gepackte Upload des Szenarios, `Content-Encoding:
    // gzip` in ihren eigenen Kopfzeilen.
    final FlowDetail upload = _first(
      client,
      (FlowDetail detail) =>
          contentEncodingOf(detail.request?.headers ?? const <Header>[]) ==
          'gzip',
    );
    // Die Antwort: gepackt nach den Kopfzeilen der Antwort, nicht der
    // Anfrage. Wer dem Antwort-Tab die Kopfzeilen der Anfrage reicht, zeigt
    // hier Hex statt Text.
    final FlowDetail answered = _plainJson(client);
    final List<int> packed = gzip.encode(
      utf8.encode('{"answer": "unpacked in the view"}'),
    );
    final BodyRef reference = answered.responseBody!.copyWith(
      size: packed.length,
    );
    client.state.bodies[_keyOf(reference)] = Uint8List.fromList(packed);
    client.state.details[answered.summary.id] = answered.copyWith(
      responseBody: reference,
      response: answered.response!.copyWith(
        headers: <Header>[
          ...answered.response!.headers,
          Header(name: 'content-encoding', value: utf8.encode('gzip')),
        ],
      ),
    );
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );

    await _show(tester, container, upload.summary.id);
    expect(find.byKey(const Key('body-tree')), findsOneWidget);
    expect(find.byKey(const Key('body-hex')), findsNothing);
    expect(
      find.textContaining(upload.summary.id.value, findRichText: true),
      findsWidgets,
    );

    await _show(tester, container, answered.summary.id);
    await _tab(tester, 'response');
    expect(find.byKey(const Key('body-tree')), findsOneWidget);
    expect(
      find.textContaining('unpacked in the view', findRichText: true),
      findsWidgets,
    );
  });

  testWidgets('history_request_findings_are_marked', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final FlowDetail plain = _plainJson(client);
    final String id = plain.summary.id.value;
    final String body = utf8.decode(
      client.state.bodies[_keyOf(plain.request!.body)]!,
    );
    final int start = body.indexOf(id);
    client.state.details[plain.summary.id] = plain.copyWith(
      findings: <Finding>[
        Finding(
          kind: 'email',
          location: FindingLocation.body,
          spanStart: start,
          spanEnd: start + id.length,
          tier: FindingTier.regex,
        ),
      ],
    );
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );

    await _show(tester, container, plain.summary.id);
    expect(find.byKey(const Key('body-finding-chip-0')), findsOneWidget);
    await tester.tap(find.text(english.interceptBodyPaneRaw));
    await tester.pump();
    expect(_underlined(tester), <String>[id]);

    // Die Antwort trägt dieselbe Flow-Id im Text, aber der Daemon hat sie
    // nicht durchsucht: keine Chips, keine Markierung.
    await _tab(tester, 'response');
    expect(find.byKey(const Key('body-raw')), findsOneWidget);
    expect(find.byKey(const Key('body-finding-chip-0')), findsNothing);
    expect(_underlined(tester), isEmpty);
  });

  testWidgets('a body the recorder cut short says so', (
    WidgetTester tester,
  ) async {
    // Das alte Detail sagte es in einem eigenen Satz; die Rumpf-Ansicht sagt
    // es jetzt selbst, auf beiden Bildschirmen (CONVENTIONS 4.13).
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final FlowDetail plain = _plainJson(client);
    final BodyRef reference = plain.request!.body;
    client.state.details[plain.summary.id] = plain.copyWith(
      request: plain.request!.copyWith(
        body: reference.copyWith(truncated: true),
      ),
    );
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );

    await _show(tester, container, plain.summary.id);
    expect(
      find.text(
        english.interceptBodyRecorderStopped(formatBytes(reference.size)),
      ),
      findsOneWidget,
    );
  });
}
