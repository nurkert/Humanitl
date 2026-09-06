// `FixControl` (HUM-106): Was ein `Diagnostic` als Abhilfe vorschlägt, muss
// die Oberfläche entweder ausführen oder gar nicht anbieten.
//
// `docs/UX.md` 4.4 sagt, ein `Diagnostic` mit `FixAction` und ohne sichtbare
// Aktion sei ein Defekt; `backlog/CONVENTIONS.md` 4.13 sagt, ein Control, das
// etwas verspricht, was nicht geschieht, sei schlimmer als keines. Für
// `SetEnv` liegt genau ein ausführbarer Teil dazwischen: den Befehl kopieren.
// Das Schreiben in die Konfiguration braucht `SetConfig` und kommt mit
// HUM-069.

import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/fix_control.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/l10n/l10n.dart';

import 'shell_command_test.dart' show shellWords;

/// Ein Wirt mit Theme und Sprache, so schmal wie das Control es braucht.
Widget host(Widget child) => ProviderScope(
  // Seit HUM-044 liest `FixControl` seinen Installer aus
  // `serviceInstallerProvider`, wenn kein Parameter ihn setzt. Ohne Bereich
  // gäbe es keinen Provider, den es lesen könnte; die Vorgabe bleibt
  // `runInstallService`, und genau die misst
  // `install_service_without_a_stand_in_runs_the_real_command`.
  child: WidgetsApp(
    color: HColors.bg0,
    debugShowCheckedModeBanner: false,
    locale: const Locale('en'),
    localizationsDelegates: AppLocalizations.localizationsDelegates,
    supportedLocales: AppLocalizations.supportedLocales,
    onGenerateTitle: (BuildContext context) => 'fix control',
    builder: (BuildContext context, Widget? _) => HTheme(
      tokens: HTokens.dark,
      child: Align(
        alignment: Alignment.topLeft,
        child: SizedBox(width: 480, child: child),
      ),
    ),
  ),
);

/// Fängt ab, was in die Zwischenablage geschrieben wird.
List<String> captureClipboard(WidgetTester tester) {
  final List<String> written = <String>[];
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
    SystemChannels.platform,
    (MethodCall call) async {
      if (call.method == 'Clipboard.setData') {
        written.add(
          (call.arguments as Map<Object?, Object?>)['text']! as String,
        );
      }
      return null;
    },
  );
  addTearDown(
    () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      SystemChannels.platform,
      null,
    ),
  );
  return written;
}

