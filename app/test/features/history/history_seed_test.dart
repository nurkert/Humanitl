// Was das History-Szenario des Fake-Daemons liefert. Die Zeilen entscheiden,
// was jedes History-Golden zeigt; ohne diesen Test merkt das eine Änderung
// daran erst der Mensch, der das Golden ansieht.

import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/history/history_view.dart';

void main() {
  test('twenty-four rows carry all eight visual states', () {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final Set<HFlowState> seen = client.state.flows.values
        .map(historyVisualState)
        .toSet();
    expect(seen, HFlowState.values.toSet());
  });

  test('a recorded flow belongs to the recorded session, not the live one', () {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 12);
    expect(
      client.state.flows.values.every(
        (Flow flow) => flow.sessionId == historySession,
      ),
      isTrue,
    );
    expect(historySession, isNot(FakeDaemonClient.defaultSession));
  });

  test('a rule decision names the rule of this history, never the bundled '
      'block rule', () {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final Iterable<Flow> byRule = client.state.flows.values.where(
      (Flow flow) => flow.decisionSource == DecisionSource.rule,
    );
    expect(byRule, isNotEmpty);
    expect(byRule.every((Flow flow) => flow.ruleId == historyRule), isTrue);
    // The bundled rule blocks; it cannot be the source of an allow.
    expect(
      byRule.any(
        (Flow flow) => flow.ruleId == FakeDaemonClient.bundledBlockRule,
      ),
      isFalse,
    );
    // A rule both allows and blocks, so `autoRule` is reachable from both.
    expect(byRule.map((Flow flow) => flow.decision).toSet(), <DecisionKind>{
      DecisionKind.allow,
      DecisionKind.block,
    });
  });

  test('durations and sizes stay bounded as the history grows', () {
    // A duration that grows with the index would make sorting by duration the
    // same as sorting by time, and a test that tells the two apart would tell
    // nothing apart.
    final FakeDaemonClient client = FakeDaemonClient.history(count: 2400);
    final Iterable<Flow> flows = client.state.flows.values;
    for (final Flow flow in flows) {
      expect(
        flow.duration?.inMilliseconds ?? 0,
        lessThan(1000),
        reason: '${flow.id.value} took ${flow.duration}',
      );
      expect(flow.requestSize, lessThan(2000));
      expect(flow.responseSize, lessThan(20000));
    }
    // And they really vary, or the columns would be constant.
    expect(
      flows.map((Flow flow) => flow.duration).toSet().length,
      greaterThan(5),
    );
    expect(
      flows.map((Flow flow) => flow.responseSize).toSet().length,
      greaterThan(5),
    );
  });

  test('the ids rise with the arrival time, so keyset paging and sorting '
      'agree', () {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 40);
    final List<Flow> byTime = client.state.flows.values.toList()
      ..sort((Flow a, Flow b) => a.receivedAt.compareTo(b.receivedAt));
    for (int i = 1; i < byTime.length; i++) {
      expect(
        byTime[i].id.value.compareTo(byTime[i - 1].id.value),
        greaterThan(0),
      );
      expect(
        byTime[i].receivedAt.difference(byTime[i - 1].receivedAt),
        historyStep,
      );
    }
  });

  test('findings cross the states instead of marking one of them', () {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 84);
    final Set<HFlowState> withFindings = client.state.flows.values
        .where((Flow flow) => flow.findingCount > 0)
        .map(historyVisualState)
        .toSet();
    expect(withFindings.length, greaterThan(1));
  });

  test('a row that counts findings has the findings behind it', () {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    for (final FlowDetail detail in client.state.details.values) {
      expect(
        detail.findings.length,
        detail.summary.findingCount,
        reason: '${detail.summary.id.value} counts what it does not carry',
      );
    }
    // And there really are some, or the check above passes on emptiness.
    expect(
      client.state.details.values.any(
        (FlowDetail detail) => detail.findings.isNotEmpty,
      ),
      isTrue,
    );
  });

  test('a row marked edited carries the request it was edited into', () {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final Iterable<FlowDetail> edited = client.state.details.values.where(
      (FlowDetail detail) => detail.summary.edited,
    );
    expect(edited, isNotEmpty);
    for (final FlowDetail detail in edited) {
      expect(detail.editedRequest, isNotNull);
      expect(detail.editedRequest!.body.size, greaterThan(0));
      expect(
        client.state.bodies.containsKey(
          detail.editedRequest!.body.sha256
              .map((int b) => b.toRadixString(16).padLeft(2, '0'))
              .join(),
        ),
        isTrue,
        reason: 'GetBody can answer for the edited request',
      );
    }
    // A row that was not edited has no second tab to show.
    for (final FlowDetail detail in client.state.details.values) {
      if (!detail.summary.edited) {
        expect(detail.editedRequest, isNull);
      }
    }
  });

  test('an answer that came back carries a body to read', () async {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final Iterable<FlowDetail> answered = client.state.details.values.where(
      (FlowDetail detail) => detail.summary.status == 200,
    );
    expect(answered, isNotEmpty);
    for (final FlowDetail detail in answered) {
      expect(detail.responseBody, isNotNull);
      final List<int> bytes =
          (await client.getBody(detail.responseBody!).toList())
              .expand((List<int> chunk) => chunk)
              .toList();
      expect(bytes, isNotEmpty);
    }
    // A refusal has a body too: the 403 and the 504 are what the proxy
    // itself wrote, and the export shows them as `content.text`.
    for (final FlowDetail detail in client.state.details.values) {
      if (detail.summary.status == 403 || detail.summary.status == 504) {
        expect(detail.responseBody, isNotNull);
      }
      // Nothing came back at all where there is no status, and a 204 is an
      // answer without content by definition.
      if (detail.summary.status == 0 || detail.summary.status == 204) {
        expect(detail.responseBody, isNull);
        expect(detail.summary.responseSize, 0);
      }
      // The number and the bytes agree, always -- checked against what
      // `GetBody` really hands back, not against the same getter twice.
      expect(
        detail.responseBody?.size ?? 0,
        detail.summary.responseSize,
        reason: detail.summary.id.value,
      );
    }
  });

  test('every recorded body belongs to the flow that names it', () async {
    // A key derived from a single byte repeats after 256 rows, and from
    // there on `GetBody` answers with the bytes of another flow while the
    // size beside it claims something else. Ten thousand rows is the
    // scenario this fake exists for.
    final FakeDaemonClient client = FakeDaemonClient.history(count: 450);
    int checked = 0;
    for (final FlowDetail detail in client.state.details.values) {
      final String id = detail.summary.id.value;
      for (final (String role, BodyRef? ref) in <(String, BodyRef?)>[
        ('request', detail.request?.body),
        ('request_edited', detail.editedRequest?.body),
        ('response', detail.responseBody),
      ]) {
        if (ref == null || ref.isEmpty) {
          continue;
        }
        final List<int> bytes = (await client.getBody(ref).toList())
            .expand((List<int> chunk) => chunk)
            .toList();
        expect(bytes, hasLength(ref.size), reason: '$id $role size');
        expect(
          utf8.decode(bytes),
          contains(id),
          reason: '$id $role is the body of another flow',
        );
        checked++;
      }
    }
    expect(checked, greaterThan(400), reason: 'the sweep really ran');
  });

  test('no two bodies of the scenario share a key', () {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 450);
    final Set<String> keys = <String>{};
    for (final FlowDetail detail in client.state.details.values) {
      for (final BodyRef? ref in <BodyRef?>[
        detail.request?.body,
        detail.editedRequest?.body,
        detail.responseBody,
      ]) {
        if (ref == null || ref.isEmpty) {
          continue;
        }
        final String key = ref.sha256
            .map((int b) => b.toRadixString(16).padLeft(2, '0'))
            .join();
        expect(keys.add(key), isTrue, reason: 'a key was handed out twice');
      }
    }
    expect(keys, hasLength(client.state.bodies.length));
  });

  test('a request with a body declares its content type', () {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    for (final FlowDetail detail in client.state.details.values) {
      final HttpRequest? request = detail.request;
      if (request == null || request.body.isEmpty) {
        continue;
      }
      expect(
        request.headers.any((Header header) => header.name == 'content-type'),
        isTrue,
        reason: '${detail.summary.id.value} sends a body',
      );
    }
  });
}
