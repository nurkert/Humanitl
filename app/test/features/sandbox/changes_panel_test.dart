// Das Panel der Änderungen einer Wegwerf-Sandbox (HUM-351): Zeilen und
// Marken je Art, Diff und Binäres, Auswahl, die Rückfragen und genau die
// Pfad-Hashes, die beim Daemon ankommen.

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/sandbox/providers/disposable_review.dart';
import 'package:humanitl/features/sandbox/sandbox_screen.dart';

import 'disposable_fixtures.dart';
import 'harness.dart';

/// Öffnet das Panel über den Knopf im Kopf, wenn es nicht schon offen ist:
/// Nach dem ersten Lesen einer neuen Liste geht es von selbst auf.
Future<void> openPanel(WidgetTester tester) async {
  if (find.byKey(const Key('changes-panel')).evaluate().isNotEmpty) {
    return;
  }
  await tester.tap(find.byKey(const Key('sandbox-changes-open')));
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 600));
}

/// Schließt das Panel mit `Escape`.
Future<void> closePanel(WidgetTester tester) async {
  await tester.sendKeyEvent(LogicalKeyboardKey.escape);
  await settle(tester);
}

/// Der Container des Bildschirms, für die Provider.
ProviderContainer containerOf(WidgetTester tester) =>
    ProviderScope.containerOf(tester.element(find.byType(SandboxScreen)));

/// Ein Client, dessen `Sandbox(Changes)` erst antwortet, wenn der Test es
/// freigibt; die Antwort ist die Liste von dem Moment, in dem gefragt wurde.
class SlowChangesClient extends SandboxTestClient {
  /// Legt ihn mit der Fähigkeit und wartenden Änderungen an.
  SlowChangesClient() : super(info: FakeDaemonClient.disposableInfo) {
    pendingChanges = fakeDisposableChanges();
  }

  /// Hält die nächste Antwort an, solange gesetzt.
  Completer<void>? hold;

  @override
  Stream<SandboxUpdate> sandboxChanges() async* {
    final DisposableChanges? asked = pendingChanges;
    final Completer<void>? gate = hold;
    if (gate != null) {
      await gate.future;
    }
    if (asked == null) {
      yield* super.sandboxChanges();
      return;
    }
    yield SandboxUpdate.changes(asked);
  }
}

Future<void> settle(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 600));
}

Finder checkbox(String id) => find.byKey(ValueKey<String>('change-select-$id'));

Future<void> tapCheckbox(WidgetTester tester, String id) async {
  await tester.ensureVisible(checkbox(id));
  await tester.tap(checkbox(id));
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 300));
}

Future<void> tapAndSettle(WidgetTester tester, Key key) async {
  await tester.tap(find.byKey(key));
  await settle(tester);
}

