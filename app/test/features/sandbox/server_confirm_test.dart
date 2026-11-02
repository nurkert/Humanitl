// Die eine Rückfrage vor einem Start, wenn das Projekt einen anderen
// gespeicherten Server als den Standard wählt und ihn für diesen Ordner noch
// niemand bestätigt hat (HUM-405). Abbrechen ist vorgewählt und startet
// nichts; Bestätigen geht über `LlmServers(confirm_project)` und startet.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';

import 'harness.dart';

const String project = '/home/me/src/app';

/// Der Befund, mit dem der Daemon den Start ablehnt, bis bestätigt ist.
const Diagnostic unconfirmed = Diagnostic(
  code: 'CONFIG_022',
  severity: Severity.error,
  why:
      'the project $project chooses the saved model server work in '
      'llm.server, not the default server home',
);

SandboxTestClient unconfirmedClient() {
  final SandboxTestClient client = SandboxTestClient()
    ..savedServers = const <SavedServer>[
      SavedServer(name: 'home', endpoint: 'http://home.lan:11434/'),
      SavedServer(name: 'work', endpoint: 'http://work.lan:8080/v1'),
    ]
    ..activeServer = 'home';
  client.sandbox = client.sandbox.copyWith(
    llmServer: const LlmServerChoice(
      name: 'work',
      source: LlmServerSource.project,
      detail: '$project/.humanitl/profile.toml',
      unconfirmed: unconfirmed,
      projectDir: project,
    ),
  );
  return client;
}

Future<void> settle(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 600));
}

void main() {
  testWidgets('the start asks once, Cancel is preselected and starts nothing', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = unconfirmedClient();
    await pumpSandbox(tester, client: client);

    await tester.tap(find.byKey(const Key('sandbox-start')));
    await settle(tester);
    expect(find.byKey(const Key('sandbox-server-confirm')), findsOneWidget);
    expect(
      find.textContaining('Use the server work for this project?'),
      findsOneWidget,
    );
    final String body = tester
        .widget<Text>(find.byKey(const Key('sandbox-server-confirm-body')))
        .data!;
    expect(body, contains(project));
    expect(body, contains('home'));
    expect(client.startModes, isEmpty, reason: 'nothing started yet');

    // Der Fokus steht auf Abbrechen; Enter bricht ab.
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await settle(tester);
    expect(find.byKey(const Key('sandbox-server-confirm')), findsNothing);
    expect(client.startModes, isEmpty);
    expect(client.confirmedProjects, isEmpty);
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('confirming stores the confirmation and starts', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = unconfirmedClient();
    await pumpSandbox(tester, client: client);

    await tester.tap(find.byKey(const Key('sandbox-start')));
    await settle(tester);
    await tester.tap(find.byKey(const Key('sandbox-server-confirm-yes')));
    await settle(tester);

    expect(client.confirmedProjects, <String>['$project work']);
    expect(client.startModes, hasLength(1), reason: 'one start after it');
    expect(find.byKey(const Key('sandbox-server-confirm')), findsNothing);
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('a confirmed or default server starts without a question', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = unconfirmedClient();
    client.sandbox = client.sandbox.copyWith(
      llmServer: const LlmServerChoice(
        name: 'work',
        source: LlmServerSource.project,
        projectDir: project,
      ),
    );
    await pumpSandbox(tester, client: client);
    await tester.tap(find.byKey(const Key('sandbox-start')));
    await settle(tester);
    expect(find.byKey(const Key('sandbox-server-confirm')), findsNothing);
    expect(client.startModes, hasLength(1));
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  test('the fake refuses an unconfirmed start like the daemon', () async {
    final SandboxTestClient client = unconfirmedClient();
    final List<SandboxUpdate> seen = await client.startSandbox().toList();
    expect(
      seen.whereType<SandboxUpdateDiagnostic>().map(
        (SandboxUpdateDiagnostic update) => update.diagnostic.code,
      ),
      contains('CONFIG_022'),
    );
  });
}
