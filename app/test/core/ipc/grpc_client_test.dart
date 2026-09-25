// Unit-Tests der Fehlerübersetzung des gRPC-Clients (HUM-019).

import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:grpc/grpc.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ipc/generated/humanitl/v1/humanitl.pb.dart' as pb;
import 'package:humanitl/core/ipc/generated/humanitl/v1/humanitl.pbgrpc.dart'
    as pbgrpc;
import 'package:protobuf/well_known_types/google/protobuf/empty.pb.dart';
import 'package:humanitl/core/ipc/client_diagnostics.dart';
import 'package:humanitl/core/ipc/grpc_daemon_client.dart';

const String socket = '/run/user/1000/humanitl/daemon.sock';
const String token = '/run/user/1000/humanitl/token';

Diagnostic translate(GrpcError error) =>
    diagnosticFromGrpcError(error, socketPath: socket, tokenPath: token);

/// Verpackt einen Befund so, wie tonic ihn in die Trailer schreibt:
/// Standard-Alphabet ohne Padding.
String tonicTrailer(pb.Diagnostic diagnostic) =>
    base64.encode(diagnostic.writeToBuffer()).replaceAll('=', '');

/// Ein Daemon, der nur `GetInfo` kennt (HUM-164).
class InfoOnly extends pbgrpc.HumanitlServiceBase {
  /// Die Fassung, die er meldet.
  static const String version = 'hum164-woken';

  @override
  Future<pb.Info> getInfo(ServiceCall call, Empty request) async =>
      pb.Info()..daemonVersion = version;

  // Die übrigen Methoden der Schnittstelle ruft dieser Test nie.
  @override
  dynamic noSuchMethod(Invocation invocation) =>
      throw UnimplementedError('${invocation.memberName}');
}

