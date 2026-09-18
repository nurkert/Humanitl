// Der Chip „Edited“ in der Historie (HUM-047): in der Zeile, in der Spalte
// „Edited“, und im Kopf des Details. Derselbe Chip wie im Queue-Abgang
// (`queue_edited_chip_test.dart`), damit eine bearbeitete Anfrage an allen
// drei Stellen dasselbe Wort trägt.

import 'package:flutter/rendering.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/edited_badge.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/history/history_detail.dart';
import 'package:humanitl/features/history/history_metrics.dart';
import 'package:humanitl/features/history/history_table.dart';
import 'package:humanitl/features/history/providers/history_detail.dart';
import 'package:humanitl/features/history/providers/history_page.dart';
import 'package:humanitl/l10n/l10n.dart';

import 'harness.dart';

Finder _rowOf(Flow flow) => find.byWidgetPredicate(
  (Widget widget) => widget is HistoryRow && widget.flow.id == flow.id,
);

Finder _chipIn(Finder scope) =>
    find.descendant(of: scope, matching: find.byType(EditedBadge));

Future<void> _select(
  WidgetTester tester,
  ProviderContainer container,
  Flow flow,
) async {
  container.read(historySelectionProvider.notifier).select(flow.id);
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 600));
}

void main() {
  testWidgets('an edited row carries the Edited chip, a plain one none', (
    WidgetTester tester,
  ) async {
    final ProviderContainer container = await pumpHistory(
      tester,
      client: FakeDaemonClient.history(count: 24),
    );
    final List<Flow> rows = container.read(historyPageProvider).rows;
    final Flow edited = rows.firstWhere((Flow flow) => flow.edited);
    final Flow plain = rows.firstWhere(
      (Flow flow) => !flow.edited && flow.decision == DecisionKind.allow,
    );

    expect(_rowOf(edited), findsOneWidget);
    expect(_chipIn(_rowOf(edited)), findsOneWidget);
    expect(
      find.descendant(
        of: _chipIn(_rowOf(edited)),
        matching: find.text('Edited'),
      ),
      findsOneWidget,
      reason: 'the row says the word, not a dot one has to know',
    );
    expect(_rowOf(plain), findsOneWidget);
    expect(_chipIn(_rowOf(plain)), findsNothing);

    // Jede sichtbare bearbeitete Zeile trägt genau einen, keine andere einen.
    final int visibleEdited = rows
        .where((Flow flow) => flow.edited && _rowOf(flow).evaluate().isNotEmpty)
        .length;
    expect(visibleEdited, greaterThan(0));
    expect(
      find.descendant(
        of: find.byType(HistoryRow),
        matching: find.byType(EditedBadge),
      ),
      findsNWidgets(visibleEdited),
    );
  });

  testWidgets('the chip fits its column without being cut', (
    WidgetTester tester,
  ) async {
    final ProviderContainer container = await pumpHistory(
      tester,
      client: FakeDaemonClient.history(count: 24),
    );
    final Flow edited = container
        .read(historyPageProvider)
        .rows
        .firstWhere((Flow flow) => flow.edited);
    final Finder chip = _chipIn(_rowOf(edited));

    // Das Wort steht ganz da: Der Absatz hat so viel Breite bekommen, wie er
    // braucht, und ist nicht in eine zweite Zeile umgebrochen, die der Chip
    // abschneidet. Gemessen in der Testschrift, deren Zeichen so breit wie
    // hoch sind; eine echte Schrift ist schmaler, also gilt es dort erst
    // recht. Das deutsche Wort ist in dieser Schrift breiter als jede
    // vertretbare Spalte und wird deshalb hier nicht gemessen
    // (`historyEditedColumnWidth`).
    final RenderParagraph paragraph = tester.renderObject<RenderParagraph>(
      find.descendant(of: chip, matching: find.text('Edited')),
    );
    expect(
      paragraph.size.width,
      greaterThanOrEqualTo(paragraph.getMaxIntrinsicWidth(double.infinity)),
    );
    expect(paragraph.didExceedMaxLines, isFalse);
    expect(
      tester.getSize(chip).width,
      lessThanOrEqualTo(historyEditedColumnWidth - historyCellGap),
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets('the detail head of an edited flow carries the Edited chip', (
    WidgetTester tester,
  ) async {
    final ProviderContainer container = await pumpHistory(
      tester,
      client: FakeDaemonClient.history(count: 24),
    );
    final List<Flow> rows = container.read(historyPageProvider).rows;
    final Flow edited = rows.firstWhere((Flow flow) => flow.edited);
    final Flow plain = rows.firstWhere(
      (Flow flow) => !flow.edited && flow.decision == DecisionKind.allow,
    );
    const Key head = Key('history-detail-edited');

    await _select(tester, container, edited);
    expect(find.byKey(head), findsOneWidget);
    expect(
      find.descendant(of: find.byKey(head), matching: find.text('Edited')),
      findsOneWidget,
    );
    final HBadge badge = tester.widget<HBadge>(
      find.descendant(of: find.byKey(head), matching: find.byType(HBadge)),
    );
    expect(
      badge.color,
      HTheme.of(tester.element(find.byKey(head))).state.allowedEdited,
      reason: 'the same chip as the row and the queue, in the state colour',
    );

    await _select(tester, container, plain);
    expect(find.byKey(head), findsNothing);
  });

  // Der Chip im Kopf des Details ist für den Screenreader stumm: Der Zustand
  // daneben und die Tatsache „Decision“ sagen „Allowed, edited“ schon.
  group('the detail chip is silent for a screen reader', () {
    const Key head = Key('history-detail-edited');
    Finder speaking(String label) => find.descendant(
      of: find.byKey(head),
      matching: find.bySemanticsLabel(label),
      matchRoot: true,
    );

    testWidgets('when the state already says it', (WidgetTester tester) async {
      final SemanticsHandle semantics = tester.ensureSemantics();
      final ProviderContainer container = await pumpHistory(
        tester,
        client: FakeDaemonClient.history(count: 24),
      );
      final Flow edited = container
          .read(historyPageProvider)
          .rows
          .firstWhere((Flow flow) => flow.edited);
      await _select(tester, container, edited);

      expect(find.byKey(head), findsOneWidget, reason: 'the eye sees it');
      expect(speaking('Edited'), findsNothing);
      expect(find.bySemanticsLabel('Allowed, edited'), findsWidgets);
      semantics.dispose();
    });

    testWidgets('when the upstream failed after the edit', (
      WidgetTester tester,
    ) async {
      final SemanticsHandle semantics = tester.ensureSemantics();
      final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
      final ProviderContainer container = await pumpHistory(
        tester,
        client: client,
      );
      final Flow edited = container
          .read(historyPageProvider)
          .rows
          .firstWhere((Flow flow) => flow.edited);
      // Derselbe Fluss, dessen Ziel danach mit 502 antwortete: Die Zeile
      // steht als Fehler da, die Entscheidung bleibt eine bearbeitete.
      final Flow failed = edited.copyWith(state: FlowState.failed, status: 502);
      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: WidgetsApp(
            color: HColors.bg0,
            debugShowCheckedModeBanner: false,
            localizationsDelegates: AppLocalizations.localizationsDelegates,
            supportedLocales: AppLocalizations.supportedLocales,
            builder: (BuildContext context, Widget? _) => HTheme(
              tokens: HTokens.dark,
              child: Overlay(
                initialEntries: <OverlayEntry>[
                  OverlayEntry(
                    builder: (BuildContext context) =>
                        HistoryDetail(flow: failed),
                  ),
                ],
              ),
            ),
          ),
        ),
      );
      await tester.pump(const Duration(milliseconds: 600));

      expect(find.bySemanticsLabel('Error'), findsWidgets);
      expect(find.byKey(head), findsOneWidget);
      expect(speaking('Edited'), findsNothing);
      // Gesagt wird es trotzdem, von der Entscheidung und nicht vom Chip.
      expect(find.bySemanticsLabel('Allowed, edited'), findsWidgets);
      semantics.dispose();
    });
  });
}
