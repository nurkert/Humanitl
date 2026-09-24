// HUM-212: Der Client traut Laufzeitverzeichnis und Token nur, wenn beide ihm
// gehören, kein Symlink sind und für Gruppe und Andere zu sind.
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_diagnostics.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ipc/daemon_paths.dart';
import 'package:humanitl/core/ipc/grpc_daemon_client.dart';
import 'package:humanitl/core/ipc/launch_options.dart';
import 'package:humanitl/core/ipc/private_path.dart';

/// Legt `<base>/humanitl/token` an wie der Daemon: Verzeichnis 0700, Token
/// 0600. Gibt das Verzeichnis zurück.
String runtimeWithToken(String base) {
  final String dir = '$base/humanitl';
  Directory(dir).createSync();
  Process.runSync('chmod', <String>['700', dir]);
  File('$dir/token').writeAsStringSync('secret\n');
  Process.runSync('chmod', <String>['600', '$dir/token']);
  return dir;
}

void main() {
  late Directory tmp;
  final int uid = DaemonPaths.currentUid();

  setUp(() => tmp = Directory.systemTemp.createTempSync('humanitl-hum212-'));
  tearDown(() => tmp.deleteSync(recursive: true));

  test('statx reads owner and type without following a link', () {
    final String dir = runtimeWithToken(tmp.path);
    final PathStat? stat = lstatPath(dir);
    expect(stat, isNotNull);
    expect(stat!.uid, uid);
    expect(stat.isDirectory, isTrue);
    expect(stat.mode & 0x1FF, 0x1C0); // 0700
    Link('${tmp.path}/link').createSync(dir);
    expect(lstatPath('${tmp.path}/link')!.isLink, isTrue);
    expect(lstatPath('${tmp.path}/missing'), isNull);
  });

  test('an own private directory and token pass', () {
    final String dir = runtimeWithToken(tmp.path);
    expect(
      privatePathProblem(dir, PrivateEntry.directory, uid: uid)?.why,
      isNull,
    );
    expect(
      privatePathProblem('$dir/token', PrivateEntry.file, uid: uid)?.why,
      isNull,
    );
  });

  test('a directory and token of another account are refused', () {
    final String dir = runtimeWithToken(tmp.path);
    expect(
      privatePathProblem(dir, PrivateEntry.directory, uid: uid + 1)?.why,
      contains('belongs to uid $uid'),
    );
    expect(
      privatePathProblem('$dir/token', PrivateEntry.file, uid: uid + 1)?.why,
      contains('belongs to uid $uid'),
    );
  });

  test('symlinks in place of directory or token are refused', () {
    final String dir = runtimeWithToken(tmp.path);
    Link('${tmp.path}/dir-link').createSync(dir);
    Link('$dir/token-link').createSync('$dir/token');
    expect(
      privatePathProblem(
        '${tmp.path}/dir-link',
        PrivateEntry.directory,
        uid: uid,
      )?.why,
      contains('symlink'),
    );
    expect(
      privatePathProblem('$dir/token-link', PrivateEntry.file, uid: uid)?.why,
      contains('symlink'),
    );
  });

  test('open modes are refused', () {
    final String dir = runtimeWithToken(tmp.path);
    Process.runSync('chmod', <String>['644', '$dir/token']);
    expect(
      privatePathProblem('$dir/token', PrivateEntry.file, uid: uid)?.why,
      contains('is mode 0644'),
    );
    Process.runSync('chmod', <String>['755', dir]);
    expect(
      privatePathProblem(dir, PrivateEntry.directory, uid: uid)?.why,
      contains('is mode 0755'),
    );
  });

  test('the fallback fix links the install section', () {
    // Dieselben Literale prüft `the_fallback_fix_links_the_install_section`
    // in `daemon/crates/config/src/private_dir.rs` gegen die Rust-Seite.
    expect(
      ClientDiagnostics.ownRuntimeDirDocUrl,
      'https://github.com/nurkert/Humanitl/blob/main/docs/INSTALL.md#xdg_runtime_dir-ohne-logind',
    );
    expect(
      ClientDiagnostics.ownRuntimeDirHint,
      'daemon, CLI and app must all see the same XDG_RUNTIME_DIR, set for the whole session (see the linked section for bash and zsh), and it takes effect after logging in again; HUM-222 will let the clients find the directory themselves',
    );
  });

  test('a line break in the path gets no chmod', () {
    for (final String path in <String>['/tmp/a\nb/token', '/tmp/a\rb/token']) {
      final Diagnostic diagnostic = ClientDiagnostics.runtimeUntrusted(
        PrivatePathProblem(path, 'open', open: true),
        fallback: false,
      );
      expect(diagnostic.fix, isNull, reason: path);
    }
  });

  test('the client learns whether the runtime directory is the fallback', () {
    // Rückfall: kein XDG_RUNTIME_DIR, kein /run/user/<uid>.
    final GrpcDaemonClient fallback = grpcClientFor(
      const LaunchOptions(),
      resolve: () => DaemonPaths.resolve(
        environment: <String, String>{'TMPDIR': tmp.path},
        uid: uid,
        directoryExists: (String _) => false,
      ),
    );
    addTearDown(fallback.close);
    expect(fallback.runtimeFallback, isTrue);
    final GrpcDaemonClient session = grpcClientFor(
      const LaunchOptions(),
      resolve: () => DaemonPaths.resolve(
        environment: <String, String>{'XDG_RUNTIME_DIR': tmp.path},
        uid: uid,
      ),
    );
    addTearDown(session.close);
    expect(session.runtimeFallback, isFalse);
    final GrpcDaemonClient beside = grpcClientFor(
      LaunchOptions(socketPath: '${tmp.path}/d.sock'),
      resolve: () => throw StateError('--socket does not resolve'),
    );
    addTearDown(beside.close);
    expect(beside.runtimeFallback, isFalse);
  });

  group('GrpcDaemonClient', () {
    Future<void> expectUntrusted(
      GrpcDaemonClient client,
      String fragment, {
      FixAction? fix,
    }) async {
      addTearDown(client.close);
      await expectLater(
        client.getInfo(),
        throwsA(
          isA<DaemonException>()
              .having((e) => e.code, 'code', DiagnosticCodes.daemonUnreachable)
              .having((e) => e.diagnostic.why, 'why', contains(fragment))
              .having((e) => e.diagnostic.fix, 'fix', fix),
        ),
      );
    }

    // Der Weg des Befunds: ein anderes Konto hat Verzeichnis und Token vorab
    // unter dem vorhersagbaren Namen angelegt. Simuliert über eine andere UID
    // des Clients.
    test('does not read a token of another account', () async {
      final String dir = runtimeWithToken(tmp.path);
      await expectUntrusted(
        GrpcDaemonClient(
          socketPath: '$dir/daemon.sock',
          tokenPath: '$dir/token',
          callTimeout: const Duration(seconds: 2),
          uid: uid + 1,
          runtimeFallback: true,
        ),
        'belongs to uid',
        fix: const FixAction.openUrl(
          url: ClientDiagnostics.ownRuntimeDirDocUrl,
        ),
      );
    });

    // Eine gesunde Sitzung (kein Rückfall) mit offenem Token bekommt `chmod`,
    // nicht den Befehl, der `XDG_RUNTIME_DIR` umstellt: der zerlegte die
    // grafische Sitzung.
    test('a healthy session with an open token gets chmod', () async {
      // Ein `'` im Pfad bleibt ein Wort der Shell (HUM-215).
      final Directory quoted = Directory("${tmp.path}/it's")..createSync();
      final String dir = runtimeWithToken(quoted.path);
      Process.runSync('chmod', <String>['644', '$dir/token']);
      await expectUntrusted(
        GrpcDaemonClient(
          socketPath: '$dir/daemon.sock',
          tokenPath: '$dir/token',
          callTimeout: const Duration(seconds: 2),
        ),
        'is mode 0644',
        fix: FixAction.copyCommand(
          command: "chmod go-rwx '${tmp.path}/it'\\''s/humanitl/token'",
        ),
      );
    });

    // Im Rückfall ohne Verzeichnis und ohne Token läuft bloß kein Daemon.
    test('a missing fallback directory proposes starting the daemon', () async {
      final String dir = '${tmp.path}/humanitl-gone';
      final GrpcDaemonClient client = GrpcDaemonClient(
        socketPath: '$dir/daemon.sock',
        tokenPath: '$dir/token',
        callTimeout: const Duration(seconds: 2),
        runtimeFallback: true,
      );
      addTearDown(client.close);
      await expectLater(
        client.getInfo(),
        throwsA(
          isA<DaemonException>()
              .having((e) => e.code, 'code', DiagnosticCodes.daemonUnreachable)
              .having(
                (e) => e.diagnostic.fix,
                'fix',
                const FixAction.installService(),
              ),
        ),
      );
    });

    test('a foreign owner outside the fallback gets no fix', () async {
      final String dir = runtimeWithToken(tmp.path);
      await expectUntrusted(
        GrpcDaemonClient(
          socketPath: '$dir/daemon.sock',
          tokenPath: '$dir/token',
          callTimeout: const Duration(seconds: 2),
          uid: uid + 1,
        ),
        'belongs to uid',
      );
    });

    test('does not follow a symlinked runtime directory', () async {
      final String dir = runtimeWithToken(tmp.path);
      final String link = '${tmp.path}/runtime';
      Link(link).createSync(dir);
      await expectUntrusted(
        GrpcDaemonClient(
          socketPath: '$link/daemon.sock',
          tokenPath: '$link/token',
          callTimeout: const Duration(seconds: 2),
        ),
        'symlink',
      );
    });
  });
}
