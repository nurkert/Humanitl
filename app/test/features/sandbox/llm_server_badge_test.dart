// Der Modellserver der Sitzung im Kopf der Sandbox (HUM-400): Name und
// Herkunft stehen neben dem Profil, eine Wahl des Projekts fällt farblich
// auf und nennt im Hinweis die Projektdatei, und ein Name ohne gespeicherten
// Server sperrt den Start und sagt warum.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/hover_label.dart';
import 'package:humanitl/core/ui/ui.dart';

import 'harness.dart';

const String projectFile = '/home/me/src/app/.humanitl/profile.toml';

/// Der Befund, mit dem der Daemon einen nicht gespeicherten Server ablehnt.
const Diagnostic notSaved = Diagnostic(
  code: 'CONFIG_021',
  severity: Severity.error,
  title: 'Model servers not changed',
  why:
      'the project profile $projectFile names the model server gone in '
      'llm.server, and no server of that name is saved',
);

SandboxTestClient clientWith(LlmServerChoice? server) {
  final SandboxTestClient client = SandboxTestClient();
  client.sandbox = client.sandbox.copyWith(llmServer: server);
  return client;
}

Future<void> settle(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 600));
}

String badgeText(WidgetTester tester) =>
    tester.widget<HBadge>(find.byKey(const Key('sandbox-llm-server'))).text;

String hintOf(WidgetTester tester, Key key) => tester
    .widget<HoverLabel>(
      find.ancestor(of: find.byKey(key), matching: find.byType(HoverLabel)),
    )
    .label;

void main() {
  testWidgets('a server the project chose shows its name, the source and '
      'the project file', (WidgetTester tester) async {
    await pumpSandbox(
      tester,
      client: clientWith(
        const LlmServerChoice(
          name: 'work',
          source: LlmServerSource.project,
          detail: projectFile,
        ),
      ),
    );
    await settle(tester);

    expect(badgeText(tester), 'Model server work · from the project');
    final String hint = hintOf(tester, const Key('sandbox-llm-server'));
    expect(hint, contains(projectFile));
    expect(hint, contains('chooses the saved server work'));
    // The project may have chosen the active server itself; the hint does
    // not claim otherwise (HUM-400, review).
    expect(hint, isNot(contains('instead of the active')));
    final HTokens tokens = HTheme.of(
      tester.element(find.byKey(const Key('sandbox-llm-server'))),
    );
    expect(
      tester.widget<HBadge>(find.byKey(const Key('sandbox-llm-server'))).color,
      tokens.state.held,
      reason: 'a choice of the repository stands out',
    );
    expect(
      tester.widget<HButton>(find.byKey(const Key('sandbox-start'))).onPressed,
      isNotNull,
    );
  });

  testWidgets('the active server and a profile name their source', (
    WidgetTester tester,
  ) async {
    await pumpSandbox(
      tester,
      client: clientWith(
        const LlmServerChoice(name: 'home', source: LlmServerSource.active),
      ),
    );
    await settle(tester);
    expect(badgeText(tester), 'Model server home · default');

    await pumpSandbox(
      tester,
      client: clientWith(
        const LlmServerChoice(
          name: 'cloud',
          source: LlmServerSource.profile,
          detail: 'team',
        ),
      ),
    );
    await settle(tester);
    expect(badgeText(tester), 'Model server cloud · from profile team');
  });

  testWidgets('a server that is not saved blocks the start and says so', (
    WidgetTester tester,
  ) async {
    await pumpSandbox(
      tester,
      client: clientWith(
        const LlmServerChoice(
          name: 'gone',
          source: LlmServerSource.project,
          detail: projectFile,
          refusal: notSaved,
        ),
      ),
    );
    await settle(tester);

    expect(
      tester.widget<HButton>(find.byKey(const Key('sandbox-start'))).onPressed,
      isNull,
      reason: 'the daemon would refuse the start',
    );
    expect(
      hintOf(tester, const Key('sandbox-start')),
      startsWith('No saved server is called gone.'),
    );
    expect(
      hintOf(tester, const Key('sandbox-llm-server')),
      startsWith('No saved server is called gone.'),
    );
  });

  testWidgets('without a saved server the header shows none', (
    WidgetTester tester,
  ) async {
    await pumpSandbox(tester, client: clientWith(null));
    await settle(tester);
    expect(find.byKey(const Key('sandbox-llm-server')), findsNothing);
  });
}
