/// The settings as a tab of the start window (ADR-0024, HUM-535), the second
/// host of [SettingsPanel] beside the sheet of a session.
///
/// The tab draws only its frame, the background of a page. It has no close
/// button and no layer of its own: it stands beside the other tabs of the
/// start window, does not take the keyboard when it is built, and lets an
/// `Esc` on an empty search pass to the window around it. The start window
/// itself comes with HUM-537.
library;

import 'package:flutter/widgets.dart';

import '../../core/ui/ui.dart';
import 'settings_panel.dart';

/// The settings tab of the start window.
class SettingsTab extends StatelessWidget {
  /// Creates the tab.
  const SettingsTab({
    this.focusKey,
    this.focusSerial = 0,
    this.daemonRestart,
    super.key,
  });

  /// See [SettingsPanel.focusKey].
  final String? focusKey;

  /// See [SettingsPanel.focusSerial].
  final int focusSerial;

  /// See [SettingsPanel.daemonRestart].
  final Widget? daemonRestart;

  @override
  Widget build(BuildContext context) => SettingsPanel(
    autofocus: false,
    focusKey: focusKey,
    focusSerial: focusSerial,
    daemonRestart: daemonRestart,
    frame: _frame,
  );

  /// The background of a page; nothing to close.
  static Widget _frame(BuildContext context, Widget panel, VoidCallback _) =>
      ColoredBox(
        key: const Key('settings-tab'),
        color: HTheme.of(context).colors.bg1,
        child: panel,
      );
}
