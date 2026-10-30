/// The tabs above the sandbox terminal: the agent's, the further shells, and
/// the button that opens one (HUM-314).
///
/// Three pieces, each for one host of the terminal (`SandboxTerminal`):
///
/// * [TerminalTabStrip], the row under the banner. It stands only while
///   there is a choice: the daemon offers further shells and a session runs,
///   or a shell tab is open. Without `sandbox.exec` the terminal looks as it
///   did before this issue, to the pixel.
/// * [TerminalTabNotices], what the terminal says about its tabs, floating
///   over the emulator in the notice layer of HUM-302: the refusal of `Exec`
///   (`TERM_002` above all) and the one-time hint that the agent can read a
///   further shell.
/// * [TerminalTabActions], the actions behind `Ctrl+Shift+T` and
///   `Ctrl+PageUp`/`Ctrl+PageDown`. They stand only while they can act, so a
///   chord that has nothing to do reaches the agent as before.
///
/// Every tab shows the same `TerminalPane`; this file holds no second copy of
/// what the pane does (zoom, copy and paste, the keyboard under the pointer,
/// the refusal of OSC 52, the held chords).
library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/shortcuts/intents.dart';
import '../../../core/ui/diagnostic_severity.dart';
import '../../../core/ui/h_diagnostic_card.dart';
import '../../../core/ui/h_notice_layer.dart';
import '../../../core/ui/hover_label.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/terminal_provider.dart';
import '../providers/terminal_tabs_provider.dart';
import 'terminal_pane.dart' show terminalStripHeight;

/// Whether the strip stands: there is a choice to make or a tab to close.
bool terminalTabsShown(WidgetRef ref) {
  final TerminalTabsState tabs = ref.watch(terminalTabsProvider);
  return tabs.shells.isNotEmpty ||
      (tabs.sandboxId.isNotEmpty && ref.watch(shellsOfferedProvider));
}

/// The name of the tab that is active: "Agent", or "Shell n" (HUM-315).
///
/// What the title strip of the floating terminal names when it has no room
/// for the terminal itself. Without how a shell ended: the strip says which
/// tab waits behind it, the tab itself says the rest.
String terminalActiveTabLabel(TerminalTabsState tabs, AppLocalizations l10n) {
  for (final ShellTab tab in tabs.shells) {
    if (tab.shellId == tabs.active) {
      return l10n.sandboxTerminalTabShell(tab.number);
    }
  }
  return l10n.sandboxTerminalTabAgent;
}

/// Opens a further shell the size of the terminal that is shown.
void openShellFrom(WidgetRef ref) {
  final TerminalTabsState tabs = ref.read(terminalTabsProvider);
  if (tabs.sandboxId.isEmpty) {
    return;
  }
  // The size of the view the human looks at: every tab has the same one,
  // and the shell should not start at 80x24 and redraw a moment later.
  final TerminalSessionState shown = ref.read(
    terminalSessionProvider(tabs.sandboxId, shellId: tabs.active),
  );
  unawaited(
    ref
        .read(terminalTabsProvider.notifier)
        .openShell(
          cols: shown.terminal.viewWidth,
          rows: shown.terminal.viewHeight,
        ),
  );
}

/// The row of tabs under the banner of the terminal.
class TerminalTabStrip extends ConsumerWidget {
  /// Creates the strip.
  const TerminalTabStrip({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final TerminalTabsState tabs = ref.watch(terminalTabsProvider);
    final bool offered =
        tabs.sandboxId.isNotEmpty && ref.watch(shellsOfferedProvider);
    return Semantics(
      container: true,
      label: l10n.sandboxTerminalTabsLabel,
      child: Container(
        key: const Key('sandbox-terminal-tabs'),
        constraints: const BoxConstraints(minHeight: terminalStripHeight),
        color: tokens.colors.bg1,
        padding: EdgeInsets.symmetric(horizontal: tokens.spacing.x1),
        child: Row(
          children: <Widget>[
            Flexible(
              child: SingleChildScrollView(
                scrollDirection: Axis.horizontal,
                child: Row(
                  children: <Widget>[
                    _Tab(
                      tabKey: const Key('sandbox-terminal-tab-agent'),
                      label: l10n.sandboxTerminalTabAgent,
                      active: tabs.active.isEmpty,
                      onPressed: () =>
                          ref.read(terminalTabsProvider.notifier).select(''),
                    ),
                    for (final ShellTab tab in tabs.shells)
                      _ShellTab(
                        tab: tab,
                        sandboxId: tabs.sandboxId,
                        active: tabs.active == tab.shellId,
                      ),
                  ],
                ),
              ),
            ),
            if (offered) _NewShellButton(tabs: tabs),
          ],
        ),
      ),
    );
  }
}

/// The tab of one further shell, with its close glyph.
class _ShellTab extends ConsumerWidget {
  const _ShellTab({
    required this.tab,
    required this.sandboxId,
    required this.active,
  });

