// Tests der Laufzeitpfade (HUM-019): Spiegel von `Paths::runtime_dir`.

import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ipc/daemon_paths.dart';

void main() {
  test('XDG_RUNTIME_DIR wins', () {
    final DaemonPaths paths = DaemonPaths.resolve(
      environment: <String, String>{'XDG_RUNTIME_DIR': '/run/user/1000'},
      uid: 1000,
      directoryExists: (_) => false,
    );
    expect(paths.socket, '/run/user/1000/humanitl/daemon.sock');
    expect(paths.token, '/run/user/1000/humanitl/token');
    expect(paths.fallbackUsed, isFalse);
  });

  test('then /run/user/<uid> when it exists', () {
    final DaemonPaths paths = DaemonPaths.resolve(
      environment: const <String, String>{},
      uid: 1234,
      directoryExists: (String path) => path == '/run/user/1234',
    );
    expect(paths.socket, '/run/user/1234/humanitl/daemon.sock');
    expect(paths.fallbackUsed, isFalse);
  });

  test('then TMPDIR or /tmp, flagged as a fallback', () {
    final DaemonPaths tmp = DaemonPaths.resolve(
      environment: const <String, String>{},
      uid: 7,
      directoryExists: (_) => false,
    );
    expect(tmp.socket, '/tmp/humanitl-7/daemon.sock');
    expect(tmp.fallbackUsed, isTrue);

    final DaemonPaths custom = DaemonPaths.resolve(
      environment: <String, String>{'TMPDIR': '/var/tmp'},
      uid: 7,
      directoryExists: (_) => false,
    );
    expect(custom.token, '/var/tmp/humanitl-7/token');
  });

  test('an empty XDG_RUNTIME_DIR counts as unset', () {
    final DaemonPaths paths = DaemonPaths.resolve(
      environment: <String, String>{'XDG_RUNTIME_DIR': ''},
      uid: 1000,
      directoryExists: (String path) => path == '/run/user/1000',
    );
    expect(paths.runtimeDir, '/run/user/1000/humanitl');
  });

  test('beside a --socket path the token sits next to it', () {
    final DaemonPaths paths = DaemonPaths.besideSocket('/tmp/h/my.sock');
    expect(paths.socket, '/tmp/h/my.sock');
    expect(paths.token, '/tmp/h/token');
  });

  test('the real uid is a non-negative number', () {
    expect(DaemonPaths.currentUid(), greaterThanOrEqualTo(0));
  });
}
