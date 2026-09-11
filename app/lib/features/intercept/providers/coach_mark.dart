/// Whether the one-time explanation of the action bar has been shown before
/// (HUM-044).
///
/// # Where the flag lives, and why it is not in `config.toml`
///
/// There is no write path into the configuration for this flag: `SetConfig`
/// accepts only a CA variable under `sandbox.env` since HUM-151 and refuses
/// every other key with `CONFIG_014` until HUM-069, `humanitl config` has
/// only `get` and `schema`, and `UiConfig` carries `deny_unknown_fields`, so a
/// key `ui.coach_marks_seen` would be `CONFIG_001` on the next start. The flag
/// therefore lives beside the application, in a file of its own under
/// `$XDG_STATE_HOME` -- the directory the specification names for exactly this
/// kind of value: state that should survive a restart and that nobody would
/// miss if it were lost.
///
/// It is deliberately **not** in the daemon's data directory. That directory
/// holds the recording, and a coach mark is not a recording.
///
/// # Why the file is read and written synchronously
///
/// It is one object of a few dozen bytes, read once per run and written once
/// per installation. Asynchronously it would be worse in both directions: the
/// hint would flash for a frame before the answer arrived, and the flag would
/// be written after the frame in which it became true -- so a person who
/// closed the window in that moment would meet the hint again. One small
/// synchronous read is the cheaper promise.
///
/// # There is no diagnostic when the file cannot be written
///
/// A diagnostic is anchored where the failure happened (`docs/UX.md` 4.4), and
/// there is no such place here: nobody asked for anything, nothing they were
/// doing failed, and a card explaining that a hint could not be remembered
/// would be louder than the hint. What happens instead is honest and visible:
/// the flag stays in memory for this run, and the hint may come back on the
/// next start. It is a convenience, and it fails like one.
library;

import 'dart:convert';
import 'dart:io';

import 'package:riverpod_annotation/riverpod_annotation.dart';

part 'coach_mark.g.dart';

/// The key under which the flag stands in the state file.
const String coachMarkSeenKey = 'interceptCoachMarkSeen';

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
  bool setFlag(String key) {
    try {
      final Map<String, Object?> object = <String, Object?>{
        ...read(),
        key: true,
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

/// The store the application uses. Tests override it.
@Riverpod(keepAlive: true)
UiStateFile uiStateFile(Ref ref) => UiStateFile.resolve();

/// Whether the explanation of the action bar has already been shown.
@Riverpod(keepAlive: true)
class CoachMarkSeen extends _$CoachMarkSeen {
  @override
  bool build() =>
      ref.read(uiStateFileProvider).read()[coachMarkSeenKey] == true;

  /// Writes the flag down.
  ///
  /// Called the first time the hint is on screen, not when it is dismissed: it
  /// appears at the first held request and never again, whether somebody
  /// clicked it away or simply decided.
  void markShown() {
    if (state) {
      return;
    }
    state = true;
    ref.read(uiStateFileProvider).setFlag(coachMarkSeenKey);
  }
}

/// Ob der Hinweis gerade auf dem Schirm steht.
///
/// Er steht hier und nicht im Widget, weil `Esc` ihn schliesst und die Taste
/// dort ankommt, wo der Fokus ist: beim Bildschirm. Ein Popover, das den Fokus
/// selbst naehme, um seine eigene Taste zu bekommen, naehme ihn der
/// Entscheidung weg, die es erklaert (`docs/UX.md` 5.2).
///
/// Nur fuer diesen Lauf: Ob er ueberhaupt noch erscheinen darf, steht in
/// [CoachMarkSeen] und damit in einer Datei.
@Riverpod(keepAlive: true)
class CoachMarkVisible extends _$CoachMarkVisible {
  @override
  bool build() => false;

  /// Zeigt ihn.
  void show() => state = true;

  /// Schliesst ihn fuer diesen Lauf.
  void close() => state = false;
}