void main() {
  testWidgets('set_env_offers_the_export_command', (WidgetTester tester) async {
    final List<String> clipboard = captureClipboard(tester);
    await tester.pumpWidget(
      host(
        const FixControl(
          fix: FixAction.setEnv(
            key: 'CURL_CA_BUNDLE',
            value: '/etc/humanitl/ca.crt',
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    // Das Abzeichen benennt weiterhin, was zu tun ist ...
    expect(find.text('Set CURL_CA_BUNDLE'), findsOneWidget);
    // ... und darunter steht der Befehl, den diese Anwendung ausführen kann.
    // Rot, sobald der Zweig wieder nur ein `HBadge` zeichnet.
    expect(
      find.text('export CURL_CA_BUNDLE=/etc/humanitl/ca.crt'),
      findsOneWidget,
    );
    expect(find.text('Copy export command'), findsOneWidget);

    await tester.tap(find.byKey(const Key('setup-fix-copy')));
    await tester.pump();

    expect(clipboard, <String>['export CURL_CA_BUNDLE=/etc/humanitl/ca.crt']);
    // Ein Klick zeigt eine sichtbare Reaktion (`docs/UX.md` 6, Punkt 4).
    expect(find.text('Copied'), findsOneWidget);
    expect(find.text('Copy export command'), findsNothing);

    // Nach dem Rückmeldefenster steht wieder das Angebot da.
    await tester.pump(HMotion.copyFeedback);
    await tester.pumpAndSettle();
    expect(find.text('Copy export command'), findsOneWidget);
  });

  testWidgets('set_env_quotes_a_hostile_value', (WidgetTester tester) async {
    // Der Wert kommt ueber die Leitung. Ungequotet waere `; rm -rf ~` ein
    // zweiter Befehl in der Zwischenablage des Nutzers.
    //
    // Geprueft wird nicht die Schreibweise, sondern was eine Shell daraus
    // liest: genau drei Woerter. Rot, sobald der Zweig wieder interpoliert.
    const String value = "a'; rm -rf ~; '";
    final List<String> clipboard = captureClipboard(tester);
    await tester.pumpWidget(
      host(
        const FixControl(
          fix: FixAction.setEnv(key: 'CURL_CA_BUNDLE', value: value),
        ),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const Key('setup-fix-copy')));
    await tester.pump();

    expect(clipboard, hasLength(1));
    expect(shellWords(clipboard.single), <String>[
      'export',
      'CURL_CA_BUNDLE=$value',
    ]);
    // Angezeigt und kopiert ist dieselbe Zeichenkette; wer nur eine der
    // beiden saeuberte, haette den Fehler bloss verschoben.
    expect(find.text(clipboard.single), findsOneWidget);

    await tester.pump(HMotion.copyFeedback);
    await tester.pumpAndSettle();
  });

  testWidgets('set_env_with_a_line_break_offers_no_command', (
    WidgetTester tester,
  ) async {
    // Quotieren genuegte hier nicht: Ein Terminal ohne Klammer-Einfuegen
    // schickte die erste Zeile ab. Also kein Knopf, sondern der Grund.
    // Rot, sobald die Weigerung faellt.
    await tester.pumpWidget(
      host(
        const FixControl(
          fix: FixAction.setEnv(
            key: 'CURL_CA_BUNDLE',
            value: '/etc/ca.crt\nrm -rf ~',
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.byKey(const Key('setup-fix-copy')), findsNothing);
    expect(find.byKey(const Key('setup-fix-no-command')), findsOneWidget);
    // Und zwar der Satz zum Zeilenumbruch, nicht der zum Schluessel. Ohne
    // diese Zusicherung blieben beide Faelle gruen, wenn man die Zweige
    // vertauscht.
    expect(find.textContaining('spans more than one line'), findsOneWidget);
    // Das Abzeichen bleibt: Was zu tun ist, steht weiter da.
    expect(find.text('Set CURL_CA_BUNDLE'), findsOneWidget);
  });

  testWidgets('set_env_with_a_bad_key_offers_no_command', (
    WidgetTester tester,
  ) async {
    // Kein Ersatzschluessel, kein Platzhalter: gar kein Befehl.
    await tester.pumpWidget(
      host(
        const FixControl(
          fix: FixAction.setEnv(key: 'CURL;rm -rf ~', value: '/etc/ca.crt'),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.byKey(const Key('setup-fix-copy')), findsNothing);
    expect(find.byKey(const Key('setup-fix-no-command')), findsOneWidget);
    expect(
      find.textContaining('is not a name a shell can assign to'),
      findsOneWidget,
    );
    expect(find.textContaining('spans more than one line'), findsNothing);
    expect(find.textContaining('export'), findsNothing);
  });

  testWidgets('the_copy_button_takes_the_key_it_is_given', (
    WidgetTester tester,
  ) async {
    // Zwei Karten im selben Streifen truegen sonst denselben Schluessel.
    await tester.pumpWidget(
      host(
        const FixControl(
          fix: FixAction.copyCommand(command: 'humanitld'),
          copyKey: Key('own-copy'),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('own-copy')), findsOneWidget);
    expect(find.byKey(const Key('setup-fix-copy')), findsNothing);
  });

  testWidgets('no_fix_draws_nothing', (WidgetTester tester) async {
    await tester.pumpWidget(host(const FixControl(fix: null)));
    await tester.pumpAndSettle();
    expect(find.byType(HButton), findsNothing);
    expect(find.byType(HBadge), findsNothing);
  });

  testWidgets('install_service_runs_the_command_that_does_it', (
    WidgetTester tester,
  ) async {
    // Die Tabelle von HUM-044 gibt `DAEMON_001` die Aktion `InstallService`,
    // und das Akzeptanzkriterium ist gemessen: „Klick auf Fix installiert und
    // startet die Unit". Rot, sobald der Zweig wieder nur kopiert.
    int runs = 0;
    await tester.pumpWidget(
      host(
        FixControl(
          fix: const FixAction.installService(),
          installService: () async {
            runs++;
            return null;
          },
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.byType(HBadge), findsOneWidget);
    // Der Befehl steht sichtbar daneben, bevor er laeuft.
    expect(find.text(installServiceCommand), findsOneWidget);
    // Nie mit `sudo`: der Daemon ist ein Nutzerdienst.
    expect(installServiceCommand, isNot(contains('sudo')));

    await tester.tap(find.byKey(const Key('setup-fix-install')));
    await tester.pumpAndSettle();

    expect(runs, 1);
    // Ohne Fehlschlag bleibt die Zeile ohne Befund und ohne Kopierknopf: Die
    // Aktion ist eine.
    expect(find.byKey(const Key('setup-fix-install-failed')), findsNothing);
    expect(find.byKey(const Key('setup-fix-copy')), findsNothing);
  });

  /// Und ohne eingesetzte Fassung läuft der wirkliche Befehl.
  ///
  /// Jeder andere Test dieses Knopfs setzt seine eigene Fassung ein, damit
  /// kein Widget-Test einen Prozess startet -- und genau dadurch war die
  /// Vorgabe in `_install` von keinem Test gedeckt: Ein `installService`, das
  /// still `null` liefert, ließe den Knopf nichts tun, keine Karte zeigen und
  /// die Zeile rot stehen, während die Testsammlung grün bleibt.
  ///
  /// Hier hängt keine Fassung daneben, also läuft [runInstallService]. Neben
  /// dem Testläufer liegt keine Kommandozeile namens `humanitl`, also verweigert
  /// [installServiceCandidate] den Start und der Rückgabewert ist der Befund,
  /// den die Karte zeigt. Es startet dabei kein Prozess.
  testWidgets('install_service_without_a_stand_in_runs_the_real_command', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      host(const FixControl(fix: FixAction.installService())),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const Key('setup-fix-install')));
    await tester.pumpAndSettle();

    expect(
      find.byKey(const Key('setup-fix-install-failed')),
      findsOneWidget,
      reason: 'the default runner answers, and its answer is drawn',
    );
  });

  testWidgets('install_service_says_it_when_the_command_fails', (
    WidgetTester tester,
  ) async {
    // Ein Fehlschlag ist ein `Diagnostic` wie jeder andere, kein stilles
    // Nichts (`docs/UX.md` 4.4). Rot, sobald der Zweig ihn verschluckt.
    final List<String> written = captureClipboard(tester);
    await tester.pumpWidget(
      host(
        FixControl(
          fix: const FixAction.installService(),
          installService: () async => const Diagnostic(
            code: DiagnosticCodes.daemonUnreachable,
            severity: Severity.error,
            why: 'systemctl --user daemon-reload exited with 1',
            fix: FixAction.copyCommand(command: installServiceCommand),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const Key('setup-fix-install')));
    await tester.pumpAndSettle();

    expect(find.byKey(const Key('setup-fix-install-failed')), findsOneWidget);
    expect(
      find.textContaining('systemctl --user daemon-reload exited with 1'),
      findsOneWidget,
    );

    // Und danach steht der Weg von Hand da: derselbe Befehl zum Kopieren.
    await tester.tap(find.byKey(const Key('setup-fix-copy')));
    await tester.pump();
    expect(written, <String>[installServiceCommand]);
    await tester.pump(HMotion.copyFeedback);
    await tester.pumpAndSettle();
  });

  group('installServiceCandidate', () {
    // 0o755 ueberall: die Datei und jedes Verzeichnis darueber gehoeren dem
    // Eigentuemer allein. Wer einen Modus pruefen will, setzt ihn je Test.
    int owned(String path) => 0x1ED;

    String? found(
      String running,
      bool Function(String) exists, [
      int Function(String)? modeOf,
    ]) => installServiceCandidate(
      runningExecutable: running,
      exists: exists,
      modeOf: modeOf ?? owned,
    ).path;

    String? why(
      String running,
      bool Function(String) exists, [
      int Function(String)? modeOf,
    ]) => installServiceCandidate(
      runningExecutable: running,
      exists: exists,
      modeOf: modeOf ?? owned,
    ).refusal;

    test('takes the command line from bin, as the package lays it out', () {
      // HUM-053 (`backlog/sprint-4.md`): das Flutter-Binary liegt als
      // `/usr/lib/humanitl/humanitl`, die Kommandozeile eine Ebene tiefer.
      expect(
        found(
          '/usr/lib/humanitl/humanitl',
          (String p) => p == '/usr/lib/humanitl/bin/humanitl',
        ),
        '/usr/lib/humanitl/bin/humanitl',
      );
    });

    test('takes a sibling when the bundle keeps no bin directory', () {
      expect(
        found(
          '/opt/humanitl/humanitl-app',
          (String p) => p == '/opt/humanitl/humanitl',
        ),
        '/opt/humanitl/humanitl',
      );
    });

    test('prefers bin over a sibling when both are there', () {
      expect(
        found('/opt/humanitl/humanitl-app', (String _) => true),
        '/opt/humanitl/bin/humanitl',
      );
    });

    test('never starts the application itself', () {
      expect(
        found(
          '/usr/lib/humanitl/humanitl',
          (String p) => p == '/usr/lib/humanitl/humanitl',
        ),
        isNull,
      );
    });

    test('never falls back to PATH', () {
      expect(found('/opt/humanitl/humanitl-app', (String _) => false), isNull);
    });

    test('refuses a binary the group may write', () {
      // Nur das Gruppenbit, 0o775. Mit 0o777 in beiden Faellen waere die Maske
      // nur zur Haelfte gehalten: eine Mutation, die eines der beiden Bits
      // streicht, bliebe gruen.
      expect(
        found(
          '/opt/humanitl/humanitl-app',
          (String p) => p == '/opt/humanitl/bin/humanitl',
          (String p) => p == '/opt/humanitl/bin/humanitl' ? 0x1FD : 0x1ED,
        ),
        isNull,
      );
    });

    test('refuses a binary all others may write', () {
      // Nur das Andere-Bit, 0o757.
      expect(
        found(
          '/opt/humanitl/humanitl-app',
          (String p) => p == '/opt/humanitl/bin/humanitl',
          (String p) => p == '/opt/humanitl/bin/humanitl' ? 0x1EF : 0x1ED,
        ),
        isNull,
      );
    });

    test('refuses a grandparent the others may write', () {
      // Der Fall, den der Doc-Kommentar als Grund nennt: Wer
      // `/opt/humanitl` schreiben darf, ersetzt `bin` samt Inhalt und waehlt
      // die Rechte darin selbst. Eine Pruefung, die beim Elternverzeichnis
      // aufhoert, sieht davon nichts.
      expect(
        found(
          '/opt/humanitl/humanitl-app',
          (String p) => p == '/opt/humanitl/bin/humanitl',
          (String p) => p == '/opt/humanitl' ? 0x1FF : 0x1ED,
        ),
        isNull,
      );
    });

    test('a sticky directory is not a writable one', () {
      // `/tmp` traegt 1777: jeder darf anlegen, nur der Eigentuemer eines
      // Eintrags darf ihn ersetzen. Ein AppImage haengt darunter, und ohne
      // diese Ausnahme verloere es den Knopf.
      expect(
        found(
          '/tmp/.mount_hum42/humanitl-app',
          (String p) => p == '/tmp/.mount_hum42/bin/humanitl',
          (String p) => p == '/tmp' ? 0x3FF : 0x1ED,
        ),
        '/tmp/.mount_hum42/bin/humanitl',
      );
    });

    test('a path it cannot measure is a path it does not run', () {
      expect(
        found(
          '/opt/humanitl/humanitl-app',
          (String _) => true,
          (String _) => throw const FileSystemException('gone'),
        ),
        isNull,
      );
    });

    test('the refusal says which of the three it was', () {
      // Der Satz muss messen, nicht raten: eine Datei, die daliegt und nur
      // wegen ihrer Rechte verworfen wurde, ist etwas anderes als eine, die
      // fehlt (CONVENTIONS 4.13).
      expect(
        why('/opt/humanitl/humanitl-app', (String _) => false),
        contains('lies in /opt/humanitl/bin or beside'),
      );
      expect(
        why(
          '/opt/humanitl/humanitl-app',
          (String p) => p == '/opt/humanitl/bin/humanitl',
          (String p) => p == '/opt/humanitl' ? 0x1FF : 0x1ED,
        ),
        contains('may be written by somebody other than you'),
      );
      expect(
        why(
          '/opt/humanitl/humanitl-app',
          (String _) => true,
          (String _) => throw const FileSystemException('gone'),
        ),
        contains('could not be read'),
      );
    });
  });

  group('runInstallService', () {
    test('runs the binary with its arguments as a list', () async {
      String? seenExecutable;
      List<String>? seenArguments;
      final Diagnostic? failure = await runInstallService(
        resolve: () => (path: '/opt/humanitl/humanitl', refusal: null),
        run: (String executable, List<String> arguments) async {
          seenExecutable = executable;
          seenArguments = arguments;
          return ProcessResult(1, 0, '', '');
        },
      );

      expect(failure, isNull);
      expect(seenExecutable, '/opt/humanitl/humanitl');
      // Eine Liste, keine Zeile: Nichts davon geht durch eine Shell.
      expect(seenArguments, <String>['daemon', 'install']);
      expect(seenArguments, isNot(contains('sudo')));
    });

    test('turns a non-zero exit into a diagnostic with the reason', () async {
      final Diagnostic? failure = await runInstallService(
        resolve: () => (path: '/opt/humanitl/humanitl', refusal: null),
        run: (String executable, List<String> arguments) async =>
            ProcessResult(1, 3, '', 'DAEMON_005: the unit belongs to somebody'),
      );

      expect(failure, isNotNull);
      expect(failure!.code, DiagnosticCodes.daemonUnreachable);
      expect(failure.why, contains('exited with 3'));
      expect(failure.why, contains('DAEMON_005'));
      expect(
        failure.fix,
        const FixAction.copyCommand(command: installServiceCommand),
      );
    });

    test('says so when no command line was found', () async {
      final Diagnostic? failure = await runInstallService(
        resolve: () => (path: null, refusal: 'nothing usable was found'),
        run: (String executable, List<String> arguments) async =>
            throw StateError('must not run'),
      );

      expect(failure, isNotNull);
      // Der Satz der Absage wird durchgereicht, nicht durch einen eigenen
      // ersetzt: Was gemessen wurde, weiss der Aufloeser, nicht dieser Aufruf.
      expect(failure!.why, contains('nothing usable was found'));
    });

    test('turns a process that cannot start into a diagnostic', () async {
      final Diagnostic? failure = await runInstallService(
        resolve: () => (path: '/opt/humanitl/humanitl', refusal: null),
        run: (String executable, List<String> arguments) async =>
            throw const ProcessException('/opt/humanitl/humanitl', <String>[]),
      );

      expect(failure, isNotNull);
      expect(failure!.why, contains('could not be started'));
    });
  });

  testWidgets('change_setting_stays_a_badge_without_a_button', (
    WidgetTester tester,
  ) async {
    // `SetConfig` ist bis HUM-069 `unimplemented`. Ein Knopf hier wäre ein
    // Versprechen ohne Wirkung; das Abzeichen sagt nur, was zu tun ist.
    // Rot, sobald jemand dieser Aktion einen Knopf gibt, bevor der RPC steht.
    await tester.pumpWidget(
      host(
        const FixControl(
          fix: FixAction.changeSetting(key: 'llm.endpoint', value: 'off'),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.byType(HBadge), findsOneWidget);
    expect(find.byType(HButton), findsNothing);
  });
}
