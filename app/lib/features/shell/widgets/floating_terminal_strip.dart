/// The slim title strip the floating terminal shrinks to while no corner of
/// the sections leaves room for its window (HUM-315).
///
/// Before HUM-315 the window stepped back without a trace, and the person
/// who had let it float saw it gone. The strip says it is still there: the
/// name of the sandbox (its profile, or "Terminal"), the tab that waits
/// behind it (HUM-314) and one button. Where it stands is decided by its
/// host, `FloatingTerminalLayer` (`placeTerminalStrip`), under the same rule
/// as the window: it never covers what is marked with `HKeepClear`. Its size
/// is a token, `HSize.terminalStrip`.
///
/// It holds no terminal: the strip neither resizes the session nor takes a
/// key, so the session keeps its size and nothing the agent draws is lost.
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/hover_label.dart';
import '../../../core/ui/shell_glyph.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../../sandbox/providers/sandbox_status_provider.dart';
import '../../sandbox/providers/terminal_tabs_provider.dart';
import '../../sandbox/widgets/terminal_tabs.dart' show terminalActiveTabLabel;

/// What the strip says: the profile of the sandbox, or "Terminal" without
/// one, and the tab that is active, such as "opencode · Shell 1".
String _stripTitle(WidgetRef ref, AppLocalizations l10n) {
  final String profile = ref.watch(
    sandboxStatusProvider.select(
      (AsyncValue<SandboxStatus> status) => status.value?.profile ?? '',
    ),
  );
  return l10n.shellTerminalStripTitle(
    profile.isEmpty ? l10n.shellTerminalStripFallback : profile,
    terminalActiveTabLabel(ref.watch(terminalTabsProvider), l10n),
  );
}

/// The strip of the running session: [FloatingTerminalStrip] with the
/// profile and the active tab of the session that runs.
class LiveFloatingTerminalStrip extends ConsumerWidget {
  /// Creates the strip; [onOpen] opens the terminal again.
  const LiveFloatingTerminalStrip({required this.onOpen, super.key});

  /// See [FloatingTerminalStrip.onOpen].
  final VoidCallback onOpen;

  @override
  Widget build(BuildContext context, WidgetRef ref) => FloatingTerminalStrip(
    title: _stripTitle(ref, context.l10n),
    onOpen: onOpen,
  );
}

/// The strip: name, active tab and the button that opens the terminal.
///
/// It draws [title] and nothing it would have to ask for, so a golden can
/// show it without a daemon; [LiveFloatingTerminalStrip] asks.
class FloatingTerminalStrip extends StatelessWidget {
  /// Creates the strip; [onOpen] opens the terminal again.
  const FloatingTerminalStrip({
    required this.title,
    required this.onOpen,
    super.key,
  });

  /// The name of the sandbox and the active tab.
  final String title;

  /// The strip or its button was pressed: the host opens the window in the
  /// next free corner, or puts the terminal back into its pane.
  final VoidCallback onOpen;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    // Dieselbe Fläche und Linie wie die Titelleiste des Fensters: Der Streifen
    // ist das Fenster, nur ohne Terminal darunter. Kein Schatten
    // (`docs/DESIGN.md`, Antireferenz 5).
    return Semantics(
      key: const Key('shell-terminal-strip'),
      container: true,
      label: l10n.shellTerminalStripLabel,
      child: MouseRegion(
        cursor: SystemMouseCursors.click,
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: onOpen,
          child: DecoratedBox(
            decoration: BoxDecoration(
              color: tokens.colors.bg2,
              borderRadius: BorderRadius.circular(tokens.radii.card),
              border: Border.all(color: tokens.colors.lineStrong),
            ),
            child: Padding(
              padding: EdgeInsets.only(left: tokens.spacing.x2),
              child: Row(
                children: <Widget>[
                  ExcludeSemantics(
                    child: ShellGlyphIcon(
                      ShellGlyph.pictureInPicture,
                      size: 14,
                      color: tokens.colors.fg1,
                    ),
                  ),
                  SizedBox(width: tokens.spacing.x2),
                  Expanded(
                    child: Text(
                      title,
                      key: const Key('shell-terminal-strip-title'),
                      style: tokens.typography.ui12.semibold.tinted(
                        tokens.colors.fg0,
                      ),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                  HoverLabel(
                    label: l10n.shellTerminalStripOpen,
                    side: HoverLabelSide.below,
                    child: HButton(
                      key: const Key('shell-terminal-strip-open'),
                      variant: HButtonVariant.ghost,
                      size: HButtonSize.sm,
                      semanticsLabel: l10n.shellTerminalStripOpen,
                      onPressed: onOpen,
                      child: ShellGlyphIcon(
                        ShellGlyph.maximize,
                        size: 14,
                        color: tokens.colors.fg1,
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}
