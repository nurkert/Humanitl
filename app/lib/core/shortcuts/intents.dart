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

/// Expand or collapse the icon rail. `Ctrl+Shift+0` (HUM-254; `Ctrl+0`
/// until HUM-356).
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
/// are copy and paste in the terminal (HUM-313), `Ctrl+Shift+0` the rail
/// (HUM-356), `Ctrl+Shift+Enter` the whole window (HUM-298).
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

/// Open a further shell in the running sandbox, as a tab of the terminal.
/// `Ctrl+Shift+T` (HUM-314).
class OpenShellIntent extends Intent {
  /// Creates the intent.
  const OpenShellIntent();
}

/// The chord that opens a further shell (HUM-314).
///
/// `T` for tab, as in every terminal on Linux that has tabs. Checked against
/// every binding of the application (`app/lib`, `app/packages`, CONVENTIONS
/// 3.9): no binding uses `T`, with or without modifiers. The `Ctrl+Shift`
/// family of the terminal is `Enter` (HUM-298), `P` (HUM-300), `0` (HUM-301),
/// `C` and `V` (HUM-313); the queue has `Ctrl+Shift+F` and `Ctrl+Shift+L`.
///
/// **Only the terminal acts on it.** The action stands in the host of the
/// terminal (`SandboxTerminal`) and only while "+ Shell" is offered, so the
/// chord does nothing anywhere else and, without the capability, goes to the
/// emulator as before. The agent loses little while it is offered: the
/// emulator turns `Ctrl` with a letter into a control byte only without
/// `Shift`, so `Ctrl+T` (`0x14`, transpose in readline) stays with the agent,
/// and only an agent that asked for the kitty keyboard protocol could have
/// told `Ctrl+Shift+T` apart.
///
/// No repeats: a held chord would open a shell per repeat.
const SingleActivator openShellActivator = SingleActivator(
  LogicalKeyboardKey.keyT,
  control: true,
  shift: true,
  includeRepeats: false,
);

/// Show the next tab of the terminal ([delta] 1) or the previous one
/// ([delta] -1), around the end. `Ctrl+PageDown` and `Ctrl+PageUp` (HUM-314).
class SwitchTerminalTabIntent extends Intent {
  /// Creates the intent.
  const SwitchTerminalTabIntent(this.delta);

  /// 1 for the next tab, -1 for the previous one.
  final int delta;
}

/// The chords that switch between the tabs of the terminal (HUM-314).
///
/// The keys of every tabbed terminal and browser on Linux. No binding of the
/// application uses `PageUp` or `PageDown` (checked as for
/// [openShellActivator]). The action stands only while there is more than
/// one tab; with the agent's terminal alone the chords reach the agent as
/// `CSI 5;5~` and `CSI 6;5~`, as before. With repeats: holding the chord walks
/// through the tabs.
const SingleActivator previousTerminalTabActivator = SingleActivator(
  LogicalKeyboardKey.pageUp,
  control: true,
);

