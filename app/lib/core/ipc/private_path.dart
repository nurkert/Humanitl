/// Checks that the daemon's runtime directory and token file belong to this
/// account before the app trusts them, mirror of
/// `humanitl_config::private_dir::check_private` (HUM-212).
///
/// Without `$XDG_RUNTIME_DIR` and `/run/user/<uid>` the runtime directory is
/// `$TMPDIR/humanitl-<uid>`: a predictable name in a directory every account
/// shares. Another user could create it first, with a socket and a readable
/// token of their own, and an app that trusted them would send that user its
/// keystrokes and settings. So both paths are checked with `lstat`: no
/// symlink, owned by this uid, no rights for group or others.
///
/// `FileStat` knows neither the owner nor whether it followed a link, so the
/// check calls `statx(2)` through FFI with `AT_SYMLINK_NOFOLLOW`. The layout of
/// `struct statx` is the same on every Linux architecture, unlike
/// `struct stat`.
library;

import 'dart:convert';
import 'dart:ffi' as ffi;

/// What a checked path is expected to be.
enum PrivateEntry {
  /// A directory, such as the runtime directory.
  directory,

  /// A regular file, such as the token.
  file,
}

/// Owner and mode of a path as `lstat` sees it, link not followed.
class PathStat {
  /// Creates a stat record; [mode] carries the file type bits (`S_IFMT`).
  const PathStat({required this.uid, required this.mode});

  /// The owning user id.
  final int uid;

  /// Type and permission bits, as in `st_mode`.
  final int mode;

  static const int _typeMask = 0xF000; // S_IFMT
  static const int _directory = 0x4000; // S_IFDIR
  static const int _regular = 0x8000; // S_IFREG
  static const int _symlink = 0xA000; // S_IFLNK

  /// True for a directory.
  bool get isDirectory => mode & _typeMask == _directory;

  /// True for a regular file.
  bool get isFile => mode & _typeMask == _regular;

  /// True for a symbolic link.
  bool get isLink => mode & _typeMask == _symlink;
}

/// Why a path is not trusted.
class PrivatePathProblem {
  /// Creates a problem about [path]; [missing] marks a path that does not
  /// exist at all, [open] one that only has too wide permissions.
  const PrivatePathProblem(
    this.path,
    this.why, {
    this.missing = false,
    this.open = false,
  });

  /// The checked path.
  final String path;

  /// Technical detail for `Diagnostic.why`.
  final String why;

  /// True when only group or others have rights: `chmod` fixes that, unlike a
  /// foreign owner or a symlink.
  final bool open;

  /// True when the path does not exist: then no daemon runs, which is not a
  /// finding about someone else's directory.
  final bool missing;

  @override
  String toString() => why;
}

/// Why [path] is not trusted as [entry] of the account [uid], or null when it
/// is.
///
/// Tests stand in for a foreign owner, which an unprivileged test cannot
/// create, by passing a [uid] other than their own.
PrivatePathProblem? privatePathProblem(
  String path,
  PrivateEntry entry, {
  required int uid,
}) {
  final PathStat? stat = lstatPath(path);
  if (stat == null) {
    return PrivatePathProblem(path, 'cannot stat $path', missing: true);
  }
  final bool kindOk = switch (entry) {
    PrivateEntry.directory => stat.isDirectory,
    PrivateEntry.file => stat.isFile,
  };
  if (!kindOk) {
    final String expected = switch (entry) {
      PrivateEntry.directory => 'a directory',
      PrivateEntry.file => 'a regular file',
    };
    final String link = stat.isLink
        ? ' (a symlink here is refused, not followed)'
        : '';
    return PrivatePathProblem(path, '$path is not $expected$link');
  }
  if (stat.uid != uid) {
    return PrivatePathProblem(
      path,
      '$path belongs to uid ${stat.uid}, not to you (uid $uid); '
      'the socket and token there are not trusted',
    );
  }
  final int permissions = stat.mode & 0x1FF;
  if (permissions & 0x3F != 0) {
    return PrivatePathProblem(
      path,
      '$path is mode ${permissions.toRadixString(8).padLeft(4, '0')}; '
      'the daemon keeps its runtime directory at 0700 and the token at '
      '0600, so an open one was not made by your daemon',
      open: true,
    );
  }
  return null;
}

/// `lstat` through `statx(2)`: owner and mode of [path], a final symlink not
/// followed. Null when the call fails.
PathStat? lstatPath(String path) {
  final List<int> bytes = utf8.encode(path);
  final ffi.Pointer<ffi.Uint8> name = _malloc(bytes.length + 1).cast();
  final ffi.Pointer<ffi.Uint8> buffer = _malloc(_statxSize).cast();
  try {
    name.asTypedList(bytes.length + 1)
      ..setAll(0, bytes)
      ..[bytes.length] = 0;
    final int result = _statx(
      _atFdCwd,
      name,
      _atSymlinkNoFollow,
      _statxType | _statxMode | _statxUid,
      buffer,
    );
    if (result != 0) {
      return null;
    }
    // struct statx: stx_uid at byte 20 (u32), stx_mode at byte 28 (u16).
    return PathStat(
      uid: buffer.cast<ffi.Uint32>()[5],
      mode: buffer.cast<ffi.Uint16>()[14],
    );
  } finally {
    _free(name.cast());
    _free(buffer.cast());
  }
}

const int _atFdCwd = -100;
const int _atSymlinkNoFollow = 0x100;
const int _statxType = 0x1;
const int _statxMode = 0x2;
const int _statxUid = 0x8;
const int _statxSize = 256;

final ffi.DynamicLibrary _libc = ffi.DynamicLibrary.process();

final int Function(
  int,
  ffi.Pointer<ffi.Uint8>,
  int,
  int,
  ffi.Pointer<ffi.Uint8>,
)
_statx = _libc
    .lookupFunction<
      ffi.Int32 Function(
        ffi.Int32,
        ffi.Pointer<ffi.Uint8>,
        ffi.Int32,
        ffi.Uint32,
        ffi.Pointer<ffi.Uint8>,
      ),
      int Function(
        int,
        ffi.Pointer<ffi.Uint8>,
        int,
        int,
        ffi.Pointer<ffi.Uint8>,
      )
    >('statx');

final ffi.Pointer<ffi.Void> Function(int) _malloc = _libc
    .lookupFunction<
      ffi.Pointer<ffi.Void> Function(ffi.Size),
      ffi.Pointer<ffi.Void> Function(int)
    >('malloc');

final void Function(ffi.Pointer<ffi.Void>) _free = _libc
    .lookupFunction<
      ffi.Void Function(ffi.Pointer<ffi.Void>),
      void Function(ffi.Pointer<ffi.Void>)
    >('free');
