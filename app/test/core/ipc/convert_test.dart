// Tests der Übersetzung Proto ↔ Domäne (HUM-019). Der wichtigste Test ist
// der Namensabgleich: `convert.dart` bildet Enums nach Position ab und ist
// nur richtig, solange beide Seiten dieselbe Reihenfolge haben.

import 'package:fixnum/fixnum.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/convert.dart';
import 'package:humanitl/core/ipc/generated/humanitl/v1/common.pbenum.dart'
    as pb;
import 'package:humanitl/core/ipc/generated/humanitl/v1/humanitl.pb.dart' as pb;
import 'package:humanitl/core/ipc/generated/humanitl/v1/rules.pb.dart' as pb;
import 'package:protobuf/protobuf.dart' show ProtobufEnum;
import 'package:protobuf/well_known_types/google/protobuf/timestamp.pb.dart';

/// `FLOW_STATE_HELD` → `held`, `BLOCK_REASON_HOLD_MAX_FLOWS` → `holdMaxFlows`.
String camel(String wireName, String prefix) {
  final List<String> parts = wireName.substring(prefix.length).split('_');
  return parts.first.toLowerCase() +
      parts
          .skip(1)
          .map(
            (String p) =>
                p.substring(0, 1).toUpperCase() + p.substring(1).toLowerCase(),
          )
          .join();
}

void checkParity<T extends Enum>(
  List<ProtobufEnum> wire,
  List<T> domain,
  String prefix,
) {
  final List<String> wireNames = wire
      .where((ProtobufEnum e) => e.value != 0)
      .map((ProtobufEnum e) => camel(e.name, prefix))
      .toList();
  expect(wireNames, domain.map((T e) => e.name).toList(), reason: prefix);
  for (final ProtobufEnum e in wire) {
    expect(
      enumFromWire(domain, e.value)?.name,
      e.value == 0 ? isNull : camel(e.name, prefix),
    );
  }
  expect(enumFromWire(domain, 9999), isNull);
  for (final T member in domain) {
    expect(enumToWire(member), member.index + 1);
  }
}

