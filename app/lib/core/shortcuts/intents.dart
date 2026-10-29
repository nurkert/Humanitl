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

/// Show section [index] (0 = Intercept ... 5 = Setup). `Ctrl+1..6`.
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

/// Open the settings. `Ctrl+,` (HUM-069).
class SettingsIntent extends Intent {
  /// Creates the intent.
  const SettingsIntent();
}

/// Expand or collapse the icon rail. `Ctrl+0` (HUM-254).
class ToggleRailIntent extends Intent {
  /// Creates the intent.
  const ToggleRailIntent();
}

/// Show the terminal of the sandbox over the whole window, or put it back.
/// `Ctrl+Shift+Enter` (HUM-298).
class ToggleTerminalFullWindowIntent extends Intent {
  /// Creates the intent.
  const ToggleTerminalFullWindowIntent();
}

/// The chord that shows the terminal over the whole window and back
/// (HUM-298), decided by the owner on 2026-10-29.
///
/// Not `Ctrl+Shift+F`: that is `AllowGroupIntent` of the queue, whose binding
/// is nearer to the focus and would allow a group of one held request at the
/// first press. `Ctrl+Shift+Enter` is free in every section. `SingleActivator`
/// demands its modifiers exactly, so none of the queue's `Enter` bindings
/// matches it -- `Enter` allows, `Ctrl+Enter` blocks, `Shift+Enter` opens the
/// remember grid -- and none of them is asked before the shell's.
///
/// No repeats: a held chord would toggle the layout back and forth. The
/// terminal looks the chord up itself (it is one of the few keys it does not
/// hand to the agent, `docs/UX.md` 5.2), and the tooltip and the palette name
/// it.
const SingleActivator terminalFullWindowActivator = SingleActivator(
  LogicalKeyboardKey.enter,
  control: true,
  shift: true,
  includeRepeats: false,
);

/// The same chord with the `Enter` of the number block, like every `Enter`
/// binding of the queue and the history (HUM-298).
const SingleActivator terminalFullWindowNumpadActivator = SingleActivator(
  LogicalKeyboardKey.numpadEnter,
  control: true,
  shift: true,
  includeRepeats: false,
);

/// Give the keyboard to the terminal wherever it is shown, without changing
/// the layout. `Ctrl+Backquote` (HUM-303).
class FocusTerminalIntent extends Intent {
  /// Creates the intent.
  const FocusTerminalIntent();
}

/// `Ctrl` with the key left of the `1`, by its place on the keyboard
/// (HUM-303).
///
/// On a US layout that key is the backquote, and `Ctrl+Backquote` is the
/// chord the owner chose. On many other layouts the same key is a dead key
/// (`^` on a German keyboard) or another sign (`²` on a French one), and the
/// backquote itself lies elsewhere, often behind a dead key or `AltGr`. A
/// [SingleActivator] names a logical key and would then match nothing, or a
/// chord nobody can press with one hand; this activator therefore asks for
/// the physical key. It also takes a logical backquote with `Ctrl`, for a
/// layout that puts the backquote on its own key somewhere else.
///
/// Exactly `Ctrl`: with `Shift`, `Alt` or `Meta` it is another chord. No
/// repeats, so a held chord asks once. No other binding of the application
/// uses this key, with or without modifiers (checked against every
/// `SingleActivator` in `app/lib` and CONVENTIONS 3.9). The terminal looks
/// it up like the other keys of the shell, so it never reaches the agent.
class TerminalFocusActivator extends ShortcutActivator {
  /// Creates the activator.
  const TerminalFocusActivator();

  /// Null: the trigger is a physical key, and the logical key it produces
  /// differs from layout to layout. `Shortcuts` then asks [accepts] for every
  /// key event.
  @override
  Iterable<LogicalKeyboardKey>? get triggers => null;

  @override
  bool accepts(KeyEvent event, HardwareKeyboard state) =>
      event is KeyDownEvent &&
      (event.physicalKey == PhysicalKeyboardKey.backquote ||
          event.logicalKey == LogicalKeyboardKey.backquote) &&
      state.isControlPressed &&
      !state.isShiftPressed &&
      !state.isAltPressed &&
      !state.isMetaPressed;

  @override
  String debugDescribeKeys() => 'Ctrl + Backquote (physical)';
}

/// The key that expands and collapses the rail, with `Ctrl` (HUM-254).
///
/// Not `Ctrl+B`: CONVENTIONS 3.9 keeps that for `AddRuleIntent`, and a plain
/// `B` blocks a held request, so a chord whose `Ctrl` came a moment late
/// would decide instead of changing the layout. The zero belongs to the key
/// family of the rail and is unbound without `Ctrl` in every section.
const LogicalKeyboardKey railToggleKey = LogicalKeyboardKey.digit0;

/// The bindings of HUM-019: one `Ctrl+<digit>` per section, `Ctrl+K`,
/// `Ctrl+,` for the settings (HUM-069), `Ctrl+0` for the rail (HUM-254) and
/// `Ctrl+Shift+Enter` for the terminal over the whole window (HUM-298) and
/// `Ctrl+Backquote` for the keyboard to the terminal (HUM-303).
Map<ShortcutActivator, Intent> shellShortcuts() => <ShortcutActivator, Intent>{
  for (int i = 0; i < navigationKeys.length; i++)
    SingleActivator(navigationKeys[i], control: true): NavIntent(i),
  const SingleActivator(LogicalKeyboardKey.keyK, control: true):
      const PaletteIntent(),
  const SingleActivator(LogicalKeyboardKey.comma, control: true):
      const SettingsIntent(),
  const SingleActivator(railToggleKey, control: true): const ToggleRailIntent(),
  terminalFullWindowActivator: const ToggleTerminalFullWindowIntent(),
  terminalFullWindowNumpadActivator: const ToggleTerminalFullWindowIntent(),
  const TerminalFocusActivator(): const FocusTerminalIntent(),
};

/// The digit keys of the navigation shortcuts, in section order.
///
/// One key per entry of `Section`; the list grows at the end when a section
/// does, so every digit somebody has learned keeps its meaning (HUM-044). It
/// ends at nine at the latest: the zero is [railToggleKey].
const List<LogicalKeyboardKey> navigationKeys = <LogicalKeyboardKey>[
  LogicalKeyboardKey.digit1,
  LogicalKeyboardKey.digit2,
  LogicalKeyboardKey.digit3,
  LogicalKeyboardKey.digit4,
  LogicalKeyboardKey.digit5,
  LogicalKeyboardKey.digit6,
];

/// The index of the Sandbox section in [navigationKeys] (`Ctrl+4`).
///
/// A feature that leads to the terminal of the agent cannot name `Section`,
/// which lives in the shell (ARCHITECTURE 5); it names this index instead.
/// The digits keep their meaning (see [navigationKeys]), and a test of the
/// shell holds this constant to `Section.sandbox.index` (HUM-306).
const int sandboxSectionIndex = 3;

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

/// Open the request editor for the selected held flow. `E` (HUM-047).
///
/// It decides nothing. The editor replaces the card in the middle pane and
/// lets a person pseudonymise the request before it goes out; the decision is
/// still taken in the action bar, by the same notifier as every other one
/// (`backlog/CONVENTIONS.md` 3.9).
class EditIntent extends Intent {
  /// Creates the intent.
  const EditIntent();
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