void main() {
  testWidgets('every change kind has its row, its word and its badges', (
    WidgetTester tester,
  ) async {
    await pumpSandbox(tester, client: disposableClient());
    await openPanel(tester);

    expect(find.byKey(const Key('changes-panel')), findsOneWidget);
    expect(find.text('Project folder: /home/nik/clients/acme'), findsOneWidget);
    expect(find.text('7 changes, 5 selected'), findsOneWidget);
    String kind(String id) => tester
        .widget<Text>(
          find.descendant(
            of: find.byKey(ValueKey<String>('change-kind-$id')),
            matching: find.byType(Text),
          ),
        )
        .data!;
    expect(kind(logoId), 'added');
    expect(kind(utilId), 'added');
    expect(kind(readmeId), 'modified');
    expect(kind(notesId), 'deleted');
    expect(kind(linkId), 'symlink');
    expect(kind(scriptId), 'mode');

    // Konflikt und Ablehnung als Wort, mit dem Satz des Daemons darunter.
    expect(
      find.byKey(const ValueKey<String>('change-conflict-$mainId')),
      findsOneWidget,
    );
    expect(
      find.textContaining('Changed in the project since the start: the file'),
      findsOneWidget,
    );
    expect(
      find.byKey(const ValueKey<String>('change-refused-$linkId')),
      findsOneWidget,
    );
    expect(
      find.text(
        'Never applied: the symbolic link points outside the project folder '
        '(../../.ssh)',
      ),
      findsOneWidget,
    );
    expect(
      find.byKey(const ValueKey<String>('change-stripped-$scriptId')),
      findsOneWidget,
    );
    // Nur die markierten Zeilen tragen Marken.
    expect(
      find.byKey(const ValueKey<String>('change-conflict-$readmeId')),
      findsNothing,
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets('a row unfolds to the diff, a binary row to size and hash', (
    WidgetTester tester,
  ) async {
    await pumpSandbox(tester, client: disposableClient());
    await openPanel(tester);

    expect(
      find.byKey(const ValueKey<String>('change-diff-$readmeId')),
      findsNothing,
    );
    await tester.tap(
      find.byKey(const ValueKey<String>('change-expand-$readmeId')),
    );
    await settle(tester);
    final Finder diff = find.byKey(
      const ValueKey<String>('change-diff-$readmeId'),
    );
    expect(diff, findsOneWidget);
    // Gezeigt wie geschickt, Zeile für Zeile, mit Plus und Minus.
    expect(
      find.descendant(of: diff, matching: find.text('+    make test')),
      findsOneWidget,
    );
    expect(
      find.descendant(
        of: diff,
        matching: find.byKey(const ValueKey<String>('diff-line-added')),
      ),
      findsNWidgets(3),
    );
    expect(
      find.descendant(
        of: diff,
        matching: find.byKey(const ValueKey<String>('diff-line-removed')),
      ),
      findsOneWidget,
    );

    await tester.tap(
      find.byKey(const ValueKey<String>('change-expand-$logoId')),
    );
    await settle(tester);
    expect(
      find.text(
        'No text diff (binary or too large). Size 18.4 kB, SHA-256 '
        'a29180f7e6d5c4b3a29180f7e6d5c9f2c1e4b7a6d5c3b4b3a29180f7e6d5c4b3',
      ),
      findsOneWidget,
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets('apply selected sends exactly the chosen path hashes', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = disposableClient();
    await pumpSandbox(tester, client: client);
    await openPanel(tester);

    await tapCheckbox(tester, notesId);
    expect(find.text('7 changes, 4 selected'), findsOneWidget);
    await tapAndSettle(tester, const Key('changes-apply-selected'));

    // Die Rückfrage nennt Zahl und Ordner; nichts ist geschickt.
    expect(find.text('Apply 4 changes?'), findsOneWidget);
    expect(
      find.textContaining('They are written into /home/nik/clients/acme.'),
      findsOneWidget,
    );
    expect(client.applyCalls, isEmpty);

    await tapAndSettle(tester, const Key('changes-confirm-apply'));
    expect(client.applyCalls, hasLength(1));
    final FakeApplyCall call = client.applyCalls.single;
    expect(call.sandboxId, fakeDisposableSandbox);
    expect(call.ids, <String>[logoId, readmeId, scriptId, utilId]);
    expect(call.overwriteIds, isEmpty);
    expect(
      find.text('Applied: 4 written, 3 discarded.'),
      findsOneWidget,
      reason: 'the outcome replaces the list',
    );
    expect(find.byKey(const Key('changes-list')), findsNothing);
  });

  testWidgets('a refused row cannot be selected', (WidgetTester tester) async {
    await pumpSandbox(tester, client: disposableClient());
    await openPanel(tester);

    await tapCheckbox(tester, linkId);
    expect(find.text('7 changes, 5 selected'), findsOneWidget);
    expect(find.text('Overwrite link-to-home?'), findsNothing);
  });

  testWidgets('a conflict needs its own confirmation and overwrites only '
      'that file', (WidgetTester tester) async {
    final DisposableChanges two = fakeDisposableChanges().copyWith(
      changes: <DisposableChange>[
        for (final DisposableChange change in fakeDisposableChanges().changes)
          if (change.id == readmeId)
            change.copyWith(conflict: 'the size changed (1184 -> 1190)')
          else
            change,
      ],
    );
    final SandboxTestClient client = disposableClient(changes: two);
    await pumpSandbox(tester, client: client);
    await openPanel(tester);
    // Beide Konflikte sind zunächst nicht gewählt.
    expect(find.text('7 changes, 4 selected'), findsOneWidget);

    // Abbrechen wählt nichts.
    await tapCheckbox(tester, mainId);
    expect(find.text('Overwrite src/main.rs?'), findsOneWidget);
    await tapAndSettle(tester, const Key('changes-confirm-cancel'));
    expect(find.text('7 changes, 4 selected'), findsOneWidget);

    await tapCheckbox(tester, mainId);
    await tapAndSettle(tester, const Key('changes-confirm-overwrite'));
    expect(find.text('7 changes, 5 selected'), findsOneWidget);

    await tapAndSettle(tester, const Key('changes-apply-selected'));
    expect(
      find.text(
        '1 file that changed in the project since the start is overwritten.',
      ),
      findsOneWidget,
    );
    await tapAndSettle(tester, const Key('changes-confirm-apply'));
    final FakeApplyCall call = client.applyCalls.single;
    expect(call.ids, contains(mainId));
    expect(
      call.ids,
      isNot(contains(readmeId)),
      reason: 'the second conflict was never confirmed',
    );
    expect(call.overwriteIds, <String>[mainId]);
  });

  testWidgets('discard asks, leaves nothing waiting, and the start works '
      'again', (WidgetTester tester) async {
    final SandboxTestClient client = disposableClient();
    await pumpSandbox(tester, client: client);
    await closePanel(tester);

    // Der Daemon lehnt nur einen Wegwerf-Start ab, solange Änderungen
    // warten; ein direkter geht.
    HButton start() =>
        tester.widget<HButton>(find.byKey(const Key('sandbox-start')));
    expect(start().onPressed, isNotNull, reason: 'a direct start is allowed');
    await tester.tap(find.byKey(const Key('sandbox-disposable')));
    await settle(tester);
    expect(start().onPressed, isNull, reason: 'changes wait (SANDBOX_030)');

    await openPanel(tester);
    await tapAndSettle(tester, const Key('changes-discard-all'));
    expect(find.text('Discard all changes?'), findsOneWidget);
    expect(client.discardCalls, isEmpty);
    await tapAndSettle(tester, const Key('changes-confirm-discard'));

    expect(client.discardCalls, <String>[fakeDisposableSandbox]);
    expect(client.pendingChanges, isNull);
    expect(
      find.text('Discarded: 7 changes; the project folder is unchanged.'),
      findsOneWidget,
    );
    await tapAndSettle(tester, const Key('changes-done'));
    expect(find.byKey(const Key('changes-panel')), findsNothing);
    expect(find.byKey(const Key('sandbox-changes-open')), findsNothing);
    expect(
      start().onPressed,
      isNotNull,
      reason: 'nothing waits, so the disposable start works again',
    );
  });

  testWidgets('a partial outcome shows the daemon\'s sentence, and the rest '
      'keeps waiting', (WidgetTester tester) async {
    final SandboxTestClient client = disposableClient()
      ..applyBreaksAt = scriptId;
    await pumpSandbox(tester, client: client);
    await openPanel(tester);

    await tapAndSettle(tester, const Key('changes-apply-selected'));
    await tapAndSettle(tester, const Key('changes-confirm-apply'));

    expect(
      find.text(
        'Partly applied: 3 written before writing stopped; the rest still '
        'waits.',
      ),
      findsOneWidget,
    );
    expect(find.byKey(const Key('changes-notice')), findsOneWidget);
    expect(
      find.textContaining('scripts/run.sh: writing failed'),
      findsOneWidget,
    );
    // Die Liste steht noch, ohne die drei geschriebenen.
    expect(find.byKey(const Key('changes-list')), findsOneWidget);
    expect(checkbox(logoId), findsNothing);
    expect(checkbox(readmeId), findsNothing);
    expect(checkbox(notesId), findsNothing);
    expect(checkbox(scriptId), findsOneWidget);
  });

  testWidgets('a check that fails before the first write shows SANDBOX_031 '
      'with its fix and keeps the selection', (WidgetTester tester) async {
    final SandboxTestClient client = disposableClient();
    await pumpSandbox(tester, client: client);
    await openPanel(tester);
    // Der Mensch ändert README.md, während das Panel offen ist: Der Daemon
    // sieht den Konflikt, die Oberfläche noch nicht.
    client.pendingChanges = fakeDisposableChanges().copyWith(
      changes: <DisposableChange>[
        for (final DisposableChange change in fakeDisposableChanges().changes)
          change.id == readmeId
              ? change.copyWith(conflict: 'the mtime changed')
              : change,
      ],
    );

    await tapAndSettle(tester, const Key('changes-apply-selected'));
    await tapAndSettle(tester, const Key('changes-confirm-apply'));

    expect(find.byKey(const Key('changes-notice')), findsOneWidget);
    expect(find.text('SANDBOX_031'), findsOneWidget);
    expect(find.byKey(const Key('changes-fix-copy')), findsOneWidget);
    expect(find.text('7 changes, 5 selected'), findsOneWidget);
    expect(client.pendingChanges, isNotNull);
  });

  testWidgets('without the capability nothing is offered', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = SandboxTestClient()
      ..pendingChanges = fakeDisposableChanges();
    await pumpSandbox(tester, client: client);

    expect(find.byKey(const Key('sandbox-disposable')), findsNothing);
    expect(find.byKey(const Key('sandbox-changes-open')), findsNothing);
    expect(find.byKey(const Key('changes-panel')), findsNothing);
    expect(client.changesCalls, 0);
    final HButton start = tester.widget<HButton>(
      find.byKey(const Key('sandbox-start')),
    );
    expect(start.onPressed, isNotNull);

    await tester.tap(find.byKey(const Key('sandbox-start')));
    await settle(tester);
    expect(client.startModes, <SandboxMode?>[null]);
  });

  testWidgets('the start sends the mode of the switch, and nothing before '
      'it was touched', (WidgetTester tester) async {
    final SandboxTestClient client = disposableClient()..pendingChanges = null;
    await pumpSandbox(tester, client: client);

    expect(find.byKey(const Key('sandbox-disposable')), findsOneWidget);
    await tester.tap(find.byKey(const Key('sandbox-start')));
    await settle(tester);
    expect(client.startModes, <SandboxMode?>[null]);

    await tester.tap(find.byKey(const Key('sandbox-stop')));
    await settle(tester);
    await tester.tap(find.byKey(const Key('sandbox-stop-confirm')));
    await settle(tester);

    await tester.tap(find.byKey(const Key('sandbox-disposable')));
    await settle(tester);
    await tester.tap(find.byKey(const Key('sandbox-start')));
    await settle(tester);
    expect(client.startModes, <SandboxMode?>[null, SandboxMode.disposable]);
  });

  testWidgets('a disposable run that ends opens the panel without a click', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = disposableClient()..pendingChanges = null;
    await pumpSandbox(tester, client: client);
    expect(find.byKey(const Key('changes-panel')), findsNothing);

    await tester.tap(find.byKey(const Key('sandbox-disposable')));
    await settle(tester);
    await tester.tap(find.byKey(const Key('sandbox-start')));
    await settle(tester);
    await tester.tap(find.byKey(const Key('sandbox-stop')));
    await settle(tester);
    await tester.tap(find.byKey(const Key('sandbox-stop-confirm')));
    await settle(tester);

    expect(find.byKey(const Key('changes-panel')), findsOneWidget);
    expect(find.text('7 changes, 5 selected'), findsOneWidget);
  });

  testWidgets('Escape closes the panel and the header opens it again', (
    WidgetTester tester,
  ) async {
    await pumpSandbox(tester, client: disposableClient());
    // Nach einer (Wieder-)Verbindung geht das Panel einmal von selbst auf.
    expect(find.byKey(const Key('changes-panel')), findsOneWidget);

    await closePanel(tester);
    expect(find.byKey(const Key('changes-panel')), findsNothing);
    expect(find.text('Review changes (7)'), findsOneWidget);
    // Ein weiteres Lesen derselben Liste öffnet es nicht noch einmal.
    await containerOf(tester).read(disposableReviewProvider.notifier).fetch();
    await settle(tester);
    expect(find.byKey(const Key('changes-panel')), findsNothing);

    await openPanel(tester);
    expect(find.byKey(const Key('changes-panel')), findsOneWidget);
  });

  testWidgets('the panel holds at double text size', (
    WidgetTester tester,
  ) async {
    await pumpSandbox(
      tester,
      client: disposableClient(),
      textScaler: const TextScaler.linear(2),
    );
    await openPanel(tester);
    await tester.tap(
      find.byKey(const ValueKey<String>('change-expand-$readmeId')),
    );
    await settle(tester);
    expect(find.byKey(const Key('changes-panel')), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('a re-read drops a new conflict and a confirmation whose '
      'conflict changed', (WidgetTester tester) async {
    final SandboxTestClient client = disposableClient();
    await pumpSandbox(tester, client: client);
    await openPanel(tester);
    await tapCheckbox(tester, mainId);
    await tapAndSettle(tester, const Key('changes-confirm-overwrite'));
    expect(find.text('7 changes, 6 selected'), findsOneWidget);

    // Beim nächsten Lesen hat README.md einen Konflikt, den niemand bestätigt
    // hat, und der von src/main.rs ist ein anderer.
    client.pendingChanges = fakeDisposableChanges().copyWith(
      changes: <DisposableChange>[
        for (final DisposableChange change in fakeDisposableChanges().changes)
          if (change.id == readmeId)
            change.copyWith(conflict: 'the size changed (1184 -> 1190)')
          else if (change.id == mainId)
            change.copyWith(conflict: 'the file is gone from the project')
          else
            change,
      ],
    );
    await containerOf(tester).read(disposableReviewProvider.notifier).fetch();
    await settle(tester);

    final DisposableReview review = containerOf(tester)
        .read(disposableReviewProvider);
    expect(review.selected, isNot(contains(readmeId)));
    expect(review.selected, isNot(contains(mainId)));
    expect(review.overwrite, isEmpty);
    expect(find.text('7 changes, 4 selected'), findsOneWidget);
  });

  testWidgets('a late answer to a read does not bring back a discarded list', (
    WidgetTester tester,
  ) async {
    final SlowChangesClient client = SlowChangesClient();
    await pumpSandbox(tester, client: client);
    await openPanel(tester);

    final Completer<void> gate = Completer<void>();
    client.hold = gate;
    final Future<void> late = containerOf(tester)
        .read(disposableReviewProvider.notifier)
        .fetch();
    await settle(tester);

    await tapAndSettle(tester, const Key('changes-discard-all'));
    await tapAndSettle(tester, const Key('changes-confirm-discard'));
    expect(client.pendingChanges, isNull);

    gate.complete();
    await late;
    await settle(tester);
    expect(
      containerOf(tester).read(disposableReviewProvider).waiting,
      isFalse,
      reason: 'the answer was asked before the discard',
    );
    expect(find.byKey(const Key('sandbox-changes-open')), findsNothing);
  });

  testWidgets('the apply question sends what it showed, and asks again when '
      'the selection changed', (WidgetTester tester) async {
    final SandboxTestClient client = disposableClient();
    await pumpSandbox(tester, client: client);
    await openPanel(tester);
    await tapAndSettle(tester, const Key('changes-apply-selected'));
    expect(find.text('Apply 5 changes?'), findsOneWidget);

    // Während die Frage steht, liest das Panel neu: README.md hat jetzt einen
    // Konflikt und fällt aus der Auswahl.
    client.pendingChanges = fakeDisposableChanges().copyWith(
      changes: <DisposableChange>[
        for (final DisposableChange change in fakeDisposableChanges().changes)
          change.id == readmeId
              ? change.copyWith(conflict: 'the mtime changed')
              : change,
      ],
    );
    await containerOf(tester).read(disposableReviewProvider.notifier).fetch();
    await settle(tester);

    await tapAndSettle(tester, const Key('changes-confirm-apply'));
    expect(client.applyCalls, isEmpty, reason: 'the shown selection is gone');
    expect(find.text('Apply 4 changes?'), findsOneWidget);

    await tapAndSettle(tester, const Key('changes-confirm-apply'));
    expect(client.applyCalls.single.ids, <String>[
      logoId,
      notesId,
      scriptId,
      utilId,
    ]);
  });

  testWidgets('invisible and direction characters of the agent are replaced, '
      'not shown', (WidgetTester tester) async {
    final SandboxTestClient client = disposableClient(
      changes: fakeDisposableChanges().copyWith(
        changes: <DisposableChange>[
          const DisposableChange(
            id: 'aaaaaaaaaaaaaaaa',
            path: 'evil\u202Etxt.sh',
            kind: DisposableChangeKind.added,
            entry: 'file',
            diff:
                '--- /dev/null\n+++ b/x\n@@ -0,0 +1 @@\n'
                '+rm -rf ~ \u{E0041}\u{E0042}\n',
            conflict: 'changed \u2066hidden\u2069',
          ),
        ],
      ),
    );
    await pumpSandbox(tester, client: client);
    await openPanel(tester);
    await tester.tap(
      find.byKey(const ValueKey<String>('change-expand-aaaaaaaaaaaaaaaa')),
    );
    await settle(tester);

    expect(find.text('evil\uFFFDtxt.sh'), findsOneWidget);
    expect(find.textContaining('\u202E'), findsNothing);
    expect(find.text('+rm -rf ~ \uFFFD\uFFFD\uFFFD\uFFFD'), findsOneWidget);
    expect(find.textContaining('\u{E0041}'), findsNothing);
    expect(
      find.text(
        'Changed in the project since the start: changed \uFFFDhidden\uFFFD',
      ),
      findsOneWidget,
    );
  });

  testWidgets('a daemon before 1.26 offers no overwrite of a single file and '
      'never the flag for all', (WidgetTester tester) async {
    final SandboxTestClient client =
        SandboxTestClient(
            info: FakeDaemonClient.disposableInfo.copyWith(protoMinor: 25),
          )
          ..pendingChanges = fakeDisposableChanges()
          ..disposableRunChanges = fakeDisposableChanges();
    await pumpSandbox(tester, client: client);
    await openPanel(tester);

    expect(
      find.byKey(
        const ValueKey<String>('change-overwrite-unavailable-$mainId'),
      ),
      findsOneWidget,
    );
    await tapCheckbox(tester, mainId);
    expect(find.text('Overwrite src/main.rs?'), findsNothing);
    expect(find.text('7 changes, 5 selected'), findsOneWidget);
    // Auch der Weg am Kästchen vorbei bestätigt nichts.
    expect(
      containerOf(tester)
          .read(disposableReviewProvider.notifier)
          .confirmOverwrite(
            mainId,
            sawConflict: fakeDisposableChanges().changes
                .firstWhere((DisposableChange c) => c.id == mainId)
                .conflict,
            sandboxId: fakeDisposableSandbox,
          ),
      isFalse,
    );
    await tapAndSettle(tester, const Key('changes-apply-selected'));
    await tapAndSettle(tester, const Key('changes-confirm-apply'));
    expect(client.applyCalls.single.overwriteIds, isEmpty);
    expect(client.applyCalls.single.ids, isNot(contains(mainId)));
  });

  testWidgets('an apply question about one run never applies another run with '
      'the same ids', (WidgetTester tester) async {
    final SandboxTestClient client = disposableClient();
    await pumpSandbox(tester, client: client);
    await openPanel(tester);
    await tapAndSettle(tester, const Key('changes-apply-selected'));
    expect(find.text('Apply 5 changes?'), findsOneWidget);

    // Ein anderer Client verwirft Lauf A; Lauf B im selben Ordner hinterlässt
    // dieselben Pfade und damit dieselben Kennungen.
    client.pendingChanges = fakeDisposableChanges().copyWith(
      sandboxId: 'run-b',
    );
    await containerOf(tester).read(disposableReviewProvider.notifier).fetch();
    await settle(tester);

    await tapAndSettle(tester, const Key('changes-confirm-apply'));
    expect(client.applyCalls, isEmpty, reason: 'run B was never shown');
    expect(find.text('Apply 5 changes?'), findsOneWidget);
    await tapAndSettle(tester, const Key('changes-confirm-apply'));
    expect(client.applyCalls.single.sandboxId, 'run-b');
  });

  testWidgets('"Apply all" is bound to the run and the ids it showed', (
    WidgetTester tester,
  ) async {
    final DisposableChanges clean = fakeDisposableChanges().copyWith(
      changes: <DisposableChange>[
        for (final DisposableChange change in fakeDisposableChanges().changes)
          if (!change.isRefused && !change.hasConflict) change,
      ],
    );
    final SandboxTestClient client = disposableClient(changes: clean);
    await pumpSandbox(tester, client: client);
    await openPanel(tester);
    await tapAndSettle(tester, const Key('changes-apply-all'));
    expect(find.text('Apply 5 changes?'), findsOneWidget);

    client.pendingChanges = clean.copyWith(sandboxId: 'run-b');
    await containerOf(tester).read(disposableReviewProvider.notifier).fetch();
    await settle(tester);

    await tapAndSettle(tester, const Key('changes-confirm-apply'));
    expect(client.applyCalls, isEmpty, reason: 'run B was never shown');
    await tapAndSettle(tester, const Key('changes-confirm-apply'));
    expect(client.applyCalls.single.sandboxId, 'run-b');
  });

  testWidgets('an older "changes wait" answer never overrides a newer '
      '"nothing waits"', (WidgetTester tester) async {
    final SlowChangesClient client = SlowChangesClient();
    await pumpSandbox(tester, client: client);
    final DisposableReviewNotifier review = containerOf(tester)
        .read(disposableReviewProvider.notifier);

    final Completer<void> gate = Completer<void>();
    client.hold = gate;
    final Future<void> older = review.fetch();
    await settle(tester);
    // Inzwischen hat ein anderer Client verworfen; die neuere Frage sieht es.
    client
      ..hold = null
      ..pendingChanges = null;
    await review.fetch();
    await settle(tester);
    expect(containerOf(tester).read(disposableReviewProvider).waiting, isFalse);

    gate.complete();
    await older;
    await settle(tester);
    expect(
      containerOf(tester).read(disposableReviewProvider).waiting,
      isFalse,
      reason: 'the older answer came last',
    );
  });

  testWidgets('an overwrite question about one run never marks the same file '
      'of another run', (WidgetTester tester) async {
    final SandboxTestClient client = disposableClient();
    await pumpSandbox(tester, client: client);
    await openPanel(tester);
    await tapCheckbox(tester, mainId);
    expect(find.text('Overwrite src/main.rs?'), findsOneWidget);

    // Lauf B im selben Ordner: derselbe Pfad, derselbe Satz.
    client.pendingChanges = fakeDisposableChanges().copyWith(
      sandboxId: 'run-b',
    );
    await containerOf(tester).read(disposableReviewProvider.notifier).fetch();
    await settle(tester);

    await tapAndSettle(tester, const Key('changes-confirm-overwrite'));
    final DisposableReview review = containerOf(tester)
        .read(disposableReviewProvider);
    expect(review.overwrite, isEmpty, reason: 'run B was never confirmed');
    expect(review.selected, isNot(contains(mainId)));
    // Die Frage kommt neu, jetzt über Lauf B.
    expect(find.text('Overwrite src/main.rs?'), findsOneWidget);
    await tapAndSettle(tester, const Key('changes-confirm-overwrite'));
    expect(
      containerOf(tester).read(disposableReviewProvider).overwrite.keys,
      <String>[mainId],
    );
  });

  testWidgets('discard all asks again when the list changed within the run', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = disposableClient();
    await pumpSandbox(tester, client: client);
    await openPanel(tester);
    await tapAndSettle(tester, const Key('changes-discard-all'));
    expect(
      find.textContaining('None of the 7 changes reaches'),
      findsOneWidget,
    );

    // Derselbe Lauf, aber eine Zeile ist nicht mehr da (die Datei auf dem
    // Host gleicht jetzt der Schicht).
    client.pendingChanges = fakeDisposableChanges().copyWith(
      changes: <DisposableChange>[
        for (final DisposableChange change in fakeDisposableChanges().changes)
          if (change.id != notesId) change,
      ],
    );
    await containerOf(tester).read(disposableReviewProvider.notifier).fetch();
    await settle(tester);

    await tapAndSettle(tester, const Key('changes-confirm-discard'));
    expect(client.discardCalls, isEmpty, reason: 'the shown list is gone');
    expect(
      find.textContaining('None of the 6 changes reaches'),
      findsOneWidget,
    );
    await tapAndSettle(tester, const Key('changes-confirm-discard'));
    expect(client.discardCalls, <String>[fakeDisposableSandbox]);
  });
}
