/// The keyboard vocabulary of the shell (CONVENTIONS 3.9).
///
/// Intents live in `core` so that every feature can bind actions to them
/// without importing another feature. HUM-019 binds [NavIntent] and
/// [PaletteIntent]; the single-key intents of the queue (`AllowIntent`,
/// `BlockIntent`, ...) arrive with HUM-020 and must consult
/// [isTextInputFocused] first, because `Shortcuts` fires inside text fields.
library;

import 'package:flutter/foundation.dart';
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

/// Let the terminal of the sandbox float over the sections, or put it back
/// into its pane. `Ctrl+Shift+P` (HUM-300).
class ToggleTerminalFloatIntent extends Intent {
  /// Creates the intent.
  const ToggleTerminalFloatIntent();
}

/// The chord that lets the terminal float and puts it back (HUM-300).
///
/// `P` for picture in picture, next to the `Enter` of the whole window in
/// the same family of `Ctrl+Shift` chords that move the terminal. Checked
/// against every binding of the application (`app/lib`, `app/packages`,
/// CONVENTIONS 3.9): `P` alone is "Pseudonymise" in the findings pause of the
/// queue, and [SingleActivator] demands its modifiers exactly, so that
/// binding never matches `Ctrl+Shift+P`; no other binding uses `P`. The queue
/// decides with `Enter`, `A`, `B`, `Ctrl+F`, `Ctrl+L`, `Ctrl+Shift+F` and
/// `Ctrl+Shift+L`, none of which this is. `Ctrl+Shift+C` and `Ctrl+Shift+V`
/// are copy and paste in the terminal (HUM-313), `Ctrl+Shift+0` the zoom
/// reset (HUM-301), `Ctrl+Shift+Enter` the whole window (HUM-298).
///
/// **What the agent loses.** The terminal looks the chord up itself, like
/// every chord of the shell, so the agent never receives it. A terminal on
/// Linux sends `Ctrl+Shift+P` as `0x10`, the same byte as `Ctrl+P`, and
/// `Ctrl+P` itself stays with the agent (the previous line in readline);
/// only an agent that asked for the kitty keyboard protocol could have told
/// the two apart.
///
/// No repeats: a held chord would open and close the window in turn.
const SingleActivator terminalFloatActivator = SingleActivator(
  LogicalKeyboardKey.keyP,
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

/// What a zoom chord does to the text of the terminal (HUM-301).
enum TerminalZoom {
  /// One step larger. `Ctrl` + `+`, `Ctrl+=`, `Ctrl` + numpad `+`.
  zoomIn,

  /// One step smaller. `Ctrl` + `-`, `Ctrl` + numpad `-`.
  zoomOut,

  /// Back to the default size. `Ctrl+Shift+0`, `Ctrl` + numpad `0`.
  reset,
}

/// Make the text of the terminal that is shown larger, smaller or the
/// default size again (HUM-301).
class TerminalZoomIntent extends Intent {
  /// Creates the intent for [zoom].
  const TerminalZoomIntent(this.zoom);

  /// Which way the size changes.
  final TerminalZoom zoom;
}

/// The chords that zoom the text of the terminal (HUM-301).
///
/// **Zoom in** takes `+` with or without `Shift`: on a US layout `+` is
/// `Shift+=`, on a German one it has its own key, and the keypad has one of
/// its own. Flutter names a key by what it types without modifiers, so the US
/// chord arrives as `equal` with `Shift` and the German one as `add`.
///
/// **Zoom out** takes `-` only without `Shift`: `Ctrl+Shift+-` is `Ctrl+_`,
/// undo in readline, and stays with the agent.
///
/// **Reset is `Ctrl+Shift+0`, not `Ctrl+0`.** `Ctrl+0` is the reset of every
/// browser and terminal and here the toggle of the rail ([railToggleKey],
/// HUM-254), in every section and from inside the terminal as well. Two
/// meanings for one chord, one of them only while the terminal has the
/// keyboard, would leave whoever wants the rail with a smaller text and the
/// other way round. `Shift` with the same key keeps the reset next to the
/// digit people reach for; [SingleActivator] demands its modifiers exactly,
/// so the rail binding does not match it. On a German keyboard
/// `Shift+0` types `=`, but the key is still named `digit0`, so the chord is
/// the same there and is not mistaken for zoom in. `Ctrl` with the `0` of the
/// keypad resets as well, without `Shift`, because `Shift` turns that key
/// into `Insert` while Num Lock is on.
///
/// Zoom in and out repeat while held, the reset does not. Exactly `Ctrl` as
/// the other modifier: with `Alt` or `Meta` it is another chord. None of these
/// keys is bound anywhere else in the application with `Ctrl` (checked
/// against every `SingleActivator` in `app/lib` and `app/packages` and
/// CONVENTIONS 3.9). The terminal looks them up like the other keys of the
/// shell, so they never reach the agent.
class TerminalZoomActivator extends ShortcutActivator {
  /// Creates the activator for [zoom].
  const TerminalZoomActivator(this.zoom);

  /// The chord this activator stands for.
  final TerminalZoom zoom;

  @override
  Iterable<LogicalKeyboardKey> get triggers => switch (zoom) {
    TerminalZoom.zoomIn => const <LogicalKeyboardKey>[
      LogicalKeyboardKey.equal,
      LogicalKeyboardKey.add,
      LogicalKeyboardKey.numpadAdd,
    ],
    TerminalZoom.zoomOut => const <LogicalKeyboardKey>[
      LogicalKeyboardKey.minus,
      LogicalKeyboardKey.numpadSubtract,
    ],
    TerminalZoom.reset => const <LogicalKeyboardKey>[
      LogicalKeyboardKey.digit0,
      LogicalKeyboardKey.numpad0,
    ],
  };

  @override
  bool accepts(KeyEvent event, HardwareKeyboard state) {
    final bool down =
        event is KeyDownEvent ||
        (event is KeyRepeatEvent && zoom != TerminalZoom.reset);
    if (!down ||
        !triggers.contains(event.logicalKey) ||
        !state.isControlPressed ||
        state.isAltPressed ||
        state.isMetaPressed) {
      return false;
    }
    final bool shift = state.isShiftPressed;
    return switch (zoom) {
      TerminalZoom.zoomIn => true,
      TerminalZoom.zoomOut => !shift,
      TerminalZoom.reset =>
        event.logicalKey == LogicalKeyboardKey.digit0 ? shift : !shift,
    };
  }

  @override
  String debugDescribeKeys() => switch (zoom) {
    TerminalZoom.zoomIn => 'Ctrl + Plus / Equal / Numpad Add',
    TerminalZoom.zoomOut => 'Ctrl + Minus / Numpad Subtract',
    TerminalZoom.reset => 'Ctrl + Shift + 0 / Ctrl + Numpad 0',
  };
}

/// What a clipboard chord of the terminal does (HUM-313).
enum TerminalClipboard {
  /// Copy the selection. `Ctrl+Shift+C`, `Cmd+C` on Apple platforms.
  copy,

  /// Paste the clipboard as input for the agent. `Ctrl+Shift+V`, `Cmd+V` on
  /// Apple platforms.
  paste,
}

/// The chords that copy from and paste into the terminal (HUM-313).
///
/// **`Shift` is part of the chord, as in every terminal on Linux.** Plain
/// `Ctrl+C` is the interrupt of the agent (`0x03`) and plain `Ctrl+V` is
/// quoted insert in readline (`0x16`); both stay with the agent. On Apple
/// platforms the chords are `Cmd+C` and `Cmd+V`, because `Cmd` never reaches
/// a terminal program there.
///
/// **Not part of [shellShortcuts].** The chords act only while the emulator
/// has the keyboard: `TerminalPane._onKey` asks this activator itself, after
/// the shell's bindings. Outside the terminal `Ctrl+Shift+C` and
/// `Ctrl+Shift+V` stay free for whoever has the focus. None of the bindings
/// of the application uses `C` or `V` with `Ctrl` and `Shift` (checked against
/// every `SingleActivator` in `app/lib` and `app/packages` and CONVENTIONS
/// 3.9; `Ctrl+Shift+F` and `Ctrl+Shift+L` are the group decisions of the
/// queue, `Ctrl+Shift+Enter` the full window, `Ctrl+Shift+0` the zoom reset).
///
/// Exactly these modifiers: `Ctrl+Alt+Shift+C` is another chord. No repeats:
/// a held chord copies or pastes once; the terminal keeps the repeats and the
/// release of the key from the agent.
class TerminalClipboardActivator extends ShortcutActivator {
  /// Creates the activator for [action].
  const TerminalClipboardActivator(this.action);

  /// The chord this activator stands for.
  final TerminalClipboard action;

  LogicalKeyboardKey get _key => switch (action) {
    TerminalClipboard.copy => LogicalKeyboardKey.keyC,
    TerminalClipboard.paste => LogicalKeyboardKey.keyV,
  };

  @override
  Iterable<LogicalKeyboardKey> get triggers => <LogicalKeyboardKey>[_key];

  @override
  bool accepts(KeyEvent event, HardwareKeyboard state) {
    if (event is! KeyDownEvent || event.logicalKey != _key) {
      return false;
    }
    return switch (defaultTargetPlatform) {
      TargetPlatform.iOS || TargetPlatform.macOS =>
        state.isMetaPressed &&
            !state.isControlPressed &&
            !state.isShiftPressed &&
            !state.isAltPressed,
      _ =>
        state.isControlPressed &&
            state.isShiftPressed &&
            !state.isAltPressed &&
            !state.isMetaPressed,
    };
  }

  @override
  String debugDescribeKeys() => switch (action) {
    TerminalClipboard.copy => 'Ctrl + Shift + C (Cmd + C on Apple)',
    TerminalClipboard.paste => 'Ctrl + Shift + V (Cmd + V on Apple)',
  };
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
/// `Ctrl+Backquote` for the keyboard to the terminal (HUM-303), the zoom
/// chords of the terminal (HUM-301) and `Ctrl+Shift+P` for the floating
/// terminal (HUM-300).
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
  terminalFloatActivator: const ToggleTerminalFloatIntent(),
  const TerminalFocusActivator(): const FocusTerminalIntent(),
  for (final TerminalZoom zoom in TerminalZoom.values)
    TerminalZoomActivator(zoom): TerminalZoomIntent(zoom),
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
