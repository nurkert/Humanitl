// Das Ziehen eines Abbilds von der Karte eines `SANDBOX_043` (HUM-439).
//
// Kein Start zieht ein Abbild. Fehlt es, nennt der Daemon als Abhilfe den
// Befehl, der zieht; die Karte bietet dasselbe als ersten Knopf. Bevor etwas
// gezogen wird, sagt sie, dass das Netz dabei direkt genutzt wird und nicht
// durch den Proxy läuft, und die Antwort, die zieht, ist nie die vorgewählte.

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/sandbox/widgets/image_pull.dart';
import 'package:humanitl_ui/humanitl_ui.dart';

import 'harness.dart';

final String digestHex = 'a' * 64;
final String reference = 'docker.io/library/debian@sha256:$digestHex';

/// Der Befund, den der Daemon vor dem Start schickt, wenn das Abbild fehlt.
Diagnostic missingImage() => Diagnostic(
  code: 'SANDBOX_043',
  severity: Severity.blocking,
  why:
      'the image $reference is not in the local storage of the container '
      'engine. A start never pulls an image; pulling is a step of its own '
      'that uses the network directly, not through the proxy',
  fix: FixAction.copyCommand(command: 'humanitl sandbox image pull $reference'),
);

final Finder pull = find.byKey(const Key('sandbox-image-pull'));
final Finder network = find.byKey(const Key('sandbox-image-pull-network'));
final Finder confirm = find.byKey(const Key('sandbox-image-pull-confirm'));
final Finder cancel = find.byKey(const Key('sandbox-image-pull-cancel'));

/// Ein Client, dessen letzter Start an [diagnostic] scheiterte; sein Daemon
/// spricht den Vertrag in Minor [minor].
SandboxTestClient refused(Diagnostic diagnostic, {int minor = 0}) {
  final SandboxTestClient client = SandboxTestClient(
    info: FakeDaemonClient.defaultInfo.copyWith(protoMinor: minor),
  );
  client.sandbox = client.sandbox.copyWith(
    state: SandboxState.failed,
    diagnostics: <Diagnostic>[diagnostic],
  );
  return client;
}