/// See [previousTerminalTabActivator].
const SingleActivator nextTerminalTabActivator = SingleActivator(
  LogicalKeyboardKey.pageDown,
  control: true,
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
  /// One step larger. `Ctrl+Alt` + `+`, `Ctrl+Alt+=`, `Ctrl+Alt` + numpad
  /// `+` (HUM-356; before, `Ctrl` alone).
  zoomIn,

  /// One step smaller. `Ctrl+Alt` + `-`, `Ctrl+Alt` + numpad `-`.
  zoomOut,

  /// Back to the default size. `Ctrl+Alt+0`, `Ctrl+Alt` + numpad `0`.
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

/// Die Tasten einer Zoom-Familie: größer, kleiner, zurück.
///
/// Dieselben Tasten für die Skalierung der Anwendung ([UiScaleActivator])
/// und für die Schrift des Terminals ([TerminalZoomActivator]); die beiden
/// unterscheiden sich nur im Modifikator.
///
/// **Größer** nimmt `+` mit oder ohne `Shift`: Auf einer US-Tastatur ist `+`
/// `Shift+=`, auf einer deutschen hat es eine eigene Taste, und der
/// Ziffernblock hat noch eine. Flutter benennt eine Taste nach dem, was sie
/// ohne Modifikator tippt; der US-Griff kommt als `equal` mit `Shift` an, der
/// deutsche als `add`.
///
/// **Kleiner** nimmt `-` nur ohne `Shift`: `Ctrl+Shift+-` ist `Ctrl+_`, Undo
/// in readline, und bleibt beim Agenten.
///
/// **Zurück** nimmt die `0` der Zahlenreihe und des Ziffernblocks, beide ohne
/// `Shift`: `Ctrl+Shift+0` ist die Leiste ([railToggleKey]), und `Shift`
/// macht aus der `0` des Ziffernblocks bei Num Lock `Insert`.
///
/// Größer und kleiner wiederholen sich beim Halten, zurück nicht.
enum _ZoomKeys {
  larger(<LogicalKeyboardKey>[
    LogicalKeyboardKey.equal,
    LogicalKeyboardKey.add,
    LogicalKeyboardKey.numpadAdd,
  ]),
  smaller(<LogicalKeyboardKey>[
    LogicalKeyboardKey.minus,
    LogicalKeyboardKey.numpadSubtract,
  ]),
  reset(<LogicalKeyboardKey>[
    LogicalKeyboardKey.digit0,
    LogicalKeyboardKey.numpad0,
  ]);

  const _ZoomKeys(this.keys);

  final List<LogicalKeyboardKey> keys;

  /// Ob [event] eine Taste dieser Familie ist, gedrückt oder (außer bei
  /// [reset]) wiederholt, mit einem `Shift`, das die Familie zulässt. Die
  /// übrigen Modifikatoren prüft der Aufrufer.
  bool matches(KeyEvent event, HardwareKeyboard state) {
    final bool down =
        event is KeyDownEvent || (event is KeyRepeatEvent && this != reset);
    if (!down || !keys.contains(event.logicalKey)) {
      return false;
    }
    return this == larger || !state.isShiftPressed;
  }
}

/// The chords that zoom the text of the terminal (HUM-301): `Ctrl+Alt` with
/// the keys of [_ZoomKeys].
///
/// **`Ctrl+Alt` since HUM-356.** `Ctrl` with `+`, `-` and `0` scales the
/// whole application now ([UiScaleActivator]), as in every browser; the
/// owner asked for exactly these chords on 2026-10-30. The scaling grows the
/// terminal text along with everything else, so the terminal zoom stays as
/// the finer, separate control, one modifier further. `Ctrl+Alt` with these
/// keys is bound nowhere else in the application (checked against every
/// activator in `app/lib` and `app/packages` and CONVENTIONS 3.9). What the
/// agent loses: a terminal sends `Ctrl+Alt+-` as `ESC` and `0x1f`, the meta
/// form of readline's undo; only that, and only while a terminal is shown.
///
/// Exactly `Ctrl` and `Alt`: with `Meta` it is another chord. The terminal
/// looks them up like the other keys of the shell, so they never reach the
/// agent.
class TerminalZoomActivator extends ShortcutActivator {
  /// Creates the activator for [zoom].
  const TerminalZoomActivator(this.zoom);

  /// The chord this activator stands for.
  final TerminalZoom zoom;

  _ZoomKeys get _keys => switch (zoom) {
    TerminalZoom.zoomIn => _ZoomKeys.larger,
    TerminalZoom.zoomOut => _ZoomKeys.smaller,
    TerminalZoom.reset => _ZoomKeys.reset,
  };

  @override
  Iterable<LogicalKeyboardKey> get triggers => _keys.keys;

  @override
  bool accepts(KeyEvent event, HardwareKeyboard state) =>
      state.isControlPressed &&
      state.isAltPressed &&
      !state.isMetaPressed &&
      _keys.matches(event, state);

  @override
  String debugDescribeKeys() => switch (zoom) {
    TerminalZoom.zoomIn => 'Ctrl + Alt + Plus / Equal / Numpad Add',
    TerminalZoom.zoomOut => 'Ctrl + Alt + Minus / Numpad Subtract',
    TerminalZoom.reset => 'Ctrl + Alt + 0 / Numpad 0',
  };
}

/// What a scale chord does to the whole application (HUM-356).
enum UiScaleStep {
  /// One step larger, 10 %. `Ctrl` + `+`, `Ctrl+=`, `Ctrl` + numpad `+`.
  zoomIn,

  /// One step smaller, 10 %. `Ctrl` + `-`, `Ctrl` + numpad `-`.
  zoomOut,

  /// Back to 100 %. `Ctrl+0`, `Ctrl` + numpad `0`.
  reset,
}

/// Make the whole application larger, smaller or 100 % again (HUM-356).
class UiScaleIntent extends Intent {
  /// Creates the intent for [step].
  const UiScaleIntent(this.step);

  /// Which way the scale changes.
  final UiScaleStep step;
}

/// The chords that scale the whole application (HUM-356): `Ctrl` with the
/// keys of [_ZoomKeys], on macOS `Cmd` as well.
///
/// **They belong to the application, also inside the terminal.** These are
/// the zoom chords of every browser and of the desktop, and terminal
/// programs hardly use them: `Ctrl+=` and `Ctrl+0` send nothing a TUI
/// reads, and `Ctrl+-` had been the terminal zoom since HUM-301 already. The
/// terminal looks them up like the other keys of the shell, so a TUI that
/// has the keyboard cannot swallow them; the setting and the palette work
/// too.
///
/// **`Ctrl+0` resets, and the rail moved to `Ctrl+Shift+0`.** Until HUM-356
/// `Ctrl+0` toggled the rail ([railToggleKey], HUM-254) and the terminal zoom
/// reset sat on `Ctrl+Shift+0`. The owner asked for the browser chords on
/// 2026-10-30; the rail keeps its digit and takes `Shift`, the terminal zoom
/// takes `Alt` ([TerminalZoomActivator]).
///
/// Exactly one of `Ctrl` or, on macOS, `Cmd`; no `Alt`. `Ctrl` and `Cmd`
/// together are another chord.
class UiScaleActivator extends ShortcutActivator {
  /// Creates the activator for [step]. [platform] defaults to
  /// [defaultTargetPlatform]; tests pass their own.
  const UiScaleActivator(this.step, {this.platform});

  /// The chord this activator stands for.
  final UiScaleStep step;

  /// The platform whose modifier counts, or null for the running one.
  final TargetPlatform? platform;

  _ZoomKeys get _keys => switch (step) {
    UiScaleStep.zoomIn => _ZoomKeys.larger,
    UiScaleStep.zoomOut => _ZoomKeys.smaller,
    UiScaleStep.reset => _ZoomKeys.reset,
  };

  @override
  Iterable<LogicalKeyboardKey> get triggers => _keys.keys;

  @override
  bool accepts(KeyEvent event, HardwareKeyboard state) {
    final bool mac =
        (platform ?? defaultTargetPlatform) == TargetPlatform.macOS;
    final bool control = state.isControlPressed;
    final bool meta = state.isMetaPressed;
    // Genau einer der beiden: `Ctrl`, oder auf macOS `Cmd`.
    final bool one = mac ? control != meta : control && !meta;
    if (!one || state.isAltPressed) {
      return false;
    }
    return _keys.matches(event, state);
  }

  @override
  String debugDescribeKeys() => switch (step) {
    UiScaleStep.zoomIn => 'Ctrl + Plus / Equal / Numpad Add',
    UiScaleStep.zoomOut => 'Ctrl + Minus / Numpad Subtract',
    UiScaleStep.reset => 'Ctrl + 0 / Numpad 0',
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
/// quoted insert in readline (`0x16`); both stay with the agent. One
/// exception (HUM-353): while visible text is selected, plain `Ctrl+C`
/// copies it instead (`TerminalPane._onKey`, not on Apple platforms). On Apple
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
/// queue, `Ctrl+Shift+Enter` the full window, `Ctrl+Shift+0` the rail).
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

/// The key that expands and collapses the rail, with `Ctrl+Shift` (HUM-254).
///
/// Not `Ctrl+B`: CONVENTIONS 3.9 keeps that for `AddRuleIntent`, and a plain
/// `B` blocks a held request, so a chord whose `Ctrl` came a moment late
/// would decide instead of changing the layout. The zero belongs to the key
/// family of the rail and is unbound without `Ctrl` in every section.
///
/// **With `Shift` since HUM-356.** `Ctrl+0` is the reset of the scaling now,
/// as in every browser ([UiScaleActivator]); the owner asked for it on
/// 2026-10-30. The rail keeps its digit; [SingleActivator] demands its
/// modifiers exactly, so neither chord matches the other. On a German
/// keyboard `Shift+0` types `=`, but the key is still named `digit0`, so the
/// chord is the same there. In the terminal `Ctrl+Shift+0` sends nothing a
/// TUI reads.
const LogicalKeyboardKey railToggleKey = LogicalKeyboardKey.digit0;

/// The chord of the rail (HUM-254, HUM-356): `Ctrl+Shift+0`.
const SingleActivator railToggleActivator = SingleActivator(
  railToggleKey,
  control: true,
  shift: true,
);

/// The bindings of HUM-019: one `Ctrl+<digit>` per section, `Ctrl+K`,
/// `Ctrl+,` for the settings (HUM-069), `Ctrl+Shift+0` for the rail (HUM-254)
/// and
/// `Ctrl+Shift+Enter` for the terminal over the whole window (HUM-298) and
/// `Ctrl+Backquote` for the keyboard to the terminal (HUM-303), the zoom
/// chords of the terminal (HUM-301) and `Ctrl+Shift+P` for the floating
/// terminal (HUM-300), `Ctrl+Shift+T` for a further shell and
/// `Ctrl+PageUp`/`Ctrl+PageDown` for its tabs (HUM-314), and `Ctrl` with
/// `+`, `-` and `0` for the scaling of the whole application (HUM-356).
Map<ShortcutActivator, Intent> shellShortcuts() =>
    withCommandOnMac(<ShortcutActivator, Intent>{
      for (int i = 0; i < navigationKeys.length; i++)
        SingleActivator(navigationKeys[i], control: true): NavIntent(i),
      const SingleActivator(LogicalKeyboardKey.keyK, control: true):
          const PaletteIntent(),
      const SingleActivator(LogicalKeyboardKey.comma, control: true):
          const SettingsIntent(),
      railToggleActivator: const ToggleRailIntent(),
      terminalFullWindowActivator: const ToggleTerminalFullWindowIntent(),
      terminalFullWindowNumpadActivator: const ToggleTerminalFullWindowIntent(),
      terminalFloatActivator: const ToggleTerminalFloatIntent(),
      const TerminalFocusActivator(): const FocusTerminalIntent(),
      for (final TerminalZoom zoom in TerminalZoom.values)
        TerminalZoomActivator(zoom): TerminalZoomIntent(zoom),
      openShellActivator: const OpenShellIntent(),
      previousTerminalTabActivator: const SwitchTerminalTabIntent(-1),
      nextTerminalTabActivator: const SwitchTerminalTabIntent(1),
      for (final UiScaleStep step in UiScaleStep.values)
        UiScaleActivator(step): UiScaleIntent(step),
    });

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

/// On macOS every `Ctrl` chord answers to `Cmd` as well (HUM-325).
///
/// The `Ctrl` form stays, so the hints that name it remain true there; `Cmd`
/// is what a Mac user presses first. [platform] defaults to
/// [defaultTargetPlatform]; tests pass their own.
Map<ShortcutActivator, Intent> withCommandOnMac(
  Map<ShortcutActivator, Intent> shortcuts, {
  TargetPlatform? platform,
}) {
  if ((platform ?? defaultTargetPlatform) != TargetPlatform.macOS) {
    return shortcuts;
  }
  return <ShortcutActivator, Intent>{
    ...shortcuts,
    for (final MapEntry<ShortcutActivator, Intent> entry in shortcuts.entries)
      if (entry.key case final SingleActivator chord when chord.control)
        SingleActivator(
          chord.trigger,
          meta: true,
          shift: chord.shift,
          alt: chord.alt,
          includeRepeats: chord.includeRepeats,
        ): entry.value,
  };
}
