/// Der Editor auf dem Bildschirm (HUM-047).
///
/// Zwei Ebenen: der Editor für sich, mit einem Entwurf, den der Test baut, und
/// der ganze Bildschirm über einem Fake-Daemon, damit `E`, `Esc` und das
/// Senden den Weg gehen, den sie im Betrieb gehen.
library;

import 'dart:convert';

import 'package:flutter/gestures.dart' show PointerDeviceKind;
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
// `Override` lebt in riverpod 3 im Nebeneingang `misc.dart`.
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/features/editor/editor_host.dart';
import 'package:humanitl/features/editor/editor_screen.dart';
import 'package:humanitl/features/editor/model/draft.dart';
import 'package:humanitl/features/editor/providers/draft_provider.dart';
import 'package:humanitl/features/editor/providers/session_pseudonyms.dart';
import 'package:humanitl/l10n/l10n.dart';

import '../intercept/fixtures.dart';
import '../intercept/harness.dart';

/// Die englischen Texte, ohne einen Baum zu bauen.
final AppLocalizations english = lookupAppLocalizations(const Locale('en'));

const FlowId _flowId = FlowId('01930000-0000-7000-8000-0000000000ff');

/// Ein Fund im Rumpf über `[start, end)`.
Finding _bodyFinding(
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

/// Die Sitzung, in der die Entwürfe dieser Tests liegen, solange ein Test
/// keine andere nennt.
const SessionId _session = SessionId('01930000-0000-7000-8000-00000000aaaa');

DraftSource _source({
  required String body,
  List<Finding> findings = const <Finding>[],
  BodyKind kind = BodyKind.text,
  bool bytes = true,
  SessionId session = _session,
}) => DraftSource(
  session: session,
  request: HttpRequest(
    method: Method.post,
    scheme: Scheme.https,
    authority: const Authority(host: 'api.example.com', port: 443),
    pathAndQuery: '/v1/chat',
    // Gross geschrieben, wie ein Client sie schickt: Kopfzeilennamen sind
    // ohne Ruecksicht auf Gross- und Kleinschreibung zu lesen (RFC 9110 5.1),
    // und die Sperre muss das auch tun.
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
  findings: findings,
  bodyText: body,
  bodyKind: kind,
  bodyBytes: bytes ? Uint8List.fromList(utf8.encode(body)) : null,
);

/// Hängt den Editor allein in ein Fenster.
Future<ProviderContainer> pumpEditor(
  WidgetTester tester, {
  required DraftSource source,
  void Function(EditedRequest request, List<Replacement> replacements)? onSend,
  VoidCallback? onClose,
  bool canSend = true,
}) async {
  await tester.binding.setSurfaceSize(const Size(1200, 800));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  // Ein Fake mit leerem Skript: Der Entwurf hört auf den Ereignisstrom
  // (`DraftNotifier`, `Recorded`), und ohne Fake griffe der Strom nach dem
  // echten Daemon-Socket. Jeder Fluss darin ist angehalten: Das erste
  // Verbinden meldet `Lagged`, der Entwurf fragt dann nach seinem Fluss, und
  // ein Fluss, den der Fake nicht kennt, räumte ihn weg.
  final ProviderContainer container = ProviderContainer(
    overrides: <Override>[
      daemonClientProvider.overrideWithValue(_AlwaysHeld()),
    ],
  );
  addTearDown(container.dispose);
  addTearDown(() async {
    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pump();
  });
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: Directionality(
        textDirection: TextDirection.ltr,
        child: MediaQuery(
          data: const MediaQueryData(),
          child: Localizations(
            locale: const Locale('en'),
            delegates: AppLocalizations.localizationsDelegates,
            child: HTheme(
              tokens: HTokens.dark,
              child: Overlay(
                initialEntries: <OverlayEntry>[
                  OverlayEntry(
                    builder: (BuildContext context) => EditorScreen(
                      flowId: _flowId,
                      source: source,
                      canSend: canSend,
                      onClose: onClose ?? () {},
                      onSend:
                          onSend ??
                          (
                            EditedRequest request,
                            List<Replacement> replacements,
                          ) {},
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    ),
  );
  await tester.pump();
  await tester.pump();
  return container;
}

/// Oeffnet den Editor mit `E` und laesst ihn fertig laden.
///
/// Zwei Antworten sind unterwegs, bevor er etwas zeigt: das Detail des Flusses
/// und der zerlegte Rumpf. Beide sind Futures, also braucht der Aufbau mehr
/// als einen Rahmen.
Future<void> openEditor(WidgetTester tester) async {
  await tester.sendKeyEvent(LogicalKeyboardKey.keyE);
  for (int i = 0; i < 6; i++) {
    await tester.pump(const Duration(milliseconds: 16));
  }
}

/// Das Rumpf-Feld des Editors.
///
/// Über den Schlüssel des `KeyedSubtree` und nicht über den Typ: Kopf und
/// Kopfzeilen-Tabelle tragen eigene Felder, und `find.byType` träfe die auch.
EditableText bodyField(WidgetTester tester) => tester.widget<EditableText>(
  find.descendant(
    of: find.byKey(const Key('editor-draft-body')),
    matching: find.byType(EditableText),
  ),
);

/// Ein Fake, dessen `decide` nur mitschreibt.
///
/// Der echte Daemon meldet `Recorded` erst nach der Antwort des Ziels, lange
/// nachdem der Editor geschlossen ist. `FakeDaemonClient.decide` meldet es
/// sofort, noch bevor der Editor zugeht — ein Test darauf sähe den Entwurf
/// verschwinden, auch wenn ihn nur der Editor selbst weggeräumt hätte. Hier
/// kommt `Recorded` nur aus dem Skript, zu der Zeit, die der Test bestimmt.
/// Ein Fake, für den jeder Fluss angehalten ist.
class _AlwaysHeld extends FakeDaemonClient {
  _AlwaysHeld()
    : super(script: const <ScriptedEvent>[], clock: () => testStart);

  @override
  Future<FlowDetail> getFlow(FlowId id) async {
    final FlowDetail detail = held(1);
    return detail.copyWith(summary: detail.summary.copyWith(id: id));
  }
}

class _QuietDecide extends FakeDaemonClient {
  _QuietDecide(List<ScriptedEvent> script)
    : super(script: script, clock: () => testStart);

  /// Wie oft `GetFlow` gefragt wurde.
  int getFlowCalls = 0;

  @override
  Future<Rule?> decide(FlowId id, Decision decision, {Rule? remember}) async {
    decisions.add(RecordedDecision(id, decision, remember));
    return null;
  }

  @override
  Future<FlowDetail> getFlow(FlowId id) {
    getFlowCalls++;
    return super.getFlow(id);
  }
}

/// Hängt den [EditorHost] über [client] in ein Fenster.
///
/// Der Wirt und nicht der Bildschirm: Was hier geprüft wird — das Warten auf
/// die Antwort des Daemons und das Wegräumen des Entwurfs — steht in ihm.
Future<ProviderContainer> pumpHost(
  WidgetTester tester, {
  required FakeDaemonClient client,
  required FlowId flowId,
}) async {
  await tester.binding.setSurfaceSize(const Size(1200, 800));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  final ProviderContainer container = ProviderContainer(
    overrides: <Override>[daemonClientProvider.overrideWithValue(client)],
  );
  // Der Baum wird vor dem Container abgeraeumt: Erst damit kuendigt der
  // Ereignisstrom, und der Fake laesst dann keinen Zeitgeber zurueck
  // (`FakeDaemonClient.subscribe`, `onCancel`).
  addTearDown(container.dispose);
  addTearDown(() async {
    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pump();
  });
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: Directionality(
        textDirection: TextDirection.ltr,
        child: MediaQuery(
          data: const MediaQueryData(),
          child: Localizations(
            locale: const Locale('en'),
            delegates: AppLocalizations.localizationsDelegates,
            child: HTheme(
              tokens: HTokens.dark,
              child: Overlay(
                initialEntries: <OverlayEntry>[
                  OverlayEntry(
                    builder: (BuildContext context) =>
                        EditorHost(flowId: flowId, onClose: () {}),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    ),
  );
  // Der Fake laesst nach `subscribe` einen Zeitgeber von 400 ms laufen; ohne
  // ihn ablaufen zu lassen endet der Test mit einem haengenden Timer.
  await tester.pump(const Duration(milliseconds: 500));
  // Zwei Rahmen mehr: Der Editor legt den Entwurf erst nach seinem ersten
  // Rahmen an (`addPostFrameCallback`), und erst der naechste zeichnet ihn.
  await tester.pump();
  await tester.pump();
  return container;
}

/// Jede Markierung, die der Editor gerade malt.
List<HEditorDecoration> decorationsOf(WidgetTester tester) {
  final EditableText field = bodyField(tester);
  final TextEditingController controller = field.controller;
  return controller is HDecoratedTextController
      ? controller.decorations
      : const <HEditorDecoration>[];
}

void main() {
  group('the editor alone', () {
    testWidgets('open_with_findings_underlines_them', (
      WidgetTester tester,
    ) async {
      await pumpEditor(
        tester,
        source: _source(
          body: 'mail a@x.de key AKIAIOSFODNN7EXAMPLE',
          findings: <Finding>[
            _bodyFinding(5, 11),
            _bodyFinding(
              16,
              36,
              kind: 'api_key:aws',
              tier: FindingTier.checksum,
              hash: 2,
            ),
          ],
        ),
      );

      final List<HEditorDecoration> marks = decorationsOf(tester);

      expect(marks, hasLength(2));
      expect(marks.first.kind, HEditorDecorationKind.pii);
      expect(marks.last.kind, HEditorDecorationKind.secret);
      expect(find.byKey(const Key('editor-finding-chip-EMAIL')), findsOne);
      expect(find.byKey(const Key('editor-finding-chip-API_KEY')), findsOne);
    });

    testWidgets('replace_all_glows', (WidgetTester tester) async {
      await pumpEditor(
        tester,
        source: _source(
          body: 'a@x.de b@y.de c@z.de',
          findings: <Finding>[
            _bodyFinding(0, 6),
            _bodyFinding(7, 13, hash: 2),
            _bodyFinding(14, 20, hash: 3),
          ],
        ),
      );

      await tester.tap(find.byKey(const Key('editor-replace-all')));
      await tester.pump();

      final List<HEditorDecoration> marks = decorationsOf(tester);
      final List<HEditorDecoration> glowing = <HEditorDecoration>[
        for (final HEditorDecoration mark in marks)
          if (mark.kind == HEditorDecorationKind.replaced) mark,
      ];
      expect(glowing, hasLength(3));
      expect(
        bodyField(tester).controller.text,
        '<EMAIL_1> <EMAIL_2> <EMAIL_3>',
      );
    });

    testWidgets('the send button hands over the edited body as utf-8', (
      WidgetTester tester,
    ) async {
      EditedRequest? sent;
      await pumpEditor(
        tester,
        source: _source(
          body: 'Grüsse a@x.de',
          findings: <Finding>[_bodyFinding(8, 14)],
        ),
        onSend: (EditedRequest request, List<Replacement> _) => sent = request,
      );
      await tester.tap(find.byKey(const Key('editor-replace-all')));
      await tester.pump();

      await tester.tap(find.byKey(const Key('editor-send')));
      await tester.pump();

      expect(sent, isNotNull);
      expect(utf8.decode(sent!.body), 'Grüsse <EMAIL_1>');
      // `Grüsse ` ist 7 Zeichen und 8 Bytes.
      expect(sent!.body.length, utf8.encode('Grüsse <EMAIL_1>').length);
      expect(sent!.url, 'https://api.example.com/v1/chat');
      // Die drei Kopfzeilen, die der Daemon selbst setzt, reisen nicht mit.
      expect(
        <String>[for (final Header h in sent!.headers) h.name],
        <String>['Content-Type'],
      );
    });

    testWidgets('host and content-length are visibly locked', (
      WidgetTester tester,
    ) async {
      await pumpEditor(tester, source: _source(body: 'x'));

      await tester.tap(find.text(english.editorTabHeaders));
      await tester.pump();

      expect(find.byKey(const Key('editor-header-locked-host')), findsOne);
      expect(
        find.byKey(const Key('editor-header-locked-content-length')),
        findsOne,
      );
      // Eine gesperrte Zeile hat keinen Loeschknopf; die freie hat einen.
      expect(find.byKey(const Key('editor-header-remove-0')), findsOne);
      expect(find.byKey(const Key('editor-header-remove-1')), findsNothing);
      expect(find.byKey(const Key('editor-header-remove-2')), findsNothing);
    });

    testWidgets('an unlocked header can be typed into', (
      WidgetTester tester,
    ) async {
      final ProviderContainer container = await pumpEditor(
        tester,
        source: _source(body: 'x'),
      );
      await tester.tap(find.text(english.editorTabHeaders));
      await tester.pump();

      await tester.enterText(
        find.byKey(const Key('editor-header-value-0')),
        'text/plain',
      );
      await tester.pump();

      expect(
        container.read(draftProvider(_flowId))?.headers[0].value,
        'text/plain',
      );
      expect(container.read(draftProvider(_flowId))?.dirty, isTrue);
    });

    testWidgets('an added header can be named and then travels', (
      WidgetTester tester,
    ) async {
      EditedRequest? sent;
      final ProviderContainer container = await pumpEditor(
        tester,
        source: _source(body: 'x'),
        onSend: (EditedRequest request, List<Replacement> _) => sent = request,
      );
      await tester.tap(find.text(english.editorTabHeaders));
      await tester.pump();

      await tester.tap(find.byKey(const Key('editor-header-add')));
      await tester.pump();
      await tester.enterText(
        find.byKey(const Key('editor-header-name-3')),
        'X-Note',
      );
      await tester.pump();
      await tester.enterText(
        find.byKey(const Key('editor-header-value-3')),
        'hello',
      );
      await tester.pump();
      await tester.tap(find.byKey(const Key('editor-send')));
      await tester.pump();

      expect(
        container.read(draftProvider(_flowId))?.headers.last.name,
        'X-Note',
      );
      expect(<String>[
        for (final Header h in sent!.headers) h.name,
      ], contains('X-Note'));
    });

    testWidgets('a locked header has no field at all', (
      WidgetTester tester,
    ) async {
      await pumpEditor(tester, source: _source(body: 'x'));

      await tester.tap(find.text(english.editorTabHeaders));
      await tester.pump();

      // Zeile 0 ist `Content-Type` und frei, 1 und 2 sind gesperrt.
      expect(find.byKey(const Key('editor-header-name-0')), findsOne);
      expect(find.byKey(const Key('editor-header-name-1')), findsNothing);
      expect(find.byKey(const Key('editor-header-name-2')), findsNothing);
    });

    testWidgets('method and path are fields, the target is not', (
      WidgetTester tester,
    ) async {
      final ProviderContainer container = await pumpEditor(
        tester,
        source: _source(body: 'x'),
      );

      await tester.enterText(find.byKey(const Key('editor-method')), 'put');
      await tester.pump();
      await tester.enterText(
        find.byKey(const Key('editor-path')),
        '/v1/other?a=1',
      );
      await tester.pump();

      // Gross geschrieben abgelegt, wie `EDIT_002` es verlangt.
      expect(container.read(draftProvider(_flowId))?.method, 'PUT');
      expect(
        container.read(draftProvider(_flowId))?.pathAndQuery,
        '/v1/other?a=1',
      );
      // Die Query ist eine Sicht auf denselben Text, keine zweite Ablage.
      expect(container.read(draftProvider(_flowId))?.query, 'a=1');
      // Das Ziel bleibt ein Text mit Schloss: kein Feld, nichts zu tippen.
      expect(find.byKey(const Key('editor-authority')), findsOne);
      expect(
        tester.widget<Text>(find.byKey(const Key('editor-authority'))).data,
        'api.example.com',
      );
    });

    testWidgets('free typing moves the glow with its pseudonym', (
      WidgetTester tester,
    ) async {
      final ProviderContainer container = await pumpEditor(
        tester,
        source: _source(
          body: 'a@x.de rest',
          findings: <Finding>[_bodyFinding(0, 6)],
        ),
      );
      await tester.tap(find.byKey(const Key('editor-replace-all')));
      await tester.pump();
      final Draft glowing = container.read(draftProvider(_flowId))!;
      expect(glowing.replacements.single.start, 0);

      // Ein Zeichen davor: Der Glow muss mitwandern, sonst leuchtet Text, den
      // niemand ersetzt hat.
      container
          .read(draftProvider(_flowId).notifier)
          .setBody('x${glowing.body}');
      await tester.pump();

      final Draft after = container.read(draftProvider(_flowId))!;
      expect(after.replacements.single.start, 1);
      expect(
        after.body.substring(
          after.replacements.single.start,
          after.replacements.single.end,
        ),
        '<EMAIL_1>',
      );
    });

    testWidgets('a double click selects a word on the desktop', (
      WidgetTester tester,
    ) async {
      // `flutter test` laeuft als Android, wo ein langer Druck ein Wort
      // auswaehlt; auf dem Desktop, fuer den dieses Programm gebaut ist,
      // gibt es nur Doppelklick und Ziehen. Ein nacktes `EditableText` kann
      // beides nicht, und ohne Auswahl hat `Ctrl+R` nichts zu tun.
      await pumpEditor(tester, source: _source(body: 'hello world'));
      final Offset word =
          tester.getTopLeft(find.byKey(const Key('editor-draft-body'))) +
          const Offset(12, 6);

      await tester.tapAt(word, kind: PointerDeviceKind.mouse);
      await tester.pump(const Duration(milliseconds: 50));
      await tester.tapAt(word, kind: PointerDeviceKind.mouse);
      await tester.pump(const Duration(milliseconds: 300));

      final TextSelection selection = bodyField(tester).controller.selection;
      expect(selection.isCollapsed, isFalse);
      expect(
        bodyField(tester).controller.text
            .substring(selection.start, selection.end),
        'hello',
      );
    }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

    testWidgets('an IME composition keeps its own underline', (
      WidgetTester tester,
    ) async {
      // Waehrend fcitx oder ibus ein Zeichen zusammensetzen, gehoert der
      // Unterstrich darunter der Eingabemethode. Die Markierungen des Editors
      // duerfen ihn nicht verdecken.
      await pumpEditor(
        tester,
        source: _source(
          body: 'a@x.de rest',
          findings: <Finding>[_bodyFinding(0, 6)],
        ),
      );
      await tester.tap(find.byKey(const Key('editor-replace-all')));
      await tester.pump();
      final TextEditingController controller = bodyField(tester).controller;
      final String text = controller.text;
      controller.value = TextEditingValue(
        text: text,
        selection: TextSelection.collapsed(offset: text.length),
        composing: TextRange(start: text.length - 4, end: text.length),
      );

      final TextSpan span = controller.buildTextSpan(
        context: tester.element(find.byKey(const Key('editor-draft-body'))),
        withComposing: true,
      );

      final TextSpan composing = <TextSpan>[
        for (final InlineSpan child in span.children ?? const <InlineSpan>[])
          if (child is TextSpan && child.text == 'rest') child,
      ].single;
      expect(composing.style?.decoration, TextDecoration.underline);
    });

    testWidgets('a free row named Host stops the send and says why', (
      WidgetTester tester,
    ) async {
      final ProviderContainer container = await pumpEditor(
        tester,
        source: _source(body: 'x'),
      );
      container.read(draftProvider(_flowId).notifier).addHeader();
      container
          .read(draftProvider(_flowId).notifier)
          .setHeader(3, 'Host', 'evil.io');
      await tester.pump();

      expect(
        tester.widget<HButton>(find.byKey(const Key('editor-send'))).onPressed,
        isNull,
      );
      expect(
        tester.widget<Text>(find.byKey(const Key('editor-note'))).data,
        english.editorHeaderOwned('Host'),
      );
    });

    testWidgets('a carriage return in a value stops the send', (
      WidgetTester tester,
    ) async {
      final ProviderContainer container = await pumpEditor(
        tester,
        source: _source(body: 'x'),
      );
      container
          .read(draftProvider(_flowId).notifier)
          .setHeader(0, 'Content-Type', 'text/plain\rX-Evil: 1');
      await tester.pump();

      expect(
        tester.widget<HButton>(find.byKey(const Key('editor-send'))).onPressed,
        isNull,
      );
      expect(
        tester.widget<Text>(find.byKey(const Key('editor-note'))).data,
        english.editorHeaderInvalidValue('Content-Type'),
      );
    });

    testWidgets('binary_body_disables_editor', (WidgetTester tester) async {
      await pumpEditor(
        tester,
        source: _source(body: '', kind: BodyKind.binary, bytes: false),
      );

      expect(find.byKey(const Key('editor-body-not-editable')), findsOne);
      expect(find.byKey(const Key('editor-draft-body')), findsNothing);
    });

    testWidgets('ctrl_r_pseudonymises_the_selection', (
      WidgetTester tester,
    ) async {
      await pumpEditor(tester, source: _source(body: 'acme corp writes'));
      final EditableText field = bodyField(tester);
      field.controller.selection = const TextSelection(
        baseOffset: 0,
        extentOffset: 9,
      );
      await tester.pump();

      await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.keyR);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
      await tester.pump();
      await tester.enterText(
        find.byKey(const Key('editor-pseudonymize-label')),
        'client',
      );
      await tester.tap(find.byKey(const Key('editor-pseudonymize-confirm')));
      await tester.pump();

      expect(bodyField(tester).controller.text, '<CLIENT_1> writes');
    });

    testWidgets('an invalid method disables sending and says why', (
      WidgetTester tester,
    ) async {
      final ProviderContainer container = await pumpEditor(
        tester,
        source: _source(body: 'x'),
      );

      container.read(draftProvider(_flowId).notifier).setMethod('');
      await tester.pump();

      expect(
        tester.widget<HButton>(find.byKey(const Key('editor-send'))).onPressed,
        isNull,
      );
      expect(find.byKey(const Key('editor-note')), findsOne);
    });

    testWidgets('the mapping shows the masked original, never the value', (
      WidgetTester tester,
    ) async {
      await pumpEditor(
        tester,
        source: _source(
          body: 'anna@example.com writes',
          findings: <Finding>[_bodyFinding(0, 16)],
        ),
      );
      await tester.tap(find.byKey(const Key('editor-replace-all')));
      await tester.pump();

      await tester.tap(find.byKey(const Key('editor-mapping-toggle')));
      await tester.pump();

      expect(find.text('an************om'), findsOne);
      expect(find.text('anna@example.com'), findsNothing);
    });
  });

  group('the session', () {
    testWidgets('a second draft keeps counting where the first stopped', (
      WidgetTester tester,
    ) async {
      // Zwei gehaltene Anfragen, zwei Entwuerfe, ein Zaehler. Begaenne der
      // zweite wieder bei `<EMAIL_1>`, bezeichnete derselbe Name zuverlaessig
      // verschiedene Menschen (`backlog/sprint-4.md`, HUM-047).
      final ProviderContainer container = await pumpEditor(
        tester,
        source: _source(
          body: 'a@x.de',
          findings: <Finding>[_bodyFinding(0, 6)],
        ),
      );
      await tester.tap(find.byKey(const Key('editor-replace-all')));
      await tester.pump();
      expect(container.read(draftProvider(_flowId))?.body, '<EMAIL_1>');

      const FlowId second = FlowId('01930000-0000-7000-8000-0000000000fe');
      container
          .read(draftProvider(second).notifier)
          .load(
            _source(
              body: 'b@y.de',
              findings: <Finding>[_bodyFinding(0, 6, hash: 9)],
            ),
          );
      container.read(draftProvider(second).notifier).replaceAllOpen();

      expect(container.read(draftProvider(second))?.body, '<EMAIL_2>');
    });

    testWidgets('the same value gets the same name in both drafts', (
      WidgetTester tester,
    ) async {
      // Der zweite Entwurf nennt zuerst einen neuen Wert und dann den alten.
      // Nur ein Zaehler der Sitzung gibt dem neuen `<EMAIL_2>` und dem alten
      // wieder `<EMAIL_1>`; ein Entwurf, der von vorn zaehlte, vertauschte
      // beide Namen.
      final ProviderContainer container = await pumpEditor(
        tester,
        source: _source(
          body: 'a@x.de',
          findings: <Finding>[_bodyFinding(0, 6)],
        ),
      );
      await tester.tap(find.byKey(const Key('editor-replace-all')));
      await tester.pump();

      const FlowId second = FlowId('01930000-0000-7000-8000-0000000000fd');
      container
          .read(draftProvider(second).notifier)
          .load(
            _source(
              body: 'b@y.de and a@x.de',
              findings: <Finding>[
                _bodyFinding(0, 6, hash: 9),
                // Derselbe Wert wie oben, also derselbe Hash.
                _bodyFinding(11, 17),
              ],
            ),
          );
      container.read(draftProvider(second).notifier).replaceAllOpen();

      expect(
        container.read(draftProvider(second))?.body,
        '<EMAIL_2> and <EMAIL_1>',
      );
    });
  });

  group('the host', () {
    testWidgets('a refused send keeps the editor open and says why', (
      WidgetTester tester,
    ) async {
      // Der Fluss wurde entschieden, waehrend jemand tippte; `decide` wirft
      // dann `IPC_003`. Ohne `await` und `catch` liefe der Befund in
      // `Zone.handleUncaughtError`, der Editor schloesse sich, und der Mensch
      // hielte eine Anfrage fuer gesendet, die nie hinausging.
      // Ein ausdruecklich leeres Skript: Ohne eines spielt der Fake sein
      // Standard-Szenario ab und laesst dabei Zeitgeber laufen, die der Test
      // am Ende als haengend meldet.
      final FakeDaemonClient client = fakeDaemon(const <ScriptedEvent>[]);
      final FlowDetail detail = held(1);
      final FlowId id = detail.summary.id;
      // `decided` und nicht `recorded`: `recorded` raeumt den Entwurf weg --
      // das ist der Fall des Tests darunter --, und dann gaebe es keinen
      // Editor mehr, der einen Befund zeigen koennte.
      client.state.flows[id] = detail.summary.copyWith(
        state: FlowState.decided,
        decision: DecisionKind.allow,
      );
      client.state.details[id] = detail;
      final ProviderContainer container = await pumpHost(
        tester,
        client: client,
        flowId: id,
      );

      await tester.tap(find.byKey(const Key('editor-send')));
      await tester.pump();
      await tester.pump();

      expect(find.byKey(const Key('editor-send-error')), findsOne);
      expect(find.byKey(const Key('editor-send')), findsOne);
      expect(container.read(draftProvider(id)), isNotNull);
    });
  });

  group('the draft outlives its editor only until Recorded', () {
    testWidgets('Recorded after the editor closed on send clears the draft', (
      WidgetTester tester,
    ) async {
      final FlowDetail detail = held(1);
      final FlowId id = detail.summary.id;
      final _QuietDecide client = _QuietDecide(<ScriptedEvent>[
        ScriptedEvent(
          const Duration(seconds: 3),
          (FakeSessionState state, DateTime now) =>
              FlowEvent.recorded(at: now, flowId: id),
        ),
      ]);
      client.state.flows[id] = detail.summary;
      client.state.details[id] = detail;
      final ProviderContainer container = await pumpHost(
        tester,
        client: client,
        flowId: id,
      );
      container.read(draftProvider(id).notifier).setBody('secret removed');
      await tester.tap(find.byKey(const Key('editor-send')));
      await tester.pump();
      expect(client.decisions, hasLength(1));

      // Der Editor ist zu, wie nach jedem Senden. Erst danach kommt
      // `Recorded` — nach der Antwort des Ziels.
      await tester.pumpWidget(const SizedBox.shrink());
      expect(container.read(draftProvider(id)), isNotNull);
      await tester.pump(const Duration(seconds: 3));
      await tester.pump();

      expect(container.read(draftProvider(id)), isNull);
    });

    // `Recorded` geht in einer Lücke verloren; der Strom meldet sie mit
    // `Lagged`, und [during] stellt ein, was der Daemon danach über den Fluss
    // weiß.
    Future<Draft?> afterAGap(
      WidgetTester tester,
      void Function(FakeSessionState state, FlowDetail detail) during,
    ) async {
      final FlowDetail detail = held(1);
      final FlowId id = detail.summary.id;
      final _QuietDecide client = _QuietDecide(<ScriptedEvent>[
        ScriptedEvent(const Duration(seconds: 3), (
          FakeSessionState state,
          DateTime now,
        ) {
          during(state, detail);
          return FlowEvent.lagged(at: now, dropped: 1);
        }),
      ]);
      client.state.flows[id] = detail.summary;
      client.state.details[id] = detail;
      final ProviderContainer container = await pumpHost(
        tester,
        client: client,
        flowId: id,
      );
      container.read(draftProvider(id).notifier).setBody('secret removed');
      await tester.tap(find.byKey(const Key('editor-send')));
      await tester.pump();
      await tester.pumpWidget(const SizedBox.shrink());
      expect(container.read(draftProvider(id)), isNotNull);
      await tester.pump(const Duration(seconds: 3));
      await tester.pump();
      return container.read(draftProvider(id));
    }

    FlowDetail inState(FlowDetail detail, FlowState state) =>
        detail.copyWith(summary: detail.summary.copyWith(state: state));

    testWidgets('a Recorded lost in a gap still clears the draft', (
      WidgetTester tester,
    ) async {
      final Draft? draft = await afterAGap(tester, (
        FakeSessionState state,
        FlowDetail detail,
      ) {
        state.details[detail.summary.id] = inState(detail, FlowState.recorded);
      });
      expect(draft, isNull);
    });

    testWidgets(
      'a flow the restarted daemon no longer knows clears the draft',
      (WidgetTester tester) async {
        final Draft? draft = await afterAGap(tester, (
          FakeSessionState state,
          FlowDetail detail,
        ) {
          state.details.remove(detail.summary.id);
          state.flows.remove(detail.summary.id);
        });
        expect(draft, isNull);
      },
    );

    testWidgets('a gap before Recorded keeps the draft', (
      WidgetTester tester,
    ) async {
      // Unterwegs zum Ziel: `Recorded` kommt noch, und bis dahin bleibt der
      // Entwurf. Ohne diesen Fall räumte jede Lücke jeden Entwurf weg.
      final Draft? draft = await afterAGap(tester, (
        FakeSessionState state,
        FlowDetail detail,
      ) {
        state.details[detail.summary.id] = inState(detail, FlowState.forwarded);
      });
      expect(draft?.body, 'secret removed');
    });

    testWidgets('a cleared draft stops listening', (WidgetTester tester) async {
      // Nach dem Wegräumen hört der Entwurf nicht mehr hin: Eine Lücke danach
      // fragt nicht mehr nach seinem Fluss. Sonst hielte jeder je geöffnete
      // Entwurf einen Horcher am Strom.
      final FlowDetail detail = held(1);
      final FlowId id = detail.summary.id;
      final _QuietDecide client = _QuietDecide(<ScriptedEvent>[
        ScriptedEvent(
          const Duration(seconds: 3),
          (FakeSessionState state, DateTime now) =>
              FlowEvent.recorded(at: now, flowId: id),
        ),
        ScriptedEvent(
          const Duration(seconds: 4),
          (FakeSessionState state, DateTime now) =>
              FlowEvent.lagged(at: now, dropped: 1),
        ),
      ]);
      client.state.flows[id] = detail.summary;
      client.state.details[id] = detail;
      final ProviderContainer container = await pumpHost(
        tester,
        client: client,
        flowId: id,
      );
      await tester.tap(find.byKey(const Key('editor-send')));
      await tester.pump();
      await tester.pumpWidget(const SizedBox.shrink());
      await tester.pump(const Duration(seconds: 3));
      await tester.pump();
      expect(container.read(draftProvider(id)), isNull);
      final int before = client.getFlowCalls;

      await tester.pump(const Duration(seconds: 2));
      await tester.pump();

      expect(client.getFlowCalls, before);
    });

    testWidgets('Recorded after Esc clears the draft', (
      WidgetTester tester,
    ) async {
      final FlowDetail detail = held(1);
      final FlowId id = detail.summary.id;
      final FakeDaemonClient client = fakeDaemon(<ScriptedEvent>[
        ...holdScript(<FlowDetail>[detail]),
        ScriptedEvent(
          const Duration(seconds: 3),
          (FakeSessionState state, DateTime now) =>
              FlowEvent.recorded(at: now, flowId: id),
        ),
      ]);
      await pumpIntercept(tester, client: client);
      await playScript(tester);
      await openEditor(tester);
      final ProviderContainer container = containerOf(tester);
      container.read(draftProvider(id).notifier).setBody('secret removed');
      await tester.pump();

      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pump();
      expect(find.byKey(const Key('editor-send')), findsNothing);
      expect(container.read(draftProvider(id))?.body, 'secret removed');

      await tester.pump(const Duration(seconds: 4));
      await tester.pump();

      expect(container.read(draftProvider(id)), isNull);
    });
  });

  group('the session ledger', () {
    testWidgets('a Ctrl+R selection never enters the session ledger', (
      WidgetTester tester,
    ) async {
      // Der Schlüssel einer Auswahl ist der ausgewählte Text. Der Stand der
      // Sitzung lebt bis zum Ende der Anwendung; er darf ihn nie halten.
      final ProviderContainer container = await pumpEditor(
        tester,
        source: _source(body: 'hunter2 and hunter2'),
      );
      final DraftNotifier notifier = container.read(
        draftProvider(_flowId).notifier,
      );

      notifier.replaceSelection(DraftLocation.body, 0, 7, 'secret');
      final String once = container.read(draftProvider(_flowId))!.body;
      final int second = once.indexOf('hunter2');
      notifier.replaceSelection(
        DraftLocation.body,
        second,
        second + 7,
        'secret',
      );

      // Innerhalb der Anfrage bekommt derselbe Text denselben Namen …
      expect(
        container.read(draftProvider(_flowId))?.body,
        '<SECRET_1> and <SECRET_1>',
      );
      // … und die Sitzung kennt davon nur den Zähler.
      final PseudonymLedger ledger = container.read(sessionPseudonymsProvider);
      for (final MapEntry<String, String> entry in ledger.assigned.entries) {
        expect(entry.key, isNot(contains('hunter2')));
        expect(entry.value, isNot(contains('hunter2')));
      }
      expect(ledger.counters['SECRET'], 1);
    });

    testWidgets('a new session starts again at <EMAIL_1>', (
      WidgetTester tester,
    ) async {
      final ProviderContainer container = await pumpEditor(
        tester,
        source: _source(
          body: 'a@x.de',
          findings: <Finding>[_bodyFinding(0, 6)],
        ),
      );
      await tester.tap(find.byKey(const Key('editor-replace-all')));
      await tester.pump();

      const FlowId later = FlowId('01930000-0000-7000-8000-0000000000fc');
      const SessionId next = SessionId('01930000-0000-7000-8000-00000000bbbb');
      container
          .read(draftProvider(later).notifier)
          .load(
            _source(
              body: 'b@y.de and a@x.de',
              findings: <Finding>[
                _bodyFinding(0, 6, hash: 9),
                // Derselbe Wert wie in der alten Sitzung.
                _bodyFinding(11, 17),
              ],
              session: next,
            ),
          );
      container.read(draftProvider(later).notifier).replaceAllOpen();

      // Die neue Sitzung zählt von vorn und erbt keine Zuordnung: Der alte
      // Wert bekommt keinen alten Namen, sondern den nächsten freien.
      expect(
        container.read(draftProvider(later))?.body,
        '<EMAIL_1> and <EMAIL_2>',
      );
      expect(container.read(sessionPseudonymsProvider).session, next);
    });

    testWidgets('a draft from an older session leaves the ledger alone', (
      WidgetTester tester,
    ) async {
      // Die laufende Sitzung ist die spätere; ein Entwurf von vor einem
      // Neustart des Daemons ist noch offen.
      const SessionId current = SessionId(
        '01930000-0000-7000-8000-00000000bbbb',
      );
      final ProviderContainer container = await pumpEditor(
        tester,
        source: _source(
          body: 'a@x.de',
          findings: <Finding>[_bodyFinding(0, 6)],
          session: current,
        ),
      );
      await tester.tap(find.byKey(const Key('editor-replace-all')));
      await tester.pump();
      expect(container.read(draftProvider(_flowId))?.body, '<EMAIL_1>');

      const FlowId stale = FlowId('01930000-0000-7000-8000-0000000000fb');
      final DraftNotifier old = container.read(draftProvider(stale).notifier);
      old.load(
        _source(
          body: 'c@z.de and qqq',
          findings: <Finding>[_bodyFinding(0, 6, hash: 5)],
        ),
      );
      old.replaceAllOpen();
      final String once = container.read(draftProvider(stale))!.body;
      final int at = once.indexOf('qqq');
      old.replaceSelection(DraftLocation.body, at, at + 3, 'email');

      // Der alte Entwurf zählt in sich weiter und vergibt keinen seiner
      // Namen doppelt …
      expect(
        container.read(draftProvider(stale))?.body,
        '<EMAIL_1> and <EMAIL_2>',
      );
      // … aber der Stand gehört weiter der laufenden Sitzung, mit ihrem Zähler.
      final PseudonymLedger ledger = container.read(sessionPseudonymsProvider);
      expect(ledger.session, current);
      expect(ledger.counters['EMAIL'], 1);

      const FlowId fresh = FlowId('01930000-0000-7000-8000-0000000000fa');
      container
          .read(draftProvider(fresh).notifier)
          .load(
            _source(
              body: 'd@w.de',
              findings: <Finding>[_bodyFinding(0, 6, hash: 6)],
              session: current,
            ),
          );
      container.read(draftProvider(fresh).notifier).replaceAllOpen();

      // Der nächste Entwurf der laufenden Sitzung zählt weiter: Ein zweites
      // `<EMAIL_1>` hiesse in ihr zwei verschiedene Werte.
      expect(container.read(draftProvider(fresh))?.body, '<EMAIL_2>');
    });
  });

  group('the host after the deadline', () {
    testWidgets('a timed-out flow can no longer be sent', (
      WidgetTester tester,
    ) async {
      final FlowDetail detail = held(1);
      final FlowId id = detail.summary.id;
      final FakeDaemonClient client = fakeDaemon(<ScriptedEvent>[
        ScriptedEvent(
          const Duration(milliseconds: 10),
          (FakeSessionState state, DateTime now) =>
              FlowEvent.timedOut(at: now, flowId: id),
        ),
      ]);
      client.state.flows[id] = detail.summary;
      client.state.details[id] = detail;

      await pumpHost(tester, client: client, flowId: id);

      // Der Entwurf bleibt lesbar, aber der Knopf ist aus und sagt warum;
      // ein Senden nach der Frist wuerde der Daemon mit `IPC_003` ablehnen.
      expect(
        tester.widget<HButton>(find.byKey(const Key('editor-send'))).onPressed,
        isNull,
      );
      expect(
        tester.widget<Text>(find.byKey(const Key('editor-note'))).data,
        english.editorTimedOut,
      );
    });
  });

  group('the editor in the queue', () {
    testWidgets('open_with_E_shows_editor', (WidgetTester tester) async {
      final FakeDaemonClient client = fakeDaemon(
        holdScript(<FlowDetail>[held(1)]),
      );
      await pumpIntercept(tester, client: client);
      await playScript(tester);

      await openEditor(tester);

      expect(find.byKey(const Key('editor-send')), findsOne);
      expect(find.byKey(const Key('intercept-allow')), findsNothing);
    });

    testWidgets('esc_keeps_draft', (WidgetTester tester) async {
      final FakeDaemonClient client = fakeDaemon(
        holdScript(<FlowDetail>[held(1)]),
      );
      await pumpIntercept(tester, client: client);
      await playScript(tester);
      await openEditor(tester);
      final ProviderContainer container = containerOf(tester);
      final FlowId id = client.state.flows.keys.first;
      container.read(draftProvider(id).notifier).setBody('changed by hand');
      await tester.pump();

      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pump();
      expect(find.byKey(const Key('editor-send')), findsNothing);
      await openEditor(tester);

      expect(container.read(draftProvider(id))?.body, 'changed by hand');
      expect(container.read(draftProvider(id))?.dirty, isTrue);
    });

    testWidgets('a recorded flow throws the draft away', (
      WidgetTester tester,
    ) async {
      final FakeDaemonClient client = fakeDaemon(
        holdScript(<FlowDetail>[held(1)]),
      );
      await pumpIntercept(tester, client: client);
      await playScript(tester);
      await openEditor(tester);
      final ProviderContainer container = containerOf(tester);
      final FlowId id = client.state.flows.keys.first;
      container.read(draftProvider(id).notifier).setBody('secret removed');
      await tester.pump();
      expect(container.read(draftProvider(id)), isNotNull);

      await tester.tap(find.byKey(const Key('editor-send')));
      for (int i = 0; i < 8; i++) {
        await tester.pump(const Duration(milliseconds: 16));
      }

      // Der Entwurf traegt den Originalwert jeder Ersetzung im Klartext; er
      // darf nicht ueber das Ende des Flusses hinaus liegen bleiben.
      expect(container.read(draftProvider(id)), isNull);
    });

    testWidgets('send_edited_calls_decide_with_ALLOW_EDITED', (
      WidgetTester tester,
    ) async {
      final FakeDaemonClient client = fakeDaemon(
        holdScript(<FlowDetail>[held(1)]),
      );
      await pumpIntercept(tester, client: client);
      await playScript(tester);
      await openEditor(tester);
      final ProviderContainer container = containerOf(tester);
      final FlowId id = client.state.flows.keys.first;
      container.read(draftProvider(id).notifier).setBody('cleaned');
      await tester.pump();

      await tester.tap(find.byKey(const Key('editor-send')));
      await tester.pump();
      await tester.pump();

      expect(client.decisions, hasLength(1));
      final Decision taken = client.decisions.single.decision;
      expect(taken, isA<DecisionAllowEdited>());
      expect(taken.kind, DecisionKind.allowEdited);
      expect(
        utf8.decode((taken as DecisionAllowEdited).request.body),
        'cleaned',
      );
    });
  });
}
