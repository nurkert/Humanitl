// Die Abhilfe eines Starts, den wartende Änderungen sperren (HUM-419).
//
// Der Daemon lehnt einen Wegwerf-Start mit `SANDBOX_030` ab, solange die
// Änderungen eines früheren Wegwerf-Laufs warten, und gibt als Abhilfe
// `humanitl sandbox changes` mit. In der App ist die Abhilfe das Panel der
// Änderungen: Die Karte bietet „Review changes" als ersten Knopf, der
// Kopierbefehl bleibt als zweiter Weg stehen, und der Grund ist ein Satz in
// einfacher Sprache statt des technischen Satzes des Daemons.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl_ui/humanitl_ui.dart';

import 'disposable_fixtures.dart';
import 'harness.dart';

/// Der Befund, den der Daemon schickt (`disposable.rs`,
/// `pending_blocks_start`).
const Diagnostic waitingRefusal = Diagnostic(
  code: 'SANDBOX_030',
  severity: Severity.blocking,
  title: 'Disposable sandbox not possible',
  why:
      'the changes of the disposable sandbox 0b5e4f1a still wait for apply or '
      'discard; a new disposable run would put them out of reach, and a run '
      'that falls back to direct would write under them (only an explicit '
      '--direct leaves them waiting)',
  fix: FixAction.copyCommand(command: 'humanitl sandbox changes'),
);

/// Ein `SANDBOX_030` aus einem anderen Grund: Hier hilft das Panel nicht.
const Diagnostic oldBubblewrap = Diagnostic(
  code: 'SANDBOX_030',
  severity: Severity.blocking,
  title: 'Disposable sandbox not possible',
  why: 'bubblewrap 0.9.0 cannot stack an overlay; 0.11 or newer can',
  fix: FixAction.copyCommand(command: 'sudo apt install bubblewrap'),
);

const String plainWhy =
    'A previous disposable run still has changes waiting for you. Review '
    'them and apply or discard them, then start again.';

final Finder review = find.byKey(
  const Key('sandbox-diagnostic-review-changes'),
);
final Finder panel = find.byKey(const Key('changes-panel'));

Future<void> settle(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 600));
}

/// Ein Client, dessen letzter Start an [diagnostic] scheiterte.
SandboxTestClient refusedClient(Diagnostic diagnostic) {
  final SandboxTestClient client = disposableClient();
  client.sandbox = client.sandbox.copyWith(
    state: SandboxState.failed,
    diagnostics: <Diagnostic>[diagnostic],
  );
  return client;
}

void main() {
  testWidgets('the card offers Review changes first and keeps the command', (
    WidgetTester tester,
  ) async {
    await pumpSandbox(tester, client: refusedClient(waitingRefusal));

    expect(review, findsOneWidget);
    expect(find.text('Review changes'), findsOneWidget);
    expect(tester.widget<HButton>(review).variant, HButtonVariant.primary);
    // Der Kopierbefehl bleibt, als zweiter Weg.
    expect(find.byKey(const Key('setup-fix-copy')), findsOneWidget);
    expect(find.text('humanitl sandbox changes'), findsOneWidget);
    // Der Grund in einfacher Sprache, nicht der Satz des Daemons.
    expect(find.text(plainWhy), findsOneWidget);
    expect(find.textContaining('out of reach'), findsNothing);
    // Und eine Überschrift, die sagt, was ist, statt „nicht möglich".
    expect(find.text('Changes are waiting for you'), findsOneWidget);
    expect(find.text('Disposable sandbox not possible'), findsNothing);
  });

  testWidgets('Review changes opens the panel the person had closed', (
    WidgetTester tester,
  ) async {
    await pumpSandbox(tester, client: refusedClient(waitingRefusal));
    // Eine neue Liste öffnet das Panel einmal von selbst; geschlossen, öffnet
    // nur ein Klick es wieder.
    expect(panel, findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await settle(tester);
    expect(panel, findsNothing);
    expect(review, findsOneWidget);

    await tester.tap(review);
    await settle(tester);

    expect(panel, findsOneWidget);
  });

  testWidgets('Review changes fetches a list the app has not seen yet', (
    WidgetTester tester,
  ) async {
    // Die Änderungen kamen von einem Lauf, von dem die App nichts weiß,
    // etwa `humanitl run --disposable`: Beim ersten Fragen wartet nichts.
    final SandboxTestClient client = refusedClient(waitingRefusal)
      ..pendingChanges = null;
    await pumpSandbox(tester, client: client);
    expect(panel, findsNothing);

    client.pendingChanges = fakeDisposableChanges();
    await tester.tap(review);
    await settle(tester);

    expect(panel, findsOneWidget);
  });

  testWidgets('another cause of SANDBOX_030 keeps the daemon sentence and no '
      'review button', (WidgetTester tester) async {
    await pumpSandbox(tester, client: refusedClient(oldBubblewrap));

    expect(review, findsNothing);
    expect(find.text(plainWhy), findsNothing);
    expect(find.text(oldBubblewrap.why), findsOneWidget);
    expect(find.text('Changes are waiting for you'), findsNothing);
    expect(find.text('Disposable sandbox not possible'), findsOneWidget);
    expect(find.text('sudo apt install bubblewrap'), findsOneWidget);
  });
}
