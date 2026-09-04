// Der History-Screen: Blättern, der abgelehnte Filter mit seiner Schlüssel-
// liste, der Doppelklick auf eine angehaltene Zeile, die Zahl der Zeilen-
// Builds beim Nachladen und die Tastenmengen.

import 'package:flutter/gestures.dart';
import 'package:flutter/semantics.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ipc/flow_handoff.dart';
import 'package:humanitl/core/shortcuts/intents.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/history/history_screen.dart';
import 'package:humanitl/features/history/history_table.dart';
import 'package:humanitl/features/history/providers/history_detail.dart';
import 'package:humanitl/features/history/history_view.dart';
import 'package:humanitl/features/history/providers/history_page.dart';
import 'package:humanitl/features/history/providers/history_query.dart';

import 'harness.dart';

Future<void> _scrollTo(WidgetTester tester, double fraction) async {
  final ScrollableState scrollable = tester.state<ScrollableState>(
    find.descendant(
      of: find.byKey(const Key('history-list')),
      matching: find.byType(Scrollable),
    ),
  );
  final ScrollPosition position = scrollable.position;
  position.jumpTo(position.maxScrollExtent * fraction);
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 16));
}

void main() {
  testWidgets('the table shows the recorded rows and their footer', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    await pumpHistory(tester, client: client);
    expect(find.byType(HistoryRow), findsWidgets);
    final int visible = client.state.flows.values
        .where((Flow flow) => !flow.passthrough)
        .length;
    // The footer counts what is loaded against what the filter matches, and
    // the second number is exact because the daemon said so.
    expect(find.textContaining('$visible of $visible loaded'), findsOneWidget);
    expect(find.text('registry.npmjs.org'), findsWidgets);
  });

  testWidgets('paging_loads_more_at_80pct', (WidgetTester tester) async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 450);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    expect(
      container.read(historyPageProvider).rows,
      hasLength(historyPageSize),
    );

    // Half way down nothing is asked for.
    await _scrollTo(tester, 0.5);
    expect(
      container.read(historyPageProvider).rows,
      hasLength(historyPageSize),
    );

    // Past four fifths the next page is asked for and arrives.
    await _scrollTo(tester, 0.85);
    await settleHistory(tester, container);
    expect(
      container.read(historyPageProvider).rows.length,
      greaterThan(historyPageSize),
    );
    expect(
      container
          .read(historyPageProvider)
          .rows
          .map((Flow flow) => flow.id.value)
          .toSet(),
      hasLength(container.read(historyPageProvider).rows.length),
      reason: 'a second page duplicates no row',
    );
  });

  testWidgets('a page change rebuilds no row that was already on screen', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 450);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    // The rows that are on screen right now; scrolling would build the ones
    // that come into view, so the count is taken without moving the list.
    await tester.pump();
    debugHistoryRowBuilds = 0;

    await container.read(historyPageProvider.notifier).loadMore();
    await settleHistory(tester, container);

    // The new page sits below the viewport: not one visible row was rebuilt,
    // because the table hands back the same widget instance per flow and
    // `Element.updateChild` stops at an identical child (`docs/UX.md` 7).
    expect(debugHistoryRowBuilds, 0);
    expect(
      container.read(historyPageProvider).rows.length,
      greaterThan(historyPageSize),
      reason: 'a second page really arrived',
    );
  });

  testWidgets('a live state change rebuilds exactly its own row', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    final Flow held = container
        .read(historyPageProvider)
        .rows
        .firstWhere((Flow flow) => flow.isHeld);
    debugHistoryRowBuilds = 0;
    await client.decide(held.id, const Decision.allow());
    await settleHistory(tester, container);
    expect(debugHistoryRowBuilds, 1);
  });

  testWidgets('a splitter drag rebuilds no row and no cell', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    final Flow first = container.read(historyPageProvider).rows.first;
    container.read(historySelectionProvider.notifier).select(first.id);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 600));

    debugHistoryRowBuilds = 0;
    final Offset splitter = tester.getCenter(
      find.byWidgetPredicate(
        (Widget widget) =>
            widget is MouseRegion &&
            widget.cursor == SystemMouseCursors.resizeRow,
      ),
    );
    final TestGesture drag = await tester.startGesture(splitter);
    for (int i = 0; i < 10; i++) {
      await drag.moveBy(const Offset(0, -4));
      await tester.pump();
    }
    await drag.up();
    await tester.pump();

    // The share lives in a `ValueNotifier` that only the layout widget hears,
    // so ten pointer moves cost no row build at all (`docs/UX.md` 7).
    expect(debugHistoryRowBuilds, 0);
  });

  testWidgets('a session of nothing but model calls names the chip', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    for (final FlowId id in client.state.flows.keys.toList()) {
      client.state.flows[id] = client.state.flows[id]!.copyWith(
        passthrough: true,
      );
    }
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    // Not "The record is open": three requests were recorded and the chip is
    // hiding them.
    expect(find.byKey(const Key('history-empty-passthrough')), findsOneWidget);
    expect(find.text('The record is open'), findsNothing);

    // The chip is the way back, and it works from here.
    await tester.tap(
      find.descendant(
        of: find.byKey(const Key('history-empty-passthrough')),
        matching: find.text('LLM traffic hidden'),
      ),
    );
    await settleHistory(tester, container);
    expect(find.byType(HistoryRow), findsWidgets);
  });

  testWidgets('the filter hint teaches a grammar that matches something', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 120);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    final String hint = tester
        .widget<Text>(find.byKey(const Key('history-filter-hint')))
        .data!;

    // Every term of the one string whose purpose is to teach is accepted.
    for (final String term in hint.split(' ')) {
      container.read(historyQueryProvider.notifier).submit(term);
      await settleHistory(tester, container);
      expect(
        container.read(historyPageProvider).failure,
        isNull,
        reason: '$term was refused',
      );
    }

    // And the two that name a set really name one. `since:1h` is left out on
    // purpose: it is relative to the clock, and the recorded session is a
    // fixed point in the past.
    for (final String term in <String>['host:github.com', 'decision:block']) {
      container.read(historyQueryProvider.notifier).submit(term);
      await settleHistory(tester, container);
      expect(
        container.read(historyPageProvider).rows,
        isNotEmpty,
        reason: '$term matches nothing',
      );
    }

    // Why the hint changed: `state:` compares against the seven states of
    // the automaton, and `blocked` is not one of them. No error, no warning,
    // just nothing -- the worst thing an example can do.
    container.read(historyQueryProvider.notifier).submit('state:blocked');
    await settleHistory(tester, container);
    expect(container.read(historyPageProvider).failure, isNull);
    expect(container.read(historyPageProvider).rows, isEmpty);
    expect(hint, isNot(contains('state:blocked')));
  });

  testWidgets('the detail head names the decision, not the visual state', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    // A flow that was allowed and whose upstream then failed: the row is an
    // error, the decision was an allow.
    final Flow failed = container
        .read(historyPageProvider)
        .rows
        .firstWhere((Flow flow) => flow.upstreamError != null);
    expect(historyVisualState(failed), HFlowState.error);
    expect(failed.decision, DecisionKind.allow);

    container.read(historySelectionProvider.notifier).select(failed.id);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 600));

    final Finder head = find.ancestor(
      of: find.text('Decision'),
      matching: find.byType(Row),
    );
    expect(
      find.descendant(of: head.first, matching: find.text('Allowed')),
      findsOneWidget,
    );
    expect(
      find.descendant(of: head.first, matching: find.text('Error')),
      findsNothing,
    );
  });

  testWidgets('filter_error_shows_keys', (WidgetTester tester) async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 12);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    await tester.enterText(
      find.byKey(const Key('history-filter-input')),
      'hosst:github.com',
    );
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await settleHistory(tester, container);

    expect(find.text('RECORDER_002'), findsOneWidget);
    expect(find.text('The filter cannot be read'), findsOneWidget);
    // The daemon's sentence is shown as it stands, with every valid key in
    // it; the app does not rewrite it.
    for (final String key in fakeFilterKeys) {
      expect(
        find.textContaining(key, findRichText: true),
        findsWidgets,
        reason: key,
      );
    }
  });

  testWidgets('an empty filter result names the filter and the way back', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    container.read(historyQueryProvider.notifier).submit('host:nowhere.test');
    await settleHistory(tester, container);
    expect(
      find.textContaining('host:nowhere.test matches 0 of'),
      findsOneWidget,
    );
    expect(find.text('Reset filter'), findsWidgets);
  });

  testWidgets('double_click_held_navigates_intercept', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    expect(container.read(flowHandoffProvider), isNull);

    final Flow held = container
        .read(historyPageProvider)
        .rows
        .firstWhere((Flow flow) => flow.isHeld);
    final Finder row = find.byWidgetPredicate(
      (Widget widget) => widget is HistoryRow && widget.flow.id == held.id,
    );
    expect(row, findsOneWidget);
    await tester.tap(row);
    await tester.pump(kDoubleTapMinTime);
    await tester.tap(row);
    await tester.pump(kDoubleTapTimeout);

    // The history asks; the shell carries it out. A feature may not reach
    // into another feature (ARCHITECTURE 5), and `history_handoff_test.dart`
    // holds the other half of the promise.
    expect(container.read(flowHandoffProvider), held.id);
  });

  testWidgets('a double click on a finished row opens the sheet', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    final Flow done = container
        .read(historyPageProvider)
        .rows
        .firstWhere((Flow flow) => !flow.isHeld);
    final Finder row = find.byWidgetPredicate(
      (Widget widget) => widget is HistoryRow && widget.flow.id == done.id,
    );
    expect(find.byType(HSheet), findsNothing);

    await tester.tap(row);
    await tester.pump(kDoubleTapMinTime);
    await tester.tap(row);
    await tester.pump(kDoubleTapTimeout);
    await tester.pump(const Duration(milliseconds: 300));

    // The sheet is really there, and it shows the row that was clicked.
    expect(find.byType(HSheet), findsOneWidget);
    expect(
      find.descendant(
        of: find.byType(HSheet),
        matching: find.textContaining(done.host),
      ),
      findsWidgets,
    );
    expect(container.read(historySelectionProvider), done.id);
    // A finished request is read where it is; nothing is handed over.
    expect(container.read(flowHandoffProvider), isNull);
  });

  testWidgets('Escape closes the sheet', (WidgetTester tester) async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    final Flow done = container
        .read(historyPageProvider)
        .rows
        .firstWhere((Flow flow) => !flow.isHeld);
    final Finder row = find.byWidgetPredicate(
      (Widget widget) => widget is HistoryRow && widget.flow.id == done.id,
    );
    await tester.tap(row);
    await tester.pump(kDoubleTapMinTime);
    await tester.tap(row);
    await tester.pump(kDoubleTapTimeout);
    await tester.pump(const Duration(milliseconds: 300));
    expect(find.byType(HSheet), findsOneWidget);

    // Checked at the behaviour and not at the mapping: `WidgetsApp` binds
    // `Escape` to `DismissIntent` itself, so a shortcut map of one's own
    // proves nothing -- the handler is what closes the sheet.
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump(const Duration(milliseconds: 300));
    expect(find.byType(HSheet), findsNothing);
  });

  testWidgets('the splitter moves with the arrow keys, not only with a drag', (
    WidgetTester tester,
  ) async {
    // Few rows on purpose: every row is a focus stop, and the walk to the
    // splitter would otherwise be as long as the table.
    final FakeDaemonClient client = FakeDaemonClient.history(count: 3);
    await pumpHistory(tester, client: client);
    final Finder table = find.byType(HistoryTable);
    final double before = tester.getSize(table).height;

    // Tab until the splitter has the focus; it is a focus stop like every
    // other pointer affordance (`docs/UX.md` 5.1).
    bool onSplitter = false;
    for (int i = 0; i < 30 && !onSplitter; i++) {
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
      final BuildContext? focused = FocusManager.instance.primaryFocus?.context;
      onSplitter =
          focused != null &&
          find
              .descendant(
                of: find.byKey(const Key('history-splitter')),
                matching: find.byWidget(focused.widget),
              )
              .evaluate()
              .isNotEmpty;
    }
    expect(onSplitter, isTrue, reason: 'the splitter takes the focus');

    await tester.sendKeyEvent(LogicalKeyboardKey.arrowUp);
    await tester.pump();
    final double afterUp = tester.getSize(table).height;
    expect(afterUp, lessThan(before), reason: 'up gives the detail more room');

    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pump();
    expect(tester.getSize(table).height, greaterThan(afterUp));
  });

  testWidgets('the splitter is a slider to a screen reader', (
    WidgetTester tester,
  ) async {
    final SemanticsHandle handle = tester.ensureSemantics();
    final FakeDaemonClient client = FakeDaemonClient.history(count: 12);
    await pumpHistory(tester, client: client);
    final SemanticsNode node = tester.getSemantics(
      find.bySemanticsLabel('Resize the detail area'),
    );
    expect(node.flagsCollection.isSlider, isTrue);
    handle.dispose();
  });

  testWidgets('the focus is claimed once, not taken back on every build', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    // Move the focus somewhere else on purpose, the way Tab would.
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump();
    final FocusNode? afterTab = FocusManager.instance.primaryFocus;
    expect(afterTab, isNotNull);

    expect(afterTab!.debugLabel, isNot('history-table'));

    // The screen asked for the keyboard once, when it became visible. Its own
    // focus node cannot tell the difference -- a focused row counts as
    // focused -- so the count is what holds the promise: a claim per build
    // would pull the focus out of the shell's rail one frame after somebody
    // tabbed there.
    final int claims = debugHistoryFocusClaims;
    final List<Flow> rows = container.read(historyPageProvider).rows;
    for (int i = 0; i < 3; i++) {
      container.read(historySelectionProvider.notifier).select(rows[i].id);
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 400));
    }
    expect(debugHistoryFocusClaims, claims);
    expect(FocusManager.instance.primaryFocus, same(afterTab));
  });

  testWidgets('a click selects a row and the detail head follows', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    final Flow first = container.read(historyPageProvider).rows.first;
    await tester.tap(
      find.byWidgetPredicate(
        (Widget widget) => widget is HistoryRow && widget.flow.id == first.id,
      ),
    );
    await tester.pump(kDoubleTapTimeout);
    await tester.pump(const Duration(milliseconds: 300));
    expect(container.read(historySelectionProvider), first.id);
    expect(find.text(first.url), findsOneWidget);
  });

  testWidgets('J and K move the selection, the slash focuses the filter', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    final List<Flow> rows = container.read(historyPageProvider).rows;
    await tester.sendKeyEvent(LogicalKeyboardKey.keyJ);
    await tester.pump();
    expect(container.read(historySelectionProvider), rows.first.id);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyJ);
    await tester.pump();
    expect(container.read(historySelectionProvider), rows[1].id);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyK);
    await tester.pump();
    expect(container.read(historySelectionProvider), rows.first.id);

    await tester.sendKeyEvent(LogicalKeyboardKey.slash);
    await tester.pump();
    expect(isTextInputFocused(), isTrue);
    // A single letter no longer decides anything while the caret is in the
    // field: the key reaches the text (`docs/UX.md` 5.2).
    await tester.sendKeyEvent(LogicalKeyboardKey.keyJ);
    await tester.pump();
    expect(container.read(historySelectionProvider), rows.first.id);
  });

  testWidgets('a sort click changes the order the daemon is asked for', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 60);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    expect(container.read(historyQueryProvider).orderBy, 'ts desc');
    await tester.tap(find.text('Host'));
    await settleHistory(tester, container);
    expect(container.read(historyQueryProvider).orderBy, 'host desc');
    await tester.tap(find.text('Host'));
    await settleHistory(tester, container);
    expect(container.read(historyQueryProvider).orderBy, 'host asc');
  });

  testWidgets('a chip writes its term into the field', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    await tester.tap(find.byKey(const Key('history-chip-held')));
    await settleHistory(tester, container);
    expect(container.read(historyQueryProvider).filter, 'state:held');
    expect(
      container.read(historyPageProvider).rows.every((Flow f) => f.isHeld),
      isTrue,
    );
    await tester.tap(find.byKey(const Key('history-chip-held')));
    await settleHistory(tester, container);
    expect(container.read(historyQueryProvider).filter, isEmpty);
  });

  testWidgets('Enter opens the selected row, like a double click', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    final Flow held = container
        .read(historyPageProvider)
        .rows
        .firstWhere((Flow flow) => flow.isHeld);
    container.read(historySelectionProvider.notifier).select(held.id);
    await tester.pump();

    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(container.read(flowHandoffProvider), held.id);
  });

  testWidgets('a focused control keeps Enter for itself', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    final Flow first = container.read(historyPageProvider).rows.first;
    container.read(historySelectionProvider.notifier).select(first.id);
    await tester.pump();

    // Tab until a control that handles `ActivateIntent` has the focus; from
    // then on Enter belongs to it, not to the row (`docs/UX.md` 5.2).
    bool onControl = false;
    for (int i = 0; i < 12 && !onControl; i++) {
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
      final BuildContext? focused = FocusManager.instance.primaryFocus?.context;
      onControl =
          focused != null &&
          // `maybeFind<ActivateIntent>` bricht ab, sobald die gefundene
          // Action ein `CallbackAction<Intent>` ist — und genau das legt
          // `Clickable` aus `shadcn_flutter` unter jedes Control. Der
          // Typparameter `Intent` ist der Weg, den Flutter dafür nennt
          // (flutter/flutter#180871).
          Actions.maybeFind<Intent>(focused, intent: const ActivateIntent()) !=
              null;
    }
    expect(onControl, isTrue, reason: 'a control takes the focus');

    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await settleHistory(tester, container);
    expect(container.read(flowHandoffProvider), isNull);
  });

  testWidgets('eine Bildschirmtaste bricht nicht, wenn ein Control steht', (
    WidgetTester tester,
  ) async {
    // Derselbe Pfad wie oben, aber über `_SingleKeyAction.isActionEnabled`:
    // der läuft bei **jedem** Druck auf `j`, `k` und `/`. Vor der Korrektur
    // warf `maybeFind<ActivateIntent>` dort auf der `CallbackAction<Intent>`
    // von `Clickable` (flutter/flutter#180871).
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    final Flow first = container.read(historyPageProvider).rows.first;
    container.read(historySelectionProvider.notifier).select(first.id);
    await tester.pump();

    bool onControl = false;
    for (int i = 0; i < 12 && !onControl; i++) {
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
      final BuildContext? focused = FocusManager.instance.primaryFocus?.context;
      onControl =
          focused != null &&
          Actions.maybeFind<Intent>(focused, intent: const ActivateIntent()) !=
              null;
    }
    expect(onControl, isTrue, reason: 'ein Control nimmt den Fokus');

    for (final LogicalKeyboardKey key in <LogicalKeyboardKey>[
      LogicalKeyboardKey.keyJ,
      LogicalKeyboardKey.keyK,
      LogicalKeyboardKey.slash,
    ]) {
      await tester.sendKeyEvent(key);
      await settleHistory(tester, container);
      expect(tester.takeException(), isNull, reason: key.keyLabel);
    }
  });

  test('every bound key has an action in the same screen', () {
    final Set<Type> bound = historyShortcuts().values
        .map((Intent intent) => intent.runtimeType)
        .toSet();
    const Set<Type> handled = <Type>{
      OpenFlowIntent,
      FilterIntent,
      NextFlowIntent,
      PrevFlowIntent,
    };
    expect(bound, handled);
  });
}
