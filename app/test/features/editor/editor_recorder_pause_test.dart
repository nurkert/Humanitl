// Das Senden aus dem Editor hält während einer Störung der Aufzeichnung an
// derselben Pause wie die Warteschlange, einmal je Sitzung (HUM-269).
//
// Die Bestätigung ist eine für beide: Wer sie in der Warteschlange gab,
// sendet aus dem Editor ohne Halt, und wer sie im Editor gab, wird in der
// Warteschlange nicht mehr gefragt ([RecorderPause.owed]).

import 'dart:async';
import 'dart:convert';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
// `Override` steht in Riverpod 3 nur noch in `misc.dart`.
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/recorder/recorder_health.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/editor/editor_host.dart';
import 'package:humanitl/features/editor/providers/draft_provider.dart';
import 'package:humanitl/l10n/l10n.dart';

import '../../harness/ui_state.dart';
import '../intercept/fixtures.dart';
import '../intercept/harness.dart';

/// Eine Aufzeichnung, die von Anfang an gestört ist.
class _Disrupted extends RecorderHealth {
  @override
  RecorderOutage? build() => RecorderOutage(
    since: testStart,
    diagnostic: const Diagnostic(
      code: 'RECORDER_003',
      severity: Severity.error,
      title: 'Aufzeichnung konnte nicht schreiben',
      why: 'database or disk is full',
    ),
  );
}

