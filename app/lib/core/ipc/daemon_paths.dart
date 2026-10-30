/// Where the daemon's socket and token live on this machine, mirror of
/// `humanitl_config::Paths::runtime_dir` (CONVENTIONS 3.4, 4.11).
///
/// Order: `$XDG_RUNTIME_DIR/humanitl`, then on macOS
/// `~/Library/Application Support/Humanitl/run` (HUM-314), then
/// `/run/user/<uid>/humanitl` when that directory exists, otherwise
/// `$TMPDIR/humanitl-<uid>` (or `/tmp`).
/// The app never creates any of these; the daemon does, with mode `0700`.
library;

import 'dart:convert';
import 'dart:ffi' as ffi;
import 'dart:io';

/// The two files the app needs from the daemon's runtime directory.
class DaemonPaths {
  /// Creates paths inside [runtimeDir]. [socketOverride] replaces the
  /// socket path while the token stays in [runtimeDir].
  const DaemonPaths({
    required this.runtimeDir,
    this.fallbackUsed = false,
    this.socketOverride,
  });

  /// Paths beside an explicitly chosen socket (`humanitld --socket PATH`):
  /// the token file always sits next to the socket.
  factory DaemonPaths.besideSocket(String socketPath) {
    final int slash = socketPath.lastIndexOf('/');
    return DaemonPaths(
      runtimeDir: slash <= 0 ? '.' : socketPath.substring(0, slash),
      socketOverride: socketPath,
    );
  }

  /// Resolves the runtime directory the way the daemon does.
  ///
  /// [environment] defaults to the process environment, [uid] to the real
  /// user id, [directoryExists] to a filesystem check; tests pass their own.
  factory DaemonPaths.resolve({
    Map<String, String>? environment,
    int? uid,
    bool Function(String path)? directoryExists,
    bool? macOS,
  }) {
    final Map<String, String> env = environment ?? Platform.environment;
    final bool Function(String) exists =
        directoryExists ?? ((String path) => Directory(path).existsSync());

    final String? xdg = _nonEmpty(env['XDG_RUNTIME_DIR']);
    if (xdg != null) {
      return DaemonPaths(runtimeDir: '$xdg/$appDir');
    }
    // macOS clears its temporary directory of files nobody touched for three
    // days, so the daemon keeps socket and token under Application Support.
    // Unless the longest socket below it would not fit into `sun_path` (104
    // bytes with the NUL); then the short temporary directory, like the
    // daemon (`Paths::runtime_dir_with`). Without HOME the daemon takes
    // `$TMPDIR/humanitl-<uid>-home` (`Paths::home`), and so does this mirror.
    if (macOS ?? Platform.isMacOS) {
      // Without the trailing slash of the macOS TMPDIR, as `PathBuf::join`.
      final String tmp = (_nonEmpty(env['TMPDIR']) ?? '/tmp').replaceFirst(
        RegExp(r'/+$'),
        '',
      );
      final int id = uid ?? currentUid();
      final String home = _nonEmpty(env['HOME']) ?? '$tmp/$appDir-$id-home';
      final String support = '$home/$macRuntimeDir';
      if (utf8.encode('$support/proxy/proxy.sock').length < 104) {
        return DaemonPaths(runtimeDir: support);
      }
      return DaemonPaths(runtimeDir: '$tmp/$appDir-$id', fallbackUsed: true);
    }
    final int id = uid ?? currentUid();
    final String runUser = '/run/user/$id';
    if (exists(runUser)) {
      return DaemonPaths(runtimeDir: '$runUser/$appDir');
    }
    final String tmp = _nonEmpty(env['TMPDIR']) ?? '/tmp';
    return DaemonPaths(runtimeDir: '$tmp/$appDir-$id', fallbackUsed: true);
  }

  /// Name of the daemon's directory below the runtime directory.
  static const String appDir = 'humanitl';

  /// The runtime directory below the home on macOS, mirror of
  /// `Paths::mac_support_dir().join("run")`.
  static const String macRuntimeDir =
      'Library/Application Support/Humanitl/run';

  /// Name of the socket file.
  static const String socketName = 'daemon.sock';

  /// Name of the token file.
  static const String tokenName = 'token';

  /// The directory that holds socket and token.
  final String runtimeDir;

  /// True when neither `$XDG_RUNTIME_DIR` nor `/run/user/<uid>` was usable
  /// and the temporary directory stands in (`CONFIG_004` on the daemon side).
  final bool fallbackUsed;

  /// A socket path chosen with `--socket`, or null for the default.
  final String? socketOverride;

  /// The gRPC socket.
  String get socket => socketOverride ?? '$runtimeDir/$socketName';

  /// The session token for the `x-humanitl-token` metadata header.
  String get token => '$runtimeDir/$tokenName';

  /// The real user id of this process, from libc.
  static int currentUid() => _getuid();

  static String? _nonEmpty(String? value) =>
      value == null || value.isEmpty ? null : value;

  @override
  String toString() => 'DaemonPaths(socket: $socket, token: $token)';
}

final int Function() _getuid = ffi.DynamicLibrary.process()
    .lookupFunction<ffi.Uint32 Function(), int Function()>('getuid');
