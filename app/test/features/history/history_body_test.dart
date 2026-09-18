// Die Rümpfe im History-Detail (HUM-116): dieselbe Ansicht wie in der
// Warteschlange. Baum, Formular oder Hex je nach Rumpf, ausgepackt nach den
// Kopfzeilen der eigenen Seite, und die Funde der Anfrage an ihrer Stelle.
// Jeder Test hier wird rot, sobald das Detail wieder eigene Textzeilen
// zeichnet oder einer Seite die Kopfzeilen oder die Funde der anderen reicht.

import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
// `Override` lebt in riverpod 3 im Nebeneingang `misc.dart`.
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/body/body_decode.dart';
import 'package:humanitl/core/body/body_view.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/flow_events.dart';
import 'package:humanitl/core/ipc/flow_reveal.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/text/format.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/history/history_table.dart';
import 'package:humanitl/features/history/history_view.dart';
import 'package:humanitl/features/history/providers/history_detail.dart';
import 'package:humanitl/features/history/providers/history_page.dart';
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

/// Pumpt [n] Bilder von je 200 ms.
Future<void> _settle(WidgetTester tester, [int n = 8]) async {
  for (int i = 0; i < n; i++) {
    await tester.pump(const Duration(milliseconds: 200));
  }
}

/// [finder], aber nur innerhalb des Blatts.
Finder _inSheet(Finder finder) =>
    find.descendant(of: find.byType(HSheet), matching: finder);

/// Öffnet [id] im Blatt, mitten im Strom, und stellt den Antwort-Tab ein.
///
/// Die Zeile steht danach auf `responded`, und das aufgezeichnete Detail hat
/// noch keinen Antwort-Rumpf: genau der Zustand, in dem der Recorder die
/// Antwortnachricht noch nicht geschrieben hat.
Future<void> _openInSheet(
  WidgetTester tester,
  ProviderContainer container,
  FakeDaemonClient client,
  FlowId id,
) async {
  final Flow row = container
      .read(historyPageProvider)
      .rows
      .firstWhere((Flow flow) => flow.id == id);
  final FlowDetail recorded = client.state.details[id]!;
  final Flow streaming = row.copyWith(state: FlowState.responded);
  client.state.flows[id] = streaming;
  client.state.details[id] = recorded.copyWith(
    summary: streaming,
    responseBody: null,
  );
  container
      .read(historyPageProvider.notifier)
      .applyEventForTest(
        FlowEvent.responseHeaders(
          at: row.receivedAt,
          flowId: id,
          head: HttpResponseHead(
            status: row.status,
            headers: const <Header>[],
            version: 'HTTP/1.1',
          ),
        ),
      );
  await _settle(tester, 4);

  final Finder target = find.byWidgetPredicate(
    (Widget widget) => widget is HistoryRow && widget.flow.id == id,
  );
  expect(target, findsOneWidget);
  await tester.tap(target);
  await tester.pump(kDoubleTapMinTime);
  await tester.tap(target);
  await tester.pump(kDoubleTapTimeout);
  await _settle(tester);
  expect(find.byType(HSheet), findsOneWidget);
  await tester.tap(
    _inSheet(find.byKey(const Key('history-tab-response'))).first,
  );
  await _settle(tester);
  expect(
    _inSheet(find.byKey(const Key('body-pending'))),
    findsOneWidget,
    reason: 'mid-stream the sheet is right to wait',
  );
}

/// Ein Fake, der zählt, wie oft nach einem Flow gefragt wird, und die erste
/// Antwort zurückhalten kann.
///
/// [gate] hält die Antwort an, bis der Test sie freigibt. Gelesen wird der
/// Stand beim Aufruf, nicht bei der Freigabe: so antwortet ein Daemon, der
/// gefragt wurde, bevor der Flow fertig war.
class _CountingClient extends FakeDaemonClient {
  _CountingClient() : super(script: <ScriptedEvent>[]);

  /// Wie oft `GetFlow` gefragt wurde, je Flow.
  final Map<FlowId, int> asked = <FlowId, int>{};

  /// Solange gesetzt und offen, wartet jede Antwort darauf.
  Completer<void>? gate;

  @override
  Future<FlowDetail> getFlow(FlowId id) async {
    asked[id] = (asked[id] ?? 0) + 1;
    final FlowDetail? captured = state.details[id];
    final Completer<void>? held = gate;
    if (held != null) {
      await held.future;
    }
    if (captured == null) {
      return super.getFlow(id);
    }
    return captured;
  }
}

/// Legt [detail] mit seinem Rumpf in [client] ab, ohne Zeile in der Seite.
void _adopt(
  _CountingClient client,
  FakeDaemonClient source,
  FlowDetail detail,
) {
  client.state.details[detail.summary.id] = detail;
  source.state.bodies.forEach((String key, Uint8List bytes) {
    client.state.bodies[key] = bytes;
  });
}