void main() {
  test('every domain enum lists the wire members in wire order', () {
    checkParity(pb.FlowState.values, FlowState.values, 'FLOW_STATE_');
    checkParity(pb.DecisionKind.values, DecisionKind.values, 'DECISION_KIND_');
    checkParity(pb.BlockReason.values, BlockReason.values, 'BLOCK_REASON_');
    checkParity(
      pb.DecisionSource.values,
      DecisionSource.values,
      'DECISION_SOURCE_',
    );
    checkParity(
      pb.UpstreamError.values,
      UpstreamError.values,
      'UPSTREAM_ERROR_',
    );
    checkParity(pb.Method.values, Method.values, 'METHOD_');
    checkParity(pb.Scheme.values, Scheme.values, 'SCHEME_');
    checkParity(pb.Upgrade.values, Upgrade.values, 'UPGRADE_');
    checkParity(pb.Severity.values, Severity.values, 'SEVERITY_');
    checkParity(pb.RuleAction.values, RuleAction.values, 'RULE_ACTION_');
    checkParity(pb.FindingTier.values, FindingTier.values, 'FINDING_TIER_');
    checkParity(
      pb.FindingLocation.values,
      FindingLocation.values,
      'FINDING_LOCATION_',
    );
    checkParity(pb.SandboxState.values, SandboxState.values, 'SANDBOX_STATE_');
    checkParity(pb.MountMode.values, MountMode.values, 'MOUNT_MODE_');
    checkParity(pb.ValueOrigin.values, ValueOrigin.values, 'VALUE_ORIGIN_');
  });

  test('Info becomes DaemonInfo', () {
    final DaemonInfo info =
        (pb.Info()
              ..daemonVersion = '0.1.0'
              ..protoMajor = 1
              ..protoMinor = 3
              ..capabilities.addAll(<String>['fake', 'proxy.h1'])
              ..sessionId = 'abc')
            .toDomain();
    expect(info.daemonVersion, '0.1.0');
    expect(info.protoVersion, '1.3');
    expect(info.isFake, isTrue);
    expect(info.hasSession, isTrue);
  });

  test('FlowEvent.received carries the summary and the deadline', () {
    final DateTime deadline = DateTime.utc(2026, 9, 3, 10, 5);
    final pb.FlowEvent event = pb.FlowEvent()
      ..at = Timestamp.fromDateTime(DateTime.utc(2026, 9, 3, 10))
      ..received = (pb.FlowEvent_Received()
        ..summary = (pb.FlowSummary()
          ..flowId = '018f0000-0000-7000-8000-000000000002'
          ..sessionId = '018f0000-0000-7000-8000-000000000001'
          ..method = pb.Method.METHOD_POST
          ..scheme = pb.Scheme.SCHEME_HTTPS
          ..authority = (pb.Authority()
            ..host = 'xn--mnchen-3ya.example'
            ..port = 443
            ..displayHost = 'münchen.example')
          ..path = '/graphql'
          ..state = pb.FlowState.FLOW_STATE_HELD
          ..requestSize = Int64(306)
          ..deadline = Timestamp.fromDateTime(deadline)
          ..originTool = 'opencode'));

    final FlowEvent? domain = event.toDomain();

    expect(domain, isA<FlowEventReceived>());
    final Flow flow = (domain! as FlowEventReceived).flow;
    expect(flow.id, const FlowId('018f0000-0000-7000-8000-000000000002'));
    expect(flow.method, Method.post);
    expect(flow.methodLabel, 'POST');
    expect(flow.host, 'münchen.example');
    expect(flow.authority.display(Scheme.https), 'münchen.example');
    expect(flow.state, FlowState.held);
    expect(flow.isHeld, isTrue);
    expect(flow.decision, isNull);
    expect(flow.deadline?.toUtc(), deadline);
    expect(flow.requestSize, 306);
    expect(domain.flowId, flow.id);
  });

  test('an unset event is null, an unknown method keeps its raw token', () {
    expect(pb.FlowEvent().toDomain(), isNull);
    final Flow flow =
        (pb.FlowSummary()
              ..method = pb.Method.METHOD_OTHER
              ..methodRaw = 'propfind')
            .toDomain();
    expect(flow.method, Method.other);
    expect(flow.methodLabel, 'PROPFIND');
  });

  test('Decision becomes a DecideRequest', () {
    const FlowId id = FlowId('018f0000-0000-7000-8000-000000000004');
    expect(
      const Decision.allow().toProto(id).whichDecision(),
      pb.DecideRequest_Decision.allow,
    );
    final pb.DecideRequest block = const Decision.block(note: 'use PyPI')
        .toProto(id);
    expect(block.whichDecision(), pb.DecideRequest_Decision.block);
    expect(block.block.note, 'use PyPI');
    expect(block.flowIds, <String>[id.value]);
    final pb.DecideRequest edited = const Decision.allowEdited(
      request: EditedRequest(
        method: Method.post,
        url: 'https://api.github.com/repos',
        headers: <Header>[
          Header(name: 'x', value: <int>[1]),
        ],
        body: <int>[0, 255],
      ),
    ).toProto(id);
    expect(edited.allowEdited.body, <int>[0, 255]);
    expect(edited.allowEdited.method, pb.Method.METHOD_POST);
    expect(
      () => const Decision.timedOut().toProto(id),
      throwsA(isA<ArgumentError>()),
    );
  });

  test('a rule survives the round trip', () {
    final Rule rule = Rule(
      id: const RuleId('018f0000-0000-7000-8000-0000000000a1'),
      action: RuleAction.allow,
      matcher: const RuleMatcher(
        host: '**.npmjs.org',
        methods: <Method>[Method.get, Method.head],
        path: '/**',
        scheme: Scheme.https,
        port: 443,
        upgrade: Upgrade.none,
      ),
      expires: RuleExpiry.at(at: DateTime.utc(2026, 9, 3, 12)),
      createdFrom: const FlowId('018f0000-0000-7000-8000-000000000002'),
      note: 'npm install',
      createdAt: DateTime.utc(2026, 9, 3, 11),
      position: 2,
      hitCount: 14,
      allowPrivate: true,
    );
    final Rule back = rule.toProto().toDomain();
    // Zeitstempel kommen lokal zurück; verglichen wird der Zeitpunkt.
    expect(
      back.copyWith(expires: rule.expires, createdAt: rule.createdAt),
      rule,
    );
    expect(
      (back.expires as RuleExpiryAt).at.toUtc(),
      DateTime.utc(2026, 9, 3, 12),
    );
    expect(back.createdAt?.toUtc(), DateTime.utc(2026, 9, 3, 11));
    expect(back.createdAt?.isUtc, isFalse);
    final Rule remembered = const Decision.allow()
        .toProto(const FlowId('x'), remember: rule)
        .remember
        .toDomain();
    expect(remembered.matcher.host, '**.npmjs.org');
    expect(
      (pb.Rule()..expires = pb.RuleExpiry()).toDomain().expires,
      const RuleExpiry.session(),
    );
  });

  test('rule_disabled_survives_the_round_trip', () {
    // Lesen: eine Proto, die das Feld trägt, kommt als abgeschaltete Regel an.
    expect((pb.Rule()..disabled = true).toDomain().disabled, isTrue);
    expect(pb.Rule().toDomain().disabled, isFalse);

    // Schreiben: `toProto` gibt das Feld weiter. Ohne diese Zeile verlöre
    // jede Regel, die einmal durch den Konverter läuft -- Probelauf, Editor --
    // ihren Zustand.
    final Rule off = Rule(
      id: const RuleId('018f0000-0000-7000-8000-0000000000a1'),
      action: RuleAction.block,
      matcher: const RuleMatcher(host: 'models.dev'),
      expires: const RuleExpiry.never(),
      bundled: true,
      disabled: true,
    );
    expect(off.toProto().disabled, isTrue);
    expect(off.toProto().toDomain().disabled, isTrue);
    expect(off.copyWith(disabled: false).toProto().disabled, isFalse);
  });

  test('a diagnostic with a fix and a docs link converts', () {
    final Diagnostic diagnostic =
        (pb.Diagnostic()
              ..code = 'TLS_001'
              ..severity = pb.Severity.SEVERITY_WARNING
              ..why = 'curl does not trust the CA'
              ..fix = (pb.FixAction()
                ..setEnv = (pb.FixAction_SetEnv()
                  ..key = 'CURL_CA_BUNDLE'
                  ..value = '/etc/humanitl/ca.crt'))
              ..docsUrl = 'https://example.invalid/#tls_001')
            .toDomain();
    expect(diagnostic.code, 'TLS_001');
    expect(diagnostic.area, 'tls');
    expect(diagnostic.isFailure, isFalse);
    expect(
      diagnostic.fix,
      const FixAction.setEnv(
        key: 'CURL_CA_BUNDLE',
        value: '/etc/humanitl/ca.crt',
      ),
    );
    expect(pb.Diagnostic().toDomain().fix, isNull);
  });

  test('FlowFilter becomes a ListFlowsRequest', () {
    final pb.ListFlowsRequest request = const FlowFilter(
      query: 'host:github.com',
      since: FlowId('018f'),
      includePassthrough: true,
    ).toProto(limit: 50, cursor: 'c1');
    expect(request.filter, 'host:github.com');
    expect(request.sinceFlowId, '018f');
    expect(request.limit, 50);
    expect(request.cursor, 'c1');
    expect(request.includePassthrough, isTrue);
  });
}
