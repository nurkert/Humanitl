// Die Sackgassen des Setup-Flows (HUM-044): der Start, der zweimal ausgelöst
// wird, der gescheiterte Start, der den Ordner verschwinden lässt, die
// Rückkehr des Daemons, nach der niemand den Ordner neu misst, und die leere
// Warteschlange, aus der kein Weg zum Start führt (`docs/UX.md` 4.2, Fall 3).

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/work_dir_picker.dart';
import 'package:humanitl/features/intercept/widgets/queue_pane.dart';
import 'package:humanitl/features/sandbox/providers/sandbox_status_provider.dart';
import 'package:humanitl/features/setup/providers/setup_provider.dart';
import 'package:humanitl/features/shell/providers/navigation.dart';
import 'package:humanitl/features/shell/section.dart';
import 'package:humanitl/features/shell/shell_screen.dart';

import '../../harness/app_harness.dart';
import 'setup_test.dart' show containerOf, reasonOf, startEnabled, stateOf;

/// Ein Fake, dessen `Sandbox(Start)` `starting` meldet und dann stehen
/// bleibt, bis der Test ihn loslässt.
///
/// Genau das ist das Fenster, in dem ein zweiter Druck einen zweiten Start
/// auslösen kann: Der Daemon hat geantwortet, aber der Start ist noch nicht
/// fertig.
class StallingStartClient extends FakeDaemonClient {
  /// Erzeugt den Fake mit einer Maschine, die der Doctor als in Ordnung
  /// meldet, damit alle vier Zeilen grün sind.
  StallingStartClient() {
    doctorReport = fakeDoctorOk();
  }

  /// Wie oft `Sandbox(Start)` gerufen wurde.
  int startCalls = 0;

  /// Wird erfüllt, wenn der Start weiterlaufen darf.
  final Completer<void> release = Completer<void>();

  @override
  Stream<SandboxUpdate> startSandbox({
    String? profile,
    String? workDir,
    WorkMode? workMode,
  }) async* {
    startCalls++;
    sandbox = sandbox.copyWith(state: SandboxState.starting);
    yield SandboxUpdate.status(sandbox);
    await release.future;
    sandbox = sandbox.copyWith(
      state: SandboxState.running,
      agentRunning: true,
      sandboxId: FakeDaemonClient.defaultSandbox,
    );
    for (final IsolationCheckResult result in isolationChecks) {
      yield SandboxUpdate.check(result);
    }
    yield SandboxUpdate.status(sandbox);
  }
}

/// Ein Fake, dessen `Sandbox(Start)` stehen bleibt und danach abgelehnt wird.
///
/// Das ist das Fenster, in dem ein **zweiter** `Sandbox`-Aufruf ganz
/// durchlaeuft, waehrend der Start noch auf sein naechstes Ereignis wartet:
/// Wer mit `Ctrl+4` in den Sandbox-Abschnitt geht, loest dort ein
/// `Sandbox(Status)` aus.
class StallingRefusedStartClient extends FakeDaemonClient {
  /// Erzeugt den Fake mit einer gruenen Maschine und einem Start, den der
  /// Daemon am Ende ablehnt.
  StallingRefusedStartClient() {
    doctorReport = fakeDoctorOk();
    sandboxStartFailure = const Diagnostic(
      code: DiagnosticCodes.isolationNoReport,
      severity: Severity.blocking,
      title: 'Isolation check without a report',
      why: 'the sandbox reported no isolation check at all',
    );
  }

  /// Wird erfuellt, wenn der Start weiterlaufen darf.
  final Completer<void> release = Completer<void>();

  @override
  Stream<SandboxUpdate> startSandbox({
    String? profile,
    String? workDir,
    WorkMode? workMode,
  }) async* {
    sandbox = sandbox.copyWith(state: SandboxState.starting);
    yield SandboxUpdate.status(sandbox);
    await release.future;
    yield* super.startSandbox(
      profile: profile,
      workDir: workDir,
      workMode: workMode,
    );
  }
}

/// Der Ordner, den der Knopf des Pickers nennt, oder null.
String? pickedFolder(WidgetTester tester) => tester
    .widget<WorkDirPicker>(
      find.ancestor(
        of: find.byKey(const Key('setup-workdir')),
        matching: find.byType(WorkDirPicker),
      ),
    )
    .workDir;