/// Hängt den Wirt über [client] ein; [shown] schaltet ihn ab, ohne den
/// Container zu verwerfen.
Future<ProviderContainer> _pumpHost(
  WidgetTester tester, {
  required FakeDaemonClient client,
  required FlowId flowId,
  required ValueNotifier<bool> shown,
}) async {
  await tester.binding.setSurfaceSize(const Size(1200, 800));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  final ProviderContainer container = ProviderContainer(
    overrides: <Override>[
      uiStateOverride(),
      daemonClientProvider.overrideWithValue(client),
      recorderHealthProvider.overrideWith(_Disrupted.new),
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
                    builder: (BuildContext context) =>
                        ValueListenableBuilder<bool>(
                          valueListenable: shown,
                          builder: (BuildContext context, bool on, Widget? _) =>
                              on
                              ? EditorHost(flowId: flowId, onClose: () {})
                              : const SizedBox.shrink(),
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
  // Der Fake lässt nach `subscribe` einen Zeitgeber von 400 ms laufen, und der
  // Editor legt den Entwurf erst nach seinem ersten Rahmen an.
  await tester.pump(const Duration(milliseconds: 500));
  await tester.pump();
  await tester.pump();
  return container;
}

/// Ein Fluss, der gehalten ist, und ein Fake, der ihn kennt.
({FakeDaemonClient client, FlowId id}) _heldFlow() {
  final FakeDaemonClient client = fakeDaemon(const <ScriptedEvent>[]);
  final FlowDetail detail = held(1);
  client.state.flows[detail.summary.id] = detail.summary;
  client.state.details[detail.summary.id] = detail;
  return (client: client, id: detail.summary.id);
}

/// Ein Fake, dessen `Decide` erst antwortet, wenn der Test es sagt, und dann
/// ablehnt: Der Daemon lässt die fortgesetzte Freigabe warten und weist sie
/// zurück (HUM-269, Review).
class _SlowRefusal extends FakeDaemonClient {
  _SlowRefusal()
    : super(script: const <ScriptedEvent>[], clock: () => testStart);

  final Completer<void> answer = Completer<void>();

  @override
  Future<Rule?> decide(FlowId id, Decision decision, {Rule? remember}) async {
    decisions.add(RecordedDecision(id, decision, remember));
    await answer.future;
    throw const DaemonException(
      Diagnostic(
        code: 'IPC_003',
        severity: Severity.error,
        title: 'Flow already decided',
        why: 'the flow timed out while the decision was on its way',
      ),
    );
  }
}

/// Ein Fake, dessen `Decide` offen bleibt und dabei [onDecide] ruft.
///
/// Was [onDecide] plant, stammt aus der fortgesetzten Freigabe und läuft,
/// während sie noch auf den Daemon wartet (HUM-269, Review Runde 2).
class _PendingDecide extends FakeDaemonClient {
  _PendingDecide()
    : super(script: const <ScriptedEvent>[], clock: () => testStart);

  final Completer<void> answer = Completer<void>();
  void Function()? onDecide;

  @override
  Future<Rule?> decide(FlowId id, Decision decision, {Rule? remember}) async {
    decisions.add(RecordedDecision(id, decision, remember));
    onDecide?.call();
    await answer.future;
    return null;
  }
}

const Key _pause = ValueKey<String>('editor-recorder-pause');
const Key _sendUnrecorded = Key('intercept-recorder-pause-send');

void main() {
  testWidgets('the first edited send stops at the pause, once per session', (
    WidgetTester tester,
  ) async {
    final (:FakeDaemonClient client, :FlowId id) = _heldFlow();
    final ValueNotifier<bool> shown = ValueNotifier<bool>(true);
    addTearDown(shown.dispose);
    final ProviderContainer container = await _pumpHost(
      tester,
      client: client,
      flowId: id,
      shown: shown,
    );
    container.read(draftProvider(id).notifier).setBody('secret removed');
    await tester.pump();

    await tester.tap(find.byKey(const Key('editor-send')));
    await tester.pump();
    // Die Leiste geht in `HMotion.sweep`; 300 ms liegen noch vor
    // `HMotion.rearm`.
    await tester.pump(const Duration(milliseconds: 300));
    await tester.pump();

    // Nichts ist hinausgegangen; an der Stelle der Leiste steht die Pause.
    expect(client.decisions, isEmpty);
    expect(find.byKey(_pause), findsOne);
    expect(find.byKey(const Key('editor-send')), findsNothing);

    // Vor `HMotion.rearm` nimmt der Knopf nicht an: Ein zweiter Klick auf
    // dieselbe Stelle überspränge die Pause sonst ungelesen.
    await tester.tap(find.byKey(_sendUnrecorded));
    await tester.pump();
    expect(client.decisions, isEmpty);

    await tester.pump(HMotion.rearm);
    await tester.tap(find.byKey(_sendUnrecorded));
    await tester.pump();
    await tester.pump();

    // Genau die angehaltene Freigabe ging hinaus, mit dem bearbeiteten Rumpf.
    expect(client.decisions, hasLength(1));
    final Decision sent = client.decisions.single.decision;
    expect(sent, isA<DecisionAllowEdited>());
    expect(
      utf8.decode((sent as DecisionAllowEdited).request.body),
      'secret removed',
    );
    // Die Bestätigung gilt für die Sitzung, auch in der Warteschlange.
    expect(container.read(recorderPauseProvider.notifier).owed, isFalse);
    expect(container.read(recorderPauseProvider).isOpen, isFalse);
  });

  testWidgets('a session confirmed in the queue sends without stopping', (
    WidgetTester tester,
  ) async {
    final (:FakeDaemonClient client, :FlowId id) = _heldFlow();
    final ValueNotifier<bool> shown = ValueNotifier<bool>(true);
    addTearDown(shown.dispose);
    final ProviderContainer container = await _pumpHost(
      tester,
      client: client,
      flowId: id,
      shown: shown,
    );
    // Die Warteschlange hielt schon an, und dort ging eine Freigabe hinaus.
    final RecorderPause pause = container.read(recorderPauseProvider.notifier)
      ..open((RecorderBypass bypass) async {
        container.read(recorderPauseProvider.notifier).noteAllowSent(bypass);
      });
    await tester.pump(HMotion.rearm);
    await pause.confirm();
    expect(pause.owed, isFalse);

    await tester.tap(find.byKey(const Key('editor-send')));
    await tester.pump();
    await tester.pump();

    expect(find.byKey(_pause), findsNothing);
    expect(client.decisions, hasLength(1));
  });

  testWidgets('Esc closes the pause, sends nothing and keeps the editor', (
    WidgetTester tester,
  ) async {
    final (:FakeDaemonClient client, :FlowId id) = _heldFlow();
    final ValueNotifier<bool> shown = ValueNotifier<bool>(true);
    addTearDown(shown.dispose);
    final ProviderContainer container = await _pumpHost(
      tester,
      client: client,
      flowId: id,
      shown: shown,
    );
    await tester.tap(find.byKey(const Key('editor-send')));
    await tester.pump();
    expect(find.byKey(_pause), findsOne);

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 300));
    await tester.pump();

    expect(find.byKey(_pause), findsNothing);
    expect(find.byKey(const Key('editor-send')), findsOne);
    expect(client.decisions, isEmpty);
    // Zurück bestätigt nichts: Das nächste Senden hält wieder an.
    expect(container.read(recorderPauseProvider.notifier).owed, isTrue);
  });

  testWidgets('a closing editor takes its pause along', (
    WidgetTester tester,
  ) async {
    final (:FakeDaemonClient client, :FlowId id) = _heldFlow();
    final ValueNotifier<bool> shown = ValueNotifier<bool>(true);
    addTearDown(shown.dispose);
    final ProviderContainer container = await _pumpHost(
      tester,
      client: client,
      flowId: id,
      shown: shown,
    );
    await tester.tap(find.byKey(const Key('editor-send')));
    await tester.pump();
    expect(container.read(recorderPauseProvider).isOpen, isTrue);

    // Stünde die Pause weiter, zeigte die Leiste der Warteschlange sie, und
    // ihr Knopf setzte eine Freigabe fort, die niemand mehr sieht.
    shown.value = false;
    await tester.pump();
    await tester.pump();

    expect(container.read(recorderPauseProvider).isOpen, isFalse);
    expect(client.decisions, isEmpty);
  });

  testWidgets('a second send waits while the resumed one is refused', (
    WidgetTester tester,
  ) async {
    final _SlowRefusal client = _SlowRefusal();
    final FlowDetail detail = held(1);
    final FlowId id = detail.summary.id;
    client.state.flows[id] = detail.summary;
    client.state.details[id] = detail;
    final ValueNotifier<bool> shown = ValueNotifier<bool>(true);
    addTearDown(shown.dispose);
    final ProviderContainer container = await _pumpHost(
      tester,
      client: client,
      flowId: id,
      shown: shown,
    );
    final RecorderPause pause = container.read(recorderPauseProvider.notifier);
    await tester.tap(find.byKey(const Key('editor-send')));
    await tester.pump();
    await tester.pump(HMotion.rearm);
    await tester.tap(find.byKey(_sendUnrecorded));
    await tester.pump();
    // Die fortgesetzte Freigabe ist beim Daemon und wartet auf seine Antwort.
    expect(client.decisions, hasLength(1));

    // Eine zweite Freigabe in dieser Zeit, aus der Benachrichtigung oder der
    // Warteschlange, fragt außerhalb der fortgesetzten: Sie hält an.
    expect(pause.owed, isTrue);

    client.answer.complete();
    await tester.pump();
    await tester.pump();

    // Der Daemon lehnte ab: Nichts ist bestätigt, und der Editor sagt warum.
    expect(pause.owed, isTrue);
    expect(find.byKey(const Key('editor-send-error')), findsOne);
  });

  testWidgets('send unrecorded takes the draft as it stands then', (
    WidgetTester tester,
  ) async {
    final (:FakeDaemonClient client, :FlowId id) = _heldFlow();
    final ValueNotifier<bool> shown = ValueNotifier<bool>(true);
    addTearDown(shown.dispose);
    final ProviderContainer container = await _pumpHost(
      tester,
      client: client,
      flowId: id,
      shown: shown,
    );
    final RecorderPause pause = container.read(recorderPauseProvider.notifier);
    container.read(draftProvider(id).notifier).setBody('token sk-secret');
    await tester.pump();
    await tester.tap(find.byKey(const Key('editor-send')));
    await tester.pump();

    // Während die Pause steht, löscht jemand das Geheimnis noch.
    container.read(draftProvider(id).notifier).setBody('token removed');
    // Die Auswahl der Warteschlange zieht weiter; die Pause gehört dem Editor
    // und nicht der Auswahl.
    pause.selectionMoved();
    await tester.pump(HMotion.rearm);
    expect(find.byKey(_pause), findsOne);

    await tester.tap(find.byKey(_sendUnrecorded));
    await tester.pump();
    await tester.pump();

    expect(client.decisions, hasLength(1));
    final Decision sent = client.decisions.single.decision;
    expect(
      utf8.decode((sent as DecisionAllowEdited).request.body),
      'token removed',
    );
  });

  testWidgets('a finding reopened behind the pause stops the send there', (
    WidgetTester tester,
  ) async {
    // Eine Anfrage mit einer E-Mail-Adresse in der Query; der Entwurf
    // ersetzt sie, bevor jemand sendet.
    final FakeDaemonClient client = fakeDaemon(const <ScriptedEvent>[]);
    final FlowDetail base = held(1);
    final FlowId id = base.summary.id;
    final FlowDetail detail = base.copyWith(
      request: base.request?.copyWith(pathAndQuery: '/v1/chat?to=a@x.de'),
      findings: <Finding>[
        Finding(
          kind: 'email',
          location: FindingLocation.query,
          spanStart: 3,
          spanEnd: 9,
          tier: FindingTier.regex,
          valueHash: List<int>.filled(32, 1),
        ),
      ],
    );
    client.state.flows[id] = detail.summary;
    client.state.details[id] = detail;
    final ValueNotifier<bool> shown = ValueNotifier<bool>(true);
    addTearDown(shown.dispose);
    final ProviderContainer container = await _pumpHost(
      tester,
      client: client,
      flowId: id,
      shown: shown,
    );
    final RecorderPause pause = container.read(recorderPauseProvider.notifier);
    container.read(draftProvider(id).notifier).replaceAllOpen();
    await tester.pump();
    await tester.tap(find.byKey(const Key('editor-send')));
    await tester.pump();
    expect(container.read(recorderPauseProvider).isOpen, isTrue);

    // Während die Pause steht, tippt jemand die Adresse zurück: Der Fund ist
    // wieder offen (Regel 4 in `anchoring.dart`).
    container
        .read(draftProvider(id).notifier)
        .setPathAndQuery('/v1/chat?to=a@x.de');
    await tester.pump(HMotion.rearm);
    await tester.tap(find.byKey(_sendUnrecorded));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 300));

    // Nichts ging hinaus. An der Stelle der Leiste steht die Pause mit
    // offenen Funden, und die Aufzeichnung ist weiter unbestätigt.
    expect(client.decisions, isEmpty);
    expect(find.byKey(const Key('intercept-findings-pause')), findsOne);
    expect(find.byKey(_pause), findsNothing);
    expect(pause.owed, isTrue);
  });

  testWidgets('a release scheduled inside the resumed one stops at the pause', (
    WidgetTester tester,
  ) async {
    final _PendingDecide client = _PendingDecide();
    final FlowDetail detail = held(1);
    final FlowId id = detail.summary.id;
    client.state.flows[id] = detail.summary;
    client.state.details[id] = detail;
    final ValueNotifier<bool> shown = ValueNotifier<bool>(true);
    addTearDown(shown.dispose);
    final ProviderContainer container = await _pumpHost(
      tester,
      client: client,
      flowId: id,
      shown: shown,
    );
    final RecorderPause pause = container.read(recorderPauseProvider.notifier);
    // Aus der fortgesetzten Freigabe heraus geplant, während ihr `Decide`
    // offen ist: eine zweite Freigabe, etwa aus einem Hörer oder Zeitgeber.
    // Sie fragt die Pause ohne Freibrief, wie jede andere.
    bool? secondOwed;
    client.onDecide = () => scheduleMicrotask(() => secondOwed = pause.owed);

    await tester.tap(find.byKey(const Key('editor-send')));
    await tester.pump();
    await tester.pump(HMotion.rearm);
    await tester.tap(find.byKey(_sendUnrecorded));
    await tester.pump();

    expect(client.decisions, hasLength(1));
    expect(secondOwed, isTrue);

    client.answer.complete();
    await tester.pump();
    await tester.pump();
    // Erst die angenommene fortgesetzte Freigabe bestätigt die Sitzung.
    expect(pause.owed, isFalse);
  });
}
