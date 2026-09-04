/// The keyboard vocabulary of the Intercept section (CONVENTIONS 3.9).
///
/// The intents that other sections also know -- allow, block, move -- live in
/// `core/shortcuts` and are re-exported here, so a widget of this feature
/// imports one file. The intents that only this screen has (the remember grid)
/// are declared here.
///
/// [interceptShortcuts] is the whole map, and every activator in it has an
/// action in `InterceptScreen`. A binding without an action is deleted, not
/// silenced (`docs/UX.md` 5.3): `/` and `Ctrl+D` were bound and mute and are
/// therefore gone until the screen that owns them arrives -- the filter and
/// the domain pane, each with its own issue. `N` came back with HUM-072, and
/// the group keys came with HUM-029.
library;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

import '../../core/shortcuts/intents.dart'
    show AllowIntent, BlockIntent, NextFlowIntent, NoteIntent, PrevFlowIntent;

export '../../core/shortcuts/intents.dart'
    show
        AllowIntent,
        BlockIntent,
        NextFlowIntent,
        NoteIntent,
        PrevFlowIntent,
        isTextInputFocused;

/// Open the remember grid without deciding anything. `Shift+Enter`.
class OpenRememberIntent extends Intent {
  /// Creates the intent.
  const OpenRememberIntent();
}

/// Choose how long a remembered decision holds. `1` to `4`.
class RememberDurationIntent extends Intent {
  /// Creates the intent for the segment at [index], zero based.
  const RememberDurationIntent(this.index);

  /// Index into `RememberDuration.values`.
  final int index;
}

/// Choose what a remembered decision covers. `Shift+1` to `Shift+4`.
class RememberTargetIntent extends Intent {
  /// Creates the intent for the segment at [index], zero based.
  const RememberTargetIntent(this.index);

  /// Index into `RememberTarget.values`.
  final int index;
}

/// Send every selected request, or the whole group under the cursor.
/// `Ctrl+Shift+F` (CONVENTIONS 3.9).
class AllowGroupIntent extends Intent {
  /// Creates the intent.
  const AllowGroupIntent();
}

/// Refuse every selected request, or the whole group under the cursor.
/// `Ctrl+Shift+L` (CONVENTIONS 3.9).
class BlockGroupIntent extends Intent {
  /// Creates the intent.
  const BlockGroupIntent();
}

/// Select every request of the group the cursor stands in. `Ctrl+A`.
class SelectGroupIntent extends Intent {
  /// Creates the intent.
  const SelectGroupIntent();
}

/// Fold the group the cursor stands in, or open it. `ArrowLeft`, `ArrowRight`.
///
/// The chevron of a group header is a pointer target; these two keys are its
/// keyboard half, so that folding never needs the mouse (`docs/UX.md` 5.1).
class ToggleGroupIntent extends Intent {
  /// Creates the intent; [open] says which of the two directions is meant.
  const ToggleGroupIntent({required this.open});

  /// True opens the group, false folds it.
  final bool open;
}

/// Take the arrivals that waited into the list. `Shift+J`.
///
/// Without a key the pill of `docs/UX.md` 2.8 would be reachable by pointer
/// only, and a keyboard user could never merge what arrived while they read.
class MergeArrivalsIntent extends Intent {
  /// Creates the intent.
  const MergeArrivalsIntent();
}

/// The digits of the two segmented controls, in segment order.
const List<LogicalKeyboardKey> rememberKeys = <LogicalKeyboardKey>[
  LogicalKeyboardKey.digit1,
  LogicalKeyboardKey.digit2,
  LogicalKeyboardKey.digit3,
  LogicalKeyboardKey.digit4,
];

/// The bindings of the Intercept section.
///
/// The two decision keys carry `includeRepeats: false`: a key repeat is the
/// keyboard telling us that a finger has not moved, and a finger that has not
/// moved has not read the next URL (`docs/UX.md` 5.4). The screen refuses a
/// repeat a second time, because a test harness -- and some platforms -- can
/// deliver a plain down event for what a person meant as one press.
Map<ShortcutActivator, Intent>
interceptShortcuts() => <ShortcutActivator, Intent>{
  const SingleActivator(LogicalKeyboardKey.enter, includeRepeats: false):
      const AllowIntent(),
  const SingleActivator(LogicalKeyboardKey.numpadEnter, includeRepeats: false):
      const AllowIntent(),
  const SingleActivator(LogicalKeyboardKey.keyA, includeRepeats: false):
      const AllowIntent(),
  const SingleActivator(
    LogicalKeyboardKey.keyF,
    control: true,
    includeRepeats: false,
  ): const AllowIntent(
    chord: true,
  ),
  const SingleActivator(LogicalKeyboardKey.keyB, includeRepeats: false):
      const BlockIntent(),
  // `Ctrl+Enter` blocks with the note, from inside the field: a chord works
  // where a single letter must not (HUM-072).
  const SingleActivator(
    LogicalKeyboardKey.enter,
    control: true,
    includeRepeats: false,
  ): const BlockIntent(
    chord: true,
  ),
  const SingleActivator(
    LogicalKeyboardKey.keyL,
    control: true,
    includeRepeats: false,
  ): const BlockIntent(
    chord: true,
  ),
  const SingleActivator(LogicalKeyboardKey.keyJ): const NextFlowIntent(),
  const SingleActivator(LogicalKeyboardKey.arrowDown): const NextFlowIntent(),
  const SingleActivator(LogicalKeyboardKey.keyK): const PrevFlowIntent(),
  const SingleActivator(LogicalKeyboardKey.arrowUp): const PrevFlowIntent(),
  const SingleActivator(LogicalKeyboardKey.enter, shift: true):
      const OpenRememberIntent(),
  const SingleActivator(LogicalKeyboardKey.keyN): const NoteIntent(),
  const SingleActivator(
    LogicalKeyboardKey.keyF,
    control: true,
    shift: true,
    includeRepeats: false,
  ): const AllowGroupIntent(),
  const SingleActivator(
    LogicalKeyboardKey.keyL,
    control: true,
    shift: true,
    includeRepeats: false,
  ): const BlockGroupIntent(),
  const SingleActivator(LogicalKeyboardKey.keyA, control: true):
      const SelectGroupIntent(),
  const SingleActivator(LogicalKeyboardKey.keyJ, shift: true):
      const MergeArrivalsIntent(),
  const SingleActivator(LogicalKeyboardKey.arrowRight): const ToggleGroupIntent(
    open: true,
  ),
  const SingleActivator(LogicalKeyboardKey.arrowLeft): const ToggleGroupIntent(
    open: false,
  ),
  for (int i = 0; i < rememberKeys.length; i++)
    SingleActivator(rememberKeys[i]): RememberDurationIntent(i),
  for (int i = 0; i < rememberKeys.length; i++)
    SingleActivator(rememberKeys[i], shift: true): RememberTargetIntent(i),
};
