// Der Unterschied zwischen „kommt noch" und „keiner da" (HUM-154). Beide
// Zustände zeichnet dieselbe Ansicht, und nur das Flag `pending` trennt sie:
// ein fehlender Rumpf ist Warten, solange die Seite noch einläuft, und sonst
// eine Aussage. Wer das Flag an einer Aufrufstelle vergisst, macht aus dem
// Warten eine Behauptung über einen Rumpf, den niemand gesehen hat
// (`docs/UX.md` 2.11).

import 'dart:async';

import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
// `Override` lebt in riverpod 3 im Nebeneingang `misc.dart`.
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/body/body_view.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/providers/now.dart';
import 'package:humanitl/features/intercept/widgets/request_card.dart';
import 'package:humanitl/l10n/l10n.dart';

import '../fixtures.dart';
import 'harness.dart';

/// Ein Fake, dessen Detail nie ankommt.
class _NeverAnswers extends TestDaemonClient {
  @override
  Future<FlowDetail> getFlow(FlowId id) => Completer<FlowDetail>().future;
}

/// Hängt [child] in ein Fenster mit Theme, Sprache und [client].
///
/// [retry] schaltet die Wiederholung eines gescheiterten Providers ab.
/// Riverpod versucht es sonst von selbst wieder, und ein Detail, das scheitert,
/// steht dann abwechselnd auf `loading` und auf `error`; ein Test, der die
/// beiden unterscheiden will, liest sonst einen Zufall.
Future<void> _pump(
  WidgetTester tester,
  TestDaemonClient client,
  Widget child, {
  bool retry = true,
}) async {
  await tester.binding.setSurfaceSize(const Size(700, 600));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    ProviderScope(
      retry: retry ? null : (int count, Object error) => null,
      overrides: <Override>[
        daemonClientProvider.overrideWithValue(client),
        nowProvider.overrideWith(() => FixedNow(testStart)),
      ],
      child: WidgetsApp(
        color: HTokens.dark.colors.bg0,
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        builder: (BuildContext context, Widget? _) => HTheme(
          tokens: HTokens.dark,
          child: Overlay(
            initialEntries: <OverlayEntry>[
              OverlayEntry(builder: (BuildContext context) => child),
            ],
          ),
        ),
      ),
    ),
  );
  for (int i = 0; i < 3; i++) {
    await tester.pump(const Duration(milliseconds: 200));
  }
}

/// Die Karte zu einem angehaltenen Flow unter [client].
Future<void> _pumpCard(
  WidgetTester tester,
  TestDaemonClient client, {
  bool retry = true,
}) => _pump(
  tester,
  client,
  RequestCard(
    flow: heldFlow(n: 1, deadline: testStart.add(const Duration(minutes: 5))),
  ),
  retry: retry,
);

void main() {
  testWidgets('the queue card waits while no detail has arrived', (
    WidgetTester tester,
  ) async {
    await _pumpCard(tester, _NeverAnswers());
    expect(find.byKey(const Key('body-pending')), findsOneWidget);
    expect(find.byKey(const Key('body-empty')), findsNothing);
  });

  testWidgets('a detail that failed does not claim an empty body', (
    WidgetTester tester,
  ) async {
    // `TestDaemonClient.getFlow` wirft für einen unbekannten Flow, also endet
    // das Detail als Fehler ohne Wert. `isLoading` wäre dann falsch und die
    // Karte behauptete „No body." über eine Anfrage, von der nie etwas
    // ankam; gefragt wird deshalb nach dem Wert.
    await _pumpCard(tester, TestDaemonClient(), retry: false);
    expect(find.byKey(const Key('body-pending')), findsOneWidget);
    expect(find.byKey(const Key('body-empty')), findsNothing);
  });

  testWidgets('a body view that is not waiting names the missing body', (
    WidgetTester tester,
  ) async {
    // Kein Rumpf und ein leerer Rumpf sind dieselbe Aussage, und sie bekommen
    // denselben Satz in `fg1` (`docs/UX.md` 6, Sekundärtext).
    await _pump(
      tester,
      TestDaemonClient(),
      const BodyView(
        flowId: FlowId('018f0001-0000-7000-8000-00000000b001'),
        body: null,
        headers: <Header>[],
        findings: <Finding>[],
        pending: false,
      ),
    );
    expect(find.byKey(const Key('body-pending')), findsNothing);
    // Der Titel nennt keine Groesse und keine Art: ohne Verweis hat sie
    // niemand gemessen (`backlog/CONVENTIONS.md` 4.13).
    expect(find.text(english.interceptSectionBodyPending), findsOneWidget);
    expect(find.textContaining('0 B'), findsNothing);
    final Text sentence = tester.widget<Text>(
      find.byKey(const Key('body-empty')),
    );
    expect(sentence.data, english.interceptBodyEmpty);
    expect(sentence.style?.color, HTokens.dark.colors.fg1);
  });

  testWidgets('a body view that is waiting draws the skeleton', (
    WidgetTester tester,
  ) async {
    await _pump(
      tester,
      TestDaemonClient(),
      const BodyView(
        flowId: FlowId('018f0001-0000-7000-8000-00000000b002'),
        body: null,
        headers: <Header>[],
        findings: <Finding>[],
        pending: true,
      ),
    );
    expect(find.byKey(const Key('body-pending')), findsOneWidget);
    expect(find.byKey(const Key('body-empty')), findsNothing);
  });
}
