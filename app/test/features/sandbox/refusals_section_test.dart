// Die verweigerten Versuche im Isolations-Reiter (HUM-138): gezählt je Art
// von Socket, mit Grund, und drei Zustände, die nie gleich aussehen dürfen --
// noch nicht gemeldet, gemeldet und leer, verweigert aber nicht gezählt.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';

import 'harness.dart';

/// Was der Shim nach dem Escape-Fall von ESC-1 meldet: drei `AF_UNIX`, zwei
/// `SOCK_DGRAM`.
final SandboxRefusals escapeRefusals = SandboxRefusals(
  reporting: SandboxRefusalReporting.on,
  entries: <SandboxRefusal>[
    SandboxRefusal(
      family: 'AF_UNIX',
      socketType: 'SOCK_STREAM',
      reason: SandboxRefusalReason.family,
      count: 3,
      firstAt: sandboxTestNow,
      lastAt: sandboxTestNow.add(const Duration(seconds: 7)),
    ),
    const SandboxRefusal(
      family: 'AF_INET',
      socketType: 'SOCK_DGRAM',
      reason: SandboxRefusalReason.type,
      count: 2,
    ),
  ],
);

/// Ein laufender Client, dessen Messung zusätzlich einen Stand der
/// Verweigerungen schickt, so wie der Start-Strom des Daemons es tut.
class RefusingClient extends SandboxTestClient {
  /// Was nach den drei Garantien kommt.
  SandboxRefusals? streamed;

  @override
  Stream<SandboxUpdate> checkIsolation() async* {
    yield* super.checkIsolation();
    final SandboxRefusals? streamed = this.streamed;
    if (streamed != null) {
      yield SandboxUpdate.refusals(streamed);
    }
  }
}

RefusingClient refusingClient({
  SandboxRefusals? snapshot,
  SandboxRefusals? streamed,
}) {
  final RefusingClient client = RefusingClient()..streamed = streamed;
  client.sandbox = client.sandbox.copyWith(
    state: SandboxState.running,
    agentRunning: true,
    startedAt: sandboxTestNow,
    refusals: snapshot,
  );
  client.isolationChecks = isolationGreenChecks;
  return client;
}

Future<void> openIsolation(WidgetTester tester) =>
    openTab(tester, 'sandbox-tab-isolation');

void main() {
  final TargetPlatformVariant linux = TargetPlatformVariant.only(
    TargetPlatform.linux,
  );

  testWidgets('a_refused_socket_stands_counted_with_its_reason', (
    WidgetTester tester,
  ) async {
    await pumpSandbox(
      tester,
      client: refusingClient(
        snapshot: const SandboxRefusals(reporting: SandboxRefusalReporting.on),
        streamed: escapeRefusals,
      ),
    );
    await openIsolation(tester);

    // The event from the stream replaced the empty snapshot.
    expect(statusOf(tester).refusals, escapeRefusals);
    expect(find.byKey(const Key('sandbox-refusals-title')), findsOneWidget);
    final Finder unix = find.byKey(
      const Key('sandbox-refusal-AF_UNIX-SOCK_STREAM'),
    );
    expect(unix, findsOneWidget);
    expect(
      find.descendant(of: unix, matching: find.text('3 attempts')),
      findsOneWidget,
    );
    expect(
      find.descendant(
        of: unix,
        matching: find.text('socket(AF_UNIX, SOCK_STREAM)'),
      ),
      findsOneWidget,
    );
    expect(
      find.descendant(of: unix, matching: find.text('family not allowed')),
      findsOneWidget,
    );
    final Finder dgram = find.byKey(
      const Key('sandbox-refusal-AF_INET-SOCK_DGRAM'),
    );
    expect(
      find.descendant(of: dgram, matching: find.text('2 attempts')),
      findsOneWidget,
    );
    expect(
      find.descendant(of: dgram, matching: find.text('type not allowed')),
      findsOneWidget,
    );
    // A tally is never an empty state at the same time.
    expect(find.byKey(const Key('sandbox-refusals-none')), findsNothing);
    expect(find.byKey(const Key('sandbox-refusals-pending')), findsNothing);
    // And the honest half: a refused connect is not in the list.
    expect(find.byKey(const Key('sandbox-refusals-network')), findsOneWidget);
  }, variant: linux);

  testWidgets('nothing_refused_pending_and_off_never_look_alike', (
    WidgetTester tester,
  ) async {
    await pumpSandbox(
      tester,
      client: refusingClient(
        snapshot: const SandboxRefusals(reporting: SandboxRefusalReporting.on),
      ),
    );
    await openIsolation(tester);
    expect(find.byKey(const Key('sandbox-refusals-none')), findsOneWidget);
    expect(find.byKey(const Key('sandbox-refusals-pending')), findsNothing);
    // The words, not only the key: "no socket refused so far" is a claim the sandbox
    // made, "not reported yet" is the absence of one.
    expect(find.text('No socket refused so far.'), findsOneWidget);
    expect(find.text('Not reported yet.'), findsNothing);

    await pumpSandbox(tester, client: refusingClient());
    await openIsolation(tester);
    expect(find.byKey(const Key('sandbox-refusals-pending')), findsOneWidget);
    expect(find.byKey(const Key('sandbox-refusals-none')), findsNothing);
    expect(find.text('Not reported yet.'), findsOneWidget);
    expect(find.text('No socket refused so far.'), findsNothing);

    await pumpSandbox(
      tester,
      client: refusingClient(
        snapshot: const SandboxRefusals(
          reporting: SandboxRefusalReporting.off,
          offReason: 'errno16',
        ),
      ),
    );
    await openIsolation(tester);
    final Finder off = find.byKey(const Key('sandbox-refusals-off'));
    expect(off, findsOneWidget);
    expect(tester.widget<Text>(off).data, contains('errno16'));
    expect(find.byKey(const Key('sandbox-refusals-none')), findsNothing);
  }, variant: linux);

  testWidgets('a_stopped_sandbox_shows_no_refusals_section', (
    WidgetTester tester,
  ) async {
    await pumpSandbox(tester, client: SandboxTestClient());
    await openIsolation(tester);
    expect(find.byKey(const Key('sandbox-refusals-title')), findsNothing);
  }, variant: linux);
}
