/// What the header bar does in place of a toolkit title bar (HUM-255): it is
/// the handle that moves the window, and it carries the window buttons.
///
/// The Linux runner sets a GTK header bar but never shows it, so neither GTK
/// nor, on GNOME, the compositor draws a title bar above the app. Every call
/// to the window goes through [WindowPort]; no widget here knows
/// `window_manager`.
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/ui/hover_label.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../../tray/desktop_ports.dart';
import '../../tray/providers/attention.dart';

/// Lays [child] over a surface that moves the window when dragged and
/// maximizes or restores it on a double click.
///
/// The surface lies *below* [child], as a sibling in a [Stack], and not
/// around it. A double-tap recognizer in an ancestor would join the gesture
/// arena of every control in [child] and hold each click there until
/// `kDoubleTapTimeout` has passed. Below, it only gets the pointer where
/// [child] lets it through: on empty space and on whatever [child] wraps in
/// an [IgnorePointer]. A control in [child] takes its own clicks and drags,
/// so a click on it never moves the window.
class WindowDragArea extends ConsumerWidget {
  /// Creates the area around [child].
  const WindowDragArea({required this.child, super.key});

  /// The content of the bar.
  final Widget child;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    WindowPort window() => ref.read(desktopPortsProvider).window;
    return Stack(
      fit: StackFit.passthrough,
      children: <Widget>[
        Positioned.fill(
          child: GestureDetector(
            key: const Key('window-drag-area'),
            behavior: HitTestBehavior.opaque,
            onPanStart: (DragStartDetails _) => window().startDragging(),
            onDoubleTap: () => window().toggleMaximize(),
          ),
        ),
        child,
      ],
    );
  }
}

/// Minimize, maximize or restore, and close: the window buttons at the right
/// end of the header bar.
///
/// They stay on every desktop. A tiling window manager may ignore minimize
/// and maximize, but GTK 3 cannot tell which one does, and guessing from
/// environment variables hides working buttons on desktops nobody listed
/// (HUM-255).
class WindowButtons extends ConsumerWidget {
  /// Creates the three buttons.
  const WindowButtons({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    WindowPort window() => ref.read(desktopPortsProvider).window;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        _WindowButton(
          key: const Key('window-minimize'),
          glyph: HGlyph.minus,
          label: l10n.shellWindowMinimize,
          onPressed: () => window().minimize(),
        ),
        SizedBox(width: tokens.spacing.x1),
        _WindowButton(
          key: const Key('window-maximize'),
          glyph: HGlyph.square,
          label: l10n.shellWindowMaximize,
          onPressed: () => window().toggleMaximize(),
        ),
        SizedBox(width: tokens.spacing.x1),
        // Der Weg des Fenstermanagers, nicht der des Tray-Menüs: `close`
        // schickt dasselbe `delete-event` wie das × eines Fenstermanagers.
        _WindowButton(
          key: const Key('window-close'),
          glyph: HGlyph.close,
          label: l10n.shellWindowClose,
          onPressed: () => window().close(),
        ),
      ],
    );
  }
}

/// The title bar of a screen that fills the window without the shell: the
/// splash and the setup screen of a cold start (HUM-255).
///
/// The shell has its own: its header bar is the handle and carries the
/// buttons. Every other screen that takes the whole window stands below this
/// bar, so the window can be moved and closed with the mouse on every screen,
/// and the shell never shows two bars.
class WindowBar extends StatelessWidget {
  /// Creates the bar.
  const WindowBar({super.key});

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return SizedBox(
      key: const Key('window-bar'),
      height: tokens.sizes.headerBar,
      child: WindowDragArea(
        child: Padding(
          padding: EdgeInsets.symmetric(horizontal: tokens.spacing.x3),
          child: const Row(children: <Widget>[Spacer(), WindowButtons()]),
        ),
      ),
    );
  }
}

/// [child] below a [WindowBar], filling the rest of the window.
class WindowFramed extends StatelessWidget {
  /// Frames [child].
  const WindowFramed({required this.child, super.key});

  /// The screen below the bar.
  final Widget child;

  @override
  Widget build(BuildContext context) => Column(
    children: <Widget>[
      const WindowBar(),
      Expanded(child: child),
    ],
  );
}

class _WindowButton extends StatelessWidget {
  const _WindowButton({
    required this.glyph,
    required this.label,
    required this.onPressed,
    super.key,
  });

  final HGlyph glyph;
  final String label;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) => HoverLabel(
    label: label,
    side: HoverLabelSide.below,
    child: HIconButton(
      glyph: glyph,
      semanticsLabel: label,
      onPressed: onPressed,
    ),
  );
}
