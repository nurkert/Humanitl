// Der Weg des Befunds HUM-205, gemessen statt nachgestellt: Eine Regel mit
// Pfadpräfixen kommt über den echten gRPC-Client aus `list`, der
// Regel-Bildschirm macht sie dauerhaft oder ändert sie, und die Anfrage, die
// beim Daemon ankommt, trägt dieselben Präfixe. Vorher kam dort eine leere
// Liste an, und der Daemon liest die als „jeder Pfad": aus
// `allow api.example.com` unter `/v1/` wurde `allow api.example.com` für
// alles.
//
// Der Server ist ein Dienst im selben Prozess auf einem Unix-Socket. Er
// kennt nur `Rules`, merkt sich jede Anfrage und antwortet mit dem Satz, den
// er hält; alles andere an der Schnittstelle bleibt unberührt.

import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:grpc/grpc.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ipc/generated/humanitl/v1/humanitl.pbgrpc.dart'
    as pb;
import 'package:humanitl/core/ipc/generated/humanitl/v1/rules.pb.dart' as pb;
import 'package:humanitl/core/ipc/grpc_daemon_client.dart';
import 'package:humanitl/features/rules/providers/rules.dart';
import 'package:protobuf/well_known_types/google/protobuf/timestamp.pb.dart';

const String ruleId = '018f0000-0000-7000-8000-0000000002a5';
const List<String> prefixes = <String>['/v1/', '/v2/admin'];

/// Ein Daemon, der nur Regeln kennt.
class RulesOnlyService extends pb.HumanitlServiceBase {
  RulesOnlyService(this.rule);

  /// Die eine Regel, die er hält; `update` ersetzt sie.
  pb.Rule rule;

  /// Jede Anfrage an `Rules`, in der Reihenfolge ihres Eintreffens.
  final List<pb.RulesRequest> requests = <pb.RulesRequest>[];

  @override
  Future<pb.RulesResponse> rules(
    ServiceCall call,
    pb.RulesRequest request,
  ) async {
    requests.add(request);
    if (request.hasUpdate()) {
      rule = request.update;
    }
    return pb.RulesResponse()..rules.add(rule);
  }

  // Die übrigen Methoden der Schnittstelle ruft dieser Test nie.
  @override
  dynamic noSuchMethod(Invocation invocation) =>
      throw UnimplementedError('${invocation.memberName}');
}

/// Eine eigene, dauerhafte Regel mit Frist und zwei Präfixen, wie sie
/// `rules.yaml` des Nutzers halten kann.
pb.Rule narrowedRule() {
  final pb.Rule rule = pb.Rule()
    ..ruleId = ruleId
    ..action = pb.RuleAction.RULE_ACTION_ALLOW
    ..matcher = (pb.RuleMatcher()..host = 'api.example.com')
    ..expires = (pb.RuleExpiry()
      ..at = Timestamp.fromDateTime(DateTime.utc(2099, 1, 1)))
    ..position = 1;
  rule.matcher.pathPrefixes.addAll(prefixes);
  return rule;
}

void main() {
  late Directory dir;
  late Server server;
  late RulesOnlyService service;
  late ProviderContainer container;

  setUp(() async {
    dir = Directory.systemTemp.createTempSync('humanitl-hum205-');
    File('${dir.path}/token').writeAsStringSync('secret\n');
    // Wie der Daemon: Verzeichnis 0700 und Token 0600, sonst traut der
    // Client ihnen nicht (HUM-212).
    Process.runSync('chmod', <String>['700', dir.path]);
    Process.runSync('chmod', <String>['600', '${dir.path}/token']);
    service = RulesOnlyService(narrowedRule());
    server = Server.create(services: <Service>[service]);
    await server.serve(
      address: InternetAddress(
        '${dir.path}/daemon.sock',
        type: InternetAddressType.unix,
      ),
      port: 0,
    );
    final GrpcDaemonClient client = GrpcDaemonClient(
      socketPath: '${dir.path}/daemon.sock',
      tokenPath: '${dir.path}/token',
      callTimeout: const Duration(seconds: 5),
    );
    container = ProviderContainer(
      overrides: <Override>[daemonClientProvider.overrideWithValue(client)],
    );
  });

  tearDown(() async {
    container.dispose();
    await server.shutdown();
    dir.deleteSync(recursive: true);
  });

  Future<Rule> listed() async {
    final RuleSet set = await container.read(rulesProvider.future);
    final Rule rule = set.rules.single;
    expect(rule.matcher.pathPrefixes, prefixes, reason: 'list reads them');
    return rule;
  }

  test('making a rule permanent keeps its path prefixes', () async {
    final Rule rule = await listed();
    final Diagnostic? failed = await container
        .read(rulesProvider.notifier)
        .makePermanent(rule);
    expect(failed, isNull);
    final pb.RulesRequest sent = service.requests.last;
    expect(sent.hasUpdate(), isTrue);
    expect(sent.update.expires.hasNever(), isTrue);
    expect(sent.update.matcher.pathPrefixes, prefixes);
    // Der Satz, den der Bildschirm danach zeigt, ist wieder der enge.
    final RuleSet after = await container.read(rulesProvider.future);
    expect(after.rules.single.matcher.pathPrefixes, prefixes);
  });

  test('changing a rule in the editor keeps its path prefixes', () async {
    final Rule rule = await listed();
    final Diagnostic? failed = await container
        .read(rulesProvider.notifier)
        .change(
          rule.copyWith(
            matcher: rule.matcher.copyWith(methods: <Method>[Method.get]),
          ),
        );
    expect(failed, isNull);
    expect(service.requests.last.update.matcher.pathPrefixes, prefixes);
  });
}
