/// The keyboard vocabulary of the shell (CONVENTIONS 3.9).
///
/// Intents live in `core` so that every feature can bind actions to them
/// without importing another feature. HUM-019 binds [NavIntent] and
/// [PaletteIntent]; the single-key intents of the queue (`AllowIntent`,
/// `BlockIntent`, ...) arrive with HUM-020 and must consult
/// [isTextInputFocused] first, because `Shortcuts` fires inside text fields.
library;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

/// Show section [index] (0 = Intercept ... 4 = Audit). `Ctrl+1..5`.
class NavIntent extends Intent {
  /// Creates the intent for section [index].
  const NavIntent(this.index);

  /// Zero-based section index.
  final int index;
}

/// Open the command palette. `Ctrl+K`.
class PaletteIntent extends Intent {
  /// Creates the intent.
  const PaletteIntent();
}

/// The bindings of HUM-019: `Ctrl+1..5` and `Ctrl+K`.
Map<ShortcutActivator, Intent> shellShortcuts() => <ShortcutActivator, Intent>{
  for (int i = 0; i < navigationKeys.length; i++)
    SingleActivator(navigationKeys[i], control: true): NavIntent(i),
  const SingleActivator(LogicalKeyboardKey.keyK, control: true):
      const PaletteIntent(),
};

/// The digit keys of `Ctrl+1..5`, in section order.
const List<LogicalKeyboardKey> navigationKeys = <LogicalKeyboardKey>[
  LogicalKeyboardKey.digit1,
  LogicalKeyboardKey.digit2,
  LogicalKeyboardKey.digit3,
  LogicalKeyboardKey.digit4,
  LogicalKeyboardKey.digit5,
];

/// True when the keyboard focus sits in an editable text.
///
/// Single-key shortcuts must not fire while the person types; this is the
/// check they run first.
bool isTextInputFocused() {
  final BuildContext? context = FocusManager.instance.primaryFocus?.context;
  if (context == null) {
    return false;
  }
  return context.widget is EditableText ||
      context.findAncestorWidgetOfExactType<EditableText>() != null;
}

// --- Intercept (HUM-020) -----------------------------------------------------
//
// The single-key intents fire only while no text field has the focus; the
// `chord` variants (`Ctrl+F`, `Ctrl+L`) fire anywhere, so a note can be typed
// and the request blocked without leaving the field.

/// Allow the selected held flow once, unchanged. Enter, `A`, `Ctrl+F`.
class AllowIntent extends Intent {
  /// Creates the intent. [chord] is true for `Ctrl+F`, which also works
  /// inside a text field.
  const AllowIntent({this.chord = false});

  /// True when a modifier chord fired the intent.
  final bool chord;
}

/// Block the selected held flow. `B`, `Ctrl+L`.
class BlockIntent extends Intent {
  /// Creates the intent. [chord] is true for `Ctrl+L`, which also works
  /// inside a text field.
  const BlockIntent({this.chord = false});

  /// True when a modifier chord fired the intent.
  final bool chord;
}

/// Select the next flow of the queue. `J`, ↓.
class NextFlowIntent extends Intent {
  /// Creates the intent.
  const NextFlowIntent();
}

/// Select the previous flow of the queue. `K`, ↑.
class PrevFlowIntent extends Intent {
  /// Creates the intent.
  const PrevFlowIntent();
}

/// Focus the queue filter. `/`.
class FilterIntent extends Intent {
  /// Creates the intent.
  const FilterIntent();
}

/// Show or hide the domain pane. `Ctrl+D`.
class ToggleDomainPanelIntent extends Intent {
  /// Creates the intent.
  const ToggleDomainPanelIntent();
}

/// Focus the block-note field. `N`.
class NoteIntent extends Intent {
  /// Creates the intent.
  const NoteIntent();
}

// The bindings of the Intercept section live with that section, in
// `features/intercept/intents.dart`: they carry `includeRepeats: false` on
// the two decisions, and a binding without an action in the screen is deleted
// rather than silenced (`docs/UX.md` 5.3). The intents themselves stay here,
// because `FilterIntent`, `ToggleDomainPanelIntent` and `NoteIntent` belong to
// the vocabulary of CONVENTIONS 3.9 and come back with the screens that
// answer them.