void main() {
  /// Zwischen dem Klick und der ersten Antwort des Daemons baut sich nichts
  /// neu auf. Ohne Sperre schickt ein zweiter Druck ein zweites
  /// `Sandbox(Start)`, und beide Ströme falten ihre Ereignisse in denselben
  /// Schnappschuss.
  testWidgets('the start button does not fire a second start', (
    WidgetTester tester,
  ) async {
    final StallingStartClient client = StallingStartClient();
    await pumpApp(tester, client: client);
    await pressCtrl(tester, LogicalKeyboardKey.digit6);
    await tester.pump();
    expect(startEnabled(tester), isTrue);

    await tester.tap(find.byKey(const Key('setup-start')));
    await tester.pump();

    // Der Start läuft; der Knopf ruht, so wie er im Kopf des
    // Sandbox-Bildschirms ruht.
    expect(client.startCalls, 1);
    expect(startEnabled(tester), isFalse);

    await tester.tap(find.byKey(const Key('setup-start')), warnIfMissed: false);
    await tester.pump();
    expect(client.startCalls, 1);

    client.release.complete();
    await tester.pumpAndSettle();
    expect(client.startCalls, 1);
  });

  /// Ein Start, der im Transport scheitert, sagt nichts über den Ordner. Zwei
  /// Dinge folgen daraus, und beide standen sich schon einmal im Weg: Der
  /// Ordner ist weiter der, den der Daemon zuletzt genannt hat, und der Knopf
  /// des Pickers zeigt ihn -- **und** die dritte Zeile bleibt so grün, wie ihre
  /// letzte Messung war. Der Befund gehört an den Ort des Fehlschlags, also
  /// unter den Knopf, auf den jemand gedrückt hat, und nicht auf eine Zeile
  /// über eine Eingabe, an der sich nichts geändert hat (`docs/UX.md` 4.4).
  testWidgets('a start that fails in transport keeps the chosen folder', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient()
      ..doctorReport = fakeDoctorOk();
    await pumpApp(tester, client: client);
    await pressCtrl(tester, LogicalKeyboardKey.digit6);
    await tester.pump();
    expect(startEnabled(tester), isTrue);
    expect(pickedFolder(tester), FakeDaemonClient.defaultWorkDir);
    expect(find.byKey(const Key('setup-start-failure')), findsNothing);

    // Der Dienst wird zwischen zwei Klicks gestoppt: `Sandbox(Start)` wirft,
    // statt einen Befund zu senden.
    client.goOffline();
    await tester.tap(find.byKey(const Key('setup-start')));
    await tester.pumpAndSettle();

    // Der Ordner steht weiter im Knopf -- `work_dir_host` im Daemon hat sich
    // nicht geändert.
    expect(pickedFolder(tester), FakeDaemonClient.defaultWorkDir);
    // Und die Zeile behauptet weder eine Lücke noch einen Fehler des Ordners.
    expect(stateOf(tester, SetupCheckKind.project), 'ok');
    expect(reasonOf(tester), isNot(contains('Project folder')));
    expect(reasonOf(tester), isNot(contains('Nothing was measured')));
    // Der Befund steht unter dem Knopf, mit dem Code des Transports.
    final Finder card = find.byKey(const Key('setup-start-failure'));
    expect(card, findsOneWidget);
    expect(
      find.descendant(
        of: card,
        matching: find.text(DiagnosticCodes.daemonUnreachable),
      ),
      findsOneWidget,
    );
    // Und der Satz auf der Knopfzeile sagt, was nicht geschehen ist.
    expect(reasonOf(tester), contains('did not arrive'));
  });

  /// Akzeptanzkriterium 1, zu Ende gegangen: Der Daemon fehlt beim Start,
  /// jemand startet ihn im Terminal, der Zwei-Sekunden-Takt bringt ihn
  /// zurück -- und danach ist jede der vier Zeilen wieder gemessen, nicht nur
  /// die des Dienstes.
  testWidgets('the reconnect measures the project folder again', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient()
      ..doctorReport = fakeDoctorOk()
      ..goOffline();
    await pumpApp(
      tester,
      client: client,
      reconnect: const Duration(seconds: 2),
    );

    client.goOnline();
    await tester.pump(const Duration(seconds: 2));
    await tester.pumpAndSettle();
    expect(find.byType(ShellScreen), findsOneWidget);

    await pressCtrl(tester, LogicalKeyboardKey.digit6);
    await tester.pumpAndSettle();

    expect(stateOf(tester, SetupCheckKind.project), 'ok');
    expect(pickedFolder(tester), FakeDaemonClient.defaultWorkDir);
    expect(startEnabled(tester), isTrue);
  });

  /// Und die Sperre gilt weiter, solange eine Sandbox läuft.
  ///
  /// `Section.setup` ist ein fester Eintrag der Leiste, also kommt jemand mit
  /// `Ctrl+6` jederzeit zurück -- auch mitten in einer laufenden Sitzung. Die
  /// vier Zeilen sind dann alle grün, denn keine von ihnen misst die Sitzung.
  /// Ohne die `isUp`-Hälfte der Sperre schickte ein Druck ein zweites
  /// `Sandbox(Start)` in eine laufende Sandbox hinein, und der Ordner-Knopf
  /// verlöre seine Marke, so dass ein `Sandbox(Plan)` in den Schnappschuss
  /// einer laufenden Sitzung liefe.
  testWidgets('a running sandbox keeps the start button shut', (
    WidgetTester tester,
  ) async {
    final StallingStartClient client = StallingStartClient();
    await pumpApp(tester, client: client);
    await pressCtrl(tester, LogicalKeyboardKey.digit6);
    await tester.pump();
    expect(startEnabled(tester), isTrue);

    await tester.tap(find.byKey(const Key('setup-start')));
    await tester.pump();
    expect(client.startCalls, 1);

    // Der Start kommt durch; die Sandbox läuft, und die Anwendung wechselt in
    // die Warteschlange.
    client.release.complete();
    await tester.pumpAndSettle();
    expect(containerOf(tester).read(navigationProvider), Section.intercept);

    // Und wer mit `Ctrl+6` zurückkommt, findet den Knopf aus.
    await pressCtrl(tester, LogicalKeyboardKey.digit6);
    await tester.pumpAndSettle();
    expect(stateOf(tester, SetupCheckKind.project), 'ok');
    expect(startEnabled(tester), isFalse);

    await tester.tap(find.byKey(const Key('setup-start')), warnIfMissed: false);
    await tester.pumpAndSettle();
    expect(client.startCalls, 1);
  });

  /// Ein Start, den der Daemon ablehnt, sagt nichts über den Ordner.
  ///
  /// `Sandbox(Plan)` hat genau diesen Ordner gemessen und für gut befunden;
  /// was hier scheiterte, ist die Garantie im Inneren der Sandbox. Der Befund
  /// gehört deshalb unter den Knopf, auf den jemand gedrückt hat. Stünde er
  /// auf der dritten Zeile, führte der Bildschirm in eine Sackgasse: Der Start
  /// ist aus, das einzige Bedienelement daneben ist der Ordner-Knopf, und
  /// dessen `Sandbox(Plan)` räumt die Befunde weg und färbt die Zeile wieder
  /// grün -- jemand repariert einen Ordner, der nie kaputt war, und es sieht
  /// aus, als hätte es geholfen (`docs/UX.md` 4.4).
  testWidgets('a start the daemon refuses is reported under the button', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient()
      ..doctorReport = fakeDoctorOk()
      // Der echte rote Weg: Die Sandbox kam hoch, eine Garantie hielt nicht,
      // und der Daemon hat sie wieder angehalten (HUM-041).
      ..isolationChecks = fakeIsolationNoReport();
    await pumpApp(tester, client: client);
    await pressCtrl(tester, LogicalKeyboardKey.digit6);
    await tester.pump();
    expect(startEnabled(tester), isTrue);
    expect(find.byKey(const Key('setup-start-failure')), findsNothing);

    await tester.tap(find.byKey(const Key('setup-start')));
    await tester.pumpAndSettle();

    // Die dritte Zeile bleibt, was ihre letzte Messung sagte.
    expect(stateOf(tester, SetupCheckKind.project), 'ok');
    expect(pickedFolder(tester), FakeDaemonClient.defaultWorkDir);
    expect(reasonOf(tester), isNot(contains('Project folder')));

    // Der Befund steht unter dem Knopf, mit dem Code der Garantie.
    final Finder card = find.byKey(const Key('setup-start-failure'));
    expect(card, findsOneWidget);
    expect(
      find.descendant(
        of: card,
        matching: find.text(DiagnosticCodes.isolationNoReport),
      ),
      findsOneWidget,
    );

    // Und der Satz nennt, was geschehen ist: Der Aufruf kam an, der Start
    // nicht hoch.
    expect(reasonOf(tester), contains('did not come up'));
    expect(reasonOf(tester), isNot(contains('did not arrive')));

    // Keine Sackgasse: Es läuft keine Sandbox, also ist der Start wieder
    // drückbar, und der Befund darunter sagt, was zu tun ist.
    expect(startEnabled(tester), isTrue);
  });

  /// Und dieselbe Trennung haelt auch, wenn zwei `Sandbox`-Aufrufe zugleich
  /// unterwegs sind.
  ///
  /// Ein Start dauert so lange, wie eine Sandbox zum Hochkommen braucht. Wer
  /// in dieser Zeit mit `Ctrl+4` in den Sandbox-Abschnitt geht, loest dort ein
  /// `Sandbox(Status)` aus, und das laeuft ganz durch, bevor der Start seine
  /// Ablehnung schickt. Wuerde sich der Notifier den laufenden Aufruf in einem
  /// gemeinsamen Feld merken, traege der spaeter eintreffende Befund des
  /// Starts die Kennung `status`, `SandboxCall.isAboutTheFolder` sagte ja, und
  /// die dritte Zeile stuende wieder rot mit einem Befund, der nichts ueber
  /// den Ordner sagt -- genau die Sackgasse aus `docs/UX.md` 4.4, zurueck
  /// durch ein Wettrennen.
  testWidgets('a start refused while another call runs is not the folder', (
    WidgetTester tester,
  ) async {
    final StallingRefusedStartClient client = StallingRefusedStartClient();
    await pumpApp(tester, client: client);
    await pressCtrl(tester, LogicalKeyboardKey.digit6);
    await tester.pump();
    expect(startEnabled(tester), isTrue);

    await tester.tap(find.byKey(const Key('setup-start')));
    await tester.pump();
    expect(startEnabled(tester), isFalse);

    // Der zweite Aufruf, waehrend der Start haengt: Der Sandbox-Abschnitt
    // wird sichtbar und fragt `Sandbox(Status)`.
    await pressCtrl(tester, LogicalKeyboardKey.digit4);
    await tester.pump();
    await tester.pump();
    expect(
      containerOf(tester).read(sandboxCallProvider),
      SandboxCall.status,
      reason: 'the visible sandbox section really did ask in between',
    );

    // Und jetzt erst antwortet der Start, mit seiner Ablehnung.
    client.release.complete();
    await tester.pumpAndSettle();
    await pressCtrl(tester, LogicalKeyboardKey.digit6);
    await tester.pumpAndSettle();

    // Die dritte Zeile bleibt, was ihre letzte Messung sagte, und der Ordner
    // steht weiter im Knopf.
    expect(stateOf(tester, SetupCheckKind.project), 'ok');
    expect(pickedFolder(tester), FakeDaemonClient.defaultWorkDir);
    expect(reasonOf(tester), isNot(contains('Project folder')));
    // Der Befund steht unter dem Knopf, auf den jemand gedrueckt hat.
    final Finder card = find.byKey(const Key('setup-start-failure'));
    expect(card, findsOneWidget);
    expect(
      find.descendant(
        of: card,
        matching: find.text(DiagnosticCodes.isolationNoReport),
      ),
      findsOneWidget,
    );
    expect(
      containerOf(tester).read(sandboxCallProvider),
      SandboxCall.start,
      reason: 'the finding belongs to the call that produced it',
    );
  });

  /// `docs/UX.md` 4.2, Fall 3: Der Daemon läuft, keine Sitzung, die
  /// Warteschlange ist leer -- und aus ihr führt ein Weg zu dem Knopf, der
  /// eine Sitzung startet. Ohne ihn steht der Start auf einem Abschnitt, den
  /// niemand geöffnet hat.
  testWidgets('the empty queue leads to the start', (
    WidgetTester tester,
  ) async {
    // Der zweite Start mit fertiger Einrichtung: Die Maschine ist gemessen,
    // den Modellserver hat niemand ansprechen lassen. Damit sperrt der Start,
    // ohne dass eine Zeile rot ist, und die Anwendung öffnet die
    // Warteschlange.
    await pumpApp(tester, client: FakeDaemonClient.empty());
    final ProviderContainer container = ProviderScope.containerOf(
      tester.element(find.byType(ShellScreen)),
    );
    expect(container.read(navigationProvider), Section.intercept);
    expect(find.byType(QueueEmptyState), findsOneWidget);

    await tester.tap(find.byKey(const Key('queue-start-session')));
    await tester.pumpAndSettle();

    expect(container.read(navigationProvider), Section.setup);
    expect(find.byKey(const Key('setup-start')), findsOneWidget);
  });
}
