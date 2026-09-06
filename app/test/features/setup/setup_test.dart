// Widget-Tests des Setup-Bildschirms (HUM-019, HUM-044): fehlender Daemon,
// Versionskonflikt, erneut verbinden, Herzschlag, die vier Zeilen, der
// Start-Knopf und die Zeile, die sagt, was nicht gemessen wurde.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/fix_control.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl_ui/humanitl_ui.dart';
import 'package:humanitl/core/ipc/client_diagnostics.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/setup/providers/setup_provider.dart';
import 'package:humanitl/features/setup/setup_screen.dart';
import 'package:humanitl/features/setup/widgets/doctor_list.dart';
import 'package:humanitl/features/intercept/intercept_screen.dart';
import 'package:humanitl/features/shell/providers/navigation.dart';
import 'package:humanitl/features/shell/section.dart';
import 'package:humanitl/features/shell/shell_screen.dart';
import 'package:humanitl/features/shell/widgets/frozen_banner.dart';
import 'package:humanitl/features/shell/widgets/splash.dart';

import '../../harness/app_harness.dart';

/// Der Text der Zustandszeile einer Zeile.
String stateOf(WidgetTester tester, SetupCheckKind kind) =>
    tester.widget<Text>(find.byKey(Key('setup-state-${kind.name}'))).data!;

/// Ob der Start-Knopf gedrückt werden kann.
bool startEnabled(WidgetTester tester) =>
    tester.widget<HButton>(find.byKey(const Key('setup-start'))).enabled;

/// Ob der Knopf der Dienst-Zeile gedrückt werden kann.
bool retryEnabled(WidgetTester tester) =>
    tester.widget<HButton>(find.byKey(const Key('setup-daemon-retry'))).enabled;

/// Der Satz unter dem Start-Knopf.
String reasonOf(WidgetTester tester) =>
    tester.widget<Text>(find.byKey(const Key('setup-start-reason'))).data!;

/// Ein Fake, dessen Maschine der Doctor als in Ordnung meldet.
FakeDaemonClient healthy() => FakeDaemonClient()..doctorReport = fakeDoctorOk();

/// Der Behälter des laufenden Fensters.
ProviderContainer containerOf(WidgetTester tester) =>
    ProviderScope.containerOf(tester.element(find.byType(ShellScreen)));

/// Die Beschriftung des Halte-Abzeichens in der Kopfzeile.
String heldBadge(WidgetTester tester) =>
    tester.widget<HBadge>(find.byKey(const Key('header-held-badge'))).text;

/// Ein Fake, dessen `GetInfo` erst nach 200 ms scheitert.
///
/// Ein Fehlschlag, der noch im selben Mikrotask kommt, überlebt keinen Frame,
/// und ein Splash, den niemand sehen kann, bewiese nichts. Der echte Weg ist
/// ohnehin nie so schnell: `GrpcDaemonClient` liest bei jedem Aufruf die
/// Token-Datei von der Platte und wartet dann auf einen gRPC-Aufruf mit fünf
/// Sekunden Frist.
class SlowlyFailingClient extends FakeDaemonClient {
  /// Erzeugt den Fake mit einem Daemon, der nicht antwortet.
  SlowlyFailingClient() {
    goOffline();
  }

  @override
  Future<DaemonInfo> getInfo() async {
    await Future<void>.delayed(const Duration(milliseconds: 200));
    return super.getInfo();
  }
}

/// Ein Daemon, den es erst gibt, nachdem jemand ihn installiert hat.
///
/// `goOffline` taugt dafür nicht: Sein Befund trägt `fake: true`, und der
/// gewöhnliche Weg — Nutzer-Unit, Standard-Socket — ist der einzige, auf dem
/// `DAEMON_001` die Abhilfe `InstallService` trägt (`client_diagnostics.dart`).
/// Der Knopf, um den es hier geht, entstünde sonst gar nicht.
class UninstalledDaemonClient extends FakeDaemonClient {
  /// True, sobald der Installer gelaufen ist.
  bool installed = false;

  @override
  Future<DaemonInfo> getInfo() async {
    if (!installed) {
      throw DaemonException(
        ClientDiagnostics.daemonUnreachable(
          socketPath: FakeDaemonClient.defaultSocket,
          detail: 'connection refused',
        ),
      );
    }
    return super.getInfo();
  }
}

