// HUM-502: Ein Bericht, in dem der Daemon ein Backend noch prüfte (Zeile
// `available` ohne Messung, `DOCTOR_012`), wird nach kurzer Zeit leise neu
// erfragt; ein vollständiger nicht.

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/setup/providers/setup_provider.dart';

/// Ein Bericht mit einer Gruppe, deren Zeile `available` so aussieht.
DoctorReport reportWith(DoctorCheck available) => DoctorReport(
  backends: <DoctorBackendGroup>[
    DoctorBackendGroup(name: 'container', checks: <DoctorCheck>[available]),
  ],
);

const DoctorCheck pending = DoctorCheck(
  id: DoctorBackendCheckId.available,
  status: DoctorStatus.warn,
  evidence: 'not measured: still being checked',
  diagnostic: Diagnostic(
    code: DiagnosticCodes.doctorNotPerformed,
    severity: Severity.warning,
    why: 'the check available could not be performed: still being checked',
  ),
);

const DoctorCheck found = DoctorCheck(
  id: DoctorBackendCheckId.available,
  status: DoctorStatus.ok,
  evidence: '/usr/bin/container 1.0',
);

/// Ein Fake, dessen `Doctor` die Antworten aus [answers] der Reihe nach
/// gibt; die letzte bleibt stehen.
class ScriptedDoctor extends FakeDaemonClient {
  ScriptedDoctor(this.answers);

  final List<Future<DoctorReport> Function()> answers;
  int asked = 0;

  @override
  Future<DoctorReport> doctor() {
    final int index = asked < answers.length ? asked : answers.length - 1;
    asked += 1;
    return answers[index]();
  }
}

Future<DoctorReport> checking() async => reportWith(pending);
Future<DoctorReport> complete() async => reportWith(found);
Future<DoctorReport> failing() async => throw const DaemonException(
  Diagnostic(code: 'DAEMON_001', severity: Severity.error, why: 'gone'),
);

ProviderContainer containerFor(FakeDaemonClient client) {
  final ProviderContainer container = ProviderContainer(
    overrides: [daemonClientProvider.overrideWithValue(client)],
  );
  addTearDown(container.dispose);
  final ProviderSubscription<AsyncValue<DoctorReport>> watched = container
      .listen(setupDoctorProvider, (_, _) {});
  addTearDown(watched.close);
  return container;
}

