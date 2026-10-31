// The restart of the daemon on macOS (HUM-325): no systemd, no
// `humanitl daemon restart` restarts the launchd agent, and only when no agent
// session holds the daemon: retire-if-idle first, as on Linux.

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/features/shell/daemon_restart.dart';

void main() {
  final List<List<String>> ran = <List<String>>[];
  Future<ProcessResult> run(String executable, List<String> arguments) async {
    ran.add(<String>[executable, ...arguments]);
    return ProcessResult(1, 0, '', '');
  }

  setUp(ran.clear);

  test('a retired daemon is restarted through daemon restart', () async {
    const String cli = '/Applications/Humanitl.app/Contents/MacOS/bin/humanitl';
    Future<ProcessResult> retiring(String executable, List<String> args) async {
      ran.add(<String>[executable, ...args]);
      final bool retire = args.contains('retire-if-idle');
      return ProcessResult(1, 0, retire ? '{"outcome":"retired"}' : '', '');
    }

    final DaemonRestart outcome = await restartUserDaemon(
      retiring,
      cli: cli,
      daemonMinor: 23,
      askSession: () async => true,
      macOS: true,
    );
    expect(outcome, isA<DaemonRestarted>());
    expect(ran, <List<String>>[
      <String>[cli, '--json', 'daemon', 'retire-if-idle'],
      <String>[cli, 'daemon', 'restart'],
    ]);
  });

  group('restartAfterStop (HUM-367)', () {
    const String cli = '/x/humanitl';

    /// Answers `retire-if-idle` with [outcomes] in turn; the last one stays.
    CommandRunner answering(List<String> outcomes) {
      int asked = 0;
      return (String executable, List<String> args) async {
        ran.add(<String>[executable, ...args]);
        if (!args.contains('retire-if-idle')) {
          return ProcessResult(1, 0, '', '');
        }
        final String outcome = outcomes[asked.clamp(0, outcomes.length - 1)];
        asked++;
        return ProcessResult(1, 0, '{"outcome":"$outcome"}', '');
      };
    }

    Future<DaemonRestart> restart(CommandRunner runner, void Function() stop) =>
        restartAfterStop(
          runner,
          stop: () async => stop(),
          cli: cli,
          daemonMinor: 23,
          askSession: () async => true,
          macOS: true,
          pause: Duration.zero,
        );

    test('it stops first and waits out the moment the session ends', () async {
      final List<String> order = <String>[];
      final DaemonRestart outcome = await restart(
        answering(<String>['busy', 'busy', 'retired']),
        () => order.add('stop:${ran.length}'),
      );
      expect(order, <String>['stop:0']);
      expect(outcome, isA<DaemonRestarted>());
      expect(
        ran.where((List<String> c) => c.last == 'retire-if-idle').length,
        3,
      );
      expect(ran.last, <String>[cli, 'daemon', 'restart']);
    });

    test('a daemon that stays busy is never restarted', () async {
      final DaemonRestart outcome = await restart(
        answering(<String>['busy']),
        () {},
      );
      expect(outcome, isA<DaemonSessionHolds>());
      expect(ran.where((List<String> c) => c.last == 'restart'), isEmpty);
      expect(ran.length, restartAfterStopAttempts);
    });
  });

  test('an old daemon without retire is asked, then restarted', () async {
    final DaemonRestart outcome = await restartUserDaemon(
      run,
      cli: '/x/humanitl',
      daemonMinor: 18,
      askSession: () async => false,
      macOS: true,
    );
    expect(outcome, isA<DaemonRestarted>());
    expect(ran, <List<String>>[
      <String>['/x/humanitl', 'daemon', 'restart'],
    ]);
  });

  test('a session holds the daemon, and nothing runs', () async {
    final DaemonRestart outcome = await restartUserDaemon(
      run,
      cli: '/x/humanitl',
      daemonMinor: 18,
      askSession: () async => null,
      macOS: true,
    );
    expect(outcome, isA<DaemonSessionHolds>());
    expect(ran, isEmpty);
  });

  test('without a command line next to the app it says what to run', () async {
    final DaemonRestart outcome = await restartUserDaemon(
      run,
      cli: null,
      daemonMinor: 23,
      askSession: () async => false,
      macOS: true,
    );
    expect(outcome, isA<DaemonRestartFailed>());
    expect(
      (outcome as DaemonRestartFailed).detail,
      contains('humanitl daemon restart'),
    );
  });
}