/// Elf Zeilen, alle gemessen, nur `daemon` nicht.
///
/// Die Form, die ein echter Daemon auf einem gesunden Rechner schickt: Er kann
/// von innen nicht sagen, ob ein Client seinen Socket erreicht, und meldet
/// seine eigene Zeile deshalb immer als nicht versucht.
DoctorReport doctorOkExceptOwnLine() => DoctorReport(
  checks: <DoctorCheck>[
    for (final DoctorCheck check in fakeDoctorOk().checks)
      if (check.id == DoctorCheckId.daemon)
        check.copyWith(
          status: DoctorStatus.warn,
          evidence: 'not measured: a daemon does not probe itself',
          diagnostic: const Diagnostic(
            code: DiagnosticCodes.doctorNotPerformed,
            severity: Severity.warning,
            why: 'the check daemon cannot be performed from the inside',
          ),
        )
      else
        check,
  ],
);

/// Elf gemessene Zeilen, von denen genau eine gelb ist.
DoctorReport doctorWarning() => DoctorReport(
  checks: <DoctorCheck>[
    for (final DoctorCheck check in fakeDoctorOk().checks)
      if (check.id == DoctorCheckId.diskSpace)
        check.copyWith(
          status: DoctorStatus.warn,
          diagnostic: const Diagnostic(
            code: 'DOCTOR_011',
            severity: Severity.warning,
            why: 'less than a gigabyte is left in the data directory',
          ),
        )
      else
        check,
  ],
);

