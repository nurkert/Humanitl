// Die Befunde, die der Client selbst erhebt (HUM-044, Tabelle der
// Diagnostics): Welche Aktion an `DAEMON_001` und `DAEMON_002` hängt, ist
// keine Geschmacksfrage -- ohne sie steht dort eine Versionsnummer und ein
// Dienst, der nicht läuft, und niemand kann etwas tun (`docs/UX.md` 4.4).

import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_diagnostics.dart';

void main() {
  group('DAEMON_001', () {
    test('offers to install and start the user unit', () {
      final Diagnostic diagnostic = ClientDiagnostics.daemonUnreachable(
        socketPath: '/run/user/1000/humanitl/daemon.sock',
        detail: 'connection refused',
      );

      // Rot, sobald die Zeile wieder nur einen Befehl zum Kopieren anbietet.
      expect(diagnostic.fix, const FixAction.installService());
      // Der Befehl steht im `why` und nicht als zweite Aktion: Die Leitung
      // trägt genau einen `FixAction`.
      expect(diagnostic.why, contains('systemctl --user start humanitld'));
      expect(diagnostic.why, contains('/run/user/1000/humanitl/daemon.sock'));
      // Der Daemon ist ein Nutzerdienst und wird nie mit `sudo` installiert.
      expect(diagnostic.why, isNot(contains('sudo')));
    });

    test('keeps the command when a daemon was named by hand', () {
      // Mit `--fake` oder `--socket` meint der Mensch einen bestimmten Daemon.
      // Die Nutzer-Unit anzulegen startete einen anderen auf einem anderen
      // Socket und beantwortete eine Frage, die niemand gestellt hat.
      final Diagnostic fake = ClientDiagnostics.daemonUnreachable(
        socketPath: '/run/user/1000/humanitl/daemon.sock',
        fake: true,
      );
      expect(
        fake.fix,
        const FixAction.copyCommand(
          command: ClientDiagnostics.startFakeCommand,
        ),
      );
      expect(fake.why, isNot(contains('systemctl')));

      final Diagnostic bespoke = ClientDiagnostics.daemonUnreachable(
        socketPath: '/tmp/own.sock',
        socketFlag: true,
      );
      expect(
        bespoke.fix,
        const FixAction.copyCommand(
          command: 'humanitld --socket /tmp/own.sock',
        ),
      );
    });
  });

  group('DAEMON_002', () {
    test('leads to the page that has both halves of one release', () {
      const DaemonInfo info = DaemonInfo(
        daemonVersion: '0.4.0',
        protoMajor: 2,
        protoMinor: 0,
      );

      final Diagnostic diagnostic = ClientDiagnostics.protoIncompatible(info);

      // Rot, sobald der Befund wieder ohne Aktion herauskommt.
      expect(
        diagnostic.fix,
        const FixAction.openUrl(url: ClientDiagnostics.releasesUrl),
      );
      expect(ClientDiagnostics.releasesUrl, contains('/releases'));
      expect(diagnostic.severity, Severity.blocking);
    });
  });
}
