/// How the app was started: against the real daemon, against the Rust fake
/// daemon, or against an in-process [FakeDaemonClient] scenario
/// (CONVENTIONS 4.7).
///
/// `--dart-define=HUMANITL_FAKE=1` (or `default`, or the flag `--fake`) keeps
/// the gRPC client and expects `humanitld --fake <session.jsonl>` on the usual
/// socket. Any other value names a Dart scenario (HUM-058). `HUMANITL_SOCKET`
/// or `--socket PATH` points at a daemon started with `--socket`.
///
/// [FakeDaemonClient]: fake_daemon_client.dart
library;

import 'dart:io' show Platform;

/// Which client the app builds at start.
enum ClientMode {
  /// gRPC against the real daemon.
  daemon,

  /// gRPC against `humanitld --fake`; only the setup hints differ.
  fakeDaemon,

  /// An in-process scenario, no socket at all.
  fakeClient,
}

/// The resolved start options.
class LaunchOptions {
  /// Creates options.
  const LaunchOptions({
    this.mode = ClientMode.daemon,
    this.scenario,
    this.socketPath,
  });

  /// The value of `--dart-define=HUMANITL_FAKE=...`, empty when unset.
  static const String fakeDefine = String.fromEnvironment('HUMANITL_FAKE');

  /// The value of `--dart-define=HUMANITL_SOCKET=...`, empty when unset.
  static const String socketDefine = String.fromEnvironment('HUMANITL_SOCKET');

  /// Command line flag equivalent to `HUMANITL_FAKE=1`.
  static const String fakeFlag = '--fake';

  /// Command line flag that names the socket: `--socket PATH` or
  /// `--socket=PATH`.
  static const String socketFlag = '--socket';

  /// The values of `HUMANITL_FAKE` that mean "the Rust fake daemon".
  static const Set<String> fakeDaemonValues = <String>{'1', 'default', 'true'};

  /// The values that mean "not set".
  static const Set<String> offValues = <String>{'', '0', 'false'};

  /// The environment variable read when the define is empty.
  static const String fakeVariable = 'HUMANITL_FAKE';

  /// The environment variable read when the socket define is empty.
  static const String socketVariable = 'HUMANITL_SOCKET';

  /// Resolves the options from [args], the dart-defines and [environment].
  ///
  /// Precedence: command line, then dart-define, then environment. Tests pass
  /// [fakeDefine] and [socketDefine] to stand in for the compile-time values.
  factory LaunchOptions.resolve(
    List<String> args, {
    Map<String, String>? environment,
    String fakeDefine = LaunchOptions.fakeDefine,
    String socketDefine = LaunchOptions.socketDefine,
  }) {
    final Map<String, String> env = environment ?? Platform.environment;

    String fake = args.contains(fakeFlag) ? '1' : '';
    if (fake.isEmpty) {
      fake = fakeDefine;
    }
    if (fake.isEmpty) {
      fake = env[fakeVariable] ?? '';
    }
    fake = fake.trim().toLowerCase();

    String socket = _socketFromArgs(args) ?? '';
    if (socket.isEmpty) {
      socket = socketDefine;
    }
    if (socket.isEmpty) {
      socket = env[socketVariable] ?? '';
    }
    socket = socket.trim();

    final ClientMode mode;
    String? scenario;
    if (offValues.contains(fake)) {
      mode = ClientMode.daemon;
    } else if (fakeDaemonValues.contains(fake)) {
      mode = ClientMode.fakeDaemon;
    } else {
      mode = ClientMode.fakeClient;
      scenario = fake;
    }
    return LaunchOptions(
      mode: mode,
      scenario: scenario,
      socketPath: socket.isEmpty ? null : socket,
    );
  }

  /// Which client to build.
  final ClientMode mode;

  /// The scenario name for [ClientMode.fakeClient], otherwise null.
  final String? scenario;

  /// An explicit socket path, otherwise null for the XDG default.
  final String? socketPath;

  /// True when a fake of either kind is in use.
  bool get isFake => mode != ClientMode.daemon;

  static String? _socketFromArgs(List<String> args) {
    for (int i = 0; i < args.length; i++) {
      final String arg = args[i];
      if (arg == socketFlag && i + 1 < args.length) {
        return args[i + 1];
      }
      if (arg.startsWith('$socketFlag=')) {
        return arg.substring(socketFlag.length + 1);
      }
    }
    return null;
  }

  @override
  String toString() =>
      'LaunchOptions(mode: $mode, scenario: $scenario, socket: $socketPath)';
}
