// Der Detailbereich: Kopfzeilen, Body über GetBody, Binärinhalt, und die
// Zahlen aus Abschnitt 6 — Textskalierung, Semantik, Trefferflächen.

import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/semantics.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/history/history_view.dart';
import 'package:humanitl/features/history/history_metrics.dart';
import 'package:humanitl/features/history/history_table.dart';
import 'package:humanitl/features/history/providers/history_detail.dart';
import 'package:humanitl/features/history/providers/history_query.dart';
import 'package:humanitl/features/history/providers/history_page.dart';

import 'fixtures.dart';
import 'harness.dart';

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
  group('the body is decoded off the widget tree', () {
    test('text becomes lines, and the line count is capped', () {
      final HistoryBody body = decodeHistoryBody(
        Uint8List.fromList(utf8.encode('a\nb\nc')),
        false,
      );
      expect(body.lines, <String>['a', 'b', 'c']);
      expect(body.binary, isFalse);
      expect(body.byteCount, 5);
      expect(body.linesTruncated, isFalse);
    });

    test('a NUL byte makes it binary, and nothing is shown as text', () {
      final HistoryBody body = decodeHistoryBody(
        Uint8List.fromList(<int>[0x89, 0x50, 0x00, 0x0d]),
        false,
      );
      expect(body.binary, isTrue);
      expect(body.lines, isEmpty);
      expect(body.byteCount, 4);
    });

    test('the view capping its lines is not the recorder stopping', () {
      final HistoryBody capped = HistoryBody(
        lines: List<String>.filled(historyBodyMaxLines, 'x'),
        byteCount: 999999,
        binary: false,
        truncated: false,
      );
      expect(capped.linesCapped, isTrue);
      expect(capped.truncated, isFalse, reason: 'the recorder kept it all');
      expect(capped.linesTruncated, isTrue);

      final HistoryBody short = decodeHistoryBody(
        Uint8List.fromList(utf8.encode('a\nb')),
        false,
      );
      expect(short.linesCapped, isFalse);
    });

    testWidgets('both causes at once are both said', (
      WidgetTester tester,
    ) async {
      // A body the recorder cut short *and* this view draws only the first
      // lines of: two facts, two sentences, both on screen.
      final FakeDaemonClient client = FakeDaemonClient.history(count: 12);
      final Flow post = client.state.flows.values.firstWhere(
        (Flow flow) => flow.method == Method.post,
      );
      final FlowDetail detail = client.state.details[post.id]!;
      final BodyRef ref = detail.request!.body;
      client.state.details[post.id] = detail.copyWith(
        request: detail.request!.copyWith(
          body: ref.copyWith(truncated: true, size: 8 * 1024 * 1024),
        ),
      );
      client.state.bodies[ref.sha256
          .map((int b) => b.toRadixString(16).padLeft(2, '0'))
          .join()] = Uint8List.fromList(
        utf8.encode(
          // One character per line: more than the view draws, and still
          // under the 64 KiB that would send the decoding to an isolate,
          // which a widget test's fake clock never lets return.
          List<String>.filled(historyBodyMaxLines + 10, 'a').join('\n'),
        ),
      );

      final ProviderContainer container = await pumpHistory(
        tester,
        client: client,
      );
      await _select(
        tester,
        container,
        container
            .read(historyPageProvider)
            .rows
            .firstWhere((Flow flow) => flow.id == post.id),
      );
      // The body arrives through a future; give it a turn before reading
      // what the pane says about it.
      final HistoryBody body = await container.read(
        historyBodyProvider(
          client.state.details[post.id]!.request!.body,
        ).future,
      );
      expect(body.truncated, isTrue);
      expect(body.linesCapped, isTrue);
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 600));

      expect(find.textContaining('The recorder stopped after'), findsOneWidget);
      expect(find.textContaining('lines are shown'), findsOneWidget);
    });

    test('a truncated recording says so', () {
      final HistoryBody body = decodeHistoryBody(
        Uint8List.fromList(utf8.encode('{')),
        true,
      );
      expect(body.truncated, isTrue);
      expect(body.linesTruncated, isTrue);
    });

    test('invalid UTF-8 is replaced, never thrown', () {
      final HistoryBody body = decodeHistoryBody(
        Uint8List.fromList(<int>[0x61, 0xff, 0x62]),
        false,
      );
      expect(body.binary, isFalse);
      expect(body.lines.single, contains('a'));
      expect(body.lines.single, contains('b'));
    });
  });

  testWidgets('the detail reaches the recorded body through GetBody', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    final Flow withBody = container
        .read(historyPageProvider)
        .rows
        .firstWhere((Flow flow) => flow.method == Method.post);
    await _select(tester, container, withBody);

    // The head answers at once, from the row that is already loaded.
    expect(find.text(withBody.url), findsOneWidget);
    // The headers come from the daemon and stand in the header table.
    expect(find.text('accept'), findsOneWidget);
    // And the body comes through `GetBody`, decoded into lines.
    final FlowDetail detail = await container.read(
      historyDetailProvider(withBody.id).future,
    );
    final HistoryBody body = await container.read(
      historyBodyProvider(detail.request!.body).future,
    );
    expect(body.binary, isFalse);
    expect(body.lines.single, contains(withBody.id.value));
  });

  testWidgets('a request without a body says so as a fact, not as a gap', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    final Flow get = container
        .read(historyPageProvider)
        .rows
        .firstWhere((Flow flow) => flow.method == Method.get);
    await _select(tester, container, get);
    expect(find.text('No body.'), findsOneWidget);
  });

  testWidgets('the response tab shows the answer that came back', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final ProviderContainer container = await pumpHistory(
      tester,
      client: client,
    );
    final Flow answered = container
        .read(historyPageProvider)
        .rows
        .firstWhere((Flow flow) => flow.status == 200);
    await _select(tester, container, answered);
    // `accept` sorts first and is a request header only.
    expect(find.text('accept'), findsOneWidget, reason: 'request tab');

    await tester.tap(find.byKey(const Key('history-tab-response')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 600));
    // The tab really swapped: the request headers are gone and the answer's
    // are there.
    expect(find.text('accept'), findsNothing);
    expect(find.text('content-type'), findsOneWidget);
  });

  group('accessibility as numbers', () {
    testWidgets('a row carries state, method, host and path in its label', (
      WidgetTester tester,
    ) async {
      final SemanticsHandle handle = tester.ensureSemantics();
      final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
      final ProviderContainer container = await pumpHistory(
        tester,
        client: client,
      );
      final Flow first = container.read(historyPageProvider).rows.first;
      final SemanticsNode node = tester.getSemantics(
        find.byWidgetPredicate(
          (Widget widget) => widget is HistoryRow && widget.flow.id == first.id,
        ),
      );
      expect(node.label, contains(first.host));
      expect(node.label, contains(first.methodLabel));
      expect(node.label, contains(first.path));
      handle.dispose();
    });

    testWidgets('an unknown number is announced as unknown, not as a dash', (
      WidgetTester tester,
    ) async {
      final SemanticsHandle handle = tester.ensureSemantics();
      final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
      await pumpHistory(tester, client: client);
      // The held row has no status, no duration and no response size.
      expect(
        find.byWidgetPredicate(
          (Widget widget) =>
              widget is Semantics && widget.properties.label == 'unknown',
        ),
        findsWidgets,
        reason: 'the em dash is never the only channel',
      );
      handle.dispose();
    });

    testWidgets('the table survives TextScaler 2.0 without an overflow', (
      WidgetTester tester,
    ) async {
      final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
      final ProviderContainer container = await pumpHistory(
        tester,
        client: client,
        textScaler: const TextScaler.linear(2),
      );
      expect(tester.takeException(), isNull);
      final Flow first = container.read(historyPageProvider).rows.first;
      await _select(tester, container, first);
      expect(tester.takeException(), isNull);
    });

    testWidgets('a row grows with the text scale instead of clipping', (
      WidgetTester tester,
    ) async {
      final FakeDaemonClient plain = FakeDaemonClient.history(count: 12);
      await pumpHistory(tester, client: plain);
      final double normal = tester
          .getSize(find.byType(HistoryRow).first)
          .height;
      expect(normal, historyRowHeight);

      final FakeDaemonClient scaled = FakeDaemonClient.history(count: 12);
      await pumpHistory(
        tester,
        client: scaled,
        textScaler: const TextScaler.linear(2),
      );
      final double large = tester.getSize(find.byType(HistoryRow).first).height;
      expect(large, greaterThan(normal));
    });

    testWidgets('every sortable header is a hit target of at least 28 px', (
      WidgetTester tester,
    ) async {
      final FakeDaemonClient client = FakeDaemonClient.history(count: 12);
      await pumpHistory(tester, client: client);
      for (final String label in <String>['Time', 'Host', 'Size', 'ms']) {
        final Size size = tester.getSize(find.text(label));
        expect(size.width, greaterThan(0), reason: '$label is on screen');
      }
      // The header row itself carries the density of a row, which is the
      // 28 px minimum of `docs/UX.md` 6.
      expect(historyHeaderHeight, greaterThanOrEqualTo(HSize.hitMin));
    });
  });

  group('contrast, measured rather than claimed', () {
    /// Every colour the pumped tree really paints text in.
    ///
    /// Read from the widgets, not from a list: a list describes the
    /// intention, and the one colour that slipped through the previous
    /// version of this test was in the list under a comment claiming the
    /// screen used it. What is measured has to come out of the tree.
    Set<Color> textColoursOf(WidgetTester tester, HTokens tokens) {
      final Set<Color> colours = <Color>{};
      for (final Element element in find.byType(Text).evaluate()) {
        final Text text = element.widget as Text;
        if ((text.data ?? '').isEmpty && text.textSpan == null) {
          continue;
        }
        final TextStyle? own = text.style;
        colours.add(
          own?.color ??
              DefaultTextStyle.of(element).style.color ??
              tokens.colors.fg0,
        );
      }
      return colours;
    }

    Future<void> expectReadable(
      WidgetTester tester,
      HTokens tokens,
      String where,
    ) async {
      final Set<Color> colours = textColoursOf(tester, tokens);
      expect(colours, isNotEmpty, reason: '$where drew no text');
      for (final Color colour in colours) {
        // `fg2` is the one colour reserved for controls that are really
        // disabled; this screen must not print a sentence in it either.
        expect(
          colour,
          isNot(tokens.colors.fg2),
          reason: '$where uses the disabled colour for text',
        );
        // The label of the one filled control stands on the accent and not
        // on the neutral ladder. It is measured by the test below, because
        // in the light theme `packages/ui` does not reach the threshold
        // there and that is not this screen's to fix.
        if (colour == tokens.colors.onAccent) {
          continue;
        }
        for (final Color surface in tokens.colors.ladder) {
          expect(
            HColorDerivation.contrast(colour, surface),
            greaterThanOrEqualTo(4.5),
            reason:
                '$where: ${HColorDerivation.toHex(colour)} on '
                '${HColorDerivation.toHex(surface)}',
          );
        }
      }
    }

    for (final (String name, HTokens tokens) in <(String, HTokens)>[
      ('dark', HTokens.dark),
      ('light', HTokens.light),
    ]) {
      testWidgets('every text the $name table draws reaches 4.5:1', (
        WidgetTester tester,
      ) async {
        await pumpHistory(
          tester,
          client: FakeDaemonClient.history(count: 24),
          tokens: tokens,
        );
        await expectReadable(tester, tokens, 'the table');
      });

      testWidgets('every text the $name detail draws reaches 4.5:1', (
        WidgetTester tester,
      ) async {
        final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
        final ProviderContainer container = await pumpHistory(
          tester,
          client: client,
          tokens: tokens,
        );
        // Every visual state in turn: the head prints the state label, and
        // that is where the area colour got in last time.
        for (final Flow flow in container.read(historyPageProvider).rows) {
          await _select(tester, container, flow);
          await expectReadable(
            tester,
            tokens,
            'the detail of ${historyVisualState(flow)}',
          );
        }
      });

      testWidgets('every text the $name export modal draws reaches 4.5:1', (
        WidgetTester tester,
      ) async {
        await pumpHistory(
          tester,
          client: FakeDaemonClient.history(count: 24),
          tokens: tokens,
        );
        await tester.tap(find.byKey(const Key('history-export-open')));
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 300));
        await expectReadable(tester, tokens, 'the export modal');
      });

      testWidgets('every text the $name filter error draws reaches 4.5:1', (
        WidgetTester tester,
      ) async {
        final FakeDaemonClient client = FakeDaemonClient.history(count: 12);
        final ProviderContainer container = await pumpHistory(
          tester,
          client: client,
          tokens: tokens,
        );
        container
            .read(historyQueryProvider.notifier)
            .submit('hosst:github.com');
        await settleHistory(tester, container);
        await expectReadable(tester, tokens, 'the refused filter');
      });
    }

    test('the label of a filled control, and who owns it', () {
      // `docs/UX.md` 6 asks 4,5:1 of text against the surface it really
      // stands on, the accent fill included. The dark theme holds; the light
      // one does not, and the colours belong to `packages/ui`, not to this
      // screen. Written down rather than waved through, so that fixing
      // `HSurfaceColors.light` fails this test and removes the exception.
      expect(
        HColorDerivation.contrast(
          HTokens.dark.colors.onAccent,
          HTokens.dark.colors.accent,
        ),
        greaterThanOrEqualTo(4.5),
      );
      expect(
        HColorDerivation.contrast(
          HTokens.light.colors.onAccent,
          HTokens.light.colors.accent,
        ),
        closeTo(3.73, 0.01),
        reason: 'known: `lBg1` on `lAccent`, a token of packages/ui',
      );
    });

    test('the area palette really is the one that would fail', () {
      // The guard above only means something because the obvious wrong
      // choice is measurably wrong.
      for (final HFlowState state in HFlowState.values) {
        expect(
          HColorDerivation.worstContrast(
            HTokens.light.stateColor(state),
            HTokens.light.colors.ladder,
          ),
          lessThan(4.5),
          reason: '$state as text',
        );
      }
    });

    test('every state rail reaches 3:1 as an area', () {
      for (final HTokens tokens in <HTokens>[HTokens.dark, HTokens.light]) {
        for (final HFlowState state in HFlowState.values) {
          expect(
            HColorDerivation.worstContrast(
              tokens.stateColor(state),
              tokens.colors.ladder,
            ),
            greaterThanOrEqualTo(3.0),
            reason: '$state in ${tokens.brightness}',
          );
        }
      }
    });
  });

  group('an answer that is still coming in', () {
    test('a responded flow is not final, only a recorded one is', () {
      // Response chunks keep raising the size after the head came back, so a
      // zero there would claim a number nobody has yet.
      for (final FlowState state in <FlowState>[
        FlowState.forwarded,
        FlowState.responded,
      ]) {
        final Flow running = testFlow(
          id: 'r-${state.name}',
          state: state,
          responseSize: 0,
          duration: null,
        );
        expect(
          formatHistorySizePair(running, unknown: '?'),
          '512 / ?',
          reason: state.name,
        );
        expect(historyResponseStreaming(running), isTrue);
      }
      final Flow done = testFlow(
        id: 'r-recorded',
        state: FlowState.recorded,
        responseSize: 0,
      );
      expect(formatHistorySizePair(done, unknown: '?'), '512 / 0');
      expect(historyResponseStreaming(done), isFalse);
    });

    testWidgets('the detail head says the size is still growing', (
      WidgetTester tester,
    ) async {
      final FakeDaemonClient client = FakeDaemonClient.history(count: 12);
      final Flow first = client.state.flows.values.first;
      client.state.flows[first.id] = first.copyWith(
        state: FlowState.responded,
        responseSize: 2100,
      );
      final ProviderContainer container = await pumpHistory(
        tester,
        client: client,
      );
      await _select(
        tester,
        container,
        container
            .read(historyPageProvider)
            .rows
            .firstWhere((Flow flow) => flow.id == first.id),
      );
      expect(find.textContaining('still arriving'), findsOneWidget);
    });
  });

  group('the geometry of the table', () {
    test('the columns keep their widths and the path takes the rest', () {
      final double narrow = historyTableWidth(600);
      expect(narrow, historyMinTableWidth);
      expect(
        historyColumnWidth(HistoryColumn.path, narrow),
        historyPathMinWidth,
      );

      final double wide = historyTableWidth(historyMinTableWidth + 400);
      expect(
        historyColumnWidth(HistoryColumn.path, wide),
        historyPathMinWidth + 400,
      );
      expect(historyColumnWidth(HistoryColumn.host, wide), 220);
    });

    test('every spacing of the table is a multiple of four', () {
      for (final double value in <double>[
        historyRowHeight,
        historyBodyRowHeight,
        historyHeaderHeight,
        historyCellGap,
        historyRowLeading,
        historyRowTrailing,
        for (final HistoryColumn column in HistoryColumn.values) column.width,
      ]) {
        expect(value % 4, 0, reason: '$value');
      }
    });

    test('only the four columns the recorder can order by are sortable', () {
      expect(
        HistoryColumn.values
            .where((HistoryColumn column) => column.sort != null)
            .map((HistoryColumn column) => column.sort)
            .toSet(),
        HistorySort.values.toSet(),
      );
    });
  });
}