void main() {
  testWidgets('connection_failed_shows_setup', (WidgetTester tester) async {
    await pumpApp(tester, client: FakeDaemonClient.unavailable());

    expect(find.byType(SetupScreen), findsOneWidget);
    expect(find.byType(ShellScreen), findsNothing);
    expect(find.text(DiagnosticCodes.daemonUnreachable), findsOneWidget);
    expect(find.text('Daemon not reachable'), findsOneWidget);
    // Die Detailzeile nennt den Socket.
    expect(find.textContaining(FakeDaemonClient.defaultSocket), findsOneWidget);
    // Der Fix ist der Startbefehl des Fake-Daemons.
    expect(
      find.textContaining('humanitld --fake fixtures/sessions/mixed.jsonl'),
      findsOneWidget,
    );
    expect(find.byKey(const Key('setup-daemon-retry')), findsOneWidget);
  });

  testWidgets('setup_shows_daemon_001_with_install_action', (
    WidgetTester tester,
  ) async {
    await pumpApp(tester, client: FakeDaemonClient.unavailable());

    // Die erste Zeile sperrt den Start und sagt es auf dem Knopf.
    expect(stateOf(tester, SetupCheckKind.daemon), 'blocks the start');
    expect(startEnabled(tester), isFalse);
    expect(reasonOf(tester), contains('Background service'));
    // Und die drei anderen Zeilen behaupten nichts: ohne Daemon hat niemand
    // nachgesehen.
    expect(stateOf(tester, SetupCheckKind.sandbox), 'not measured');
  });

  testWidgets('version_mismatch_shows_setup', (WidgetTester tester) async {
    await pumpApp(tester, client: FakeDaemonClient.incompatible());

    expect(find.byType(SetupScreen), findsOneWidget);
    expect(find.text(DiagnosticCodes.protoIncompatible), findsOneWidget);
    expect(find.text('Incompatible daemon'), findsOneWidget);
    expect(find.textContaining('proto 2.0'), findsOneWidget);
  });

  testWidgets('a compatible minor is accepted', (WidgetTester tester) async {
    await pumpApp(
      tester,
      client: FakeDaemonClient.incompatible(protoMajor: 1, protoMinor: 7),
    );
    expect(find.byType(ShellScreen), findsOneWidget);
  });

  testWidgets('retry reconnects once the daemon is back', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient()..goOffline();
    await pumpApp(tester, client: client);
    expect(find.byType(SetupScreen), findsOneWidget);

    client.goOnline();
    await tester.tap(find.byKey(const Key('setup-daemon-retry')));
    await tester.pump();
    // Der Splash gehört dem ersten Versuch dieses Laufs. Ein zweiter Versuch
    // nimmt den Bildschirm nicht weg, auf dem der Befund und seine Abhilfe
    // stehen -- sonst blinkte der Zwei-Sekunden-Takt ihn alle zwei Sekunden
    // fort (`docs/UX.md` 4.2, Fall 2).
    expect(find.byType(Splash), findsNothing);
    expect(find.byType(SetupScreen), findsOneWidget);
    await tester.pump();
    expect(find.byType(ShellScreen), findsOneWidget);
    expect(client.infoCalls, 2);
  });

  /// Das Akzeptanzkriterium: Wer den Dienst im Terminal startet, sieht die
  /// Zeile grün werden, ohne im Fenster etwas anzufassen.
  testWidgets('a stopped daemon is tried again every two seconds', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient()..goOffline();
    await pumpApp(
      tester,
      client: client,
      reconnect: const Duration(seconds: 2),
    );
    expect(find.byType(SetupScreen), findsOneWidget);
    final int tried = client.infoCalls;

    // Der Mensch startet den Dienst; niemand fasst das Fenster an.
    client.goOnline();
    await tester.pump(const Duration(seconds: 2));
    await tester.pump();
    await tester.pump();

    expect(client.infoCalls, greaterThan(tried));
    expect(find.byType(ShellScreen), findsOneWidget);
  });

  testWidgets('the heartbeat notices a stopped daemon', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient();
    await pumpApp(
      tester,
      client: client,
      heartbeat: const Duration(seconds: 1),
    );
    expect(find.byType(ShellScreen), findsOneWidget);
    expect(client.infoCalls, 1);

    await tester.pump(const Duration(seconds: 1));
    await tester.pump();
    expect(client.infoCalls, 2);
    expect(find.byType(ShellScreen), findsOneWidget);

    client.goOffline();
    await tester.pump(const Duration(seconds: 1));
    await tester.pump();

    // Bemerkt heißt gezeigt, und gezeigt heißt nicht, dass der Bildschirm
    // verschwindet: Eine Verbindung, die stand, hinterlässt die Shell mit
    // ihren wartenden Anfragen und darüber ein Banner (`docs/UX.md` 4.2,
    // Fall 4). Der Setup-Bildschirm ersetzt sie nur beim Kaltstart.
    expect(find.byType(ShellScreen), findsOneWidget);
    expect(find.byType(FrozenBanner), findsOneWidget);
    expect(find.byType(SetupScreen), findsNothing);
    // Und der Grund ist nicht verschluckt: Er steht im Banner, mit dem Socket,
    // an dem niemand mehr antwortet. Der Code steht auf der Setup-Zeile, die
    // eingefroren nicht erreichbar ist -- gezeigt wird deshalb, was ein Mensch
    // in diesem Augenblick wirklich lesen kann.
    expect(
      tester.widget<Text>(find.byKey(const Key('shell-frozen-why'))).data,
      contains(FakeDaemonClient.defaultSocket),
    );

    // Ohne Frist schlägt nichts mehr; erneut verbinden startet neu.
    final int calls = client.infoCalls;
    await tester.pump(const Duration(seconds: 3));
    expect(client.infoCalls, calls);

    client.goOnline();
    await tester.tap(find.byKey(const Key('shell-frozen-reconnect')));
    await tester.pump();
    await tester.pump();
    expect(find.byType(ShellScreen), findsOneWidget);
    expect(find.byType(FrozenBanner), findsNothing);
  });

  testWidgets('a daemon diagnostic with its own code is shown as is', (
    WidgetTester tester,
  ) async {
    const Diagnostic shipped = Diagnostic(
      code: 'IPC_001',
      severity: Severity.error,
      title: 'Ungültiges Token',
      why: 'metadata key x-humanitl-token does not match the session token',
    );
    await pumpApp(tester, client: FakeDaemonClient(infoFailure: shipped));
    expect(find.text('IPC_001'), findsOneWidget);
    expect(find.text('Token rejected'), findsOneWidget);
    expect(
      find.textContaining('does not match the session token'),
      findsOneWidget,
    );
  });

  testWidgets('start_button_enabled_only_when_all_ok', (
    WidgetTester tester,
  ) async {
    // Eine Maschine, auf der genau eine Zeile rot ist: `bwrap` fehlt.
    final FakeDaemonClient broken = FakeDaemonClient()
      ..doctorReport = fakeDoctorFailing();
    await pumpApp(tester, client: broken);
    await pressCtrl(tester, LogicalKeyboardKey.digit6);
    await tester.pump();

    expect(stateOf(tester, SetupCheckKind.sandbox), 'blocks the start');
    expect(startEnabled(tester), isFalse);
    expect(find.text('DOCTOR_001'), findsOneWidget);

    // Dieselbe Maschine, gemessen und nicht rot, aber auch nicht grün. Genau
    // dieser Fall trennt "alle vier grün" von "nichts sperrt": Ohne ihn ginge
    // der Knopf an, obwohl eine Zeile gelb ist.
    await tester.pumpWidget(const SizedBox.shrink());
    await pumpApp(
      tester,
      client: FakeDaemonClient()..doctorReport = doctorWarning(),
    );
    await pressCtrl(tester, LogicalKeyboardKey.digit6);
    await tester.pump();

    expect(stateOf(tester, SetupCheckKind.sandbox), 'worth knowing');
    expect(startEnabled(tester), isFalse);
    expect(reasonOf(tester), contains('Sandbox check'));

    // Und dieselbe Maschine, alle elf Zeilen grün.
    await tester.pumpWidget(const SizedBox.shrink());
    await pumpApp(tester, client: healthy());
    await pressCtrl(tester, LogicalKeyboardKey.digit6);
    await tester.pump();

    expect(stateOf(tester, SetupCheckKind.sandbox), 'ok');
    expect(startEnabled(tester), isTrue);
  });

  /// Die Reihenfolge aus der Spezifikation: `Sandbox(Start)`, die Prüfungen
  /// der Isolation, **dann** der Wechsel in die Warteschlange. Vorher zu
  /// wechseln hieße, jemanden in eine leere Liste zu setzen und den Befund
  /// hinter ihm stehen zu lassen (`docs/UX.md` 4.2).
  testWidgets('the queue opens only after the start really came up', (
    WidgetTester tester,
  ) async {
    await pumpApp(tester, client: healthy());
    await pressCtrl(tester, LogicalKeyboardKey.digit6);
    await tester.pump();
    expect(startEnabled(tester), isTrue);

    await tester.tap(find.byKey(const Key('setup-start')));
    await tester.pumpAndSettle();

    // Und zwar in die Warteschlange, nicht irgendwohin. `find.byType` allein
    // sagt das nicht: Die sechs Abschnitte stehen in einem `IndexedStack`, der
    // nur den sichtbaren besucht, also ist `SetupScreen` bei jedem anderen
    // Abschnitt „nicht da" (Akzeptanzkriterium 3).
    expect(containerOf(tester).read(navigationProvider), Section.intercept);
    expect(find.byType(InterceptScreen), findsOneWidget);
    expect(find.byType(SetupScreen), findsNothing);
  });

  testWidgets('a start that fails leaves the setup on the screen', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient refusing = healthy()
      ..sandboxStartFailure = const Diagnostic(
        code: DiagnosticCodes.isolationSeccompActive,
        severity: Severity.blocking,
        why: 'the seccomp filter did not hold',
      );
    await pumpApp(tester, client: refusing);
    await pressCtrl(tester, LogicalKeyboardKey.digit6);
    await tester.pump();
    expect(startEnabled(tester), isTrue);

    await tester.tap(find.byKey(const Key('setup-start')));
    await tester.pumpAndSettle();

    // Der Bildschirm bleibt stehen, und der Grund steht darauf.
    expect(find.byType(SetupScreen), findsOneWidget);
    expect(find.text('the seccomp filter did not hold'), findsOneWidget);
  });

  /// Der Befund, den die Zahl auf der Knopfzeile deckt: Der Daemon meldet
  /// seine eigene Zeile auf jedem Rechner als nicht versucht. Zählt der
  /// Bildschirm sie mit, behauptet er auch auf einer gemessenen Maschine eine
  /// Lücke, und der grüne Satz ist nirgends erreichbar.
  testWidgets('the daemon own line never makes the screen claim a gap', (
    WidgetTester tester,
  ) async {
    await pumpApp(
      tester,
      client: FakeDaemonClient()..doctorReport = doctorOkExceptOwnLine(),
    );
    await pressCtrl(tester, LogicalKeyboardKey.digit6);
    await tester.pump();

    expect(stateOf(tester, SetupCheckKind.sandbox), 'ok');
    expect(startEnabled(tester), isTrue);
    expect(reasonOf(tester), 'Everything was measured and everything holds.');
    expect(reasonOf(tester), isNot(contains('could not be measured')));
  });

  /// Der Kern des Issues: Eine Prüfung, die nicht laufen konnte, ist nie grün,
  /// und was nicht grün ist, startet nicht. Der Knopf steht still und die
  /// Zeile darunter nennt die Zeile, die im Weg steht.
  testWidgets('an unmeasured check is its own state and keeps the start shut', (
    WidgetTester tester,
  ) async {
    // Die Vorgabe des Fakes: elf Zeilen, keine davon gemessen.
    await pumpApp(tester, client: FakeDaemonClient());
    await pressCtrl(tester, LogicalKeyboardKey.digit6);
    await tester.pump();

    expect(stateOf(tester, SetupCheckKind.sandbox), 'not measured');
    expect(startEnabled(tester), isFalse);
    // Die erste Zeile, die im Weg steht, ist der Modellserver: Ihn hat
    // niemand ansprechen lassen. Der Satz nennt sie und sagt, was hilft.
    expect(reasonOf(tester), contains('Your model server'));
    expect(reasonOf(tester), contains('Measure it above'));
  });

  testWidgets('the machine row names how many of its lines were not measured', (
    WidgetTester tester,
  ) async {
    // Modellserver gemessen, Maschine nicht: Dann ist die Maschinenzeile die
    // erste, die im Weg steht, und die Zahl auf der Knopfzeile zählt genau die
    // neun Zeilen, die diese Zeile faltet -- nicht die elf des Berichts.
    final FakeDaemonClient client = FakeDaemonClient();
    await pumpApp(tester, client: client);
    await pressCtrl(tester, LogicalKeyboardKey.digit6);
    await tester.pump();
    await tester.enterText(
      find.byKey(const Key('setup-llm-endpoint')),
      'http://192.168.1.10:11434',
    );
    await tester.tap(find.byKey(const Key('setup-llm-probe')));
    await tester.pump();
    await tester.pump();

    expect(stateOf(tester, SetupCheckKind.llm), 'ok');
    expect(stateOf(tester, SetupCheckKind.sandbox), 'not measured');
    expect(startEnabled(tester), isFalse);
    expect(reasonOf(tester), startsWith('9 checks of this machine'));
  });

  testWidgets('a folder nobody chose blocks the start and says so', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient()
      ..doctorReport = fakeDoctorOk()
      ..sandbox = const SandboxStatus();
    await pumpApp(tester, client: client);
    await pressCtrl(tester, LogicalKeyboardKey.digit6);
    await tester.pump();

    expect(stateOf(tester, SetupCheckKind.project), 'blocks the start');
    expect(startEnabled(tester), isFalse);
    // Der Befund steht auf der Karte, nicht als Satz daneben. Der Code steht
    // genau einmal darauf, als Abzeichen; die Überschrift ist der übersetzte
    // Titel und nicht noch einmal die Nummer (CONVENTIONS 4.13).
    expect(find.text(DiagnosticCodes.noProjectFolder), findsOneWidget);
    expect(find.text('No project folder chosen'), findsOneWidget);
    expect(
      find.text(
        'No project folder chosen. The agent needs exactly one folder to '
        'work in.',
      ),
      findsOneWidget,
    );
    expect(reasonOf(tester), contains('Project folder'));
  });

  testWidgets('the machine list shows every line the daemon sent', (
    WidgetTester tester,
  ) async {
    await pumpApp(tester, client: FakeDaemonClient());
    await pressCtrl(tester, LogicalKeyboardKey.digit6);
    await tester.pump();

    expect(find.byType(DoctorList), findsOneWidget);
    // Neun Zeilen: elf minus die beiden, die eine eigene Zeile haben.
    for (final String id in fakeDoctorCheckIds) {
      final Finder title = find.byKey(Key('doctor-title-$id'));
      if (DoctorCheckId.ownRow.contains(id)) {
        expect(title, findsNothing, reason: id);
      } else {
        expect(title, findsOneWidget, reason: id);
      }
    }
    // Und die Überschrift verspricht genau die neun, die darunter stehen.
    expect(find.text('9 lines about this machine'), findsOneWidget);
    expect(find.text('11 lines about this machine'), findsNothing);
  });

  /// Akzeptanzkriterium 5, mit wirklich gehaltenen Anfragen und einem
  /// `Ctrl+1`, das wirklich wechselt.
  ///
  /// Beide Hälften brauchen mehr als das Vorhandensein des Abzeichens: Ein
  /// Zähler, der `0 held` sagt, weil nie ein Skript lief, bewiese nichts, und
  /// das Abzeichen steht als Geschwister über dem `IndexedStack` und ändert
  /// sich beim Abschnittswechsel ohnehin nicht.
  testWidgets('setup_keeps_header_and_ctrl_1_while_flows_are_held', (
    WidgetTester tester,
  ) async {
    await pumpApp(
      tester,
      client: FakeDaemonClient.burst(
        count: 3,
        spacing: const Duration(milliseconds: 20),
      ),
    );
    // Die drei Anfragen laufen in 60 ms ein. Danach noch ein paar Frames,
    // damit die einmalige Weiche des Starts (`ShellScreen._offerSetup`)
    // gelaufen ist, bevor der Test selbst Abschnitte wechselt.
    await tester.pump(const Duration(milliseconds: 100));
    for (int frame = 0; frame < 5; frame++) {
      await tester.pump();
    }
    await pressCtrl(tester, LogicalKeyboardKey.digit6);
    await tester.pump();
    expect(containerOf(tester).read(navigationProvider), Section.setup);
    expect(find.byType(SetupScreen), findsOneWidget);

    // Die Kopfzeile steht weiter, und sie zählt, was wirklich wartet.
    expect(heldBadge(tester), '3 held');

    await pressCtrl(tester, LogicalKeyboardKey.digit1);
    await tester.pump();
    expect(containerOf(tester).read(navigationProvider), Section.intercept);
    expect(find.byType(InterceptScreen), findsOneWidget);
    expect(heldBadge(tester), '3 held');
  });

  /// Das Ziel des Issues: Die App startet in den Setup-Screen, wenn eine der
  /// vier Zeilen nicht grün ist.
  ///
  /// Die Gegenprobe -- vier Zeilen ohne roten Befund führen in die
  /// Warteschlange -- steht in `setup_dead_ends_test.dart`
  /// (`the empty queue leads to the start`).
  testWidgets('the app opens the setup section when a check is red', (
    WidgetTester tester,
  ) async {
    await pumpApp(
      tester,
      client: FakeDaemonClient()..doctorReport = fakeDoctorFailing(),
    );
    await tester.pumpAndSettle();

    expect(containerOf(tester).read(navigationProvider), Section.setup);
    expect(find.byType(SetupScreen), findsOneWidget);
  });

  /// Akzeptanzkriterium 1, an dem Punkt, an dem der Mensch klickt: Auf dem
  /// gewöhnlichen Weg -- Nutzer-Unit, Standard-Socket -- trägt `DAEMON_001`
  /// `InstallService`, und der Bildschirm zeigt den Knopf, der sie anlegt und
  /// startet. Mit `--fake` oder `--socket` bliebe es beim Kopierbefehl, den
  /// `connection_failed_shows_setup` prüft.
  testWidgets('setup_offers_the_install_button_for_the_ordinary_daemon_001', (
    WidgetTester tester,
  ) async {
    await pumpApp(
      tester,
      client: FakeDaemonClient(
        infoFailure: ClientDiagnostics.daemonUnreachable(
          socketPath: FakeDaemonClient.defaultSocket,
          detail: 'connection refused',
        ),
      ),
    );

    expect(find.byType(SetupScreen), findsOneWidget);
    expect(find.byKey(const Key('setup-fix-install')), findsOneWidget);
    expect(find.text(installServiceCommand), findsOneWidget);
  });

  /// Ein laufender Versuch steht auf der Zeile, für die er läuft.
  ///
  /// Der Bildschirm ist derselbe wie vorher -- das ist die Zusage aus
  /// `the two second retry never flashes the splash` daneben, und sie bleibt.
  /// **Was darauf steht, ist die andere Frage.** Solange der Versuch läuft,
  /// den jemand ausgelöst hat, sagt die erste Zeile „asking" und ihr Knopf
  /// ruht; ohne das könnte jemand einen zweiten Versuch auf einen laufenden
  /// setzen und sähe dabei einen Befund, der von vorhin ist. Der Befund und
  /// seine Abhilfe bleiben trotzdem stehen: Von diesem Bildschirm aus wird
  /// der Dienst repariert (`docs/UX.md` 4.2, Fall 2, und 4.4).
  testWidgets('a retry somebody asked for rests its own button', (
    WidgetTester tester,
  ) async {
    final SlowlyFailingClient client = SlowlyFailingClient();
    await pumpApp(tester, client: client);
    await tester.pump(const Duration(milliseconds: 250));
    await tester.pump();
    expect(find.byType(SetupScreen), findsOneWidget);
    expect(stateOf(tester, SetupCheckKind.daemon), 'blocks the start');
    expect(retryEnabled(tester), isTrue);
    final int cards = find
        .text(DiagnosticCodes.daemonUnreachable)
        .evaluate()
        .length;
    expect(cards, isPositive);

    await tester.tap(find.byKey(const Key('setup-daemon-retry')));
    await tester.pump();
    final int tried = client.infoCalls;

    expect(stateOf(tester, SetupCheckKind.daemon), 'asking');
    expect(retryEnabled(tester), isFalse);
    // Und der Bildschirm bleibt, was er war, mit dem Befund darauf.
    expect(find.byType(Splash), findsNothing);
    expect(find.byType(SetupScreen), findsOneWidget);
    expect(
      find.text(DiagnosticCodes.daemonUnreachable),
      findsNWidgets(cards),
      reason: 'the reason and its remedy do not blink away under the attempt',
    );

    // Ein zweiter Druck geht nicht durch, solange der erste läuft.
    await tester.tap(
      find.byKey(const Key('setup-daemon-retry')),
      warnIfMissed: false,
    );
    await tester.pump();
    expect(client.infoCalls, tried);

    // Scheitert der Versuch, ist die Zeile wieder rot und der Knopf wieder da.
    await tester.pump(const Duration(milliseconds: 250));
    await tester.pump();
    expect(stateOf(tester, SetupCheckKind.daemon), 'blocks the start');
    expect(retryEnabled(tester), isTrue);
    expect(client.infoCalls, greaterThan(tried - 1));
  });

  /// Der Zwei-Sekunden-Takt sagt gar nichts.
  ///
  /// Er läuft von selbst, und eine Zeile, die alle zwei Sekunden zwischen
  /// „asking" und „blocks the start" wechselte, wäre dasselbe Flackern wie ein
  /// Splash alle zwei Sekunden -- nur eine Zeile tiefer. Angekündigt wird nur,
  /// was jemand ausgelöst hat.
  testWidgets('the two second retry never says asking', (
    WidgetTester tester,
  ) async {
    final SlowlyFailingClient client = SlowlyFailingClient();
    await pumpApp(
      tester,
      client: client,
      reconnect: const Duration(seconds: 2),
    );
    await tester.pump(const Duration(milliseconds: 250));
    await tester.pump();
    expect(find.byType(SetupScreen), findsOneWidget);

    int askingFrames = 0;
    for (int frame = 0; frame < 400; frame++) {
      await tester.pump(const Duration(milliseconds: 16));
      if (stateOf(tester, SetupCheckKind.daemon) == 'asking') {
        askingFrames++;
      }
    }

    expect(client.infoCalls, greaterThan(1));
    expect(askingFrames, 0);
    expect(retryEnabled(tester), isTrue);

    // Der Baum geht, damit der Takt aufhört; danach läuft der letzte Versuch
    // in Ruhe aus.
    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pump(const Duration(milliseconds: 300));
  });

  /// Ein zweiter Versuch nimmt den Bildschirm nicht weg.
  ///
  /// Der Splash gehört dem ersten Versuch des Laufs (`docs/UX.md` 4.2,
  /// Fall 1). Danach steht der Befund mit seiner Abhilfe darauf, und der
  /// Zwei-Sekunden-Takt, der die Zeile grün machen soll, darf genau diesen
  /// Bildschirm nicht alle zwei Sekunden fortblinken -- samt dem Knopf, der
  /// `humanitl daemon install` gerade fährt.
  testWidgets('the two second retry never flashes the splash', (
    WidgetTester tester,
  ) async {
    final SlowlyFailingClient client = SlowlyFailingClient();
    await pumpApp(
      tester,
      client: client,
      reconnect: const Duration(seconds: 2),
    );
    // Der erste Versuch dauert 200 ms; solange gehört der Bildschirm dem
    // Splash.
    expect(find.byType(Splash), findsOneWidget);
    await tester.pump(const Duration(milliseconds: 250));
    await tester.pump();
    expect(find.byType(SetupScreen), findsOneWidget);
    final State<SetupScreen> first = tester.state<State<SetupScreen>>(
      find.byType(SetupScreen),
    );

    int splashFrames = 0;
    for (int frame = 0; frame < 400; frame++) {
      await tester.pump(const Duration(milliseconds: 16));
      if (find.byType(Splash).evaluate().isNotEmpty) {
        splashFrames++;
      }
    }

    expect(client.infoCalls, greaterThan(1));
    expect(splashFrames, 0);
    expect(find.byType(SetupScreen), findsOneWidget);
    expect(
      identical(
        tester.state<State<SetupScreen>>(find.byType(SetupScreen)),
        first,
      ),
      isTrue,
      reason: 'the screen a fix is running on is never rebuilt from scratch',
    );

    // Der Baum geht, damit der Zwei-Sekunden-Takt aufhört; danach läuft der
    // letzte Versuch, der noch unterwegs ist, in Ruhe aus.
    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pump(const Duration(milliseconds: 300));
  });

  /// Akzeptanzkriterium 1 von HUM-044, in einem Lauf: der Klick **und** die
  /// Rückkehr.
  ///
  /// Die vier Aussagen des Kriteriums waren einzeln gemessen — der Befund mit
  /// seiner Aktion, der Befehl ohne Shell, die Zeile, die von selbst grün wird
  /// —, die Verkettung nicht: `FixControl` bekam seinen Runner nur als
  /// Parameter, und der Anwendungsbaum reichte keinen durch. Seit
  /// [serviceInstallerProvider] gibt es die Naht, und dieser Test läuft durch
  /// sie: Klick auf den Knopf, der Installer stellt den Dienst an, und der
  /// Zwei-Sekunden-Versuch bringt den Bildschirm zurück — innerhalb der vier
  /// Sekunden, die das Kriterium nennt.
  testWidgets('the install button starts the daemon and the line turns green', (
    WidgetTester tester,
  ) async {
    final UninstalledDaemonClient client = UninstalledDaemonClient();
    await pumpApp(
      tester,
      client: client,
      reconnect: const Duration(seconds: 2),
      overrides: <Override>[
        serviceInstallerProvider.overrideWithValue(() async {
          client.installed = true;
          return null;
        }),
      ],
    );

    // Vorher: kein Daemon, der Bildschirm des Aufbaus, und der Knopf, der ihn
    // anlegt.
    expect(find.byType(SetupScreen), findsOneWidget);
    expect(find.text('DAEMON_001'), findsWidgets);
    expect(find.byKey(const Key('setup-fix-install')), findsOneWidget);
    expect(client.installed, isFalse);

    await tester.tap(find.byKey(const Key('setup-fix-install')));
    await tester.pump();
    await tester.pump();
    expect(client.installed, isTrue, reason: 'the click ran the installer');

    // Und nachher: Der Versuch, der alle zwei Sekunden läuft, findet den
    // Dienst. Niemand fasst das Fenster dafür an.
    await tester.pump(const Duration(seconds: 2));
    await tester.pump();
    await tester.pump();

    expect(find.byType(ShellScreen), findsOneWidget);
    expect(find.byType(SetupScreen), findsNothing);
  });
}
