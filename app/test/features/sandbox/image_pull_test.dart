// Das Ziehen eines Abbilds von der Karte eines `SANDBOX_043` (HUM-439).
//
// Kein Start zieht ein Abbild. Fehlt es, nennt der Daemon als Abhilfe den
// Befehl, der zieht; die Karte bietet dasselbe als ersten Knopf. Bevor etwas
// gezogen wird, sagt sie, dass das Netz dabei direkt genutzt wird und nicht
// durch den Proxy läuft, und die Antwort, die zieht, ist nie die vorgewählte.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
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

/// Ein Client, dessen letzter Start an [diagnostic] scheiterte.
SandboxTestClient refused(Diagnostic diagnostic) {
  final SandboxTestClient client = SandboxTestClient();
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
}
