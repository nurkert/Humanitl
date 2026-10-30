/// How the form opens `config.toml` in the desktop's editor (HUM-069).
///
/// Everything the form does not show stays in `config.toml`; "Open
/// config.toml" hands the file to the desktop with `xdg-open`, without a
/// shell, and says so when that did not work. Tests replace the opener: a
/// widget test must never start a program on the person's desktop.
library;

import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';

/// Opens [path] with the desktop; true when the desktop took it.
typedef FileOpener = Future<bool> Function(String path);

/// The opener the form uses. Overridden in tests.
final Provider<FileOpener> fileOpenerProvider = Provider<FileOpener>(
  (Ref ref) => openWithDesktop,
);

/// `xdg-open PATH` (on macOS `open PATH`, HUM-325), without a shell; false
/// when it could not start or did not end with 0.
Future<bool> openWithDesktop(String path) async {
  try {
    final ProcessResult result = await Process.run(
      Platform.isMacOS ? 'open' : 'xdg-open',
      <String>[path],
    );
    return result.exitCode == 0;
  } on ProcessException {
    return false;
  }
}