void main() {
  // Mutationsbeweis: ohne das erneute Fragen bleibt es bei einem Aufruf, und
  // der Bericht behält die Zeile ohne Messung.
  testWidgets('a report with a backend still being checked is asked again', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient()
      ..doctorReport = reportWith(pending);
    final ProviderContainer container = ProviderContainer(
      overrides: [daemonClientProvider.overrideWithValue(client)],
    );
    addTearDown(container.dispose);
    final ProviderSubscription<AsyncValue<DoctorReport>> watched = container
        .listen(setupDoctorProvider, (_, _) {});
    addTearDown(watched.close);

    final DoctorReport first = await container.read(setupDoctorProvider.future);
    expect(first.backendStillChecking, isTrue);
    expect(client.doctorCalls, 1);

    client.doctorReport = reportWith(found);
    await tester.pump(doctorCheckingRetry);
    await tester.pump();
    expect(client.doctorCalls, 2);
    final DoctorReport second = container.read(setupDoctorProvider).value!;
    expect(second.backendStillChecking, isFalse);

    // Vollständig: Niemand fragt mehr.
    await tester.pump(doctorCheckingRetry * 2);
    expect(client.doctorCalls, 2);
  });

  // Die Schleife ist begrenzt: Bleibt ein Backend in Prüfung, fragt der
  // Bildschirm höchstens [doctorCheckingAttempts] Mal und behält dann den
  // Bericht. Mutationsbeweis: ohne Grenze wächst die Zahl der Aufrufe weiter.
  testWidgets('the quiet retries end after a bounded number of attempts', (
    WidgetTester tester,
  ) async {
    final ScriptedDoctor client = ScriptedDoctor(
      <Future<DoctorReport> Function()>[checking],
    );
    final ProviderContainer container = containerFor(client);
    await container.read(setupDoctorProvider.future);
    for (int i = 0; i < doctorCheckingAttempts + 5; i++) {
      await tester.pump(doctorCheckingRetry);
    }
    expect(client.asked, 1 + doctorCheckingAttempts);
    expect(
      container.read(setupDoctorProvider).value!.backendStillChecking,
      isTrue,
    );
  });

  // Ein Fehlschlag während der Prüfung wird innerhalb der Grenze wiederholt;
  // scheitert auch der letzte Versuch, zeigt der Bildschirm den Fehler.
  testWidgets('a failed retry is retried, and the last failure is shown', (
    WidgetTester tester,
  ) async {
    final ScriptedDoctor client = ScriptedDoctor(
      <Future<DoctorReport> Function()>[checking, failing, complete],
    );
    final ProviderContainer container = containerFor(client);
    await container.read(setupDoctorProvider.future);
    await tester.pump(doctorCheckingRetry);
    expect(container.read(setupDoctorProvider).hasError, isFalse);
    await tester.pump(doctorCheckingRetry);
    expect(client.asked, 3);
    expect(
      container.read(setupDoctorProvider).value!.backendStillChecking,
      isFalse,
    );

    final ScriptedDoctor gone = ScriptedDoctor(
      <Future<DoctorReport> Function()>[checking, failing],
    );
    final ProviderContainer second = containerFor(gone);
    await second.read(setupDoctorProvider.future);
    for (int i = 0; i < doctorCheckingAttempts + 2; i++) {
      await tester.pump(doctorCheckingRetry);
    }
    expect(gone.asked, 1 + doctorCheckingAttempts);
    expect(second.read(setupDoctorProvider).hasError, isTrue);
  });

  // Eine späte Antwort eines älteren stillen Versuchs überschreibt nie die
  // Antwort, um die `refresh` danach gebeten hat. Mutationsbeweis: ohne den
  // Zähler der Fragen steht am Ende der ältere Bericht da.
  testWidgets('an older answer never overwrites a newer refresh', (
    WidgetTester tester,
  ) async {
    final Completer<DoctorReport> late = Completer<DoctorReport>();
    final ScriptedDoctor client = ScriptedDoctor(
      <Future<DoctorReport> Function()>[checking, () => late.future, complete],
    );
    final ProviderContainer container = containerFor(client);
    await container.read(setupDoctorProvider.future);
    await tester.pump(doctorCheckingRetry);
    expect(client.asked, 2, reason: 'the quiet retry is on its way');
    await container.read(setupDoctorProvider.notifier).refresh();
    expect(
      container.read(setupDoctorProvider).value!.backendStillChecking,
      isFalse,
    );
    late.complete(reportWith(pending));
    await tester.pump();
    expect(
      container.read(setupDoctorProvider).value!.backendStillChecking,
      isFalse,
    );
  });

  // Ein „Check again", während die erste Frage noch läuft, gewinnt: Die
  // ältere Antwort überschreibt die neuere nicht und fragt nicht weiter.
  // Mutationsbeweis: ohne den Zähler der Fragen in `build` steht am Ende
  // der ältere Bericht da.
  testWidgets('a refresh during the first request wins over its answer', (
    WidgetTester tester,
  ) async {
    final Completer<DoctorReport> first = Completer<DoctorReport>();
    final ScriptedDoctor client = ScriptedDoctor(
      <Future<DoctorReport> Function()>[() => first.future, complete],
    );
    final ProviderContainer container = containerFor(client);
    await tester.pump();
    await container.read(setupDoctorProvider.notifier).refresh();
    expect(
      container.read(setupDoctorProvider).value!.backendStillChecking,
      isFalse,
    );
    first.complete(reportWith(pending));
    await tester.pump();
    await tester.pump();
    expect(
      container.read(setupDoctorProvider).value!.backendStillChecking,
      isFalse,
    );
    await tester.pump(doctorCheckingRetry * 2);
    expect(client.asked, 2, reason: 'the older answer asks nothing again');
  });

  // Die erste Frage scheitert erst, nachdem „Check again" einen Bericht
  // brachte: Ihr Fehler ersetzt ihn nicht. Mutationsbeweis: ohne den Zähler
  // bei einem Fehler der ersten Frage steht am Ende ihr Fehler da.
  testWidgets('a late failure of the first request does not replace a newer '
      'report', (WidgetTester tester) async {
    final Completer<DoctorReport> first = Completer<DoctorReport>();
    final ScriptedDoctor client = ScriptedDoctor(
      <Future<DoctorReport> Function()>[() => first.future, complete],
    );
    final ProviderContainer container = containerFor(client);
    await tester.pump();
    await container.read(setupDoctorProvider.notifier).refresh();
    first.completeError(
      const DaemonException(
        Diagnostic(code: 'DAEMON_001', severity: Severity.error, why: 'old'),
      ),
    );
    await tester.pump();
    await tester.pump();
    final AsyncValue<DoctorReport> shown = container.read(setupDoctorProvider);
    expect(shown.hasError, isFalse, reason: '$shown');
    expect(shown.value!.backendStillChecking, isFalse);
  });

  // „Check again" scheitert, danach kommt die erste Frage mit einem Bericht:
  // Der neuere Fehler bleibt stehen. Mutationsbeweis: mit dem eigenen
  // Bericht der ersten Frage steht er statt des Fehlers da.
  testWidgets('a late report of the first request does not replace a newer '
      'error', (WidgetTester tester) async {
    final Completer<DoctorReport> first = Completer<DoctorReport>();
    final ScriptedDoctor client = ScriptedDoctor(
      <Future<DoctorReport> Function()>[() => first.future, failing],
    );
    final ProviderContainer container = containerFor(client);
    await tester.pump();
    await container.read(setupDoctorProvider.notifier).refresh();
    expect(container.read(setupDoctorProvider).hasError, isTrue);
    first.complete(reportWith(pending));
    await tester.pump();
    await tester.pump();
    expect(container.read(setupDoctorProvider).hasError, isTrue);
    await tester.pump(doctorCheckingRetry * 2);
    expect(client.asked, 2, reason: 'the older report asks nothing again');
  });
}
