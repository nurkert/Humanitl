/// The size of the text in the terminal of the sandbox (HUM-301).
///
/// The shell hears the chords (`Ctrl` + `+`, `-`, `Shift+0`) and the palette
/// offers the commands, the terminal pane hears the wheel and draws the text;
/// none of them may reach into another (ARCHITECTURE 5), so the size is a
/// note in the core between them, like the placement of the terminal
/// (`terminal_placement.dart`).
///
/// The size survives a restart in the small state file of the interface
/// (`UiStateFile`, HUM-254), read once and synchronously at the first build:
/// asynchronously the terminal would start at the default, report its
/// geometry to the daemon and jump a frame later, and the agent would redraw
/// twice. A write that does not go through leaves the size in memory for
/// this run.
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../shortcuts/intents.dart';
import '../ui_state/ui_state_file.dart';

/// The size the terminal has without a choice: the `mono13` of the tokens.
///
/// A constant and not a read of the theme, because the notifier needs it
/// before any widget asks; `the default is the size of mono13` in
/// `test/core/terminal_zoom_test.dart` keeps both equal.
const double terminalFontSizeDefault = 13;

/// The smallest size. Below it the cells of a TUI stop being readable.
const double terminalFontSizeMin = 10;

/// The largest size. Above it a pane of the Sandbox section holds too few
/// columns for the layout of pi or OpenCode.
const double terminalFontSizeMax = 28;

/// How much one chord, one wheel notch or one palette command changes.
const double terminalFontSizeStep = 1;

/// The key under which the size stands in the state file.
const String terminalFontSizeKey = 'terminalFontSize';

/// The font size of the terminal, in logical pixels.
final NotifierProvider<TerminalFontSize, double> terminalFontSizeProvider =
    NotifierProvider<TerminalFontSize, double>(TerminalFontSize.new);

/// The notifier behind [terminalFontSizeProvider].
class TerminalFontSize extends Notifier<double> {
  @override
  double build() {
    final Object? stored = ref
        .read(uiStateFileProvider)
        .read()[terminalFontSizeKey];
    if (stored is! num || !stored.isFinite) {
      return terminalFontSizeDefault;
    }
    return stored
        .toDouble()
        .clamp(terminalFontSizeMin, terminalFontSizeMax)
        .toDouble();
  }

  /// One step larger, up to [terminalFontSizeMax].
  void zoomIn() => _set(state + terminalFontSizeStep);

  /// One step smaller, down to [terminalFontSizeMin].
  void zoomOut() => _set(state - terminalFontSizeStep);

  /// Back to [terminalFontSizeDefault].
  void reset() => _set(terminalFontSizeDefault);

  /// Does what [zoom] names: the one entry for chords and palette.
  void apply(TerminalZoom zoom) => switch (zoom) {
    TerminalZoom.zoomIn => zoomIn(),
    TerminalZoom.zoomOut => zoomOut(),
    TerminalZoom.reset => reset(),
  };

  void _set(double size) {
    final double next = size
        .clamp(terminalFontSizeMin, terminalFontSizeMax)
        .toDouble();
    if (next == state) {
      return;
    }
    state = next;
    // The default is written as no choice at all, so that a later default
    // reaches whoever went back to it.
    ref
        .read(uiStateFileProvider)
        .setValue(
          terminalFontSizeKey,
          next == terminalFontSizeDefault ? null : next,
        );
  }
}

/// What the shell binds to [TerminalZoomIntent] (HUM-301).
///
/// Enabled only while [shown] says a terminal with a session is on the
/// screen. A disabled action lets `Shortcuts` answer `ignored`, so the chord
/// goes on to whoever else wants it; with no terminal to see, a zoom would
/// change a size nobody looks at. Inside the terminal the question does not
/// arise: an emulator that has the keyboard is shown, and
/// `TerminalPane._onKey` hands the chord to this action like the other keys
/// of the shell.
class TerminalZoomAction extends Action<TerminalZoomIntent> {
  /// Creates the action over [fontSize], enabled while [shown] holds.
  TerminalZoomAction({required this.fontSize, required this.shown});

  /// The notifier the action changes.
  final TerminalFontSize fontSize;

  /// Whether a terminal is shown now.
  final bool Function() shown;

  @override
  bool isEnabled(TerminalZoomIntent intent) => shown();

  @override
  Object? invoke(TerminalZoomIntent intent) {
    fontSize.apply(intent.zoom);
    return null;
  }
}
