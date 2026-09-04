// Was eine aufgezeichnete Zeile anzeigt: der Zustand, die sechs Zahlen und
// die ehrliche Trefferzahl.

import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/history/history_view.dart';

import 'fixtures.dart';

void main() {
  group('state_color_mapping', () {
    test('all eight visual states are reachable from a recorded flow', () {
      final Map<HFlowState, Flow> cases = <HFlowState, Flow>{
        HFlowState.held: testFlow(
          id: '1',
          state: FlowState.held,
          status: 0,
          duration: null,
        ),
        HFlowState.allowed: testFlow(
          id: '2',
          decision: DecisionKind.allow,
          source: DecisionSource.user,
        ),
        HFlowState.allowedEdited: testFlow(
          id: '3',
          decision: DecisionKind.allowEdited,
          source: DecisionSource.user,
          edited: true,
        ),
        HFlowState.blocked: testFlow(
          id: '4',
          decision: DecisionKind.block,
          source: DecisionSource.user,
          blockReason: BlockReason.user,
          status: 403,
        ),
        HFlowState.timedOut: testFlow(
          id: '5',
          decision: DecisionKind.timedOut,
          source: DecisionSource.timeout,
          blockReason: BlockReason.timeout,
          status: 504,
        ),
        HFlowState.autoRule: testFlow(
          id: '6',
          decision: DecisionKind.allow,
          source: DecisionSource.rule,
          ruleId: testRule,
        ),
        HFlowState.passthroughLlm: testFlow(
          id: '7',
          decision: DecisionKind.allow,
          source: DecisionSource.passthrough,
          passthrough: true,
        ),
        HFlowState.error: testFlow(
          id: '8',
          state: FlowState.failed,
          decision: DecisionKind.allow,
          source: DecisionSource.user,
          upstreamError: UpstreamError.connect,
          status: 0,
        ),
      };
      expect(cases.length, HFlowState.values.length);
      cases.forEach((HFlowState expected, Flow flow) {
        expect(historyVisualState(flow), expected, reason: flow.id.value);
      });
    });

    test('a 5xx answer and no_route read as an error', () {
      expect(
        historyVisualState(
          testFlow(
            id: '9',
            decision: DecisionKind.allow,
            source: DecisionSource.user,
            status: 502,
          ),
        ),
        HFlowState.error,
      );
      expect(
        historyVisualState(
          testFlow(
            id: '10',
            decision: DecisionKind.block,
            source: DecisionSource.system,
            blockReason: BlockReason.noRoute,
            status: 403,
          ),
        ),
        HFlowState.error,
      );
    });

    test('every state carries a glyph beside its colour', () {
      for (final HFlowState state in HFlowState.values) {
        expect(state.glyph, isNotNull);
        expect(state.l10nKey, isNotEmpty);
      }
    });
  });

  group('the numbers of a row', () {
    test('a capped total is written with a plus, an exact one without', () {
      // The flag comes from the daemon (`FlowPage.capped`); the surface only
      // has to write it down (`backlog/CONVENTIONS.md` 4.13).
      const FlowPage exact = FlowPage(total: 1284);
      const FlowPage lowerBound = FlowPage(total: 10000, capped: true);
      expect(exact.capped, isFalse);
      expect(exact.totalText('1,284'), '1,284');
      expect(lowerBound.totalText('10,000'), '10,000+');
    });

    test('sizes are compact and decimal', () {
      expect(formatHistoryCompactSize(0), '0');
      expect(formatHistoryCompactSize(999), '999');
      expect(formatHistoryCompactSize(1000), '1.0k');
      expect(formatHistoryCompactSize(2100), '2.1k');
      expect(formatHistoryCompactSize(48000), '48k');
      expect(formatHistoryCompactSize(1200000), '1.2M');
    });

    test('an unfinished response says unknown instead of zero', () {
      final Flow running = testFlow(
        id: '11',
        state: FlowState.forwarded,
        responseSize: 0,
        duration: null,
        status: 0,
      );
      expect(formatHistorySizePair(running, unknown: '?'), '512 / ?');
      expect(formatHistoryDuration(running, unknown: '?'), '?');
      expect(formatHistoryStatus(running, unknown: '?'), '?');
    });

    test('a finished response with no body prints a zero, not a dash', () {
      final Flow empty = testFlow(id: '12', responseSize: 0, status: 204);
      expect(formatHistorySizePair(empty, unknown: '?'), '512 / 0');
    });

    test('time is local, the timestamp is written out, ISO 8601 is UTC', () {
      final DateTime at = DateTime.utc(2026, 9, 3, 8, 15, 30, 250);
      expect(formatHistoryIso8601(at), '2026-09-03T08:15:30.250Z');
      expect(formatHistoryTime(at), matches(RegExp(r'^\d{2}:\d{2}:30$')));
      expect(
        formatHistoryTimestamp(at),
        matches(RegExp(r'^2026-09-0\d \d{2}:\d{2}:30$')),
      );
    });
  });

  group('what decided a flow', () {
    test('the five words', () {
      expect(
        historyDecider(testFlow(id: 'a', state: FlowState.held, status: 0)),
        HistoryDecider.pending,
      );
      expect(
        historyDecider(
          testFlow(
            id: 'b',
            decision: DecisionKind.allow,
            source: DecisionSource.user,
          ),
        ),
        HistoryDecider.manual,
      );
      expect(
        historyDecider(
          testFlow(
            id: 'c',
            decision: DecisionKind.block,
            source: DecisionSource.rule,
            ruleId: testRule,
          ),
        ),
        HistoryDecider.rule,
      );
      expect(
        historyDecider(
          testFlow(
            id: 'd',
            decision: DecisionKind.timedOut,
            source: DecisionSource.timeout,
          ),
        ),
        HistoryDecider.timeout,
      );
      expect(
        historyDecider(
          testFlow(
            id: 'e',
            passthrough: true,
            decision: DecisionKind.allow,
            source: DecisionSource.passthrough,
          ),
        ),
        HistoryDecider.passthrough,
      );
    });

    test('a rule id is shortened to its first eight characters', () {
      expect(historyRuleShort(testRule), '018f0005');
      expect(historyRuleShort(const RuleId('abc')), 'abc');
    });
  });
}
