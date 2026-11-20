/// The settings in a session: the overlay from below (HUM-404; before that
/// a sheet from the right, HUM-069), since HUM-535 one of the two hosts of
/// [SettingsPanel].
///
/// # Shape
///
/// A panel over three quarters of the window height, the full width but a
/// margin, the application dimmed behind it. What stands in it is the
/// panel's; the sheet draws only the frame: the dimmed application, the
/// border with its rounded top corners and the motion.
///
/// # Keys
///
/// The sheet holds the keyboard while it is open (`HLayerScope`) and gives
/// it back when it closes. `Esc` empties the search first and closes the
/// sheet after that, as does a click on the dimmed application and the
/// close button. The other keys are the panel's.
///
/// # Motion
///
/// It slides in from below over [HMotion.arrive] on the `enter` curve and
/// out on `exit`; under reduced motion it only fades.
library;

import 'dart:async';

import 'package:flutter/widgets.dart';

import '../../core/ui/ui.dart';
import '../../l10n/l10n.dart';
import 'settings_panel.dart';

/// The settings sheet of a session.
class SettingsSheet extends StatefulWidget {
  /// Creates the sheet; [onClose] asks to close it, [onClosed] reports
  /// that it has slid out and may be taken away.
  const SettingsSheet({
    required this.onClose,
    this.onClosed,
    this.closing = false,
    this.focusKey,
    this.focusSerial = 0,
    this.daemonRestart,
    this.onOpenSession,
    super.key,
  });

  /// Asks the owner to close the sheet; the owner then sets [closing].
  final VoidCallback onClose;

  /// Invoked once the sheet has slid out after [closing] turned true.
  final VoidCallback? onClosed;

  /// Whether the sheet is on its way out.
  final bool closing;

  /// See [SettingsPanel.focusKey].
  final String? focusKey;

  /// See [SettingsPanel.focusSerial].
  final int focusSerial;

  /// See [SettingsPanel.daemonRestart].
  final Widget? daemonRestart;

  /// See [SettingsPanel.onOpenSession].
  final VoidCallback? onOpenSession;

  /// The share of the window height the panel covers.
  static const double heightFraction = 0.75;

  /// The margin left and right of the panel.
  static const double sideMargin = HSpace.x4;

  @override
  State<SettingsSheet> createState() => _SettingsSheetState();
}

class _SettingsSheetState extends State<SettingsSheet>
    with SingleTickerProviderStateMixin {
  late final AnimationController _motion = AnimationController(
    vsync: this,
    duration: HMotion.arrive,
  )..addStatusListener(_motionChanged);

  late final CurvedAnimation _curve = CurvedAnimation(
    parent: _motion,
    curve: HMotion.enter,
    reverseCurve: HMotion.exit,
  );

  @override
  void initState() {
    super.initState();
    if (widget.closing) {
      _motion.value = 0;
    } else {
      unawaited(_motion.forward());
    }
  }

  @override
  void didUpdateWidget(SettingsSheet oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.closing != oldWidget.closing) {
      unawaited(widget.closing ? _motion.reverse() : _motion.forward());
    }
  }

  @override
  void dispose() {
    _curve.dispose();
    _motion.dispose();
    super.dispose();
  }

  void _motionChanged(AnimationStatus status) {
    if (status != AnimationStatus.dismissed || !widget.closing) {
      return;
    }
    // Nach dem Bild und nicht im Rückruf: Ohne Weg steht die Bewegung schon
    // im Bau des Bildes am Ende, und der Besitzer darf dann nicht bauen.
    WidgetsBinding.instance
      ..addPostFrameCallback((Duration _) {
        if (mounted && widget.closing && _motion.isDismissed) {
          widget.onClosed?.call();
        }
      })
      ..ensureVisualUpdate();
  }

  @override
  Widget build(BuildContext context) => SettingsPanel(
    onClose: widget.onClose,
    closing: widget.closing,
    focusKey: widget.focusKey,
    focusSerial: widget.focusSerial,
    daemonRestart: widget.daemonRestart,
    onOpenSession: widget.onOpenSession,
    frame: _frame,
  );

  /// The dimmed application, and the panel from below in its border.
  Widget _frame(BuildContext context, Widget panel, VoidCallback close) {
    final HTokens tokens = HTheme.of(context);
    final bool reduced = HReducedMotion.of(context);
    return HLayerScope(
      child: IgnorePointer(
        ignoring: widget.closing,
        child: Stack(
          fit: StackFit.expand,
          children: <Widget>[
            FadeTransition(
              opacity: _curve,
              child: Semantics(
                button: true,
                label: context.l10n.settingsClose,
                child: GestureDetector(
                  key: const Key('settings-dim'),
                  behavior: HitTestBehavior.opaque,
                  onTap: close,
                  child: ColoredBox(
                    color: HColorDerivation.fade(tokens.colors.bg0, 0.72),
                  ),
                ),
              ),
            ),
            LayoutBuilder(
              builder: (BuildContext context, BoxConstraints box) {
                final Widget framed = _border(tokens, panel);
                return Align(
                  alignment: Alignment.bottomCenter,
                  child: Padding(
                    padding: const EdgeInsets.symmetric(
                      horizontal: SettingsSheet.sideMargin,
                    ),
                    child: SizedBox(
                      height: box.maxHeight * SettingsSheet.heightFraction,
                      child: reduced
                          ? FadeTransition(opacity: _curve, child: framed)
                          : SlideTransition(
                              position: Tween<Offset>(
                                begin: const Offset(0, 1),
                                end: Offset.zero,
                              ).animate(_curve),
                              child: framed,
                            ),
                    ),
                  ),
                );
              },
            ),
          ],
        ),
      ),
    );
  }

  /// The border of the panel with its rounded top corners.
  Widget _border(HTokens tokens, Widget panel) => DecoratedBox(
    key: const Key('settings-overlay'),
    decoration: BoxDecoration(
      color: tokens.colors.bg1,
      border: Border.all(
        color: tokens.colors.lineStrong,
        width: HSize.hairline,
      ),
      borderRadius: const BorderRadius.vertical(
        top: Radius.circular(HRadius.card),
      ),
    ),
    child: ClipRRect(
      borderRadius: const BorderRadius.vertical(
        top: Radius.circular(HRadius.card),
      ),
      child: panel,
    ),
  );
}
