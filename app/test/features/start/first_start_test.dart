// Der Erststart-Bildschirm (HUM-538): unter „Neu" für einen unbekannten
// Ordner, alles aus `Projects.proposal`, `Enter` startet mit den Vorgaben,
// nicht verfügbare und gesperrte Werte sagen warum, ein bekanntes Projekt
// startet ohne ihn.

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/time/now.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/core/ui/work_dir_picker.dart';
import 'package:humanitl/features/shell/shell_screen.dart';
import 'package:humanitl/features/start/providers/first_start.dart';
import 'package:humanitl/features/start/providers/projects.dart';
import 'package:humanitl/features/start/start_screen.dart';

import '../../harness/app_harness.dart';
import '../../harness/fixed_now.dart';

final DateTime _now = DateTime.utc(2026, 11, 21, 12);

/// Ein Fake mit einem bekannten Projekt `api`.
FakeDaemonClient _client() => FakeDaemonClient()
  ..projects = <ProjectEntry>[
    ProjectEntry(
      projectId: 'p-api',
      path: '/home/dev/api',
      displayName: 'api',
      lastStarted: _now.subtract(const Duration(hours: 2)),
      lastAgent: 'claude',
      runs: 3,
      hasChoice: true,
    ),
  ];

Future<void> _pump(
  WidgetTester tester,
  FakeDaemonClient client, {
  String folder = '/home/dev/fresh',
}) async {
  await pumpApp(
    tester,
    client: client,
    startWindow: true,
    overrides: [
      nowProvider.overrideWith(() => FixedNow(_now)),
      directoryChooserProvider.overrideWithValue(() async => folder),
    ],
  );
  await tester.pump();
  await tester.pump();
}

Future<void> _settle(WidgetTester tester) async {
  for (int i = 0; i < 4; i++) {
    await tester.pump();
  }
}

/// „Neu", der Ordnerdialog liefert den Ordner, und der Vorschlag kommt.
Future<void> _openNew(WidgetTester tester) async {
  await tester.tap(find.byKey(const Key('start-new-project')));
  await _settle(tester);
}

Future<void> _customize(WidgetTester tester) async {
  await tester.tap(find.byKey(const Key('first-start-customize')));
  await _settle(tester);
}

/// Das Segment mit dieser Beschriftung im Feld [field].
HSegment<String> _segment(WidgetTester tester, String field, String label) =>
    tester.widget<HSegment<String>>(
      find.descendant(
        of: find.byKey(Key('first-start-options-$field')),
        matching: find.byWidgetPredicate(
          (Widget w) => w is HSegment<String> && w.option.label == label,
        ),
      ),
    );

bool _startEnabled(WidgetTester tester) =>
    tester
        .widget<HButton>(find.byKey(const Key('first-start-start')))
        .onPressed !=
    null;

