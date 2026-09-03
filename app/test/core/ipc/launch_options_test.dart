// Tests der Startoptionen (HUM-019, CONVENTIONS 4.7).

import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ipc/launch_options.dart';

void main() {
  LaunchOptions resolve(
    List<String> args, {
    String define = '',
    String socketDefine = '',
    Map<String, String> env = const <String, String>{},
  }) => LaunchOptions.resolve(
    args,
    environment: env,
    fakeDefine: define,
    socketDefine: socketDefine,
  );

  test('nothing set means the real daemon', () {
    final LaunchOptions options = resolve(const <String>[]);
    expect(options.mode, ClientMode.daemon);
    expect(options.isFake, isFalse);
    expect(options.socketPath, isNull);
    expect(options.scenario, isNull);
  });

  test('1, default and true mean the Rust fake daemon', () {
    for (final String value in <String>['1', 'default', 'TRUE', ' 1 ']) {
      expect(
        resolve(const <String>[], define: value).mode,
        ClientMode.fakeDaemon,
      );
    }
    expect(resolve(const <String>['--fake']).mode, ClientMode.fakeDaemon);
    expect(
      resolve(
        const <String>[],
        env: <String, String>{'HUMANITL_FAKE': '1'},
      ).mode,
      ClientMode.fakeDaemon,
    );
  });

  test('0, false and empty mean off', () {
    for (final String value in <String>['0', 'false', '', 'False']) {
      expect(resolve(const <String>[], define: value).mode, ClientMode.daemon);
    }
  });

  test('any other value is a Dart scenario', () {
    final LaunchOptions options = resolve(const <String>[], define: 'Empty');
    expect(options.mode, ClientMode.fakeClient);
    expect(options.scenario, 'empty');
    expect(options.isFake, isTrue);
  });

  test('command line beats define beats environment', () {
    expect(
      resolve(
        const <String>['--fake'],
        define: 'empty',
        env: <String, String>{'HUMANITL_FAKE': 'unavailable'},
      ).mode,
      ClientMode.fakeDaemon,
    );
    expect(
      resolve(
        const <String>[],
        define: 'empty',
        env: <String, String>{'HUMANITL_FAKE': '1'},
      ).scenario,
      'empty',
    );
  });

  test('the socket comes from --socket, the define or the environment', () {
    expect(
      resolve(const <String>['--socket', '/tmp/a.sock']).socketPath,
      '/tmp/a.sock',
    );
    expect(
      resolve(const <String>['--socket=/tmp/b.sock']).socketPath,
      '/tmp/b.sock',
    );
    expect(
      resolve(const <String>[], socketDefine: '/tmp/c.sock').socketPath,
      '/tmp/c.sock',
    );
    expect(
      resolve(
        const <String>[],
        env: <String, String>{'HUMANITL_SOCKET': '/tmp/d.sock'},
      ).socketPath,
      '/tmp/d.sock',
    );
    // Ein `--socket` ohne Wert ist kein Socket.
    expect(resolve(const <String>['--socket']).socketPath, isNull);
  });
}
