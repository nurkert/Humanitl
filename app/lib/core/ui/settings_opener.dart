/// The way from a diagnostic to the field of the settings sheet that resolves
/// it (HUM-275).
///
/// A `ChangeSetting` fix names a configuration key. Where the settings sheet
/// shows a field for that key, the fix becomes a button that opens the sheet,
/// scrolls to the field and focuses it. The sheet belongs to the shell, the
/// button to whichever screen draws the diagnostic, and no feature imports
/// another one (ARCHITECTURE 5). The shell therefore puts this widget above
/// its sections, and `FixControl` asks for it.
///
/// Where no shell is above, there is no opener, and the fix keeps the badge
/// that names the setting: a button that opens nothing would promise what does
/// not happen (`docs/UX.md` 4.4).
library;

import 'package:flutter/widgets.dart';

/// Opens the settings sheet at the field of a configuration key.
class SettingsOpener extends InheritedWidget {
  /// Puts [open] above [child].
  const SettingsOpener({required this.open, required super.child, super.key});

  /// Opens the sheet; with a key, at that key's field.
  final void Function(String? key) open;

  /// The nearest opener, or null when no shell is above [context].
  static SettingsOpener? maybeOf(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<SettingsOpener>();

  @override
  bool updateShouldNotify(SettingsOpener oldWidget) => open != oldWidget.open;
}