void main() {
  test('only a SANDBOX_043 that names the pull gets the button', () {
    expect(imagePullReference(missingImage()), reference);
    const Diagnostic unusable = Diagnostic(
      code: 'SANDBOX_043',
      severity: Severity.blocking,
      why: 'its environment sets HUMANITL_BRIDGES',
      fix: FixAction.changeSetting(key: 'sandbox.backend', value: 'auto'),
    );
    expect(imagePullReference(unusable), isNull);
    const Diagnostic other = Diagnostic(
      code: 'SANDBOX_001',
      severity: Severity.blocking,
      why: 'the sandbox program is not on PATH',
      fix: FixAction.copyCommand(command: 'humanitl sandbox image pull x'),
    );
    expect(imagePullReference(other), isNull);
  });

  testWidgets('the card says that pulling bypasses the proxy before it pulls', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = refused(missingImage());
    await pumpSandbox(tester, client: client);

    expect(pull, findsOneWidget);
    expect(tester.widget<HButton>(pull).variant, HButtonVariant.primary);
    // Der Kopierbefehl bleibt, als zweiter Weg.
    expect(find.text('humanitl sandbox image pull $reference'), findsOneWidget);
    expect(network, findsNothing);

    await tester.ensureVisible(pull);
    await tester.tap(pull);
    await tester.pump();

    expect(client.imagePulls, isEmpty, reason: 'nothing before the answer');
    expect(network, findsOneWidget);
    await tester.pumpAndSettle();
    // Die Karte ist in der Höhe begrenzt; die Rückfrage scrollt sich selbst
    // in Sicht, die Antwort wird ohne Scrollen des Tests getroffen.
    final String said = tester.widget<Text>(network).data!;
    expect(said, contains(reference));
    expect(said, contains('directly'));
    expect(said, contains('not through the Humanitl proxy'));
    // „Abbrechen" hat den Fokus, nicht die Antwort, die zieht.
    expect(tester.widget<HButton>(cancel).autofocus, isTrue);
    expect(tester.widget<HButton>(confirm).autofocus, isFalse);

    await tester.tap(confirm);
    await tester.pump();
    await tester.pump();

    expect(client.imagePulls, <String>[reference]);
    expect(
      find.byKey(const Key('sandbox-image-pulled')),
      findsOneWidget,
      reason: 'the answer names the digest',
    );
    expect(find.textContaining('sha256:$digestHex'), findsWidgets);
  });

  testWidgets('cancel pulls nothing', (WidgetTester tester) async {
    final SandboxTestClient client = refused(missingImage());
    await pumpSandbox(tester, client: client);

    await tester.ensureVisible(pull);
    await tester.tap(pull);
    await tester.pump();
    await tester.ensureVisible(cancel);
    await tester.tap(cancel);
    await tester.pump();

    expect(client.imagePulls, isEmpty);
    expect(network, findsNothing);
    expect(pull, findsOneWidget);
  });

  testWidgets('a failed pull shows the daemon sentence and offers it again', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = refused(missingImage())
      ..imagePullFailure = const Diagnostic(
        code: 'SANDBOX_048',
        severity: Severity.blocking,
        why: 'the backend is not admitted yet',
      );
    await pumpSandbox(tester, client: client);

    await tester.ensureVisible(pull);
    await tester.tap(pull);
    await tester.pump();
    await tester.ensureVisible(confirm);
    await tester.tap(confirm);
    await tester.pump();
    await tester.pump();

    expect(find.byKey(const Key('sandbox-image-pull-failed')), findsOneWidget);
    expect(
      find.textContaining('the backend is not admitted yet'),
      findsOneWidget,
    );
    expect(pull, findsOneWidget);
  });

  // HUM-457: Ein laufendes Ziehen lässt sich abbrechen, über die Kennung,
  // die die Karte für genau dieses Ziehen gewählt hat.
  final Finder cancelRunning = find.byKey(
    const Key('sandbox-image-pull-cancel-running'),
  );

  /// Ein Client, dessen Daemon den Abbruch kennt und dessen Ziehen wartet,
  /// bis [hold] fertig ist oder abgebrochen wird.
  SandboxTestClient holding(Completer<void> hold, {int minor = 37}) {
    final SandboxTestClient client = refused(missingImage(), minor: minor);
    client.imagePullHold = hold;
    return client;
  }

  Future<void> startPull(WidgetTester tester) async {
    await tester.ensureVisible(pull);
    await tester.tap(pull);
    await tester.pump();
    await tester.ensureVisible(confirm);
    await tester.tap(confirm);
    await tester.pump();
  }

  testWidgets('a running pull is cancelled by its own id and the card says so', (
    WidgetTester tester,
  ) async {
    final Completer<void> hold = Completer<void>();
    final SandboxTestClient client = holding(hold);
    await pumpSandbox(tester, client: client);
    await startPull(tester);

    expect(find.byKey(const Key('sandbox-image-pulling')), findsOneWidget);
    expect(cancelRunning, findsOneWidget);
    expect(
      tester.widget<HButton>(cancelRunning).variant,
      HButtonVariant.secondary,
    );
    expect(client.imagePullIds, hasLength(1));
    final String id = client.imagePullIds.single;
    expect(
      id,
      matches(
        RegExp(
          r'^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$',
        ),
      ),
      reason: 'the card chooses a UUID for its pull',
    );

    await tester.ensureVisible(cancelRunning);
    await tester.tap(cancelRunning);
    await tester.pump();
    await tester.pump();

    expect(client.imagePullCancels, <String>[id]);
    expect(
      find.byKey(const Key('sandbox-image-pull-cancelled')),
      findsOneWidget,
    );
    expect(find.byKey(const Key('sandbox-image-pull-failed')), findsNothing);
    expect(cancelRunning, findsNothing);
    expect(pull, findsOneWidget, reason: 'the card offers the pull again');

    // Ein neues Ziehen bekommt eine neue Kennung und lässt den Hinweis fallen.
    await startPull(tester);
    expect(client.imagePullIds, hasLength(2));
    expect(client.imagePullIds.last, isNot(id));
    expect(find.byKey(const Key('sandbox-image-pull-cancelled')), findsNothing);
    hold.complete();
    await tester.pump();
    await tester.pump();
    expect(find.byKey(const Key('sandbox-image-pulled')), findsOneWidget);
  });

  testWidgets('a daemon that cannot cancel gets no cancel button', (
    WidgetTester tester,
  ) async {
    final Completer<void> hold = Completer<void>();
    final SandboxTestClient client = holding(hold, minor: 36);
    await pumpSandbox(tester, client: client);
    await startPull(tester);

    expect(find.byKey(const Key('sandbox-image-pulling')), findsOneWidget);
    expect(cancelRunning, findsNothing);
    expect(client.imagePullIds, <String>['']);
    hold.complete();
    await tester.pump();
    await tester.pump();
    expect(find.byKey(const Key('sandbox-image-pulled')), findsOneWidget);
  });

  Future<void> tapCancel(WidgetTester tester) async {
    await tester.ensureVisible(cancelRunning);
    await tester.tap(cancelRunning);
    await tester.pump();
    await tester.pump();
  }

  // Der Abbruch kann die Anmeldung seines Ziehens im Daemon überholen; dann
  // antwortet der Daemon „läuft nicht", und das Ziehen geht weiter. Die Karte
  // fragt nach kurzer Pause noch einmal.
  testWidgets(
    'a cancel that overtook its pull is sent again and then finds it',
    (WidgetTester tester) async {
      final Completer<void> hold = Completer<void>();
      final SandboxTestClient client = holding(hold)..imagePullCancelMisses = 1;
      await pumpSandbox(tester, client: client);
      await startPull(tester);
      final String id = client.imagePullIds.single;

      await tapCancel(tester);
      expect(client.imagePullCancels, <String>[id]);
      expect(find.byKey(const Key('sandbox-image-pulling')), findsOneWidget);
      expect(
        tester.widget<HButton>(cancelRunning).onPressed,
        isNull,
        reason: 'one cancel at a time',
      );

      await tester.pump(const Duration(milliseconds: 300));
      await tester.pump();
      expect(client.imagePullCancels, <String>[id, id]);
      expect(
        find.byKey(const Key('sandbox-image-pull-cancelled')),
        findsOneWidget,
      );
      expect(find.byKey(const Key('sandbox-image-pull-failed')), findsNothing);
    },
  );

  testWidgets('processes that survived the cancel are named in the note', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = holding(Completer<void>())
      ..imagePullCancelStillRunning = 2;
    await pumpSandbox(tester, client: client);
    await startPull(tester);
    await tapCancel(tester);

    final Finder note = find.byKey(const Key('sandbox-image-pull-cancelled'));
    expect(note, findsOneWidget);
    expect(tester.widget<Text>(note).data, contains('2'));
  });

  // Die Antwort auf den Abbruch blieb aus (etwa die Warnung, dass das Ziehen
  // nicht rechtzeitig endete); der SANDBOX_043 des Ziehens ist dann der
  // Abbruch und kein Fehlschlag.
  // Ist der Ausgang des Abbruchs unbekannt, entscheidet der Satz des Daemons
  // zum Ziehen; ein SANDBOX_043 allein ist kein Abbruch, er kann auch ein
  // Fehler der Registry sein.
  testWidgets('an unknown cancel outcome shows the pull\'s own sentence', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = holding(Completer<void>())
      ..imagePullCancelFailure = const Diagnostic(
        code: 'SANDBOX_043',
        severity: Severity.warning,
        why: 'the image pull was asked to stop but had not ended after 10s',
      );
    await pumpSandbox(tester, client: client);
    await startPull(tester);
    await tapCancel(tester);

    expect(find.byKey(const Key('sandbox-image-pull-cancelled')), findsNothing);
    expect(find.byKey(const Key('sandbox-image-pull-failed')), findsOneWidget);
    expect(find.textContaining('cancelled on request'), findsOneWidget);
  });

  // Geht der Abbruch auf dem Weg zum Daemon verloren, läuft das Ziehen
  // weiter, und der Knopf ist wieder nutzbar.
  testWidgets('a lost cancel leaves the button usable while the pull runs', (
    WidgetTester tester,
  ) async {
    final Completer<void> hold = Completer<void>();
    final SandboxTestClient client = holding(hold)
      ..imagePullCancelLost = const Diagnostic(
        code: 'IPC_001',
        severity: Severity.error,
        why: 'the daemon could not be reached',
      );
    await pumpSandbox(tester, client: client);
    await startPull(tester);
    await tapCancel(tester);

    expect(client.imagePullCancels, hasLength(1));
    expect(tester.widget<HButton>(cancelRunning).onPressed, isNotNull);

    client.imagePullCancelLost = null;
    await tapCancel(tester);
    expect(client.imagePullCancels, hasLength(2));
    expect(
      find.byKey(const Key('sandbox-image-pull-cancelled')),
      findsOneWidget,
    );
  });
}
