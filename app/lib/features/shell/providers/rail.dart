/// Whether the icon rail is expanded (HUM-254).
///
/// The choice survives a restart in the same small file as the hint of the
/// action bar (`UiStateFile`, HUM-044): state of the interface that nobody
/// would miss if it were lost, and that does not belong in `config.toml`.
/// Reading it is one synchronous read at the first build, for the same reason
/// as there: asynchronously the rail would start collapsed and jump open a
/// frame later.
///
/// A write that does not go through leaves the choice in memory for this run.
/// It is a convenience, and it fails like one.
library;

import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../intercept/providers/coach_mark.dart';

part 'rail.g.dart';

/// The key under which the choice stands in the state file.
const String railExpandedKey = 'shellRailExpanded';

/// True while the rail shows the name and the shortcut of every section.
///
/// Collapsed by default: the rail of HUM-019, glyphs only.
@Riverpod(keepAlive: true)
class RailExpanded extends _$RailExpanded {
  @override
  bool build() => ref.read(uiStateFileProvider).read()[railExpandedKey] == true;

  /// Expands a collapsed rail and collapses an expanded one, and writes the
  /// new state down.
  void toggle() {
    state = !state;
    ref.read(uiStateFileProvider).setBool(railExpandedKey, state);
  }
}
