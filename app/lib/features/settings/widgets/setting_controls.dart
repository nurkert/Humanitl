/// The small controls the rows of the overlay share (HUM-404).
library;

import 'dart:async';

import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../settings_scope.dart';

/// Writes [value] to [key] through the overlay above [context].
///
/// Outside an overlay -- a golden of a single row -- nothing happens.
void writeSetting(BuildContext context, String key, String value) {
  final SettingsController? scope = SettingsScope.maybeOf(context);
  if (scope != null) {
    unawaited(scope.write(key, value));
  }
}

/// On and Off as two segments.
///
/// A switch of its own is not in the system, and a checkbox wants a label
/// beside it that would only repeat the name of the row. Two segments are as
/// high as every other control of the page and stand on the same right edge.
class OnOffControl extends StatelessWidget {
  /// Creates the control showing [value]; [onChanged] gets the new value.
  const OnOffControl({required this.value, required this.onChanged, super.key});

  /// Whether the setting is on.
  final bool value;

  /// Invoked with the chosen value.
  final ValueChanged<bool> onChanged;

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = context.l10n;
    return HSegmented<bool>(
      options: <HSegmentOption<bool>>[
        HSegmentOption<bool>(value: true, label: l10n.settingsOn),
        HSegmentOption<bool>(value: false, label: l10n.settingsOff),
      ],
      selected: value,
      onSelect: (bool on) {
        if (on != value) {
          onChanged(on);
        }
      },
    );
  }
}