  final ShellTab tab;
  final String sandboxId;
  final bool active;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final AppLocalizations l10n = context.l10n;
    // Only how it ended: the rest of the session changes with every byte and
    // would rebuild the strip with it.
    final _ShellEnd end = ref.watch(
      terminalSessionProvider(
        sandboxId,
        shellId: tab.shellId,
      ).select(_ShellEnd.of),
    );
    final TerminalTabs tabs = ref.read(terminalTabsProvider.notifier);
    final int? exit = end.exitCode;
    final String label = exit != null
        ? l10n.sandboxTerminalTabShellExited(tab.number, exit)
        : end.gone
        ? l10n.sandboxTerminalTabShellGone(tab.number)
        : l10n.sandboxTerminalTabShell(tab.number);
    final String? endHint = exit != null
        ? l10n.sandboxTerminalTabShellExitedHint(exit)
        : end.gone
        ? l10n.sandboxTerminalTabShellGoneHint
        : null;
    final Widget button = _Tab(
      tabKey: ValueKey<String>('sandbox-terminal-tab-${tab.shellId}'),
      label: label,
      active: active,
      // An ended shell has nothing left to show but its last lines, and its
      // tab says how it ended: a click closes it.
      onPressed: endHint == null
          ? () => tabs.select(tab.shellId)
          : () => tabs.close(tab.shellId),
    );
    final String closeLabel = l10n.sandboxTerminalCloseShell(tab.number);
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        if (endHint == null)
          button
        else
          HoverLabel(label: endHint, side: HoverLabelSide.below, child: button),
        HoverLabel(
          label: closeLabel,
          side: HoverLabelSide.below,
          child: HIconButton(
            key: ValueKey<String>('sandbox-terminal-tab-close-${tab.shellId}'),
            glyph: HGlyph.close,
            size: 12,
            semanticsLabel: closeLabel,
            onPressed: () => tabs.close(tab.shellId),
          ),
        ),
      ],
    );
  }
}

/// How a shell ended, as far as its tab needs to know.
///
/// [gone] is a shell that ended without its exit code reaching this window:
/// it ended before the first attach and the daemon kept its terminal no
/// longer than its grace period (`FINISHED_RETENTION`, HUM-299), so `Open`
/// answered `IPC_006`. Without this the tab stayed "Shell 1" for ever, and
/// a click only showed the refusal again (review round 1).
@immutable
class _ShellEnd {
  const _ShellEnd({this.exitCode, this.gone = false});

  static _ShellEnd of(TerminalSessionState session) => _ShellEnd(
    exitCode: session.exitCode,
    gone:
        session.phase == TerminalPhase.refused &&
        session.diagnostic?.code == DiagnosticCodes.capabilityUnavailable,
  );

  final int? exitCode;
  final bool gone;

  @override
  bool operator ==(Object other) =>
      other is _ShellEnd && other.exitCode == exitCode && other.gone == gone;

  @override
  int get hashCode => Object.hash(exitCode, gone);
}

/// One tab: a quiet button with the mark of `SandboxTabs` under it.
class _Tab extends StatelessWidget {
  const _Tab({
    required this.tabKey,
    required this.label,
    required this.active,
    required this.onPressed,
  });

  final Key tabKey;
  final String label;
  final bool active;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Semantics(
      selected: active,
      child: Stack(
        children: <Widget>[
          Padding(
            padding: const EdgeInsets.only(bottom: HSize.selectionRail),
            child: HButton(
              key: tabKey,
              variant: HButtonVariant.ghost,
              size: HButtonSize.sm,
              onPressed: onPressed,
              child: Text(
                label,
                style: active
                    ? tokens.typography.ui12.semibold.tinted(tokens.colors.fg0)
                    : tokens.typography.ui12.tinted(tokens.colors.fg1),
              ),
            ),
          ),
          Positioned(
            left: tokens.spacing.x2,
            right: tokens.spacing.x2,
            bottom: 0,
            height: HSize.selectionRail,
            child: ColoredBox(
              color: active ? tokens.colors.accent : tokens.colors.bg1,
            ),
          ),
        ],
      ),
    );
  }
}

/// "+ Shell": opens a further shell; disabled with four open.
class _NewShellButton extends ConsumerWidget {
  const _NewShellButton({required this.tabs});