/// Öffnet [id] über `flowRevealProvider` im Blatt, Antwort-Tab.
Future<void> _revealInSheet(
  WidgetTester tester,
  ProviderContainer container,
  FlowId id,
) async {
  container.read(flowRevealProvider.notifier).request(id);
  await _settle(tester);
  expect(find.byType(HSheet), findsOneWidget);
  await tester.tap(
    _inSheet(find.byKey(const Key('history-tab-response'))).first,
  );
  await _settle(tester);
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

  testWidgets('an empty body is named by the body view, in fg1', (
    WidgetTester tester,
  ) async {
    // Ein Satz für einen leeren Rumpf, nicht zwei: das Detail reicht ihn an
    // die Rumpf-Ansicht weiter, statt ihn selbst zu beschriften (HUM-154).
    // Wer dem Detail wieder einen eigenen Weg gibt, verliert den Schlüssel
    // `body-empty` und diesen Test.
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final FlowDetail empty = _first(
      client,
      (FlowDetail detail) => detail.request?.body.isEmpty ?? false,
    );
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );

    await _show(tester, container, empty.summary.id);
    expect(find.byType(BodyView), findsOneWidget);
    final Text sentence = tester.widget<Text>(
      find.byKey(const Key('body-empty')),
    );
    expect(sentence.data, english.interceptBodyEmpty);
    expect(find.text(english.interceptBodyEmpty), findsOneWidget);
    // `fg2` misst 3,02:1 auf `bg3` und ist für Sätze zu schwach
    // (`docs/UX.md` 6, Sekundärtext).
    expect(sentence.style?.color, HTokens.dark.colors.fg1);
    expect(sentence.style?.color, isNot(HTokens.dark.colors.fg2));
  });

  testWidgets('a response that never came gets the same sentence', (
    WidgetTester tester,
  ) async {
    // Eine Seite ohne Rumpf und ein Rumpf ohne Inhalt sind dieselbe Aussage.
    // Auch dafür hat das Detail keinen eigenen Satz mehr: ein fehlendes
    // `BodyRef` geht durch dieselbe Ansicht, und weil an diesem Flow nichts
    // mehr ankommen kann, sagt sie denselben Satz in `fg1` (HUM-154).
    // `responseIsFinal` und nicht „nicht am Streamen": ein angehaltener Flow
    // hat auch keinen Antwort-Rumpf, aber seine Antwort kann noch kommen.
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final FlowDetail unanswered = _first(
      client,
      (FlowDetail detail) =>
          detail.responseBody == null && responseIsFinal(detail.summary),
    );
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );

    await _show(tester, container, unanswered.summary.id);
    await _tab(tester, 'response');
    expect(find.byType(BodyView), findsOneWidget);
    expect(find.byKey(const Key('body-pending')), findsNothing);
    final Text sentence = tester.widget<Text>(
      find.byKey(const Key('body-empty')),
    );
    expect(sentence.data, english.interceptBodyEmpty);
    expect(sentence.style?.color, HTokens.dark.colors.fg1);
  });

  testWidgets('a response still arriving waits, and stops when it lands', (
    WidgetTester tester,
  ) async {
    // Der Recorder schreibt die Antwortnachricht erst am Ende des Stroms,
    // während Zustand und Status schon stehen. Ein fehlender Rumpf heißt dann
    // „kommt noch", nicht „keiner da": erst steht das Skelett, nicht der Satz
    // (`docs/UX.md` 2.11).
    //
    // Und es bleibt nicht stehen. Das Detail wird je Auswahl einmal geholt,
    // die Zeile darüber lebt; ohne das Nachfassen beim Übergang auf
    // `recorded` bliebe das Skelett unter einem fertigen Kopf für immer
    // stehen, während der Rumpf im Daemon liegt.
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final FlowDetail answered = _plainJson(client);
    final FlowId id = answered.summary.id;
    final Flow streaming = answered.summary.copyWith(
      state: FlowState.responded,
    );
    client.state.flows[id] = streaming;
    client.state.details[id] = answered.copyWith(
      summary: streaming,
      responseBody: null,
    );
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );

    await _show(tester, container, id);
    await _tab(tester, 'response');
    expect(find.byKey(const Key('body-pending')), findsOneWidget);
    expect(find.byKey(const Key('body-empty')), findsNothing);
    // Die Anfrage daneben ist vollständig aufgezeichnet und wartet nicht.
    await _tab(tester, 'request');
    expect(find.byKey(const Key('body-pending')), findsNothing);

    await _tab(tester, 'response');
    client.state.flows[id] = answered.summary;
    client.state.details[id] = answered;
    container
        .read(historyPageProvider.notifier)
        .applyEventForTest(
          FlowEvent.recorded(at: answered.summary.receivedAt, flowId: id),
        );
    for (int i = 0; i < 6; i++) {
      await tester.pump(const Duration(milliseconds: 200));
    }
    expect(find.byKey(const Key('body-pending')), findsNothing);
    expect(find.byKey(const Key('body-empty')), findsNothing);
    expect(find.byKey(const Key('body-tree')), findsOneWidget);
  });

  testWidgets('the wait ends with the row, not with the snapshot', (
    WidgetTester tester,
  ) async {
    // Dieselbe Lage, aber der Daemon hat nichts Neues: das nachgeholte Detail
    // trägt weiter die Zusammenfassung von mitten im Strom. Gefragt wird
    // trotzdem die lebende Zeile, sonst wartete der Abschnitt für immer unter
    // einem Kopf, der längst fertig ist (`docs/UX.md` 2.11).
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final FlowDetail answered = _plainJson(client);
    final FlowId id = answered.summary.id;
    final Flow streaming = answered.summary.copyWith(
      state: FlowState.responded,
    );
    client.state.flows[id] = streaming;
    client.state.details[id] = answered.copyWith(
      summary: streaming,
      responseBody: null,
    );
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );

    await _show(tester, container, id);
    await _tab(tester, 'response');
    expect(find.byKey(const Key('body-pending')), findsOneWidget);

    container
        .read(historyPageProvider.notifier)
        .applyEventForTest(
          FlowEvent.recorded(at: answered.summary.receivedAt, flowId: id),
        );
    for (int i = 0; i < 6; i++) {
      await tester.pump(const Duration(milliseconds: 200));
    }
    expect(find.byKey(const Key('body-pending')), findsNothing);
    expect(find.byKey(const Key('body-empty')), findsOneWidget);
  });

  testWidgets('the sheet stops waiting when the recorded answer has no body', (
    WidgetTester tester,
  ) async {
    // Form A: das Blatt hält einen Abzug der Zeile, und kein Ereignis des
    // Daemons erreicht ihn. Wenn die aufgezeichnete Antwort keinen eigenen
    // Rumpf hat, sagt der geteilte Bereich darunter „kein Rumpf", während das
    // Blatt auf etwas wartet, das nie kommt (HUM-154).
    //
    // Nur ohne Rumpf zeigt sich das: die Ansicht fragt `pending` erst, wenn
    // kein `BodyRef` da ist. Ein Test über einen Flow mit Rumpf ginge grün,
    // egal was das Blatt über das Warten glaubt.
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    final Flow row = container
        .read(historyPageProvider)
        .rows
        .firstWhere((Flow flow) => !flow.isHeld && flow.status == 0);
    final FlowId id = row.id;
    await _openInSheet(tester, container, client, id);

    final FlowDetail recorded = client.state.details[id]!;
    final Flow done = row.copyWith(state: FlowState.recorded);
    client.state.flows[id] = done;
    client.state.details[id] = recorded.copyWith(summary: done);
    container
        .read(historyPageProvider.notifier)
        .applyEventForTest(FlowEvent.recorded(at: row.receivedAt, flowId: id));
    await _settle(tester, 12);

    expect(
      _inSheet(find.byKey(const Key('body-pending'))),
      findsNothing,
      reason: 'the sheet is stuck on the skeleton of a finished flow',
    );
    expect(_inSheet(find.byKey(const Key('body-empty'))), findsOneWidget);
  });

  testWidgets('the sheet stops waiting after the selection moved away', (
    WidgetTester tester,
  ) async {
    // Form B: das Blatt bleibt offen, die Auswahl zieht weiter. Dann sieht
    // niemand sonst diesen Flow an, und wenn das Blatt der Zeile nicht folgt,
    // holt auch niemand das Detail nach: das Skelett bleibt stehen, obwohl
    // der Daemon den Rumpf längst hat (HUM-154).
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    final Flow row = container
        .read(historyPageProvider)
        .rows
        .firstWhere(
          (Flow flow) =>
              !flow.isHeld &&
              flow.status != 0 &&
              !(client.state.details[flow.id]?.responseBody?.isEmpty ?? true),
        );
    final FlowId id = row.id;
    final FlowDetail answered = client.state.details[id]!;
    await _openInSheet(tester, container, client, id);

    final Flow other = container
        .read(historyPageProvider)
        .rows
        .firstWhere((Flow flow) => flow.id != id && !flow.isHeld);
    container.read(historySelectionProvider.notifier).select(other.id);
    await _settle(tester);

    final Flow done = row.copyWith(state: FlowState.recorded);
    client.state.flows[id] = done;
    client.state.details[id] = answered.copyWith(summary: done);
    container
        .read(historyPageProvider.notifier)
        .applyEventForTest(FlowEvent.recorded(at: row.receivedAt, flowId: id));
    await _settle(tester, 12);

    expect(
      _inSheet(find.byKey(const Key('body-pending'))),
      findsNothing,
      reason: 'the sheet is stuck on the skeleton after the selection left',
    );
    expect(_inSheet(find.byKey(const Key('body-empty'))), findsNothing);
    expect(_inSheet(find.byKey(const Key('body-tree'))), findsOneWidget);
  });

  testWidgets('the sheet follows a revealed flow that no row carries', (
    WidgetTester tester,
  ) async {
    // Ein Flow über `flowRevealProvider` steht in keiner geladenen Zeile —
    // das Blatt nimmt seine Zusammenfassung aus `GetFlow`. Dann ist die Zeile
    // keine Quelle, und ohne den Strom wartete der Rumpf-Abschnitt für immer
    // (HUM-154).
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final FlowDetail answered = _plainJson(client);
    final FlowId id = answered.summary.id;
    // Aufgezeichnet, aber in keiner Zeile: genau die Lage des Reveal-Wegs.
    client.state.flows.remove(id);
    client.state.details[id] = answered.copyWith(
      summary: answered.summary.copyWith(state: FlowState.responded),
      responseBody: null,
    );
    final StreamController<FlowEvent> events =
        StreamController<FlowEvent>.broadcast();
    addTearDown(events.close);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
      overrides: <Override>[
        flowEventsProvider.overrideWith((Ref ref) => events.stream),
      ],
    );

    container.read(flowRevealProvider.notifier).request(id);
    await _settle(tester);
    expect(find.byType(HSheet), findsOneWidget);
    await tester.tap(
      _inSheet(find.byKey(const Key('history-tab-response'))).first,
    );
    await _settle(tester);
    expect(
      _inSheet(find.byKey(const Key('body-pending'))),
      findsOneWidget,
      reason: 'mid-stream the sheet is right to wait',
    );

    client.state.details[id] = answered;
    events.add(FlowEvent.recorded(at: answered.summary.receivedAt, flowId: id));
    await _settle(tester, 12);
    expect(
      _inSheet(find.byKey(const Key('body-pending'))),
      findsNothing,
      reason: 'nothing but the stream can end this wait',
    );
    expect(_inSheet(find.byKey(const Key('body-tree'))), findsOneWidget);
  });

  testWidgets('a reload does not throw the sheet back to its first state', (
    WidgetTester tester,
  ) async {
    // Ein Flow, der aus der Seite fällt — gefiltert, weitergeblättert, aus
    // dem Fenster geschoben —, hat dort keine Zeile mehr. Fiele das Blatt
    // dann auf den Abzug vom Öffnen zurück, stünde über einem fertigen Flow
    // wieder das Skelett von mitten im Strom (HUM-154).
    //
    // Wieder einer ohne Antwort-Rumpf: mit Rumpf fragt die Ansicht `pending`
    // gar nicht, und der Test wäre grün, egal was das Blatt glaubt.
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    final Flow row = container
        .read(historyPageProvider)
        .rows
        .firstWhere((Flow flow) => !flow.isHeld && flow.status == 0);
    final FlowId id = row.id;
    await _openInSheet(tester, container, client, id);

    final FlowDetail recorded = client.state.details[id]!;
    client.state.details[id] = recorded.copyWith(
      summary: row.copyWith(state: FlowState.recorded),
    );
    container
        .read(historyPageProvider.notifier)
        .applyEventForTest(FlowEvent.recorded(at: row.receivedAt, flowId: id));
    await _settle(tester, 12);
    expect(_inSheet(find.byKey(const Key('body-pending'))), findsNothing);
    expect(_inSheet(find.byKey(const Key('body-empty'))), findsOneWidget);

    // Die Zeile verschwindet aus der Seite: ein Filter schließt sie aus, die
    // Fensterkante schiebt sie hinaus, ein Neuladen bringt sie nicht zurück.
    // Hier liefert der Fake sie schlicht nicht mehr.
    client.state.flows.remove(id);
    await container.read(historyPageProvider.notifier).reload();
    await _settle(tester);
    expect(
      container.read(historyPageProvider).rows.any((Flow row) => row.id == id),
      isFalse,
      reason: 'the row has to be gone for this test to say anything',
    );

    expect(
      _inSheet(find.byKey(const Key('body-pending'))),
      findsNothing,
      reason: 'the sheet fell back to the row it was opened with',
    );
    expect(_inSheet(find.byKey(const Key('body-empty'))), findsOneWidget);
  });

  testWidgets('a gap in the stream ends the sheet\'s wait too', (
    WidgetTester tester,
  ) async {
    // Eine Luecke traegt keine Id: was waehrend ihr geschah, weiss niemand.
    // Die Seite laedt darauf neu, das Blatt fragt nach seinem eigenen Flow —
    // sonst bliebe es auf dem Stand von vor der Luecke stehen (HUM-154).
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final FlowDetail answered = _plainJson(client);
    final FlowId id = answered.summary.id;
    client.state.flows.remove(id);
    client.state.details[id] = answered.copyWith(
      summary: answered.summary.copyWith(state: FlowState.responded),
      responseBody: null,
    );
    final StreamController<FlowEvent> events =
        StreamController<FlowEvent>.broadcast();
    addTearDown(events.close);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
      overrides: <Override>[
        flowEventsProvider.overrideWith((Ref ref) => events.stream),
      ],
    );

    container.read(flowRevealProvider.notifier).request(id);
    await _settle(tester);
    expect(find.byType(HSheet), findsOneWidget);
    await tester.tap(
      _inSheet(find.byKey(const Key('history-tab-response'))).first,
    );
    await _settle(tester);
    expect(_inSheet(find.byKey(const Key('body-pending'))), findsOneWidget);

    client.state.details[id] = answered;
    events.add(FlowEvent.lagged(at: answered.summary.receivedAt, dropped: 7));
    await _settle(tester, 12);
    expect(
      _inSheet(find.byKey(const Key('body-pending'))),
      findsNothing,
      reason: 'a gap leaves the sheet waiting for ever',
    );
    expect(_inSheet(find.byKey(const Key('body-tree'))), findsOneWidget);
  });

  testWidgets('the edited request arrives with the decision, not with the '
      'answer', (WidgetTester tester) async {
    // Der Proxy schreibt `Dir::RequestEdited` vor dem Weiterleiten
    // (`daemon/crates/proxy/src/handler.rs`), also steht sie fest, sobald die
    // Zeile sie meldet. Wer erst am Ende der Antwort nachfasst, laesst das
    // Skelett den ganzen Download ueber stehen (HUM-154).
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final FlowDetail edited = _first(
      client,
      (FlowDetail detail) => detail.editedRequest != null,
    );
    final FlowId id = edited.summary.id;
    // Vorher: entschieden ist noch nichts, der Abzug traegt keine bearbeitete
    // Anfrage.
    final Flow waiting = edited.summary.copyWith(
      state: FlowState.held,
      edited: false,
    );
    client.state.flows[id] = waiting;
    client.state.details[id] = edited.copyWith(
      summary: waiting,
      editedRequest: null,
    );
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    await _show(tester, container, id);

    // Jetzt entscheidet jemand, und der Daemon hat die bearbeitete Anfrage.
    client.state.details[id] = edited.copyWith(
      summary: edited.summary.copyWith(state: FlowState.decided),
    );
    container
        .read(historyPageProvider.notifier)
        .applyEventForTest(
          FlowEvent.decided(
            at: waiting.receivedAt,
            flowId: id,
            kind: DecisionKind.allowEdited,
          ),
        );
    await _settle(tester, 12);
    await _tab(tester, 'edited');
    expect(
      find.byKey(const Key('body-pending')),
      findsNothing,
      reason: 'the skeleton waits for an answer that is not the question',
    );
    expect(find.byKey(const Key('body-empty')), findsNothing);
  });

  testWidgets('a failed flow is fetched again once its record is written', (
    WidgetTester tester,
  ) async {
    // Ein Flow, der oben scheitert, ist schon `failed`, wenn er `recorded`
    // wird. `responseIsFinal` war da längst wahr; nachgefasst werden muss
    // trotzdem, denn erst der Datensatz trägt die bearbeitete Anfrage. Ohne
    // das sagte der Tab „kein Rumpf" über eine Anfrage, die jemand von Hand
    // geändert hat (HUM-154).
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final FlowDetail edited = _first(
      client,
      (FlowDetail detail) => detail.editedRequest != null,
    );
    final FlowId id = edited.summary.id;
    final Flow failed = edited.summary.copyWith(state: FlowState.failed);
    client.state.flows[id] = failed;
    client.state.details[id] = edited.copyWith(
      summary: failed,
      editedRequest: null,
    );
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    await _show(tester, container, id);
    await _tab(tester, 'edited');
    expect(find.byKey(const Key('body-pending')), findsOneWidget);

    client.state.details[id] = edited;
    container
        .read(historyPageProvider.notifier)
        .applyEventForTest(
          FlowEvent.recorded(at: failed.receivedAt, flowId: id),
        );
    await _settle(tester, 12);
    expect(find.byKey(const Key('body-pending')), findsNothing);
    expect(
      find.byKey(const Key('body-empty')),
      findsNothing,
      reason: 'the record came, but nobody asked for it',
    );
  });

  testWidgets('a finished sheet does not walk back to an older row', (
    WidgetTester tester,
  ) async {
    // Eine Seite, die nicht aufgefrischt ist, kann für dieselbe Id eine
    // ältere Zeile führen. Einmal fertig, immer fertig: sonst stünde über
    // einem aufgezeichneten Flow wieder das Skelett, und weil das
    // Endereignis verbraucht ist, bliebe es dort (HUM-154).
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    final Flow row = container
        .read(historyPageProvider)
        .rows
        .firstWhere((Flow flow) => !flow.isHeld && flow.status == 0);
    final FlowId id = row.id;
    await _openInSheet(tester, container, client, id);

    final FlowDetail recorded = client.state.details[id]!;
    client.state.details[id] = recorded.copyWith(
      summary: row.copyWith(state: FlowState.recorded),
    );
    final HistoryPageNotifier page = container.read(
      historyPageProvider.notifier,
    );
    page.applyEventForTest(FlowEvent.recorded(at: row.receivedAt, flowId: id));
    await _settle(tester, 12);
    expect(_inSheet(find.byKey(const Key('body-empty'))), findsOneWidget);

    // Dieselbe Id, ein älterer Stand.
    page.applyEventForTest(FlowEvent.forwarded(at: row.receivedAt, flowId: id));
    await _settle(tester, 12);
    expect(
      _inSheet(find.byKey(const Key('body-pending'))),
      findsNothing,
      reason: 'the sheet walked back from recorded to forwarded',
    );
    expect(_inSheet(find.byKey(const Key('body-empty'))), findsOneWidget);
  });

  testWidgets('an end that falls into the reveal fetch is not lost', (
    WidgetTester tester,
  ) async {
    // Zwischen `_sheetFlow = null` und der Antwort von `GetFlow` gibt es
    // keinen Abzug, an dem ein Ende erkannt werden könnte. Endet der Flow
    // genau dann, trägt die Antwort den Stand von vorher — und ohne ein
    // Nachholen säße das Blatt für immer darauf (HUM-154).
    final FakeDaemonClient source = FakeDaemonClient.history(count: 24);
    final FlowDetail answered = _plainJson(source);
    final FlowId id = answered.summary.id;
    final _CountingClient client = _CountingClient();
    _adopt(
      client,
      source,
      answered.copyWith(
        summary: answered.summary.copyWith(state: FlowState.responded),
        responseBody: null,
      ),
    );
    final StreamController<FlowEvent> events =
        StreamController<FlowEvent>.broadcast();
    addTearDown(events.close);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
      overrides: <Override>[
        flowEventsProvider.overrideWith((Ref ref) => events.stream),
      ],
    );

    final Completer<void> gate = Completer<void>();
    client.gate = gate;
    container.read(flowRevealProvider.notifier).request(id);
    await tester.pump();
    // Der Abruf läuft und trägt den Stand von mitten im Strom. Jetzt endet
    // der Flow, und der Daemon hat den Datensatz — einen ohne Antwort-Rumpf:
    // mit Rumpf fragte die Ansicht `pending` gar nicht, und der Test wäre
    // grün, egal auf welchem Stand das Blatt sitzt.
    events.add(FlowEvent.recorded(at: answered.summary.receivedAt, flowId: id));
    await tester.pump();
    client.state.details[id] = answered.copyWith(responseBody: null);
    client.gate = null;
    gate.complete();
    await _settle(tester);
    await tester.tap(
      _inSheet(find.byKey(const Key('history-tab-response'))).first,
    );
    await _settle(tester, 12);

    expect(
      _inSheet(find.byKey(const Key('body-pending'))),
      findsNothing,
      reason: 'the end came while the sheet was being fetched and was lost',
    );
    expect(_inSheet(find.byKey(const Key('body-empty'))), findsOneWidget);
  });

  testWidgets('a failure and its record ask the daemon once', (
    WidgetTester tester,
  ) async {
    // Der Daemon schickt `Failed` und `Recorded` direkt hintereinander.
    // Wer auf beide hört, fragt zweimal, und die zweite Frage läuft in die
    // erste hinein; `Recorded` kommt für jeden fertigen Flow, auch für den
    // gescheiterten (HUM-154).
    final FakeDaemonClient source = FakeDaemonClient.history(count: 24);
    final FlowDetail answered = _plainJson(source);
    final FlowId id = answered.summary.id;
    final _CountingClient client = _CountingClient();
    _adopt(
      client,
      source,
      answered.copyWith(
        summary: answered.summary.copyWith(state: FlowState.forwarded),
        responseBody: null,
      ),
    );
    final StreamController<FlowEvent> events =
        StreamController<FlowEvent>.broadcast();
    addTearDown(events.close);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
      overrides: <Override>[
        flowEventsProvider.overrideWith((Ref ref) => events.stream),
      ],
    );
    await _revealInSheet(tester, container, id);
    final int before = client.asked[id] ?? 0;

    client.state.details[id] = answered.copyWith(
      summary: answered.summary.copyWith(state: FlowState.failed),
      responseBody: null,
    );
    events.add(
      FlowEvent.failed(
        at: answered.summary.receivedAt,
        flowId: id,
        error: UpstreamError.connect,
      ),
    );
    await _settle(tester, 4);
    client.state.details[id] = answered.copyWith(
      summary: answered.summary.copyWith(state: FlowState.recorded),
      responseBody: null,
    );
    events.add(FlowEvent.recorded(at: answered.summary.receivedAt, flowId: id));
    await _settle(tester, 12);

    expect((client.asked[id] ?? 0) - before, 1);
    expect(_inSheet(find.byKey(const Key('body-empty'))), findsOneWidget);
  });

  testWidgets('an edit learnt from a gap is fetched once', (
    WidgetTester tester,
  ) async {
    // Nach einer Lücke holt das Blatt seinen Flow neu, und die Antwort trägt
    // die bearbeitete Anfrage schon. Dass die Zeile danach `edited` meldet,
    // ist dann kein Grund, noch einmal zu fragen (HUM-154).
    final FakeDaemonClient source = FakeDaemonClient.history(count: 24);
    final FlowDetail edited = _first(
      source,
      (FlowDetail detail) => detail.editedRequest != null,
    );
    final FlowId id = edited.summary.id;
    final _CountingClient client = _CountingClient();
    _adopt(
      client,
      source,
      edited.copyWith(
        summary: edited.summary.copyWith(state: FlowState.held, edited: false),
        editedRequest: null,
      ),
    );
    final StreamController<FlowEvent> events =
        StreamController<FlowEvent>.broadcast();
    addTearDown(events.close);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
      overrides: <Override>[
        flowEventsProvider.overrideWith((Ref ref) => events.stream),
      ],
    );
    await _revealInSheet(tester, container, id);
    final int before = client.asked[id] ?? 0;

    client.state.details[id] = edited.copyWith(
      summary: edited.summary.copyWith(state: FlowState.decided),
    );
    events.add(FlowEvent.lagged(at: edited.summary.receivedAt, dropped: 2));
    await _settle(tester, 12);
    expect((client.asked[id] ?? 0) - before, 1);
  });

  testWidgets('a gap asks nothing about a sheet that is already finished', (
    WidgetTester tester,
  ) async {
    // Fertig ist fertig: eine Lücke im Strom ändert an einem aufgezeichneten
    // Flow nichts mehr, und eine Frage danach wäre eine Frage ohne Grund.
    final FakeDaemonClient source = FakeDaemonClient.history(count: 24);
    final FlowDetail answered = _plainJson(source);
    final FlowId id = answered.summary.id;
    final _CountingClient client = _CountingClient();
    _adopt(client, source, answered);
    final StreamController<FlowEvent> events =
        StreamController<FlowEvent>.broadcast();
    addTearDown(events.close);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
      overrides: <Override>[
        flowEventsProvider.overrideWith((Ref ref) => events.stream),
      ],
    );
    await _revealInSheet(tester, container, id);
    final int before = client.asked[id] ?? 0;

    events.add(FlowEvent.lagged(at: answered.summary.receivedAt, dropped: 3));
    await _settle(tester, 8);
    expect((client.asked[id] ?? 0) - before, 0);
  });

  testWidgets('an edit the recorder had not written yet is asked again', (
    WidgetTester tester,
  ) async {
    // Der Daemon veröffentlicht `Decided`, bevor der Schreiber die
    // bearbeitete Anfrage übernimmt; ein `GetFlow` gleich danach kann ohne
    // sie zurückkommen. Jeder weitere Schritt des Flows fragt deshalb noch
    // einmal, solange sie fehlt (HUM-154).
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final FlowDetail edited = _first(
      client,
      (FlowDetail detail) => detail.editedRequest != null,
    );
    final FlowId id = edited.summary.id;
    final Flow waiting = edited.summary.copyWith(
      state: FlowState.held,
      edited: false,
    );
    client.state.flows[id] = waiting;
    // Der Schreiber ist noch nicht so weit: auch nach der Entscheidung fehlt
    // die bearbeitete Anfrage.
    client.state.details[id] = edited.copyWith(
      summary: waiting,
      editedRequest: null,
    );
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    await _show(tester, container, id);
    final HistoryPageNotifier page = container.read(
      historyPageProvider.notifier,
    );
    page.applyEventForTest(
      FlowEvent.decided(
        at: waiting.receivedAt,
        flowId: id,
        kind: DecisionKind.allowEdited,
      ),
    );
    await _settle(tester, 8);
    await _tab(tester, 'edited');
    expect(find.byKey(const Key('body-pending')), findsOneWidget);

    // Jetzt hat der Schreiber sie übernommen, und der Flow geht weiter.
    client.state.details[id] = edited.copyWith(
      summary: edited.summary.copyWith(state: FlowState.forwarded),
    );
    page.applyEventForTest(
      FlowEvent.forwarded(at: waiting.receivedAt, flowId: id),
    );
    await _settle(tester, 12);
    expect(
      find.byKey(const Key('body-pending')),
      findsNothing,
      reason: 'the edit was written, but nobody asked again',
    );
    expect(find.byKey(const Key('body-empty')), findsNothing);
  });

  testWidgets('a double click wins over a reveal still being fetched', (
    WidgetTester tester,
  ) async {
    // Ein Abruf über `flowRevealProvider` ist unterwegs, da öffnet ein
    // Doppelklick eine andere Zeile. Die späte Antwort gehört zu einem Blatt,
    // das es nicht mehr gibt, und darf das neue nicht überschreiben (HUM-154).
    final FakeDaemonClient source = FakeDaemonClient.history(count: 24);
    final FlowDetail revealed = _plainJson(source);
    final FlowDetail other = _first(
      source,
      (FlowDetail detail) =>
          detail.summary.id != revealed.summary.id &&
          detail.summary.host != revealed.summary.host &&
          !detail.summary.isHeld &&
          detail.summary.state.isTerminal,
    );
    final _CountingClient client = _CountingClient();
    _adopt(client, source, revealed);
    _adopt(client, source, other);
    client.state.flows[other.summary.id] = other.summary;
    final StreamController<FlowEvent> events =
        StreamController<FlowEvent>.broadcast();
    addTearDown(events.close);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
      overrides: <Override>[
        flowEventsProvider.overrideWith((Ref ref) => events.stream),
      ],
    );

    final Completer<void> gate = Completer<void>();
    client.gate = gate;
    container.read(flowRevealProvider.notifier).request(revealed.summary.id);
    await tester.pump();

    final Finder target = find.byWidgetPredicate(
      (Widget widget) =>
          widget is HistoryRow && widget.flow.id == other.summary.id,
    );
    expect(target, findsOneWidget);
    await tester.tap(target);
    await tester.pump(kDoubleTapMinTime);
    await tester.tap(target);
    await tester.pump(kDoubleTapTimeout);
    await _settle(tester, 4);
    final String otherTitle = english.historySheetTitle(
      other.summary.methodLabel,
      other.summary.host,
    );
    expect(_inSheet(find.text(otherTitle)), findsOneWidget);

    client.gate = null;
    gate.complete();
    await _settle(tester, 8);
    expect(
      _inSheet(find.text(otherTitle)),
      findsOneWidget,
      reason: 'the late reveal answer took over the sheet',
    );
  });

  testWidgets('a finished sheet still takes a fuller finished row', (
    WidgetTester tester,
  ) async {
    // `Recorded` setzt in der Seite nur den Zustand; Dauer und Größe bringt
    // erst ein Neuladen. Das Blatt friert den Zustand ein, nicht die Zeile —
    // sonst stünde in seinem Kopf weiter „—", während die Tabelle die Dauer
    // zeigt (HUM-154).
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    final Flow row = container
        .read(historyPageProvider)
        .rows
        .firstWhere((Flow flow) => !flow.isHeld && flow.status == 0);
    final FlowId id = row.id;
    await _openInSheet(tester, container, client, id);

    final FlowDetail recorded = client.state.details[id]!;
    final Flow thin = row.copyWith(state: FlowState.recorded, duration: null);
    client.state.details[id] = recorded.copyWith(summary: thin);
    final HistoryPageNotifier page = container.read(
      historyPageProvider.notifier,
    );
    page.applyEventForTest(FlowEvent.recorded(at: row.receivedAt, flowId: id));
    await _settle(tester, 8);
    expect(_inSheet(find.textContaining('4321')), findsNothing);

    client.state.flows[id] = thin.copyWith(
      duration: const Duration(milliseconds: 4321),
    );
    await page.reload();
    await _settle(tester, 8);
    expect(
      _inSheet(find.textContaining('4321')),
      findsOneWidget,
      reason: 'the sheet froze the whole row, not only its state',
    );

    // Und die vollere Zeile bleibt, wenn sie die Seite wieder verlässt: sie
    // ist der neue Abzug, nicht nur das, was gerade angezeigt wird. Sonst
    // fiele das Blatt beim nächsten Neuladen, Filter oder an der
    // Fensterkante auf die dünne Zeile zurück.
    client.state.flows.remove(id);
    await page.reload();
    await _settle(tester, 8);
    expect(
      container.read(historyPageProvider).rows.any((Flow row) => row.id == id),
      isFalse,
      reason: 'the row has to be gone for this test to say anything',
    );
    expect(
      _inSheet(find.textContaining('4321')),
      findsOneWidget,
      reason: 'the fuller row was shown but never kept',
    );
  });

  testWidgets('a thin finished row does not take what the sheet knew', (
    WidgetTester tester,
  ) async {
    // Das Blatt hat die volle Zusammenfassung schon geholt, die Seite führt
    // für dieselbe Id eine dünne. Zwischen zwei fertigen Zeilen darf die
    // dünne nur ergänzen, nichts wegnehmen: sonst stünde im Kopf wieder „—"
    // statt der Dauer (HUM-154).
    final FakeDaemonClient source = FakeDaemonClient.history(count: 24);
    final FlowDetail answered = _plainJson(source);
    final FlowId id = answered.summary.id;
    final Flow full = answered.summary.copyWith(
      state: FlowState.recorded,
      duration: const Duration(milliseconds: 4321),
    );
    final _CountingClient client = _CountingClient();
    _adopt(client, source, answered.copyWith(summary: full));
    final StreamController<FlowEvent> events =
        StreamController<FlowEvent>.broadcast();
    addTearDown(events.close);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
      overrides: <Override>[
        flowEventsProvider.overrideWith((Ref ref) => events.stream),
      ],
    );
    await _revealInSheet(tester, container, id);
    expect(_inSheet(find.textContaining('4321')), findsOneWidget);

    client.state.flows[id] = full.copyWith(duration: null);
    await container.read(historyPageProvider.notifier).reload();
    await _settle(tester, 8);
    expect(
      container.read(historyPageProvider).rows.any((Flow row) => row.id == id),
      isTrue,
      reason:
          'the thin row has to be on the page for this test to say '
          'anything',
    );
    expect(
      _inSheet(find.textContaining('4321')),
      findsOneWidget,
      reason: 'a thin finished row erased what the sheet already knew',
    );
  });

  testWidgets('an older fetch of the same revealed flow changes nothing', (
    WidgetTester tester,
  ) async {
    // Aufgedeckt, per Doppelklick verlassen, noch einmal aufgedeckt — und der
    // erste Abruf kommt erst danach zurück. Die Notiz nennt dann wieder
    // denselben Flow; allein an ihr sähe der alte Abruf aktuell aus und
    // setzte das Blatt auf seinen älteren Stand (HUM-154).
    final FakeDaemonClient source = FakeDaemonClient.history(count: 24);
    final FlowDetail revealed = _plainJson(source);
    final FlowId id = revealed.summary.id;
    final FlowDetail other = _first(
      source,
      (FlowDetail detail) =>
          detail.summary.id != id &&
          !detail.summary.isHeld &&
          detail.summary.state.isTerminal,
    );
    final _CountingClient client = _CountingClient();
    _adopt(client, source, other);
    client.state.flows[other.summary.id] = other.summary;
    final StreamController<FlowEvent> events =
        StreamController<FlowEvent>.broadcast();
    addTearDown(events.close);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
      overrides: <Override>[
        flowEventsProvider.overrideWith((Ref ref) => events.stream),
      ],
    );

    // Erster Abruf: ein älterer Stand ohne Dauer, angehalten.
    _adopt(
      client,
      source,
      revealed.copyWith(
        summary: revealed.summary.copyWith(
          state: FlowState.responded,
          duration: null,
        ),
      ),
    );
    final Completer<void> first = Completer<void>();
    client.gate = first;
    container.read(flowRevealProvider.notifier).request(id);
    await tester.pump();

    final Finder target = find.byWidgetPredicate(
      (Widget widget) =>
          widget is HistoryRow && widget.flow.id == other.summary.id,
    );
    await tester.tap(target);
    await tester.pump(kDoubleTapMinTime);
    await tester.tap(target);
    await tester.pump(kDoubleTapTimeout);
    await _settle(tester, 2);

    // Zweiter Abruf desselben Flows: der neuere Stand.
    client.state.details[id] = revealed.copyWith(
      summary: revealed.summary.copyWith(
        state: FlowState.recorded,
        duration: const Duration(milliseconds: 4321),
      ),
    );
    final Completer<void> second = Completer<void>();
    client.gate = second;
    container.read(flowRevealProvider.notifier).request(id);
    await tester.pump();

    // Der alte kommt zuerst zurück, der neue danach.
    client.gate = null;
    first.complete();
    await _settle(tester, 4);
    second.complete();
    await _settle(tester, 8);

    expect(
      _inSheet(find.textContaining('4321')),
      findsOneWidget,
      reason: 'the older fetch took the sheet for itself',
    );
  });

  testWidgets('response chunks do not ask again for a missing edit', (
    WidgetTester tester,
  ) async {
    // Nachgefasst wird bei einem Schritt des Flows, nicht bei jedem Paket:
    // in `responded` kommt je Paket eine neue Zeile, und ohne diese Grenze
    // fragte jedes davon den Daemon, solange die bearbeitete Anfrage fehlt
    // (HUM-154).
    final FakeDaemonClient source = FakeDaemonClient.history(count: 24);
    final FlowDetail edited = _first(
      source,
      (FlowDetail detail) => detail.editedRequest != null,
    );
    final FlowId id = edited.summary.id;
    final Flow streaming = edited.summary.copyWith(
      state: FlowState.responded,
      edited: true,
    );
    final _CountingClient client = _CountingClient();
    _adopt(
      client,
      source,
      edited.copyWith(summary: streaming, editedRequest: null),
    );
    client.state.flows[id] = streaming;
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    await _show(tester, container, id);
    final int before = client.asked[id] ?? 0;

    final HistoryPageNotifier page = container.read(
      historyPageProvider.notifier,
    );
    for (int chunk = 1; chunk <= 5; chunk++) {
      page.applyEventForTest(
        FlowEvent.responseChunk(
          at: streaming.receivedAt,
          flowId: id,
          bytesSoFar: chunk * 1000,
        ),
      );
      await _settle(tester, 2);
    }
    expect((client.asked[id] ?? 0) - before, lessThanOrEqualTo(1));
  });

  testWidgets('an edited request of a failed flow still waits', (
    WidgetTester tester,
  ) async {
    // `failed` ist kein Ende: `daemon/crates/core-types/src/flow.rs` kennt
    // `Failed` + `Record` = `Recorded`, und `fail_closed` bringt jeden
    // Zustand dorthin. Bis dahin ist die bearbeitete Anfrage nicht
    // geschrieben, und „kein Rumpf" wäre eine Behauptung über etwas, das ein
    // Mensch von Hand geändert hat.
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final FlowDetail edited = _first(
      client,
      (FlowDetail detail) => detail.editedRequest != null,
    );
    final FlowId id = edited.summary.id;
    final Flow failed = edited.summary.copyWith(state: FlowState.failed);
    client.state.flows[id] = failed;
    client.state.details[id] = edited.copyWith(
      summary: failed,
      editedRequest: null,
    );
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );

    await _show(tester, container, id);
    await _tab(tester, 'edited');
    expect(find.byKey(const Key('body-pending')), findsOneWidget);
    expect(find.byKey(const Key('body-empty')), findsNothing);
  });

  testWidgets('an edited request that is not written yet waits too', (
    WidgetTester tester,
  ) async {
    // Der Recorder schreibt die bearbeitete Anfrage mit dem Rest am Ende.
    // Zwischen Entscheidung und `recorded` fehlt sie, und das heißt nicht
    // „kein Rumpf" und schon gar nicht „0 B" (`backlog/CONVENTIONS.md` 4.13).
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final FlowDetail edited = _first(
      client,
      (FlowDetail detail) => detail.editedRequest != null,
    );
    final FlowId id = edited.summary.id;
    final Flow running = edited.summary.copyWith(state: FlowState.decided);
    client.state.flows[id] = running;
    client.state.details[id] = edited.copyWith(
      summary: running,
      editedRequest: null,
    );
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );

    await _show(tester, container, id);
    await _tab(tester, 'edited');
    expect(find.byKey(const Key('body-pending')), findsOneWidget);
    expect(find.byKey(const Key('body-empty')), findsNothing);
  });

  testWidgets('a flow that is still held claims nothing about its answer', (
    WidgetTester tester,
  ) async {
    // Eine angehaltene Anfrage hat noch keine Antwort, und ihr Rumpf ist
    // deshalb keine Aussage: „Body (0 B, unknown type)" behauptete eine
    // Größe, die niemand kennt (`backlog/CONVENTIONS.md` 4.13). Der Zustand
    // `held` ist weder `recorded` noch `failed`, also wird gewartet.
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final FlowDetail waiting = _first(
      client,
      (FlowDetail detail) => detail.summary.state == FlowState.held,
    );
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );

    await _show(tester, container, waiting.summary.id);
    await _tab(tester, 'response');
    expect(find.byKey(const Key('body-pending')), findsOneWidget);
    expect(find.byKey(const Key('body-empty')), findsNothing);
    // Und darüber ebenso: auf Warten antwortet dieser Bildschirm mit einem
    // Skelett, nicht mit „Es kam keine Antwort zurück." über eine Antwort,
    // die noch kommen kann (`docs/UX.md` 2.11).
    expect(
      find.byKey(const Key('history-detail-headers-pending')),
      findsOneWidget,
    );
    expect(find.text(english.historyDetailNoResponse), findsNothing);
    expect(find.text(english.historyDetailNoHeaders), findsNothing);
  });
}
