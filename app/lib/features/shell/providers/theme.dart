/// Dark or light (`themeModeProvider`, CONVENTIONS 3.9).
///
/// The design is drawn dark-first; light is derived from it (HUM-008). The
/// choice is not persisted yet: `ui.theme` in `config.toml` arrives with the
/// settings screen.
library;

import 'package:flutter/widgets.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../core/ui/ui.dart';

part 'theme.g.dart';

/// The theme mode of the app.
@Riverpod(keepAlive: true, name: 'themeModeProvider')
class ThemeModeSetting extends _$ThemeModeSetting {
  @override
  HThemeMode build() => HThemeMode.dark;

  /// Uses [mode].
  void set(HThemeMode mode) => state = mode;

  /// Switches between dark and light. [platformBrightness] resolves
  /// [HThemeMode.system] so that the switch always visibly changes something.
  void toggle(Brightness platformBrightness) {
    final Brightness current = state.resolve(platformBrightness).brightness;
    state = current == Brightness.dark ? HThemeMode.light : HThemeMode.dark;
  }
}
