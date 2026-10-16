/// Dark or light (`themeModeProvider`, CONVENTIONS 3.9).
///
/// The design is drawn dark-first; light is derived from it (HUM-008). The
/// mode starts from `ui.theme` as the daemon reports it (HUM-069) and follows
/// it whenever that value changes; the palette toggle changes this window
/// until then and writes nothing.
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/config_providers.dart';
import '../../../core/ui/ui.dart';

part 'theme.g.dart';

/// The theme mode of the app.
@Riverpod(keepAlive: true, name: 'themeModeProvider')
class ThemeModeSetting extends _$ThemeModeSetting {
  @override
  HThemeMode build() {
    final String? theme = ref.watch(
      configProvider.select(
        (AsyncValue<ConfigValues> config) => config.value?.uiTheme,
      ),
    );
    return themeModeOf(theme);
  }

  /// Uses [mode].
  void set(HThemeMode mode) => state = mode;

  /// Switches between dark and light. [platformBrightness] resolves
  /// [HThemeMode.system] so that the switch always visibly changes something.
  void toggle(Brightness platformBrightness) {
    final Brightness current = state.resolve(platformBrightness).brightness;
    state = current == Brightness.dark ? HThemeMode.light : HThemeMode.dark;
  }
}

/// The mode a value of `ui.theme` names; dark for anything else, the mode the
/// design is drawn for.
HThemeMode themeModeOf(String? theme) => switch (theme) {
  'light' => HThemeMode.light,
  'system' => HThemeMode.system,
  _ => HThemeMode.dark,
};
