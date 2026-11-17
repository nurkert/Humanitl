// Goldens des Hinweises auf Cloud-Inferenz (HUM-516), je in den vier
// Varianten aus `support/golden_variants.dart`: das orangefarbene Tag in der
// Kopfzeile einer laufenden Sitzung neben Backend und Server, dasselbe mit
// dem geöffneten ganzen Text, und die eine Frage vor dem ersten Start.
//
// Erneuern mit `make goldens-update`.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';

import '../features/sandbox/harness.dart';
import 'support/golden_variants.dart';

const CloudInference anthropic = CloudInference(
  agent: 'opencode',
  provider: 'api.anthropic.com',
);

/// Eine laufende Sitzung mit Backend, Stufe, gespeichertem Server und
/// bestätigter Cloud-Inferenz: alle Abzeichen der Kopfzeile nebeneinander.
SandboxTestClient runningCloud() {
  final SandboxTestClient client = runningClient();
  client.sandbox = client.sandbox.copyWith(
    backend: 'chosen-backend',
    isolationLevel: IsolationLevel.namespaces,
    workDirHost: '/home/me/src/app',
    llmServer: const LlmServerChoice(
      name: 'anthropic',
      source: LlmServerSource.active,
    ),
    cloudInference: anthropic.confirmed(),
  );
  return client;
}

/// Vor dem ersten Start mit dieser Kombination.
SandboxTestClient firstStart() {
  final SandboxTestClient client = SandboxTestClient();
  client.sandbox = client.sandbox.copyWith(
    workDirHost: '/home/me/src/app',
    cloudInference: anthropic,
  );
  return client;
}

Future<void> _settle(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 600));
}

Future<void> _openTag(WidgetTester tester) async {
  await _settle(tester);
  await tester.tap(find.byKey(const Key('cloud-inference-tag')));
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 600));
  expect(find.byKey(const Key('cloud-inference-popover')), findsOneWidget);
}

Future<void> _askAtStart(WidgetTester tester) async {
  await _settle(tester);
  await tester.tap(find.byKey(const Key('sandbox-start')));
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 600));
}

void main() {
  const BoxConstraints window = BoxConstraints.tightFor(
    width: 1280,
    height: 800,
  );

  goldenVariants(
    'sandbox_header_cloud',
    constraints: window,
    pumpBeforeTest: _settle,
    builder: (GoldenVariant variant) => sandboxUnderTest(
      client: runningCloud(),
      mode: variant.mode,
      locale: variant.locale,
    ),
  );

  goldenVariants(
    'sandbox_cloud_tag_open',
    constraints: window,
    pumpBeforeTest: _openTag,
    builder: (GoldenVariant variant) => sandboxUnderTest(
      client: runningCloud(),
      mode: variant.mode,
      locale: variant.locale,
    ),
  );

  goldenVariants(
    'sandbox_cloud_confirm',
    constraints: window,
    pumpBeforeTest: _askAtStart,
    builder: (GoldenVariant variant) => sandboxUnderTest(
      client: firstStart(),
      mode: variant.mode,
      locale: variant.locale,
    ),
  );
}