void main() {
  group('grpc_client_translates_status', () {
    test('UNAVAILABLE is DAEMON_001 with the socket in why', () {
      final Diagnostic diagnostic = translate(
        const GrpcError.unavailable('Connection refused'),
      );
      expect(diagnostic.code, DiagnosticCodes.daemonUnreachable);
      expect(diagnostic.severity, Severity.error);
      expect(diagnostic.why, contains(socket));
      expect(diagnostic.why, contains('UNAVAILABLE'));
      expect(diagnostic.why, contains('Connection refused'));
      // Seit HUM-044 legt die Zeile die Nutzer-Unit selbst an, statt einen
      // Befehl zum Kopieren anzubieten; der Befehl steht im `why`
      // (`backlog/sprint-3.md`, Tabelle der Diagnostics).
      expect(diagnostic.fix, const FixAction.installService());
      expect(diagnostic.why, contains(ClientDiagnostics.startUnitCommand));
    });

    test('UNAUTHENTICATED is IPC_001 with the token path in why', () {
      final Diagnostic diagnostic = translate(
        const GrpcError.unauthenticated('bad token'),
      );
      expect(diagnostic.code, DiagnosticCodes.tokenInvalid);
      expect(diagnostic.why, contains(token));
    });

    test(
      'any other status without details is DAEMON_001 naming the status',
      () {
        for (final GrpcError error in <GrpcError>[
          const GrpcError.deadlineExceeded(),
          const GrpcError.internal('boom'),
          const GrpcError.unknown(),
          const GrpcError.custom(4242),
        ]) {
          final Diagnostic diagnostic = translate(error);
          expect(diagnostic.code, DiagnosticCodes.daemonUnreachable);
          expect(diagnostic.why, contains(grpcStatusName(error.code)));
        }
        expect(grpcStatusName(4242), 'STATUS_4242');
      },
    );

    test('a diagnostic in the trailers wins, code and fix included', () {
      final pb.Diagnostic shipped = pb.Diagnostic()
        ..code = 'IPC_003'
        ..severity = pb.Severity.SEVERITY_WARNING
        ..title = 'Flow nicht mehr gehalten'
        ..why = 'flow 018f… was decided at 12:00'
        ..fix = (pb.FixAction()..copyCommand = 'humanitl flows show 018f…')
        ..docsUrl = 'https://example.invalid/DIAGNOSTICS.md#ipc_003';
      final GrpcError error = GrpcError.custom(
        StatusCode.failedPrecondition,
        'flow not held',
        const [],
        null,
        <String, String>{statusDetailsTrailer: tonicTrailer(shipped)},
      );

      final Diagnostic diagnostic = translate(error);

      expect(diagnostic.code, 'IPC_003');
      expect(diagnostic.severity, Severity.warning);
      expect(diagnostic.title, 'Flow nicht mehr gehalten');
      expect(diagnostic.why, contains('decided at 12:00'));
      expect(
        diagnostic.fix,
        const FixAction.copyCommand(command: 'humanitl flows show 018f…'),
      );
      expect(diagnostic.docsUrl, endsWith('#ipc_003'));
    });

    test('the url-safe alphabet with padding decodes as well', () {
      final pb.Diagnostic shipped = pb.Diagnostic()
        ..code = 'DAEMON_003'
        ..severity = pb.Severity.SEVERITY_ERROR
        ..why = 'socket busy';
      final String url = base64Url.encode(shipped.writeToBuffer());
      final Diagnostic? decoded = diagnosticFromTrailers(<String, String>{
        statusDetailsTrailer: url,
      });
      expect(decoded?.code, 'DAEMON_003');
    });

    test('garbage in the trailer falls back to the status', () {
      for (final String raw in <String>['%%%', 'AAAA', '']) {
        final Diagnostic diagnostic = translate(
          GrpcError.custom(
            StatusCode.unavailable,
            'down',
            const [],
            null,
            <String, String>{statusDetailsTrailer: raw},
          ),
        );
        expect(diagnostic.code, DiagnosticCodes.daemonUnreachable, reason: raw);
      }
      expect(diagnosticFromTrailers(null), isNull);
      expect(diagnosticFromTrailers(const <String, String>{}), isNull);
    });

    test('a missing token file is DAEMON_001 naming the token path', () async {
      // Der Daemon schreibt das Token beim Start und löscht es beim Ende:
      // kein Token heißt kein Daemon, nicht falsches Token.
      final Directory dir = Directory.systemTemp.createTempSync('humanitl-');
      addTearDown(() => dir.deleteSync(recursive: true));
      // Das Verzeichnis ist privat wie beim Daemon (HUM-212); es fehlt nur
      // das Token.
      Process.runSync('chmod', <String>['700', dir.path]);
      final GrpcDaemonClient client = GrpcDaemonClient(
        socketPath: '${dir.path}/daemon.sock',
        tokenPath: '${dir.path}/token',
        callTimeout: const Duration(seconds: 2),
      );
      addTearDown(client.close);
      await expectLater(
        client.getInfo(),
        throwsA(
          isA<DaemonException>()
              .having((e) => e.code, 'code', DiagnosticCodes.daemonUnreachable)
              .having((e) => e.diagnostic.why, 'why', contains('/token'))
              // Ohne Socket gibt es nichts zu wecken (HUM-164).
              .having(
                (e) => e.diagnostic.why,
                'why',
                contains('nothing listens on'),
              ),
        ),
      );
    });

    test('a dead socket is DAEMON_001 over the real channel', () async {
      // Fallstricke: `InternetAddress(path, type: unix)` mit `port: 0`; die
      // Verbindung scheitert am fehlenden Socket, nicht an einem TCP-Versuch.
      final Directory dir = Directory.systemTemp.createTempSync('humanitl-');
      addTearDown(() => dir.deleteSync(recursive: true));
      File('${dir.path}/token').writeAsStringSync('secret\n');
      // Wie der Daemon: Verzeichnis 0700 und Token 0600, sonst traut der
      // Client ihnen nicht (HUM-212).
      Process.runSync('chmod', <String>['700', dir.path]);
      Process.runSync('chmod', <String>['600', '${dir.path}/token']);
      final GrpcDaemonClient client = GrpcDaemonClient(
        socketPath: '${dir.path}/daemon.sock',
        tokenPath: '${dir.path}/token',
        callTimeout: const Duration(seconds: 2),
        fake: true,
      );
      addTearDown(client.close);
      await expectLater(
        client.getInfo(),
        throwsA(
          isA<DaemonException>()
              .having((e) => e.code, 'code', DiagnosticCodes.daemonUnreachable)
              .having(
                (e) => e.diagnostic.why,
                'why',
                contains('${dir.path}/daemon.sock'),
              )
              .having(
                (e) => e.diagnostic.fix,
                'fix',
                const FixAction.copyCommand(
                  command: 'humanitld --fake fixtures/sessions/mixed.jsonl',
                ),
              ),
        ),
      );
      // Auch der Stream endet mit demselben Befund.
      await expectLater(
        client.subscribe().toList(),
        throwsA(
          isA<DaemonException>().having(
            (e) => e.code,
            'code',
            DiagnosticCodes.daemonUnreachable,
          ),
        ),
      );
    });

    // HUM-164: Hinter `humanitld.socket` läuft noch kein Daemon, und es gibt
    // kein Token. Der Client öffnet den Socket trotzdem; erst diese
    // Verbindung weckt den „Daemon", der dann den Socket übernimmt und sein
    // Token schreibt. Ohne Weckruf käme keine Verbindung an, und der Aufruf
    // endete mit DAEMON_001.
    test('a socket without a token wakes the daemon, and the call goes '
        'through', () async {
      final Directory dir = Directory.systemTemp.createTempSync('hum164-');
      addTearDown(() => dir.deleteSync(recursive: true));
      Process.runSync('chmod', <String>['700', dir.path]);
      final String socketPath = '${dir.path}/daemon.sock';
      final InternetAddress address = InternetAddress(
        socketPath,
        type: InternetAddressType.unix,
      );
      final ServerSocket activator = await ServerSocket.bind(address, 0);
      final Server server = Server.create(services: <Service>[InfoOnly()]);
      addTearDown(server.shutdown);
      final List<Socket> held = <Socket>[];
      addTearDown(() {
        for (final Socket socket in held) {
          socket.destroy();
        }
      });
      final Future<void> woken = activator.first.then((Socket first) async {
        // Wie systemd: Die erste Verbindung startet den Dienst, er übernimmt
        // den Socket und schreibt danach sein Token, `0600` vor dem Umbenennen.
        held.add(first);
        await activator.close();
        if (File(socketPath).existsSync()) {
          File(socketPath).deleteSync();
        }
        await server.serve(address: address, port: 0);
        final File staged = File('${dir.path}/token.new')
          ..writeAsStringSync('secret\n');
        Process.runSync('chmod', <String>['600', staged.path]);
        staged.renameSync('${dir.path}/token');
      });
      final GrpcDaemonClient client = GrpcDaemonClient(
        socketPath: socketPath,
        tokenPath: '${dir.path}/token',
        callTimeout: const Duration(seconds: 5),
      );
      addTearDown(client.close);

      // Ein Fehler wird zum Wert, damit die Erwartung ihn als Abweichung
      // meldet und nicht als geworfene Ausnahme.
      final Object answer = await client
          .getInfo()
          .timeout(const Duration(seconds: 20))
          .then<Object>((DaemonInfo info) => info, onError: (Object e) => e);
      expect(
        answer,
        isA<DaemonInfo>().having(
          (DaemonInfo info) => info.daemonVersion,
          'daemonVersion',
          InfoOnly.version,
        ),
      );
      await woken;
    });

    // Die Frist aus den Fallstricken: Nimmt der Socket an, aber kein Daemon
    // schreibt je ein Token, endet der Aufruf mit DAEMON_001, statt zu hängen.
    test('a socket that wakes nobody ends at the deadline', () async {
      final Directory dir = Directory.systemTemp.createTempSync('hum164-');
      addTearDown(() => dir.deleteSync(recursive: true));
      Process.runSync('chmod', <String>['700', dir.path]);
      final String socketPath = '${dir.path}/daemon.sock';
      final ServerSocket silent = await ServerSocket.bind(
        InternetAddress(socketPath, type: InternetAddressType.unix),
        0,
      );
      addTearDown(silent.close);
      final List<Socket> held = <Socket>[];
      silent.listen(held.add);
      addTearDown(() {
        for (final Socket socket in held) {
          socket.destroy();
        }
      });
      final GrpcDaemonClient client = GrpcDaemonClient(
        socketPath: socketPath,
        tokenPath: '${dir.path}/token',
        callTimeout: const Duration(seconds: 2),
        wakeTimeout: const Duration(milliseconds: 300),
      );
      addTearDown(client.close);

      await expectLater(
        client.getInfo().timeout(const Duration(seconds: 5)),
        throwsA(
          isA<DaemonException>()
              .having((e) => e.code, 'code', DiagnosticCodes.daemonUnreachable)
              .having(
                (e) => e.diagnostic.why,
                'why',
                allOf(
                  contains('accepted a connection'),
                  contains('within 300 ms'),
                ),
              ),
        ),
      );
      expect(held, hasLength(1), reason: 'exactly one wake-up connection');
    });

    test('the fake flag and --socket shape the proposed command', () {
      final Diagnostic diagnostic = diagnosticFromGrpcError(
        const GrpcError.unavailable(),
        socketPath: '/tmp/x/daemon.sock',
        tokenPath: '/tmp/x/token',
        fake: true,
        socketFlag: true,
      );
      expect(
        diagnostic.fix,
        const FixAction.copyCommand(
          command:
              'humanitld --fake fixtures/sessions/mixed.jsonl '
              '--socket /tmp/x/daemon.sock',
        ),
      );
    });
  });
}
