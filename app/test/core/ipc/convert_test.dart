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
import 'package:protobuf/protobuf.dart'
    show FieldInfo, GeneratedMessage, ProtobufEnum;
import 'package:protobuf/well_known_types/google/protobuf/empty.pb.dart';
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

  test('a row carries the apex, and a missing field is the empty string', () {
    final Flow known =
        (pb.FlowSummary()
              ..flowId = '018f0000-0000-7000-8000-000000000002'
              ..authority = (pb.Authority()..host = 'a.b.github.io')
              ..apex = 'b.github.io')
            .toDomain();
    expect(known.apex, 'b.github.io');

    // Ein Daemon, der nichts sagt, sagt nicht „unbedenklich": Das Feld bleibt
    // leer, und der Client leitet nichts aus dem Host ab (HUM-091).
    final Flow unknown =
        (pb.FlowSummary()
              ..flowId = '018f0000-0000-7000-8000-000000000003'
              ..authority = (pb.Authority()..host = 'a.b.github.io'))
            .toDomain();
    expect(unknown.apex, isEmpty);
  });

  test('a row carries the catalog id, and a missing field is empty', () {
    final Flow known =
        (pb.FlowSummary()
              ..flowId = '018f0000-0000-7000-8000-000000000005'
              ..authority = (pb.Authority()..host = 'registry.npmjs.org')
              ..apex = 'npmjs.org'
              ..catalogId = 'npm')
            .toDomain();
    expect(known.catalogId, 'npm');

    // Ein Daemon, der keinen Dienst kennt, sagt nicht „unbedenklich": Das Feld
    // bleibt leer, und der Client schlägt nichts aus dem Host nach (HUM-094).
    final Flow unknown =
        (pb.FlowSummary()
              ..flowId = '018f0000-0000-7000-8000-000000000006'
              ..authority = (pb.Authority()..host = 'evil.example'))
            .toDomain();
    expect(unknown.catalogId, isEmpty);
  });

  test(
    'a row carries the note of a block, and no note is the empty string',
    () {
      final Flow blocked =
          (pb.FlowSummary()
                ..flowId = '018f0000-0000-7000-8000-000000000004'
                ..authority = (pb.Authority()..host = 'api.github.com')
                ..decision = pb.DecisionKind.DECISION_KIND_BLOCK
                ..blockReason = pb.BlockReason.BLOCK_REASON_USER
                ..decisionNote = 'use PyPI')
              .toDomain();
      expect(blocked.decisionNote, 'use PyPI');

      // Kein Feld heißt: zu dieser Entscheidung gibt es keine Notiz. Der Client
      // erfindet keine und säubert keine nach (HUM-117).
      final Flow allowed =
          (pb.FlowSummary()
                ..flowId = '018f0000-0000-7000-8000-000000000005'
                ..authority = (pb.Authority()..host = 'api.github.com')
                ..decision = pb.DecisionKind.DECISION_KIND_ALLOW)
              .toDomain();
      expect(allowed.decisionNote, isEmpty);
    },
  );

  test('the trail of open findings travels both ways (HUM-160)', () {
    const FlowId id = FlowId('018f0000-0000-7000-8000-000000000006');
    expect(
      const Decision.allow(acknowledgedFindings: <int>[0, 2])
          .toProto(id)
          .acknowledgedFindings,
      <int>[0, 2],
    );
    expect(const Decision.allow().toProto(id).acknowledgedFindings, isEmpty);

    final Flow counted =
        (pb.FlowSummary()
              ..flowId = id.value
              ..authority = (pb.Authority()..host = 'api.example.com')
              ..unresolvedFindings = 0)
            .toDomain();
    expect(counted.unresolvedFindings, 0, reason: 'a zero is a count');
    final Flow uncounted =
        (pb.FlowSummary()
              ..flowId = id.value
              ..authority = (pb.Authority()..host = 'api.example.com'))
            .toDomain();
    expect(uncounted.unresolvedFindings, isNull, reason: 'absent is not zero');

    final FlowEvent? decided =
        (pb.FlowEvent()
              ..decided = (pb.FlowEvent_Decided()
                ..flowId = id.value
                ..kind = pb.DecisionKind.DECISION_KIND_ALLOW
                ..unresolvedFindings = 1))
            .toDomain();
    expect((decided as FlowEventDecided).unresolvedFindings, 1);
  });

  test('the hard block travels in the row (HUM-159)', () {
    final Flow locked =
        (pb.FlowSummary()
              ..flowId = 'f'
              ..authority = (pb.Authority()..host = 'bank.example.com')
              ..sendRefusal = (pb.Diagnostic()
                ..code = 'HOLD_004'
                ..why = 'the request carries a checksum-confirmed iban'))
            .toDomain();
    expect(locked.sendRefusal?.code, 'HOLD_004');
    expect(
      locked.sendRefusal?.why,
      'the request carries a checksum-confirmed iban',
    );
    final Flow open =
        (pb.FlowSummary()
              ..flowId = 'g'
              ..authority = (pb.Authority()..host = 'bank.example.com'))
            .toDomain();
    expect(open.sendRefusal, isNull, reason: 'absent is no lock');
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

  test('flow_diagnostic_carries_the_flow_id', () {
    // Feld 16 nennt den Fluss, zu dem der Befund gehört; Feld 12 nennt keinen.
    // Fällt die Kennung hier weg, kann keine Ansicht den Befund mehr an die
    // Anfrage hängen, die gerade gescheitert ist. Rot, sobald `convert.dart`
    // `flowId` wieder weglässt.
    final pb.Diagnostic refused = pb.Diagnostic()
      ..code = 'TLS_001'
      ..severity = pb.Severity.SEVERITY_WARNING
      ..why = 'curl in the sandbox does not trust the Humanitl CA yet';
    final FlowEvent? event =
        (pb.FlowEvent()
              ..flowDiagnostic = (pb.FlowEvent_FlowDiagnostic()
                ..flowId = '018f0001-0000-7000-8000-000000060000'
                ..diagnostic = refused))
            .toDomain();
    expect(event, isA<FlowEventDiagnostic>());
    final FlowEventDiagnostic withFlow = event! as FlowEventDiagnostic;
    expect(
      withFlow.flowId,
      const FlowId('018f0001-0000-7000-8000-000000060000'),
    );
    // Und über den gemeinsamen Getter, den jeder Aufrufer benutzt, der nur
    // ein `FlowEvent` in der Hand hat.
    expect(
      (withFlow as FlowEvent).flowId,
      const FlowId('018f0001-0000-7000-8000-000000060000'),
    );
    expect(withFlow.diagnostic.code, 'TLS_001');

    final FlowEvent? plain = (pb.FlowEvent()..diagnostic = refused).toDomain();
    expect(plain, isA<FlowEventDiagnostic>());
    final FlowEventDiagnostic sessionWide = plain! as FlowEventDiagnostic;
    expect(sessionWide.flowId, isNull);
    expect((sessionWide as FlowEvent).flowId, isNull);

    // Ein leeres Feld 16 ist keine Kennung. Die ganze Unterscheidung „gehört
    // zu einem Fluss" gegen „gehört zur Sitzung" läuft über null; ein
    // `FlowId('')` wäre ein Fluss, den es nicht gibt.
    // Rot, sobald der Zweig wieder bedingungslos `FlowId(...)` schreibt.
    final FlowEvent? empty =
        (pb.FlowEvent()
              ..flowDiagnostic = (pb.FlowEvent_FlowDiagnostic()
                ..flowId = ''
                ..diagnostic = refused))
            .toDomain();
    expect((empty! as FlowEventDiagnostic).flowId, isNull);
    expect(empty.flowId, isNull);
  });

  // Der Hinweis des Daemons erreicht den Emulator nie (HUM-042). Er kommt als
  // eigener Rahmen, und diese Seite hat eine eigene Fläche dafür: den Streifen
  // über dem Terminal. Ginge er als Bild-Frame durch, stünde die Zeile quer
  // über dem Bild eines Vollbild-TUI -- so war es bis zum 2026-09-07, und ein
  // Mensch vor dem Bildschirm hat es gemeldet.
  test('a notice of the daemon never becomes a terminal frame', () {
    expect(
      (pb.TerminalOutput()
            ..notice = '[humanitl] request held: GET example.com/ · waiting')
          .toDomain(),
      isNull,
    );
    // Und die Bytes daneben kommen weiterhin an, sonst wäre oben nur alles
    // still.
    expect(
      (pb.TerminalOutput()..data = <int>[104, 105]).toDomain(),
      isA<TerminalOutput>(),
    );
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

  group('HUM-205: nothing of a rule is lost on the way', () {
    /// Die Feldnamen einer Proto-Nachricht, wie `protoc-gen-dart` sie nennt.
    Set<String> wireFields(GeneratedMessage message) => message
        .info_
        .fieldInfo
        .values
        .map((FieldInfo<Object?> f) => f.name)
        .toSet();

    test('rule_matcher_names_every_wire_field', () {
      // Voll besetzt, damit `toJson` jeden Schlüssel schreibt.
      const RuleMatcher matcher = RuleMatcher(
        host: 'api.example.com',
        methods: <Method>[Method.get],
        path: '/**',
        scheme: Scheme.https,
        port: 443,
        upgrade: Upgrade.none,
        pathPrefixes: <String>['/v1/'],
      );
      expect(matcher.toJson().keys.toSet(), wireFields(pb.RuleMatcher()));
    });

    test('rule_names_every_wire_field', () {
      final Rule rule = Rule(
        id: const RuleId('018f0000-0000-7000-8000-0000000000a1'),
        action: RuleAction.allow,
        matcher: const RuleMatcher(host: 'api.example.com'),
        createdFrom: const FlowId('018f0000-0000-7000-8000-000000000002'),
        note: 'n',
        createdAt: DateTime.utc(2026, 9, 23),
      );
      // Zwei Namen weichen ab, weil die Domäne Id-Typen trägt.
      const Map<String, String> renamed = <String, String>{
        'id': 'ruleId',
        'createdFrom': 'createdFromFlowId',
      };
      expect(
        rule.toJson().keys.map((String k) => renamed[k] ?? k).toSet(),
        wireFields(pb.Rule()),
      );
    });

    // Die Namensgleichheit oben schützt keine Zuweisung im Konverter. Dieser
    // Rundlauf tut es: jedes Feld trägt einen Wert, der nicht der Vorgabe
    // entspricht, also fällt jede fehlende Zuweisung in `toDomain` oder
    // `toProto` als Unterschied auf.
    pb.RuleMatcher fullMatcher() {
      final pb.RuleMatcher m = pb.RuleMatcher()
        ..host = '**.example.com'
        ..path = '/api/**'
        ..scheme = pb.Scheme.SCHEME_WSS
        ..port = 8443
        ..upgrade = pb.Upgrade.UPGRADE_WEBSOCKET;
      m.methods.addAll(<pb.Method>[
        pb.Method.METHOD_GET,
        pb.Method.METHOD_POST,
      ]);
      m.pathPrefixes.addAll(<String>['/v1/', '/v2/admin']);
      return m;
    }

    test('a fully populated matcher survives toDomain then toProto', () {
      final pb.RuleMatcher p = fullMatcher();
      expect(p.toDomain().toProto(), p);
    });

    test('a fully populated rule survives toDomain then toProto', () {
      final List<pb.RuleExpiry> expiries = <pb.RuleExpiry>[
        pb.RuleExpiry()..never = Empty(),
        pb.RuleExpiry()..session = Empty(),
        pb.RuleExpiry()
          ..at = Timestamp.fromDateTime(DateTime.utc(2099, 1, 2, 3, 4, 5)),
      ];
      for (final pb.RuleExpiry expiry in expiries) {
        final pb.Rule p = pb.Rule()
          ..ruleId = '018f0000-0000-7000-8000-0000000002a5'
          ..action = pb.RuleAction.RULE_ACTION_ALLOW
          ..matcher = fullMatcher()
          ..expires = expiry
          ..stream = true
          ..createdFromFlowId = '018f0000-0000-7000-8000-000000000002'
          ..bundled = true
          ..note = 'npm install'
          ..createdAt = Timestamp.fromDateTime(DateTime.utc(2026, 9, 23, 11))
          ..position = 3
          ..hitCount = Int64(14)
          ..allowPrivate = true
          ..disabled = true
          ..passthroughLlm = true;
        expect(p.toDomain().toProto(), p, reason: expiry.whichExpiry().name);
      }
    });

    test('path_prefixes_survive_list_then_make_permanent', () {
      // Wie `list` sie liefert: eine Regel mit Frist und zwei Präfixen.
      final pb.Rule listed = pb.Rule()
        ..ruleId = '018f0000-0000-7000-8000-0000000002a5'
        ..action = pb.RuleAction.RULE_ACTION_ALLOW
        ..matcher = (pb.RuleMatcher()..host = 'api.example.com')
        ..expires = (pb.RuleExpiry()
          ..at = Timestamp.fromDateTime(DateTime.utc(2099)))
        ..passthroughLlm = true;
      listed.matcher.pathPrefixes.addAll(<String>['/v1/', '/v2/admin']);

      final Rule rule = listed.toDomain();
      expect(rule.matcher.pathPrefixes, <String>['/v1/', '/v2/admin']);
      expect(rule.passthroughLlm, isTrue);

      // „Dauerhaft machen" bei einer Frist schickt `update` mit `never`.
      final pb.Rule sent = rule
          .copyWith(expires: const RuleExpiry.never())
          .toProto();
      expect(sent.matcher.pathPrefixes, <String>['/v1/', '/v2/admin']);
      expect(sent.passthroughLlm, isTrue);
    });

    test('a proposed rule in a fix keeps its path prefix', () {
      final pb.Rule proposed = pb.Rule()
        ..action = pb.RuleAction.RULE_ACTION_ALLOW
        ..matcher = (pb.RuleMatcher()..host = 'api.example.com');
      proposed.matcher.pathPrefixes.add('/v1/');
      final FixAction? fix = (pb.FixAction()..addRule = proposed).toDomain();
      expect((fix! as FixActionAddRule).rule.matcher.pathPrefixes, <String>[
        '/v1/',
      ]);
    });
  });

  // HUM-138: der Stand der verweigerten Versuche, aus der Momentaufnahme und
  // als eigenes Ereignis.
  test('refused sockets reach the domain with count, reason and time', () {
    final pb.SandboxEvent_Refusals wire = pb.SandboxEvent_Refusals(
      reporting: pb.RefusalReporting.REFUSAL_REPORTING_ON,
      entries: <pb.SandboxEvent_Refusal>[
        pb.SandboxEvent_Refusal(
          syscall: 'socket',
          family: 'AF_UNIX',
          socketType: 'SOCK_STREAM',
          reason: pb.RefusalReason.REFUSAL_REASON_FAMILY,
          count: Int64(3),
          lastAt: Timestamp(seconds: Int64(1_790_000_000)),
        ),
        pb.SandboxEvent_Refusal(
          family: 'AF_INET',
          socketType: 'SOCK_DGRAM',
          reason: pb.RefusalReason.REFUSAL_REASON_TYPE,
          count: Int64(2),
        ),
      ],
    );
    final SandboxStatus status = pb.SandboxEvent_Status(
      state: pb.SandboxState.SANDBOX_STATE_RUNNING,
      refusals: wire,
    ).toDomain();
    final SandboxRefusals? refusals = status.refusals;
    expect(refusals, isNotNull);
    expect(refusals!.reporting, SandboxRefusalReporting.on);
    expect(refusals.total, 5);
    expect(refusals.entries.first.family, 'AF_UNIX');
    expect(refusals.entries.first.reason, SandboxRefusalReason.family);
    expect(
      refusals.entries.first.lastAt!.toUtc(),
      DateTime.fromMillisecondsSinceEpoch(1_790_000_000_000, isUtc: true),
    );
    expect(refusals.entries.last.reason, SandboxRefusalReason.type);
    expect(refusals.entries.last.lastAt, isNull);

    // Without the field the snapshot says nothing, not "nothing refused".
    expect(
      pb.SandboxEvent_Status(state: pb.SandboxState.SANDBOX_STATE_RUNNING)
          .toDomain()
          .refusals,
      isNull,
    );
    expect(
      pb.SandboxEvent_Refusals(
        reporting: pb.RefusalReporting.REFUSAL_REPORTING_OFF,
        offReason: 'errno16',
      ).toDomain(),
      const SandboxRefusals(
        reporting: SandboxRefusalReporting.off,
        offReason: 'errno16',
      ),
    );
  });
}
