// Der Hinweis auf Cloud-Inferenz in der Sitzung (HUM-516): das orangefarbene
// Tag oben in der Kopfzeile, im Token `outflow` und mit dem Namen des
// Anbieters, ohne Cloud-Endpunkt keines; vor dem ersten Start mit dieser
// Kombination aus Agent und Anbieter einmal die Frage, deren Bestätigung der
// Daemon hält, danach still.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/ui.dart';

import 'harness.dart';
import 'server_confirm_test.dart' show unconfirmedClient;

const CloudInference anthropic = CloudInference(
  agent: 'opencode',
  provider: 'api.anthropic.com',
);

SandboxTestClient cloudClient({bool acknowledged = false}) {
  final SandboxTestClient client = SandboxTestClient();
  client.sandbox = client.sandbox.copyWith(
    cloudInference: acknowledged ? anthropic.confirmed() : anthropic,
  );
  return client;
}

Future<void> settle(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 600));
}

void main() {
  testWidgets('the tag stands in the header, in the outflow colour, with the '
      'provider', (WidgetTester tester) async {
    await pumpSandbox(tester, client: cloudClient(acknowledged: true));

    final Finder tag = find.byKey(const Key('cloud-inference-tag'));
    expect(tag, findsOneWidget);
    expect(
      find.descendant(
        of: find.byKey(const Key('sandbox-header')),
        matching: tag,
      ),
      findsOneWidget,
      reason: 'at the top of the session, in its header',
    );
    final HBadge badge = tester.widget<HBadge>(tag);
    expect(badge.text, contains('api.anthropic.com'));
    final HTokens tokens = HTheme.of(tester.element(tag));
    expect(badge.color, tokens.colors.outflow, reason: 'the orange token');
    expect(badge.color, isNot(tokens.state.error), reason: 'never red');
    expect(badge.color, isNot(tokens.colors.warning), reason: 'not yellow');
    expect(badge.textColor, tokens.colors.outflowText);

    // Ein Klick zeigt den ganzen Text.
    await tester.tap(tag);
    await tester.pump();
    final Finder popover = find.byKey(const Key('cloud-inference-popover'));
    expect(popover, findsOneWidget);
    expect(
      find.descendant(
        of: popover,
        matching: find.textContaining('deliberately allowed'),
      ),
      findsOneWidget,
    );
  });

  testWidgets('without a cloud endpoint there is no tag', (
    WidgetTester tester,
  ) async {
    await pumpSandbox(tester, client: SandboxTestClient());
    expect(find.byKey(const Key('cloud-inference-tag')), findsNothing);
  });

  testWidgets('the first start asks, Cancel starts nothing, Confirm is kept by '
      'the daemon and starts', (WidgetTester tester) async {
    final SandboxTestClient client = cloudClient();
    await pumpSandbox(tester, client: client);

    await tester.tap(find.byKey(const Key('sandbox-start')));
    await settle(tester);
    expect(find.byKey(const Key('sandbox-cloud-confirm')), findsOneWidget);
    final String body = tester
        .widget<Text>(find.byKey(const Key('sandbox-cloud-confirm-body')))
        .data!;
    expect(body, contains('api.anthropic.com'));
    expect(client.startModes, isEmpty, reason: 'nothing started yet');

    await tester.tap(find.byKey(const Key('sandbox-cloud-confirm-cancel')));
    await settle(tester);
    expect(find.byKey(const Key('sandbox-cloud-confirm')), findsNothing);
    expect(client.startModes, isEmpty, reason: 'Cancel starts nothing');
    expect(client.cloudAcknowledgements, isEmpty);

    await tester.tap(find.byKey(const Key('sandbox-start')));
    await settle(tester);
    await tester.tap(find.byKey(const Key('sandbox-cloud-confirm-yes')));
    await settle(tester);
    expect(client.cloudAcknowledgements, <String>[
      'opencode api.anthropic.com',
    ]);
    expect(client.startModes, hasLength(1), reason: 'confirmed and started');
  });

  testWidgets('a confirmed combination starts without the question', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = cloudClient(acknowledged: true);
    await pumpSandbox(tester, client: client);

    await tester.tap(find.byKey(const Key('sandbox-start')));
    await settle(tester);
    expect(find.byKey(const Key('sandbox-cloud-confirm')), findsNothing);
    expect(client.startModes, hasLength(1));
    expect(client.cloudAcknowledgements, isEmpty);
  });

  testWidgets('another provider asks again', (WidgetTester tester) async {
    final SandboxTestClient client = cloudClient(acknowledged: true);
    client.sandbox = client.sandbox.copyWith(
      cloudInference: const CloudInference(
        agent: 'opencode',
        provider: 'api.openai.com',
      ),
    );
    await pumpSandbox(tester, client: client);

    await tester.tap(find.byKey(const Key('sandbox-start')));
    await settle(tester);
    expect(find.byKey(const Key('sandbox-cloud-confirm')), findsOneWidget);
    expect(client.startModes, isEmpty);
  });

  testWidgets('a long host at text scale 2 is cut with an ellipsis, never '
      'overflows the header, and stays whole in the tooltip', (
    WidgetTester tester,
  ) async {
    const String host =
        'inference-gateway.eu-central-1.very-long-provider-name.example.com';
    final SandboxTestClient client = runningClient();
    client.sandbox = client.sandbox.copyWith(
      backend: 'chosen-backend',
      isolationLevel: IsolationLevel.namespaces,
      workDirHost: '/home/me/src/app',
      cloudInference: const CloudInference(
        agent: 'opencode',
        provider: host,
        acknowledged: true,
      ),
    );
    await pumpSandbox(
      tester,
      client: client,
      textScaler: const TextScaler.linear(2),
    );
    expect(tester.takeException(), isNull, reason: 'no overflow');
    final Finder tag = find.byKey(const Key('cloud-inference-tag'));
    final HBadge badge = tester.widget<HBadge>(tag);
    expect(badge.overflow, TextOverflow.ellipsis);
    expect(badge.semanticsLabel, contains(host), reason: 'the whole host');
    final Size header = tester.getSize(find.byKey(const Key('sandbox-header')));
    expect(
      tester.getRect(tag).right,
      lessThanOrEqualTo(header.width),
      reason: 'the tag stays inside the header',
    );
    // Der Klick zeigt den ganzen Host.
    await tester.tap(tag);
    await tester.pump();
    expect(
      find.descendant(
        of: find.byKey(const Key('cloud-inference-popover')),
        matching: find.textContaining(host),
      ),
      findsWidgets,
    );
  });

  testWidgets('a refused server confirmation asks nothing about the cloud '
      '(review)', (WidgetTester tester) async {
    final SandboxTestClient client = unconfirmedClient();
    client.sandbox = client.sandbox.copyWith(cloudInference: anthropic);
    await pumpSandbox(tester, client: client);

    await tester.tap(find.byKey(const Key('sandbox-start')));
    await settle(tester);
    expect(find.byKey(const Key('sandbox-server-confirm')), findsOneWidget);
    client.llmServersFailure = const Diagnostic(
      code: 'CONFIG_021',
      severity: Severity.error,
      why: 'the server is gone',
    );
    await tester.tap(find.byKey(const Key('sandbox-server-confirm-yes')));
    await settle(tester);
    expect(find.byKey(const Key('sandbox-cloud-confirm')), findsNothing);
    expect(client.startModes, isEmpty);
  });

  testWidgets('a confirmed server leads to the cloud question', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = unconfirmedClient();
    client.sandbox = client.sandbox.copyWith(cloudInference: anthropic);
    await pumpSandbox(tester, client: client);

    await tester.tap(find.byKey(const Key('sandbox-start')));
    await settle(tester);
    await tester.tap(find.byKey(const Key('sandbox-server-confirm-yes')));
    await settle(tester);
    expect(find.byKey(const Key('sandbox-cloud-confirm')), findsOneWidget);
    expect(client.startModes, isEmpty);
  });
}
