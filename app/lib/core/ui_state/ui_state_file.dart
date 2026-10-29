/// The small file in which the interface keeps its state across restarts
/// (HUM-044, HUM-254, HUM-305).
///
/// Where the file lies and why it is read synchronously is explained where it
/// began, in `features/intercept/providers/coach_mark.dart`. It moved here
/// when the Sandbox section and the editor began to keep their pane split in
/// it: a feature may not import another feature (ARCHITECTURE 5,
/// `tools/check-deps.sh`), so the store became a service of the core.
library;

import 'dart:convert';
import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';

/// Where small pieces of interface state survive a restart.
///
/// Reads and writes one small JSON object. Every failure is an answer and
/// never an exception: a missing file means "not seen", an unreadable one
/// means the same, and a write that does not go through leaves the flag in
/// memory.
class UiStateFile {
  /// Creates a store over [path].
  const UiStateFile(this.path);

  /// The store at the place XDG names for it.
  ///
  /// `$XDG_STATE_HOME/humanitl/ui-state.json`, and `~/.local/state/...` when
  /// the variable is unset -- the same order the daemon uses for its own
  /// directories (`humanitl_config::Paths`).
  factory UiStateFile.resolve({Map<String, String>? environment}) {
    final Map<String, String> env = environment ?? Platform.environment;
    final String? state = env['XDG_STATE_HOME'];
    final String base = state == null || state.isEmpty
        ? '${env['HOME'] ?? '.'}/.local/state'
        : state;
    return UiStateFile('$base/humanitl/$fileName');
  }

  /// The name of the file.
  static const String fileName = 'ui-state.json';

  /// Where the file lies.
  final String path;

  /// The whole object, or an empty one when there is nothing readable.
  Map<String, Object?> read() {
    try {
      final File file = File(path);
      if (!file.existsSync()) {
        return const <String, Object?>{};
      }
      final Object? decoded = jsonDecode(file.readAsStringSync());
      return decoded is Map<String, Object?>
          ? decoded
          : const <String, Object?>{};
    } on IOException {
      return const <String, Object?>{};
    } on FormatException {
      return const <String, Object?>{};
    }
  }

  /// Writes [key] as true. Answers whether it went through.
  bool setFlag(String key) => setBool(key, true);

  /// Writes [key] as [value], leaving every other key as it was. Answers
  /// whether it went through.
  ///
  /// The shell keeps the width of its rail here as well (HUM-254); a flag
  /// that can be switched back needs `false` as much as `true`.
  bool setBool(String key, bool value) => setValue(key, value);

  /// Writes [key] as [value], any value `jsonEncode` accepts, leaving every
  /// other key as it was. Answers whether it went through.
  ///
  /// The panes of a screen keep their split here as a list of numbers
  /// (HUM-305).
  bool setValue(String key, Object? value) {
    try {
      final Map<String, Object?> object = <String, Object?>{
        ...read(),
        key: value,
      };
      final File file = File(path);
      file.parent.createSync(recursive: true);
      file.writeAsStringSync(jsonEncode(object), flush: true);
      return true;
    } on IOException {
      return false;
    }
  }
}

/// The store the application uses. Tests override it with
/// `uiStateOverride()` from `test/harness/ui_state.dart`.
final Provider<UiStateFile> uiStateFileProvider = Provider<UiStateFile>(
  (Ref ref) => UiStateFile.resolve(),
);