  final TerminalTabsState tabs;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final bool enabled = !tabs.full && !tabs.opening;
    final String label = tabs.full
        ? l10n.sandboxTerminalNewShellFull
        : l10n.sandboxTerminalNewShellHint;
    return HoverLabel(
      label: label,
      side: HoverLabelSide.below,
      child: HButton(
        key: const Key('sandbox-terminal-new-shell'),
        variant: HButtonVariant.ghost,
        size: HButtonSize.sm,
        semanticsLabel: label,
        leading: HGlyphIcon(
          HGlyph.plus,
          size: 12,
          color: enabled ? tokens.colors.fg1 : tokens.colors.fg2,
        ),
        onPressed: enabled ? () => openShellFrom(ref) : null,
        child: Text(
          l10n.sandboxTerminalNewShell,
          style: tokens.typography.ui12.tinted(
            enabled ? tokens.colors.fg1 : tokens.colors.fg2,
          ),
        ),
      ),
    );
  }
}

/// What the terminal says about its tabs, as notices over the emulator.
class TerminalTabNotices extends ConsumerWidget {
  /// Creates the notices; they take no room while there is nothing to say.
  const TerminalTabNotices({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final TerminalTabsState tabs = ref.watch(terminalTabsProvider);
    final bool hintSeen = ref.watch(shellHintSeenProvider);
    final Diagnostic? refusal = tabs.refusal;
    final AppLocalizations l10n = context.l10n;
    final HTokens tokens = HTheme.of(context);
    return HNoticeColumn(
      children: <Widget>[
        if (refusal != null)
          if (DiagnosticL10n.resolve(refusal, l10n)
              case final DiagnosticText text)
            HDiagnosticCard(
              key: const Key('sandbox-terminal-shell-refusal'),
              code: refusal.code,
              severityLabel: severityLabel(l10n, refusal.severity),
              color: severityColor(tokens, refusal.severity),
              title: text.title,
              // The daemon's own sentence; the application writes the title,
              // never the reason (`docs/UX.md` 4.4).
              why: text.cause,
              docsUrl: refusal.docsUrl,
              width: double.infinity,
              dense: true,
              dismiss: HNoticeDismiss(
                label: l10n.sandboxDiagnosticDismiss,
                buttonKey: const Key('sandbox-terminal-shell-refusal-dismiss'),
                onPressed: ref
                    .read(terminalTabsProvider.notifier)
                    .dismissRefusal,
              ),
            ),
        // Said once, when the first further shell is open (`docs/SECURITY.md`
        // 4): the shell runs as the agent's user in the agent's sandbox, and
        // the agent can read what is typed there.
        if (tabs.shells.isNotEmpty && !hintSeen)
          HNoticeCard(
            key: const Key('sandbox-terminal-shell-hint'),
            header: Text(
              l10n.sandboxTerminalShellHint,
              style: tokens.typography.ui12.tinted(tokens.colors.fg0),
            ),
            dismissLabel: l10n.sandboxTerminalShellHintDismiss,
            dismissKey: const Key('sandbox-terminal-shell-hint-dismiss'),
            onDismiss: ref.read(shellHintSeenProvider.notifier).dismiss,
            rail: tokens.state.held,
            child: const SizedBox.shrink(),
          ),
      ],
    );
  }
}

/// The actions behind the chords of the tabs, around the terminal of a host.
///
/// `TerminalPane` hands every chord of the shell to [Actions] before the
/// emulator sees it and leaves it to the agent when no action stands. So the
/// actions stand only while they can act: "+ Shell" while it is offered, the
/// switch while there are two tabs or more.
class TerminalTabActions extends ConsumerWidget {
  /// Wraps [child].
  const TerminalTabActions({required this.child, super.key});

  /// The terminal of the host.
  final Widget child;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final TerminalTabsState tabs = ref.watch(terminalTabsProvider);
    final bool offered =
        tabs.sandboxId.isNotEmpty && ref.watch(shellsOfferedProvider);
    return Actions(
      actions: <Type, Action<Intent>>{
        if (offered)
          OpenShellIntent: CallbackAction<OpenShellIntent>(
            onInvoke: (OpenShellIntent _) {
              openShellFrom(ref);
              return null;
            },
          ),
        if (tabs.shells.isNotEmpty)
          SwitchTerminalTabIntent: CallbackAction<SwitchTerminalTabIntent>(
            onInvoke: (SwitchTerminalTabIntent intent) {
              ref.read(terminalTabsProvider.notifier).step(intent.delta);
              return null;
            },
          ),
      },
      child: child,
    );
  }
}