void main() {
  testWidgets('Enter without further input saves exactly the defaults of the '
      'proposal and starts the new folder by the id it got; the next start '
      'of the folder needs no screen', (WidgetTester tester) async {
    final FakeDaemonClient client = _client();
    final List<ChoiceValue> defaults =
        (await FakeDaemonClient().projectsProposal(path: '/home/dev/fresh'))
            .choice;
    await _pump(tester, client);
    await _openNew(tester);

    expect(find.byKey(const Key('first-start')), findsOneWidget);
    expect(client.proposalCalls.single.$1, '/home/dev/fresh');
    expect(
      tester.widget<Text>(find.byKey(const Key('first-start-defaults'))).data,
      'Claude Code from the host · disposable folder · shared kernel · '
      'ask in the app · model server local · profile default',
    );
    // Die Felder stehen erst hinter „Anpassen…".
    expect(find.byKey(const Key('first-start-field-agent')), findsNothing);

    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await _settle(tester);

    expect(client.savedChoices, hasLength(1));
    final (String? id, String? path, List<ChoiceValue> saved) =
        client.savedChoices.single;
    expect(id, isNull);
    expect(path, '/home/dev/fresh');
    expect(saved, defaults);
    expect(client.startWorkDirs, <String?>[null]);
    expect(client.startProjectIds, <String?>['fake-project-1']);
    expect(find.byType(ShellScreen), findsOneWidget);

    // Der Ordner steht jetzt im Register; der nächste Start ist bekannt.
    final StartProposal next = await client.projectsProposal(
      path: '/home/dev/fresh',
    );
    expect(next.known, isTrue);
  });

  testWidgets('a folder the register knows starts without the screen and '
      'saves nothing', (WidgetTester tester) async {
    final FakeDaemonClient client = _client();
    await _pump(tester, client, folder: '/home/dev/api');
    await _openNew(tester);

    expect(find.byKey(const Key('first-start')), findsNothing);
    expect(client.savedChoices, isEmpty);
    expect(client.startProjectIds, <String?>['p-api']);
    expect(find.byType(ShellScreen), findsOneWidget);
  });

  testWidgets('an isolation level that is not available stands visible, '
      'disabled and with its reason; tapping it asks nothing', (
    WidgetTester tester,
  ) async {
    // Mutationsbeweis: `enabled: option.available` in `_FieldRow` durch
    // `enabled: true` ersetzt färbt den Test rot.
    final FakeDaemonClient client = _client();
    await _pump(tester, client);
    await _openNew(tester);
    await _customize(tester);

    expect(find.byKey(const Key('first-start-field-isolation')), findsOne);
    expect(_segment(tester, 'isolation', 'shared kernel').enabled, isTrue);
    expect(_segment(tester, 'isolation', 'kernel level').enabled, isFalse);
    final Text reason = tester.widget<Text>(
      find.descendant(
        of: find.byKey(const Key('first-start-reason-isolation-kernel')),
        matching: find.byType(Text),
      ),
    );
    expect(reason.data, startsWith('kernel level: '));
    expect(reason.data, contains('/dev/kvm'));

    final int asked = client.proposalCalls.length;
    await tester.tap(
      find.descendant(
        of: find.byKey(const Key('first-start-options-isolation')),
        matching: find.text('kernel level'),
      ),
      warnIfMissed: false,
    );
    await _settle(tester);
    expect(client.proposalCalls, hasLength(asked));
  });

  testWidgets('a value the company policy locks stands read-only with the '
      'policy as its reason', (WidgetTester tester) async {
    final FakeDaemonClient client = _client()
      ..proposalLocks = <String, String>{
        'ask_mode': '/etc/humanitl/policy.toml',
      };
    await _pump(tester, client);
    await _openNew(tester);
    await _customize(tester);

    expect(find.byKey(const Key('first-start-options-ask_mode')), findsNothing);
    expect(
      tester
          .widget<Text>(find.byKey(const Key('first-start-value-ask_mode')))
          .data,
      'ask in the app',
    );
    final Text locked = tester.widget<Text>(
      find.descendant(
        of: find.byKey(const Key('first-start-locked-ask_mode')),
        matching: find.byType(Text),
      ),
    );
    expect(locked.data, contains('/etc/humanitl/policy.toml'));
    // Ein ungesperrtes Feld daneben bleibt wählbar.
    expect(_segment(tester, 'folder_mode', 'folder read-only').enabled, isTrue);
  });

  testWidgets('Enter on an option chooses it and asks the daemon again; it '
      'does not start, and Start waits until the answer is there', (
    WidgetTester tester,
  ) async {
    // Mutationsbeweis: `asking` aus `FirstStart.canStart` entfernt färbt den
    // Test rot; „Start" wäre dann schon vor der Antwort an.
    final FakeDaemonClient client = _client();
    await _pump(tester, client);
    await _openNew(tester);
    await _customize(tester);

    final Completer<void> hold = Completer<void>();
    client.proposalHold = hold;
    final Finder codex = find.descendant(
      of: find.byKey(const Key('first-start-options-agent')),
      matching: find.text('Codex'),
    );
    Focus.of(tester.element(codex)).requestFocus();
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();

    expect(
      client.proposalCalls.last.$3,
      contains(const ChoiceValue('agent.adapter', 'codex')),
    );
    expect(_startEnabled(tester), isFalse);
    // Noch ein Enter, bevor der Daemon antwortet: nichts startet.
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(client.savedChoices, isEmpty);
    expect(client.startProjectIds, isEmpty);

    client.proposalHold = null;
    hold.complete();
    await _settle(tester);
    expect(_startEnabled(tester), isTrue);
    expect(
      tester.widget<Text>(find.byKey(const Key('first-start-defaults'))).data,
      startsWith('Codex from the host'),
    );
    expect(client.savedChoices, isEmpty);

    await tester.tap(find.byKey(const Key('first-start-start')));
    await _settle(tester);
    expect(
      client.savedChoices.single.$3,
      contains(const ChoiceValue('agent.adapter', 'codex')),
    );
  });

  testWidgets('"Start choice…" on a recent project opens the screen for it, '
      'and Start saves by its id and starts it', (WidgetTester tester) async {
    final FakeDaemonClient client = _client();
    await _pump(tester, client);

    await tester.tap(find.byKey(const Key('start-choice-p-api')));
    await _settle(tester);
    expect(find.byKey(const Key('first-start')), findsOneWidget);
    expect(client.proposalCalls.single.$2, 'p-api');

    await tester.tap(find.byKey(const Key('first-start-start')));
    await _settle(tester);
    expect(client.savedChoices.single.$1, 'p-api');
    expect(client.savedChoices.single.$2, isNull);
    expect(client.startProjectIds, <String?>['p-api']);
  });

  testWidgets('cloud inference asks first; cancel starts nothing, confirm '
      'acknowledges, saves and starts', (WidgetTester tester) async {
    final FakeDaemonClient client = _client()
      ..proposalCloud = const CloudInference(
        agent: 'claude',
        provider: 'api.anthropic.com',
      );
    await _pump(tester, client);
    await _openNew(tester);

    await tester.tap(find.byKey(const Key('first-start-start')));
    await _settle(tester);
    expect(find.byKey(const Key('sandbox-cloud-confirm')), findsOneWidget);
    await tester.tap(find.byKey(const Key('sandbox-cloud-confirm-cancel')));
    await _settle(tester);
    expect(find.byKey(const Key('sandbox-cloud-confirm')), findsNothing);
    expect(client.savedChoices, isEmpty);

    await tester.tap(find.byKey(const Key('first-start-start')));
    await _settle(tester);
    await tester.tap(find.byKey(const Key('sandbox-cloud-confirm-yes')));
    await _settle(tester);
    expect(client.cloudAcknowledgements, <String>['claude api.anthropic.com']);
    expect(client.savedChoices, hasLength(1));
    expect(client.startProjectIds, <String?>['fake-project-1']);
  });

  testWidgets('a refused choice stays on the screen with the finding and '
      'starts nothing', (WidgetTester tester) async {
    final FakeDaemonClient client = _client()
      ..saveChoiceFailure = const Diagnostic(
        code: 'CONFIG_024',
        severity: Severity.error,
        why: 'the project choice lowers sandbox.min_isolation',
      );
    await _pump(tester, client);
    await _openNew(tester);
    await tester.tap(find.byKey(const Key('first-start-start')));
    await _settle(tester);

    expect(find.byType(StartScreen), findsOneWidget);
    expect(find.byKey(const Key('first-start')), findsOneWidget);
    expect(find.byKey(const Key('start-diagnostic-CONFIG_024')), findsOne);
    expect(client.startProjectIds, isEmpty);
  });

  testWidgets('Cancel leaves the screen; nothing is saved or started', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = _client();
    await _pump(tester, client);
    await _openNew(tester);
    await tester.tap(find.byKey(const Key('first-start-cancel')));
    await _settle(tester);

    expect(find.byKey(const Key('first-start')), findsNothing);
    expect(find.byKey(const Key('start-new-project')), findsOneWidget);
    expect(client.savedChoices, isEmpty);
    expect(client.startProjectIds, isEmpty);
  });

  testWidgets('both sources of the chosen agent stand with their state; the '
      'one without the program says why', (WidgetTester tester) async {
    final FakeDaemonClient client = _client();
    await _pump(tester, client);
    await _openNew(tester);
    await _customize(tester);

    String note(String source) => tester
        .widget<Text>(
          find.descendant(
            of: find.byKey(Key('first-start-source-$source')),
            matching: find.byType(Text),
          ),
        )
        .data!;
    expect(note('host'), 'Program from the host');
    expect(note('depot'), startsWith('Agent depot: '));
    expect(note('depot'), contains('not installed in the agent depot'));
  });

  testWidgets('a failed proposal keeps the choice before it', (
    WidgetTester tester,
  ) async {
    // Mutationsbeweis: im Fehlerfall von `_ask` den Zustand der Frage statt
    // `failed` setzen färbt den Test rot; die abgelehnte Wahl Codex reiste
    // dann mit der nächsten Frage mit.
    final FakeDaemonClient client = _client();
    await _pump(tester, client);
    await _openNew(tester);
    await _customize(tester);

    client.proposalFailure = const Diagnostic(
      code: 'CONFIG_024',
      severity: Severity.error,
      why: 'the partial choice does not hold',
    );
    await tester.tap(
      find.descendant(
        of: find.byKey(const Key('first-start-options-agent')),
        matching: find.text('Codex'),
      ),
    );
    await _settle(tester);
    expect(find.byKey(const Key('start-diagnostic-CONFIG_024')), findsOne);
    expect(_segment(tester, 'agent', 'Claude Code').selected, isTrue);

    await tester.tap(
      find.descendant(
        of: find.byKey(const Key('first-start-options-folder_mode')),
        matching: find.text('folder read-only'),
      ),
    );
    await _settle(tester);
    expect(
      client.proposalCalls.last.$3,
      isNot(contains(const ChoiceValue('agent.adapter', 'codex'))),
    );
    expect(
      client.proposalCalls.last.$3,
      contains(const ChoiceValue('sandbox.work_mode', 'ro')),
    );
  });

  testWidgets('Start while another start of the window runs keeps the screen, '
      'says so and saves nothing', (WidgetTester tester) async {
    // Mutationsbeweis: die Prüfung auf `busyPath` in `_saveAndStart`
    // entfernt färbt den Test rot; die Wahl wäre gespeichert und der
    // Bildschirm still geschlossen.
    final FakeDaemonClient client = _client();
    await _pump(tester, client);
    await _openNew(tester);
    final ProviderContainer container = ProviderScope.containerOf(
      tester.element(find.byType(StartScreen)),
    );

    unawaited(
      container
          .read(startLaunchProvider.notifier)
          .startKnown('p-api', '/home/dev/api'),
    );
    await container.read(firstStartProvider.notifier).start();

    expect(container.read(firstStartProvider)?.otherStartBusy, isTrue);
    expect(client.savedChoices, isEmpty);
    await _settle(tester);
  });
}
